//! **Do the two paths choose the same experts?** That is what separates the two
//! live explanations of the stepwise/batched divergence, and it is a different
//! question from the one `stepwise_drift` asks.
//!
//! `stepwise_drift` measured the disagreement in the *logits* — the very end of
//! a 43-layer pass — which cannot tell these apart:
//!
//! * **A routing flip.** A near-tie in the router picks a different expert on
//!   one path, and a whole expert's contribution appears from nowhere. The
//!   compressed half would then differ only by floating-point reordering, and
//!   the 4.9 logit gap would be that rounding *amplified* by one wrong expert.
//! * **A structural difference** in what the compressor writes when a block is
//!   closed by the last of four single-token passes rather than from inside a
//!   batch. Then the experts would agree and the logits would still diverge.
//!
//! The engine already records the answer: `routing_last_token()` returns the
//! expert ids the **final** token of the most recent pass selected, per layer,
//! and `routing_last_token_reset()` exists so two passes can be compared
//! cleanly. Nothing new was needed to ask this — only noticing it was there.
//!
//! Four tokens, because that is the shortest length at which a compressed block
//! completes at all. Three is the control: the two paths agree there to cosine
//! 0.99987, and it is the only short length that closes no block.
//!
//! **Asserts almost nothing on purpose.** The comparison is for a human to read,
//! and a threshold here would be a guess about an effect nobody has
//! characterised — the mistake the first drift sweep made when it sampled only
//! multiples of `CSA_RATIO`.
//!
//! ```text
//! cargo test --release -p chaos-arch --test stepwise_layer_divergence \
//!   -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use chaos_arch::Deepseek4Config;
use chaos_model::Model;

const DEFAULT_PATH: &str =
    r"C:\Projects\models\v4flash\DeepSeek-V4-Flash-UD-Q4_K_XL-00001-of-00005.gguf";

/// The lengths to compare at.
///
/// **Two is the second control and it matters.** At two tokens *nothing* closes
/// a block anywhere in the stack — `CSA_RATIO` is 4 and `HCA_RATIO` is 128 — so
/// any layer that still differs there is diverging for a reason that has nothing
/// to do with the compressor. Layer 1 is `Raw`, and it differed at three tokens
/// too, which is what put this length on the list.
const LENGTHS: [usize; 3] = [2, 3, 4];

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
#[ignore = "reads weights from a 144 GB container; two prefills and seven steps"]
fn do_both_paths_choose_the_same_experts() {
    let Some(model) = open() else { return };
    let config = Deepseek4Config::from_model(&model).expect("config");
    let fw = chaos_arch::Deepseek4Forward::new(&model, config.clone());
    let arena = 1024usize << 20;

    // Not a repeated token: that is the degenerate case that made a perplexity
    // corpus useless earlier in this work.
    let all: Vec<i32> = (0..4i32).map(|i| (i * 137) % 900 + 11).collect();

    // **Needed to read the per-layer table at all.** A jump at a layer with no
    // compressor means something other than the compressed half; a jump at the
    // first CSA layer at four tokens and not at three means exactly it.
    let plan: Vec<String> = (0..10.min(config.n_layer))
        .map(|il| match config.attention_kind_from_ratio(il) {
            Some(k) => format!("{il}:{k:?}"),
            None => format!("{il}:?"),
        })
        .collect();
    println!();
    println!("  attention by layer: {}", plan.join("  "));
    println!(
        "  hash layers (no routed experts): 0..{}",
        config.hash_layer_count
    );

    for n in LENGTHS {
        let tokens = &all[..n];

        chaos_arch::routing_last_token_reset();
        let batched = chaos_arch::prefill(&fw, tokens, arena).expect("batched");
        let batched_routing = chaos_arch::routing_last_token();

        chaos_arch::routing_last_token_reset();
        let mut cache = chaos_arch::Deepseek4Cache::new(config.n_layer, config.kv_lora_rank);
        let mut stepwise =
            chaos_arch::forward(&fw, &mut cache, &tokens[..1], arena).expect("first");
        for &t in &tokens[1..] {
            stepwise = chaos_arch::step(&fw, &mut cache, t, arena).expect("step");
        }
        let stepwise_routing = chaos_arch::routing_last_token();

        let cos = cosine(&batched, &stepwise);
        let max_abs = batched
            .iter()
            .zip(&stepwise)
            .map(|(x, y)| (x - y).abs())
            .fold(0f32, f32::max);

        println!();
        println!("  === {n} tokens ===");
        println!(
            "  logits: cosine {cos:.6}, max |diff| {max_abs:.4}, block closed: {}",
            if n as i64 % Deepseek4Config::CSA_RATIO == 0 {
                "yes"
            } else {
                "no"
            }
        );

        // A layer with no routed experts records nothing, so compare only the
        // layers both paths logged. The three hash layers are expected empty.
        let layers = batched_routing.len().min(stepwise_routing.len());
        let mut differing = Vec::new();
        for il in 0..layers {
            let (mut a, mut b) = (batched_routing[il].clone(), stepwise_routing[il].clone());
            // `top_k` does not return indices in score order, so compare the
            // **set**. A profile built on selection order looked flat once for
            // a reason that had nothing to do with the model.
            a.sort_unstable();
            b.sort_unstable();
            if a != b {
                differing.push((il, a, b));
            }
        }
        println!(
            "  routing: {} layers logged, {} choose different experts",
            layers,
            differing.len()
        );
        for (il, a, b) in differing.iter().take(12) {
            println!("    layer {il:>2}: batched {a:?}  stepwise {b:?}");
        }
        if differing.len() > 12 {
            println!("    ... and {} more", differing.len() - 12);
        }
        if differing.is_empty() && layers > 0 {
            println!(
                "    every layer agrees -- so the logit gap is NOT a routing flip, \
                 and the compressor's values differ structurally"
            );
        }
    }

    println!();
    println!("  Read it this way: experts differing at 4 tokens but not at 3 makes");
    println!("  this a near-tie flipped by a rounding difference in the compressed");
    println!("  half. Experts agreeing at both, with the logits still 4.9 apart,");
    println!("  puts the difference in what the compressor writes.");
}
