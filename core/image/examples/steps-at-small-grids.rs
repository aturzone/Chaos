//! Do more steps make up for a worse per-step direction?
//!
//! # Why this is the question
//!
//! `research/small-images-are-the-model-2026-08-24.md` measured that the
//! denoiser's velocity is genuinely worse at small grids — cosine 0.858 at grid
//! 16 against 0.941 at grid 48, at light noise, monotonically. That is the
//! model, and there is nothing to be done about it here.
//!
//! But it leaves one lever. If each step points a little wrong, **more, smaller
//! steps** may accumulate less error than fewer large ones. The app defaults to
//! 20 steps at every size; if 50 is meaningfully better at 256×256 and no
//! better at 1024×1024, the default should depend on the size.
//!
//! If it is *not* better, that is worth knowing too: it means a user waiting
//! two and a half times as long at 256 is getting nothing for it, and the
//! ladder should say so.
//!
//! # How
//!
//! **Integration error, against a latent that is actually known.** Generating
//! from pure noise has no ground truth to score against; denoising a *partly*
//! noised real latent does.
//!
//! Take a real latent, mix it with noise at `sigma = 0.6`, then integrate the
//! flow back down to zero with `n` Euler steps and compare the result to the
//! latent it started from. More steps should land closer. How much closer is
//! the whole question.
//!
//! ```text
//! cargo run --release -p chaos-image --example steps-at-small-grids
//! ```
//!
//! The **unconditional** twin, so no text encoder and no prompt is involved —
//! this measures the sampler, and a prompt would be a second variable.

use chaos_image::pipeline::Noise;
use chaos_image::{dit, flow, safetensors::SafeTensors, vae};
use chaos_model::Model;

/// The same scale-free picture the grid sweep uses, so the two are comparable.
fn test_image(n: usize) -> Vec<u8> {
    let mut px = Vec::with_capacity(n * n * 3);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let r = ((u - 0.35).powi(2) + (v - 0.4).powi(2)).sqrt();
            let lobe = (-(r * 3.2).powi(2)).exp();
            let mut rgb = [
                0.55 + 0.35 * lobe - 0.20 * v,
                0.30 + 0.45 * u * (1.0 - v) + 0.25 * lobe,
                0.70 - 0.40 * u + 0.30 * (6.0 * (u + v)).sin(),
            ];
            if (0.60..0.88).contains(&u) && (0.15..0.42).contains(&v) {
                rgb = [0.92, 0.88, 0.30];
            }
            if (0.12..0.34).contains(&u) && (0.66..0.90).contains(&v) {
                rgb = [0.10, 0.22, 0.55];
            }
            for c in rgb {
                px.push((c.clamp(0.0, 1.0) * 255.0) as u8);
            }
        }
    }
    px
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let size: usize = args.first().and_then(|s| s.parse().ok()).unwrap_or(256);
    let steps: Vec<usize> = {
        let v: Vec<usize> = args
            .get(1)
            .map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect())
            .unwrap_or_default();
        if v.is_empty() {
            // The five the app offers.
            vec![4, 8, 20, 30, 50]
        } else {
            v
        }
    };
    // Where the integration starts. Not 1.0: at pure noise there is no latent
    // left to recover and every step count scores the same nothing.
    let start: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.6);

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let dir = std::path::Path::new(&home).join(".chaos").join("models");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    let grid = size / (vae::SCALE * 2);
    let tokens = grid * grid;
    let passes: usize = steps.iter().sum();
    println!("size         {size}x{size}, grid {grid}, {tokens} tokens");
    println!("steps        {steps:?}");
    println!("start        sigma {start}");
    println!(
        "cost         {passes} forward passes, about {:.0} minutes on this machine",
        passes as f64 * tokens as f64 * 0.0574 / 60.0
    );
    println!();

    let ae_path = dir.join("flux2-vae.safetensors");
    let file = match std::fs::read(&ae_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", ae_path.display());
            std::process::exit(1);
        }
    };
    let st = SafeTensors::parse(&file).expect("parse the autoencoder");

    let (lw, lh) = (size / vae::SCALE, size / vae::SCALE);
    let latent = {
        let arena = (512 << 20) + size * size * 48 * 1024;
        let ctx = chaos_ggml::Context::new(arena).expect("encoder arena");
        let v = vae::Vae::new(&st, &file, &ctx);
        let img = ctx
            .new_f32_4d(size as i64, size as i64, 3, 1)
            .expect("image tensor");
        img.set_f32(&vae::from_rgb8(&test_image(size), size, size))
            .expect("set image");
        let moments = v.encode(&img).expect("encode");
        let mean = v.latent_mean(&moments).expect("mean");
        ctx.compute(&mean, threads).expect("compute");
        mean.to_vec_f32()
    };

    let c = dit::Config::default();
    let mut truth = vae::pack_latent(&latent, lw, lh, c.ae_channels as usize, c.patch as usize);
    if let Some((m, v)) = vae::latent_stats(&st, &file) {
        vae::normalize_latent(&mut truth, &m, &v);
    }

    let d = dit::Denoiser::open(
        Model::open_split(dir.join("ideogram4_uncond-Q4_0.gguf")).expect("open the twin"),
        threads,
    );

    // **The same noise for every step count**, so two rows differ by their
    // step count and nothing else.
    let noise = Noise::seeded(99).normals(truth.len());
    let rel_err = |a: &[f32]| -> f64 {
        let num: f64 = a
            .iter()
            .zip(&truth)
            .map(|(x, t)| (*x as f64 - *t as f64).powi(2))
            .sum();
        let den: f64 = truth.iter().map(|t| (*t as f64).powi(2)).sum();
        (num / den).sqrt()
    };

    // Where it starts, so the improvement has a baseline to be read against.
    let x0: Vec<f32> = truth
        .iter()
        .zip(&noise)
        .map(|(t, n)| t * (1.0 - start) + n * start)
        .collect();
    println!("before any denoising, the error is {:.4}", rel_err(&x0));
    println!();
    println!("{:>7} {:>10} {:>12} {:>10}", "steps", "error", "vs 20 steps", "seconds");

    let mut at_20 = None;
    for &n in &steps {
        // The schedule the sampler actually uses, truncated to the part below
        // `start` -- so this integrates the same curve a real render does.
        let full = flow::Schedule::default().sigmas(n);
        let sigmas: Vec<f32> = full.iter().map(|s| s * start).collect();

        let t0 = std::time::Instant::now();
        let mut x = x0.clone();
        for i in 0..n {
            let v = d
                .forward(&dit::Inputs {
                    latent: &x,
                    grid_w: grid as i64,
                    grid_h: grid as i64,
                    timestep: flow::timestep_for(sigmas[i]),
                    context: &[],
                    context_len: 0,
                })
                .expect("forward");
            flow::euler_step(&mut x, &v, sigmas[i], sigmas[i + 1]);
        }
        let err = rel_err(&x);
        if n == 20 {
            at_20 = Some(err);
        }
        let rel = at_20.map_or("-".into(), |e| format!("{:.2}x", err / e));
        println!(
            "{n:>7} {err:>10.4} {rel:>12} {:>10.0}",
            t0.elapsed().as_secs_f64()
        );
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    println!();
    println!("Lower error is better. If 50 steps is much below 20, the app's fixed");
    println!("default of 20 is wrong at this size. If it is barely below, a user");
    println!("waiting two and a half times as long is getting nothing for it.");
}
