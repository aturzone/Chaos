//! Two processes, one machine: does the protocol cost what the arithmetic says?
//!
//! # The claim being tested
//!
//! `docs/graph/backlog/devices-as-resources.md` says a token's worth of
//! expert-parallel traffic is **~8 MB there and back**, which is **~66 ms over
//! 1 GbE** — against **~1560 ms of local disk** that it replaces. Every
//! decision in this design rests on that comparison.
//!
//! Two of those three numbers can be checked here and one cannot:
//!
//! * **The bytes** are exact and are checked. If a token does not move roughly
//!   what the doc says, the doc is wrong and so is everything built on it.
//! * **The protocol's own overhead** — framing, syscalls, thread handoff — is
//!   measured on loopback, where the wire is free. Whatever it costs here, it
//!   costs on a real link *as well as* the transmission time.
//! * **The link** cannot be measured on one machine. Loopback is not a gigabit
//!   ethernet and pretending otherwise would be the exact species of claim this
//!   project keeps retracting. The transmission time is arithmetic from the
//!   measured byte count, stated as arithmetic.
//!
//! Run with `cargo test -p chaos-worker --test loopback -- --ignored --nocapture`.

use chaos_worker::serve::{self, Client};
use chaos_worker::slice::Slice;
use chaos_worker::wire::{Compute, Job};
use std::net::TcpListener;
use std::sync::Arc;

fn model() -> Option<String> {
    for p in [
        r"C:\Projects\models\qwen3moe\Qwen3-30B-A3B-Q4_K_M.gguf",
        r"C:\Users\atur\.chaos\models\Qwen3-30B-A3B-Q4_K_M.gguf",
    ] {
        if std::path::Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    None
}

/// V4-Flash's real shape, for the arithmetic. Measured off the container:
/// 43 blocks, 6 of 256 experts per token, 4096 wide.
const V4_LAYERS: u64 = 43;
const V4_USED: u64 = 6;
const V4_WIDTH: u64 = 4096;
/// 1 GbE, the link the design assumes.
const GBE_BYTES_PER_SEC: f64 = 125_000_000.0;
/// What a V4-Flash token costs today, and how much of it is expert reads.
const TOKEN_MS_TODAY: f64 = 2400.0;
const DISK_MS_TODAY: f64 = 1560.0;

fn start_worker(slice: Slice) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let h = std::thread::spawn(move || {
        let _ = serve::serve(listener, Arc::new(slice), 4, |_| {});
    });
    (addr, h)
}

/// A worker answers what it holds, and the answer is the same one the local
/// path produces — end to end, over a real socket.
#[test]
#[ignore]
fn a_worker_over_a_socket_agrees_with_the_local_path() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };
    let local = Slice::load(&path, &[0], &[0, 1, 2, 3], f32::INFINITY, |_, _| {}).expect("load");
    let width = local.width;
    let hidden: Vec<f32> = (0..width as usize)
        .map(|i| ((i as f32 * 0.017).sin() * 0.3) + (i as f32 * 1e-4))
        .collect();

    let remote = Slice::load(&path, &[0], &[0, 1, 2, 3], f32::INFINITY, |_, _| {}).expect("load");
    let (addr, _h) = start_worker(remote);

    let mut client = Client::connect(&addr).expect("connect");
    assert_eq!(client.held.width, width);
    assert_eq!(client.held.experts, vec![0, 1, 2, 3]);
    assert!(client.holds(0, 2));
    assert!(!client.holds(0, 9), "it does not hold expert 9");
    assert!(!client.holds(5, 2), "it does not hold layer 5");

    let jobs = vec![
        Job {
            token: 0,
            expert: 1,
        },
        Job {
            token: 0,
            expert: 3,
        },
    ];
    let req = Compute {
        layer: 0,
        tokens: 1,
        width,
        jobs: jobs.clone(),
        hidden: hidden.clone(),
    };
    let ans = client.compute(&req).expect("compute");
    let here = local
        .compute(0, &jobs, 1, width, &hidden, 4)
        .expect("local");

    assert_eq!(ans.jobs, 2);
    assert_eq!(ans.values.len(), here.len());
    for (i, (a, b)) in ans.values.iter().zip(&here).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "element {i}: over the socket {a}, locally {b}"
        );
    }
}

/// **An error comes back as a frame and the connection survives.**
///
/// The main device's answer to "I do not hold that" is to read the expert from
/// its own disk. It should not have to reconnect to ask the next question, and
/// a worker that closed the socket would make a recoverable miss look like a
/// dead machine.
#[test]
#[ignore]
fn a_refusal_does_not_end_the_conversation() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };
    let slice = Slice::load(&path, &[0], &[0, 1], f32::INFINITY, |_, _| {}).expect("load");
    let width = slice.width;
    let (addr, _h) = start_worker(slice);
    let mut client = Client::connect(&addr).expect("connect");
    let hidden = vec![0.5f32; width as usize];

    let bad = Compute {
        layer: 0,
        tokens: 1,
        width,
        jobs: vec![Job {
            token: 0,
            expert: 99,
        }],
        hidden: hidden.clone(),
    };
    let err = client.compute(&bad).expect_err("expert 99 is not held");
    assert!(format!("{err}").contains("99"), "{err}");

    // The same connection still works.
    let good = Compute {
        layer: 0,
        tokens: 1,
        width,
        jobs: vec![Job {
            token: 0,
            expert: 1,
        }],
        hidden,
    };
    let ans = client.compute(&good).expect("the connection survived");
    assert_eq!(ans.jobs, 1);
}

