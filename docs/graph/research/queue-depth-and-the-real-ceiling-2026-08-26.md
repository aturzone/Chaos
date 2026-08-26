---
topic: queue depth is worth 2.55x on the disk, and it is not enough — the ceiling is the always-read trunk
status: resolved
links:
  - machine-bandwidth-2026-08-25.md
  - v4flash-ram-frontier-2026-08-16.md
  - ../backlog/the-big-bang-5-tok-s.md
---

# Rung 1 works. The ladder still does not reach 5.

**Phase 0 of `the-big-bang-5-tok-s.md` existed to answer two cheap questions
before any engine code was written for them.** The first is answered, the answer
is good, and it exposed an error in the plan's own arithmetic that would have
cost far more than the day it took.

## Queue depth: 2.55x, exactly as suspected

`chaos-qdbench`, reading 13 MiB blocks (one expert's size) from a 16.4 GiB file
on the target machine — larger than its 15.7 GiB of RAM, so the page cache
cannot answer:

```
 depth         GiB/s  vs depth 1
     1          1.34       1.00x
     2          2.07       1.54x
     4          3.20       2.39x
     8          3.41       2.55x     <- peak
    16          3.36       2.51x
```

**Depth 1 reproduces what Chaos gets today.** `machine-bandwidth-2026-08-25`
measured Chaos reading V4-Flash's experts at **1.40 GiB/s**; a single-request
benchmark gives **1.34**. That is the diagnosis confirmed rather than inferred:
the engine reads experts one at a time, and the drive is idle between requests.

**Four concurrent reads get 2.39x, eight get 2.55x, sixteen give nothing more.**

## And it is not enough, because the plan forgot the arithmetic

The ladder in `the-big-bang-5-tok-s.md` modelled a token as
`bytes / bandwidth`. **That is wrong, and it is my error**: a token is

```
t  =  bytes / bandwidth  +  F
```

where `F = 0.84 s` is the work that never touches the disk, measured on
2026-08-16 and recorded in `v4flash-ram-frontier`. Putting it back:

| rung | disk | + compute | tok/s |
|---|---|---|---|
| 0 today, serial reads | 2.40 s | 0.84 s | **0.31** |
| 1 + queue depth 8 | 0.94 s | 0.84 s | **0.56** |
| 2 + 2-bit experts | 0.47 s | 0.84 s | **0.76** |
| 3 + top-3 routing | 0.24 s | 0.84 s | **0.93** |
| 3 + 29% resident | 0.15 s | 0.84 s | **0.99** |

**Rung 1 is worth 1.82x, not the 2.55x the disk improves by**, because the
0.84 s does not shrink with it. And every rung after it is fighting for a
smaller share of a token that is already mostly arithmetic.

## The ceiling, and it is lower than 5

V4-Flash has **7.38 GiB of always-read weights** and 137.06 GiB of routed
experts. The always-read part is read *every token*, from RAM, and this machine
reads RAM at **30.8 GiB/s**.

```
                                            per token    ceiling
  trunk alone                                7.38 GiB    4.17 tok/s
  + experts as they are                     10.60 GiB    2.91 tok/s
  + experts shrunk 4x                        8.19 GiB    3.76 tok/s
```

**Those numbers give the disk away for free and charge nothing for
arithmetic.** They are what the memory bus alone permits.

```
   5 tok/s needs  36.9 GiB/s for the TRUNK alone  (1.2x this machine)
  20 tok/s needs 147.6 GiB/s for the TRUNK alone  (4.8x this machine)
```

**So 5 tok/s is out of reach on this laptop even if every expert were free and
the arithmetic took no time.** The always-read weights do not fit the budget.
This is the same wall as 20 tok/s, one factor of four closer.

## What is actually achievable here, and it is worth doing

**0.43 → about 1.0 tok/s**, which is 2.3x, from work that is real:

1. **Queue depth** — 2.55x on the disk, 1.82x on the token. No model change, no
   quality risk, and the benchmark to prove it exists.
2. **A smaller expert format** — helps the streamed portion only, so it is worth
   about 1.35x on top of rung 1 rather than the 2x its size suggests.
3. **Reclaiming RAM** — the browsers hold 9.28 GiB on this machine and Chaos
   gets 1.74 GiB for residency.

**Below 1.19 tok/s the disk is the problem and these fix it. Above it, the
machine is the problem.** `v4flash-ram-frontier` said the ceiling with infinite
RAM was 1.19; this says the same number from the other direction, which is the
first time two independent measurements have agreed on it.

## What would reach 5

A memory bus of **≥37 GiB/s with 144 GB of capacity**. An Apple M-series Ultra
(≈800 GB/s unified, 192 GB) clears it by twenty times; a many-channel
EPYC/Threadripper clears it; a multi-GPU box clears it. **This laptop does
not**, and no amount of engineering moves 7.38 GiB per token through a 30.8
GiB/s bus five times a second.

## What this cannot say

- **`F` was measured once, on 2026-08-16.** It is the weakest number here. If
  the 0.84 s is itself improvable — and nobody has profiled what is inside it —
  every row above moves. **That is the one measurement left worth taking**, and
  it is now the first item of Phase 0 rather than the last.
- The queue-depth figure is one drive, one file, one session. Another machine
  has its own curve, and `chaos-qdbench` is how to find it.
