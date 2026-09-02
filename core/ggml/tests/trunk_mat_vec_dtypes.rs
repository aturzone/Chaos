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
//! So: the same mat-vec, the same shape, F32 against BF16 against Q8_0 against
//! the two K-quants, one session. If Q8_0 were slower than F32 **despite carrying
//! a quarter of the bytes**, the kernel would be the problem and C7's argument
//! would be *"move the trunk to a dtype with a kernel"*.
//!
//! # It answered no, twice, and that is why C7 is a memory lever
//!
//! `Q8_0` is the **fastest** of F32/BF16/Q8_0 here, so the missing-kernel theory
//! is dead. And across four runs in one session `Q8_0` measured 0.211-0.252 ms
//! against `Q4_K`'s 0.196-0.228 — **overlapping ranges**, so converting the trunk
//! buys no arithmetic either. What it buys is 3.12 GiB of RAM, which is a
//! different kind of win and the one C7 actually ships:
//! `../../../docs/graph/research/requantising-the-trunk-2026-09-02.md`.
//!
//! **The repacked kernels are a separate matter and this file does not measure
//! them.** `Q8_0` has none on x86; `Q4_K` does, so the engine repacks 383 trunk
//! tensors after a conversion and those run a different kernel from anything
//! timed here.
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
const Q4_K: u32 = 12;
const Q5_K: u32 = 13;

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

/// `f32` → any quantised type, through **ggml's own quantiser**.
///
/// This used to be a hand-written Q8_0 packer with a fixed power-of-two scale,
/// because nothing in the workspace could quantise. `chaos_ggml::quantize` now
/// can, which is worth more than the shorter code: every row below is produced
/// the way a real container's rows were produced, so the **first output** column
/// is a like-for-like accuracy comparison rather than an artefact of a pinned
/// scale.
fn quantised(ty: u32, src: &[f32], ne0: i64, ne1: i64) -> Vec<u8> {
    let row = chaos_ggml::row_size(GgmlType(ty), ne0).expect("row size");
    let mut out = vec![0u8; row * ne1 as usize];
    chaos_ggml::quantize(GgmlType(ty), src, ne1, ne0, &mut out).expect("quantize");
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
        "Q8_0" | "Q4_K" | "Q5_K" => {
            let ty = match kind {
                "Q8_0" => Q8_0,
                "Q4_K" => Q4_K,
                _ => Q5_K,
            };
            let t = ctx.new_typed_2d(GgmlType(ty), NE0, NE1).ok()?;
            t.set_bytes(&quantised(ty, &src, NE0, NE1)).ok()?;
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
    for name in ["F32", "BF16", "Q8_0", "Q5_K", "Q4_K"] {
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
            println!("    ** SLOWER despite reading less. That would be a missing kernel");
            println!("       rather than arithmetic. No dtype here does that. **");
        }
    }

    println!();
    println!("  V4-Flash pays this 3 times per block for the shared expert alone");
    println!("  (gate, up, down), 43 blocks, every token, inside the 0.478 s of");
    println!("  arithmetic a generated token costs -- which is 40% attention and");
    println!("  40% expert matmuls (`what-is-inside-the-final-compute-2026-09-01`).");
    println!("  An earlier version of this line said \"88% of it in the phases these");
    println!("  matmuls live in\", from a phase timer that spanned the whole");
    println!("  attention graph. Retracted; the split above replaces it.");
}