/// **The measurement.** What a token's worth of expert-parallel traffic
/// actually costs, and what the doc predicted.
#[test]
#[ignore]
fn what_a_token_of_expert_parallel_actually_costs() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };

    // One layer of a real container is enough: a token is the same exchange 43
    // times, and holding 43 layers to measure it would read gigabytes to learn
    // nothing extra.
    let slice =
        Slice::load(&path, &[0], &[0, 1, 2, 3, 4, 5], f32::INFINITY, |_, _| {}).expect("load");
    let width = slice.width;
    let (addr, _h) = start_worker(slice);
    let mut client = Client::connect(&addr).expect("connect");

    // Six experts for one token: V4-Flash's `expert_used_count`.
    let jobs: Vec<Job> = (0..V4_USED as u32)
        .map(|e| Job {
            token: 0,
            expert: e,
        })
        .collect();
    let hidden: Vec<f32> = (0..width as usize)
        .map(|i| (i as f32 * 1e-4).sin())
        .collect();
    let req = Compute {
        layer: 0,
        tokens: 1,
        width,
        jobs,
        hidden,
    };

    // Warm the path: the first exchange pays a connection's worth of
    // first-touch that no later one does, and this project's rule is to
    // discard exactly that.
    for _ in 0..20 {
        client.compute(&req).expect("warm");
    }
    let before = client.timing;

    const ROUNDS: u32 = 200;
    let t = std::time::Instant::now();
    for _ in 0..ROUNDS {
        client.compute(&req).expect("compute");
    }
    let elapsed = t.elapsed().as_secs_f64();
    let after = client.timing;

    let per_exchange_ms = elapsed / f64::from(ROUNDS) * 1e3;
    let bytes_in = (after.bytes_in - before.bytes_in) / u64::from(ROUNDS);
    let bytes_out = (after.bytes_out - before.bytes_out) / u64::from(ROUNDS);
    let both = bytes_in + bytes_out;

    // Scale one layer's exchange to a whole V4-Flash token. The width differs
    // between this container and V4-Flash, so the bytes are scaled too --
    // stated rather than hidden, because a measurement quietly reported at the
    // wrong shape is worse than none.
    let scale = V4_WIDTH as f64 / f64::from(width);
    let token_bytes = both as f64 * scale * V4_LAYERS as f64;
    let wire_ms = token_bytes / GBE_BYTES_PER_SEC * 1e3;
    let overhead_ms = per_exchange_ms * V4_LAYERS as f64;

    println!();
    println!("=== one exchange, loopback, {width}-wide container ===");
    println!("  request        {bytes_in:>9} bytes");
    println!(
        "  answer         {bytes_out:>9} bytes  ({} experts)",
        V4_USED
    );
    println!("  round trip     {per_exchange_ms:>9.3} ms   (arithmetic + framing + syscalls)");
    println!();
    println!("=== scaled to one V4-Flash token: {V4_LAYERS} layers, {V4_USED} of 256 experts, {V4_WIDTH} wide ===");
    println!("  on the wire    {:>9.2} MB", token_bytes / 1e6);
    println!("  the doc said   {:>9.2} MB", 6.9);
    println!("  transmission   {wire_ms:>9.1} ms   at 1 GbE, ARITHMETIC not measured");
    println!("  protocol cost  {overhead_ms:>9.1} ms   MEASURED on loopback, paid on any link");
    println!();
    println!("  replaces       {DISK_MS_TODAY:>9.1} ms   of local expert reads");
    println!("  out of         {TOKEN_MS_TODAY:>9.1} ms   a token costs today");
    println!();
    let saved = DISK_MS_TODAY - wire_ms - overhead_ms;
    if saved > 0.0 {
        println!(
            "  => {saved:.0} ms of {DISK_MS_TODAY:.0} recoverable, IF the experts are resident on the workers."
        );
        println!(
            "     A token would then cost about {:.0} ms, or {:.2} tok/s -- against 0.42 today.",
            TOKEN_MS_TODAY - saved,
            1000.0 / (TOKEN_MS_TODAY - saved)
        );
    } else {
        println!("  => the network costs MORE than the disk it replaces. The design is dead.");
    }
    println!();
    println!("  Loopback is not a network. The transmission line above is arithmetic");
    println!("  from a measured byte count; only the protocol cost is measured, and it");
    println!("  is paid on top of transmission rather than instead of it.");

    // The bytes are the part that can be asserted. A token that moved an order
    // of magnitude more than the doc claims would invalidate the design, and
    // the doc, silently.
    assert!(
        token_bytes < 20e6,
        "a token moves {:.1} MB, against the ~6.9 MB the design assumes",
        token_bytes / 1e6
    );
    // And the protocol itself must not be the cost. If framing alone were
    // comparable to the disk read, there would be nothing to gain.
    assert!(
        overhead_ms < DISK_MS_TODAY,
        "the protocol costs {overhead_ms:.0} ms/token before any wire time"
    );
}
