---
topic: C7 built and measured — the always-read trunk converted Q8_0 to Q4_K at load. What it buys, what it costs in accuracy, and the third effect nobody planned for.
status: CLOSED, NEGATIVE — fails two of the lossy bar's three checks: 40% byte-identical against 95%, and perplexity +3.674% against +1%. The infrastructure it needed survives; the lever does not.
links:
  - the-disk-is-no-longer-the-ceiling-2026-09-01.md
  - the-expert-cache-was-off-2026-09-01.md
  - what-is-inside-the-final-compute-2026-09-01.md
  - ../backlog/convert-the-trunk-during-the-load.md
  - ../backlog/the-big-bang-5-tok-s.md
---

# Requantising the trunk

C7 was the last untried lever on the 5 tok/s list, and the one thing every
V4-Flash document had said and none had costed: **the always-read trunk is stored
`Q8_0` while the routed experts are 4-bit.** The set that must stay in RAM
forever is stored at twice the width of the set that streams.

Its argument changed twice before it was built, and both changes were
measurements.

## What C7 was, and what it turned out to be

**First it was a kernel argument.** `v4flash-repacking-2026-08-10` recorded that
ggml has no repacked `Q8_0` kernel on x86 — 42 tensors offered, 42 declined — so
the theory was that the trunk was slow because its dtype had no fast path, and
C7's job was "move it to a dtype with a kernel". `trunk_mat_vec_dtypes` killed
that: at the trunk's own mat-vec shape, `Q8_0` is the *fastest* of `F32`, `BF16`
and `Q8_0`.

**Then it was a cache-cliff argument**, and that one survived. The expert cache
is sized from what residency leaves behind, and the measured cache curve has a
cliff at 4 GiB because 7.38 + 6 of 15.7 GiB leaves nothing for the page cache.
Halve the trunk and the same 6 GiB of cache leaves 5.8 GiB free.

**Extended here with the arithmetic, which is a third answer: it does not
matter.** Four runs in one session, same shape, `[4096, 2048]` against one token:

```
  dtype   ms per mat-vec (4 runs)          bytes/weight
  F32     0.6535  0.6145  0.6115  0.5875   4.00
  BF16    0.2592  0.2728  0.2831  0.2719   2.00
  Q8_0    0.2521  0.2359  0.2114  0.2214   1.06
  Q5_K    0.2130  0.2517  0.2277  0.2878   0.69
  Q4_K    0.2261  0.2146  0.2281  0.1963   0.56
```

`Q8_0` 0.211–0.252 against `Q4_K` 0.196–0.228: **the ranges overlap.** So the
conversion is not a compute win and none is claimed. Its whole case is the
memory it gives back — which makes C7 the first lever here whose mechanism is
*allocation* rather than arithmetic.

## What was built

`chaos_ggml::quantize` and `row_size` — this workspace could dequantise and had
no way back. `ggml_quantize_chunk` works in whole rows, which is exactly the unit
that lets 6.6 GiB be converted in bounded slices instead of held as `f32` first;
`output.weight` alone would be 2.1 GiB of floats. `IQ*` types are refused by
name: they need an importance matrix this build does not compute, and ggml would
otherwise produce bytes that are much worse than the type's reputation.

`chaos_arch::requantise` takes each resident tensor out of the set, converts it
in bands of rows across every core, and hands it back smaller. `ResidentSet` now
records a **dtype override** per tensor, because the set's contents no longer
match the container's index — and a binder that read `loc.ty` would create a
`Q8_0` tensor over `Q4_K` bytes, read twice the buffer, and return plausible
numbers. Both binders ask the set now.

`--trunk-quant off|q4_k|q5_k|q6_k`, refused by name on anything else, default
`off` because the change is lossy.

## What it does, on the real container

```
resident   loaded 1199 tensors, 7.38 GiB
trunk      511 tensors Q8_0 -> Q4_K, 6.63 -> 3.51 GiB, 3.12 GiB freed in 26.3s
repacked   383 tensors, 2.36 GiB in the CPU kernels' layout, 0.8s
cache      6.00 GiB chosen for you: 15.7 GiB of RAM, 4.26 resident
cache      34.0% hits (5155 of 15165)
```

**Resident 7.38 → 4.26 GiB. Cache 3.34 → 6.00 GiB. Hits 28.3% → 34.0%.** The
mechanism is exactly the one predicted, including the number the prediction
rested on: the 33.1% hit rate measured at a 6 GiB cache — which cost 0.352 tok/s
when it thrashed — is now affordable.

## The third effect, which nobody planned

