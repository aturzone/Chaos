//! Is the trunk slow because it is **Q8_0**, at the shape its matmuls actually use?
//!
//! **This decides whether C7 is worth building, and changes what C7 even is.**
//! `the-disk-is-no-longer-the-ceiling-2026-09-01` measured a V4-Flash generated
//! token as 0.815 s of disk and **0.478 s of arithmetic**, and put 88% of that
//! arithmetic in `tail` + `final compute`. The hyper-connection algebra named in
//! those phases cannot be the cost: `dsv4_hc_pre` collapses four 4096-wide streams
//! (16 K multiply-accumulates) and `dsv4_hc_comb` is a 4x4 Sinkhorn. Those are
//! rounding error.
//!
//! What is actually in those phases is the **dense trunk**: the Q/KV LoRA
//! projections, the attention output pair, and above all the **shared expert**,
//! which is always-read and runs every token at `[4096, 2048]` for gate, up and
//! down. That is ~25 M multiply-accumulates per block, 43 blocks, ~1.1 G a token.
//!
//! And `v4flash-repacking-2026-08-10` records the fact that makes this suspicious:
//! **"There is no x86 Q8_0 branch."** Every repackable trunk tensor in this
//! container is Q8_0, which is the one dtype ggml has no fast x86 path for.
//!
//! So: the same mat-vec, the same shape, F32 against BF16 against Q8_0, one
//! session. If Q8_0 is slower than F32 **despite carrying a quarter of the
//! bytes**, the kernel is the problem, C7 is worth building, and C7's real
//! argument is *"move the trunk to a dtype with a kernel"* rather than *"halve the
//! trunk's bytes"*.
//!
//! # This is an instrument, not a gate
//!
//! **It asserts the shapes and prints timings; it does not fail on a ratio.** A
//! timing threshold in CI on shared runners is a flake generator, and this project
//! has recorded a 13% spread between three identical runs. Read it with
//! `--nocapture`; the ratio is for a human.
//!
//! ```text
//! cargo test --release -p chaos-ggml --test trunk_mat_vec_dtypes -- --nocapture
//! ```

#![cfg(have_ggml)]

use chaos_ggml::Context;
use chaos_gguf::GgmlType;

/// The shared expert's shape: 4096 in, 2048 out, against **one** token.
///
/// One token is the point. A generated token is a matrix-*vector* product, which
/// is bound by how fast the weights can be read and decoded rather than by
/// arithmetic — and that is exactly where a missing kernel hurts most.
const NE0: i64 = 4096;
const NE1: i64 = 2048;
const REPS: usize = 100;

const BF16: u32 = 30;
const Q8_0: u32 = 8;

/// Q8_0 is 32 values per block: one `f16` scale, then 32 `i8`.
const QK8_0: usize = 32;

/// Deterministic pseudo-random values in a small symmetric range.
///
/// Not `rand`: this workspace has **zero third-party dependencies**. A fixed
/// sequence also means every dtype sees identical values, which is the point.
fn values(n: usize) -> Vec<f32> {
    let mut v = Vec::with_capacity(n);
    let mut x: u32 = 0x1234_5678;
    for _ in 0..n {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        v.push(((x >> 8) as f32 / (1u32 << 24) as f32) - 0.5);
    }
    v
}

/// `f32` → BF16 by truncation. BF16 **is** the high sixteen bits of an `f32`, so
/// this cannot be subtly wrong about exponents or subnormals.
fn to_bf16_bytes(src: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() * 2);
    for f in src {
        out.extend_from_slice(&(f.to_bits() >> 16).to_le_bytes()[..2]);
    }
    out
}

/// `f32` → Q8_0, with a **fixed power-of-two scale**.
///
/// Q8_0 blocks are `{ f16 d; i8 qs[32] }`. Writing a correct general `f32` → `f16`
/// conversion by hand is twenty lines of rounding and subnormal handling and would
/// be the most likely thing in this file to be wrong — so instead the scale is
/// pinned at **2^-6 = 0.015625**, whose `f16` encoding is exactly `0x2400` (sign 0,
/// exponent 9, mantissa 0) and needs no conversion at all. `values` produces
/// ±0.5, so ±0.5 / 0.015625 = ±32 quantises well inside `i8` with no clipping.
///
/// A single shared scale across every block is not what a real quantiser would
/// choose, and it does not matter here: **the kernel reads the same number of
/// bytes and does the same work whatever the scales are.** Stated because the
/// "first output" column below will differ from the F32 row, and this is why.
fn to_q8_0_bytes(src: &[f32]) -> Vec<u8> {
    const D_BITS: u16 = 0x2400; // f16 for 2^-6
    const D: f32 = 0.015625;
    assert!(
        src.len() % QK8_0 == 0,
        "Q8_0 needs a whole number of 32-value blocks"
    );
    let mut out = Vec::with_capacity(src.len() / QK8_0 * (2 + QK8_0));
    for block in src.chunks(QK8_0) {
        out.extend_from_slice(&D_BITS.to_le_bytes());
        for v in block {
            out.push((v / D).round().clamp(-127.0, 127.0) as i8 as u8);
        }
    }
    out
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(1, 8))
        .unwrap_or(4)
}

