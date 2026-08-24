//! Does the *wrapper* condition more strongly than the phrase it wraps?
//!
//! # What is already known, and what is not
//!
//! `backlog/image-generation-ideogram-4.md` measured that a hand-written
//! structured prompt moves the denoiser three times as far as a seven-word one:
//! cos(A, B) 0.9667 against 0.9897. That was two prompts written out by hand,
//! several hundred tokens each, describing lighting and palette and layout.
//!
//! `prompt::structure` writes none of that. It provides **scaffolding, not
//! content** — the phrase goes in `high_level_description` and the rest of the
//! frame is left empty for the user to fill, deliberately, because a wrapper
//! that invented *golden hour, bokeh, 8k* would draw a different picture than
//! the one asked for.
//!
//! So the open question is the one that matters for the button on the IMAGE
//! page: **does the empty frame help at all, or is the benefit entirely in the
//! content a person writes into it?** If the answer is "not at all", the button
//! is a lie and should not ship.
//!
//! ```text
//! cargo run --release -p chaos-image --example structure-helps
//! ```
//!
//! Method as in `try-conditioning` — run the conditional denoiser under two
//! very different prompts and measure how far apart the answers are — **but
//! over several latents rather than one.**
//!
//! That is not a refinement, it is the finding. The first version of this used
//! a single noise seed, as `try-conditioning` does, and reported that the
//! hand-written prompt conditioned **fourteen times** more strongly than a bare
//! phrase. A different seed reported that it conditioned **three times less**.
//! Both runs were correct and neither was an answer.
//!
//! Lower cosine means the prompt did more work. Three prompt styles, the same
//! two ideas throughout, eight latents by default.

use chaos_image::pipeline::Noise;
use chaos_image::{dit, flow, prompt, text, text::TextEncoder};
use chaos_model::Model;
use chaos_tokenizer::Tokenizer;

/// The same two ideas, as a bare phrase. The pair is deliberately as far apart
/// as two pictures get: if these two do not move the model, nothing will.
const BARE_A: &str = "a single red apple on a white table";
const BARE_B: &str = "a snowy mountain range at sunrise";

/// The same two ideas written out properly, from `try-conditioning`. This is
/// the ceiling: what the effort actually buys.
const FULL_A: &str = r##"{"high_level_description":"A single red apple resting on a white table, photographed straight on in a bright studio.","style_description":{"aesthetics":"clean product photography, minimal, bright, commercial","lighting":"soft even studio lighting from above, gentle shadow beneath the apple","photo":"high resolution, sharp focus on the apple skin, shallow depth of field","medium":"digital photograph","color_palette":["#C81E1E","#FFFFFF","#F2F2F2","#8A8A8A"]},"compositional_deconstruction":{"canvas":"Square canvas, upright orientation, centred subject.","background":"A plain white studio backdrop meeting a white tabletop, no visible seam.","layout":"The apple sits alone in the centre of the frame with generous empty space around it.","elements":[{"type":"obj","desc":"Exactly one ripe red apple with a short brown stem, glossy skin, a soft highlight on its upper left."},{"type":"obj","desc":"A soft contact shadow directly under the apple on the white surface."}]}}"##;

const FULL_B: &str = r##"{"high_level_description":"A snowy mountain range at sunrise, a wide landscape photograph taken from a high ridge.","style_description":{"aesthetics":"epic landscape photography, cold alpine light, vast scale","lighting":"low golden sunrise raking across the peaks, deep blue shadow in the valleys","photo":"wide angle, deep focus from foreground snow to distant peaks","medium":"digital photograph","color_palette":["#F5C77E","#9FC0E8","#20395B","#FFFFFF"]},"compositional_deconstruction":{"canvas":"Square canvas, upright orientation, horizon in the upper third.","background":"A pale sunrise sky graduating from gold at the horizon to cold blue above.","layout":"Layered ridgelines receding into haze, the tallest peak left of centre.","elements":[{"type":"obj","desc":"Snow-covered jagged peaks catching the first orange light on their east faces."},{"type":"obj","desc":"A wind-scoured snowfield in the foreground with rippled texture."}]}}"##;

fn cos(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    let (a, b) = (&a[..n], &b[..n]);
    let dot: f64 = a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|y| (*y as f64).powi(2)).sum::<f64>().sqrt();
    dot / (na * nb)
}

