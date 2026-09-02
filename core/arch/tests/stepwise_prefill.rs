//! Does feeding V4-Flash one token at a time give the same distribution as
//! prefilling the batch?
//!
//! **This decides whether a 78% perplexity gap is the model or the harness.**
//! Measured 2026-09-02 on real prose at chunk 512, with the instrument validated
//! in the same session (Qwen3-4B against llama.cpp: −1.44%; Qwen3-30B-A3B:
//! +0.37%):
//!
//! ```text
//!                             ours     llama.cpp
//!   Qwen3-30B-A3B  (17 GiB)   25.93      25.83     agreement
//!   DeepSeek-V4-Flash (144 GB) 25.12      14.10     +78%
//! ```
//!
//! llama.cpp's pair is coherent — the far larger model predicts far better. Ours
//! says a 144 GB model is no better than a 30B one, which is not credible. So
//! either the forward pass is degraded in a way eight greedy prompts never
//! caught, or **the perplexity path is wrong** — and that path was written hours
//! before the number, so it is the first suspect.
//!
//! # Why this is not already covered
//!
//! `past_the_old_256_cap_a_cached_step_agrees_with_a_full_prefill` proves
//! `prefill(0..n) + step(n)`: **one** cached step after a batched prefill. The
//! perplexity path does something much stronger — token 0 through `forward`, then
//! *every* remaining token through `step`, so the batch shape is 1 for the whole
//! sequence. Nothing tested that, and on this architecture it is not obviously
//! equivalent: the raw latents live in a sliding ring and the compressed half is
//! built per pass, so a path that only ever sees one token at a time could fill
//! them differently and still read as fluent.
//!
//! # What it asserts
//!
//! Not bit-identity — routing flips on near ties when the batch shape changes,
//! which the other equivalence tests document. What matters for perplexity is the
//! **whole distribution**, so this compares cosine and the negative log
//! probability of a fixed target, and prints both. A distribution that has moved
//! enough to double a perplexity cannot hide behind a tolerance.
//!
//! ```text
//! cargo test --release -p chaos-arch --test stepwise_prefill -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use chaos_arch::Deepseek4Config;
use chaos_model::Model;

const DEFAULT_PATH: &str =
    r"C:\Projects\models\v4flash\DeepSeek-V4-Flash-UD-Q4_K_XL-00001-of-00005.gguf";

/// 64 tokens: enough that the compressed half is built and the stepwise path has
/// run 63 single-token passes, cheap enough to finish in about two minutes.
const N: usize = 64;

fn open() -> Option<Model> {
    let p = std::env::var("CHAOS_TEST_GGUF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_PATH));
    if p.exists() {
        return Some(Model::open_split(&p).expect("open"));
    }
    if std::env::var_os("CHAOS_REQUIRE_MODEL_TESTS").is_some() {
        panic!(
            "CHAOS_REQUIRE_MODEL_TESTS is set and no container is at {}",
            p.display()
        );
    }
    eprintln!(
        "SKIPPED: no container at {} -- set CHAOS_TEST_GGUF",
        p.display()
    );
    None
}

fn softmax_nll(logits: &[f32], target: usize) -> f64 {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let sum: f64 = logits.iter().map(|l| ((*l - max) as f64).exp()).sum();
    -(((logits[target] - max) as f64) - sum.ln())
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}

#[test]
#[ignore = "reads weights from a 144 GB container, 64 single-token passes"]
fn stepwise_and_batched_prefill_agree_on_the_distribution() {
    let Some(model) = open() else { return };
    let config = Deepseek4Config::from_model(&model).expect("config");
    let fw = chaos_arch::Deepseek4Forward::new(&model, config.clone());
    let arena = 1024usize << 20;

    // Deterministic ids inside any sane vocabulary, and not a repeated token:
    // a repeated token is exactly the degenerate case that made a corpus useless
    // earlier tonight.
    let tokens: Vec<i32> = (0..N as i32).map(|i| (i * 137) % 900 + 11).collect();

    // One batched pass over everything.
    let batched = chaos_arch::prefill(&fw, &tokens, arena).expect("batched prefill");

    // The same sequence, one token at a time -- exactly what the perplexity path
    // does: token 0 through `forward`, every later token through `step`.
    let mut cache = chaos_arch::Deepseek4Cache::new(config.n_layer, config.kv_lora_rank);
    let mut stepwise = chaos_arch::forward(&fw, &mut cache, &tokens[..1], arena).expect("first");
    for &t in &tokens[1..] {
        stepwise = chaos_arch::step(&fw, &mut cache, t, arena).expect("step");
    }
    assert_eq!(
        cache.n_past(),
        tokens.len(),
        "the cache consumed a different count"
    );
    assert_eq!(batched.len(), stepwise.len(), "logit count");

    let argmax = |v: &[f32]| {
        v.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).expect("finite logits"))
            .expect("logits")
            .0
    };
    let (ab, as_) = (argmax(&batched), argmax(&stepwise));
    let cos = cosine(&batched, &stepwise);
    // The perplexity-relevant quantity: how much the log probability of one
    // token moves. A doubling of perplexity is ~0.69 nats per token.
    let target = ab;
    let nll_b = softmax_nll(&batched, target);
    let nll_s = softmax_nll(&stepwise, target);
    let max_abs = batched
        .iter()
        .zip(&stepwise)
        .map(|(x, y)| (x - y).abs())
        .fold(0f32, f32::max);

    println!();
    println!("  batched argmax  {ab}");
    println!("  stepwise argmax {as_}");
    println!("  cosine          {cos:.6}");
    println!("  max |diff|      {max_abs:.4}");
    println!("  NLL of token {target}: batched {nll_b:.4} vs stepwise {nll_s:.4} nats");
    println!("  (a doubled perplexity is about 0.69 nats per token)");
    println!();

    assert!(
        cos > 0.99,
        "the stepwise distribution has rotated away from the batched one: cosine {cos}. \
         The perplexity path feeds one token at a time, so this would explain a large \
         perplexity gap without the weights being wrong."
    );
    assert!(
        (nll_b - nll_s).abs() < 0.2,
        "the log probability of the same token moved {:.4} nats between the two paths, \
         which is most of a doubling in perplexity",
        (nll_b - nll_s).abs()
    );
}
