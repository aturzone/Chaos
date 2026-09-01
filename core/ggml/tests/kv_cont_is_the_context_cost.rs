//! Does `cont(permute(kv))` explain Chaos's 2.2x steeper context slope?
//!
//! `long-context-parity-qwen3-4b-2026-09-01` measured Chaos's per-token cost
//! rising **0.0474 ms per token of context** against llama.cpp's **0.0214** on
//! Qwen3-4B — both linear, so a constant factor of about 2.2x in whatever grows
//! with the KV cache. Flash attention, the KV dtype and the KV size are all ruled
//! out: both engines have the first and share the other two.
//!
//! What is *not* shared is this, in `qwen3.rs::attention_flash`:
//!
//! ```text
//! let k = ctx.cont(&ctx.permute(k_all, [0, 2, 1, 3])?)?;
//! let v = ctx.cont(&ctx.permute(v_all, [0, 2, 1, 3])?)?;
//! ```
//!
//! with the comment *"ggml wants `[head_dim, n_kv, n_head_kv]` … ours are
//! head-major, so permute"*. **`cont` materialises a contiguous copy of the whole
//! cache**, every layer, every token. llama.cpp stores its KV in the layout the
//! kernel wants and copies nothing.
//!
//! # Why an instrument and not a probe in the engine
//!
//! The engine's dense attention is shared by fourteen architectures and its graph
//! is realized through the device scheduler, so splitting it needs surgery on the
//! one path every model takes. This measures the same two shapes outside the
//! engine instead — the pattern that killed the BF16 hypothesis in
//! `router_matmul_dtypes` — and it can be wrong about the engine only by being
//! wrong about the shapes, which are printed by `chaos-run` itself:
//!
//! ```text
//! shape  36 layers, 2560 embd, 32 heads (8 kv), head_dim 128
//! ```
//!
//! ```text
//! cargo test --release -p chaos-ggml --test kv_cont_is_the_context_cost -- --nocapture
//! ```
//!
//! **It asserts shapes and prints timings; it does not fail on a ratio.**

#![cfg(have_ggml)]

use chaos_ggml::Context;
use chaos_gguf::GgmlType;

/// Qwen3-4B, from `chaos-run`'s own header.
const HEAD_DIM: i64 = 128;
const N_HEAD: i64 = 32;
const N_KV_HEAD: i64 = 8;
const N_LAYER: i64 = 36;
/// One generated token attending over everything before it.
const N_NEW: i64 = 1;

const F16: u32 = 1;
const REPS: usize = 20;

fn threads() -> usize {
    // Four, because that is what the engine uses to generate and what a `-t`
    // sweep confirmed is optimal at 500 and at 4031 tokens alike.
    4
}

/// Fill an F16 tensor with a fixed bit pattern.
///
/// The *values* do not matter: `cont` copies whatever is there and
/// `flash_attn_ext` reads the same bytes either way, so this measures the same
/// work as real data would. `0x3800` is f16 0.5 — finite, normal, and not a
/// denormal that some kernels special-case.
fn fill_f16(t: &chaos_ggml::Tensor<'_>) -> Option<()> {
    let n = t.bytes() / 2;
    let mut bytes = Vec::with_capacity(n * 2);
    for _ in 0..n {
        bytes.extend_from_slice(&0x3800u16.to_le_bytes());
    }
    t.set_bytes(&bytes).ok()
}

