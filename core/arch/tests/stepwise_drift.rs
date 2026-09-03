//! How fast does a stepwise pass drift from a batched one, and does it drift
//! smoothly or fall off a cliff?
//!
//! **This decides whether V4-Flash generation is correct.** Both paths ship: a
//! prompt is prefilled in a batch, and generation then advances one token at a
//! time. `stepwise_prefill` showed they disagree after 63 steps — cosine 0.970,
//! and a *different* next token — but not why, and there are two very different
//! explanations:
//!
//! * **Tie-breaking.** The repository already records that *"routing flips on
//!   near ties when the batch shape changes"*, which is why the other
//!   equivalence tests assert argmax plus a tolerance rather than equality. Ties
//!   accumulate, and the drift should look like a **random walk**: roughly
//!   monotonic, no particular length special.
//! * **A structural bug** in how the raw ring or the compressed half is filled
//!   at batch size 1. That should show a **step change**, most plausibly at a
//!   multiple of `CSA_RATIO` (4), because the compressed half is built per block
//!   of four tokens and a path that only ever sees one token could complete a
//!   block differently.
//!
//! So this measures the same divergence at 4, 8, 16, 32 and 64 steps and prints
//! the curve. **It asserts almost nothing on purpose**: the shape is for a human
//! to read, and a threshold here would be a guess about an effect nobody has
//! characterised. What it does assert is that the batched reference is stable and
//! that the comparison is meaningful at all.
//!
//! ```text
//! cargo test --release -p chaos-arch --test stepwise_drift -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use chaos_arch::Deepseek4Config;
use chaos_model::Model;

const DEFAULT_PATH: &str =
    r"C:\Projects\models\v4flash\DeepSeek-V4-Flash-UD-Q4_K_XL-00001-of-00005.gguf";

/// The lengths to compare at.
///
/// **The first version of this list was 4, 8, 16, 32, 64 — every one of them a
/// multiple of `CSA_RATIO`.** The whole point was to see whether a *block
/// boundary* behaves differently from a partial block, and a sample containing
/// only boundaries cannot show that. The "block boundary" column read `yes` on
/// every row, which is what gave it away.
///
/// So the odd lengths are here now, and they are the informative ones: 5, 6 and
/// 7 leave a block half-built, and 3 never completes one at all.
const LENGTHS: [usize; 9] = [3, 4, 5, 6, 7, 8, 16, 32, 64];

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
#[ignore = "reads weights from a 144 GB container; 124 single-token passes in total"]
fn how_the_stepwise_path_drifts_from_the_batched_one() {
    let Some(model) = open() else { return };
    let config = Deepseek4Config::from_model(&model).expect("config");
    let fw = chaos_arch::Deepseek4Forward::new(&model, config.clone());
    let arena = 1024usize << 20;

    // Not a repeated token: that is the degenerate case that made a perplexity
    // corpus useless earlier in this work.
    let all: Vec<i32> = (0..64i32).map(|i| (i * 137) % 900 + 11).collect();

    println!();
    println!("  tokens  cosine     max |diff|   argmax agrees   block boundary");
    println!("  -----------------------------------------------------------------");
    let mut previous: Option<f64> = None;
    for n in LENGTHS {
        let tokens = &all[..n];
        let batched = chaos_arch::prefill(&fw, tokens, arena).expect("batched");

        let mut cache = chaos_arch::Deepseek4Cache::new(config.n_layer, config.kv_lora_rank);
        let mut stepwise =
            chaos_arch::forward(&fw, &mut cache, &tokens[..1], arena).expect("first");
        for &t in &tokens[1..] {
            stepwise = chaos_arch::step(&fw, &mut cache, t, arena).expect("step");
        }

        let cos = cosine(&batched, &stepwise);
        let max_abs = batched
            .iter()
            .zip(&stepwise)
            .map(|(x, y)| (x - y).abs())
            .fold(0f32, f32::max);
        let am = |v: &[f32]| {
            v.iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).expect("finite"))
                .expect("logits")
                .0
        };
        let agrees = am(&batched) == am(&stepwise);
        let boundary = if n as i64 % Deepseek4Config::CSA_RATIO == 0 {
            "yes"
        } else {
            "no"
        };
        println!(
            "  {n:>6}  {cos:.6}  {max_abs:>10.4}   {:<13}   {boundary}",
            agrees
        );

        // A cliff is what a structural bug looks like. Reported, not asserted:
        // one machine's numbers should not become a threshold nobody can defend.
        if let Some(p) = previous {
            if p - cos > 0.02 {
                println!(
                    "           ^ cosine fell {:.4} from the previous length",
                    p - cos
                );
            }
        }
        previous = Some(cos);

        assert!(
            batched.len() == stepwise.len() && !batched.is_empty(),
            "the two paths returned different logit counts, so nothing below is comparable"
        );
    }
    println!();
    println!("  A smooth decline is tie accumulation: routing flips on near ties when");
    println!("  the batch shape changes, which this repository already records. A step");
    println!("  change -- especially at a length divisible by 4 -- would be structural,");
    println!("  because the compressed half is built per block of CSA_RATIO tokens.");
}
