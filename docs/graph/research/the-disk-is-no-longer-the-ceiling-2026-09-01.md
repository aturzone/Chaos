---
topic: with the expert cache on, a V4-Flash token is 63% disk and 37% arithmetic — and removing ALL disk traffic lands at 1.5–1.8 tok/s, so the arithmetic is now what caps this machine
status: resolved
links:
  - the-expert-cache-was-off-2026-09-01.md
  - the-tail-computed-once-2026-08-31.md
  - what-f-is-made-of-2026-08-31.md
  - ../backlog/the-big-bang-5-tok-s.md
---

# The disk is no longer the ceiling

The standing 5 tok/s argument is a disk argument: *137 GiB of experts cannot be
resident, so 3.22 GiB crosses a 3.41 GiB/s drive every token, and even one-bit
experts with a free trunk and zero arithmetic cap this machine at 4.26 tok/s.*

**That argument assumed every expert byte comes off the disk, and since
2026-09-01 that is false.** The expert cache serves 28.3% of the reads. So the
ceiling has to be re-derived, and when it is, the binding constraint changes.

## The profile

`CHAOS_BLOCK_TIMING`, **generation passes only** — the prefill pass is excluded
because it reads ~123 experts per layer against a generated token's 6, and
averaging the two together inflates the disk term by 2x. Three runs:

| per generated token, summed over 43 blocks | run 1 | run 2 | run 3 |
|---|---|---|---|
| expert read (disk) | 0.818 | 0.815 | 0.811 |
| qkv | 0.043 | 0.042 | 0.043 |
| tail + router compute | 0.212 | 0.218 | 0.212 |
| final compute | 0.209 | 0.214 | 0.208 |
| **inside blocks** | 1.291 | 1.304 | 1.283 |
| **arithmetic** (blocks − disk) | 0.473 | 0.489 | 0.472 |

**The disk term cross-checks against a number measured a different way.** 71.7%
of 3.22 GiB is 2.31 GiB, and 2.31 GiB in 0.815 s is **2.82 GiB/s** — against the
**2.88 GiB/s** that `what-f-is-made-of` measured for the expert read directly.
Two independent routes to the same rate is the strongest evidence in this node.

So a generated token is **63% disk, 37% arithmetic**, where before the cache it
was closer to 60/40 of a *longer* token.

## The re-derived ceiling, and it is not the drive

Within the instrumented run the wall clock was 1.47 s per token and the blocks
summed to 1.29 s, so 0.18 s is outside them — the head, the sampler, the stream
copy between blocks. Take the disk term to zero, which is what a perfect cache
would do:

```
  perfect cache:  0.475 s of block arithmetic + 0.18 s outside = 0.655 s
                                                              = 1.53 tok/s
```

The instrumentation is not free — 43 `eprintln`s a token, and the uninstrumented
median is 1.374 s against this run's 1.47 s, so about 0.10 s of the arithmetic
term is the measuring. Removing it:

```
  perfect cache, uninstrumented:   ~0.555 s  =  ~1.80 tok/s
```

**So caching every expert byte, on this machine, lands somewhere near
1.5–1.8 tok/s.** Not 4.26, and nowhere near 5. The 4.26 figure was never wrong —
it is the *disk* bound with arithmetic set to zero — but it was the wrong bound to
quote as the ceiling, because the arithmetic bound is three times lower.

```
  bound                                        tok/s
  disk, one-bit experts, zero arithmetic        4.26     <- was quoted as the ceiling
  arithmetic, all disk traffic removed          1.5-1.8  <- the real one
  measured today                                0.728
```

**5 tok/s needs 0.2 s per token in total.** The arithmetic alone is 0.48–0.65 s.
So it is not enough to remove the disk: `F` has to fall by roughly **3x** as well.
That is a much sharper statement than "the drive is too slow", and it points
somewhere else entirely.

## Where the arithmetic is, and where this node first got it wrong

