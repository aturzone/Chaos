---
topic: the dense path copies the whole KV cache every token to satisfy flash attention's layout — measured at 3.25x the attention slope, and removing it would close the long-context gap
status: resolved as a diagnosis; the fix is not built
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

## What the fix would be worth — projected, not measured

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

## Why it is not a two-line fix

The `cont` is a symptom. The cause is that the KV cache is **stored** head-major,
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

**Filed, not built.** The measurement is the deliverable here; the fix wants a
session that starts fresh rather than one that has been running all night.

## The one thing to check first

Whether ggml's `flash_attn_ext` will accept a **non-contiguous** k and v directly,
or whether `cont` is genuinely required. If it accepts strided inputs, the copy
may be removable without touching the cache's layout at all — which would be a
much smaller change than the one above. The comment says ggml *wants* that shape;
it does not say the kernel refuses a view of it.
