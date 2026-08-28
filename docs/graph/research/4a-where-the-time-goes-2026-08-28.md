---
topic: §4a — where the time and the memory actually go, measured
status: resolved — four areas measured, one new instrument, one number that moves a claim
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - ../reference/hard-won-facts.md
  - where-we-stand-vs-llamacpp-2026-08-16.md
---

# §4a: where the time goes

§4a names four pieces of untouched ground: **the dense prefill path, the
tokenizer, model load time, and the GUI's worst blocking call.** All four are
measured here. Two turn out not to matter and can be closed; one gives a number
that is worse than the standing claim; one was already fine.

**Machine and session.** This laptop, 15.7 GiB total with 7.0–7.3 GiB available,
20 threads, RTX 3050 6 GiB (unused — CPU only). 0 orphan processes checked before
starting, per the standing rule that `Get-Process` comes before trusting a
surprising number. Everything below was run in one session.

## 1. Model load time — not a bottleneck, and it saturates at four threads

`chaos-loadbench`, which bypasses the page cache so repeats re-read from the
device rather than replaying RAM.

| model | always-read set | 1 thread | 4 threads | 16 threads | best |
|---|---|---|---|---|---|
| Llama-3.2-1B Q4_K_M (770 MiB) | 0.8 GB | 0.33 s / 2.41 GB/s | 0.26 s / 3.12 GB/s | 0.26 s / 3.12 GB/s | **1.30x** |
| Qwen3-30B-A3B Q4_K_M (17.28 GiB) | ~1.0 GB | 0.59 s / 1.69 GB/s | 0.38 s / 2.61 GB/s | 0.39 s / 2.57 GB/s | **1.55x** |

Three repeats of the 1B: 1 thread 0.33/0.33/0.33 s, 4 threads 0.25/0.26/0.26 s.
**About 3% spread**, so these are stable numbers, not lucky ones.

**Findings.** Load parallelism is done at four threads — 8, 12 and 16 are all
within noise of 4, and slightly *worse*. The whole always-read set of a 17 GiB
model arrives in **0.38 s**, which is not a figure worth optimising against a
token that costs seconds. **Close this**: load time is not where the time goes.

## 2. The tokenizer — closed with a number, and it is 0.036%

Nothing in the workspace timed it, so **`chaos-tokbench` is new** (in
`core/tokenizer`, beside the crate it measures, reading only a container's
header so a 144 GB model costs the same as a small one).

Llama-3.2-1B, vocab 128,256, byte-level BPE. 42,800 bytes of deliberately mixed
input — English prose, Rust code, Persian, Japanese, Russian, hex and JSON,
because a tokenizer measured on English prose alone is measured on its best case.

| | median | throughput |
|---|---|---|
| encode | 16.5 ms | 2.5 MB/s, **850,000 tokens/s** |
| decode | 0.60 ms | 68 MB/s, 23,000,000 tokens/s |

14,001 tokens for 42,800 bytes (3.06 bytes/token). Spread 9.6% encode, 16.6%
decode over 7 rounds. **Round trip exact.**

**The number that closes it:** a 642-token prompt costs **0.76 ms** to tokenize,
against about **2,100 ms** to prefill the same prompt. That is **0.036% of
prefill** — three and a half orders of magnitude away from mattering. **Close
this too.**

### The round-trip check was wrong before it was right

The first run reported `round trip DIFFERS: 0 of 35800 chars matched` — total
corruption, apparently, of a byte-level BPE. It was the check. **`encode`
prepends BOS when the container asks for it** ("The quick brown fox" is *five*
tokens, not four), so `decode(encode(text))` begins with `<|begin_of_text|>` and
diverges at character zero. Comparing the wrong two strings is not a finding, and
this one was caught before it was written down anywhere but here.

## 3. The dense path against llama.cpp — 1.30x behind on *both* phases

The standing claim is *"behind 1.20-1.27x on the dense path when both sides are
hand-tuned"*. This is a new model and a new measurement, and it lands slightly
worse than that on prefill and adds a generation figure.

**Both command lines, as the citation rule requires:**

