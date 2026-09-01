---
topic: the first long-context parity cells, measured — prefill at parity, generation 1.45x behind, and the dense ratio FLIPS with context: 1.09x ahead short, 1.45x behind at 4031 tokens
status: resolved
links:
  - the-v4flash-parity-cell-does-not-reproduce-2026-09-01.md
  - where-we-stand-vs-llamacpp-2026-08-16.md
  - ../backlog/lts-parity-criteria.md
---

# Long-context parity, Qwen3-4B

`lts-parity-criteria.md` section B has **long-context generation untested on all
three models** — three of the eighteen cells, and the largest untested block in
the LTS bar. V4-Flash cannot supply one
(`the-v4flash-parity-cell-does-not-reproduce`), but **Qwen3-4B is dense and fits
in RAM**, so it has no page-cache fight and its numbers repeat.

## The measurement

A fixed 4031-token prompt (`scripts/long-prompt.txt`, deterministic prose so the
run repeats), 32 generated tokens, `-c 8192`, both engines alternating in one
session. Warm-up discarded, as the protocol requires.

```
chaos-run Qwen3-4B-Q4_K_M.gguf -f long-prompt.txt -n 32 --temp 0 -c 8192

llama-completion -m Qwen3-4B-Q4_K_M.gguf -f long-prompt.txt -n 32 -c 8192 --no-warmup
```

llama.cpp commit `daef2b3`.

| pair | Chaos prefill | Chaos gen | llama.cpp prefill | llama.cpp gen |
|---|---:|---:|---:|---:|
| warm-up *(discarded)* | 65.14 | 3.22 | 56.24 | 4.60 |
| 1 | 61.02 | 3.27 | 60.79 | 4.55 |
| 2 | 60.98 | 3.12 | 57.79 | 4.53 |
| 3 | 60.97 | 3.13 | 59.77 | 4.50 |
| **median** | **60.98** | **3.13** | **59.77** | **4.53** |

**Both engines are stable** — Chaos within 2%, llama.cpp within 1%. That is the
contrast with V4-Flash, where the same protocol gave llama.cpp a threefold spread:
a model that fits in memory does not fight the operating system, and the
comparison means something.

## The two cells

- **Long-context prefill: parity.** 60.98 against 59.77, a 1.02x lead that is
  inside the run-to-run spread and should be read as level, not as a lead.
- **Long-context generation: 1.45x behind.** 3.13 against 4.53.

## The finding: the ratio flips with context

The first draft of this node compared the long-context numbers against a *recorded*
short-context row from 2026-08-16 and drew a conclusion from it. That is mixing two
clocks, which this repository has a rule about, so both engines were re-measured
short in the same session — same day, same machine, alternating, warm-up discarded:

```
short context, "Write one sentence about the sea.", -n 64
  pair 1   chaos 8.39   llama.cpp 7.69
  pair 2         8.42             7.72
  pair 3         8.35             7.67
  median         8.39             7.69      Chaos 1.09x AHEAD
```

Put beside the long-context cell, everything measured within one session:

| context | Chaos | llama.cpp | |
|---|---:|---:|---|
| short | **8.39** | 7.69 | Chaos **1.09x ahead** |
| 4031 tokens | 3.13 | **4.53** | Chaos **1.45x behind** |
| slowdown from context | **2.68x** | **1.70x** | |

**The ratio flips.** Chaos is ahead at short context and behind at long, because
its per-token cost grows **2.68x** with 4000 tokens of context where llama.cpp's
grows **1.70x**. That is a concrete weakness with a number on it, and it is a
better description than the parity table's single "1.38x behind" — which is true
of neither end.

**This is the thing to chase on the dense path**, and it is not the FFN. §4a
attributed 61% of the *short-context* gap to the FFN matmul; whatever costs Chaos
an extra 1.6x as context grows is attention or the KV cache, and it is untouched.

**Do not compare the prefill number here with the `38.5 vs 111.2` in the parity
table's short-context row.** That row was measured with a different block size and
prompt and is not the same experiment; nothing in this node says prefill improved.

For the record, the KV cache at this length is **571 MiB, f16, 4062 positions**.

## The sweep: where it diverges, and what that rules out

