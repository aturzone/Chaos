---
topic: Feeding V4-Flash one token at a time does not reproduce a batched prefill — the two paths predict different next tokens after 63 steps, which makes a stepwise perplexity incomparable to any batched engine and raises a question about generation itself
status: ANSWERED — the divergence appears exactly when a compressed block completes, so it is structural rather than tie-breaking. The defect itself is not yet located.
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


## ANSWERED 2026-09-03: it is the block boundary, not tie-breaking

The sweep, at nine lengths. The final position's logits, batched against stepwise:

```
  tokens  cosine     max |diff|   completes a block?
       3  0.999866      0.4808    no    <- the paths AGREE
       4  0.981401      4.9674    yes   <- 10x the error, immediately
       5  0.996695      2.5935    no
       6  0.996145      2.4938    no
       7  0.990928      3.3050    no
       8  0.984060      5.5758    yes   <- worst of the small lengths
      16  0.990389      2.9377    yes
      32  0.985793      4.2033    yes
      64  0.970304      4.3346    yes
```

**At three tokens the two paths agree to cosine 0.99987**, a max logit difference
of 0.48 — floating-point reordering, nothing more. Three tokens is the only
length here that never completes a compressed block, because `CSA_RATIO` is 4.

**At four, the first complete block, the error jumps ten-fold.** Five, six and
seven end mid-block and recover to 0.99+. Eight ends on a boundary and falls
again, to the worst value of any small length.

**Tie-breaking cannot do that.** A near-tie in routing has no reason to care
whether the final position sits on a multiple of four. Something about
completing a compressed block differs between one pass of *n* tokens and *n*
passes of one, and it is worth roughly ten times the ordinary numerical noise.

**This is not a measurement artefact and it is not academic**: generation always
takes the stepwise path. Every token a user reads comes from the side of this
comparison that disagrees.

### Two earlier readings, both wrong, both mine

- *"The divergence accumulates over steps."* It does not. It is present at four
  tokens and no worse at thirty-two.
- The first sweep sampled **4, 8, 16, 32, 64 — every one a multiple of four**,
  and its "block boundary" column read `yes` on every row. It was built to find a
  cliff at a block boundary and could not have seen one. The odd lengths are the
  informative ones and they were missing.

### Where to look

`compressor(...)` in the `(AttentionKind::CompressedSparse, true)` arm of the
block builder, and whatever decides that a block is complete. The question is
what the batched path writes into the compressed half when a pass ends exactly
on a boundary, and what the stepwise path writes when the *last of four*
single-token passes closes the same block.

The oracle capture at 300 tokens already exists
(`llama-eval-callback`, `--no-repack`), so the next step needs no new fixture:
diff our compressed-half tensors at a length ending on a boundary against
llama.cpp's, which is how all 43 layers were built in the first place.

### One candidate eliminated by hand, 2026-09-03

**The ring alignment is not the bug.** The obvious suspect was
`compressor_project`'s ring: the stepwise path assembles a completed block from
*ring rows plus one batch row*, where the batched path takes all four from the
batch, and an off-by-one in that mapping would land exactly on a block boundary
and nowhere else.

It was checked by hand at `pos0` = 3, 7, 8 and 11, following `row_of(q) =
state_rows + q - pos0` against what the ring actually holds after `keep = 8`
rows and the drain:

| `pos0` | ring holds | front pad | block's rows land at | correct? |
|---|---|---|---|---|
| 3 | positions 0-2 | 5 zero rows | 5, 6, 7, and 8 from the batch | yes |
| 7 | positions 0-6 | 1 zero row | 5, 6, 7, 8 | yes |
| 11 | positions 3-10 (drained) | none | 5, 6, 7, 8 | yes |

The overlap half checks out too: for block 0 it reads `p = -4..-1`, every one
resolving to `zero_row`, which is the appended pad row — 0 for the kv state and
`-inf` for the score, so the softmax ignores it. Both paths do that identically.
The APE index `(pos0 + p) % ratio` also agrees, 0/1/2/3 either way.

So the divergence is **not** a misaligned ring, and the front-pad arithmetic is
right. What that leaves is the projection itself (`mul_mat` over one column
against four is a different ggml kernel, but that is a 1e-3 effect and would not
give 0.98) or how the compressed half is **consumed** at `nt = 1` — four tokens
is simply the first length at which a non-empty compressed half enters the
attention output at all, which is why the error appears there.

**This is a narrowing, not a diagnosis, and it went as far as reading gets.**
The next step is unchanged and is a measurement: diff the compressed-half
tensors against llama.cpp at a boundary length. Reading further would be
guessing, and this repository has a rule about that.

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
