---
topic: the dense path copied the whole KV cache every token; the copies are gone, byte-identical, worth 7.1% at 4031 tokens — and the instrument over-predicted that by 3.5x, which is the lesson
status: resolved, and shipped — with the projection corrected downward by 3.5x
links:
  - long-context-parity-qwen3-4b-2026-09-01.md
  - ../backlog/lts-parity-criteria.md
  - ../reference/hard-won-facts.md
---

# The KV cache is copied every token

`long-context-parity-qwen3-4b` measured Chaos's per-token cost rising **0.0474 ms
per token of context** against llama.cpp's **0.0214** — both linear, so a constant
factor of about **2.2x** in whatever grows with the KV. It ruled out flash
attention (both engines have it), the KV cache's size and dtype (both carry
571 MiB of f16 at 4062 positions), the FFN (§4a's 61% is a short-context
statement, and at 500 tokens Chaos is *ahead*), and the thread cap (4 is optimal
at both ends).

**What is left is two lines in `qwen3.rs::attention_flash`:**

```rust
let k = ctx.cont(&ctx.permute(k_all, [0, 2, 1, 3])?)?;
let v = ctx.cont(&ctx.permute(v_all, [0, 2, 1, 3])?)?;
```

with its own comment saying why: *"ggml wants `[head_dim, n_batch, n_head]` for q
and `[head_dim, n_kv, n_head_kv]` for k and v. **Ours are head-major, so
permute**."*

`cont` does not annotate a view — it **materialises a contiguous copy**. So on
every generated token, for every one of 36 layers, the entire key cache and the
entire value cache are copied into a new buffer, read once by the kernel, and
thrown away.

## Measured

`core/ggml/tests/kv_cont_is_the_context_cost.rs`, at Qwen3-4B's own shape
(`head_dim 128, 32 heads (8 kv), 36 layers`, one new token), against a cache
already in the layout the kernel wants:

| `n_total` | with `cont` | without | difference | x36 layers |
|---:|---:|---:|---:|---:|
| 500 | 0.6479 | 0.1786 | 0.4693 | 16.90 ms |
| 1001 | 1.0945 | 0.2579 | 0.8366 | 30.12 ms |
| 2011 | 2.3814 | 0.4530 | 1.9284 | 69.42 ms |
| 4031 | 3.4826 | 1.0507 | 2.4318 | **87.55 ms** |

```
  slope per token of context, all 36 layers:
    with cont     0.02890 ms
    without cont  0.00889 ms       ratio 3.25x
```

**The copy triples the cost of attention's context term.** And the instrument's
3.25x brackets the engine's own 2.2x deficit from above, which is what a complete
explanation should do: the engine's per-token slope includes work that does not
grow with context, so its ratio is diluted.

### A bias found and removed, in the direction that flattered the conclusion

The first version of this test allocated and filled the kernel-layout cache
**inside** the timing loop, so that arm read a buffer it had just written — partly
in cache — while the `cont` arm read one written once and long since evicted. It
measured 2.84x. With both arms reading a buffer written once, it measures
**3.25x**. The bias was making the conclusion *weaker*, but it was still a bias
running in the direction of what was being tested, which is the worst direction
for one to run in.

## Shipped, and the projection was wrong by 3.5x

**`flash_attn_ext` honours the strides.** `does_flash_attn_ext_accept_a_strided_kv`
hands it `permute(kv)` with no `cont` at a hand-checkable size and gets
**byte-identical** output, so the copies were never load-bearing — and the fix is
**two deletions**, not the cache-layout change described below.

Timed three ways at Qwen3-4B's shape, slope per token of context across 36 layers:

```
  cont (what it did)    0.02406 ms
  kernel layout         0.00502 ms    4.79x better  -- a layout change
  strided, no cont      0.00891 ms    2.70x better  -- two deletions
```

So the two deletions were predicted to recover 80% of what a layout change would,
worth about **61 ms of a 306.7 ms token** at 4031 positions — 3.26 to **~4.07
tok/s**.

**The engine gained 7.1%, not 25%.** Alternating in one binary
(`CHAOS_KV_CONT=1` restores the copies), three pairs at each length:

