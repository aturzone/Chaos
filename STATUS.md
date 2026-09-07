# STATUS — where Chaos is, and what is left

**Read this first, in any session.** It is the single place that says what is true
today. Update it in the same commit as any change that moves a number or closes a
task. If it disagrees with a graph node, **this file is wrong and the node is right**
— fix this file.

**Last updated**: 2026-09-08 · **Version**: v0.0.34, tagged 2026-09-08 ·
**Branch**: `main`, verified at v0.0.34 — 1018 tests, 0 failed, fmt and clippy
clean, re-run on `main` itself after the merge.

**v0.0.34 is three platforms, one mode, one book.** Atur, 2026-09-07: *"delete
Android and so on, keep only Windows, Linux and macOS... that mode selection
isn't needed any more... that book and barcode aren't needed any more, we only
use the book... the CHAOS page, its button isn't even in the menu any more"*.

- **The phone tier is gone** — tree, release job, APK asset, Kotlin tests, JNI
  crate, signing-key work and three research nodes.
- **The launch knob is gone**, 87 references across five files. The window opens
  on CHAT, `RAIL_PAGES == PAGES`, and **CHAOS has a rail entry and an
  accelerator** for the first time. The role is a dropdown at the top of that
  page, above the address it decides.
- **The reader is gone** — `scanner.html`, `/scan`, `Page::Scry`, the READ A
  CODE button and the sweep that drove its detector. The mark stays.
- The README carries download buttons and setup for the three platforms plus a
  Claude Code section; `check-readme.sh` and the rule in CLAUDE.md moved with it.

**Verified by running the app**: it opens on CHAT with 11 controls on screen
where it used to open on the knob with zero, and the run-through presses 34
controls across six pages with nothing blocking longer than 21.8 ms.

**The one UI defect found and NOT fixed** is the biggest one:
`backlog/the-window-ignores-display-scaling.md`. The window asks Windows for
per-monitor DPI awareness and then scales nothing, so on this 120-DPI display
every control and every font is ~20% smaller than designed. It has to be done
in one piece — scaling fonts alone puts 19px text in a 32px button — and the
design is written down. **Three external attempts to measure it produced three
sets of confident wrong numbers**, because `powershell.exe` is DPI-unaware and
Windows virtualises what it reads; the instrument to build first is a
pure-function layout, testable with no window at all.

**Claude Code now runs on a model this node serves, and the app has a button for it.**
tool calling, a prefix cache across requests, and `scripts/claude-chaos.cmd` as
the whole setup. Verified end to end: `claude -p "Read notes.txt and tell me
what city it names"` against Qwen3-4B produced a `Read` tool call, Claude Code
executed it, and the model answered from the result.

**`USE WITH CLAUDE CODE` on the CHAOS page** checks `claude` is installed
(offering the npm command if not), checks a model is loaded, asks which project
folder, and opens a terminal with the four environment variables set and the
tool set restricted. The wrappers and `docs/CLAUDE-CODE.md` now **ship** — in
the installer, every archive, and the Linux packages as `claude-chaos` on PATH.
They existed only in `scripts/` before, so a downloaded Chaos had none of it.

**Two instrument defects the button exposed**, both of which had been hiding
other things: `every_declared_control_is_created` never checked a control was
*laid out*, and `run-through.ps1`'s page lists were hand-written arrays — so a
control could be declared, created, positioned, wired and on screen while the
instrument built to find unreachable controls said nothing about it. There is
now a layout test (checked in both directions) and a sweep that reports every
visible control the list does not know. **The sweep found six controls that had
never been exercised on its first run**; the transcript went from 27 to 33.


Three things it cost, each a defect it uncovered:

- **`chaos-serve` capped the dense path at 2,048 tokens** and `-c` could only
  lower it, because the server prefilled in one pass where `chaos-run` had
  chunked for months. That made **every** agent client impossible. Now chunked,
  ceiling 16,384.
- **Qwen3's `<think>` blocks reached the client as the answer** — the first live
  request spent its whole budget reasoning and returned the reasoning.
- **The accept loop could wedge**: a 3 s read timeout and no write timeout, so a
  peer abandoning a request mid-answer blocked the single-threaded loop. Seen
  once as three `CLOSE_WAIT` connections and a dead port; bounded now at 60 s.

