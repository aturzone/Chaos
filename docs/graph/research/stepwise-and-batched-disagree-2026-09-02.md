---
topic: Feeding V4-Flash one token at a time does not reproduce a batched prefill — the two paths predict different next tokens after 63 steps, which makes a stepwise perplexity incomparable to any batched engine and raises a question about generation itself
status: ANSWERED 2026-09-03 -- the two paths choose DIFFERENT EXPERTS, from the first routed layer (3 of 43 layers at 3 tokens, 33 of 43 at 4). The compressor gathers identical positions (proven by test) but its values differ by more than rounding. Which path is wrong is still open: every oracle capture is batched.
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

### Two candidates eliminated, and one claim of mine withdrawn, 2026-09-03

**The ring index arithmetic is not the bug, and that is now mechanical rather
than read.** The obvious suspect was `compressor_project`'s ring: the stepwise
path assembles a completed block from *ring rows plus one batch row* where the
batched path takes all four from the batch, and an off-by-one there would land
on a block boundary and nowhere else.

The arithmetic is now a pure function, `compressor_positions(pos0, nt, ratio,
overlap)`, with four tests that need no container:

| test | asserts |
|---|---|
| `a_batch_and_a_sequence_of_steps_summarise_the_same_blocks` | one pass over `4n` tokens and `4n` single steps close the same blocks from the same positions, for 1-5 blocks, both overlap forms |
| `a_pass_that_completes_no_block_summarises_nothing` | 1, 2 and 3 tokens close nothing; block 0's overlap half lies entirely before the start |
| `nothing_is_gathered_from_before_the_ring` | no position is more than 8 rows behind `pos0`, for every `pos0` under 64 and six batch sizes -- the reason `state_rows` is 8 and not 4 |
| `chunking_a_prefill_closes_the_same_blocks` | a 48-token prefill in chunks of 4, 8, 12, 16 and 24 closes what one pass would |

All four pass. **My first version of the first test failed, and the test was
wrong** -- it concatenated the positions from a batch and from the equivalent
steps and demanded equality. They differ: a batch of 12 emits three overlap
halves and *then* three current halves, while three closing steps each emit
their own (overlap, current) pair. Both are correctly grouped **per call**,
which is all the consumer requires -- it reads `rows[0..n_read]` as the overlap
half and `rows[n_read..]` as the current half of *that call*. Comparing across
calls was meaningless. That is the second time in this investigation the test
design was the thing at fault, after the sweep that sampled only multiples of
four.

**C5e freezing is not the bug either.** It is the one deliberately
batch-shape-dependent path in the block, so it was the next suspect:
`freeze_the_tail(hash_layer_count, il, nt)` is `il >= hash_layer_count && nt <=
FREEZE_MAX_TOKENS`, and `FREEZE_MAX_TOKENS` is **192**. Every length in the
sweep -- 3, 4, 5, 6, 7, 8, 16, 32, 64 -- is under 192 on both paths, so freezing
is on for the batch and on for every step alike. Read rather than measured, but
the predicate is two terms and neither depends on anything else.

### Withdrawn: "tie-breaking cannot do that"

The previous entry asserted that a near-tie in routing *"has no reason to care
whether the final position lands on a multiple of four."* **That was too strong,
and it does have a reason.** The position that closes a block is the position
whose hidden state most directly incorporates the block just closed -- and it is
also the position whose logits the comparison reads. So a small difference in
the compressed half is most exposed exactly at a boundary, and a routing flip
would amplify it from rounding into a different answer.

Re-reading the sweep with that in mind, there are **two** effects and the
earlier note collapsed them into one:

| | cosine |
|---|---|
| no block closes (3) | **0.99987** |
| ends mid-block (5, 6, 7) | 0.9967, 0.9961, 0.9909 |
| ends on a boundary (4, 8, 16, 32) | 0.9814, 0.9841, 0.9904, 0.9858 |

