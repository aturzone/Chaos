---
topic: F profiled at last — half of the non-disk cost of a V4-Flash token is ggml graph *construction*, not arithmetic
status: resolved
links:
  - queue-depth-and-the-real-ceiling-2026-08-26.md
  - v4flash-ram-frontier-2026-08-16.md
  - ../backlog/lts-0-0-0.md
  - ../backlog/the-big-bang-5-tok-s.md
---

# What F is made of

**`F` is the part of a V4-Flash token that never touches the disk.** It was
measured once, on 2026-08-16, at **0.84 s**, and never opened up. That single
number caps this machine at **1/F = 1.19 tok/s** no matter how fast the drive
gets, so every ladder in `the-big-bang-5-tok-s.md` runs into it — and
`queue-depth-and-the-real-ceiling` says so in as many words:

> **`F` was measured once, on 2026-08-16.** It is the weakest number here. If the
> 0.84 s is itself improvable — and **nobody has profiled what is inside it** —
> every row above moves. **That is the one measurement left worth taking.**

Taken, 2026-08-31.

## The run

Atur closed applications first, which is what made the measurement clean: **10.0
GiB available, 9.0 GiB usable for weights** against a 7.38 GiB trunk, so the
trunk is resident and the numbers are comparable to the 2026-08-16 set.

```
CHAOS_BLOCK_TIMING=1 chaos-run DeepSeek-V4-Flash-UD-Q4_K_XL-00001-of-00005.gguf \
    "The capital of France is" -n 3 --temp 0 -t 4

generate   2 tokens in 4.0s (0.494 tok/s, 2.0s per token)
```

**0.494 tok/s**, against the 0.43 on record — better, in better conditions, and
the point of quoting it is that the decomposition below belongs to *this* run.
Expert traffic came out at **3268 MiB per token**, which matches the documented
3.22 GiB, and the trunk read **59 MiB**, i.e. essentially resident.

## What the timers actually mean

This is the part that makes the numbers readable, and the code says it plainly:

> The block builds one graph and evaluates it here, so every phase timer above
> measures graph **construction** (plus, in `ffn`, the disk read). This is the
> only line where arithmetic actually happens.

So `qkv`, `attn` and `tail` are the cost of *describing* work to ggml; `compute`
is the cost of *doing* it; and `ffn` contains the expert read, which is why it
tracks the expert line almost exactly.

## The decomposition, averaged over the two generated tokens

| | seconds | share of the token |
|---|---|---|
| sum of block totals | **1.84** | 100% |
| expert reads — disk | **1.11** | 60% |
| **F, everything else** | **0.73** | **40%** |

and F splits:

| inside F | seconds | share of F | what it is |
|---|---|---|---|
| `compute` | **0.47** | 64% | the whole graph evaluated — all the arithmetic |
| `tail` | **0.36** | 49% | graph *construction* for `layer_tail` + `moe_routing` |
| `qkv`, `attn` | 0.00 | ~0% | graph construction, too small to register |
| `arena`, `dense` | 0.00 | ~0% | the trunk is resident |

The two shares sum past 100% by about 0.1 s because `ffn` overlaps the expert
line and the block total covers a little that is attributed to neither. **Do not
read the sub-rows as a partition**; read them as "this is where the time is".

## The finding

**Half of F is not arithmetic. It is building a description of the arithmetic.**

`tail` — `layer_tail` plus `moe_routing` — costs **0.36 s per token, about 20% of
the whole token**, and it constructs graph nodes rather than multiplying
anything. That is a large number for what the codebase itself calls *"a handful
of small ops"*.

**`lts-0-0-0.md` T0.6 predicted this and has been asking since day one:**

> Profile `layer_tail` + `moe_routing`, which is **21% of a pass** for a handful
> of small ops and has never been looked at.

**21% predicted, 20% measured.** The ticket was right, it was never done, and
nothing that has been proposed since would have touched it — every rung in the
5 tok/s ladder attacks the 60% that is disk.

## What it is worth

Graph construction is overhead in the strictest sense: the same graph shape is
rebuilt for every block of every token. If most of `tail` can be removed — by
building the block graph once and rebinding its inputs, which is the obvious
shape of the fix and is not costed here — a token goes from **1.84 s to roughly
1.5 s**, i.e. **0.494 → about 0.61 tok/s, 1.24x**.

That is smaller than it sounds and better than it sounds. Smaller, because 60% of
the token is still disk and this does not touch it. Better, because it is
**free of quality risk**: it changes when a graph is described, not what is
computed, so unlike 2-bit experts or top-k routing it needs no quality harness to
ship.

**And it does not reach 5 tok/s.** Nothing here changes the standing arithmetic:
137 GiB of experts cannot be resident, so they cross a 3.41 GiB/s drive every
token, and even one-bit experts with a free trunk and zero arithmetic cap this
drive at 4.26 tok/s.

## Two cautions on this measurement

- **Two generated tokens, one run.** The 2026-08-16 figure was a median of four.
  These two agree closely with each other (1.87 s and 1.80 s) and F lands at
  0.73 s against the earlier 0.84 s, which is the same number under slightly
  better conditions rather than a new one. It is not a median and should not be
  quoted as one.
- **`attn_secs` is measured from a clock that is not reset after `qkv`**, so it
  is cumulative rather than a phase. Both read 0.00 here so nothing rests on it,
  but anyone reading those two columns on a machine where they are non-zero
  should look at the code first.
