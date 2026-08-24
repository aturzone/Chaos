//! Does a negative prompt respond to *meaning*, or just to having text?
//!
//! # The change being checked
//!
//! Guidance here extrapolates away from a **separately trained unconditional
//! twin** that is fed no text at all. A negative prompt replaces that reference
//! with the *conditional* denoiser conditioned on the negative text — the same
//! 5.26 GiB weights, different conditioning. That is a change to which model
//! runs on every step, so it deserves a measurement rather than a screenshot.
//!
//! ```text
//! cargo run --release -p chaos-image --example negative-prompt-moves
//! cargo run --release -p chaos-image --example negative-prompt-moves 16 0.7 1,2,3,4
//! ```
//!
//! # What would show it working, and what would show it not
//!
//! One prompt, several references, the same latents. For each reference,
//! `1 - cos(v_cond, v_ref)` — how far the guidance has to push.
//!
//! - **The identity row must read 0.00%.** The negative text *is* the prompt,
//!   so the reference pass gets byte-identical inputs. Anything else means the
//!   conditioning is not reaching the model, and every other row is noise.
//! - **Meaning should outrank length.** A long, careful description of the same
//!   apple should stay *near* the prompt; a short phrase about a mountain should
//!   land far from it. If the ordering follows token count instead, the
//!   mechanism responds to how much text there is, not what it says.
//! - **Where the twin sits is the finding that matters to a user.** If no
//!   negative text ever lands as far from the prompt as the no-text twin does,
//!   then a negative prompt guides *more weakly* than leaving it alone — which
//!   is worth saying out loud in the help text rather than discovering.
//!
//! The last table is the practical one: how different the **step actually
//! taken** is, `1 - cos(guide(v_cond, v_twin), guide(v_cond, v_neg))` at the
//! default cfg. Two references that produce the same step produce the same
//! picture, whatever their individual cosines say.
//!
//! # Run it at grid 32 or larger. Not 16.
//!
//! **The first run of this that was not confounded was still wrong, because it
//! was run at grid 16.** That is 256x256 — *half* what `Request::default()`
//! draws — and the size `small-images-are-the-model-2026-08-24.md` measured the
//! denoiser to be 2.65x worse at before any prompt is involved.
//!
//! At grid 16 the two length-matched negatives sit 4.30% and 4.32% from the
//! prompt: one measurement twice, no subject ordering at all. At grid 32 they
//! sit at 0.47% and 0.86% and the ordering is the sensible one. **The length
//! effect falls 9x between the two sizes.** A conditioning question asked at
//! grid 16 gets an answer about grid 16.
//!
//! # What this cannot say
//!
//! Whether the picture is *better*. Nothing here scores beauty, and the
//! project has already published one claim that mistook a cosine for a verdict
//! (`research/prompt-shape-does-nothing-2026-08-24.md`). This says whether the
//! lever is connected, what it is connected to, and how hard it pulls.

use chaos_image::pipeline::Noise;
use chaos_image::{dit, flow, text, text::TextEncoder};
use chaos_model::Model;
use chaos_tokenizer::Tokenizer;

/// The prompt everything is measured against.
const PROMPT: &str = "a single red apple on a white table";

/// The same apple, written out. **Long and near** — the control that separates
/// "how much text" from "what the text says".
const SAME_IDEA_LONG: &str = "A single ripe red apple resting on a white table, \
photographed straight on in a bright studio, soft even lighting from above, a \
gentle contact shadow beneath it, glossy skin with a highlight on the upper \
left, a plain white backdrop behind, clean minimal product photography";

/// Conventional negative-prompt boilerplate. **Short and unrelated to the
/// subject** — what a user will actually type first.
const BOILERPLATE: &str = "blurry, low quality, distorted, deformed, ugly, watermark, text";

/// **Short and far.** If meaning drives this at all, this outranks the long one.
const DIFFERENT: &str = "a snowy mountain range at sunrise";

/// **Long and far** — the control the first run of this did not have.
///
/// Without it the table confounds two things. `SAME_IDEA_LONG` moved the
/// reference eleven times further than `DIFFERENT` did, which reads as "length
/// beats meaning" only if length is the *only* difference between them. It is
/// not: those two differ in length **and** in subject, in opposite directions.
///
/// This one is matched to `SAME_IDEA_LONG` in length and to `DIFFERENT` in
/// subject, so the pair separates them.
///
/// - If this lands far above `SAME_IDEA_LONG`, meaning drives it at equal
///   length and the first reading was wrong.
/// - If it lands beside `SAME_IDEA_LONG`, the measure is following how much
///   text there is, and a negative prompt's *content* is close to irrelevant.
const DIFFERENT_LONG: &str = "A snowy mountain range at sunrise, photographed from a high ridge in clear cold air, low golden light raking across the peaks, deep blue shadow in the valleys below, layered ridgelines receding into haze, a wind-scoured snowfield in the foreground, epic alpine landscape photography";

