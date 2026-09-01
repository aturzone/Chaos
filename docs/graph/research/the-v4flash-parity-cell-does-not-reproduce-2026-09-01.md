---
topic: llama.cpp's V4-Flash generation figure ranges 0.16–0.47 tok/s on this machine and drifts downward within any sequence, so the parity cell cannot be measured here — no ratio is published
status: resolved, and it closes a cell by declaring it unmeasurable rather than by winning it
links:
  - where-we-stand-vs-llamacpp-2026-08-16.md
  - ../backlog/lts-parity-criteria.md
  - ../backlog/bigger-machine-prompt.md
  - ../reference/hard-won-facts.md
---

# The V4-Flash parity cell does not reproduce

`lts-parity-criteria.md` section B is the LTS bar, and its V4-Flash generation
cell reads **0.37 vs 0.39 — parity**. Chaos now measures **0.696** on that cell,
so it looked like the cheapest cell in the table to close, and closing it would
have been the project's first genuine measured lead.

**It cannot be closed here, and the reason is llama.cpp's number, not ours.**

## What Chaos measures

Three alternating pairs, `-n 8`, the protocol recorded in
`where-we-stand-vs-llamacpp-2026-08-16`:

```
  Chaos, defaults (3.34 GiB expert cache)   0.686  0.696  0.699    median 0.696
  Chaos, --cache 0                          0.592  0.602  0.601    median 0.601
```

**Stable to about 1%.** Prefill likewise: 0.70, 0.70, 0.72 tok/s.

## What llama.cpp measures

Eight observations tonight, same command line every time —
`llama-completion -m <model> --no-repack -c 512 -n 8 -p "..." --no-warmup`,
commit `daef2b3`:

| context | tok/s |
|---|---|
| clean start, nothing else run | **0.41** |
| warm-up of the alternating run | 0.37 |
| alternating pair 1 | 0.47 |
| alternating pair 2 | 0.30 |
| alternating pair 3 | 0.30 |
| three runs back to back, **no Chaos in between** | **0.43 → 0.19 → 0.16** |

**0.16 to 0.47 is a factor of three**, and the last row is the one that settles
it: with Chaos never started, and Windows reporting free memory *rising* run over
run (11.52 → 11.88 → 12.13 GiB), llama.cpp still fell from 0.43 to 0.16. It
degrades against itself.

So the drift is not contention with Chaos, and it is not visible in free memory.
The plausible cause is the standby page list filling with pages of a 144 GB file
on a 15.7 GiB machine, but **that is a hypothesis and this node does not test
it.** What is established is only that the number does not reproduce.

## Why no ratio is published

Chaos 0.696 against llama.cpp's **best** is 1.70x; against its **median of the
alternating pairs** it is 2.32x; against its **worst** it is 4.35x. Picking any
one of those is picking the answer.

`CONTRIBUTING.md`'s rule is that a competitive claim needs both command lines and
both outputs, from repeats alternating in one session. **That rule is satisfied
here and it is still not enough**, which is worth knowing: the protocol assumes
each engine has a reproducible number to alternate *with*. On this model, on this
machine, llama.cpp does not.

**So the cell stays open, and the reason it stays open is written down.** The
previously recorded `0.394 vs 0.39` is not withdrawn — it was measured under the
same protocol and is as good as any single sample — but it should no longer be
read as *the* llama.cpp figure, because tonight the same command produced 0.16
and 0.47 on the same machine.

## And the earlier stall, which is the same phenomenon at its limit

Twice tonight llama.cpp entered a state where it made **no progress at all**: 85
CPU-seconds accumulated in 21 minutes of wall clock, and **0.0 seconds of CPU
across a 20-second sample**, with a 10.66 GiB working set and 0.23 GiB free. It
was not slow; it was entirely blocked on page faults. Killing it and starting
again from a quiet machine gave 28 seconds and 0.41 tok/s.

Both times it happened immediately after a Chaos run. That is the same
memory-pressure story at its extreme, and it is why any future attempt at this
cell needs a settling gate — and why the gate has to *work*, which the first one
did not: it used `bc`, which Git Bash does not ship, and fell back to "do not
wait". **A guard whose dependency is missing must fail closed.**

## What would actually close this cell

- **A machine where the model's working set does not fight the operating
  system.** `backlog/bigger-machine-prompt.md` already asks for `F` on a larger
  CPU; this adds a second reason to want one. The 5090 box's 64 GiB would put
  llama.cpp's mmap in a completely different regime.
- Or a protocol that controls whatever is drifting — which first requires finding
  out what it is. That is a Windows memory-manager question, not an inference
  question, and nothing in this repository can answer it today.

**Until then this cell is `unmeasurable here`, which is a different state from
`untested` and from `parity`, and the table should say so.**

## What is safe to say

- **Chaos generates V4-Flash at 0.696 tok/s** on this machine, three runs, ±1%,
  with the command line above. That number stands on its own and needs no
  competitor.
- **Chaos's figure is reproducible and llama.cpp's is not**, on this model, on
  this machine. That is an observation about two engines' relationship with the
  operating system — Chaos owns its residency and bypasses the page cache, and on
  a model nine times the size of RAM that turns out to matter for *predictability*
  as well as for speed. **One model, one machine, and stated as such.**