| | with `cont` | without | |
|---|---|---|---|
| 500 tokens | 7.12 / 7.17 / 6.78 | **7.45 / 7.30 / 6.90** | +2.5% |
| 4031 tokens | 3.23 / 3.19 / 3.26 | **3.46 / 3.47 / 3.44** | **+7.1%** |

**Without wins all six pairs**, which is what makes 2.5% believable at all. An
earlier single-run comparison had suggested the change *hurt* short context by
3.6%; three alternating pairs show the opposite, and that is the difference
between one run and a protocol.

Against llama.cpp at 4031: **1.38x behind becomes 1.32x.** Real, consistent,
byte-identical — and a long way from the projection.

### Why the instrument over-predicted, and what to take from it

The instrument timed the `cont` in isolation, in a fresh arena, as its own graph
evaluation. In the engine the copy sits inside one realized graph beside
everything else, and the bytes it writes are read by the kernel immediately
afterwards — so much of the copy's cost is already hidden behind work the engine
was doing anyway.

**An instrument measures an operation; an engine measures a schedule.** The ratio
between two arms of an instrument transfers; the absolute saving does not. Both
earlier instruments in this repository were used for ratios only
(`router_matmul_dtypes` to kill a dtype theory, `trunk_mat_vec_dtypes` to kill
another) and both held up. This one was used for a *projection* and was wrong by
3.5x. The projection was labelled a projection, which is why this is a correction
rather than a retraction — but the label is not a substitute for measuring the
engine.

## What the fix was projected to be worth — superseded by the section above

Removing the copy saves **0.02001 ms per token of context** across the 36 layers.
At 4031 tokens that is **80.7 ms** off a token that currently takes 306.7 ms:

```
  Chaos today, 4031 tokens      306.7 ms/token   3.26 tok/s   1.38x behind
  minus the cont copies         ~226   ms/token   ~4.42 tok/s  ~1.02x
  llama.cpp, same run           222.7 ms/token   4.49 tok/s
```

**That is the whole long-context gap.** It is also *arithmetic on an instrument's
numbers*, not a measurement of a fixed engine, and it is written here as a
projection. The three retractions in this repository from 2026-09-01 were all
projections stated as findings; this one says which it is.

## Why the *layout* change is still not a two-line fix

The remaining 4.79x-vs-2.70x gap needs the cache stored differently, and that is
the change described here. It is worth about another 4% on top of what shipped,
and it is a great deal more work.

The cause is that the KV cache is **stored** head-major,
`[head_dim, n_kv_head, n_total]`, and flash attention wants
`[head_dim, n_total, n_kv_head]`. The fix is to store it the other way round, and
that reaches:

- the cache's append path, which writes one position per token;
- every architecture on the dense path — **fourteen of them**, all sharing
  `attention_flash`;
- the `mul_mat` attention path if it still has callers, which wants the *other*
  layout and is the reason head-major was chosen (the same file records a third
  attention implementation that was deleted for having no callers at all);
- the device path, where `set_input` flags and realization order are load-bearing
  and where writing the mask in the wrong place already segfaulted once.

So it is a real change to the one path every model takes, and it must go through
the quality gate's **exact** bar — a reordering of storage should not alter a
single logit, and if it does, something else is wrong.

**Filed, not built** — and now worth less than it looked, since the two deletions
already took the 2.70x of it that was cheap.

## The thing that was checked first, and settled it

Whether `flash_attn_ext` would accept a **non-contiguous** k and v. It does, and
byte-identically — which is what turned a fourteen-architecture change into two
deletions. The comment in `qwen3.rs` said ggml *wants* that shape; it never said
the kernel refuses a view of it, and nobody had asked.

**Verification of what shipped**: the quality gate's *exact* bar on Qwen3-4B,
**50 of 50 byte-identical**; 986 tests; and the container-backed `--ignored` suite
run against real models, 32 of 33 passing — the one failure is a **chat-template
detection bug in `chaos-tokenizer`**, which cannot see this change (that crate
depends on `chaos-gguf` and `chaos-model` only) and is filed separately.