```
chaos:      ./target/release/chaos-run.exe $M "<642-token prompt>" -n 32 -tb 20 -t 4
llama.cpp:  llama-bench -m $M -p 642 -n 32 -t {20,4} -r 3     build daef2b3
model:      Llama-3.2-1B-Instruct-Q4_K_M.gguf, 762.81 MiB, 1.24 B params, CPU
```

Alternating, three rounds, one session:

| round | chaos pp | chaos tg | llama pp t=20 | llama tg t=20 | llama pp t=4 | llama tg t=4 |
|---|---|---|---|---|---|---|
| 1 | 313.27 | 20.80 | 384.83 | 14.78 | 197.29 | 27.29 |
| 2 | 294.29 | 21.17 | 382.26 | 14.75 | 194.63 | 26.75 |
| 3 | 288.39 | 19.82 | 301.74 | 14.69 | 200.01 | 27.14 |
| **median** | **294.29** | **20.80** | **382.26** | 14.75 | 197.29 | **27.14** |

**Hand-tuned against hand-tuned** — each engine at its best thread count for each
phase:

- **Prefill: 382.26 against 294.29 → llama.cpp is 1.30x ahead.**
- **Generation: 27.14 against 20.80 → llama.cpp is 1.30x ahead.**

**Do not read this as replacing the standing 1.20-1.27x figure.** It is a
different model and a different prompt length, measured against a different
llama.cpp build, and llama.cpp's round-3 prefill (301.74 against 382/385) is a
wide enough outlier that its own spread is ~27%. What it does say is that on this
model the dense gap is **not smaller** than the standing claim, and that it is
about the same size in both phases.

### Where that 1.30x lives: the FFN, and nothing else

`chaos-run`'s own breakdown for the same run (642 prefill + 32 generated):

```
time: 0.0s disk, 0.4s qkv, 0.6s attention, 1.9s ffn,
      0.0s expert compute, 0.0s slice copies, 0.0s kv build, 0.2s other
```

**The FFN is 1.9 s of about 3.1 s of accounted compute — 61%.** Disk is 0.0 s: a
762 MiB model is resident and there is nothing to stream. So the dense gap is a
**matmul** gap, not a plumbing gap, and anything that does not make the FFN
faster cannot close it. That is a useful thing to know before anyone proposes
threading, batching or cache work on the dense path.

### The bigger lever is the thread count, not the engine

**llama.cpp's own generation is 1.84x faster at `-t 4` than at `-t 20`** (27.14
against 14.75), measured here on a third model. `hard-won-facts.md` already says
generation wants 2-4 threads and prefill wants all of them; this is that trap
costing more than the entire engine difference. Chaos picks 4 for generation on
its own, which is right.

## 4. The GUI's worst blocking call — already fine

`scripts/run-through.ps1`, every control on every page including CHAOS:
**worst blocking call 74.1 ms**, against the 200 ms threshold at which a window
looks frozen. The page transitions are the expensive ones (CHAT 8.0, MODELS 60.4,
SETTINGS 61.8, IMAGE 74.1, CHAOS 47.6 ms); every individual control is under
30 ms. Nothing to do here.

## What §4a changes

- **Two areas closed as immaterial**, with numbers rather than assurances: load
  time (0.38 s for a 17 GiB model) and tokenization (0.036% of prefill).
- **One new instrument**, `chaos-tokbench`, so the tokenizer stays measurable.
- **The dense gap is localised to the FFN** (61% of compute), which is where any
  future dense work has to go.
- **A new standing number to quote for the dense path on Llama-3.2-1B**: 1.30x
  behind on prefill and 1.30x behind on generation, both hand-tuned, with both
  command lines above.

## Found while measuring, and it belongs to §4b

**`qwen3moe` is not in `VERIFIED_ARCHITECTURES`.** `chaos-run` refuses
Qwen3-30B-A3B without `--force`, listing the fourteen it does trust. That is in
tension with the headline in `CLAUDE.md` and `README.md` — *"**Proven**:
Qwen3-30B-A3B (17.28 GiB) generates correct text on a 15.7 GiB machine"* — and
`ram-frontier-qwen3-30b-2026-08-12.md` already records that the model was
**delisted the same day** for a stable-reference divergence. Carried into §4b,
which is the item for exactly this.