Empty compressed half against non-empty is the large, unambiguous step. Boundary
against mid-block is real but much smaller -- roughly 0.983 against 0.994 -- and
7 (0.9909) overlaps 16 (0.9904), so it is a tendency, not a cliff. "Ten times
the error, immediately" described the 3-to-4 transition, which is the
empty-to-non-empty step and not the boundary.

### ANSWERED 2026-09-03: the experts disagree, and from the first routed layer

The instrument was already in the engine and nobody had pointed it at this.
`routing_last_token()` returns the expert ids the **final** token of the most
recent pass selected, per layer, and `routing_last_token_reset()` exists so that
two passes can be compared cleanly. It needs `CHAOS_ROUTING_LAST=1` — the first
run of the new test reported *"0 layers logged"*, which is what an unset gate
looks like and would have read as "the experts agree" to anyone who did not
check.

`core/arch/tests/stepwise_layer_divergence.rs`, four tokens against three, with
the logit numbers reproducing `stepwise_drift` exactly:

| | 3 tokens (closes no block) | 4 tokens (closes one) |
|---|---|---|
| logits, cosine | 0.999866 | 0.981401 |
| logits, max abs | 0.4808 | 4.9674 |
| layers choosing **different experts** | **3 of 43** | **33 of 43** |
| first such layer | **30** | **3** |

Every differing layer differs in one or two of its six, e.g. at 4 tokens layer 3
takes `[6, 62, 111, 136, 167, 250]` batched and `[62, 78, 136, 167, 249, 250]`
stepwise.

**Three things follow, and the third is the finding.**

1. **Routing flips are real on this architecture and are not the whole story.**
   Three layers flip even at 3 tokens, where no compressed block exists at all
   and the logits still agree to 0.99987. A few flips deep in the stack cost
   almost nothing.
2. **At 4 tokens the flips start at layer 3**, which is the *first* layer with
   routed experts — `hash_layer_count` is 3, so layers 0-2 have none. There is
   no earlier layer that could have flipped. The 33 differing layers are then
   mostly cascade: once one expert differs, everything downstream is perturbed
   by a whole expert's contribution.
3. **So the perturbation entering the first routed layer is far larger than
   floating-point reordering.** That is the inference the pairing supports: at 3
   tokens, rounding-level differences leave every layer before 30 agreeing; at 4
   tokens the very first router that *can* flip does flip. Rounding does not
   become 26 layers more potent because a block closed. **The compressor's
   values differ between the two shapes by more than rounding — and its indices
   are proven identical, so the difference is in what it computes, not in what
   it reads.**

### What this does NOT establish: which path is wrong

**Both oracle captures are batched.** `llama-eval-callback` was run on a prompt,
so every one of the 22 container tests — `the_library_forward_pass_matches_llama_cpp`
included — verifies the *batched* path. The stepwise path has never been diffed
against anything, and it is the path every generated token comes from.

So "the batched path is right and generation is wrong" is the likely reading but
is not yet measured. Settling it needs llama.cpp captured under `-b 1`, which no
fixture here has.

### LOCALISED to layer 2's compressed attention, 2026-09-03

`CHAOS_DUMP_LAYERS` — added to this path in the same session, because it had
none — gives the final position's sum for every layer on both paths. With the
attention plan beside it, the origin is unambiguous:

```
attention by layer: 0:Raw  1:Raw  2:CompressedSparse  3:HeavilyCompressed
                    4:CompressedSparse  5:HeavilyCompressed  ...
hash layers (no routed experts): 0..3
```

| layer | kind | n=2 | n=3 | n=4 |
|---|---|---|---|---|
| 0 | Raw | 1.51e-07 | 4.65e-06 | **0.00e+00** |
| 1 | Raw | 3.12e-07 | 6.97e-04 | 2.82e-03 |
| 2 | **CompressedSparse** | 1.22e-04 | 4.19e-03 | **2.70e-01** |
| 3 | HeavilyCompressed, first **routed** layer | 1.14e-01 | 5.48e-02 | 2.63e-01 |
| 4 | CompressedSparse | 4.20e-03 | 1.08e-01 | 1.60e-01 |

