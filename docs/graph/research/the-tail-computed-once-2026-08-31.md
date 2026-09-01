---
topic: C5e shipped — the block tail is computed once now, 1.12x measured on V4-Flash, and testing the helper found a second bug in `is_contiguous`
status: resolved
links:
  - the-router-is-a-bf16-matmul-2026-08-31.md
  - what-f-is-made-of-2026-08-31.md
  - ../reference/hard-won-facts.md
---

# The tail is computed once

`the-router-is-a-bf16-matmul` ends by filing **C5e**: `ctx.compute(&topk)` inside
`moe_routing` evaluates everything `topk` depends on, which reaches back through
`ffn_norm` into the whole of `layer_tail`, and the block's own `compute` at the
end then derives all of it **a second time**. Copy the values into leaf tensors
and the second derivation stops.

It is built, measured and gated: **1.12x on V4-Flash generation, exact, 50 of 50
answers byte-identical.** Running the gate also found a defect in the gate -- see
"the gate could not pass" below.

## The numbers

**Alternating, one binary, one session** — `CHAOS_NO_FREEZE=1` turns C5e off, which
exists precisely so this measurement does not have to compare against a number
taken half an hour earlier under a different machine state:

```
pair 1:  off 0.509 tok/s   on 0.570 tok/s
pair 2:  off 0.509         on 0.570
pair 3:  off 0.511         on 0.566
         median 0.509      median 0.570      1.120x
```

Per token: **1.965 s → 1.754 s, 0.211 s saved.** The prediction was 0.221 s and
1.13x, from `router 0.221 s of a 1.980 s token`. Prefill moved too — 5 tokens in
6.5 s → 5.8 s — because a five-token pass is under the batch bound and is frozen
like a generation step.

## The mechanism, proved within one run

The three hash layers are the control group: their `topk` is
`get_rows(ffn_gate_tid2eid, tok)`, it depends on the token ids alone, and **C5e
does not touch them.** So in one run, at three decimals, averaged over six
generated tokens:

| per block | hash (0–2), unchanged | argsort (3–42), frozen |
|---|---|---|
| `route-compute` | 0.0000 | **0.0074** |
| final `compute` | **0.0159** | **0.0086** |
| sum | 0.0159 | 0.0160 |

**The two sums are now equal, and that is the whole point.** An argsort block does
strictly more work than a hash block — the same tail, plus a 4096×256 gate matmul
and a top-k — yet its final compute is **0.0073 s smaller**. The tail has moved
out of it. Before the change the same table read 0.0055 + 0.0101 for argsort
against 0.0000 + 0.0100 for hash: an excess of 0.0056 s per block, 40 blocks, no
benefit.

**Do not compare the hash column across the two sessions** (0.0100 then, 0.0159
now, for code that did not change). That is the machine, not the fix, and it is
why the argument above is made entirely within one run.

## The shape of the change, and the two things that nearly broke it

`freeze(ctx, t)` allocates a leaf of the same extents and copies `t.to_vec_f32()`
into it. Four tensors per block plus `probs3` and `topk`: about 83 KiB at one
token.

**1. `post` and `comb` are not ancestors of `topk`.** `ffn_norm` reaches `mixes`
through `gates.pre` only, so `ctx.compute(&topk)` never evaluated `post` or
`comb`. Freezing them from that compute would have copied **uninitialised arena**;
freezing only `streams` and `ffn_norm` would have bought nothing, because
`post`/`comb` still pull the final graph back through `mixes` to `streams` and
therefore through `dsv4_hc_post`. The answer is `ctx.compute_many(&[&topk,
&gates.post, &gates.comb])` — the same work, moved rather than added, and then all
six can be frozen. `compute_many` already existed and had no caller.

**2. The three hash layers must be excluded.** Nothing of their tail is computed
at that point, so freezing there copies uninitialised memory and the result is
fluent nonsense, not a crash. One named predicate, `freeze_the_tail`, three call
sites, and `the_hash_layers_are_never_frozen` pins it.

A batch bound is in it as well: the copies are linear in `nt` (16 MiB at 192
tokens) against a fixed 1 GiB block arena, and **an exhausted ggml arena aborts
with no message** — while the saving is per *pass* and so divided by `nt`. Above
192 tokens the optimisation declines itself, which is exactly where it is worth
least.

## And a second bug, found by testing the helper

`freezing_a_strided_view_copies_the_view` asserted the leaf was contiguous. It was
not — and the leaf was a freshly allocated ggml tensor, which cannot be anything
else.

