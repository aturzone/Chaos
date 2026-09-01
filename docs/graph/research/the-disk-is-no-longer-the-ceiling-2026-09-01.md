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

## Where the arithmetic is

Of 0.478 s of arithmetic per token:

| | seconds | share |
|---|---|---|
| tail + router compute | 0.212 | 44% |
| final compute | 0.209 | 44% |
| qkv | 0.043 | 9% |
| the routed expert matmuls | **0.004** | **<1%** |

**The expert arithmetic is free.** `ffn` totals 0.822 s and the read inside it is
0.815 s, so the six expert matmuls per layer cost about four milliseconds a token
in total — which confirms from a third direction the recorded fact that V4-Flash's
routed arithmetic is under 5% of a token, and closes off any idea that begins
"batch the expert matmuls".

**88% of the arithmetic is the block tail and the final compute**: the
hyper-connection algebra (`dsv4_hc_pre`, `dsv4_hc_post`, `dsv4_hc_comb` with its
Sinkhorn iterations), the gate block, attention, and the dense FFN parts. That is
`hc_mult = 4` — four parallel residual streams, all 4096 wide, mixed by a
Sinkhorn-normalised 4x4 at every block, twice.

**That is the next thing to profile, and nobody has opened it.** C5e stopped it
being computed *twice*; nothing has yet asked what one evaluation costs and
whether it has to.

## What this does to the plan

- **The trunk requantisation (C7) is still worth doing** and its case is now
  different: it was argued as memory-bus relief, and the better argument is that
  a 3.9 GiB trunk instead of 7.38 GiB moves the expert-cache cliff, so the cache
  can hold more and the disk term shrinks further. The 6 GiB cache that measured
  **0.352 tok/s** did so because 7.38 + 6 = 13.4 GiB of 15.7; with a Q4 trunk the
  same 6 GiB leaves 5.8 GiB free.
- **The disk levers have a visible end.** Perfect caching is 1.5–1.8 tok/s, and
  every disk lever is bounded by that. It is worth about 2.5x from here, no more.
- **`F` is now the interesting half**, and 88% of it is one subsystem that has
  never been costed.

**None of this moves 5 tok/s on this machine**, and it should not be reported as
if it did. What it moves is which measurement to take next.
