---
topic: the two GPU measurements that disagreed 3x are both right — the device path's advantage is context-dependent
status: resolved
links:
  - ngl-ladder-2026-08-24.md
  - ngl-frontier-2026-08-16.md
  - ../backlog/the-big-bang-5-tok-s.md
  - ../reference/hard-won-facts.md
---

# The GPU wins on short prompts and loses on long ones

Two nodes in this repository measured `-ngl 99` on **the same model, the same
card, the same laptop** and disagreed by more than 3x:

| node | date | generation, `-ngl 99` vs CPU |
|---|---|---|
| `ngl-frontier-2026-08-16` | 2026-08-16 | 8.85 against 6.34 — **1.40x faster** |
| `ngl-ladder-2026-08-24` | 2026-08-24 | 2.92 against 6.39 — **0.46x, i.e. 2.2x slower** |

Their **CPU baselines agree to within 1%** (6.34 and 6.39), which rules out the
easy explanations — a different build, a different model file, a different
machine. Something about the *device* runs changed.

`STATUS.md` carried the second as settled (*"Vulkan generation is 2.2x slower
than CPU and nobody has looked at why"*), and `the-big-bang-5-tok-s.md` closed
the GPU lever on the strength of it — while `ngl-ladder` itself says, in the same
breath, **"do not quote this as 'the GPU is slower at generation'"**.

## Neither is wrong. They measured at different context lengths.

The answer is in `ngl-ladder`'s own command line, and it is the one thing the two
nodes do not have in common:

```
ngl-ladder   chaos-run Qwen3-4B -f <1080-token prompt> -n 32 --device 1 -ngl <N>
ngl-frontier chaos-run -n 16 --temp 0                          (a short prompt)
```

Re-measured 2026-08-31, one session, three runs per point, medians, warm-up
discarded:

| prompt | tokens | CPU (`-ngl 0`) | RTX 3050 (`-ngl 99`) | ratio |
|---|---|---|---|---|
| *"The capital of France is"* | 16 | **4.74** | **7.24 – 9.61** | **1.5–2.0x faster** |
| ~1050 tokens of prose | 32 | **~4.2** | **~3.92** | **0.93x — slower** |

**Both directions reproduce.** The device path is ahead on a short prompt and
behind on a long one, and `ngl-ladder` had already seen the mechanism without
connecting it to the other node: *"the device path degrading faster than the CPU
as the context grows — 6.39 → 5.39 on the CPU — but the device path falls
further."*

So the pair only reads as a contradiction because **neither node states the
other's condition prominently**. A reader comparing the headline numbers
compares two different experiments.

The magnitudes do not match exactly — 0.93x here against their 0.46x — and that
is not claimed to be reconciled. This machine had 6.2 GiB free with a browser
holding 1.78 GiB, and `Only compare within one session` is a rule in this
repository for exactly that reason. **What reproduces is the direction and the
mechanism, which is what the contradiction was about.**

## What this changes

- **The GPU lever is not closed, it is conditional.** The standing sentence
  "Vulkan generation is 2.2x slower than CPU" describes a long-context run and
  should never be quoted without that clause. On short prompts, on this card,
  the device path is the faster one.
- **It looked like the explanation for `--auto` picking wrong at long `-n`** —
  recorded separately as **2.14x slower at `-n 200`** and never explained. The
  reasoning was clean: if `--auto` chooses the device from short-context
  behaviour and the run is then long, it has chosen the wrong side of this
  crossover.

  **Tested, and it did not reproduce.** With a short prompt, one session:

  | `-n` | `--auto` | `-ngl 0` | |
  |---|---|---|---|
  | 16 | **7.21** | 3.37 | auto 2.1x ahead |
  | 200 | **5.42** | 4.77 | auto 1.14x ahead |

  `--auto` picks device 1 at both lengths and is ahead at both. Its margin does
  shrink with `-n` — 2.1x to 1.14x, the same direction as the crossover — but it
  does not cross. **So the recorded 2.14x slower is not explained by this**, and
  it may be about long *context* rather than long *generation*: this test varied
  `-n` with a five-token prompt, and `ngl-ladder` varied `-n` behind a
  1080-token one. Those are different experiments and only the second is the one
  the original figure came from.

  Recorded as a **failed** hypothesis rather than deleted, because the next
  person will have the same idea and should know it was tried. The variance here
  is also too wide to settle it either way: `-ngl 0` at `-n 16` gave 3.37 in this
  set and 4.74 in the one above.
- **`-ngl` is a context-dependent decision, not a machine-dependent one.** Any
  advisor that answers "should this go on the card" from hardware alone is
  answering half the question.

## What it does not change

**Nothing about V4-Flash.** That model's 7.38 GiB trunk does not fit in 5.11 GiB
of usable VRAM, and its experts stream from disk whatever `-ngl` says. The
separate measurement that the GPU is **4.3x slower on streaming MoE**
(`gpu-does-not-help-streaming-moe-2026-08-16`) is about a different mechanism —
76% of that token is disk — and is untouched by this.

## Two traps worth recording

**There are two Vulkan devices on this laptop**, and the first one is the Intel
integrated GPU:

```
0  Vulkan0  7.21G  Intel(R) RaptorLake-S Mobile Graphics Controller (IGpu)
1  Vulkan1  5.11G  NVIDIA GeForce RTX 3050 6GB Laptop GPU (Gpu)
```

Measured at `-ngl 99` on the short prompt: **device 0 gives 1.28 tok/s against
device 1's 7.24** — 0.27x of the CPU, against 1.53x. So a run that lands on the
integrated GPU by accident produces a number in the same range as the one this
node set out to explain. **It was not the explanation here** — `ngl-ladder`
passed `--device 1` — but it is a trap that would produce a very convincing wrong
answer, and `chaos-run` with no `--device` currently picks device 1 rather than
device 0, which is the right default and worth not breaking.

**The spread on a loaded machine is wide.** Three runs of the identical short-
prompt configuration gave 6.87, 7.80 and 7.24 tok/s — a 13% spread — and an
earlier set gave 8.99–9.68. A single run of either would support a different
story, which is what `ngl-frontier` warns about in its own postscript: *"the
single-run version of this table told a different story."*