/// Mean milliseconds per mat-vec, the bytes the weight occupies, and the first
/// output value.
fn time_one(kind: &str) -> Option<(f64, usize, f32)> {
    // A fresh arena per dtype: ggml aborts on an exhausted one rather than
    // returning an error, so nothing is shared between runs.
    let ctx = Context::new(768 * 1024 * 1024).ok()?;
    let src = values((NE0 * NE1) as usize);

    let w = match kind {
        "F32" => {
            let t = ctx.new_f32_2d(NE0, NE1).ok()?;
            t.set_f32(&src).ok()?;
            t
        }
        "BF16" => {
            let t = ctx.new_typed_2d(GgmlType(BF16), NE0, NE1).ok()?;
            t.set_bytes(&to_bf16_bytes(&src)).ok()?;
            t
        }
        "Q8_0" => {
            let t = ctx.new_typed_2d(GgmlType(Q8_0), NE0, NE1).ok()?;
            t.set_bytes(&to_q8_0_bytes(&src)).ok()?;
            t
        }
        other => panic!("unknown dtype {other}"),
    };
    let bytes = w.bytes();

    let x = ctx.new_f32_2d(NE0, 1).ok()?;
    x.set_f32(&values(NE0 as usize)).ok()?;

    // One untimed pass. The first evaluation of a graph shape pays for setup.
    let warm = ctx.mul_mat(&w, &x).ok()?;
    ctx.compute(&warm, threads()).ok()?;
    let first = warm.to_vec_f32().first().copied().unwrap_or(f32::NAN);

    let t0 = std::time::Instant::now();
    for _ in 0..REPS {
        let out = ctx.mul_mat(&w, &x).ok()?;
        ctx.compute(&out, threads()).ok()?;
    }
    Some((
        t0.elapsed().as_secs_f64() * 1000.0 / REPS as f64,
        bytes,
        first,
    ))
}

#[test]
fn the_trunks_mat_vec_by_dtype() {
    let macs = (NE0 * NE1) as f64;
    println!();
    println!(
        "the shared expert's shape: {NE0} x {NE1} against one token = {:.2}M multiply-accumulates",
        macs / 1e6
    );
    println!(
        "threads {}, {REPS} repetitions, one warm-up discarded",
        threads()
    );
    println!();
    println!("  dtype    ms/mat-vec    weight MiB    GB/s decoded   first output");
    println!("  ------------------------------------------------------------------");

    let mut results = Vec::new();
    for name in ["F32", "BF16", "Q8_0"] {
        match time_one(name) {
            Some((ms, bytes, first)) => {
                let mib = bytes as f64 / (1 << 20) as f64;
                let gbs = bytes as f64 / (ms / 1000.0) / 1e9;
                println!("  {name:<7} {ms:11.4}  {mib:12.2}  {gbs:14.2}   {first:+.6}");
                results.push((name, ms, bytes));
            }
            // **Said, not swallowed.** A dtype this build cannot make is a real
            // answer, and hiding it behind a blank line turns a measurement into
            // a shrug.
            None => println!("  {name:<7}  could not be constructed or computed here"),
        }
    }

    println!();
    let f32_ms = results
        .iter()
        .find(|(n, _, _)| *n == "F32")
        .map(|(_, ms, _)| *ms)
        .expect("F32 could not be measured, so the harness is broken");

    for (name, ms, bytes) in &results {
        if *name == "F32" {
            continue;
        }
        let ratio = ms / f32_ms;
        let byte_ratio = results
            .iter()
            .find(|(n, _, _)| *n == "F32")
            .map(|(_, _, b)| *b as f64 / *bytes as f64)
            .unwrap_or(1.0);
        println!(
            "  {name} is {ratio:.2}x F32's time while carrying 1/{byte_ratio:.1} of its bytes."
        );
        if ratio > 1.0 {
            println!("    ** SLOWER despite reading less. That is a missing kernel, not");
            println!("       arithmetic, and it is the whole case for C7. **");
        }
    }

    println!();
    println!("  V4-Flash pays this 3 times per block for the shared expert alone");
    println!("  (gate, up, down), 43 blocks, every token. Measured in the engine:");
    println!("  0.478 s of arithmetic per generated token, 88% of it in the phases");
    println!("  these matmuls live in.");
}
