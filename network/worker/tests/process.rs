//! The worker as a separate process, which is the only way it will ever run.
//!
//! `loopback.rs` starts the serving loop on a thread. That proves the protocol
//! and the arithmetic, and it does **not** prove the binary: argument parsing,
//! the range syntax, the bind, and the fact that a second process can reach it
//! at all are none of them exercised by a thread in the test's own address
//! space.
//!
//! This spawns `chaos-worker.exe` the way a person would and talks to it over a
//! real socket.

use chaos_worker::serve::Client;
use chaos_worker::wire::{Compute, Job};
use std::process::{Child, Command, Stdio};

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

/// The binary beside this test's own executable, which is where cargo puts it.
fn worker_exe() -> std::path::PathBuf {
    let mut p = std::env::current_exe().expect("test exe");
    p.pop(); // deps/
    p.pop();
    p.join(format!("chaos-worker{}", std::env::consts::EXE_SUFFIX))
}

/// Kills the child on the way out, however the test ends.
struct Reaped(Child);
impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore]
fn the_binary_serves_what_it_was_told_to_hold() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };
    let exe = worker_exe();
    if !exe.exists() {
        eprintln!("{} is not built; skipping", exe.display());
        return;
    }

    // A port nobody else is on. Not the default: a worker Atur left running
    // would make this test pass against the wrong process, which is worse than
    // failing.
    let addr = "127.0.0.1:18232";
    let child = Command::new(&exe)
        .arg(&path)
        .args(["--experts", "4-9", "--layers", "0-1", "--bind", addr])
        // Qwen3's MoE applies no clamp; an infinite limit is what that means.
        .args(["--clamp", "inf"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn chaos-worker");
    let _reaped = Reaped(child);

    // It has to read weights before it listens, so connect on a retry rather
    // than a sleep somebody tuned once.
    let mut client = None;
    for _ in 0..100 {
        if let Ok(c) = Client::connect(addr) {
            client = Some(c);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let mut client = client.expect("the worker never came up");

    // The range syntax means what it says.
    assert_eq!(client.held.experts, vec![4, 5, 6, 7, 8, 9]);
    assert_eq!(client.held.layers, vec![0, 1]);
    assert!(client.held.bytes > 0);
    assert!(
        client.held.model.contains("Qwen3-30B"),
        "the worker named itself {:?}",
        client.held.model
    );

    let width = client.held.width;
    let hidden: Vec<f32> = (0..width as usize)
        .map(|i| (i as f32 * 3e-4).cos())
        .collect();
    let ans = client
        .compute(&Compute {
            layer: 1,
            tokens: 1,
            width,
            jobs: vec![
                Job {
                    token: 0,
                    expert: 4,
                },
                Job {
                    token: 0,
                    expert: 9,
                },
            ],
            hidden: hidden.clone(),
        })
        .expect("compute over a real socket");

    assert_eq!(ans.jobs, 2);
    assert_eq!(ans.values.len(), 2 * width as usize);
    assert!(
        ans.block(0).unwrap().iter().any(|v| v.abs() > 1e-6),
        "the first expert produced nothing"
    );
    assert_ne!(
        ans.block(0).unwrap(),
        ans.block(1).unwrap(),
        "two different experts produced identical activations"
    );

    // An expert outside its assigned range is refused, and the connection
    // survives it — a main device's answer to a miss is to read locally, not
    // to reconnect.
    let err = client
        .compute(&Compute {
            layer: 1,
            tokens: 1,
            width,
            jobs: vec![Job {
                token: 0,
                expert: 0,
            }],
            hidden: hidden.clone(),
        })
        .expect_err("expert 0 was not assigned to this worker");
    assert!(format!("{err}").contains('0'), "{err}");

    let again = client
        .compute(&Compute {
            layer: 0,
            tokens: 1,
            width,
            jobs: vec![Job {
                token: 0,
                expert: 5,
            }],
            hidden,
        })
        .expect("the connection survived the refusal");
    assert_eq!(again.jobs, 1);
}
