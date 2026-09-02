---
topic: Project all positions through the head so a perplexity chunk is scored from one batched pass — what V4-Flash's quality parity cell is blocked on, and what would make scoring 60x faster on every architecture
status: proposed, not started
links:
  - ../research/stepwise-and-batched-disagree-2026-09-02.md
  - lts-parity-criteria.md
---

# Score a chunk from one batched pass

Perplexity feeds tokens **one at a time**, on both paths, and the reason is
written down in `perplexity_run`: *"the forward pass projects only the final
position through the output matrix (that was a 253 GFLOP saving on prefill), so
per-position logits are only available a step at a time."*

That choice was correct when it was made and it now blocks a parity cell and
costs an hour a measurement.

## Why it has to change

**The stepwise path does not reproduce a batched prefill on V4-Flash.** 64 tokens
fed one at a time against the same 64 in one pass: cosine 0.970, and the two
paths pick **different next tokens**
(`../research/stepwise-and-batched-disagree-2026-09-02.md`). `llama-perplexity`
evaluates each chunk as a batch, so a stepwise number is not comparable to it —
whatever the divergence turns out to be caused by.

**And it is slow enough to shape what gets measured.** A scored token costs ~1.4 s
on V4-Flash because each one is a whole forward pass, so five chunks of 128 is
fifteen minutes a side and five chunks of 512 is an hour. Every corpus decision
tonight was made around that budget rather than around what the measurement
needed.

A batched chunk is **one** pass for 512 positions instead of 512 passes. On this
model that is the difference between an hour and a minute.

## The change

`head` collapses the hyper-connection streams and projects **the last position**:

```rust
let last = &streams[streams.len() - hc_dim..];
let x = ctx.new_f32_3d(n_embd, hc, 1)?;
```

It needs a sibling that takes `n_tokens` and shapes `x` as
`[n_embd, hc, n_tokens]`, so `mul_mat(output.weight, result)` yields
`[vocab, n_tokens]`. Three things to get right:

1. **`dsv4_hc_pre` must accept `nt > 1` here.** It already does in the block body,
   which is where the same collapse runs for a whole prefill batch — so this is
   reuse rather than new arithmetic.
2. **The arena.** 512 positions x 129,280 vocab x 4 bytes is 265 MB of logits
   alone, on a path whose arena aborted at 1 GiB tonight for an unrelated reason.
   Size it from the batch, and remember ggml **aborts** rather than refusing.
3. **`forward` must be able to return the streams** rather than only the head's
   output, so the caller can choose which head to run. Today the tail of `forward`
   is `head(fw, &streams, arena)`; splitting that into `forward_streams` plus a
   head call is the whole refactor, and `prefill`/`step` keep their signatures.

The dense path wants the same treatment for the same two reasons, and its
`build_graph` already had the "project only the final position" surgery applied
deliberately — so this is an added capability, not a revert.

## What it unblocks

- **V4-Flash's quality parity cell**, which is the last one with no number.
- **A perplexity that can be taken on a real corpus** rather than one sized to a
  time budget: 4040 tokens becomes minutes.
- **A cheaper answer to the divergence question itself.** With batched scoring the
  control becomes "score the same chunk both ways", which is one pass plus one
  stepwise run rather than two stepwise runs.

## What it does not do

It does not decide whether the stepwise/batched divergence is tie-breaking or a
bug. That needs the step-count sweep described in the research node — cosine at
4, 8, 16, 32, 64 steps, watching for a step change at a multiple of `CSA_RATIO`.
**Do that first if the answer matters more than the cell**, because a structural
bug there would affect every long generation and not just a measurement.
