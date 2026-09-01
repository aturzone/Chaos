# STATUS — where Chaos is, and what is left

**Read this first, in any session.** It is the single place that says what is true
today. Update it in the same commit as any change that moves a number or closes a
task. If it disagrees with a graph node, **this file is wrong and the node is right**
— fix this file.

**Last updated**: 2026-08-31 · **Version**: v0.0.23, tagged 2026-08-28 ·
**Branch**: `ticket/update-from-inside` open as PR #150; `main` verified at v0.0.23.

> **This file was 5,144 lines and 104 dated sections until 2026-08-31.** It called
> itself the single source of truth while being a reverse-chronological diary in
> which the current truth was the first 60 lines. An audit found **22 contradictions**
> between it and the three other scoreboards, several of them this file disagreeing
> with itself. The history is kept verbatim in
> `docs/graph/history/status-archive-2026-08.md`. **It is history — do not cite it as
> current.**

## The four scoreboards are now two

| file | job |
|---|---|
| **`STATUS.md`** (this file) | what is true today, and what is left |
| **`CHECKLIST.md`** | the tick-list: one line per item, done or not |
| `CLAUDE.md` | how to work here — instructions, traps, layout |
| `docs/graph/` | the reasoning, one node per question |

`backlog/the-plan-v0-1-0.md` is superseded by the release ladder below.

---

## The release ladder: v0.0.24 → v0.0.30

**Decided with Atur, 2026-08-31.** v0.0.30 is the release built to LTS standard.
**Nothing is tagged until its gate is green** — 23 releases went out in 21 days and
none of them got a stabilisation period, which is the whole complaint.

```
v0.0.24  One truth                    [####################] 100%  merged #151
v0.0.25  Guard the binary             [####################] 100%  merged #152
v0.0.26  Measure before optimising    [####################] 100%  merged #153-155
v0.0.27  Quality harness, then levers [########------------]  40%  <- harness in
v0.0.28  Any machine, any model       [###########---------]  55%
v0.0.29  Every platform, actually run [#######-------------]  35%
v0.0.30  LTS                          [#-------------------]   6%
```

Each release's contents and its gate are in the plan; the short form:

- **v0.0.24 — One truth.** Collapse the scoreboards, resolve all 22 contradictions,
  retire the five dead wrapper-era epics, fix the live factual errors. *Gate*: zero
  contradictions between any two documents; every number machine-checked or dated.
- **v0.0.25 — Guard the binary.** A CI job that runs the correctness suite against a
  real model; `chaos-run` beyond 8 tests; port bound before the model loads;
  `finish_reason` surfaced; the window exercised in CI.
- **v0.0.26 — Measure before optimising. Done.** `F` profiled (93% arithmetic, and the
  **router is 31% of it**), the expert read re-measured at **2.88 GiB/s** — 84% of the
  drive, so the I/O gap was nearly closed already — and the 3x GPU contradiction
  resolved. **Three claims were corrected, two of them mine.** The renaming is
  deliberate: every item on this rung turned out to be a measurement, not a change.
