//! Does a worker compute the expert the main device asked for?
//!
//! # Why this test and not a smaller one
//!
//! **A wrong forward pass produces fluent nonsense, never a crash.** A worker
//! that returns the wrong expert's activations returns a block of exactly the
//! right shape, full of plausible floats, and the model goes on to write
//! confident rubbish with nothing in any log. There is no assertion inside the
//! pipeline that would catch it.
//!
//! So the check is a *differential* one, and it is the sharpest available
//! without a second engine: **hold the same expert two different ways and
//! require the same answer, bit for bit.**
//!
//! - Worker A holds experts 0..4 packed together; expert 3 sits at position 3.
//! - Worker B holds only expert 3; it sits at position 0.
//!
//! Ask both for expert 3. If the packing, the position lookup or the
//! `mul_mat_id` indices are wrong in any way that depends on where an expert
//! sits — which is every way they can be wrong — the two disagree.
//!
//! The test is ablated below: `different_experts_are_actually_different`
//! fails if the ids are ignored entirely, which is the one bug the differential
//! check above cannot see.
//!
//! # Why it is `#[ignore]`
//!
//! It needs a real MoE container. `cargo test -- --ignored` runs it; CI does
//! not have the weights.

use chaos_worker::slice::Slice;
use chaos_worker::wire::Job;

/// Where the MoE container is on this machine.
///
/// Qwen3-30B-A3B: 128 experts, 8 used, 48 blocks. Loading four experts of one
/// layer is a few megabytes, so this runs in seconds despite the container
/// being 17 GB.
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

/// A hidden state that is not symmetric in any direction.
///
/// **Not zeros and not a constant.** Either would make most indexing bugs
/// invisible: every expert applied to a constant vector gives *an* answer, and
/// two wrong experts can easily agree on a degenerate input.
fn hidden(width: usize) -> Vec<f32> {
    (0..width)
        .map(|i| ((i as f32 * 0.017).sin() * 0.3) + (i as f32 * 1e-4))
        .collect()
}

/// The clamp. Qwen3's MoE applies none, so an infinite limit is the honest
/// value — `clamp(-inf, inf)` is what "no clamp" means and keeps the graph the
/// same shape as the architecture that does clamp.
const NO_CLAMP: f32 = f32::INFINITY;

#[test]
#[ignore]
fn the_same_expert_gives_the_same_answer_wherever_it_sits() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };

    let many = Slice::load(&path, &[0], &[0, 1, 2, 3], NO_CLAMP, |_, _| {}).expect("load many");
    let one = Slice::load(&path, &[0], &[3], NO_CLAMP, |_, _| {}).expect("load one");

    assert_eq!(many.width, one.width);
    assert_eq!(many.held, vec![0, 1, 2, 3]);
    assert_eq!(one.held, vec![3]);

    let w = many.width as usize;
    let h = hidden(w);
    let jobs = [Job {
        token: 0,
        expert: 3,
    }];

    let a = many
        .compute(0, &jobs, 1, many.width, &h, 4)
        .expect("compute many");
    let b = one
        .compute(0, &jobs, 1, one.width, &h, 4)
        .expect("compute one");

    assert_eq!(a.len(), w, "one block of {w} floats");
    assert_eq!(b.len(), w);
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(
            x.to_bits(),
            y.to_bits(),
            "element {i}: expert 3 at position 3 gave {x}, at position 0 gave {y}"
        );
    }

    // And it is not all zeros, which would make the comparison above pass
    // while proving nothing at all.
    assert!(
        a.iter().any(|v| v.abs() > 1e-6),
        "the activation is entirely zero -- the weights are not being read"
    );
}

/// **The ablation.** If `mul_mat_id`'s indices were ignored — every job
/// computing expert 0, say — the differential test above would still pass,
/// because both workers would be equally wrong. Two different experts must
/// give two different answers.
#[test]
#[ignore]
fn different_experts_are_actually_different() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };
    let s = Slice::load(&path, &[0], &[0, 1, 2, 3], NO_CLAMP, |_, _| {}).expect("load");
    let w = s.width as usize;
    let h = hidden(w);

    let mut answers = Vec::new();
    for e in [0u32, 1, 2, 3] {
        let out = s
            .compute(
                0,
                &[Job {
                    token: 0,
                    expert: e,
                }],
                1,
                s.width,
                &h,
                4,
            )
            .expect("compute");
        answers.push(out);
    }
    for i in 0..answers.len() {
        for j in i + 1..answers.len() {
            assert_ne!(
                answers[i], answers[j],
                "experts {i} and {j} gave identical activations -- the ids are being ignored"
            );
        }
    }
}