> ### CORRECTION, same day
>
> This section originally read *"the routed expert matmuls: **0.004 s, under 1%**
> — the expert arithmetic is free"*, derived by subtracting the expert read
> (0.815 s) from the `ffn` phase (0.822 s). **That subtraction is meaningless.**
> The `ffn` phase covers graph *construction* plus the disk read; the expert
> matmuls are evaluated in the block's final `ctx.compute(&out)`, along with
> everything else. The number is withdrawn.
>
> It is the same mistake `measure-before-proposing` was written about, and it was
> made in the same file that documents it: **a phase timer was read without
> checking what the phase contains.**

What is measured, per generated token:

| | seconds | share of arithmetic |
|---|---|---|
| tail + router compute | 0.212 | 44% |
| final compute | 0.209 | 44% |
| qkv | 0.043 | 9% |

**What is *not* measured is how the 0.478 s divides between the experts,
attention, the LoRA projections and the hyper-connections.** `final compute` is
one ggml graph evaluation and the timer cannot see inside it. Saying otherwise
needs an instrument that does not exist yet.

What can be bounded from outside, with `trunk_mat_vec_dtypes` (added with this
node), which times a `[4096, 2048]` mat-vec against one token:

```
  F32   0.6088 ms    32.00 MiB    55.11 GB/s decoded
  BF16  0.2955 ms    16.00 MiB    56.77 GB/s
  Q8_0  0.2188 ms     8.50 MiB    40.73 GB/s
```

- **The shared expert is not the cost.** It is three such matmuls per block,
  Q8_0, always-read: 3 x 0.219 ms x 43 = **~28 ms a token**, about 6% of the
  arithmetic.
- **Nor is the dtype.** Q8_0 runs at **0.36x F32's time** while carrying a
  quarter of the bytes. `v4flash-repacking-2026-08-10`'s *"there is no x86 Q8_0
  branch"* is about the **repacked** fast path, not a missing base kernel — the
  base kernel is the quickest of the three here. **So C7's "move the trunk to a
  dtype that has a kernel" argument is dead**, and what is left of C7 is only the
  cache-cliff argument below.
- **The routed experts are 18 such matmuls per block** (6 experts x gate/up/down)
  against the shared expert's 3, in Q4_K rather than Q8_0. Scaling by bytes puts
  them somewhere around 100–150 ms a token, which would be 20–30% of the
  arithmetic rather than under 1%. **That is arithmetic on a bench, not a
  measurement of the engine, and it is written here as an estimate.**

All three of these are memory-bound, not compute-bound: 40–57 GB/s decoded
against `chaos-membench`'s 30.8 GiB/s peak, which means the bench keeps the weight
partly in cache across its 100 repetitions and the engine, reading each weight
once per token, will do worse. **A bench that re-reads one weight is not the
engine's access pattern**, and the ratios are what this instrument is for.

**So the next measurement is an instrument that can see inside `final
compute`** — splitting the experts from attention from the hyper-connections.
Until that exists, "88% of the arithmetic is the hyper-connection algebra" is
**not** established, and this node no longer claims it.

## What this does to the plan

- **The trunk requantisation (C7) is still worth doing** and its case is now
  different: it was argued as memory-bus relief, and the better argument is that
  a 3.9 GiB trunk instead of 7.38 GiB moves the expert-cache cliff, so the cache
  can hold more and the disk term shrinks further. The 6 GiB cache that measured
  **0.352 tok/s** did so because 7.38 + 6 = 13.4 GiB of 15.7; with a Q4 trunk the
  same 6 GiB leaves 5.8 GiB free.
- **The disk levers have a visible end.** Perfect caching is 1.5–1.8 tok/s, and
  every disk lever is bounded by that. It is worth about 2.5x from here, no more.
- **`F` is now the interesting half**, and what is inside it is genuinely
  unknown: the phase timers stop at the boundary of one graph evaluation. The
  shared expert is ~6% of it and the dtype is not the problem; the rest is
  unmeasured.

**None of this moves 5 tok/s on this machine**, and it should not be reported as
if it did. What it moves is which measurement to take next.