Two points make a ratio; four make a shape. Same protocol, four context lengths
cut from the same corpus (`scripts/long-prompt.txt`, taking the first 5, 10, 20
and 40 paragraphs), 32 generated tokens each:

| context | Chaos | llama.cpp | llama/chaos |
|---:|---:|---:|---:|
| 500 | **7.18** | 6.79 | 0.95x — Chaos ahead |
| 1001 | 6.16 | 6.34 | 1.03x |
| 2011 | 4.83 | 5.54 | 1.15x |
| 4031 | 3.26 | **4.49** | 1.38x |

**The crossover is near 1000 tokens**, and past it the gap opens smoothly rather
than stepping — so it is not a threshold, a reallocation or a cache falling over.

As milliseconds per generated token, which is where the shape shows:

```
  context     chaos    llama.cpp
    500      139.3       147.3
   1001      162.3       157.7
   2011      207.0       180.5
   4031      306.7       222.7

  slope, per token of context:
    chaos      0.0474 ms        llama.cpp  0.0214 ms       2.2x steeper
```

**Both are linear in context.** Neither is quadratic, so neither is doing
something algorithmically worse than the other — it is a **constant factor of
about 2.2x** in whatever grows with the KV.

### What that rules out

- **Not flash attention.** llama.cpp defaults to it, and `chaos-run`'s own `-fa`
  handler says *"flash attention is the only path here; -fa is the default"*.
  Both have it.
- **Not the KV cache's size or dtype.** Chaos reports **571.2 MiB, f16, 4062
  positions**; Qwen3-4B's 36 layers x 8 KV heads x 128 dims x 2 x 2 bytes x 4062
  is the same 571 MiB. Both engines carry the same bytes.
- **Not the FFN.** §4a's *"61% of the dense gap is the FFN matmul"* is a
  short-context statement, and at 500 tokens Chaos is **ahead**. Whatever this is,
  it is orthogonal to that.

### So the target is named

**Chaos's attention costs about 2.2x more per token of context than llama.cpp's**,
on the dense path, with the same KV bytes and the same kind of kernel. That is one
number, it is reproducible, and it is the first concrete dense-path deficit that is
not the FFN.

**What is not established** is which part of the attention does it — the KV read,
the mask, the scores, or the output projection. `stream.rs` has no phase timer
inside its attention, so answering it needs the same treatment
`CHAOS_ATTN_SPLIT` gave the deepseek4 path. That is the next measurement, and this
node stops short of guessing which one it will be.

## The thread hypothesis, killed before it was built

The cheapest candidate for the 2.2x was the **thread cap**. `chaos-run` holds
generation to 2–4 threads, from the recorded finding that *"generation stops
scaling past 4"* — which was established at **short** context, where a token is a
small matmul. Attention over 4031 positions is a much larger parallel workload, so
a cap tuned on the small case could plausibly be leaving the big one starved.

It is not. Same model, both context lengths, `-t` swept:

| `-t` | 4031 tokens | 500 tokens |
|---:|---:|---:|
| 2 | 2.69 | 6.67 |
| **4** | **3.38** | **7.52** |
| 8 | 3.20 | 6.32 |
| 12 | 3.05 | 5.63 |
| 20 | 2.92 | 4.74 |

**Four threads is optimal at both lengths, and more is worse at both.** The cap is
right, the finding it came from still holds at 4000 tokens, and this hypothesis is
closed. llama.cpp reaches 4.49 where Chaos's best is 3.38, so **1.33x of the gap
survives the best thread count Chaos has.**

That makes five optimisation hypotheses killed by one measurement each in two
days: the I/O headroom, graph construction, the top-k sort, the BF16 gate matmul,
and now the thread cap. The two that *were* real — the tail computed twice and the
cache defaulted to zero — were both found by profiling rather than by guessing.

### A smaller thing the sweep found

**The thread tuner leaves about 4% on the table, at both lengths.** The default
picks 3.26 tok/s at 4031 tokens where `-t 4` gives **3.38**, and 7.18 at 500 where
`-t 4` gives **7.52**. `chaos-run` says *"generation tuned on the first tokens"*, so
it is measuring and then choosing slightly wrong — consistently, in the same
direction, at two very different context lengths. Small, real, and separate from
everything else in this node.