**It is slow and that is the hardware.** Turn 1 is ~6 minutes at 12k tokens;
the prefix cache took turn 2 from 135.6 s to **52.9 s** reusing 3,793 of 3,917
tokens, verified not to change the answer. `docs/CLAUDE-CODE.md` is the setup
path; `research/claude-code-against-a-chaos-node-2026-09-07.md` has every
number.


**v0.0.32 fixes the last report Atur had open: the book.** *"the book of QR code
for Core mode is not available!!! that book where is it!!"* — and it was not, for
three different reasons, none of which a test could have caught because none of
them was in the pages:

- **The window could only reach the art through a running engine.** The pages
  were a route on the child `chaos-serve`, so the book was a feature of a loaded
  model: turn the dial to CORE, press the button before pressing LOAD, and the
  browser said the site could not be reached. `gui/app/src/brand.rs` now serves
  the same assembled bytes from the window itself, on loopback, with no model.
- **The reader could not open a camera even with a model loaded.** `getUserMedia`
  is refused outside a secure context, and a LAN address is not one — so CORE
  mode handed the reader exactly the address that cannot work. Loopback is a
  secure context; the mark still *encodes* the LAN address, because the page
  prefers an injected endpoint over its own origin.
- **On the phone it was findable only if you already knew where it was**: bottom
  of the SETTINGS tab, inside a scroll view, behind a button labelled MARK.
  The mode badge is now the door, as it already is on the desktop's rail.

**The enabling change was one CLAUDE.md had recommended and nobody had taken**:
`grimoire` is string assembly with no ggml reference, and while it lived in
`chaos-arch` the two tiers that most want to show the art could not — the window
would have had to link ggml. It is now `chaos-grimoire`, its own crate with zero
dependencies; `chaos_arch::grimoire` is a `pub use` of it, so the server is
untouched. **The APK build stopped compiling a host llama.cpp** to write two HTML
files: `chaos-qr --emit-pages` does it with no C toolchain.

