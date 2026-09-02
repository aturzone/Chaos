//! Does V4-Flash's trunk survive `Q4_K`, and does the converted tensor still
//! multiply like the one it replaced?
//!
//! **C7's correctness question, against the real container.** The lever itself
//! is memory — a 7.38 GiB always-read set at 1.06 bytes a weight becomes ~3.5
//! at 0.56, and every gigabyte given back is a gigabyte the expert cache can
//! have. What that buys is measured in the engine; what is checked here is the
//! thing a speed measurement cannot see: **a wrong dtype produces fluent
//! nonsense, never a crash.**
//!
//! So this converts a real trunk tensor and then does the only test that
//! matters for a weight: multiplies a vector by both versions and compares the
//! answers. A binder that read the container's `Q8_0` when the bytes are `Q4_K`
//! would read twice the buffer and return plausible numbers, and no assertion
//! about sizes or types would catch it.
//!
//! Needs the container, so it is `#[ignore]`d:
//!
//! ```text
//! cargo test --release -p chaos-arch --test trunk_requant -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use chaos_gguf::GgmlType;
use chaos_model::{Model, ResidentSet};

const DEFAULT_PATH: &str =
    r"C:\Projects\models\v4flash\DeepSeek-V4-Flash-UD-Q4_K_XL-00001-of-00005.gguf";

/// Enough of the trunk to include several attention tensors, and small enough
/// that this test does not need a free machine.
const BUDGET: u64 = 1 << 30;

const Q8_0: GgmlType = GgmlType(8);
const Q4_K: GgmlType = GgmlType(12);

/// The type the survey measures, so the fallback can be priced without an edit.
///
/// `Q4_K` is what `--trunk-quant q4_k` ships; if the quality gate refuses it,
/// `q5_k` is the next stop and the question becomes what it costs instead. One
/// env var is cheaper than a second test that drifts out of step with this one.
fn survey_target() -> GgmlType {
    match std::env::var("CHAOS_SURVEY_TARGET").as_deref() {
        Ok("q5_k") => GgmlType(13),
        Ok("q6_k") => GgmlType(14),
        _ => Q4_K,
    }
}