/// Several jobs in one request come back in the order they were asked for, and
/// each matches what that expert gives on its own.
///
/// **The order is the whole contract**: the main device knows where each block
/// goes only because it knows what it asked for.
#[test]
#[ignore]
fn a_batch_comes_back_in_the_order_it_was_asked_for() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };
    let s = Slice::load(&path, &[0], &[0, 1, 2, 3], NO_CLAMP, |_, _| {}).expect("load");
    let w = s.width as usize;
    let h = hidden(w);

    // Deliberately out of order, and with a repeat: nothing about the wire
    // says jobs are sorted or unique.
    let order = [3u32, 0, 2, 3];
    let jobs: Vec<Job> = order
        .iter()
        .map(|&e| Job {
            token: 0,
            expert: e,
        })
        .collect();
    let batch = s.compute(0, &jobs, 1, s.width, &h, 4).expect("batch");
    assert_eq!(batch.len(), w * order.len());

    for (slot, &e) in order.iter().enumerate() {
        let alone = s
            .compute(
                0,
                &[Job {
                    token: 0,
                    expert: e,
                }],
                1,
                s.width,
                &h,
                4,
            )
            .expect("alone");
        let got = &batch[slot * w..(slot + 1) * w];
        for (i, (x, y)) in got.iter().zip(&alone).enumerate() {
            assert_eq!(
                x.to_bits(),
                y.to_bits(),
                "slot {slot} (expert {e}) element {i}: batched {x}, alone {y}"
            );
        }
    }
}

/// Two tokens routing to different experts, which is the case a flat
/// per-batch expert list gets silently wrong.
#[test]
#[ignore]
fn each_token_gets_its_own_expert() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };
    let s = Slice::load(&path, &[0], &[0, 1, 2, 3], NO_CLAMP, |_, _| {}).expect("load");
    let w = s.width as usize;

    // Two different hidden states, so a token mix-up cannot hide.
    let mut h = hidden(w);
    h.extend(hidden(w).iter().map(|v| -v * 0.7));

    let jobs = [
        Job {
            token: 0,
            expert: 1,
        },
        Job {
            token: 1,
            expert: 2,
        },
    ];
    let both = s.compute(0, &jobs, 2, s.width, &h, 4).expect("both");

    let a = s
        .compute(
            0,
            &[Job {
                token: 0,
                expert: 1,
            }],
            1,
            s.width,
            &h[..w],
            4,
        )
        .expect("token 0 alone");
    let b = s
        .compute(
            0,
            &[Job {
                token: 0,
                expert: 2,
            }],
            1,
            s.width,
            &h[w..],
            4,
        )
        .expect("token 1 alone");

    assert_eq!(&both[..w], &a[..], "token 0 took the wrong hidden state");
    assert_eq!(&both[w..], &b[..], "token 1 took the wrong hidden state");
}

/// **An expert this worker does not hold is an error, not an abort.**
///
/// ggml does not return errors for a bad index — it aborts the process, which
/// on a worker means the main device sees a closed socket and no reason. The
/// bounds check has to happen on the Rust side, before any tensor exists.
#[test]
#[ignore]
fn an_expert_this_worker_does_not_hold_is_refused() {
    let Some(path) = model() else {
        eprintln!("no MoE container on this machine; skipping");
        return;
    };
    let s = Slice::load(&path, &[0], &[0, 1], NO_CLAMP, |_, _| {}).expect("load");
    let h = hidden(s.width as usize);

    let err = s
        .compute(
            0,
            &[Job {
                token: 0,
                expert: 99,
            }],
            1,
            s.width,
            &h,
            4,
        )
        .expect_err("expert 99 is not held");
    assert!(format!("{err}").contains("99"), "{err}");

    // A layer it does not hold, likewise.
    assert!(s
        .compute(
            7,
            &[Job {
                token: 0,
                expert: 0
            }],
            1,
            s.width,
            &h,
            4
        )
        .is_err());

    // And a hidden state of the wrong width, which would otherwise be a
    // matmul against the wrong shape.
    assert!(s
        .compute(
            0,
            &[Job {
                token: 0,
                expert: 0
            }],
            1,
            s.width + 1,
            &h,
            4
        )
        .is_err());
}