- **v0.0.27 — The quality harness, then the levers.** `scripts/quality-gate.sh` is in,
  with **different bars per lever** (Atur's call): *exact* changes must be 100%
  byte-identical, *lossy* ones need ≥95% plus no checkable regression plus perplexity
  within 1%. Verified against 1 MiB of zeros — all three checks fired. Next is the
  router (C5c), which goes behind the *exact* bar.
- **v0.0.28 — Any machine, any model.** Quant selection, self-configuration, and a
  prediction that holds across the range. **This is the claim that is actually ours.**
- **v0.0.29 — Every platform, actually run.** Four of nine published assets have never
  been executed by anyone.
- **v0.0.30 — LTS.** The parity gate, the competitive claim with command lines, a
  written support policy.

---

## The honest scoreboard

**Current**: **985 tests** (0 failed, 42 ignored — the V4-Flash set needs the
container and the autoencoder set needs the 336 MB `flux2-vae`), clippy
`--workspace --all-targets -D warnings` clean, fmt clean.

| | Chaos | llama.cpp | note |
|---|---|---|---|
| CLI long flags | **165** implemented, 17 declined, 0 unrecognised | 182 | counted from both binaries, test-enforced |
| Chat templates | **52** verified byte-identical | 54 | the two missing are Hunyuan variants |
| Tokenizer families | **5** (BPE, SPM, WordPiece, Unigram, RWKV) | 6 | RWKV implemented, not verified |
| Architectures diffed at 8 prompts | **14** | 141 declared | this is what `VERIFIED_ARCHITECTURES` means |
| GPU backends | **1**, Vulkan, **not verified** | 5 | fails 1 of 8 parity prompts |
| Binaries shipped | **21**, every one on every platform | — | test-enforced both directions |

**Measured speed**, this laptop (i7-13650HX, 15.7 GiB, RTX 3050 6 GB, SK Hynix NVMe):

| | |
|---|---|
| Qwen2-0.5B / Falcon3-1B generation | **28** / **21 tok/s** (medians of three) |
| DeepSeek-V4-Flash, 144 GB, in 15.7 GiB | **0.727 tok/s** — 0.509 before C5e, 0.570 after it, **0.727** once the expert cache stopped defaulting to zero. Three alternating passes each, one session. The **0.43** on record until 2026-08-31 was taken under memory pressure |
| V4-Flash against llama.cpp | **no ratio publishable.** Chaos **0.696** ±1%; llama.cpp **0.16–0.47** across 8 runs of one command line, drifting down within every sequence even with Chaos never started. The cell is `unmeasurable here` — `the-v4flash-parity-cell-does-not-reproduce-2026-09-01.md` |
| Dense path, hand-tuned | **1.30× behind**, and §4a showed the gap is 61% FFN matmul |
| Dense path, defaults, **same session** | Qwen3-4B short **8.39 vs 7.69 — 1.09× ahead**; at 4031 tokens **3.13 vs 4.53 — 1.45× behind**. **The ratio flips**: Chaos slows 2.68× with context, llama.cpp 1.70× |
| RAM read, peak / achieved in generation | **30.8** / ~19 GiB/s |
| NVMe sequential / expert reads / QD-8 | **3.09** / **2.88** / **3.41 GiB/s** — the expert figure re-measured 2026-08-31 with the trunk resident; the old 1.40/1.88/2.02 were all taken while the trunk was *also* streaming |
| `tok/s ≈ 19 / resident GiB` | holds across nine models spanning 23× |

**Retracted, do not requote**: *"Proven: Qwen3-30B-A3B"* (`qwen3moe` is not in
`VERIFIED_ARCHITECTURES` and needs `--force`); *"runs models larger than RAM and
llama.cpp cannot"* (it can, with `--no-repack`); *"V4-Flash prefill 1.62× behind,
generation 3-4× behind"*; *"generation ~2× behind on Qwen3-30B"*. Do not replace them
with a claimed lead either — the ranges overlap.

---

## What is open, in the order it is worth taking

**Nothing below is blocking a merge. All of it is blocking v0.0.30.**

1. ~~**No CI job runs the correctness suite against a real model.**~~ **Closed
   2026-08-31.** CI now fetches a 397 MB Qwen2-0.5B, verifies it is really a GGUF, and
   runs four layers with `CHAOS_REQUIRE_MODEL_TESTS=1` so a missing model fails rather
   than skips. The layers' sensitivity is **measured**: 4 KiB of zeros in the container
   changes nothing observable, **1 MiB moves the byte-exact golden but not the substring
   tripwire**, 16 MiB moves both — which is why there are two output checks and not one.
   The byte-exact golden exists for `x86_64-windows` only; the other platforms run the
   remaining three layers and say so rather than inventing a pass.
2. ~~**`F = 0.84 s` has never been profiled.**~~ **Profiled 2026-08-31**, trunk resident,
   three runs in one session: **0.494 / 0.510 / 0.511 tok/s**. A token is **60% disk** and
   **40% F**, and **F is 93% arithmetic** — `compute` 0.44 s, **the router 0.22 s**, graph
   construction only 0.05 s. (A first pass claimed the reverse by trusting a code comment
   instead of reading `moe_routing`, which runs `ctx.compute` in its middle; corrected the
   same day.) **The router is the target**: 5.5 ms in each of the 40 `argsort_top_k`
   blocks to pick 6 of 256 floats, against **0.000 s** in the 3 hash layers — so it is
   graph *dispatch*, paid 40 times a token. Two fixes were filed for it and
   **both are dead, each killed by a measurement before it was built**: a CPU top-k
   (the sort costs ~0) and converting the BF16 gate weight (**F32 0.1503 ms vs BF16
   0.1501 ms — 1.00x**, and 43x faster than the engine pays). **The real cause is
   that the block tail is computed twice**: `ctx.compute(&topk)` reaches back
   through `ffn_norm` into `layer_tail`, and the final `compute` redoes it — the
   argsort blocks' final compute is 0.0101 s against the hash blocks' 0.0100 s, so
   the early evaluation was extra, not early. **C5e is now shipped**: the computed
   tensors are copied into leaves, `compute_many` puts `post` and `comb` in the same
   compute rather than adding work, and the three hash layers are excluded because
   nothing of their tail exists yet. **0.509 -> 0.570 tok/s, 1.120x**, three pairs
   alternating with one binary (`CHAOS_NO_FREEZE=1` turns it off), against a
   1.13x prediction — and **50 of 50 answers byte-identical, the *exact* bar met**,
   against a baseline re-recorded with `CHAOS_NO_FREEZE=1` so both sides are one binary
   in one session. **The first attempt reported 0 of 50 and the harness was the cause**:
   it kept the `generate ... tok/s` timing line in every answer, so it could not have
   passed a build against itself. Fixed, and now proved in both directions on
   Qwen2-0.5B (50/50 against itself; 35 changed and 2 checkables lost against 1 MiB of
   zeros) -- the same "tested in one direction only" bug as `is_contiguous`, the same
   afternoon.
   The mechanism is proved inside one run using the hash layers as an untouched
   control: argsort blocks now pay route 0.0074 + compute 0.0086 against hash's
   0.0000 + 0.0159, **equal sums though an argsort block does strictly more work**.
   Writing its test found a second bug: **`is_contiguous` was off by one dimension**
   and answered `false` for every tensor with more than one row, so every
   `to_vec_f32` in the engine walked strides where a `memcpy` would do.
   `the-tail-computed-once-2026-08-31.md`.
3. **The GPU tier is not verified** — the device path fails 1 of 8 parity prompts where
   the CPU path fails none. ~~And the GPU evidence contradicts itself.~~ **Reconciled
   2026-08-31**: both measurements are right and they used different context lengths.
   Short prompt, this card is **1.5–2.0× faster** than the CPU; at ~1050 tokens it is
   **0.93×, slower**. So *"Vulkan generation is 2.2× slower"* is a long-context
   statement and is wrong without that clause. It looked like the explanation for
   `--auto` picking wrong at long `-n`, **and that was tested and did not reproduce**:
   `--auto` is ahead at both `-n 16` and `-n 200` with a short prompt. What is still
   open on the GPU is **parity**, not speed.
4. **Four of nine published assets have never been run by anyone**: the `.deb`, the
   AppImage and both macOS tarballs. No model has ever been run on macOS or Linux, and
   macOS has no direct-I/O path (`F_NOCACHE` unwired).
5. **No real camera has seen the mark or the reader.** Everything else in that chain is
   now proven — `scripts/scan-sweep.js` gets **210 of 210 at 3–12 px per module across
   30 angles with zero wrong strings**, feeding the shipped detector real pixels. What
   is left is photons.
6. **The APK has never run on a phone**, only an emulator; `chaos-android` has 1 test.
   The SDK cannot be installed here (`dl.google.com` 404s this network), so CI is the
   only build.
7. **`chaos-run` has 8 tests** and is the binary most people type.
8. **The port is bound after the model loads** — a 144 GB model reads for minutes and
   then fails on a taken port, with the raw OS string naming neither the port nor
   `chaos stop`. `chaos-serve.rs:207-209` already states the principle it violates.
9. **`finish_reason` is not surfaced** to `chaos connect` — long answers stop mid-word
   with no explanation and no flag to raise the cap.
10. Smaller and honest: `chaos scan` is declared NOT BUILT; zsh and fish completions are
    generated but never sourced; no contrast audit or screen-reader story for the
    window; full-disk behaviour deliberately unmeasured; `strip` has never been run and
    a served page re-read.

**Parked by Atur**: iOS, until everything else is good.

---

## Three habits, each bought with a wrong finding

- **`IsWindowVisible` is not "on screen"** — `layout` parks unreachable rail buttons at
  `(-3200,-3200)`. Read client-rects.
- **Cross-process `GetWindowTextW` reads a caption, never an EDIT's text.** Use
  `WM_GETTEXT`. This produced an entire false "the CHAOS page is blank" finding.
- **When a crude recount disagrees with a number whose source says it was computed,
  suspect the recount.** Three were wrong that way in one day.

And two that cost a release each:

- **A green transcript is not a working window.** `run-through.ps1` reported a clean
  pass over the app that shipped as v0.0.21 with nine controls floating over the mode
  knob, because it drove pages by `WM_COMMAND` and never left the launch screen.
- **A sentence is not a mechanism.** The README's architecture count was corrected in
  prose and left wrong in the progress bar nine lines above, and CI stayed green for
  three days. The count is now guarded where it is *written*, not where it was
  convenient to guard.

**CI logs cannot be read from this machine** — the logs endpoint redirects to an Azure
blob host that does not resolve here. Reproduce a CI failure locally from the
workflow's own commands; that found v0.0.22's release failure in one try.

---

## Dead ends, measured. Do not re-propose.

Expert factorisation (1.0×, and rank-512 holds 20.4% of energy against 16.6% for
matched noise), contextual sparsity (9.1% of neurons negligible, against 80–95% in the
literature), a pinned hot set (37.5% out-of-sample against 25% random), expert
read/compute overlap (1.03×), `--op-offload` (19% slower), `mul_mat_id` batching on the
streaming path, porting parallel-experts to V4-Flash (its whole routed arithmetic is
under 5% of a token), and dropping the router's tail (the 6th expert still carries 8.8%
of the mass against a uniform 16.7%).