**`repacked 383 tensors, 2.36 GiB`.** On x86 `Q8_0` has no repacked kernel, so
the trunk sat in its stored layout for every token of every run this project has
ever measured. `Q4_K` *does* have one, and ggml takes it: `q4_K_8x8`. So the
conversion silently enabled a second optimisation that had been unavailable since
the model was first loaded.

That is good news and a measurement problem: the shipped configuration changes
three things at once. Hence the attribution below.

## What it costs in accuracy

Weight-space, one tensor of each kind read straight from the container, 256 rows
each, `Q8_0 → f32 → Q4_K → f32`:

```
  blk.0.attn_q_a.weight        [4096, 1024]    7.14%   cosine 0.997453
  blk.0.attn_q_b.weight        [1024, 32768]   7.16%          0.997440
  blk.0.attn_kv.weight         [4096, 512]     7.13%          0.997457
  blk.0.attn_output_a.weight   [4096, 8192]    7.17%          0.997427
  blk.0.attn_output_b.weight   [8192, 4096]    7.13%          0.997460
  blk.0.ffn_gate_shexp.weight  [4096, 2048]    7.14%          0.997449
  blk.0.ffn_up_shexp.weight    [4096, 2048]    7.14%          0.997454
  blk.0.ffn_down_shexp.weight  [2048, 4096]    7.25%          0.997373
  blk.0.ffn_gate_inp.weight    [4096, 256]     7.25%          0.997373   (BF16, not converted)
  token_embd.weight            [4096, 129280]  7.12%          0.997466
  output.weight                [4096, 129280]  7.14%          0.997452   (not converted)
```

**7.1–7.25% everywhere.** The error is the dtype's, not any tensor's, and there
is no sensitive kind to protect specially — which is worth knowing because the
obvious next move would have been to hunt for one.

And the end-to-end check: bind both versions of a real trunk tensor into a ggml
context, multiply the same vector by each, compare. **Cosine 0.997439, relative
error 7.16%** — the weight error carries straight through to the output, as it
should for a mat-vec over 4096 terms with independent errors.

**One threshold was wrong before it was measured.** That test first asked for
cosine > 0.999 and failed at 0.9974, which looked like a bug and was not. The
bound is now set from a control — an unrelated vector scores 0.051 — so it
discriminates a mis-bound dtype from ordinary quantisation loss rather than from
an intuition about how close "close" should be.

## Two tensors are deliberately not converted

- **`output.weight`**, the projection to 129,280 logits. Its measured error is
  the same 7.14% as everything else; what differs is where the error *lands* —
  directly on the sampled token rather than averaged over a residual stream.
  llama.cpp's own `Q4_K_M` keeps this tensor at `Q6_K` for that reason. **This is
  a judgement, not a measurement**, and it is priced: 0.26 GiB of the 3.38 GiB
  available.
- **the router, `ffn_gate_inp`** — `BF16`, so the type filter skips it anyway. It
  is in the survey above because seeing its number is the argument: an error
  there changes *which experts are read*, and that is not a small perturbation of
  an answer but a different answer.

## What it costs in time

**26.3 s of CPU on every load**, on 20 threads, and the load itself is 3.4 s when
the trunk is in the page cache. So the feature makes starting the model roughly
eight times slower for a result that is a pure function of the container and the
target type. `../backlog/convert-the-trunk-during-the-load.md` has the fix — a
sidecar file — and it would make loading *faster* than the baseline rather than
slower.

## Speed: three rounds, because the first two were not trustworthy

**The answer is ~1.10x, and the first round said 1.29x.** The difference is
entirely protocol, and the sequence is worth recording because both flaws are
easy to repeat.

**Round 1** — three alternating pairs, back to back, the same protocol the
expert-cache measurement used:

```
  pair 1:  Q8_0 0.576    Q4_K 0.794
  pair 2:       0.611         0.791
  pair 3:       0.721         0.782
           median 0.611  median 0.791     1.29x
```

The candidate held a 1.5% band and the baseline climbed **25%** — because Firefox
had just been closed and Windows was still releasing 6.43 GiB, so the load budget
grew 7.97 → 9.24 → 9.42 GiB across the three runs. **Alternating pairs did not
save this; they only made the drift visible.**

**Round 2** — four arms, still back to back, and each arm always in the same
slot: D, E, B, A. The *shipped* arm came back slower than two arms it
structurally dominates, twice (B 0.697/0.681 against D 0.739/0.767 and E
0.741/0.784), while the arm that always ran fourth returned the tightest numbers
of the night (A 0.585/0.590). **Position and arm were confounded**, so nothing in
that round can be attributed.

**Round 3** — available RAM settled to ≥9.5 GiB before every run, and the order
rotated so each arm spends one iteration in each slot:

