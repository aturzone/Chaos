---
topic: what this machine's memory actually delivers, and the law it implies for tok/s
status: resolved
links:
  - v4flash-ram-frontier-2026-08-16.md
  - v4flash-has-no-slack-2026-08-10.md
  - ngl-ladder-2026-08-24.md
  - ../backlog/the-big-bang-5-tok-s.md
---

# 30.8 GiB/s, and `tok/s ≈ 19 / GiB`

**Question**: the project has repeatedly said 20 tok/s on V4-Flash is out of
reach on this machine, on the strength of a *fixed cost per token*. That is a
measurement of Chaos, not of the machine. **Is the wall the hardware or the
implementation?**

**Answer**: the hardware, with a little room. This laptop reads **30.8 GiB/s**
from RAM at its best. Chaos generates at **~19–20 GiB/s**, which is **65% of
peak** — good for a dequantising matmul, and nowhere near the 40× that 20 tok/s
would need to find.

## The machine

`scripts/bench-memory-bandwidth`: a 4 GiB buffer, every page touched before
timing so the page-fault handler is not what gets measured, one `u64` read per
64-byte cache line, summed and `black_box`ed so nothing is optimised away, best
of three per thread count.

```
 threads         GiB/s          GB/s
       1          17.9          19.2
       2          26.0          27.9
       4          29.3          31.4
       8          27.9          29.9
      12          29.8          32.0
      16          30.8          33.1     <- peak
      20          29.3          31.4
```

**30.8 GiB/s = 33.1 GB/s.** The spec sheet for DDR5-4800 dual channel says
76.8 GB/s; silicon delivers 43% of it. **Quoting the datasheet would have
overstated the budget by 2.3x**, which is the whole reason this was measured.

Note the shape: 1 → 4 threads buys 1.6x, and 4 → 20 buys nothing. The bus is
saturated by four cores. That also explains why generation wants 2–4 threads and
prefill wants all of them — one is bandwidth-bound and the other is not.

## The law

Nine models, `-n 24..32`, short prompt, one session, browsers running (so these
are the real numbers on a working machine, not a laboratory one):

| model | resident GiB | tok/s | GiB/s |
|---|---|---|---|
| Qwen2-0.5B | 0.36 | 31.52 | 11.3 |
| Llama-3.2-1B | 0.74 | 24.79 | 18.3 |
| gemma-3-1b | 0.74 | 19.24 | 14.2 |
| Falcon3-1B | 0.98 | 20.41 | 20.0 |
| Llama-3.2-3B | 1.87 | 10.61 | 19.8 |
| gemma-3-4b | 2.31 | 7.73 | 17.9 |
| Qwen3-4B | 2.32 | 7.49 | 17.4 |
| Mistral-7B | 4.07 | 5.96 | 24.3 |
| Qwen3-14B | 8.38 | 2.40 | 20.1 |

**`resident GiB × tok/s` is constant at ~19 GiB/s across a 23x range of model
size.** So:

```
tok/s  ≈  19 / resident GiB
```

The two low outliers are informative rather than noise. **Qwen2-0.5B at 11.3
GiB/s is too small to saturate the bus** — at 31 tok/s the per-token overheads
stop being negligible. **gemma-3-1b at 14.2 against Falcon3-1B's 20.0, at
identical 0.74 GiB**, says architecture still matters: same bytes, 25% fewer
tokens. Size predicts speed; it does not determine it.

## What it settles

**20 tok/s on a model of size S needs `20 × S` GiB/s.** For V4-Flash, a token
reads **3.22 GiB** of experts (43 blocks × 6 routed × 12.8 MiB, matching the
3288 MB measured independently in `v4flash-has-no-slack`), so:

```
20 tok/s  ->  64.4 GiB/s required
                30.8 GiB/s available   (2.1x short)
                 3.09 GiB/s from disk  (21x short)
```

and the 2.1x assumes all 137 GiB of experts are resident on a 15.7 GiB machine,
with the dense trunk free and the implementation at 100% of peak.

**The corroboration from outside**: `kimi-k3-in-c` reports a 128 GB+ workstation
with the model fully in RAM and the disk gone taking **5.6 s/token** — 0.18
tok/s. Removing the disk does not rescue a memory-bound problem, which is
exactly what this law predicts.

**5 tok/s, by contrast, needs 16.1 GiB/s — which is inside this machine's
30.8.** That is why the target moved there, and the ladder is in
`backlog/the-big-bang-5-tok-s.md`.

## The lead this opened

Chaos reads experts at **1.40 GiB/s** while the same NVMe does **3.09 GiB/s**
sequential. That is **45% of the drive**, and unlike the RAM figure it is not
close to peak. An NVMe reaches its rated speed only with several requests in
flight; 12.8 MiB experts read one at a time leave it idle between requests.

**This is not the read/compute overlap already measured at 1.03x** — that
overlapped one read with compute. This is several reads with *each other*, which
is what fills a queue. Untested, and worth a day.

## What this cannot say

- **Write and mixed bandwidth are not measured.** Generation is read-dominated,
  so this is the number that matters, but a claim about anything else needs its
  own run.
- **The 65% figure is not a defect report.** A dequantising matmul does real
  work per byte; nobody has shown 30.8 GiB/s is achievable while also computing.
- **Nine models on one machine.** The law is this laptop's. Another machine has
  its own constant, and `bench-memory-bandwidth` is how to find it.