**Two premises that were wrong, found 2026-08-31 by reading the code**:
concurrent expert reads were said to be unimplemented — they are shipped, 8 threads
with 8 file handles (`deepseek4_forward.rs:2021-2059`) — and the ladder's rung-0 row
double-uses the number 2.40 for two different quantities and over-predicts today's
token by 35%. `docs/graph/backlog/the-big-bang-5-tok-s.md` carries the correction.

## The night of 2026-08-31 to 2026-09-01, in one place

**Seven pull requests, #160–#166.** Two changed the engine and both are *exact*,
through the quality gate's 100%-byte-identical bar; five are measurements, three of
which retracted something.

| | |
|---|---|
| **C5e** — the block tail was computed twice | **1.120x**, 50/50 byte-identical |
| **The expert cache defaulted to zero** | **1.20x**, 50/50 byte-identical |
| **together** | **0.509 → 0.728 tok/s** |

**Four bugs found by writing a test or a harness, not by reading code**:
`Tensor::is_contiguous` was off by one dimension and answered `false` for every
tensor with more than one row; `quality-gate.sh` kept a timing line in every
recorded answer and so **could not have passed a build against itself**;
`speed-five.sh` read a CRLF file and reported five dashes while `chaos-run` was
printing the reason; and a settling gate written to prevent a page-fault storm
**failed open** because it used `bc`, which Git Bash does not ship.