```
  arm                          runs                  median
  A  Q8_0, cache 3.34    0.645  0.709  0.720         0.709
  B  Q4_K, cache 6.00    0.778  0.772  0.786         0.778     1.10x over A
  D  Q4_K, cache 3.34    0.752  0.699  0.756         0.752     1.06x over A
```

`avail` before each run stayed inside 11.47–11.85 GiB, so the machine really was
in the same state each time.

**The claim is 1.10x on medians, and every B run beat every A run** — worst B
0.772 against best A 0.720. Pairing within iterations gives 1.07x to 1.22x, and
the tightest pairing, iteration 3, gives 1.09x. **The baseline was still trending
upward across round 3** (0.645 → 0.709 → 0.720) even with settling, so 1.10x is
if anything generous.

That is the smallest of the three levers this ladder has shipped — C5e was 1.12x
and the expert-cache default 1.20x, both *exact* — and it is the only lossy one.

## Attribution: the cache, not the kernel, and neither by much

D holds the cache at the baseline's 3.34 GiB, so D − A is the conversion without
the extra cache, and B − D is the cache. Round 3: **A 0.709 → D 0.752 → B
0.778**, so roughly 0.043 for the smaller resident set and 0.026 for the cache it
pays for. Both are inside the baseline's own 0.075 spread, so **the split is
indicative and not established.**

Repacking was isolated separately with `CHAOS_NO_REPACK=1` (round 2's E arm,
0.741/0.784 against B's 0.697/0.681 — E *faster*, which cannot be right and is
the position confound again). **So the repacked `q4_K_8x8` kernels are not shown
to help.** What is certain is only that the conversion makes them reachable,
which was never true before; whether they earn anything needs a rotated,
settled round of its own.

## The gate could not have judged this, and that was not about C7

Running C7 through the *lossy* bar found a hole in the bar itself.

**`--perplexity` never reached the streaming path.** The flag is parsed, plumbed
through `run`, and handed to `run_streaming` — and the deepseek4 dispatch returns
from `run` before that, exactly as `--auto` did. So on DeepSeek-V4-Flash the flag
was silently ignored, and the first attempt to use it did something else
entirely: it treated the corpus as a *prompt* and refused at the batch limit.

```
prompt     1818 tokens
chaos-run: 1818 tokens in one pass; this path takes 897 at a time.
```

**That is a third of the lossy bar missing on the one model the project is built
around** — and not for this lever only. C8 (2-bit experts) would have hit it, and
so would every later lossy change on this path. The bar would have been applied
as "two of its three parts", which is how a gate becomes a formality.

Two more things the same attempt turned up, both about the *method* rather than
the plumbing:

- **The engine says its own logits are approximate above ~2051 tokens.** The
  lightning indexer is not implemented, and skipping it is only a no-op while
  `n_tokens / 4` stays inside `indexer_top_k` — so a single 1818-token pass
  printed *"These logits are APPROXIMATE"*. A perplexity comparison between two
  builds that are equally approximate would still detect a rise, but a
  distribution check on an approximated distribution is the sort of claim this
  repository keeps withdrawing.
- **Chunking removes that problem for free.** Every chunk starts from an empty
  cache, so a chunk is at most `PPL_CHUNK` tokens of context and the logits are
  exact.

So the streaming path now scores a corpus, with the dense path's windowing —
whole chunks, empty cache each, only positions from `len / 2 + 1` scored, which is
llama.cpp's `n_ctx - 1 - n_ctx/2`. Tokens go in one at a time for the same reason
as on the dense path: the head projects only the final position through
`output.weight`, so per-position logits arrive a step at a time. On this model
that is ~1.4 s a scored token, which makes the corpus length a **time budget**
rather than an accuracy one — the measurement is deterministic and both sides
score the same tokens. The run used five chunks of 128, 315 scored tokens, about
fifteen minutes a side.

**A short corpus risks being unrepresentative, not imprecise** — and this one was
worse than unrepresentative, which is the caveat above. `scripts/ppl-corpus.txt`
replaced it: the repository's own prose, ~2000 tokens, every line unique, and
validated against llama.cpp at **-1.44%** on a model the two engines agree on.

## The verdict: it fails the lossy bar, decisively

```
checkable answers correct: 41 of 50
byte-identical answers:    20 of 50  (40.0%)
checkables in baseline:    41
bar: lossy -- >=95% identical, no checkable regression, perplexity +<=1.0%
FAIL: 40.0% identical, below 95%.
GATE FAILED. The change does not ship, and no tok/s number from it may be quoted.
```

And the distribution check, run separately because the gate could not run it
until this session built the path for it:

```
baseline  Q8_0 trunk : 7.7464
candidate Q4_K trunk : 8.0310
rise 3.674%   (the lossy bar allows +1.0%)
```

