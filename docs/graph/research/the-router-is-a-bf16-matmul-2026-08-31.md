---
topic: the router's 0.22 s is the block tail computed TWICE — not the top-k sort, and not the BF16 dtype. The filename is stale and kept so links do not rot.
status: resolved
links:
  - what-f-is-made-of-2026-08-31.md
  - v4flash-repacking-2026-08-10.md
  - ../reference/hard-won-facts.md
---

# The router is a BF16 matmul

> ## CORRECTION, same day: it is not the dtype either. It is computed twice.
>
> **The title of this node is wrong** and the fix it proposed (C5d, convert the
> BF16 gate weight at load) is dead alongside the one it replaced. Both were
> killed by measuring instead of reasoning, which is now the fourth time in a row.
>
> `core/ggml/tests/router_matmul_dtypes.rs` times that exact matmul, 4096 x 256
> against one token, 200 repetitions, warm-up discarded:
>
> | dtype | ms/matmul | GFLOP/s |
> |---|---|---|
> | F32 | 0.1503 | 13.96 |
> | BF16 | **0.1501** | 13.97 |
>
> **BF16 is 1.00x F32, and both take 0.15 ms — not 6.4 ms.** A 43x gap between
> the isolated matmul and what the engine pays means the 6.4 ms was never the
> matmul.
>
> ### What it actually is
>
> `ctx.compute(&topk)` does not evaluate the router. It evaluates **everything
> `topk` depends on**, and that chain reaches back through `probs` to `logits` to
> `ffn_norm` — which is `layer_tail`'s output. So it computes the block's tail,
> and then the block's own `ctx.compute(&out)` at the end computes it **again**.
>
> The three hash layers prove it, because their `topk` is `get_rows(tid2eid, tok)`
> and depends on the token ids alone:
>
> | per block | hash (0-2) | argsort (3-42) |
> |---|---|---|
> | `route-compute` | 0.0000 | **0.0055** |
> | final `compute` | 0.0100 | **0.0101** |
>
> **If the early evaluation had done that work early rather than extra, the final
> compute would be smaller by about 0.0055. It is identical.** The 40 argsort
> blocks each pay 5.5 ms twice; the 3 hash blocks pay once.
>
> ### So the fix is neither of the two that were filed
>
> It is **do not recompute**. After `ctx.compute(&topk)`, the values in
> `ffn_norm`, `probs3` and `topk` are already correct; copying them into fresh
> leaf tensors (`ggml_new_tensor` with no op) before the downstream graph is
> built would stop ggml walking back through the tail a second time. That is a
> few thousand floats copied against 5.5 ms of arithmetic saved per block.
>
> **Worth 0.221 s of a 1.980 s token — 1.13x — and exact**, since copying a
> computed value changes nothing numerically. It goes behind the quality gate's
> *exact* bar and should come back 100% byte-identical. Filed as **C5e**.
>
> Everything below this line is the superseded reasoning, kept because the two
> dead ends are worth not re-walking.

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
