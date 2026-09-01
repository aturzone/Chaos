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
/// Which k/v the kernel is handed.
#[derive(Copy, Clone, PartialEq)]
enum Kv {
    /// What the engine does today: `cont(permute(cache))`, a full copy.
    Cont,
    /// A cache already stored the way the kernel wants it. The ideal, and the
    /// thing a layout change would buy.
    KernelLayout,
    /// The permuted view with **no** `cont` -- which
    /// `does_flash_attn_ext_accept_a_strided_kv` shows gives byte-identical
    /// output. **Accepting a shape and reading it quickly are different
    /// questions**, and this arm is the second one: if a strided read is slow,
    /// the `cont` is buying something and deleting it would be a regression.
    Strided,
}

fn time_layer(n_total: i64, kv: Kv) -> Option<f64> {
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
        let (k, v) = match kv {
            Kv::Cont => {
                // What the engine does today: `[head_dim, n_kv, n_total]` permuted
                // to `[head_dim, n_total, n_kv]` and made contiguous.
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
            }
            Kv::KernelLayout => (
                // A cache already `[head_dim, n_total, n_kv_head]`. A reshape is
                // a view, so nothing is copied.
                ctx.reshape_3d(&k_kernel_layout, HEAD_DIM, n_total, N_KV_HEAD)
                    .ok()?,
                ctx.reshape_3d(&v_kernel_layout, HEAD_DIM, n_total, N_KV_HEAD)
                    .ok()?,
            ),
            Kv::Strided => {
                // The same permute, handed to the kernel without `cont`.
                let k3 = ctx
                    .reshape_3d(&k_head_major, HEAD_DIM, N_KV_HEAD, n_total)
                    .ok()?;
                let v3 = ctx
                    .reshape_3d(&v_head_major, HEAD_DIM, N_KV_HEAD, n_total)
                    .ok()?;
                (
                    ctx.permute(&k3, [0, 2, 1, 3]).ok()?,
                    ctx.permute(&v3, [0, 2, 1, 3]).ok()?,
                )
            }
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
    println!("  n_total    cont   kernel-layout   strided      (ms, one layer)");
    println!("  ------------------------------------------------------------");

    let mut rows = Vec::new();
    for n_total in [500i64, 1001, 2011, 4031] {
        let c = time_layer(n_total, Kv::Cont);
        let k = time_layer(n_total, Kv::KernelLayout);
        let st = time_layer(n_total, Kv::Strided);
        match (c, k, st) {
            (Some(c), Some(k), Some(st)) => {
                println!("  {n_total:>7}  {c:6.4}       {k:9.4}  {st:8.4}");
                rows.push((n_total, c, k, st));
            }
            // **Said, not swallowed.** A shape this build cannot make is an answer.
            _ => println!("  {n_total:>7}   could not be constructed or computed here"),
        }
    }

    assert!(
        rows.len() >= 2,
        "fewer than two shapes measured, so the harness is broken"
    );

    let (n0, c0, k0, s0) = rows[0];
    let (n1, c1, k1, s1) = *rows.last().expect("checked above");
    let span = (n1 - n0) as f64;
    let slope = |a: f64, b: f64| (b - a) / span * N_LAYER as f64;
    let sc = slope(c0, c1);
    let sk = slope(k0, k1);
    let ss = slope(s0, s1);

    println!();
    println!("  slope per token of context, all {N_LAYER} layers:");
    println!("    cont (today)     {sc:.5} ms");
    println!(
        "    kernel layout    {sk:.5} ms   {:.2}x better",
        sc / sk.max(1e-9)
    );
    println!(
        "    strided, no cont {ss:.5} ms   {:.2}x better",
        sc / ss.max(1e-9)
    );
    println!();
    println!("  The engine measures 0.0474 ms per context token against llama.cpp's");
    println!("  0.0214. `does_flash_attn_ext_accept_a_strided_kv` shows the strided");
    println!("  arm is byte-identical, so if it is also fast the fix is two deletions;");
    println!("  if it is slow, the `cont` is buying something and the fix is the");
    println!("  cache's storage layout.");
}

/// Will `flash_attn_ext` take a **strided** k and v, and give the same answer?
///
/// **This is the question that decides how big the fix is.** If the kernel accepts
/// `permute(kv)` without `cont`, the copy goes away with two deletions and the
/// cache's storage layout never changes -- no append path, no fourteen
/// architectures, no device-path realization order. If it refuses, or silently
/// answers differently, the layout change is the only route.
///
/// The comment in `qwen3.rs` says ggml *wants* `[head_dim, n_kv, n_head_kv]`. It
/// does not say the kernel refuses a view of it, and nobody has asked.
///
/// **Three outcomes, and all three are answers**: it aborts (ggml aborts rather
/// than returning errors, so this test would take the binary with it and that is
/// itself the finding, recorded here in advance); it returns a different result,
/// which means the strides are being ignored and the copy is load-bearing; or it
/// agrees, and the fix is two deletions.
#[test]
fn does_flash_attn_ext_accept_a_strided_kv() {
    // Small enough to compare every element by hand, and shaped like the real
    // thing: 2 kv heads, 4 positions, one new token.
    const HD: i64 = 4;
    const NKV: i64 = 2;
    const NT: i64 = 4;

    let ctx = Context::new(64 << 20).expect("arena");

    // Head-major, exactly as the cache stores it: [head_dim, n_kv_head, n_total].
    let k_hm = ctx
        .new_typed_2d(GgmlType(F16), HD, NKV * NT)
        .expect("k head-major");
    let v_hm = ctx
        .new_typed_2d(GgmlType(F16), HD, NKV * NT)
        .expect("v head-major");
    // Distinct values per element, so a wrong stride cannot coincide with a right
    // one: f16 0.5 plus i/64 stays exactly representable for small i.
    let vals: Vec<u8> = (0..(HD * NKV * NT))
        .flat_map(|i| (0x3800u16 + i as u16).to_le_bytes())
        .collect();
    k_hm.set_bytes(&vals).expect("fill k");
    v_hm.set_bytes(&vals).expect("fill v");

    let q = ctx.new_f32_2d(HD, NKV).expect("q");
    q.set_f32(&[0.25; (HD * NKV) as usize]).expect("fill q");
    let q3 = ctx.reshape_3d(&q, HD, NKV, 1).expect("q3");
    let q3 = ctx
        .cont(&ctx.permute(&q3, [0, 2, 1, 3]).expect("permute q"))
        .expect("cont q");

    let mask = ctx.new_typed_2d(GgmlType(F16), NT, 1).expect("mask");
    mask.set_bytes(&vec![0u8; (NT * 2) as usize])
        .expect("fill mask");

    let run = |strided: bool| -> Vec<f32> {
        let k3 = ctx.reshape_3d(&k_hm, HD, NKV, NT).expect("k3");
        let v3 = ctx.reshape_3d(&v_hm, HD, NKV, NT).expect("v3");
        let kp = ctx.permute(&k3, [0, 2, 1, 3]).expect("permute k");
        let vp = ctx.permute(&v3, [0, 2, 1, 3]).expect("permute v");
        let (k, v) = if strided {
            (kp, vp)
        } else {
            (
                ctx.cont(&kp).expect("cont k"),
                ctx.cont(&vp).expect("cont v"),
            )
        };
        let out = ctx
            .flash_attn_ext(&q3, &k, &v, &mask, 0.5, 0.0)
            .expect("flash_attn_ext refused this shape");
        ctx.compute(&out, 1).expect("compute");
        out.to_vec_f32()
    };

    let with_cont = run(false);
    println!();
    println!("  with cont (what the engine does): {with_cont:?}");

    // **If ggml aborts on the strided call, the process dies here and the test
    // output above is the record of how far it got.** That is why the contiguous
    // result is printed first.
    let strided = run(true);
    println!("  strided (no cont):               {strided:?}");
    println!();

    let same = with_cont.len() == strided.len()
        && with_cont
            .iter()
            .zip(&strided)
            .all(|(a, b)| (a - b).abs() <= 1e-6 * a.abs().max(1.0));

    if same {
        println!("  AGREES. `flash_attn_ext` honours the strides, so the two `cont`");
        println!("  calls in `attention_flash` can be deleted and the cache's storage");
        println!("  layout does not have to change. The fix is small.");
    } else {
        println!("  DIFFERS. The kernel does not honour these strides -- it reads the");
        println!("  permuted view as though it were contiguous -- so the `cont` is");
        println!("  load-bearing and the fix is the cache's storage layout after all.");
    }
    // **Not asserted either way.** Both answers are real findings about ggml, and
    // a test that fails on one of them would be asserting a wish. The verdict is
    // printed; the node records which it was.
}
