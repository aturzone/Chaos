//! What this machine's memory actually delivers.
//!
//! **The datasheet is not the machine.** DDR5-4800 dual channel is 76.8 GB/s on
//! paper; this laptop delivers 33.1 GB/s — 43% of it. Quoting the spec would
//! have overstated every bandwidth budget in the project by 2.3x, and those
//! budgets are what decide whether a tok/s target is reachable or arithmetic
//! nonsense.
//!
//! It exists because "20 tok/s on V4-Flash is out of reach" had only ever been
//! argued from a *fixed cost per token* — a measurement of Chaos, not of the
//! hardware. Those are different claims and only one of them is about physics.
//!
//! # Reading the result
//!
//! `tok/s ≈ bandwidth / resident GiB` for generation, which is
//! bandwidth-bound. Nine models on this machine hold that constant to ±8% over
//! a 23x range of size (`research/machine-bandwidth-2026-08-25.md`). So the
//! number this prints is the one that says what any model will do here.
//!
//! Expect the curve to flatten early: this machine saturates at four threads
//! and gains nothing from the remaining sixteen. That is also why generation
//! wants 2-4 threads and prefill wants all of them.
//!
//! ```text
//! chaos-membench [GiB]      # default 4
//! ```

use std::sync::Arc;
use std::time::Instant;

fn main() {
    // Every binary in this repo answers `--version`, and a test enforces it --
    // which is how this one was caught before it shipped.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("chaos-membench {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("chaos-membench [GiB]   what this machine reads from RAM, by thread count");
        println!();
        println!("  tok/s = bandwidth / resident GiB for generation, so this number");
        println!("  predicts what any model will do here. Default buffer 4 GiB; it must");
        println!("  be far larger than L3, or the cache is what gets measured.");
        return;
    }
    let gib: usize = args.first().and_then(|s| s.parse().ok()).unwrap_or(4);
    let bytes = gib * 1024 * 1024 * 1024;
    let n = bytes / 8;
    // Touched once so the pages are really mapped; an untouched Vec measures
    // the page fault handler, not the memory.
    let mut v = vec![0u64; n];
    for (i, x) in v.iter_mut().enumerate() {
        *x = i as u64;
    }
    let v = Arc::new(v);

    println!(
        "buffer {gib} GiB, {} threads available",
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
    );
    println!("{:>8}  {:>12}  {:>12}", "threads", "GiB/s", "GB/s");
    for threads in [1usize, 2, 4, 8, 12, 16, 20] {
        let mut best = 0.0f64;
        for _ in 0..3 {
            let start = Instant::now();
            let chunk = n / threads;
            let mut hs = Vec::new();
            for t in 0..threads {
                let v = Arc::clone(&v);
                hs.push(std::thread::spawn(move || {
                    let lo = t * chunk;
                    let hi = if t == threads - 1 {
                        v.len()
                    } else {
                        lo + chunk
                    };
                    // Sum, so the reads cannot be optimised away.
                    let mut acc = 0u64;
                    let mut i = lo;
                    while i < hi {
                        acc = acc.wrapping_add(v[i]);
                        i += 8; // one per 64-byte cache line
                    }
                    acc
                }));
            }
            let mut acc = 0u64;
            for h in hs {
                acc = acc.wrapping_add(h.join().unwrap());
            }
            std::hint::black_box(acc);
            let secs = start.elapsed().as_secs_f64();
            let gibs = bytes as f64 / (1024.0 * 1024.0 * 1024.0) / secs;
            if gibs > best {
                best = gibs;
            }
        }
        println!(
            "{:>8}  {:>12}  {:>12}",
            threads,
            format!("{:.1}", best),
            format!("{:.1}", best * 1.073_741_824)
        );
    }
}
