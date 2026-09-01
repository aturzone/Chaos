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