**v0.0.30 shipped with a broken front door
and a green test suite** — `chaos connect` took any unknown `--flag` as the hostname,
so it looked like the QR pages and device-to-device connection were missing when both
worked. `scripts/smoke-the-surface.sh` now runs 32 checks against the built binaries in
CI: every subcommand, every node route, and one machine asking another.

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
v0.0.27  Quality harness, then levers [####################] 100%  2 passed, 1 refused
v0.0.28  Any machine, any model       [################----]  80%  needs other machines
v0.0.29  Every platform, actually run [##############------]  70%  macOS is what is left
v0.0.30  LTS                          [####################] 100%  19 of 19 cells measured
```

**This block was stale until 2026-09-03** and read 40/55/35/6% while the README
said 100/100/80/70/100 and two versions were already tagged — the exact drift
the v0.0.24 rung exists to remove, in the file that calls itself the single
source of truth. `scripts/check-readme.sh` machine-checks the README's copy
against `CHECKLIST.md`; nothing checked this one, so **read the ladder from the
README**, which is enforced.

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
  written support policy. **The policy is written**: `SUPPORT.md` — what LTS means
  (the surface stops moving, correctness bugs get fixed on it, every claim was
  measured), what gets fixed in a patch and in what order, and a section that says
  plainly what is *not* supported: macOS untested, three Linux assets never
  executed, no GPU tier verified, no TLS in the HTTP
  client. **No support window in years** — one person cannot promise that, so the
  promise is structural: the newest LTS is supported until the next one exists.

---

## FIXED the same day: V4-Flash's pre-tokenizer was missing a rule

**Found and fixed 2026-09-02.** V4-Flash measured **+31.6%** worse than
llama.cpp in perplexity. The cause was one missing alternative in the
`joyai-llm` pre-tokenizer, and the fix closed it:

```
  before   18.5548     +31.6%
  after    14.6877      +4.1%   <- inside llama.cpp's 14.1034 +/- 1.79
```

`joyai-llm` begins `[!"#$%&'()*+,\-./:;<=>?@\[\\]^_`{|}~][A-Za-z]+` — one
ASCII punctuation mark plus the letters after it, as **one piece**. Ours was
transcribed from the *second* alternative onward, so `SUPPORT.md` became `.` +
`md` and `project's` became `'` + `s`. **Our ids now match `llama-tokenize`
exactly, 294 for 294.**

**Why four separate checks missed it**: it is the only `joyai-llm` container in
existence, so every other model agreed; the test named
`v4flash_still_uses_joyai_llm_unchanged` only checks *which* pre-tokenizer is
selected, and passes either way; the eight-prompt greedy diff that put the
architecture in `VERIFIED_ARCHITECTURES` cannot see a distribution gap; and the
perplexity harness that would have caught it **did not exist on this path** until
the same day.

**Quality against llama.cpp, all three models, instrument validated in one
session:** Qwen3-4B **-1.44%**, Qwen3-30B-A3B **+0.37%**, V4-Flash **+4.1%**.
Parity on quality across the range.

## Open, and the largest one: V4-Flash generation disagrees with its own prefill

**Found 2026-09-03, localised the same day, not yet fixed.** Feeding V4-Flash one
token at a time does not reproduce a batched prefill, and the disagreement
begins as soon as a **compressed block has completed**:

```
  tokens  cosine     max |diff|   closes a block on the LAST position?
       3  0.999866      0.4808    no  -- closes none at all
       4  0.981401      4.9674    yes
       5  0.996695      2.5935    no  -- closed at 3
       6  0.996145      2.4938    no
       7  0.990928      3.3050    no
       8  0.984060      5.5758    yes
      16  0.990389      2.9377    yes
      32  0.985793      4.2033    yes
      64  0.970304      4.3346    yes
```

`CSA_RATIO` is 4, so three tokens is the only length that closes no block at
all, and there the paths agree to 0.99987 — floating-point reordering and
nothing more. **Every length that closes one disagrees**, and the large step is
empty-against-non-empty, not the boundary.

**Two candidates eliminated 2026-09-03, and one of my own claims withdrawn.**

The ring index arithmetic is **not** it, and that is now mechanical rather than
read: `compressor_positions` is a pure function with four tests needing no
container, and a batch of `4n` tokens closes the same blocks from the same
positions as `4n` single steps, chunked prefills included. C5e freezing is not
it either — `FREEZE_MAX_TOKENS` is 192, so every length in the sweep freezes on
both paths.

**Withdrawn**: *"tie-breaking cannot do that"*. It can. The position that closes
a block is both the one whose state most directly incorporates that block and
the one whose logits are compared, so a routing flip would show up exactly
there. And the sweep holds two effects the earlier note merged: empty against
non-empty compressed half is the large step (0.99987 → 0.98), while boundary
against mid-block is a much smaller tendency (~0.983 against ~0.994) with 7 and
16 overlapping.

**ANSWERED 2026-09-03: the two paths choose different experts.**
`routing_last_token()` was already in the engine and had never been pointed at
this (it needs `CHAOS_ROUTING_LAST=1`; the first run said "0 layers logged",
which is what an unset gate looks like):

| | 3 tokens | 4 tokens |
|---|---|---|
| logits cosine | 0.999866 | 0.981401 |
| layers picking different experts | **3 of 43** | **33 of 43** |
| first such layer | 30 | **3** |

Layer 3 is the *first* layer with routed experts — `hash_layer_count` is 3 — so
at four tokens the earliest router that can flip does flip, and the 33 are
mostly cascade. At three tokens, where no compressed block exists, nothing
before layer 30 differs. **Rounding does not become 26 layers more potent
because a block closed**, so the compressor's *values* differ by more than
floating-point reordering, while its *positions* are identical by test.

**Not established: which path is wrong.** Every oracle capture is batched —
`llama-eval-callback` on a prompt — so all 22 container tests verify the batched
path and the stepwise path has never been diffed against anything. Settling it
needs llama.cpp under `-b 1`, which no fixture has.

**LOCALISED to layer 2's compressed attention.** `CHAOS_DUMP_LAYERS` — added to
this path in the same session, because it had none — gives every layer's final
position on both paths:

| layer | kind | n=2 | n=3 | n=4 |
|---|---|---|---|---|
| 0 | Raw | 1.5e-07 | 4.7e-06 | **0.00** |
| 1 | Raw | 3.1e-07 | 7.0e-04 | 2.8e-03 |
| 2 | **CompressedSparse** | 1.2e-04 | 4.2e-03 | **2.7e-01** |
| 3 | HCA, first **routed** layer | 1.1e-01 | 5.5e-02 | 2.6e-01 |

**Two tokens is the control.** Nothing closes a block anywhere at two tokens,
and there layer 2 sits at 1.2e-04. One token more and it is at 2.7e-01 — the
only thing that changed for it is that its first block closed. Note that a
per-layer figure is not proportional to the logit error: layer 3 is at 1.1e-01
at two tokens while the logits agree to 0.999874, because one flip in the first
routed layer is absorbed downstream. What marks four tokens is *where* the
first large number appears — one layer earlier than the first routed layer.

Layer 2 is the **first** `CompressedSparse` layer and four tokens is exactly
where its first block closes (`CSA_RATIO` is 4) — a hundredfold jump across that
boundary. It is also a **hash layer** (`hash_layer_count` is 3), so it has no
routed experts and the jump cannot be a routing flip; the 33 differing routers
from layer 3 on are downstream of it. And layer 0, `Raw`, is bit-identical,
which clears the KV cache path.

Still open: why layer 1 differs at all when layer 0 is identical and layer 1 has
neither compressor nor routed experts; whether layer 2's jump is that difference
amplified through an 8-entry softmax or a second defect; and which path is
wrong, since every oracle capture is batched.
`research/stepwise-and-batched-disagree-2026-09-02.md`.

## The honest scoreboard

**Current**: **1018 tests** (0 failed, 50 ignored — the V4-Flash set needs the
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

**Nothing below is blocking a merge, and v0.0.30 has shipped.** This list still
said "all of it is blocking v0.0.30" two versions after that tag; items 7, 8 and
9 were done and still written here as open. **Each one below was re-checked
against the code on 2026-09-03**, and what closed says so with the line that
proves it.

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
7. ~~**`chaos-run` has 8 tests**~~ **Closed.** It has **16**, across four files:
   `a_bad_value_is_refused` 5, `forward_pass_is_not_broken` 4,
   `refused_flags_decline` 7, plus one ignored polyfill test that needs a
   container.
8. ~~**The port is bound after the model loads**~~ **Closed.** `chaos-serve`
   binds at `lib.rs:220` and opens the container at `222` — the listener first,
   with `port_taken_message` on failure, so a 144 GB model no longer reads for
   minutes before failing on a taken port.
9. ~~**`finish_reason` is not surfaced**~~ **Closed.** `chaos_cli::finish_reason`
   parses it out of the stream and `cli/chaos/src/main.rs:564` reports it, so an
   answer cut off at the cap says so instead of stopping mid-word.
10. Smaller and honest: `chaos scan` is declared NOT BUILT; zsh and fish completions are
    generated but never sourced; no contrast audit or screen-reader story for the
    window; full-disk behaviour deliberately unmeasured; `strip` has never been run and
    a served page re-read.

**Out, not parked**: Android and iOS. Three platforms is the product -- Atur, 2026-09-07.

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

## The published assets, and which have now been exercised

Measured 2026-09-01. **None of them could be downloaded**: every route ends at
`release-assets.githubusercontent.com`, which resolves, accepts TCP in 0.13 s and
**resets during TLS**, while `api.github.com` answers 200 in 0.34 s. So this works
from artefacts already on disk, or from the source the assets are built from.

| asset | state |
|---|---|
| `windows-x86_64-Setup.exe` | **run here** — an install exists and its binaries work; today's build embeds `chaos.exe` |
| `windows-x86_64.zip` | its binaries are the ones this machine builds and runs daily |
| `linux-x86_64.tar.gz` | **its contents run**: a model generated on Debian 12, and the suite passes 991/0/42 there |
| `chaos_amd64.deb` | not executed — cannot be downloaded |
| `linux-x86_64.AppImage` | not executed — cannot be downloaded |
| `linux-arm64.tar.gz` | not executed — no arm64 Linux here |
| `macos-arm64.tar.gz` | not executed — no macOS here |
| `macos-x86_64.tar.gz` | not executed — no macOS here |

The phone tier was removed on 2026-09-07 before that was ever settled.

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
