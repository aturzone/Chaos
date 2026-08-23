# The worker protocol, measured on loopback (2026-08-24)

**The plan's own instruction was: protocol, then a worker that computes, then
*measure and stop and report*.** This is the report. Nothing has been wired into
the forward pass, and nothing should be until these numbers are read.

`network/worker` — `chaos-worker`, the wire, and
`tests/{parity,loopback}.rs`. Reproduce with:

```
cargo test --release -p chaos-worker --test loopback -- --ignored --nocapture
```

## What was measured

Two processes on one machine, a real `Qwen3-30B-A3B-Q4_K_M` container, one
layer, six of its experts held resident, 200 exchanges after 20 discarded.

```
=== one exchange, loopback, 2048-wide container ===
  request             8268 bytes
  answer             49172 bytes  (6 experts)
  round trip         0.838 ms   (arithmetic + framing + syscalls)
```

Scaled to a V4-Flash token — 43 layers, 6 of 256 experts, 4096 wide:

| | |
|---|---|
| on the wire | **4.94 MB** |
| transmission at 1 GbE | **39.5 ms** — arithmetic, not measured |
| protocol cost | **36.0 ms** — measured, and paid on any link |
| replaces | **1560 ms** of local expert reads |
| out of | 2400 ms a token costs today |

**≈76 ms to replace 1560 ms.** A token would cost about 916 ms — **1.09 tok/s
against 0.42 today, 2.6x** — *if* the experts are resident on the workers.

## Three things this does not say

**Loopback is not a network.** Only the protocol's own cost is measured here;
the transmission line is arithmetic from a measured byte count. On a real LAN
each exchange also pays a round-trip latency, and there are 43 of them per
token — at 0.3 ms RTT that is another ~13 ms, at 2 ms it is 86 ms and the
saving halves. **The next measurement is on two machines, and it is the one
that matters.**

**"If the experts are resident" is the whole condition.** 1.09 tok/s assumes a
worker answers from RAM. Pooling 144 GB across machines is what makes that
true, and nothing here has done it.

**The ceiling has not moved.** 0.84 s of every V4-Flash token never touches the
disk, so full residency across devices still lands near **1.19 tok/s**. This
result is *consistent with* that ceiling, not an escape from it. Four machines
get single-digit tok/s, not 20.

## The doc said 6.9 MB and the measurement says 4.94 — both are right

`devices-as-resources.md` assumed the six experts were spread across four
workers: the hidden state goes out **four times** (once per worker) and six
answers come back, so ~10 × 16 KB × 43 = 6.9 MB.

This measurement has one worker holding all six: one request out, six answers
back, 7 × 16 KB × 43 = 4.94 MB.

So the traffic depends on **how many workers a token's experts are spread
over**, not on how many workers exist — and it is minimised by *concentrating*
each token's experts, which is the opposite of what a naive round-robin
assignment does. That is a real finding and it belongs in the assignment
policy: bin-pack experts so that co-routed ones land together.

## The correctness result, which matters more than the speed

`tests/loopback.rs::a_worker_over_a_socket_agrees_with_the_local_path`:
activations computed on a worker, over a real TCP socket, are **bit-identical**
to the same experts computed locally.

That is not a formality. **A wrong forward pass produces fluent nonsense, never
a crash** — a worker returning the wrong expert returns a block of the right
shape full of plausible floats, and the model writes confident rubbish with
nothing in any log.

`tests/parity.rs` is the differential check, and it found a real bug:

> `WeightSet::bind` collapses every dimension past the first —
> `[a, rest @ ..] => (a, product(rest))` — so a stack bound as
> `[n_embd, n_ff, n_held]` arrives as `[n_embd, n_ff * n_held]`. `mul_mat_id`
> then reads `ne[1]` as the output width and produces a gate `n_held` times too
> wide. **ggml aborted the whole test binary** inside the *down* matmul, two ops
> later, with no Rust frame: `GGML_ASSERT(as->ne[0] == b->ne[0])`.

The reference path reshapes after binding for exactly this reason. The test
that caught it holds the same expert two ways — packed at position 3 among
four, and alone at position 0 — and requires the same answer bit for bit. It is
ablated by `different_experts_are_actually_different`, which fails if the ids
were ignored entirely, the one bug the differential check cannot see.

## What to do next, in order

1. **Two machines.** Everything above is one number away from being about a
   real network, and that number is a LAN round trip. Until then this is a
   protocol that works and a saving that is projected.
2. **Assignment that concentrates co-routed experts**, per the finding above.
   `core/plan` already scores residency policies; this is the same scoring with
   device identity added — and **out of sample**, with a uniform null, because
   in-sample hot sets have lied here twice.
3. **Then** wire it into the forward pass, with local-disk fallback, and
   measure against the single-machine baseline alternating in one session.

**Not yet done and deliberately so**: discovery, tensor-parallel, and any
integration with `deepseek4_forward`. The plan says measure and stop. This is
the stop.