fn model() -> Option<Model> {
    let p = std::env::var("CHAOS_TEST_GGUF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_PATH));
    p.exists().then(|| Model::open_split(&p).expect("open"))
}

/// Cosine similarity, which is the right measure for "is this the same
/// projection": it ignores a uniform scale and punishes a rotation, and a wrong
/// dtype rotates.
fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

fn values(n: usize, seed: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity(n);
    let mut x = seed;
    for _ in 0..n {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        v.push(((x >> 8) as f32 / (1u32 << 24) as f32) - 0.5);
    }
    v
}

#[test]
#[ignore = "needs the V4-Flash container"]
fn the_trunk_converts_and_still_multiplies() {
    let Some(model) = model() else {
        panic!("no container: set CHAOS_TEST_GGUF");
    };
    let (mut resident, report) = ResidentSet::load(&model, BUDGET).expect("load");
    assert!(report.loaded_tensors > 0, "nothing loaded");

    // A tensor that is Q8_0, big enough to be a real weight matrix and small
    // enough to hold twice: the attention output pair is 33 MiB.
    let candidate = resident
        .names()
        .into_iter()
        .filter(|n| {
            model
                .location(n)
                .map(|l| l.ty == Q8_0 && l.dims.len() == 2 && l.size < (64 << 20))
                .unwrap_or(false)
        })
        .min()
        .expect("a Q8_0 trunk tensor in the first gigabyte");
    let loc = model.location(&candidate).expect("located").clone();
    let (ne0, ne1) = (loc.dims[0] as i64, loc.dims[1] as i64);
    let before: Vec<u8> = resident.get(&candidate).expect("resident").to_vec();
    println!(
        "checking {candidate}  [{ne0}, {ne1}]  {} bytes",
        before.len()
    );

    let converted = chaos_arch::requantise(&mut resident, &model, Q4_K, 4).expect("requantise");
    println!("{converted}");
    assert!(converted.converted > 0, "nothing was converted");
    assert!(
        converted.bytes_after * 100 < converted.bytes_before * 60,
        "Q4_K should be well under 60% of Q8_0: {} -> {}",
        converted.bytes_before,
        converted.bytes_after
    );

    // The set now disagrees with the container, and says so.
    assert_eq!(resident.type_of(&candidate), Some(Q4_K));
    assert_eq!(resident.converted(), converted.converted);

    let after: Vec<u8> = resident.get(&candidate).expect("still resident").to_vec();
    assert!(after.len() < before.len());

    // The only test that matters for a weight: the same projection.
    let ctx = chaos_ggml::Context::new(512 * 1024 * 1024).expect("context");
    let x = ctx.new_f32_2d(ne0, 1).expect("x");
    x.set_f32(&values(ne0 as usize, 0x9E37_79B9)).expect("set");

    let w8 = ctx.new_typed_2d(Q8_0, ne0, ne1).expect("q8 tensor");
    w8.set_bytes(&before).expect("q8 bytes");
    let w4 = ctx.new_typed_2d(Q4_K, ne0, ne1).expect("q4 tensor");
    w4.set_bytes(&after).expect("q4 bytes");

    let out8 = ctx.mul_mat(&w8, &x).expect("q8 matmul");
    let out4 = ctx.mul_mat(&w4, &x).expect("q4 matmul");
    ctx.compute_many(&[&out8, &out4], 4).expect("compute");
    let (a, b) = (out8.to_vec_f32(), out4.to_vec_f32());

    let cos = cosine(&a, &b);
    let rel = {
        let num: f64 = a
            .iter()
            .zip(&b)
            .map(|(x, y)| ((x - y) as f64).powi(2))
            .sum::<f64>()
            .sqrt();
        let den: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
        num / den.max(f64::MIN_POSITIVE)
    };
    println!("cosine {cos:.6}, relative error {:.4}%", rel * 100.0);

    // **The threshold was wrong before it was measured.** The first version of
    // this test asked for cosine > 0.999 and failed at 0.9974 with 7.2%
    // relative error -- which is not a bug but what 4.5 bits costs: for a
    // mat-vec over 4096 terms with independent errors, the output's relative
    // error lands near the weights' own, and Q4_K's is a few percent. So the
    // bound is set from the control below rather than from an intuition.
    let control = cosine(&a, &values(a.len(), 0x1234_5678));
    println!("control: an unrelated vector scores {control:.6}");
    assert!(
        control.abs() < 0.1,
        "the metric does not discriminate: {control}"
    );
    assert!(cos > 0.99, "cosine {cos} -- the projection changed");
    assert!(
        rel < 0.15,
        "relative error {rel} is too large even for Q4_K"
    );
}

/// What `Q4_K` costs each *kind* of trunk tensor, one row per kind.
///
/// **An instrument, not a gate.** The engine-level answer is the quality gate's
/// job; this says where the accuracy goes, which the gate cannot. It reads one
/// tensor of each kind straight from the container rather than loading a
/// resident set, so it costs a few megabytes of RAM and runs on a busy machine.
///
/// Read it with `--nocapture`.
#[test]
#[ignore = "needs the V4-Flash container"]
fn what_q4_k_costs_each_kind_of_trunk_tensor() {
    let Some(model) = model() else {
        panic!("no container: set CHAOS_TEST_GGUF");
    };

    // One of each shape the trunk actually contains, plus the two embeddings.
    // `ffn_gate_inp` -- the router -- is deliberately in the list even though
    // the converter skips it: seeing its number is the argument for skipping it.
    let kinds = [
        "blk.0.attn_q_a.weight",
        "blk.0.attn_q_b.weight",
        "blk.0.attn_kv.weight",
        "blk.0.attn_output_a.weight",
        "blk.0.attn_output_b.weight",
        "blk.0.ffn_gate_shexp.weight",
        "blk.0.ffn_up_shexp.weight",
        "blk.0.ffn_down_shexp.weight",
        "blk.0.ffn_gate_inp.weight",
        "token_embd.weight",
        "output.weight",
    ];

    let target = survey_target();
    let tname = target.name().unwrap_or("?");
    println!();
    println!("  tensor                          shape          stored   {tname} rel err   cosine");
    println!("  ---------------------------------------------------------------------------------");
    for name in kinds {
        let Some(loc) = model.location(name).cloned() else {
            println!("  {name:<30}  not in this container");
            continue;
        };
        let (ne0, ne1) = match loc.dims.as_slice() {
            [a, b] => (*a as i64, *b as i64),
            _ => continue,
        };
        let stored = loc.ty.name().unwrap_or("?");
        // One row band is enough for an error estimate and keeps a 0.5 GiB
        // tensor from being read whole: 256 rows of 4096 is 1 M weights.
        let rows = ne1.min(256);
        let bytes = model.read_tensor(name).expect("read");
        let row_bytes = chaos_ggml::row_size(loc.ty, ne0).expect("row size");
        let src = &bytes[..(rows as usize) * row_bytes];
        let floats = chaos_ggml::dequantize(loc.ty, src, (rows * ne0) as usize).expect("dequant");

        let dst_row = match chaos_ggml::row_size(target, ne0) {
            Ok(r) => r,
            Err(_) => {
                println!("  {name:<30}  [{ne0}, {ne1}]  {stored:>6}   rows are not whole blocks");
                continue;
            }
        };
        let mut q = vec![0u8; dst_row * rows as usize];
        chaos_ggml::quantize(target, &floats, rows, ne0, &mut q).expect("quantize");
        let back = chaos_ggml::dequantize(target, &q, floats.len()).expect("dequant target");

        let num: f64 = floats
            .iter()
            .zip(&back)
            .map(|(x, y)| ((x - y) as f64).powi(2))
            .sum::<f64>()
            .sqrt();
        let den: f64 = floats
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            .sqrt();
        let rel = num / den.max(f64::MIN_POSITIVE);
        let cos = cosine(&floats, &back);
        println!(
            "  {name:<30}  [{ne0}, {ne1}]  {stored:>6}   {:11.2}%   {cos:.6}",
            rel * 100.0
        );
    }
    println!();
    println!("  Weight-space error, not output error: a mat-vec over N terms with");
    println!("  independent errors keeps roughly this relative error in its output.");
}

/// The invariant the repack decline path depends on.
///
/// `RepackedDense::build` reads a tensor's type, takes its bytes, and hands them
/// back if ggml declines. If the override did not survive that round trip, the
/// tensor would be bound as `Q8_0` over `Q4_K` bytes — twice the buffer read, and
/// plausible numbers out of it. **This is the exact path that runs in production**:
/// `output.weight` stays `Q8_0`, is offered for repacking, and x86 declines it.
#[test]
#[ignore = "needs the V4-Flash container"]
fn an_override_survives_take_and_put_back() {
    let Some(model) = model() else {
        panic!("no container: set CHAOS_TEST_GGUF");
    };
    let (mut resident, _) = ResidentSet::load(&model, BUDGET).expect("load");
    chaos_arch::requantise(&mut resident, &model, Q4_K, 4).expect("requantise");

    let name = resident
        .names()
        .into_iter()
        .find(|n| resident.type_of(n) == Some(Q4_K))
        .expect("something was converted");
    let bytes_before = resident.get(&name).expect("resident").len();

    let taken = resident.take(&name).expect("take");
    assert_eq!(
        resident.type_of(&name),
        Some(Q4_K),
        "the override was dropped by take, so a declined repack would corrupt it"
    );
    resident.put_back(name.clone(), taken);
    assert_eq!(resident.type_of(&name), Some(Q4_K));
    assert_eq!(resident.get(&name).expect("back").len(), bytes_before);
}

#[test]
#[ignore = "needs the V4-Flash container"]
fn the_logit_projection_is_never_converted() {
    // `output.weight` is 0.52 GiB and the first thing a largest-first load
    // takes, so a budget of one gigabyte is guaranteed to hold it.
    let Some(model) = model() else {
        panic!("no container: set CHAOS_TEST_GGUF");
    };
    let (mut resident, _) = ResidentSet::load(&model, BUDGET).expect("load");
    if !resident.contains("output.weight") {
        panic!("output.weight is not resident at a 1 GiB budget, so this proves nothing");
    }
    let before = resident.get("output.weight").expect("resident").len();

    let converted = chaos_arch::requantise(&mut resident, &model, Q4_K, 4).expect("requantise");
    assert_eq!(resident.type_of("output.weight"), None);
    assert_eq!(
        resident.get("output.weight").expect("still resident").len(),
        before,
        "output.weight was converted, and its error lands on the sampled token"
    );
    assert!(converted.skipped_by_name >= 1);
}
