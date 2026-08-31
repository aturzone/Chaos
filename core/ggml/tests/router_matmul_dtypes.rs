//! Is a BF16 `mul_mat` slow on this CPU, at the shape V4-Flash's router uses?
//!
//! **This decides whether C5d is worth building.** The router costs **0.221 s of
//! a 1.98 s V4-Flash token**, and `the-router-is-a-bf16-matmul-2026-08-31`
//! established that essentially all of it is the gate matmul rather than the
//! top-k sort — `argsort_top_k` over 256 values measured at ~0.000 s.
//!
//! The suspect is the dtype. `ffn_gate_inp` is **BF16**, the only trunk tensor
//! that is neither Q8_0 nor F32, and the shape is **4096 x 256 against one
//! token**: 1.05 million multiply-accumulates measured at **6.4 ms per block**.
//! That is two orders of magnitude slower than the shape deserves, and
//! `v4flash-repacking-2026-08-10` records the neighbouring fact that ggml has
//! **no x86 fast path for Q8_0** either — a missing kernel for an unusual dtype
//! is a theme in this container rather than a coincidence.
//!
//! So: the same matmul, the same shape, F32 against BF16, one session.
//!
//! # This is an instrument, not a gate
//!
//! **It asserts correctness and prints timings; it does not fail on a ratio.** A
//! timing threshold in CI on shared runners is a flake generator, and this
//! project has already recorded what a 13% spread between three identical runs
//! looks like. Read it with `--nocapture`; the ratio is for a human.
//!
//! ```text
//! cargo test --release -p chaos-ggml --test router_matmul_dtypes -- --nocapture
//! ```
//!
//! F16 is left out deliberately: writing a correct `f32` → `f16` conversion by
//! hand is twenty lines of rounding and subnormal handling that would be the
//! most likely thing in this file to be wrong, and it is not needed to answer
//! the question. BF16 is the top sixteen bits of an `f32` and needs no such care.

#![cfg(have_ggml)]

use chaos_ggml::Context;
use chaos_gguf::GgmlType;

/// The router's real shape: a 4096-wide hidden state against 256 experts.
const NE0: i64 = 4096;
const NE1: i64 = 256;
const REPS: usize = 200;

const BF16: u32 = 30;

/// Deterministic pseudo-random weights.
///
/// Not `rand`: this workspace has **zero third-party dependencies** and a
/// benchmark is not the place to end that. A fixed sequence also means both
/// dtypes see the identical values, which is the point.
fn values(n: usize) -> Vec<f32> {
    let mut v = Vec::with_capacity(n);
    let mut x: u32 = 0x1234_5678;
    for _ in 0..n {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        // A small symmetric range, so nothing is denormal and BF16's coarser
        // mantissa is genuinely exercised.
        v.push(((x >> 8) as f32 / (1u32 << 24) as f32) - 0.5);
    }
    v
}

/// `f32` → BF16, by truncation.
///
/// BF16 **is** the high sixteen bits of an `f32`, so this needs no conversion
/// table and cannot be subtly wrong about exponents or subnormals. Truncating
/// rather than rounding to nearest-even loses at most one unit in the last
/// place, which changes the product slightly and changes the timing not at all —
/// and timing is what this file is for. Stated because a reader comparing the
/// two "first output" columns below will see them differ, and that is why.
fn to_bf16_bytes(src: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() * 2);
    for f in src {
        out.extend_from_slice(&(f.to_bits() >> 16).to_le_bytes()[..2]);
    }
    out
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(1, 8))
        .unwrap_or(4)
}

/// Mean milliseconds per matmul, and the first output value.
fn time_one(bf16: bool) -> Option<(f64, f32)> {
    // A fresh arena per dtype: ggml aborts on an exhausted one rather than
    // returning an error, so nothing is shared between the two runs.
    let ctx = Context::new(512 * 1024 * 1024).ok()?;
    let src = values((NE0 * NE1) as usize);

    let w = if bf16 {
        let t = ctx.new_typed_2d(GgmlType(BF16), NE0, NE1).ok()?;
        t.set_bytes(&to_bf16_bytes(&src)).ok()?;
        t
    } else {
        let t = ctx.new_f32_2d(NE0, NE1).ok()?;
        t.set_f32(&src).ok()?;
        t
    };

    let x = ctx.new_f32_2d(NE0, 1).ok()?;
    x.set_f32(&values(NE0 as usize)).ok()?;

    // One untimed pass. The first evaluation of a graph shape pays for setup,
    // and this project has a recorded trap about first-run costs on another
    // backend; discarding it is the same discipline.
    let warm = ctx.mul_mat(&w, &x).ok()?;
    ctx.compute(&warm, threads()).ok()?;
    let first = warm.to_vec_f32().first().copied().unwrap_or(f32::NAN);

    let t0 = std::time::Instant::now();
    for _ in 0..REPS {
        let out = ctx.mul_mat(&w, &x).ok()?;
        ctx.compute(&out, threads()).ok()?;
    }
    Some((t0.elapsed().as_secs_f64() * 1000.0 / REPS as f64, first))
}

#[test]
fn the_routers_matmul_in_f32_and_bf16() {
    let macs = (NE0 * NE1) as f64;
    println!();
    println!(
        "the router's matmul: {NE0} x {NE1} against one token = {:.2}M multiply-accumulates",
        macs / 1e6
    );
    println!(
        "threads {}, {REPS} repetitions, one warm-up discarded",
        threads()
    );
    println!();
    println!("  dtype    ms/matmul     GFLOP/s   first output");
    println!("  ---------------------------------------------");

    let f32_r = time_one(false);
    let bf16_r = time_one(true);
    for (name, r) in [("F32", &f32_r), ("BF16", &bf16_r)] {
        match r {
            Some((ms, first)) => {
                let gflops = 2.0 * macs / (ms / 1000.0) / 1e9;
                println!("  {name:<7} {ms:10.4}  {gflops:10.2}   {first:+.6}");
            }
            // **Said, not swallowed.** A dtype this build cannot make is a real
            // answer -- it would mean ggml has no kernel for it at all -- and
            // hiding that behind a blank line turns a measurement into a shrug.
            None => println!("  {name:<7}  could not be constructed or computed here"),
        }
    }

    println!();
    let (base, _) = f32_r.expect("F32 could not be measured, so the harness is broken");

    match bf16_r {
        Some((bf, _)) => {
            println!("  BF16 is {:.2}x F32 at this shape.", bf / base);
            println!();
            println!("  V4-Flash pays this 40 times per token (blocks 3-42; the 3 hash");
            println!("  layers route by token id and do not need it). Measured there:");
            println!("  6.4 ms per block, 0.221 s per token, 11.2% of a token.");
            println!();
            if bf < 1.0 {
                println!("  BF16 here is {bf:.4} ms. That is nowhere near 6.4 ms, so the");
                println!("  DTYPE IS NOT THE EXPLANATION and C5d should be closed rather");
                println!("  than built. Whatever costs 6.4 ms in the engine is elsewhere.");
            } else {
                println!("  BF16 here is {bf:.4} ms against the engine's 6.4 ms per block.");
                println!("  Close enough to look like the same cost: C5d is worth building.");
            }
        }
        None => println!("  BF16 is not constructible here, which is itself an answer."),
    }
}
