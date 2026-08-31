---
topic: the router's 0.22 s is the BF16 gate matmul, not the top-k sort — so moving the selection to the CPU saves nothing
status: resolved
links:
  - what-f-is-made-of-2026-08-31.md
  - v4flash-repacking-2026-08-10.md
  - ../reference/hard-won-facts.md
---

# The router is a BF16 matmul

`what-f-is-made-of` found the router costing **0.22 s of a V4-Flash token** —
5.5 ms in each of the 40 blocks that use `argsort_top_k`, against **0.000 s** in
the 3 hash layers that look routing up by token id. The obvious reading was that
`argsort_top_k` was slow, and the obvious fix was to select the top 6 of 256 on
the CPU instead, with no graph. That was filed as **C5c** and estimated at 1.26x.

**Both the fix and the estimate were wrong, and one measurement settled it before
a line of it was written.**

## Which half costs the time

`CHAOS_ROUTE_SPLIT=1` (added with this node) evaluates `probs` on its own before
`topk` is built, so the two halves can be separated. Averaged over generated
tokens, blocks 3–42:

| | seconds |
|---|---|
| `route-probs` — BF16 `mul_mat` + `softplus` + `sqrt` | **0.256** |
| `route-compute` — the same DAG **plus** `add` and `argsort_top_k` | **0.221** |
| ⇒ `argsort_top_k` and the bias `add` | **~0.000** |

`route-compute` measuring *slightly less* than `route-probs` is the honest shape
of two separate timings of nearly the same work, one of which warms the cache for
the other. What it rules out is any meaningful cost in the sort.

**So the entire router cost is the gate matmul.** `argsort_top_k` over 256 values
is free, which it should be — it is 256 floats.

**C5c is dead.** Selecting the top 6 on the CPU would still need `probs` on the
CPU, which needs the same matmul and the same graph evaluation. It would move
work that costs nothing and leave the work that costs everything.

## And the estimate was inflated, by mixing two clocks

C5c was quoted at **1.26x**. That number divided the **sum of block totals**
(1.77 s) while comparing against a **wall-clock** tok/s (0.511). There is about
**0.21 s per token outside the blocks**, so the two are not interchangeable:

```
  measured, three runs      0.494 / 0.510 / 0.511 tok/s   mean 0.505
  => real wall-clock        1.980 s per token
  router                    0.221 s   =  11.2% of the token
  router free               1.759 s   =  0.568 tok/s   =  1.13x
```

**1.13x, not 1.26x**, and that is the ceiling on removing the router *entirely* —
which no fix does.

## What the real candidate is

`ffn_gate_inp` is **BF16** (`v4flash-repacking-2026-08-10` lists it, and it is
the one trunk tensor that is neither Q8_0 nor F32). The matmul is 4096 x 256 for
a single token: **1.05 million multiply-accumulates, taking 6.4 ms**. That is
roughly two orders of magnitude slower than the same shape has any right to be.

The same repacking node records the adjacent fact: *"every repackable trunk
tensor is Q8_0 and ggml has **no x86 Q8_0 branch**"*. A missing fast path for an
unusual dtype is a theme in this container rather than a coincidence, and **BF16
on x86 is the obvious suspect**.

**Untested, and stated as untested.** The next measurement is a like-for-like
`mul_mat` of that shape in BF16 against F16 and F32 — `chaos-kernelbench` is the
place for it. If BF16 is the problem, converting `ffn_gate_inp` once at load
costs about **90 MB** of extra residency against a 7.38 GiB trunk, which is
nothing, and it is **exact**: a BF16 value converted to F32 is the same value, so
this would go behind the quality gate's *exact* bar and should come back 100%
byte-identical.

If BF16 is *not* the problem, then a 4096 x 256 matmul really does cost 6.4 ms
here and something much more basic is wrong, which is worth knowing either way.

## The pattern in the last three findings

Three claims in a row have been corrected by measuring rather than reasoning, and
all three were mine:

| claimed | measured |
|---|---|
| the I/O headroom is 1.69x | **1.19x** — the expert read was already at 2.88 GiB/s |
| half of F is graph construction | **7%** — F is 93% arithmetic |
| the router's cost is the top-k sort | **~0%** — it is the BF16 matmul |

Each was a reasonable inference from a real number, and each was wrong. The
common thread is that the number was read from a document or a comment rather
than from the thing itself: the expert rate from a node taken under memory
pressure, the construction/arithmetic split from a comment that omitted a
`compute` call, and the router's cost from the presence of the word `argsort`.
