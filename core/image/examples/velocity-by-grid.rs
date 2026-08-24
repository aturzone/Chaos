//! Is the denoiser worse at small grids, or is something else?
//!
//! # The question
//!
//! Atur: 256 and 512 come out visibly worse than a linear reading of the size
//! ladder suggests, while 1024 is photorealistic. There are three candidates
//! and they call for completely different work:
//!
//! 1. **The denoiser** genuinely predicts a worse direction at fewer tokens.
//! 2. **The sampler** — twenty steps that are right at 4096 tokens and wrong at
//!    256, or a sigma schedule that does not transfer.
//! 3. **Neither.** 256x256 is 256 image tokens against 4096; a sixteenth of the
//!    capacity produces a sixteenth of the detail, and that is arithmetic
//!    rather than a bug.
//!
//! This measures (1), which is the only one that can be measured without
//! generating anything. If the velocity cosine is flat across grids, the
//! denoiser is fine and the answer is (2) or (3) — and a great deal of work on
//! the model would have been wasted.
//!
//! # How
//!
//! `try-velocity`'s method, at five sizes. A rectified-flow model is trained
//! on a straight line between a real latent and pure noise; at noise level
//! `sigma` the input is `latent * (1 - sigma) + noise * sigma` and the velocity
//! it should predict is exactly `noise - latent`. **Both terms are known**, so
//! the answer is scored by cosine with no picture involved and nothing to fool
//! the eye.
//!
//! The same picture at every size, so the only thing that changes is the grid.
//! It is the synthetic image from `tests/vae_roundtrip.rs`, for the reason
//! given there — the only photographs to hand are other people's — and it
//! round-trips at 37.59 dB against 36.09 for a real photograph, so it is not an
//! easier input than the real thing.
//!
//! ```text
//! cargo run --release -p chaos-image --example velocity-by-grid
//! ```
//!
//! **This is slow and says so before it starts**: a forward pass costs roughly
//! 0.057 s per image token, so 768x768 is about two minutes for one sigma.
//!
//! # 1024 used to be out of reach, and it was the arena rather than the model
//!
//! `vae::encode` builds an **unplanned** graph — every tensor allocated, none
//! freed — at about 48 KiB per input pixel, so a 1024x1024 encode asks for
//! **51 GiB** and ggml aborts with `GGML_ASSERT(ctx->mem_buffer != NULL)`. This
//! sweep stopped at 768 for that reason, and an arena limit standing in for a
//! model limit is the worst kind of missing datum: it looks like a result.
//!
//! `vae::encode_planned` fixes it, mirroring `decode_planned`: **1.51 GiB at
//! 1024 instead of 48.5, and bit-identical** at every size where both can run.

use chaos_image::pipeline::Noise;
use chaos_image::{dit, flow, safetensors::SafeTensors, vae};
use chaos_model::Model;

/// The picture, at whatever size is asked for.
///
/// Smooth colour so a scale error shifts it, hard edges so a padding error
/// blurs them, and a soft lobe so it is not piecewise constant. **Scale-free**:
/// every feature is placed in fractions of the side, so 256 and 1024 carry the
/// same picture rather than the same pixels — which is the whole point of
/// comparing them.
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

