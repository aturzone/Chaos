//! `chaos-worker` — hold expert weights in RAM, answer with activations.
//!
//! ```text
//! chaos-worker <model.gguf> --experts 0-63 --bind 0.0.0.0:8232
//! ```
//!
//! # What this is for
//!
//! Chaos's limit is not arithmetic. It is that **3.3 GB must be read per token**
//! on V4-Flash, and a token costs 2.4 s: 1.56 s of expert reads plus 0.84 s of
//! compute that never touches the disk. The frontier is memory — and other
//! machines on a LAN have memory.
//!
//! The shape of the model makes that unusually favourable. A hidden state is
//! **16 KB**; a token's expert weights are **3.3 GB**. So:
//!
//! > **Send the work to the weights, never the weights to the work.**
//!
//! A worker holds a slice of the experts resident and answers with activations.
//! Moving activations costs 0.2–3% of a token. Moving weights costs eleven
//! times the whole token, and serving them over 1 GbE is *twenty times slower
//! than the NVMe this project already streams them from*.
//!
//! # The honest ceiling, before anybody builds on this
//!
//! Pooling RAM moves along the measured frontier — 16 GB 0.42 tok/s, 64 GB
//! 0.55, 128 GB 0.93, 160 GB 1.19 — so full residency across devices lands near
//! **1.19 tok/s**, because 0.84 s of every token never touches the disk and
//! pooling memory does nothing for it. **Four machines get single-digit tok/s
//! on V4-Flash, not 20.** Distributed makes big models *usable*; it does not
//! make them fast. `docs/graph/backlog/devices-as-resources.md` has the
//! arithmetic.