`Tensor::is_contiguous` accumulated the expected stride **one dimension late**:

```rust
let mut expect = nb[0];
for d in 1..4 {
    if ne[d] > 1 && nb[d] != expect { return false; }   // compares nb[1] with nb[0]
    expect *= ne[d].max(1) as usize;                     // should be ne[d - 1]
}
```

So it returned `false` for **every tensor with more than one row**. Nothing broke,
because its only two callers are `to_vec_f32` and `to_vec_i32` and a `false` sent
them down the stride-walking path, which is correct for a contiguous tensor too —
just element by element, four multiplies and an unaligned read each, where a
`memcpy` would do. Every block boundary of every architecture in the engine paid
that, invisibly.

**It survived because all three tests asserted a view is *not* contiguous**, which
a function that always returns `false` answers correctly. The missing direction is
now `a_fresh_tensor_is_contiguous_at_every_rank`.

Quantised tensors still report `false`, deliberately: `nb[0]` is a block's bytes
there rather than an element's, both callers read `f32`/`i32` only, and a clever
`true` would be a wrong `memcpy`.

## The gate could not pass

**The first gate run reported 0 of 50 byte-identical, and the change was not the
reason.** Two facts arrived together and did not fit: 0 of 50 identical, and
*exactly* the same 39 of 50 checkable answers correct as the baseline. Changed
arithmetic does not usually leave the score untouched.

With stderr discarded, `chaos-run` prints two things on stdout: the completion
behind an `output` marker, and

```
generate   11 tokens in 21.7s (0.507 tok/s, 2.0s per token)
```

**a timing line, which differs on every run of any build.** The harness kept it in
every recorded answer. So the gate could not have passed a comparison of a build
against *itself*, and C5e — whose text was in fact identical, as the very first
manual run had already shown — failed it 50 times out of 50.

A second defect sat beside it: `gen=${answer#*"$prompt"}` was meant to strip an
echoed prompt, and `chaos-run` never echoes one. It therefore did nothing at all —
except on the rows where the completion happened to quote the prompt back, where it
silently discarded the front of the answer. That is why the recorded baseline's
first row read ` Paris.", generate 11 tokens in 21.7s (...)`.

**And that one understated the model.** Because the front of an answer could be cut
away, so could the word being looked for: the broken harness scored V4-Flash at
**39 of 50** checkable answers and the fixed one scores the same build at **41 of
50**. A harness defect that moves a quality score *down* is the one nobody goes
looking for.

### Why it was not caught when the harness was written

**It was validated in one direction only.** `quality-gate.sh` was proved against
1 MiB of zeros written into a container: 22.0% identical, four checkables lost,
perplexity +1.16%. All three fired — and all three fire the same way whether the
harness is right or broken, because *a failure looks like a failure*. Nobody ran
the harness against an unchanged build and asked for 100%.

**This is the same shape of bug as `is_contiguous` above, found the same
afternoon**, and the same lesson twice: a check tested only where it should say
"no" is a check that may only ever say "no".

Both directions are now proved, on Qwen2-0.5B because it costs ninety seconds
rather than half an hour:

| | result |
|---|---|
| the same build against itself | **50 of 50, 100% — PASSED** |
| 1 MiB of zeros at offset 200 MB | **35 of 50 changed, 2 checkables lost — FAILED** |

The two lost checkables are worth quoting, because they are what damage reads
like: *"the colour of most healthy plant leaves"* answered with chlorophyll prose
instead of green, and *"the liquid that covers about 71% of Earth's surface"*
answered **"is called the atmosphere."**

### And the V4-Flash baseline had to be re-recorded

Not to make a comparison pass — the recorded strings were **not the model's
answers**, so there was nothing there to compare against. It was re-recorded with
`CHAOS_NO_FREEZE=1`: the same binary, the same session, C5e switched off. That is a
strictly better control than the original, which came from a separate build half an
hour earlier.

## What is left of `F`

`what-f-is-made-of` put `F` at 0.71 s of a 1.98 s token, 93% arithmetic, of which
the router — really the tail computed twice — was 0.221 s. That is now spent. The
remaining `F` is the arithmetic a token genuinely needs, and the token is
correspondingly more disk-bound than before: **1.754 s, of which the expert read
is still the largest single term.**

The 5 tok/s ceiling does not move. 137 GiB of experts cross a 3.41 GiB/s drive
every token, and `the-big-bang-5-tok-s.md` records that even one-bit experts with
a free trunk cap this machine at 4.26 tok/s. What C5e changes is that the engine
is no longer wasting 11% of a token on work it had already done.
