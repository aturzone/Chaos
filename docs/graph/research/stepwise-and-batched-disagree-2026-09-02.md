---
topic: Feeding V4-Flash one token at a time does not reproduce a batched prefill — the two paths predict different next tokens after 63 steps, which makes a stepwise perplexity incomparable to any batched engine and raises a question about generation itself
status: open — the divergence is measured, its cause is not attributed
links:
  - requantising-the-trunk-2026-09-02.md
  - ../backlog/score-a-chunk-from-one-batched-pass.md
  - ../reference/hard-won-facts.md
---

# Stepwise and batched prefill disagree

A perplexity comparison put V4-Flash **78% worse than llama.cpp** on the same
prose with the same windowing. The instrument had been validated in the same
session — Qwen3-4B against llama.cpp at **−1.44%**, Qwen3-30B-A3B at **+0.37%** —
so the number looked like a finding about the model.

**It was a finding about the harness, and one control settled it.**

## The control

One batched prefill of 64 tokens against the same 64 fed one at a time, which is
exactly what the perplexity path does — token 0 through `forward`, every later
token through `step`:

```
batched argmax   253
stepwise argmax  103
cosine           0.970304
max |diff|       4.3346
NLL of token 253: batched 2.5959 vs stepwise 2.6377 nats
```

**The two paths predict different next tokens.** `core/arch/tests/stepwise_prefill.rs`.

## Why nothing had caught it

`past_the_old_256_cap_a_cached_step_agrees_with_a_full_prefill` proves
`prefill(0..n) + step(n)`: **one** cached step after a batched prefill, and it
passes. The perplexity path runs the *whole* sequence at batch size 1, so it takes
63 consecutive steps — and whatever the one-step test tolerates, 63 of them
accumulate past an argmax flip.

That is a gap in the equivalence harness rather than a surprise in the engine:
nothing had ever asked for the property, because nothing had ever needed it.

## What it does and does not tell us

**It withdraws the 78%.** `llama-perplexity` evaluates each chunk as a batch and
reads all-position logits; ours stepped one token at a time. Those are not the
same measurement, so the number says nothing about V4-Flash's quality, in either
direction. It is not evidence that quality is fine either.

**Two readings remain, and this node does not choose between them:**

1. **Legitimate divergence.** The repo already records that *"routing flips on
   near ties when the batch shape changes"*, which is why the existing
   equivalence tests assert argmax plus a tolerance rather than equality. Over 63
   steps those flips compound. If that is all this is, neither path is wrong —
   but a stepwise perplexity is still incomparable to any batched engine, and the
   cell needs the batched path regardless.
2. **A real defect.** Both paths ship. A prompt is prefilled in a batch and
   generation then steps, so if they disagree by more than tie-breaking, one of
   them is wrong and every long generation drifts.

**The cheap experiment that separates them** is not more perplexity: it is to run
the control at 4, 8, 16, 32 and 64 steps and watch how the cosine falls. Tie
accumulation should look like a smooth random walk; a structural bug in how the
compressed half or the sliding ring is filled at batch size 1 should show a step
change — most likely at a multiple of `CSA_RATIO`, which is 4, since the
compressed half is built per block of four tokens.

## What it means for the parity gate

**V4-Flash's quality cell cannot be measured comparably yet**, and it now has a
written reason rather than a wrong number — which is what the gate asks for after
Atur redefined it as *every cell measured, not every cell won*.

The fix is named and filed:
`../backlog/score-a-chunk-from-one-batched-pass.md`. Project all positions
through the head so a chunk is scored from a single batched pass, the way
llama.cpp does. That was considered and deferred earlier the same day as "not
needed, because the dense path feeds one token at a time too" — the dense path
gets away with it and this one does not.

## The shape of the mistake, for next time

Three wrong numbers came out of this one measurement, each looking like a model
result:

| number | cause |
|---|---|
| V4-Flash quality +50% | corpus was one sentence repeated 80 times |
| the same, +78% after the corpus fix | stepwise path diverges from batched |
| +9.7% on a control that should agree to 1% | BOS missing at each chunk start |

**Every one was found by a control and none by inspection.** The corpus was
caught by asking a model that already agrees whether it still agreed; the BOS by
reading llama.cpp's source rather than assuming; the stepwise divergence by
testing the property the harness relied on instead of the property it had.