**Three facts pin it.**

1. **Layer 2 is the first `CompressedSparse` layer, and four tokens is exactly
   where its first block closes.** `CSA_RATIO` is 4. At three tokens the same
   layer differs by 4.19e-03; at four it differs by 2.70e-01, a **hundredfold
   jump** across the boundary.
2. **Layer 2 has no routed experts**, because `hash_layer_count` is 3 and covers
   layers 0-2. So the jump cannot be a routing flip — there is nothing there to
   flip. The 33 differing routers from layer 3 onward are downstream of this.
3. **Layer 0 is bit-identical** at four tokens, which is the control this needed:
   a `Raw` layer, reading keys 0-2 from the cache on one path and from its own
   batch on the other, reproduces exactly. The KV cache path is not the problem.

**Two tokens is the control that makes this tight.** At two tokens *nothing*
closes a block anywhere in the stack — `CSA_RATIO` is 4 and `HCA_RATIO` is 128 —
and there layer 2 sits at **1.22e-04**. Same layer, same code, one token more,
and it is at 2.70e-01. The only thing that changed for it is that its first
block closed.

**Read the table with one caveat: a per-layer relative difference is not
proportional to the final logit error.** Layer 3 is at 1.14e-01 at two tokens
while the logits still agree to cosine 0.999874, because it is the first layer
with routed experts and a single flip there is absorbed downstream. What
distinguishes four tokens is not the size of any one number but *where* the
first large one appears: at two and three tokens the stack is clean until the
first routed layer, and at four it is already broken one layer earlier.


So: **the origin is layer 2's compressed attention at the length where its first
block closes.** Everything after it — the routing flips, the 4.9 logit gap, the
different sampled token — is consequence.

### Still open, and stated as open

**Why layer 1 differs at all.** Layer 0 is bit-identical, so layer 1 receives
identical input, and layer 1 is `Raw` with no compressor and no routed experts —
yet it differs by 2.82e-03. That is larger than rounding usually looks and it is
unexplained. It may be a `mul_mat` over one column against four taking a
different kernel; that is a guess, not a measurement.

**Whether layer 2's jump is that difference amplified, or an independent
defect.** A softmax over eight entries can turn a small score change into a
large output change when the entries are close, so 2.8e-03 in and 2.7e-01 out is
not by itself proof of a second bug. Distinguishing them needs the compressor's
own tensors compared between the two shapes — the same checkpoints
`csa_compressor_matches_llama_cpp` already asserts at `pos0 = 0`, taken again at
`pos0 = 3, nt = 1`.

**Which path is wrong.** Unchanged: every oracle capture is batched, so the
batched path is the verified one and generation has never been diffed against
anything. The evidence says the two disagree and says where; it does not yet say
llama.cpp agrees with our batch at this length under `-b 1`.

### Where to look, concretely

Layer 3's compressor, comparing the **values** it produces at `pos0 = 0, nt = 4`
against `pos0 = 3, nt = 1`. The indices are equal by test; the candidates left
are what `compressor_project` puts in the ring and what the two shapes do with
it:

* the ring holds **projected** rows, `mul_mat(W, attn_norm)` — computed over four
  columns at once on one path and one column at a time on the other;
* `state_rows` front-padding with zeros when the ring is shorter than 8;
* the score half's `-inf` padding and the `soft_max` over it, where a padding
  row that is 0 instead of `-inf` would average a spurious entry in — and would
  do so *only* when the ring is involved.

`CHAOS_DUMP_LAYERS` now works on this path (it had no dump at all, which is why
the ring was invisible to all 43 layer tests) and prints `l_out-N` for the whole
tensor plus `l_last-N` for the final position, the one sum comparable across
batch shapes.