/// Milliseconds for one layer's attention at `n_total` cached positions, with and
/// without the `cont` copies.
fn time_layer(n_total: i64, with_cont: bool) -> Option<f64> {
    let ctx = Context::new(1536 * 1024 * 1024).ok()?;

    // The cache as Chaos stores it: head-major, `[head_dim, n_kv_head, n_total]`.
    let k_head_major = ctx
        .new_typed_2d(GgmlType(F16), HEAD_DIM, N_KV_HEAD * n_total)
        .ok()?;
    let v_head_major = ctx
        .new_typed_2d(GgmlType(F16), HEAD_DIM, N_KV_HEAD * n_total)
        .ok()?;
    fill_f16(&k_head_major)?;
    fill_f16(&v_head_major)?;

    let q = ctx.new_f32_2d(HEAD_DIM, N_HEAD * N_NEW).ok()?;
    q.set_f32(&vec![0.1f32; (HEAD_DIM * N_HEAD * N_NEW) as usize])
        .ok()?;
    let q3 = ctx.reshape_3d(&q, HEAD_DIM, N_HEAD, N_NEW).ok()?;
    let q3 = ctx.cont(&ctx.permute(&q3, [0, 2, 1, 3]).ok()?).ok()?;

    let mask = ctx.new_typed_2d(GgmlType(F16), n_total, N_NEW).ok()?;
    fill_f16(&mask)?;

    // **The kernel-layout cache is built once, outside the loop, and that matters.**
    // The first version of this test allocated and filled it *inside* the loop, so
    // that arm read a buffer it had just written -- partly in cache -- while the
    // `cont` arm read one written once and long since evicted. The bias favoured
    // the conclusion being tested, which is the worst direction for it to run in.
    // Now both arms read a buffer written once.
    let k_kernel_layout = ctx
        .new_typed_2d(GgmlType(F16), HEAD_DIM, n_total * N_KV_HEAD)
        .ok()?;
    let v_kernel_layout = ctx
        .new_typed_2d(GgmlType(F16), HEAD_DIM, n_total * N_KV_HEAD)
        .ok()?;
    fill_f16(&k_kernel_layout)?;
    fill_f16(&v_kernel_layout)?;

    // **No closure here, deliberately.** A closure returning a `Tensor` borrowed
    // from a captured `Context` cannot be written without fighting lifetimes, and
    // the fight is not worth a test: the graph is built inline, once per
    // iteration, which is also what the engine does per token.
    //
    // `REPS + 1` iterations: the first is the warm-up and is not timed, because
    // the first evaluation of a graph shape pays for setup.
    let mut elapsed = std::time::Duration::ZERO;
    for rep in 0..=REPS {
        let (k, v) = if with_cont {
            // What the engine does today: `[head_dim, n_kv, n_total]` permuted to
            // `[head_dim, n_total, n_kv]` and made contiguous.
            let k3 = ctx
                .reshape_3d(&k_head_major, HEAD_DIM, N_KV_HEAD, n_total)
                .ok()?;
            let v3 = ctx
                .reshape_3d(&v_head_major, HEAD_DIM, N_KV_HEAD, n_total)
                .ok()?;
            (
                ctx.cont(&ctx.permute(&k3, [0, 2, 1, 3]).ok()?).ok()?,
                ctx.cont(&ctx.permute(&v3, [0, 2, 1, 3]).ok()?).ok()?,
            )
        } else {
            // What a cache stored in the kernel's own layout would give: already
            // `[head_dim, n_total, n_kv_head]`, no copy. A reshape is a view.
            (
                ctx.reshape_3d(&k_kernel_layout, HEAD_DIM, n_total, N_KV_HEAD)
                    .ok()?,
                ctx.reshape_3d(&v_kernel_layout, HEAD_DIM, n_total, N_KV_HEAD)
                    .ok()?,
            )
        };
        let out = ctx
            .flash_attn_ext(&q3, &k, &v, &mask, 0.088_388_35, 0.0)
            .ok()?;
        let t0 = std::time::Instant::now();
        ctx.compute(&out, threads()).ok()?;
        if rep > 0 {
            elapsed += t0.elapsed();
        }
    }
    Some(elapsed.as_secs_f64() * 1000.0 / REPS as f64)
}

#[test]
fn the_cont_copies_against_a_cache_already_in_the_kernels_layout() {
    println!();
    println!("Qwen3-4B: head_dim {HEAD_DIM}, {N_HEAD} heads ({N_KV_HEAD} kv), {N_LAYER} layers");
    println!(
        "one generated token, {REPS} repetitions, {} threads",
        threads()
    );
    println!();
    println!("  n_total   with cont   without   difference   per token, x{N_LAYER} layers");
    println!("  ----------------------------------------------------------------------");

    let mut rows = Vec::new();
    for n_total in [500i64, 1001, 2011, 4031] {
        let with = time_layer(n_total, true);
        let without = time_layer(n_total, false);
        match (with, without) {
            (Some(a), Some(b)) => {
                println!(
                    "  {n_total:>7}   {a:9.4}  {b:8.4}   {:10.4}   {:8.2} ms",
                    a - b,
                    (a - b) * N_LAYER as f64
                );
                rows.push((n_total, a, b));
            }
            // **Said, not swallowed.** A shape this build cannot make is an answer.
            _ => println!("  {n_total:>7}   could not be constructed or computed here"),
        }
    }

    assert!(
        rows.len() >= 2,
        "fewer than two shapes measured, so the harness is broken"
    );

    println!();
    let (n0, a0, b0) = rows[0];
    let (n1, a1, b1) = *rows.last().expect("checked above");
    let span = (n1 - n0) as f64;
    let slope_with = (a1 - a0) / span * N_LAYER as f64;
    let slope_without = (b1 - b0) / span * N_LAYER as f64;
    println!("  slope per token of context, all {N_LAYER} layers:");
    println!("    with cont     {slope_with:.5} ms");
    println!("    without cont  {slope_without:.5} ms");
    if slope_without > 0.0 {
        println!("    ratio         {:.2}x", slope_with / slope_without);
    }
    println!();
    println!("  The engine measures 0.0474 ms per context token against llama.cpp's");
    println!("  0.0214 -- a 2.2x ratio. If the ratio above is near that, the `cont`");
    println!("  copies are the explanation and storing the cache in the kernel's own");
    println!("  layout is the fix. If it is near 1.0, they are not, and the 2.2x is");
    println!("  somewhere else in attention.");
}