**So it fails two of the three checks, and passes only the third.** Agreement
40.0% against 95%; perplexity +3.674% against +1%; checkables 41 of 50 against
the baseline's 41.

**The perplexity figure carries a caveat found hours later, and it is recorded
rather than quietly dropped.** It was taken on a corpus cut from
`long-prompt.txt` — 41 unique lines repeated to 80 — which later turned out to be
**degenerate for this purpose**: given 257 tokens of it both engines fall to a
perplexity near 1.0. It was also taken before Chaos started putting BOS at the
start of every chunk the way llama.cpp does.

Neither defect touches the comparison's *validity as a self-comparison* — both
sides were the same binary scoring the same tokens with the same windowing, so
the rise between them is real. What is uncertain is its **size**: on text a model
can pattern-match almost perfectly, a small perturbation can cost proportionally
more than it would on natural prose, so +3.674% may be overstated.

**The verdict does not rest on it.** The agreement check is corpus-independent —
50 fixed prompts, 20 identical — and 40.0% against 95% is a refusal on its own.
Re-measuring the distribution check on `scripts/ppl-corpus.txt` would sharpen the
record and cannot change the outcome, so it is filed rather than run tonight.

**40% against a 95% bar is not a near miss.** The bar was agreed before any run —
that is what §5 of `the-big-bang-5-tok-s` asked for and what C6 was built to
provide — and the gate's own header forbids widening a threshold in the change it
is blocking. So the speed figures above stand as *evidence about the mechanism*
and **not as a result to quote**. Anyone reaching for the 1.10x should stop here.

**In hindsight the number was predictable and nobody predicted it.** The
per-tensor survey said 7.1–7.25% weight error, the mat-vec check said that error
carries straight through, and 43 blocks of it will move a greedy argmax often.
The 7% was measured *before* the gate ran and read as reassuring, because the
cosine was 0.9974 and that looks like agreement. It is not: cosine near one and
top-1 agreement are different questions, and only the second one is the bar.

**One result inside the failure is worth keeping.** Checkables came back **41 of
50, exactly the baseline's 41** — not one factual answer was lost. So this change
alters how the model words things in 60% of cases and what it knows in none of
them. That is a real property of a K-quant trunk and it is *not* a defence of the
lever: the agreed bar is agreement, and a change that rewrites three answers in
five is not the same model.

## What this closes and what survives

**C7 is closed, negative, with the numbers** — the fourth lever on the 5 tok/s
list to die to a measurement rather than to an argument, after expert
factorisation, contextual sparsity and a pinned hot set.

`--trunk-quant` stays in the tree, **off by default and documented as failing the
bar**, on the same footing as `--op-offload` (measured 19% slower here, shipped
off) — a measured-bad option that is honest about being measured bad is more use
than a deleted branch nobody can re-check.

What survives on its own merits, none of it changing what the model computes:

- **`chaos_ggml::quantize` and `row_size`** — the workspace could dequantise and
  had no way back. The dtype instrument needed them to answer C7's original
  question at all.
- **Perplexity on the streaming path** — a third of the *lossy* bar was
  unmeasurable on the flagship model, for every lossy lever and not just this
  one. That hole is closed whatever happens to C7.
- **`--flags` in the gate** — without it the gate could only ever have measured
  the default, so no flag-shaped lever could have been judged.
- **The `ResidentSet` dtype override**, which is what
  `../backlog/convert-the-trunk-during-the-load.md` would need if the *memory*
  case is ever pursued for a machine where the trunk does not fit at all. That
  case is untouched by this failure: it is about fitting, not about speed.

## What would have to change for a trunk conversion to pass

Not a wider bar. Either a narrower conversion — `q5_k` or `q6_k`, which the flag
already accepts and which the survey can price in ten seconds — or a mixed policy
that leaves the attention path alone. **Neither is worth building on a guess**:
the cheap next step is to run the same gate at `q6_k` and see whether agreement
comes back, and that is one command, not a design.

## One sentence written before the gate ran, kept for the record

> *"7% weight error is not obviously survivable, and the honest position is that
> nobody here knows yet. Real `Q4_K_M` containers carry the same error and are
> what most people run, which is a reason to expect it to pass rather than an
> argument that it will."*

That was the state of knowledge an hour before the answer arrived, and it is left
here rather than tidied away: the reasoning was sound, the caveat was the right
caveat, and the conclusion still went the other way. **A real `Q4_K_M` container
is quantised once, from full precision, with an importance matrix; this converts
an already-`Q8_0` trunk without one, and it does it to the attention path of
every one of 43 blocks.** Those are not the same thing, and the resemblance is
what made the guess feel safer than it was.
