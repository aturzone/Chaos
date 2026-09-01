---
topic: F split at last — attention 40%, the expert matmuls 40%, and the hyper-connection algebra 8%. F is work, not waste, which is why 5 tok/s is out of reach here for a hard reason.
status: resolved
links:
  - the-disk-is-no-longer-the-ceiling-2026-09-01.md
  - what-f-is-made-of-2026-08-31.md
  - ../backlog/the-big-bang-5-tok-s.md
  - ../reference/hard-won-facts.md
---

# What is inside the final compute

`the-disk-is-no-longer-the-ceiling` established that a V4-Flash generated token is
**0.815 s of disk and 0.478 s of arithmetic**, and that the arithmetic is now the
binding constraint. It could not say what the arithmetic *is*: the block builds one
ggml graph and evaluates it once, so no phase timer sees inside. That node guessed
"88% is the hyper-connection algebra" and **retracted it the same day**, along with a
claim that the expert matmuls cost 0.004 s.

Two probes now answer it, and both guesses were wrong in opposite directions.

## The instruments

`CHAOS_FFN_SPLIT` and `CHAOS_ATTN_SPLIT`, off by default, each computing one
subgraph on its own and then **freezing it into a leaf** so the next evaluation does
not redo it. Freezing is what makes the split clean — without it the final compute
walks back through `ffn_out` and does the whole FFN twice, which is the bug C5e
fixed one layer up.

Both are behind their own variables rather than `CHAOS_BLOCK_TIMING`, because three
graph evaluations are not one: they change the thing they measure. **This time they
barely do**, which is the reason to trust the result — see below.

## The split

Two consecutive runs, generation passes only, prefill excluded:

| per generated token | run 1 | run 2 | share |
|---|---|---|---|
| attention arithmetic | 0.198 | 0.195 | **40%** |
| expert matmuls (6 routed + 1 shared, gate/up/down) | 0.199 | 0.198 | **40%** |
| hyper-connections + router | 0.040 | 0.040 | 8% |
| qkv construction | 0.043 | 0.042 | 9% |
| residual write-back (`dsv4_hc_post`) | 0.003 | 0.003 | 0.6% |
| **arithmetic total** | **0.494** | **0.484** | |

**The cross-check is what makes this believable.** The unsplit measurement of the
same quantity, taken with neither probe, was **0.473 / 0.489 / 0.472 s**. Splitting
the graph three ways changed the total by under 3%, so the shares are not an artefact
of the instrument — which is exactly what could not be said of the first attempt at
this (below).

## Both earlier guesses were wrong, in opposite directions

| claimed | measured |
|---|---|
| the hyper-connection algebra is **88%** of `F` | **8%** — the whole of it, router included |
| the routed expert matmuls are **under 1%** | **40%** |

The first came from reading `tail` (0.212 s) as hyper-connection work. It is not:
**`attn_out` feeds `layer_tail`'s `dsv4_hc_post`, which feeds `ffn_norm`, which the
router's `compute` needs** — so the entire attention graph is evaluated inside the
phase the block line calls `tail`. The `attn` column reads 0.000 s and means only
that building the graph is free.

The second came from subtracting the expert read from the `ffn` phase, which covers
construction plus the read and not the arithmetic at all.

**Both are the same error**: attributing a phase timer's number to a subsystem
without reading what falls between the two `Instant`s. It is in
`reference/hard-won-facts.md` now, twice over.

## And one outlier, discarded on the record

The first run of the two-probe build measured **attention at 0.394 s** and the
arithmetic total at 0.753 s — nearly double. It was the first run after a `cargo
build`, with a cold page cache, which is the third time that pattern has appeared in
two days (the other two are in `the-expert-cache-was-off`). It is discarded, and it
is written down rather than dropped so that nobody re-derives a 52% attention share
from it.

## What it means for 5 tok/s

**`F` is work, not waste.** The two 40% halves are both doing arithmetic they have
to do:

- **Attention**, at 0.197 s over 43 blocks, is 4.6 ms a block. The shape: 64 heads,
  a 128-position sliding window, K and V reconstructed from a 512-rank latent, and a
  1024-rank output pair. That is roughly 22 M multiply-accumulates a block, and at
  the ~14 GFLOP/s this machine reaches on a mat-vec — memory-bound, not
  compute-bound — 22 M lands near **3.1 ms**. Measured 4.6 ms. There is no factor
  of ten hiding in it.
- **The expert matmuls** are 18 `[4096, 2048]` products a block for the six routed
  experts plus 3 more for the always-read shared one. `trunk_mat_vec_dtypes` times
  one such product at **0.219 ms in Q8_0**; 21 of them is 4.6 ms a block, which is
  what is measured. There is no factor of ten hiding here either.

So the arithmetic is not a missing kernel, a duplicated computation or a wrong
dtype — the three things that *were* wrong this week and are now fixed. It is the
model's own arithmetic, running at about the rate this memory system allows.

```
  measured today                                0.728 tok/s
  all disk traffic removed (perfect cache)      1.5-1.8
  5 tok/s needs                                 0.200 s per token
  the arithmetic alone is                       0.478-0.494 s
```

**5 tok/s on this machine would need the arithmetic to run 2.4x faster than the
memory system allows, with the disk switched off entirely.** That is a hardware
statement, and it is the same conclusion the RAM-frontier sweep reached from the
other direction — but this is the first time it rests on a measured decomposition of
`F` rather than on `F` as one opaque number.

## What is left worth taking

- **The expert cache**, which is the only lever with room: 28.3% hits today, and
  perfect caching is worth 2.5x from here. C7 (a Q4 trunk) matters because it moves
  the cache cliff, not because Q8_0 is slow — it is not, it is the fastest of the
  three dtypes measured.
- **Fewer experts per token** would cut both halves at once — the disk *and* the
  40% of arithmetic that is expert matmuls. That is a quality question, not a speed
  one, and it belongs behind the gate's *lossy* bar.
- **Nothing in the hyper-connections.** 8% including the router, and the residual
  write-back is 0.6%. Any plan that starts there is chasing 40 ms.
