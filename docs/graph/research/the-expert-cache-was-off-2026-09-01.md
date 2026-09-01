---
topic: the expert cache defaulted to zero on V4-Flash and that cost 1.20x — and `--auto` turned out never to run on that model at all
status: resolved
links:
  - the-tail-computed-once-2026-08-31.md
  - v4flash-ram-frontier-2026-08-16.md
  - ../reference/hard-won-facts.md
---

# The expert cache was off, and `--auto` never ran

After C5e a V4-Flash token was 1.675 s and the header said this, unprompted:

```
cache      off. The always-read set fits, so --cache <GiB> is now
cache      worth measuring: a cached step reads 6 experts per layer,
cache      not the ~123 a long prefill does.
```

The engine was asking to be measured. It was measured, and it was right — and
what the measurement found underneath was worse than a missing default.

## The curve

Sixteen generated tokens, one session, nothing else running. 15,198 expert reads
per run, so the hit rates are over real traffic rather than a two-token warm-up:

```
  cache      tok/s    hits
  off        0.603     --
  1 GiB      0.649    14.2%
  2 GiB      0.695    21.3%
  3 GiB      0.721    26.8%    <- peak
  4 GiB      0.620
  5 GiB      0.505    33.1%
  6 GiB      0.352
```

**The hit rate climbs all the way while the speed collapses after 3 GiB.** That is
the whole shape of it: a cached byte is a byte the OS cannot use, and past the peak
the memory pressure costs more than the hits pay for. At 5 GiB the cache is getting
*a third* of its reads for free and is still **1.19x slower than having no cache at
all**.

Confirmed at the peak, three alternating pairs:

```
  pair 1:  off 0.597   cache 3 GiB 0.721
  pair 2:  off 0.604               0.704
  pair 3:  off 0.603               0.723
           median 0.603            median 0.721      1.196x
```

## Two things wrong, and the second is the bad one

**1. The default was zero.** `expert_cache_budget` came through as `None` and the
match arm read `None => 0`. There was no sizing at all: unless a human typed
`--cache N`, a 144 GB model streamed every expert from disk on every token with
3 GiB of usable RAM sitting idle.

**2. `--auto` never runs on this model.** The obvious reply to (1) is "that is what
`--auto` is for". It is not, here. `auto_plan` takes a `Qwen3Config`, and the
deepseek4 dispatch returns from `main` *before* that config is ever built. Side by
side:

```
$ chaos-run qwen3-4b "hi" --auto
threads    -t 4 to generate, -tb 20 to prefill (20 cores: ...)
batch      -b 2048 tokens per prefill block, from 11.4 GiB free (...)
io         buffered -- the model fits in memory, so the page cache is worth having
device     none: no discrete GPU, so everything runs on the CPU

$ chaos-run v4flash "hi" --auto
cache      off. The always-read set fits, so --cache <GiB> is now
```

**Nothing. Not one decision.** `--auto` is silently a no-op on DeepSeek-V4-Flash,
and `CLAUDE.md` ticks R6 as done: *"`--auto` picks device, cache, threads, prefill
block and I/O mode, and predicts tok/s"*. It does that for the dense path. On the
flagship model — the one the whole project is built around, the one where
self-configuration is worth the most — it does none of it and says nothing.

That is worse than the missing default, because the missing default is a number
and this is a claim.

## And the hint was pointing at a cliff

The old message invited a guess with no bound: *"`--cache <GiB>` is now worth
measuring."* The natural guesses are round numbers, and on this machine 5 GiB is
**1.19x slower** than off and 6 GiB is **1.71x slower**. The engine suggested an
experiment whose obvious settings lose.

Both are replaced. A size is chosen for the user, and if there is genuinely no room
the message says which resource ran out instead of hinting.

## The rule, and exactly what it rests on

```
cache = clamp(total_ram - resident_bytes - 5 GiB, 0, 6 GiB)
```

**Total RAM, not free RAM**, and that is the part worth arguing. Free RAM is read
before the trunk loads and drifts with whatever else the machine is doing — sizing
a long-lived allocation from it is how the dense path's planner arrives at
`avail / 2`, which here would be **4.9 GiB**, a value this curve measures at 0.505,
*worse than off*. Total RAM is a property of the machine.

On this laptop the rule picks **15.7 - 7.38 - 5 = 3.34 GiB**, and measures:

```
  pass 1:  off 0.602   cache 3 GiB 0.725   default 0.633
  pass 2:  off 0.599               0.727           0.733
  pass 3:  off 0.602               0.729           0.724
           median 0.602            0.727           0.724
```

**The two low readings are both first-runs-after-a-build**, a cold page cache, which
is the trap this repository already records for the Vulkan shader cache. Discard
neither — they are in the table — but the median is the number.

**This is one machine's curve and the constant is fitted to it.** The *shape* —
rise, peak, collapse — is a memory-pressure argument that should hold anywhere. The
*position* of the peak is this laptop's. `RESERVE = 5 GiB` covers the OS, the KV
cache, the per-block arenas and enough page cache for the streaming reads not to
starve, and it has been checked on exactly one machine. On a bigger machine the rule
gives more cache, which is the right direction and an unmeasured amount.

## What it adds up to

```
  before tonight, no cache          0.509 tok/s
  C5e, tail computed once           0.570        1.12x
  + expert cache defaulted          0.727        1.28x
                                                 1.43x together
```

Both are **exact**: C5e copies a computed value, and the cache returns the same
bytes the disk would have. Both went through the gate's *exact* bar on the same
recorded baseline.

**The 5 tok/s ceiling has not moved.** 137 GiB of experts still cross a
3.41 GiB/s drive, and `the-big-bang-5-tok-s.md` still caps this machine at 4.26
tok/s with one-bit experts and a free trunk. What has changed is that two of the
things standing between the engine and its own ceiling were an accident and a
`None => 0`.

## What is now open, and it is the interesting one

**`--auto` should run on every architecture, not the dense one.** The fix here gives
the deepseek4 path a cache; it does not give it threads, prefill block, I/O mode or
a device. Those are all decided for Qwen3 and silently skipped for the model where
the arithmetic is hardest. That is v0.0.28's T4, and it now has a measured reason
rather than a checkbox.
