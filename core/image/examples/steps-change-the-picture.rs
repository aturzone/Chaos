//! Does the step count change the picture that comes out?
//!
//! # Why this is a different question from the last one
//!
//! `steps-at-small-grids` measures **reconstruction**: noise a known latent,
//! integrate back, and see how close it lands. That is the right way to measure
//! integration error and it found that step count barely moves it.
//!
//! It cannot answer the question a user actually has. **Generation starts from
//! pure noise and has no ground truth** — the model is inventing, not
//! recovering, and "close to the original" is not defined. A sampler could have
//! flat reconstruction error and still produce visibly different pictures at 4
//! steps and 50.
//!
//! So this asks the one thing that *can* be measured without an opinion:
//! **do two step counts produce the same latent?**
//!
//! | cosine between them | what it means |
//! |---|---|
//! | ~1.0 | the same picture. Step count is buying nothing, and the cheap one wins. |
//! | well below | different pictures, and **which is better is not a question this can answer** |
//!
//! That second row is the honest limit. A difference does not tell you the
//! slower one is better — only that somebody has to look. This project's rule
//! is that an image decoder is checked by round trip and never by looking, but
//! *aesthetic* quality has no such instrument, and pretending otherwise is how
//! three deliberate bugs each produced a recognisable picture.
//!
//! ```text
//! cargo run --release -p chaos-image --example steps-change-the-picture 16
//! ```
//!
//! The **unconditional** twin: no text encoder, no prompt, so the comparison is
//! about the sampler and nothing else.

use chaos_image::pipeline::Noise;
use chaos_image::{dit, flow};
use chaos_model::Model;

fn cos(a: &[f32], b: &[f32]) -> f64 {
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
    let steps: Vec<usize> = std::env::args()
        .nth(2)
        .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![4, 8, 20, 50]);

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let dir = std::path::Path::new(&home).join(".chaos").join("models");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    let c = dit::Config::default();
    let n = (grid * grid * c.in_channels) as usize;
    let passes: usize = steps.iter().sum();
    println!("grid         {grid}x{grid}, {} tokens", grid * grid);
    println!("steps        {steps:?}");
    println!(
        "cost         {passes} forward passes, about {:.0} minutes",
        passes as f64 * (grid * grid) as f64 * 0.0574 / 60.0
    );
    println!();

    let d = dit::Denoiser::open(
        Model::open_split(dir.join("ideogram4_uncond-Q4_0.gguf")).expect("open the twin"),
        threads,
    );

    // **The same starting noise for every step count.** Two runs from different
    // noise would differ for a reason that has nothing to do with the sampler.
    let start = Noise::seeded(2024).normals(n);

    let mut results: Vec<(usize, Vec<f32>, f64)> = Vec::new();
    for &k in &steps {
        let sigmas = flow::Schedule::default().sigmas(k);
        let t0 = std::time::Instant::now();
        let mut x = start.clone();
        for i in 0..k {
            let v = d
                .forward(&dit::Inputs {
                    latent: &x,
                    grid_w: grid,
                    grid_h: grid,
                    timestep: flow::timestep_for(sigmas[i]),
                    context: &[],
                    context_len: 0,
                })
                .expect("forward");
            flow::euler_step(&mut x, &v, sigmas[i], sigmas[i + 1]);
        }
        let secs = t0.elapsed().as_secs_f64();
        println!("  {k:>3} steps done in {secs:>6.0}s");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        results.push((k, x, secs));
    }

    println!();
    println!("cosine between the latents each step count produced:");
    print!("{:>8}", "");
    for (k, _, _) in &results {
        print!(" {k:>8}");
    }
    println!();
    for (a, xa, _) in &results {
        print!("{a:>8}");
        for (_, xb, _) in &results {
            print!(" {:>8.4}", cos(xa, xb));
        }
        println!();
    }

    println!();
    let base = results
        .iter()
        .find(|(k, _, _)| *k == 20)
        .or_else(|| results.last());
    if let Some((bk, bx, bs)) = base {
        println!("against {bk} steps, which is the app's default:");
        for (k, x, s) in &results {
            let c = cos(x, bx);
            println!(
                "  {k:>3} steps  cos {c:.4}  ({:.1}x the time)",
                s / bs.max(1e-9)
            );
        }
    }
    println!();
    println!("A cosine near 1.0 means the same picture, and the cheap step count wins.");
    println!("Anything else means they are different pictures -- and WHICH IS BETTER is");
    println!("not a question this can answer. Somebody has to look.");
}