fn main() {
    let grid: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);
    let sigma: f32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.7);

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let dir = std::path::Path::new(&home).join(".chaos").join("models");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    let styles: [(&str, String, String); 3] = [
        ("bare phrase", BARE_A.into(), BARE_B.into()),
        (
            "wrapped by prompt::structure",
            prompt::structure(BARE_A),
            prompt::structure(BARE_B),
        ),
        ("written out by hand", FULL_A.into(), FULL_B.into()),
    ];

    // -- conditioning, once per prompt ---------------------------------------
    let mut conditioned = Vec::new();
    {
        let model = Model::open_split(dir.join("Qwen3-VL-8B-Instruct-Q4_K_M.gguf"))
            .expect("open the text encoder");
        let tok = Tokenizer::from_metadata(model.metadata()).expect("tokenizer");
        let enc = TextEncoder::open(model, threads).expect("text encoder");
        for (name, a, b) in &styles {
            let run = |p: &str| {
                let ids = tok.encode(&text::wrap_prompt(p));
                enc.encode(&ids, &mut |_, _| {}).expect("encode")
            };
            let (ca, cb) = (run(a), run(b));
            println!("{name:<28} {:>5} and {:>5} tokens", ca.tokens, cb.tokens);
            conditioned.push((*name, ca, cb));
        }
    }

    // -- one latent, six forward passes --------------------------------------
    let path = dir.join("ideogram4-Q4_0.gguf");
    let model = Model::open_split(&path).expect("open the denoiser");
    let d = dit::Denoiser::open(model, threads);

    let c = dit::Config::default();
    let n = (grid * grid * c.in_channels) as usize;
    let t = flow::timestep_for(sigma);

    // **Several latents, because one is not a measurement.** The first version
    // of this used a single noise seed and reported that the hand-written
    // prompt conditioned *fourteen times* more strongly than a bare phrase. A
    // second seed reported that it conditioned three times **less**. Both runs
    // were correct; neither was an answer.
    //
    // The published "3x more strongly" claim rests on one latent, and this is
    // how that gets checked.
    let seeds: Vec<u64> = match std::env::args().nth(3) {
        Some(list) => list
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect(),
        None => (1..=8).collect(),
    };

    println!();
    println!("latent       {grid}x{grid} grid, sigma {sigma}, timestep {t:.0}");
    println!("seeds        {seeds:?}");
    println!("lower cos(A, B) means the prompt moved the model further.");
    println!();

    let mut per_style: Vec<Vec<f64>> = vec![Vec::new(); conditioned.len()];
    print!("{:>6}", "seed");
    for (name, _, _) in &conditioned {
        print!(" {name:>16}");
    }
    println!();

    for &seed in &seeds {
        let x = Noise::seeded(seed).normals(n);
        print!("{seed:>6}");
        for (i, (_, ca, cb)) in conditioned.iter().enumerate() {
            let run = |ctx: &text::Encoded| {
                d.forward(&dit::Inputs {
                    latent: &x,
                    grid_w: grid,
                    grid_h: grid,
                    timestep: t,
                    context: &ctx.hidden,
                    context_len: ctx.tokens,
                })
                .expect("forward")
            };
            let (va, vb) = (run(ca), run(cb));
            let effect = (1.0 - cos(&va, &vb)) * 100.0;
            per_style[i].push(effect);
            print!(" {effect:>15.2}%");
        }
        println!();
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    println!();
    println!(
        "{:<28} {:>9} {:>9} {:>9} {:>9}",
        "prompt style", "mean", "min", "max", "vs bare"
    );
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let bare = mean(&per_style[0]);
    for (i, (name, _, _)) in conditioned.iter().enumerate() {
        let v = &per_style[i];
        let m = mean(v);
        let lo = v.iter().cloned().fold(f64::MAX, f64::min);
        let hi = v.iter().cloned().fold(f64::MIN, f64::max);
        println!(
            "{name:<28} {m:>8.2}% {lo:>8.2}% {hi:>8.2}% {:>8.1}x",
            m / bare
        );
    }

    println!();
    println!("Read the spread before the mean. If min and max straddle each other");
    println!("across styles, one latent proves nothing and neither does this table");
    println!("without more of them.");
    println!("If the wrapper's row is no better than the bare phrase's, the button on");
    println!("the IMAGE page buys nothing and should not exist. If it lands between the");
    println!("two, it is worth what it costs -- which is one press.");
}