fn cos(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    let (a, b) = (&a[..n], &b[..n]);
    let dot: f64 = a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|y| (*y as f64).powi(2)).sum::<f64>().sqrt();
    dot / (na * nb)
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// A function rather than a closure: `Inputs` borrows the latent, and a closure
/// that returns a borrow of its own argument needs the lifetime written down.
fn inputs<'a>(
    x: &'a [f32],
    ctx: Option<&'a text::Encoded>,
    grid: i64,
    timestep: f32,
) -> dit::Inputs<'a> {
    dit::Inputs {
        latent: x,
        grid_w: grid,
        grid_h: grid,
        timestep,
        context: ctx.map_or(&[][..], |e| &e.hidden[..]),
        context_len: ctx.map_or(0, |e| e.tokens),
    }
}

fn main() {
    let grid: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);
    let sigma: f32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.7);
    // Eight, for the reason `structure-helps` records: one latent reported a
    // fourteen-fold effect that a second latent reported as three-fold the
    // other way. One seed is not a measurement.
    let seeds: Vec<u64> = match std::env::args().nth(3) {
        Some(list) => list
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect(),
        None => (1..=8).collect(),
    };
    let cfg: f32 = 4.0;

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let dir = std::path::Path::new(&home).join(".chaos").join("models");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    // **A fourth argument `long` keeps only the three that decide it.**
    //
    // The verdict rests on two references matched in length and opposite in
    // subject, and everything here is measured at grid 16 -- the size
    // `small-images-are-the-model-2026-08-24.md` found the denoiser is
    // **2.65x worse at** than grid 64, before any prompt is involved. A
    // conclusion about conditioning taken only at the model's worst resolution
    // is a conclusion about that resolution.
    //
    // Checking it at grid 32 costs 4x per forward, so the full seven-reference
    // sweep is not the thing to repeat: three references and three seeds are.
    let only_long = std::env::args().nth(4).as_deref() == Some("long");
    let negatives: Vec<(&str, &str)> = if only_long {
        vec![
            // The baseline: with the negative equal to the prompt, `guide`
            // returns `v_cond`, so its row in the step table is the distance
            // between the twin's step and NO guidance at all.
            ("the prompt itself", PROMPT),
            // The one a person actually types, and the only short reference
            // worth the 4x cost of a bigger grid.
            ("boilerplate", BOILERPLATE),
            ("same idea, long", SAME_IDEA_LONG),
            ("different, long", DIFFERENT_LONG),
        ]
    } else {
        vec![
            ("the prompt itself", PROMPT),
            ("green not red", "a single green apple on a white table"),
            ("boilerplate", BOILERPLATE),
            ("a different picture", DIFFERENT),
            ("same idea, long", SAME_IDEA_LONG),
            ("different, long", DIFFERENT_LONG),
        ]
    };

    // -- conditioning, in its own scope so 4.68 GiB closes before 5.26 opens --
    let (prompt_ctx, neg_ctx): (text::Encoded, Vec<(&str, usize, text::Encoded)>) = {
        let model = Model::open_split(dir.join("Qwen3-VL-8B-Instruct-Q4_K_M.gguf"))
            .expect("open the text encoder");
        let tok = Tokenizer::from_metadata(model.metadata()).expect("tokenizer");
        let enc = TextEncoder::open(model, threads).expect("text encoder");
        let run = |p: &str| {
            let ids = tok.encode(&text::wrap_prompt(p));
            enc.encode(&ids, &mut |_, _| {}).expect("encode")
        };
        let p = run(PROMPT);
        println!("prompt   {:>4} tokens  {PROMPT:?}", p.tokens);
        let mut v = Vec::new();
        for (name, txt) in negatives.iter().copied() {
            let e = run(txt);
            println!("negative {:>4} tokens  {name}", e.tokens);
            v.push((name, e.tokens, e));
        }
        (p, v)
    };

    let c = dit::Config::default();
    let n = (grid * grid * c.in_channels) as usize;
    let t = flow::timestep_for(sigma);

    println!();
    println!("latent   {grid}x{grid} grid, sigma {sigma}, timestep {t:.0}, cfg {cfg}");
    println!("seeds    {seeds:?}");
    println!();
    println!("How far the guidance has to push: 1 - cos(v_cond, v_reference).");
    println!();

    let latents: Vec<Vec<f32>> = seeds.iter().map(|&s| Noise::seeded(s).normals(n)).collect();

    // **The twin first, then dropped.** Both denoisers are 5.26 GiB and this
    // machine has 15.7, so holding them together would be 10.5 GiB resident to
    // save re-opening one file. A velocity at grid 16 is 131 KB; keeping eight
    // of those and closing the model is the obvious trade.
    let v_twins: Vec<Vec<f32>> = {
        let twin = dit::Denoiser::open(
            Model::open_split(dir.join("ideogram4_uncond-Q4_0.gguf")).expect("open the twin"),
            threads,
        );
        latents
            .iter()
            .map(|x| twin.forward(&inputs(x, None, grid, t)).expect("forward"))
            .collect()
    };

    let cond = dit::Denoiser::open(
        Model::open_split(dir.join("ideogram4-Q4_0.gguf")).expect("open the denoiser"),
        threads,
    );

    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); neg_ctx.len() + 1];
    let mut steps: Vec<Vec<f64>> = vec![Vec::new(); neg_ctx.len()];
    // **Distance from the prompt is one projection, and it can hide the
    // answer.** Two references the same distance from `v_cond` can point in
    // opposite directions, and guidance uses the direction. If the two
    // length-matched negatives -- the same apple and a mountain -- sit apart
    // from *each other* while sitting the same distance from the prompt, then
    // content does move the model and the column above was the wrong number to
    // read. This is that check.
    let mut pairwise: Vec<Vec<f64>> = vec![Vec::new(); neg_ctx.len() * neg_ctx.len()];

    print!("{:>6}", "seed");
    print!(" {:>14}", "twin (no text)");
    for (name, _, _) in &neg_ctx {
        print!(" {name:>20}");
    }
    println!();

    for (s_i, &seed) in seeds.iter().enumerate() {
        let x = &latents[s_i];
        let v_cond = cond
            .forward(&inputs(x, Some(&prompt_ctx), grid, t))
            .expect("forward");
        let v_twin = &v_twins[s_i];
        let guided_twin = flow::guide(&v_cond, v_twin, cfg);

        print!("{seed:>6}");
        let e = (1.0 - cos(&v_cond, v_twin)) * 100.0;
        columns[0].push(e);
        print!(" {e:>13.2}%");

        let mut v_negs: Vec<Vec<f32>> = Vec::with_capacity(neg_ctx.len());
        for (i, (_, _, ctx)) in neg_ctx.iter().enumerate() {
            let v_neg = cond
                .forward(&inputs(x, Some(ctx), grid, t))
                .expect("forward");
            let e = (1.0 - cos(&v_cond, &v_neg)) * 100.0;
            columns[i + 1].push(e);
            print!(" {e:>19.2}%");

            let guided_neg = flow::guide(&v_cond, &v_neg, cfg);
            steps[i].push((1.0 - cos(&guided_twin, &guided_neg)) * 100.0);
            v_negs.push(v_neg);
        }
        for i in 0..v_negs.len() {
            for j in 0..v_negs.len() {
                pairwise[i * v_negs.len() + j].push((1.0 - cos(&v_negs[i], &v_negs[j])) * 100.0);
            }
        }
        println!();
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    println!();
    println!(
        "{:<22} {:>8} {:>8} {:>8} {:>8}",
        "reference", "tokens", "mean", "min", "max"
    );
    let row = |name: &str, tokens: String, v: &[f64]| {
        let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        println!(
            "{name:<22} {tokens:>8} {:>7.2}% {lo:>7.2}% {hi:>7.2}%",
            mean(v)
        );
    };
    row("twin (no text)", "-".into(), &columns[0]);
    for (i, (name, tokens, _)) in neg_ctx.iter().enumerate() {
        row(name, tokens.to_string(), &columns[i + 1]);
    }

    println!();
    println!("The step actually taken, against the twin's step at cfg {cfg}:");
    println!("1 - cos(guide(v_cond, v_twin), guide(v_cond, v_neg)).");
    println!();
    for (i, (name, _, _)) in neg_ctx.iter().enumerate() {
        println!("{name:<22} {:>7.2}%", mean(&steps[i]));
    }

    println!();
    // Stated as a check to run, not as a conclusion: the numbers above are the
    // finding and this only says how to read them.
    println!();
    println!("Between the references themselves: 1 - cos(v_i, v_j), mean.");
    println!("Distance from the prompt is one projection; this is the other.");
    println!();
    print!("{:<22}", "");
    for (name, _, _) in &neg_ctx {
        print!(" {:>10}", &name[..name.len().min(10)]);
    }
    println!();
    for (i, (name, _, _)) in neg_ctx.iter().enumerate() {
        print!("{name:<22}");
        for j in 0..neg_ctx.len() {
            print!(" {:>9.2}%", mean(&pairwise[i * neg_ctx.len() + j]));
        }
        println!();
    }

    println!();
    println!("Read it this way:");
    if grid < 32 {
        println!("  *** GRID {grid} IS BELOW THE DEFAULT AND THIS ANSWER IS NOT ABOUT");
        println!("  *** NEGATIVE PROMPTS. At 256x256 the denoiser is 2.65x worse before");
        println!("  *** any prompt is involved, the subject ordering disappears, and the");
        println!("  *** length effect is 9x what it is at grid 32. Run this at 32.");
        println!();
    }
    println!("  'the prompt itself' must be 0.00%. It is the same inputs twice.");
    println!("  THE COMPARISON IS 'same idea, long' AGAINST 'different, long'.");
    println!("  They are matched in length and opposite in subject, so if they");
    println!("  land together the measure is following how much text there is,");
    println!("  and a negative prompt's content is close to irrelevant.");
    println!("  Comparing either of them to a short negative confounds the two.");
    println!("  Then check the matrix: if those same two sit FAR APART from each");
    println!("  other, content does move the model and the column was the wrong");
    println!("  projection to read it in.");
    println!("  If every negative sits below the twin, a negative prompt guides");
    println!("  more weakly than leaving it off.");
}
