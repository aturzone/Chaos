//! Does an NVMe go faster when several reads are in flight?
//!
//! **This decides rung 1 of the 5 tok/s ladder, and it costs a day to guess.**
//! `research/machine-bandwidth-2026-08-25.md` measured Chaos reading V4-Flash's
//! experts at **1.40 GiB/s** from a drive that does **3.09 GiB/s** sequential —
//! 45% of it. The suspicion is queue depth: an expert is 12.8 MiB, they are
//! read one at a time, and an NVMe reaches its rated speed only with several
//! requests outstanding.
//!
//! If concurrency does not raise the number, rung 1 is dead and the ladder
//! starts at 0.43 tok/s instead of 0.96 — which is worth knowing before any
//! engine code is written for it.
//!
//! # What it measures, and what it refuses to
//!
//! Reads of one expert's size, at several queue depths, from a real model file
//! on this machine. **The page cache is the enemy**: a second pass over the
//! same bytes measures RAM, not the drive, so every depth reads a *different*
//! region and the file must be far larger than memory.
//!
//! ```text
//! chaos-qdbench <path-to-a-large-gguf> [MiB-per-read]
//! ```

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("chaos-qdbench {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("chaos-qdbench <file> [MiB-per-read]");
        println!();
        println!("  How fast this drive reads expert-sized blocks, against how");
        println!("  many reads are in flight. Rung 1 of the 5 tok/s ladder is");
        println!("  worth building only if this rises with depth.");
        println!();
        println!("  Point it at a file much larger than RAM, or the page cache");
        println!("  answers instead of the drive.");
        return;
    }

    let path = args[0].clone();
    let block_mib: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(13);
    let block = block_mib * 1024 * 1024;

    let len = match std::fs::metadata(&path) {
        Ok(m) => m.len(),
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            std::process::exit(1);
        }
    };
    println!();
    println!("file    {path}");
    println!(
        "size    {:.1} GiB, reading {block_mib} MiB per request",
        len as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!();
    println!("{:>6}  {:>12}  {:>10}", "depth", "GiB/s", "vs depth 1");

    // Each depth reads a fresh slice of the file, so nothing is answered from
    // the page cache that a previous depth put there.
    let mut cursor: u64 = 0;
    let mut baseline = 0.0f64;
    for depth in [1usize, 2, 4, 8, 16] {
        let per_thread = 6u64; // requests each worker issues
        let total = depth as u64 * per_thread;
        if cursor + total * block > len {
            cursor = 0; // wrap rather than stop; a wrapped read is still a read
        }
        let start_at = cursor;
        cursor += total * block;

        let read = Arc::new(AtomicU64::new(0));
        let t0 = Instant::now();
        let mut hs = Vec::new();
        for w in 0..depth {
            let path = path.clone();
            let read = Arc::clone(&read);
            hs.push(std::thread::spawn(move || {
                let Ok(mut f) = File::open(&path) else { return };
                let mut buf = vec![0u8; block as usize];
                for i in 0..per_thread {
                    let off = start_at + (w as u64 * per_thread + i) * block;
                    if f.seek(SeekFrom::Start(off)).is_err() {
                        return;
                    }
                    match f.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            read.fetch_add(n as u64, Ordering::Relaxed);
                            // Touch it, so the read cannot be elided and the
                            // pages are really faulted in.
                            std::hint::black_box(buf[n - 1]);
                        }
                        _ => return,
                    }
                }
            }));
        }
        for h in hs {
            let _ = h.join();
        }
        let secs = t0.elapsed().as_secs_f64();
        let gibs = read.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0 * 1024.0) / secs;
        if baseline == 0.0 {
            baseline = gibs;
        }
        let ratio = if baseline > 0.0 {
            format!("{:.2}x", gibs / baseline)
        } else {
            "-".into()
        };
        println!("{depth:>6}  {gibs:>12.2}  {ratio:>10}");
    }
    println!();
    println!("If this is flat, rung 1 of the ladder is closed and V4-Flash stays");
    println!("near 0.43 tok/s until the model itself is made smaller.");
}