**What was retracted, all of it mine and all within hours**: *"the routed expert
matmuls are 0.004 s"* (they are 40%), *"88% of `F` is the hyper-connection
algebra"* (it is 8%), and *"parity on everything that streams"* (the cell is
unmeasurable here). Each came from attributing a phase timer to a subsystem
without reading what falls between the two `Instant`s.

## What moved on 2026-09-01, and what did not

```
  before                            0.509 tok/s
  C5e, the tail computed once       0.570        1.12x   exact, 50/50
  + the expert cache defaulted      0.727        1.28x   exact, gated
                                                 1.43x together
```

**Both were accidents rather than missing features.** C5e was a value derived twice
because ggml has no notion of "already computed"; the cache was `None => 0` with no
sizing anywhere, on a model where `--auto` turns out never to run at all.

**The ceiling has not moved.** 137 GiB of experts still cross a 3.41 GiB/s drive
every token, and one-bit experts with a free trunk still cap this machine at 4.26
tok/s. See below.

## The 5 tok/s position

**137 GiB of experts cannot live in 15.7 GiB of RAM, so they cross the disk every
token.** At the drive's QD-8 rate of 3.41 GiB/s:

| experts | per token | disk time | ceiling |
|---|---|---|---|
| today, Q4_K | 3.22 GiB | 0.94 s | **1.06 tok/s** |
| at 2-bit | 1.61 GiB | 0.47 s | **2.13 tok/s** |
| at 1-bit | 0.80 GiB | 0.24 s | **4.26 tok/s** |