fn cos(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum();
    let na: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|y| (*y as f64).powi(2)).sum::<f64>().sqrt();
    dot / (na * nb)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sizes: Vec<usize> = {
        let v: Vec<usize> = args.iter().filter_map(|s| s.parse().ok()).collect();
        if v.is_empty() {
            vec![256, 384, 512, 640, 768, 1024]
        } else {
            v
        }
    };
    // Three noise levels, not one. A denoiser can be right at heavy noise —
    // where "point away from the noise" is most of the answer — and wrong at
    // light noise, where it has to know what the picture is.
    let sigmas = [0.8f32, 0.5, 0.25];

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let dir = std::path::Path::new(&home).join(".chaos").join("models");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    // Say the cost before spending it.
    let total: f64 = sizes
        .iter()
        .map(|s| {
            let tokens = (s / (vae::SCALE * 2)).pow(2) as f64;
            tokens * 0.0574 * sigmas.len() as f64
        })
        .sum();
    println!("sizes        {sizes:?}");
    println!("sigmas       {sigmas:?}");
    println!(
        "cost         about {:.0} minutes on this machine, {} forward passes",
        total / 60.0,
        sizes.len() * sigmas.len()
    );

    println!();

    let ae_path = dir.join("flux2-vae.safetensors");
    let file = match std::fs::read(&ae_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", ae_path.display());
            eprintln!("`chaos-pull flux2-vae` fetches it.");
            std::process::exit(1);
        }
    };
    let st = SafeTensors::parse(&file).expect("parse the autoencoder");

    let path = dir.join("ideogram4_uncond-Q4_0.gguf");
    let model = match Model::open_split(&path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            eprintln!("`chaos-pull ideogram-4-uncond` fetches it.");
            std::process::exit(1);
        }
    };
    // The **unconditional** twin, so no text encoder is involved: it predicts
    // what an average image would do, which is most of the velocity at any
    // noticeable noise level, and it removes the prompt as a variable.
    let d = dit::Denoiser::open(model, threads);

    println!(
        "{:>6} {:>6} {:>8} {:>7} {:>9} {:>9} {:>9}",
        "size", "grid", "tokens", "sigma", "cos(v)", "cos(-L)", "x0 err"
    );
    for &n in &sizes {
        let rgb = test_image(n);
        let (lw, lh) = (n / vae::SCALE, n / vae::SCALE);

        // **Planned, which is what makes 1024 reachable at all.** The
        // unplanned encoder costs ~48 KiB per input pixel -- 51 GiB at 1024,
        // where ggml aborts -- and that arena limit used to stand in for a
        // model limit here, which is the worst kind of missing datum because
        // it looks like a result. `encode_planned` is 1.51 GiB at 1024 and
        // bit-identical where both can run (`try-planned-encode`).
        let latent = vae::encode_planned(
            &st,
            &file,
            &vae::from_rgb8(&rgb, n, n),
            n as i64,
            n as i64,
            threads,
        )
        .expect("planned encode")
        .0;

        let c = dit::Config::default();
        let mut packed =
            vae::pack_latent(&latent, lw, lh, c.ae_channels as usize, c.patch as usize);
        if let Some((m, v)) = vae::latent_stats(&st, &file) {
            vae::normalize_latent(&mut packed, &m, &v);
        }
        let (gw, gh) = (lw / 2, lh / 2);

        for &sigma in &sigmas {
            // **The same seed at every size**, so two grids differ by their
            // grid and not by their noise.
            let noise = Noise::seeded(1234).normals(packed.len());
            let x: Vec<f32> = packed
                .iter()
                .zip(&noise)
                .map(|(l, nz)| l * (1.0 - sigma) + nz * sigma)
                .collect();
            let truth: Vec<f32> = noise.iter().zip(&packed).map(|(nz, l)| nz - l).collect();

            let pred = d
                .forward(&dit::Inputs {
                    latent: &x,
                    grid_w: gw as i64,
                    grid_h: gh as i64,
                    timestep: flow::timestep_for(sigma),
                    context: &[],
                    context_len: 0,
                })
                .expect("forward");

            let neg_latent: Vec<f32> = packed.iter().map(|l| -l).collect();
            let x0: Vec<f32> = x.iter().zip(&pred).map(|(xi, v)| xi - sigma * v).collect();
            let err = {
                let num: f64 = x0
                    .iter()
                    .zip(&packed)
                    .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
                    .sum();
                let den: f64 = packed.iter().map(|b| (*b as f64).powi(2)).sum();
                (num / den).sqrt()
            };
            println!(
                "{n:>6} {gw:>6} {:>8} {sigma:>7.2} {:>9.4} {:>9.4} {:>9.4}",
                gw * gh,
                cos(&pred, &truth),
                cos(&pred, &neg_latent),
                err
            );
        }
    }

    println!();
    println!("cos(v) is the whole answer. If it is flat across grids, the denoiser");
    println!("is equally right at every size and small images are worse for another");
    println!("reason -- the sampler, or simply that 256 tokens carry a sixteenth of");
    println!("what 4096 do. If it falls with the grid, the model is the problem.");
}