use chaos_worker::serve;
use chaos_worker::slice::Slice;
use std::net::TcpListener;
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut path = String::new();
    let mut bind = String::from("127.0.0.1:8232");
    let mut experts_arg = String::new();
    let mut layers_arg = String::new();
    let mut threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut limit = f32::INFINITY;

    let mut i = 0;
    while i < args.len() {
        let take = |i: usize| args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--bind" => {
                bind = take(i);
                i += 2;
            }
            "--experts" => {
                experts_arg = take(i);
                i += 2;
            }
            "--layers" => {
                layers_arg = take(i);
                i += 2;
            }
            "-t" | "--threads" => {
                threads = take(i).parse().unwrap_or(threads);
                i += 2;
            }
            "--clamp" => {
                limit = take(i).parse().unwrap_or(limit);
                i += 2;
            }
            "-h" | "--help" => {
                usage();
                return ExitCode::SUCCESS;
            }
            "--version" => {
                println!("chaos-worker {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("chaos-worker: unknown option {other:?}\n");
                usage();
                return ExitCode::from(2);
            }
            other => {
                if path.is_empty() {
                    path = other.to_string();
                }
                i += 1;
            }
        }
    }

    if path.is_empty() {
        usage();
        return ExitCode::from(2);
    }

    let model = match chaos_model::Model::open_split(&path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("chaos-worker: {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let n_expert = model.arch_u64("expert_count").unwrap_or(0) as u32;
    let n_layer = model.arch_u64("block_count").unwrap_or(0) as u32;
    if n_expert == 0 {
        eprintln!("chaos-worker: {path} has no routed experts, so there is nothing to hold.");
        eprintln!("             A worker exists to keep experts in memory; a dense model");
        eprintln!("             has none, and `chaos-run` is what you want.");
        return ExitCode::FAILURE;
    }
    drop(model);

    let experts = match parse_range(&experts_arg, n_expert) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("chaos-worker: --experts {experts_arg:?}: {e}");
            return ExitCode::from(2);
        }
    };
    let layers = match parse_range(&layers_arg, n_layer) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("chaos-worker: --layers {layers_arg:?}: {e}");
            return ExitCode::from(2);
        }
    };

    // **What this will cost, before it is spent.** Reading a slice of a 144 GB
    // container is minutes and gigabytes; a worker that started that silently
    // and then ran out of memory would be the worst version of this.
    let free = chaos_probe::Machine::probe(std::path::Path::new("."), false)
        .ram_available_bytes
        .unwrap_or(0);
    println!("model      {path}");
    println!(
        "holding    {} of {n_expert} experts across {} of {n_layer} layers",
        experts.len(),
        layers.len()
    );

    let started = std::time::Instant::now();
    let mut last = std::time::Instant::now();
    let slice = match Slice::load(&path, &layers, &experts, limit, |il, so_far| {
        if last.elapsed().as_secs_f64() > 2.0 {
            eprint!(
                "\r  reading    layer {il}, {:.2} GiB so far      ",
                so_far as f64 / (1u64 << 30) as f64
            );
            last = std::time::Instant::now();
        }
    }) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("\nchaos-worker: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprint!("\r                                                        \r");

    let gib = slice.bytes as f64 / (1u64 << 30) as f64;
    println!(
        "resident   {gib:.2} GiB in {:.1}s ({:.2} GiB/s)",
        started.elapsed().as_secs_f64(),
        gib / started.elapsed().as_secs_f64().max(1e-9)
    );
    if slice.bytes > free {
        // Said, not enforced: the OS may well have found the memory since the
        // probe, and refusing after the read would waste the read.
        println!(
            "           NOTE: that is more than the {:.2} GiB the probe called free.",
            free as f64 / (1u64 << 30) as f64
        );
        println!("           If this machine starts swapping, hold fewer experts.");
    }

    let listener = match TcpListener::bind(&bind) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("chaos-worker: cannot listen on {bind}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("ready      {bind}");
    // **Said plainly, because it is the first thing a user needs to know.**
    // The protocol works and is measured; what does not exist yet is a main
    // device that speaks it. Shipping a binary that looks finished and quietly
    // has nobody to talk to is worse than not shipping it.
    println!();
    println!("           NOTHING CONNECTS TO THIS YET. `chaos-serve` has no --workers");
    println!("           flag: the protocol and the arithmetic are proven and measured");
    println!("           (research/worker-protocol-measured-2026-08-24.md), and wiring");
    println!("           it into the forward pass waits on a two-machine measurement.");
    println!();
    println!(
        "           a hidden state is {} bytes; a token's experts on this",
        slice.width * 4
    );
    println!("           model are gigabytes. That ratio is the whole design.");

    if let Err(e) = serve::serve(listener, Arc::new(slice), threads, |peer| {
        println!("connected  {peer}");
    }) {
        eprintln!("chaos-worker: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// `0-63`, `0,4,9`, `0-3,120-127`, or empty for everything up to `n`.
///
/// **Empty means all**, because the common case on a first run is one worker
/// holding everything, and making that spell out `0-255` is friction on the
/// only command anybody types twice.
fn parse_range(s: &str, n: u32) -> Result<Vec<u32>, String> {
    if n == 0 {
        return Err("the container declares none of these".into());
    }
    let s = s.trim();
    if s.is_empty() || s == "all" {
        return Ok((0..n).collect());
    }
    let mut out = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (lo, hi) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let lo: u32 = lo.parse().map_err(|_| format!("{lo:?} is not a number"))?;
        let hi: u32 = hi.parse().map_err(|_| format!("{hi:?} is not a number"))?;
        if lo > hi {
            return Err(format!("{lo}-{hi} counts backwards"));
        }
        if hi >= n {
            return Err(format!("{hi} is past the last one, which is {}", n - 1));
        }
        out.extend(lo..=hi);
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err("no values".into());
    }
    Ok(out)
}

fn usage() {
    println!(
        "chaos-worker {} -- hold expert weights in RAM, answer with activations",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("usage: chaos-worker <model.gguf> [options]");
    println!();
    println!("  --bind ADDR      where to listen (default 127.0.0.1:8232)");
    println!("  --experts RANGE  which experts to hold: 0-63, 0,4,9, or all (default all)");
    println!("  --layers RANGE   which layers (default all)");
    println!("  -t, --threads N  threads for the expert matmuls");
    println!("  --clamp F        clamp gate and up to +/-F (deepseek4 wants this; Qwen3 does not)");
    println!();
    println!("A worker is chaos-run without a token loop. It reads its slice of the");
    println!("experts once, holds them in memory, and answers 16 KB questions with");
    println!("16 KB answers. The main device keeps routing, sampling and the KV cache,");
    println!("so a worker that dies is a slowdown and not a corruption.");
    println!();
    println!("NOTHING CONNECTS TO THIS YET. chaos-serve has no --workers flag; the");
    println!("protocol and the arithmetic are proven and measured, and wiring it into");
    println!("the forward pass waits on a measurement across two real machines.");
    println!();
    println!("BEFORE BUILDING ON THIS: four machines get single-digit tok/s on");
    println!("V4-Flash, not 20. Pooling RAM moves along the measured frontier and");
    println!("0.84s of every token never touches the disk. Distributed makes big");
    println!("models usable; it does not make them fast.");
}