**Even one-bit experts, with a free trunk and zero arithmetic, do not reach 5 tok/s on
this drive.** That needs no assumption about the trunk and no estimate of F.

### But the disk was the wrong bound, and 4.26 was the wrong ceiling

**Corrected 2026-09-01** (`the-disk-is-no-longer-the-ceiling-2026-09-01.md`). Every
row above assumes each expert byte comes off the disk. The expert cache now serves
**28.3%** of them, so the ceiling has to be re-derived — and the constraint changes
hands:

| bound | tok/s |
|---|---|
| disk, one-bit experts, zero arithmetic | 4.26 |
| **arithmetic, all disk traffic removed** | **1.5–1.8** |
| measured today | 0.728 |

The generation-only profile, three runs, prefill excluded: **disk 0.815 s,
arithmetic 0.478 s** per token. The disk term cross-checks a second way — 71.7% of
3.22 GiB in 0.815 s is **2.82 GiB/s** against the 2.88 measured directly.

**So 5 tok/s wants 0.2 s a token and the arithmetic alone is 0.48–0.65 s.** Removing
the disk entirely is worth about **2.5x from here and no more**; after that `F` has
to fall roughly **3x**.

### And `F` is now split, which closes the argument

`what-is-inside-the-final-compute-2026-09-01.md`. Two probes, each computing one
subgraph alone and freezing it, cross-checked against the unsplit total (0.484–0.494
against 0.472–0.489, so under 3% of instrument effect):

| per generated token | seconds | share of `F` |
|---|---|---|
| attention | 0.197 | **40%** |
| expert matmuls (6 routed + 1 shared) | 0.199 | **40%** |
| hyper-connections + router | 0.040 | 8% |
| qkv construction | 0.043 | 9% |
| residual write-back | 0.003 | 0.6% |

**Two earlier guesses were wrong in opposite directions and both are retracted**:
*"88% is the hyper-connection algebra"* (it is 8%) and *"the routed expert matmuls
are 0.004 s"* (they are 40%). Both came from attributing a phase timer to a
subsystem without reading what falls between the two `Instant`s — `attn_out` feeds
`layer_tail`, which feeds `ffn_norm`, so **the whole attention graph is evaluated
inside the phase called `tail`.**

**`F` is work, not waste.** Attention is ~22 M multiply-accumulates a block, which
lands near 3.1 ms at this machine's ~14 GFLOP/s mat-vec rate against 4.6 ms
measured; 21 `[4096, 2048]` products a block at 0.219 ms each is also 4.6 ms. No
factor of ten is hiding in either. So **5 tok/s here would need the arithmetic
2.4x faster than the memory system allows, with the disk switched off entirely** —
a hardware statement, and the first to rest on a measured decomposition of `F`.

The dtype is not the problem either: `trunk_mat_vec_dtypes` measures **F32 0.609 ms,
BF16 0.296, Q8_0 0.219**, so Q8_0 is the *fastest* of the three and C7's *"move the
trunk to a dtype that has a kernel"* argument is dead. What survives of C7 is the
cache cliff.

**Best honest case remains roughly 1.5–2 tok/s on this machine, and that is not 5.**
It will keep being said plainly rather than missed quietly.
