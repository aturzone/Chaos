//! Does planned encoding give the same latent, and how much less memory?
//!
//! **An example, not a test**, for the reason `try-planned-decode` gives:
//! `ggml_gallocr` aborts rather than erroring, and a test binary that dies
//! takes every other test with it.
//!
//! The unplanned encoder allocates every tensor and frees none, so it costs
//! about **48 KiB per input pixel** — 12.9 GiB at 512, 29 GiB at 768, and
//! **51 GiB at 1024**, where ggml aborts with
//! `GGML_ASSERT(ctx->mem_buffer != NULL)`. That ceiling is what stopped
//! `velocity-by-grid` measuring the denoiser at 1024: an arena limit standing
//! in for a model limit, which is the worst kind of missing datum because it
//! looks like a result.
//!
//! ```text
//! cargo run --release -p chaos-image --example try-planned-encode 256
//! cargo run --release -p chaos-image --example try-planned-encode 1024
//! ```
//!
//! **An identical latent is the requirement.** Buffer reuse that changes an
//! answer is aliasing, and a latent is exactly the place it would not be
//! noticed — the picture would come out slightly wrong and nothing would say
//! so. At sizes the unplanned path cannot reach there is nothing to compare
//! against, and the example says that rather than implying agreement.

use chaos_image::{safetensors::SafeTensors, vae};

/// The scale-free picture the other image examples use.
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

/// About what the unplanned graph asks for, per input pixel. Measured: 512
/// works at 12.9 GiB of virtual arena and 1024 aborts at 51.5 GiB.
const UNPLANNED_PER_PIXEL: usize = 48 * 1024;

/// The largest input the unplanned graph reaches on this machine. Above it the
/// process aborts, so the comparison simply cannot be made.
const UNPLANNED_CEILING: usize = 768;

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);
    if n % 8 != 0 {
        eprintln!("size must be a multiple of 8 -- three stride-2 convolutions");
        std::process::exit(2);
    }

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let ae = std::path::Path::new(&home)
        .join(".chaos")
        .join("models")
        .join("flux2-vae.safetensors");
    let file = match std::fs::read(&ae) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", ae.display());
            eprintln!("`chaos-pull flux2-vae` fetches it.");
            std::process::exit(1);
        }
    };
    let st = SafeTensors::parse(&file).expect("parse the autoencoder");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    let rgb = vae::from_rgb8(&test_image(n), n, n);
    let gib = |b: usize| b as f64 / (1u64 << 30) as f64;

    let unplanned_arena = (512 << 20) + n * n * UNPLANNED_PER_PIXEL;
    println!("image        {n}x{n}");
    println!("unplanned    {:.2} GiB of arena", gib(unplanned_arena));
    println!(
        "planned      {:.2} GiB predicted",
        gib(vae::encode_planned_bytes(n, n))
    );

    let t = std::time::Instant::now();
    let (planned, bytes) =
        vae::encode_planned(&st, &file, &rgb, n as i64, n as i64, threads).expect("planned encode");
    println!(
        "             {:.2} GiB actual, {:.1}s, {} values",
        gib(bytes),
        t.elapsed().as_secs_f64(),
        planned.len()
    );
    println!("saving       {:.0}x", unplanned_arena as f64 / bytes as f64);

    // **Only where the unplanned path can actually run.** Measured on this
    // machine: 512 works at 12.9 GiB of virtual arena, 768 works at 29 GiB, and
    // 1024 aborts at 51.5 GiB. A hard cut at 768 rather than a memory reading,
    // because ggml aborts rather than failing and an example that killed itself
    // proving a point would prove nothing.
    //
    // `--compare` forces it anyway, for somebody on a bigger machine.
    let forced = std::env::args().any(|a| a == "--compare");
    if n > UNPLANNED_CEILING && !forced {
        println!();
        println!("The unplanned path is not run above {UNPLANNED_CEILING}: it would ask for");
        println!(
            "{:.0} GiB of arena and ggml aborts rather than failing.",
            gib(unplanned_arena)
        );
        println!("So there is nothing to compare against at this size -- which is the");
        println!("whole reason `encode_planned` exists. Pass --compare to try anyway.");
        return;
    }

    let t = std::time::Instant::now();
    let unplanned = {
        let ctx = chaos_ggml::Context::new(unplanned_arena).expect("unplanned arena");
        let v = vae::Vae::new(&st, &file, &ctx);
        let img = ctx.new_f32_4d(n as i64, n as i64, 3, 1).expect("image");
        img.set_f32(&rgb).expect("set image");
        let moments = v.encode(&img).expect("encode");
        let mean = v.latent_mean(&moments).expect("mean");
        ctx.compute(&mean, threads).expect("compute");
        mean.to_vec_f32()
    };
    println!("unplanned    {:.1}s", t.elapsed().as_secs_f64());

    println!();
    assert_eq!(planned.len(), unplanned.len(), "different latent shapes");
    let mut worst = 0.0f32;
    let mut at = 0usize;
    for (i, (a, b)) in planned.iter().zip(&unplanned).enumerate() {
        let d = (a - b).abs();
        if d > worst {
            worst = d;
            at = i;
        }
    }
    if worst == 0.0 {
        println!(
            "IDENTICAL -- every one of {} values, bit for bit.",
            planned.len()
        );
    } else {
        println!(
            "DIFFERENT -- worst {worst:e} at index {at} ({} vs {}).",
            planned[at], unplanned[at]
        );
        println!("Buffer reuse that changes an answer is aliasing. This is a bug.");
        std::process::exit(1);
    }
}
