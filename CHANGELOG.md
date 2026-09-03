# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While the major version is `0`, anything may change in a minor release.

## [Unreleased]

## [0.0.32] — 2026-09-03

**The book was unreachable, in three different ways.** Atur, testing v0.0.31 on
Windows and on a phone: *"the book of QR code for Core mode is not available!!!
that book where is it!!"* None of the three causes was in the pages themselves,
which is why every test of them passed.

### Fixed

- **The window showed the art only while a model was loaded.** The mark and the
  reader were routes on the child `chaos-serve`, so pressing the button before
  pressing LOAD opened a browser tab that could not reach the site. The window
  now serves the same assembled bytes itself, on loopback, needing no model —
  `gui/app/src/brand.rs`.
- **The reader could not open a camera at all in CORE mode.** `getUserMedia` is
  refused outside a secure context and a LAN address is not one, so the window
  was handing the reader precisely the address that cannot work. It is served
  from `127.0.0.1` now, which is a secure context; the **mark still encodes the
  LAN address**, because the page prefers an injected endpoint over the origin it
  was served from.
- **On Android the book was findable only if you knew where to look**: the
  bottom of the SETTINGS tab, inside a scroll view, behind a button labelled
  MARK. The mode badge is now the door, matching the desktop, where the badge at
  the foot of the rail is already the CHAOS page's only entrance.
- Three items in `STATUS.md`'s open list had been done for two releases and were
  still written as open, and its release ladder read 40/55/35/6% while the
  README's machine-checked copy read 100/100/100/100/80/70/100.

### Changed

- **`grimoire` is its own crate.** It is string assembly over two `include_str!`d
  HTML files with no ggml reference and no dependencies, and living inside
  `chaos-arch` — the one crate that cannot build without a compiled ggml — meant
  the two tiers that most want to show the art could not. `chaos_arch::grimoire`
  is a `pub use` of `chaos-grimoire`, so the server and `--emit-pages` are
  unchanged. CLAUDE.md had recommended this move; this release takes it.
- **The Android release no longer compiles a host llama.cpp to write two HTML
  files.** `chaos-qr --emit-pages` emits them with no C toolchain, deleting a
  second full cmake from every release — a step that failed the first time it
  ever ran, on v0.0.22, for exactly this reason.
- Thirteen of the fourteen CI-checked crates now build with no ggml, up from
  twelve of thirteen, and CI enforces the new crate too.

## [0.0.31] — 2026-09-03

**v0.0.30's front door was broken and its test suite was green.** 999 tests,
clippy, fmt, four document checks, seven release jobs — and nobody had run
`chaos connect` or opened `/qr` on a real node. Atur found it in the first
minutes of testing, reported as *"the QR creator and the reader are not in this
version and devices cannot connect"*.

### The bug

`chaos connect`'s argument loop pushed anything it did not recognise into the
positional list, so an unknown `--flag` became the **hostname**:

```
$ chaos connect --port 8080 --status
chaos: cannot resolve --port:8080: No such host is known. (os error 11001)
```

The route is positional — `chaos connect 192.168.1.20:8080 "your prompt"` — so
there was never a `--port` to type, and the error named DNS instead of the flag.
From outside, that is indistinguishable from a missing feature. It now says:

```
chaos connect: unknown option "--port".

The route is positional, not a flag:
  chaos connect 192.168.1.20:8080 "your prompt"

Options: --max-tokens N (or -n N). Nothing else.
```

**This is the defect E7 found in 43 of `chaos-run`'s flags**, which survived
untouched in the front door because nothing tested it. Everything the report
said was missing was present and working the whole time: `/qr` returns 363 KB,
`/scan` 229 KB, `chaos-qr` prints a code in a bare terminal, and a client got a
real answer from a node.

### The check that would have caught it

`scripts/smoke-the-surface.sh` — **29 checks against the built binaries**, in CI
on Linux where a real model is already cached:

- every binary answers `--version`
- every subcommand works or refuses clearly, including `scan` declaring itself
  not built and `connect` refusing an unknown option **by name**
- `chaos-qr` prints a code
- all eight node routes, with `/favicon.ico` asserted at 204 because that is
  deliberate
- `/qr` and `/scan` proven to **fetch nothing external**, which is what proves
  the fonts were embedded rather than linked
- `/v1/chat/completions` and `/v1/embeddings` answering with content
- **one machine asking another and getting an answer**

### Fixed

- **`chaos connect` refuses unknown options by name** instead of treating them
  as a hostname.
- **`chaos-qr --version`** — ten of eleven shipped binaries answered it and this
  one refused it as an unknown flag. `gguf-info` had the same gap once.
- **Three places in two documents said `/v1/embeddings` answers 501** for weeks
  after it was implemented on the dense path from a real hidden state. The
  `SUPPORT.md` written the day before repeated it. Corrected, and the smoke test
  now asks the endpoint rather than trusting the note. The V4-Flash path is still
  refused **by name**, because its forward pass exposes no hidden state.

### Also, on my own tests

Two of the four failures the new script first reported were **the script being
wrong**: `/favicon.ico` answers 204 by design, and its external-reference check
matched an ordinary `<a href>` link to the project's repository, which fetches
nothing. Both fixed before landing, because a check that cries wolf is a check
people learn to ignore.


## [0.0.30] — 2026-09-02

**The first release built to LTS standard, and the first tag in twenty-nine
rungs.** v0.0.24 through v0.0.29 were phases of work, never tagged: 23 releases
went out in 21 days once and none of them got a stabilisation period, so this
time nothing shipped until there was something to stand behind.

**What "LTS" means here is written down, and it is narrower than the phrase
usually implies** — [SUPPORT.md](SUPPORT.md): the CLI's flags, the HTTP
endpoints, the settings file and the catalogue layout stop moving; correctness
bugs get fixed on this release and not only on `main`; and every claim in these
notes was measured on hardware with the command line recorded. It does **not**
mean a support window in years — one person cannot promise that — so the promise
is structural instead: the newest LTS is supported until the next one exists.

**It does not mean parity with llama.cpp on every measure.** The gate for this
release is *every parity cell **measured***, not every cell won — Atur's decision,
because counting cells won can never reach 100% from here and a gate that cannot
be met is a reason never to tag. The table is **19 cells** (recounted: it claimed
18), all 19 now have a number or a written reason, and **three read
`unmeasurable here`** because llama.cpp will not produce a stable figure for
V4-Flash on this machine — it returned 0.16–0.47 tok/s across eight runs of one
command line, and hung outright after 40 minutes on a 4040-token prefill with all
threads waiting and zero CPU. `docs/graph/backlog/lts-parity-criteria.md` has
every cell.

### Faster, and each one through a gate

**DeepSeek-V4-Flash generation went 0.509 → 0.728 tok/s, 1.43x**, from two
changes that are *byte-identical* — 50 of 50 answers unchanged. A third lever was
built, measured, and **refused by the quality gate**.

- **The block tail was computed twice.** Freezing it is worth **1.120x**, proved
  inside one run using the three hash layers as an untouched control: the argsort
  route now pays 0.0074 + 0.0086 against the hash layers' 0.0000 + 0.0159 —
  equal sums, though an argsort block does more work. `CHAOS_NO_FREEZE=1` turns
  it off, which is why the A/B is honest.
- **The expert cache defaulted to zero on V4-Flash**, and `--auto` never ran on
  that model at all. Sized from total RAM now — `clamp(total − resident − 5 GiB,
  0, 6 GiB)` — which is **1.20x**. The old hint, *"`--cache <GiB>` is now worth
  measuring"*, pointed at a cliff: 3 GiB measured 0.721 tok/s and 6 GiB measured
  0.352 while the hit rate kept climbing.
- **`--trunk-quant q4_k` was REFUSED and no speed figure from it is quoted** —
  the gate's own rule. It converts the always-read trunk `Q8_0 → Q4_K` at load
  (resident **7.38 → 4.26 GiB**, cache 3.34 → 6.00, hits 28.3% → 34.0%) and then
  failed two of the three *lossy* checks independently: **20 of 50 answers
  byte-identical, 40.0% against a 95% bar**, and perplexity **+3.674%** against a
  +1% band. Only the checkable-answer check passed — 41 of 50, exactly the
  baseline's 41 — so a 7% weight error changes how the model words things in 60%
  of cases and what it knows in none. The flag stays in the tree **off by
  default**, documented as failing in `--help`, in its module docs, and in a line
  printed at run time.
- **Two `cont` calls left the dense KV path**: byte-identical, **+7.1% at 4031
  tokens** and +2.5% at 500, across all six alternating pairs.

### Every platform, actually run

- **Linux could not build from the README, and now a model runs there.** cmake
  emits `libggml-base.a` under GCC and both build scripts looked only for
  `ggml-base.a`, so the documented instructions produced a file the error
  message called missing. CI never caught it because CI stages the archives with
  `sed 's/^lib//'`. Fixed, and on Debian 12: Qwen2-0.5B at 50.01 tok/s,
  V4-Flash generating, and the suite passing with Windows's exact counts.
- **The published APK installs, launches and renders its interface** on an
  android-34 x86_64 emulator — its arm64 library runs because that image lists
  `x86_64,arm64-v8a`. It then crashes entering a mode, in one anonymous frame of
  translated code with `libchaos_android.so` never mapped, which is **not
  attributed to Chaos**. Still never run on a phone.
- **The Windows installer's install was verified from the artefact on disk**, and
  the missing `chaos.exe` in it is the pre-v0.0.22 gap — evidence that fix
  mattered.

### Any machine, any model

- **`chaos-pull` picks the quant that fits your machine** instead of advising you
  to. It probes before choosing, takes the largest quant whose always-read set
  fits, says what it passed over and why, and **predicts tok/s only where the law
  is calibrated** — declining for streaming containers and for anything outside
  1–24 GiB resident. Its one prediction here, 8.2 tok/s for Qwen3-4B, is within
  1% of the measured 8.27.
- **The deepseek4 path reports which threads it used**, because generation wants
  2–4 and prefill wants all of them.

### Guarded rather than documented

- **`scripts/quality-gate.sh`** — 50 checkable prompts with **a different bar per
  lever**: *exact* needs 100% byte-identical, *lossy* needs ≥95% plus no
  checkable regression plus perplexity within 1%. Validated against 1 MiB of
  zeros written into a container: 22.0% identical, four checkables lost,
  perplexity +1.16% — all three checks fired independently.
- **`scripts/check-docs.sh`** — the documents are counted now, not assumed.
  **Ten graph nodes were in no index line at all**, which in a repository whose
  own instructions say "read `INDEX.md` first" means ten nodes nobody would ever
  read. 46 of 148 declare no topic; that debt is **ratcheted** — today's count per section is a ceiling, so adding an undeclared node fails and fixing files lets the ceiling come down.
- **`scripts/check-readme.sh`** — the README carries three things and nothing
  else, enforced: the bars, the document map, and tok/s for five fixed models
  with the machine named and dated. It was 393 lines once.
- **Perplexity reaches the streaming path.** It was plumbed to the dense runner
  and the deepseek4 dispatch returned before it, so the flag was silently ignored
  on the one model this project is built around — a third of the *lossy* bar,
  unmeasurable, for every lossy lever rather than for one.

### The bug that made V4-Flash look worse than it is, found and fixed

**Our `joyai-llm` pre-tokenizer was missing an entire regex alternative**, and it
cost 31.6% of perplexity against llama.cpp.

```
  before   18.5548   +31.6% against llama.cpp
  after    14.6877    +4.1%  -- inside llama.cpp's 14.1034 +/- 1.79378
```

llama.cpp's rule begins `[!"#$%&'()*+,\-./:;<=>?@\[\\]^_`{|}~][A-Za-z]+` --
one ASCII punctuation mark plus the ASCII letters after it, as **one piece**.
Ours was transcribed starting from the *second* alternative, so `SUPPORT.md`
became `.` + `md` and `project's` became `'` + `s`. The model has only ever seen
those whole: feeding it `16, 17949` where it expects `26887` is out of
distribution, expensive at that position, and it corrupts the context after it.
**Our token ids now match `llama-tokenize` exactly, 294 for 294.**

**Four checks could have caught it and none did**, which is the part worth
keeping: it is the only `joyai-llm` container in existence, so every other model
agreed with llama.cpp; the test called `v4flash_still_uses_joyai_llm_unchanged`
checks only *which* pre-tokenizer is selected and passes either way; the
**eight-prompt greedy diff** that put the architecture in
`VERIFIED_ARCHITECTURES` cannot see a distribution gap, because a wrong split
still reads as fluent English; and the perplexity harness that would have shown
it **did not exist on this path** until the same day.

### Quality against llama.cpp, measured across the range

One session, real prose, chunk 512, per-chunk BOS, both engines scoring the same
tokens:

| model | ours | llama.cpp | |
|---|---|---|---|
| Qwen3-4B dense | 44.8667 | 45.5245 +/- 5.70 | **-1.44%** |
| Qwen3-30B-A3B | 25.9308 | 25.8347 +/- 2.78 | **+0.37%** |
| DeepSeek-V4-Flash | 14.6877 | 14.1034 +/- 1.79 | **+4.1%** |

**Parity on quality across all three**, each inside the other's error bar. On
speed, Qwen3-30B-A3B long-context generation is **1.38x ahead** -- Chaos's worst
run beating llama.cpp's best -- and dense prefill is 1.24x behind.

### Retracted

Kept here because a retraction is part of the record.

- *"Proven: Qwen3-30B-A3B"* — `qwen3moe` is not in `VERIFIED_ARCHITECTURES` and
  needs `--force`; its eight-prompt diff came back 1 FAIL.
- *"The routed expert matmuls are 0.004 s, under 1%"* — they are **40%**. The
  figure came from subtracting an expert read from a phase timer that measured
  construction plus the read.
- *"88% of `F` is the hyper-connection algebra"* — it is **8%**; attention is 40%.
- *"4.26 tok/s is the ceiling"* — that was a disk bound with the arithmetic set
  to zero. The arithmetic bound is three times lower: **1.5–1.8 tok/s** with the
  disk removed entirely.
- *"C7's argument is to move the trunk to a dtype with a kernel"* — `Q8_0` is the
  *fastest* of F32/BF16/Q8_0 at the trunk's own shape.
- *"The TinyLlama chat template is wrong"* — filed and withdrawn within the hour:
  the detector was right and the test was stale.
- *"`-t` is ignored on the deepseek4 path"* — it is bridged via `CHAOS_THREADS`.
  The evidence was one run taken after three others had starved the machine.

## [0.0.23] — 2026-08-28

Four decisions the v0.0.22 audit left open, taken and implemented.

### A corrupt model can now be told from an intact one

**The worst finding of the audit.** Four kilobytes of zeros written into a
container's tensor data loaded, exited 0, and answered *"The capital of France
is"* with *" Paris. The capital of Germany is Berlin"* where the intact model says
*" Paris. The capital of France is Paris"*. Both fluent, neither flagged. There
was no checksum: `download` verified the four magic bytes and nothing else.

```
chaos verify <model> [--expect <sha256>]
```

Hashes the container and compares it with `.chaos-sha256` beside it. With nothing
on file the reading becomes the record — and says so, because trust-on-first-use
means *"unchanged since then"*, not *"what the publisher shipped"*. `--expect`
takes a publisher's digest for the stronger answer. `chaos pull` records a digest
as each file finishes, which is when it is cheap and certain.

**Size is checked before the hash**, because it is free and conclusive: a file of
the wrong length cannot be the right file, and saying so takes a millisecond
rather than minutes. That is the corrupt-resume case, where a resume ends up
*larger* and passes every other check.

Measured on the exact corruption above:

```
intact          RECORDED  3f5a2242…  807,694,368 bytes in 2.44 s (~330 MB/s)
4 KiB zeroed    WRONG CONTENTS — same length, different bytes, exit 1
5 bytes longer  WRONG SIZE — expected 807694368, found 807694373, exit 1
```

**SHA-256, written out** — the workspace still has zero third-party crates. Chosen
over a faster non-cryptographic hash for one practical reason: publishers,
Hugging Face included, publish SHA-256, so a file can be checked against *their*
value. Verified against the published FIPS 180-4 vectors, the million-character
vector that catches a broken length field, every chunk size and length across the
padding boundary, and **byte-for-byte against Python's `hashlib` on a real 807 MB
model**. 330 MB/s puts a 144 GB container at roughly seven minutes, which is why
it is a command rather than something every load pays for.

### `/status` and `/health` are behind the key, from the network

With a key set, an unauthenticated caller used to be served both — the model
name, its context size and the node's route. Now the rule is the **peer**, not the
bind address: **this machine is never gated**, and the network needs the key.

Gating by the bind address would have broken two things that ask locally with no
key: the window probes `/health` on `127.0.0.1` to learn whether the server it
just started is answering, and `chaos status` reads `/status` the same way.
`chaos status` now sends the key as well, so it still works against a remote node.

**The mark and the reader stay open from anywhere.** A stranger's phone has no
key, and pointing its camera at `/qr` is the entire point of them.

Measured from this machine's own LAN address against a node bound to it, which
makes the peer non-loopback:

```
no key    /status 401   /health 401   /v1/models 401   /qr 200
with key  /status 200   /v1/models 200
loopback  chaos status works with no key, as before
```

### Every binary ships, and the rule is written down

Three of six benchmarks shipped, for no stated reason — the same failure that hid
`chaos-qr` from every list. All six ship now: additive, so nobody loses a binary
they were using. The rule is a comment at the staging loop rather than something
to infer.

### The ggml source is pinned

Both workflows did `git clone --depth 1` of llama.cpp, so *"CI was green"* was a
statement about whatever `master` was that day, and a build that broke overnight
was indistinguishable from one we broke. Five clone sites now fetch one pinned
commit held in each workflow's `env` block. Tried for real: the fetch lands the
exact SHA and stays shallow.

**953 tests**, 0 failed, clippy and fmt clean.


## [0.0.22] — 2026-08-28

### The window opened onto its own launch screen

Atur, on the installed v0.0.21: *"the latest version was very broken — even mode
selection got mixed up inside the application … everything was falling apart."*
Reproduced on the installed build, and it was one defect with a second one hiding
behind a bad probe.

`WM_CREATE` ended with `show_page(Page::Chat)` while `ui.launched` was still
`false`. `WM_PAINT` stops at `paint_launch` — but **the controls are real HWNDs and
painting the knob cannot cover them**, so the window opened with the mode dial
painted *underneath* the chat transcript, its composer, SEND, CLEAR, the four rail
buttons and STOP.

```
on open        installed v0.0.21        after
controls       9 on-screen              0 on-screen
ESC            no effect                no effect
RETURN         9                        9
```

ESC doing nothing at open is the proof `launched` was false: it is the only caller
of `back_to_knob`. Measured by client-rect, not `IsWindowVisible` — `layout` parks
unreachable rail buttons at `(-3200,-3200)` and leaves them "visible".

**A "blank CHAOS page" was reported and retracted the same day.** Cross-process
`GetWindowTextW` reads a window's *caption*, never an EDIT's text, so the probe was
blank and the app was not.

### The mode is asked once, then remembered

Atur's call. `role` cannot carry "has this been asked" — its default is a real
role — so a new `mode_chosen` setting does. First launch shows the dial and writes
the key; later launches go straight in; ESC still returns. An existing
`settings.txt` has no such key, so everyone upgrading is asked exactly once more.

### CHAOS left the menu; the mode lives in the rail

Atur: *"why is the CHAOS option in the app's menu list? it is chosen the first
time the app opens … at the bottom left we should show the mode + a CHANGE MODE
button"*. CHAOS is no longer a rail page; the badge at the bottom of the rail shows
the mode and is the page's only door. **Leaving a mode asks first — and so does
Escape**, which had gone straight to the dial since the dial existed, so one
keystroke could unload a model and clear a conversation unasked.

### The command line became a tier

*"someone may run Chaos on a headless server and connect from their own machine,
and the CLI must not be a lesser tier."* Six of the plan's seven items are done.

**`chaos <subcommand>` is the front door, and every old binary name still works** —
`chaos run` *is* `chaos-run`, arguments untouched. New: `chaos start` / `stop` /
`status` manage a node as a background process with a pid file and a log, and
`chaos connect <route> "prompt"` streams from another machine's node.

**One settings file, read by both tiers.** `Settings` moved from `gui/app` to
`core/config`, which is what fixed *"the app has a settings file the CLI cannot
read"*. `chaos start` builds the server's flags with `Settings::serve_args` — the
window's own function, so there is no second list to drift.

**New `core/http`**, because `/status` had to be reported *without curl* and the
workspace had no client at all. Plain HTTP, no TLS, documented as a limit.

```
chaos start Llama-3.2-1B   pid 23660, reachable in ~1 s
chaos status               model / route / context / off-loopback  — no curl
chaos connect …            streamed "Red", key from the shared settings
```

`chaos scan` is **listed as NOT BUILT and says so**: `core/qr` encodes only, and
decoding a photograph is Reed-Solomon *correction* whose every stage fails by
returning a plausible wrong string.

**`chaos-qr` was in no ship list at all** while the brand tier claimed it reached a
bare terminal. It and `chaos` now ship on every platform.

### Fixed

- **An SSE stream that ends without `[DONE]` is a truncation, not a short
  answer.** `[DONE]` is the only evidence an answer is whole; a gracefully closed
  connection looks exactly like a finished one. Both that and a mid-stream read
  error now say how many chunks arrived and that what was printed is incomplete.
- **Two working endpoints were missing from `chaos-serve --help`**: `POST
  /v1/completions` and `POST /v1/embeddings`, both implemented *and* unit-tested.
  Found by a new test the first time it ran.
- **The embedded fonts were attributed nowhere a user receives.** Cinzel, IBM Plex
  Mono and UnifrakturMaguntia are OFL 1.1 and embedded in every build; the shipped
  `NOTICE` had zero mentions and the served page zero occurrences of "Copyright".
  Both fixed, with a test.
- **"No telemetry" was true and stated nowhere** — the only occurrence of the word
  in the repository was the plan asking for it. Now in the README and `SECURITY.md`.
- The README's headline **31 tok/s on Qwen2-0.5B was the best of three runs**, not
  the median. Now 28, with the old figures shown. Falcon3-1B's 20 was conservative
  against a 20.72 median.
- `crates/chaos-ggml` in `NOTICE` and `SECURITY.md` — a directory that does not
  exist, in the two documents whose job is accuracy.

### Retracted

- **"Proven: Qwen3-30B-A3B generates correct text on a 15.7 GiB machine."** The
  model needs `--force`: `qwen3moe` is not in `VERIFIED_ARCHITECTURES`, because its
  eight-prompt diff returns 1 FAIL — a demonstrated near-tie where llama.cpp
  produces our exact answer under `-b 1`. The engine is probably right and the
  claim is not earned. **V4-Flash is verified and carries the headline now.**

### Measured

- **The dense gap against llama.cpp is entirely the FFN.** Hand-tuned against
  hand-tuned on Llama-3.2-1B: llama.cpp is **1.30x ahead on prefill (382.26 v
  294.29) and 1.30x ahead on generation (27.14 v 20.80)**, and `chaos-run`'s own
  breakdown puts **1.9 s of 3.1 s of compute in the FFN** with 0.0 s disk. The
  thread count is a bigger lever than the engine: llama.cpp's generation is 1.84x
  faster at `-t 4` than at `-t 20`.
- **Load time and tokenization are immaterial**, with numbers: a 17 GiB model's
  always-read set arrives in **0.38 s** and saturates at four threads; tokenization
  costs **0.76 ms against 2,100 ms of prefill — 0.036%**. New `chaos-tokbench`.
- **Nothing detects a corrupt model.** 4 KiB of zeros in the weights loads, exits
  0 and answers *"the capital of Germany is Berlin"* where the intact model says
  *"France is Paris"*. There is no checksum: `download` verifies the magic bytes.
- **14 GPU tests run and pass on an RTX 3050** via Vulkan — the first time the
  suite has actually executed. `CHAOS_REQUIRE_GPU=1` turns a skip into a failure,
  because a skip had been reporting as a pass.
- **The SBOM is one fact**: `Cargo.lock` holds 22 packages and all 22 are crates in
  this repository. Zero third-party dependencies.

### Added

- `core/config`, `core/http`, `cli/chaos`, `chaos-tokbench`.
- `core/arch/tests/documented_counts.rs`, which enforces the architecture count,
  the binary count and the consistency of `release.yml`'s three ship lists — three
  documented counts drifted in a single day.
- `network/serve/tests/`, which it never had, holding two cross-file contracts.
- Shell completions for bash, zsh, fish and powershell; **bash and powershell are
  sourced into their real shell and driven**, zsh and fish are generated only.

**942 tests**, 0 failed, clippy and fmt clean.


## [0.0.21] — 2026-08-26

### A model runs on the phone

Atur: *"android can not do any one of works in windows and just can connect
windows"*. He was right — the dial offered four modes and only CLIENT did
anything. On an Android 34 emulator, with no PC involved:

```
you
What is the capital of France?

chaos
The capital of France is Paris.

7 tokens in 1.8s (3.938 tok/s), finish=stop
POST /v1/chat/completions -> 200 (stream) in 3.2s
```

**`chaos-serve` is a library now.** `network/serve/src/main.rs` became `lib.rs`
plus a thin `src/bin/chaos-serve.rs` that is the only thing parsing arguments.
Everything the phone needed already existed there — the token loop, sampling,
streaming, the endpoints — and the Kotlin client already spoke to it, so no
second token loop had to be written.

**The engine runs as a child process, not inside the app.** Loading it in worked
for anything that did not make a thread; the moment `StreamingRunner::new`
called `pthread_create` the app died with SIGSEGV/SEGV_ACCERR inside
`__init_tcb`. A 16 MiB stack did not help, moving the call to a JVM thread did
not help — the crash moved deeper — and the library has no `PT_TLS` segment to
blame. **The same engine as an executable makes threads perfectly on the same
device**, which `chaos-run` proved in v0.0.19. So Android does what the desktop
has always done: spawns the server and talks to it over the API.

Models go in the app's external files directory — reachable over USB or a file
manager, which a `.gguf` of several hundred megabytes has to be, and app-private
so uninstalling takes them with it.

### The launch screen

The window and the app now open on **one question** instead of six pages of
controls: a gas-stove dial with four detents across a 180° top sweep, and stops
at both ends. Turn it, press, and the shell shows only what that mode can do.

- **Windows** renders it per pixel — plain GDI has no gradient fill, and the
  chamfer ring is most of what makes it read as three-dimensional. Light is
  computed in screen space while geometry is read in knob space, so turning the
  knob does not turn the highlight.
- **Android** draws the same geometry with `Canvas`.
- The badge is `assets/logo.svg`, rendered per density and **never redrawn**.
- The logo animates in before the dial, and any key or click skips it.
- `ESC` on the desktop, **CHANGE MODE** on the phone, returns to the dial.

### Fixed

- **The logo rasteriser was 190x too slow at splash sizes.** One splash repaint
  measured **1510 ms**; timing each separately gave 1510, then 1, 1, 1 — the
  cache was fine, the cost was a single rasterisation. `logo_coverage`
  supersamples 8x8 and its grid is `n * SS` per side, so 170 px meant a
  1360-square grid. The 8 exists because a ray is one pixel wide at 44 px; at
  170 it is four. Size-aware now, and the first frame is **8 ms**.
- **The dial's badge was invisible** — white ink on a white knob face, because
  the launcher's ink is white for a blue tile. A screenshot showed **0 dark
  pixels** in the badge; it is 825 now.
- **The mode labels were unreadable** — `0xFF111111` on a `#0D1117` background —
  and ALONE was clipped off the left edge entirely.
- **The dial had a black surround.** The app's theme is dark and a white moulded
  knob on it read as a hole; the dial screen has its own light ground.
- **The phone's note said "THIS PHONE IS A CLIENT" in every mode**, including
  the ones where it was running a model itself.
- **Every size on the dial screen is a dimension resource**, with values for
  `sw360dp`, `sw600dp` and `h480dp`.
- **The dial clicks**, with a system click and a clock-tick haptic as each
  detent is crossed.
- **ggml is built position-independent.** `android/jni` is a cdylib and gained
  ggml through `chaos-serve`; Linux CI's non-PIC archives failed the link with a
  page of `R_X86_64_PC32` relocation errors against `stderr` that name neither.

### Measured

- **Queue depth is worth 2.55x on the disk.** `chaos-qdbench` reading
  expert-sized blocks: depth 1 gives 1.34 GiB/s — reproducing what Chaos gets
  today, which confirms it reads serially — depth 4 gives 2.39x and depth 8
  gives **2.55x**.

### Retracted

- **The 5 tok/s ladder does not reach 5.** It modelled a token as
  `bytes / bandwidth` and omitted the **0.84 s of arithmetic that never touches
  the disk**. With that restored the rungs are 0.31 / 0.56 / 0.76 / 0.93 /
  **0.99** — rung 1 is worth 1.82x, not 2.55x.
- **And 5 tok/s is out of reach on this laptop at all.** V4-Flash reads
  **7.38 GiB of always-read weights every token**; at this machine's measured
  30.8 GiB/s that caps it at **4.17 tok/s with the disk free and no compute**.
  5 tok/s needs 36.9 GiB/s for the trunk alone. **What is achievable here is
  0.43 → about 1.0 tok/s**, which agrees with the 1.19 ceiling
  `v4flash-ram-frontier` found from the opposite direction.

### Still not done

**HELPER is reserved.** `chaos-worker` speaks the protocol and is measured, but
no CORE routes an expert to it yet, and both the dial and the phone say so.

## [0.0.20] — 2026-08-25

### The engine runs inside the Android app

v0.0.19 proved Chaos runs on Android as a command-line binary in
`/data/local/tmp`. That is not something an app can use. **This release puts it
in the app**, and the APK CI publishes now carries it.

Read off the running app's own screen on an emulator:

> **engine 0.0.20 on this phone: 4 threads, 2.4 GiB total, 1.6 GiB available
> [/proc/meminfo]**

That line is produced by **Rust running in the app process**, calling the same
`core/probe` the desktop uses. It is also what will decide which model a given
phone can hold — *"a powerful phone or a simple phone"* — by measuring the
device rather than matching a hard-coded list of handsets.

**No `jni` dependency.** The project has none and the APK has none. JNI's ABI is
a table of function pointers at fixed indices; exactly one entry is declared —
`NewStringUTF`, index 167 — with the padding before it marked load-bearing,
because a wrong index calls a different function through a pointer and that is
neither a compile error nor a clean crash.

**The library is allowed to be absent**, and every APK before this one took that
path. `Engine.available` is false, the device line falls back to Android's own
reading, and the app carries on as a client. The `.so` is **not committed** —
half a megabyte per ABI of build output nobody can review — so CI builds it, and
**greps the finished APK to prove it is in there**: a `cp` that worked and a
package step that dropped the file look identical from outside, and the app is
built to survive the difference silently.

### Fixed

- **`UnsatisfiedLinkError` is an `Error`, not an `Exception`.** A
  `catch (e: Exception)` would have let the app die in a static initialiser with
  nothing a user could act on.
- **The NDK's `.cmd` wrapper cannot link a cdylib.** rustc passes
  `--version-script=<path>` to control exported symbols; cmd.exe mangles it and
  the link dies with `--version-script=...\list"" was unexpected at this time`,
  which names neither Rust nor the NDK. The executables in v0.0.19 were fine
  because they never get that flag.
- **The app was telling a lie it had outgrown.** The CHAOS note still read
  *"this phone cannot be a CORE yet — running a model needs Chaos built for
  Android, which is not done"* while the engine was demonstrably running two
  lines above it. Caught only by reading the screen.

### Retracted

- **"Nothing about this app is verified on the machine that wrote it"**, which
  the release workflow had said in a comment since the APK first shipped. The
  NDK and the SDK are both on public mirrors that were never tried. The app has
  since been built *and run* locally, and running it found four defects a build
  never would. What stays true, and is what the comment says now: **CI builds
  the shipped artefact from Google's own repositories**, and **nobody has run
  this on a real handset.**

### Still not done

Loading a model file and running the token loop on the phone. **The bridge is
finished; that part is not**, and no release note should imply otherwise.

## [0.0.19] — 2026-08-25

### Chaos builds and runs on Android

**"Phase B is blocked" was wrong.** v0.0.18's notes said running models on the
phone needed the NDK and that `dl.google.com` 404s this network, so CI was the
only route. The Google host is unreachable — but **the NDK is on the same
Tencent mirror the SDK came from**, which had never been tested. The block was
inferred from one host.

```
$ adb shell /data/local/tmp/chaos-run --version
chaos-run 0.0.19

$ adb shell /data/local/tmp/chaos-probe --quick
os         android (x86_64)
cpu        4 threads
ram        2.4 GiB total, 1.6 GiB available   [/proc/meminfo]
```

**It took less than the plan assumed.** Every crate already type-checked for
`aarch64-linux-android` with no source change — including `core/probe`, the
crate the backlog named as the obstacle, whose unix branch reads `/proc/meminfo`
and works on a phone unmodified. That is also what will answer *"a powerful
phone or a simple phone"*: Chaos can measure the device and pick a model to fit.

All the work was link flags in `core/ggml/build.rs`, each right elsewhere and
wrong here: the NDK has no `libgomp`, bionic keeps pthreads inside libc, there
is no `libstdc++` (the NDK ships LLVM's libc++) — and naming nothing is also
wrong, because rustc passes `-nodefaultlibs`, so `libc++_static` must be named
**and** its sysroot directory searched.

**Two mistakes inside that fix, both kept in the node.** `Command::new` cannot
execute a `.cmd` and the NDK's Windows compiler is one, so the lookup silently
returned nothing and the failure looked like ggml missing `operator new`. Then
the helper read `CC_aarch64_linux_android` **by name** and returned nothing for
the emulator's x86_64 ABI, bringing the same failure back on a target just
proven to work. Both derive from `TARGET` now.

### Known: every Android CLI binary aborts at exit

`FORTIFY: pthread_mutex_destroy called on a destroyed mutex`, SIGABRT, exit 134.
**The work completes first** — `--list-devices` enumerates the backend and reads
the device's memory correctly — and the abort is strictly in `exit()`.

Located precisely: `exit → __cxa_finalize → std::mutex::~mutex()`, on
`ggml_critical_section_mutex`, ggml's one global mutex. Three theories are
eliminated in the node so nobody retries them — it is not duplicate linkage, not
a duplicated C++ runtime, and not Rust's runtime, since a ggml-free Rust binary
exits 0 on the same device.

**Probably harmless for an app, and that is reasoning rather than a
measurement**: an Android app never calls `exit()` — it loads a `.so` and is
killed — so `__cxa_finalize` should never run. It must be checked against a real
JNI library before anyone relies on it.

### The three open roadmap items, answered rather than left implied

- **V4-Flash at 20 tok/s is closed by measurement and cannot be delivered on
  this machine at any effort.** A token is 1.56 s of expert reads plus 0.84 s of
  arithmetic that never touches the disk, so with every expert resident this CPU
  tops out at **1.19 tok/s** — the fixed cost alone is **17x** over a 50 ms
  budget — and 20 tok/s separately needs **67.7 GB/s** to the experts. Holding
  all 144 GB in RAM is worth **2.9x, not 48x**. Every idea that might have
  closed it is on the measured dead-end list.
- **Devices as resources is half done.** `chaos-worker` speaks the protocol and
  is measured at 38x in favour of sending the work to the weights; **no CORE
  routes an expert to it yet**, and the CHAOS page says so rather than implying
  otherwise.
- **llama.cpp is parity, not a lead.** Parity on everything that streams,
  1.20–1.27x behind on the hand-tuned dense path, 1.23x ahead out of the box.
  **The ranges overlap.**

## [0.0.18] — 2026-08-24

### Your phone could never reach the desktop, and nothing was broken

Atur: *"when i try connect desktop nothing happen"*.

**Nothing was listening anywhere a phone could reach.** `chaos-serve` binds
`127.0.0.1` unless told otherwise, and the window passed `--port` and **never
`--host`** — so every server this app has ever started could answer only the
machine it ran on. No route in, and therefore no error either.

The Android testing in v0.0.17 missed it because `chaos-serve --host 0.0.0.0
--api-key` was run **by hand**: a configuration the app cannot produce. The
phone was tested against a server the app never creates.

### A CHAOS page, because the fix is a decision about what this machine is

```
ALONE    only this machine; 127.0.0.1, no route in
CORE     holds the models and answers; 0.0.0.0, others connect here
HELPER   lends its memory and cores to a CORE
CLIENT   uses a CORE elsewhere, loads nothing here
```

Choosing CORE opens the route, generates an api key, and shows the address and
key with COPY beside each — they get typed into a phone once. `chaos-serve`
refuses `0.0.0.0` with no key and is right to; refusing is the wrong thing to
do to somebody who has just pressed CORE, so the window makes one.

Measured end to end with the arguments the page produces:

```
TCP 0.0.0.0:8231 LISTENING
GET  /v1/models  no key                 -> 401
GET  /v1/models  with the key           -> the model list
POST /v1/chat/completions over the LAN  -> a real answer
```

Every page and every role driven through the real window: **worst blocking call
20.1 ms.**

**HELPER says out loud that it is not finished.** `chaos-worker` speaks the
protocol and is measured, but a CORE does not route any expert to it yet, so
the role is reserved and does no work.

### The phone

A CHAOS section saying what a CLIENT is, why this phone cannot be a CORE yet
(running a model needs Chaos built for Android, which is not done), and why
cache, threads and device settings are deliberately absent: **they decide how a
model runs, so they belong to whichever machine runs it.**

### Fixed

- **`new_key()` could return the same key twice.** It seeded from the clock and
  the process id and nothing else, so two calls inside one tick produced
  identical keys. Windows' finer clock hid it; macOS CI failed on the second
  call. Not cosmetic — pressing NEW KEY twice could hand out a key somebody
  else already has. An atomic counter now guarantees distinctness; the clock,
  the pid and an ASLR-moved heap address make it hard to guess. The test asks
  for a hundred in a row, because two did not catch it either.
- **The Android address hint said port 8080** for two releases — `chaos-serve`'s
  own default, but not the port the window uses, so the placeholder gave the
  wrong number for the one thing it exists to help with. It says 8231.
- **A page could be built, laid out and never shown.** `SHELL_CONTROLS` carried
  a comment saying IMAGE had once been added that way; the comment did not stop
  CHAOS doing exactly the same. There is now a test: every page's rail button
  must be shell chrome. A sentence is not a mechanism.

### Measured

- **The `-ngl` ladder**, open since 2026-08-16. Qwen3-4B fully resident in a
  6 GB RTX 3050: prefill **1.77x**, generation **0.46x** — never once faster
  than the CPU. Qwen3-8B: prefill **2.31x**, generation **0.58x**. The card
  saves 5.63 ms per prompt token and loses 185.97 ms per generated one, so it
  pays only above a prompt:generation ratio of **33:1** (4B) or **14:1** (8B).
  `--auto` decides on whether the model *fits* and is **2.14x slower** than the
  CPU on an ordinary `-n 200` reply — measured end to end, 117.0 s against
  54.6 s. Not fixed: the break-even moved by more than 2x between two models on
  one machine, so a constant would be wrong.
- **Do not read the generation row as "the GPU is slower".** VRAM bandwidth is
  about 2x this laptop's DDR5, so resident weights should read *faster*. The
  2.2x is per-token overhead in this Vulkan path, and nobody has looked at why.
- **A negative prompt follows what you write and barely pulls.** At 512×512 the
  untexted twin sits 7.29% from the conditional velocity; *"blurry, low
  quality, distorted"* sits **0.13% — 56x weaker** — and its guided step lands
  0.69 points out of 20 from the step with guidance **off**.
- **`--update --yes`**, because `--update` could not be scripted at all: it read
  the question from stdin and then opened the installer's *window*.
- **Five GPU flags** — `--list-devices`, `--device`, `-ngl`, `-ot`,
  `--op-offload` — all worked and none appeared in `chaos-run --help`.

### Retracted

- **"`--auto` is 41% slower at `-n 200`."** That was extrapolated from
  per-token rates measured over 32 tokens. Measured end to end it is **2.14x**.
  A per-token rate measured over 32 tokens is not a constant, and the device
  path degrades faster than the CPU as the context grows — which also makes the
  break-even ratios above **lower bounds** rather than estimates.

## [0.0.17] — 2026-08-24

### The Android app runs, and running it found four defects

v0.0.16 shipped an APK that had never been started. It has now been started —
on an Android 34 emulator, against a real `chaos-serve` on the host — and it
answered the project's own correctness prompt:

> **The capital of France is \*\*Paris\*\*.**

`chaos-serve` logged `GET /v1/models -> 200` and `POST /v1/chat/completions ->
200 (stream)`. Four defects were on the screen within a minute, and **not one of
them could have been found by building it**:

| what running it showed | cause |
|---|---|
| "Chaos" twice, stacked | the framework theme draws an ActionBar with the app label, and the layout has its own heading |
| the key field read `API key (required` | CONNECT sits beside it, so the hint did not fit |
| a bare `<think>`/`</think>` around every reply | **the tags arrive split across streamed pieces** — Qwen3 emits `<`, `think`, `>` as three tokens — so filtering each piece as it arrives sees none of them |
| the address and key were lost | saved in `onPause`, which **never runs when the process is killed** rather than paused |

`ThinkFilter` accumulates the stream and holds back only a tail that could still
be forming a tag. It is the one piece of real logic in the app and the one that
cannot be checked by looking, so it has nine unit tests — including that the
result does not depend on how the stream was chunked, and that an *unterminated*
block is released rather than swallowed, because a truncated stream showing
nothing is indistinguishable from a server that never replied. CI runs them
before it builds the APK.

**Still not run on real hardware.** An emulator is not a phone.

### `chaos-worker` — one machine holds the experts, another asks

The first step of "devices as resources": a process that keeps expert weights in
RAM and answers with activations. `chaos-run`'s arithmetic without its token
loop. The plan's order was protocol, then a worker that computes, then **measure
and stop** — this is all three, and it stops here.

```
request             8268 bytes      scaled to a V4-Flash token:
answer             49172 bytes        on the wire     4.94 MB
                                      over 1 GbE        41 ms
                                      read from disk  1560 ms
```

**38x in favour of sending the work to the weights.** The protocol is measured
rather than estimated, and the measurement is the deliverable; a second machine
is what would make it about a real network, and this machine is one machine.

### 1024×1024 encodes, because the graph is planned now

`encode_planned` mirrors `decode_planned`: `ggml_gallocr` reuses buffers for
tensors whose lifetimes do not overlap, which is the difference between an arena
that fits and one that aborts. Encoding a 1024×1024 image is possible in this
process for the first time.

### What the prompt advice said, and what it should have said

`chaos-draw --help` and `docs/APP.md` now say **describe the picture, do not
just name it**, with the measurement behind it — and `--template` prints the
description frame for a person to fill in rather than silently wrapping their
prompt in it, because a flag that wrapped it would look like it helped and would
not.

### Fixed

- **`chaos-worker` was not packaged.** The new binary was built and then not put
  in any archive — caught by the release check, before a release rather than
  after one.
- **The release dry run stamped `0.0.16-dryrun` into the Android build file.**
  `[0-9]*.[0-9]*.[0-9]*` is a glob, not a version check: the trailing `*`
  swallowed the suffix. A dry run should differ from a release only in not
  publishing, and a version it would never ship is not that.

### Retracted

- **"Structured, JSON-shaped prompts condition about three times as strongly as
  a bare phrase."** Published in four places, measured on **one latent** — in a
  quantity that varies nineteen-fold between latents. Over eight: a phrase
  wrapped in an empty JSON frame scores **0.9x** the bare phrase, and a phrase
  *written out* — lighting, background, palette — scores **11.3x**. It is the
  sentences, not the braces. The STRUCTURE button that was built on the old
  claim was measured and not shipped.
  `research/prompt-shape-does-nothing-2026-08-24.md`.
- **"If 4 steps looks acceptable, 256×256 gets five times faster."** Never
  published, and wrong: four steps confines the whole image to a **56-level
  grey band** (1st–99th percentile 123–179 out of 255). `chaos-draw` prints the
  standard deviation of every picture it writes and had already printed 12.2 for
  that render. **8 steps is where to start looking**, at 0.4x the default's
  time; the default is unchanged, because ruling one option out does not rank
  the rest. `research/step-count-never-settles-2026-08-24.md`.

## [0.0.16] — 2026-08-24

### Switching to INSTALLED froze the window for a second and a half

Atur: *"when i switch between available and installed models installed models
load with lag and make problem"*. Measured with real clicks into the real
window, warm page cache, 39 models installed:

```
 202  AVAILABLE        6.1 ms
 201  INSTALLED    1,584.4 ms   <-- every single time
 202  AVAILABLE        2.7 ms
 201  INSTALLED    1,535.8 ms
```

Not the list box and not the painting. `rescan()` called `models::list()` on
the UI thread, and that calls `why_incomplete` on every model — which opens
every shard of every container and parses up to 4 MB of header out of each.
`find::list()` is 3.7 ms; `why_incomplete` across 39 models is **1885 ms**.
99.8% of the click, re-derived on every switch to produce an answer that could
not have changed.

A container's verdict is now remembered against the file's own length and
modified time. It cannot gain the bytes it was missing without changing both,
and a download still in flight changes them continuously — so it is re-read
until it stops moving, which is the file a user most needs the truth about.
The scan itself moved to a worker thread, and the list says "looking for
models..." while it runs, because *"nothing installed"* is a claim and before
the first scan it is one nobody has checked.

**After: worst click 10.8 ms.**

### The image generator chooses a model, keeps the latent, and can turn guidance off

Atur: *"why image generator do not have select model options??"* and *"now i
run to draw a image without select any model!! wtf is that lol"*. Both had one
cause: the four filenames were a constant, so there was nothing to choose and
nothing to be missing — DRAW worked with no image models installed at all and
failed minutes later inside the pipeline.

**An image model is four files, not one**: a denoiser, a separately trained
unconditional twin of it for guidance, a text encoder, and an autoencoder.
`chaos_model::image` groups them, the unit chosen is the denoiser, and the
other three are found by role. The filename is the only signal there is:
`ideogram4-Q4_0.gguf` has 458 tensors and **zero metadata keys**.

- `chaos-draw --list-models`, `-m <name>`, and a chooser on the IMAGE page.
- **`--keep-latent`**, on by default from the window. Six hours of correct
  denoising was thrown away once because only the PNG was written. A 128x128
  draw took 21 s; `--from-latent` re-decoded it in **0.7 s** to a
  byte-identical PNG. At 1024x1024 that is seconds against hours.
- **Guidance is a control.** It runs a second 5.26 GiB denoiser on every step,
  so it is exactly a factor of two on the wait; the options say so, and the
  estimate follows the setting rather than assuming guidance is always on.

### The model list sorts, filters, searches, and says what each model is for

Thirty-nine containers in one flat alphabetical list, several of them parts of
an image pipeline rather than models to talk to. Now: sort by name, by size or
by what it is; show everything, chat models or image models; and a search box.
An image part says "image" in its row and LOAD greys out for one.

A clicked row is no longer an entry index — `ui.shown` maps position back to
model. Without that, filtering to image models and pressing LOAD would have
loaded whatever sat at that position in the unfiltered list: a real container,
so no error, just the wrong model.

AVAILABLE now marks what is already downloaded. It said nothing, so a model
fetched ten minutes ago looked identical to one never fetched — and
`flux2-vae.safetensors` is not a GGUF, so it never appeared on INSTALLED
either: downloaded, and invisible in both lists.

### Four of the nine icon sizes were a pixel off centre

Atur asked three times. He was right, and it was one line: `make-ico.py`
centred the mark with `off = (px - inner) // 2`, which floors, so an odd
margin put the whole thing one pixel left of centre and one pixel high — at
16, 32, 40 and 64 px. **16 and 32 are the taskbar and notification-area
sizes**, which is where it was noticed.

Measured on the file that ships rather than on the arithmetic:
`tools/check-logo-centred.py` decodes every frame and reports the four
margins. **4 of 9 frames exactly centred before, 8 of 9 after.** The ninth is
the mark's own geometry, not its placement.

The threshold alone would have hidden this: the broken icon was also "centred
to within 1 px".

### An Android app, shipped as an APK

**A client for a Chaos on a PC**, which is the half that makes the *big*
models usable from a phone. One Activity, framework views, **no androidx and
no dependencies at all** — the Rust side has none either. It reads the phone's
memory and names the largest model that would fit, while saying plainly that
it does not run models yet.

`chaos-serve --host` opens the endpoint to the LAN, and **the api key stops
being optional the moment the socket leaves loopback**: a non-loopback bind
with no key refuses to start, before the model loads rather than four minutes
after.

**It is built in CI because it cannot be built here.** `dl.google.com` — the
sole distributor of the Android SDK, build-tools and androidx — answers 404 to
every request from this network, including a Go download that certainly
exists. So the CI build is the only check the app has had, and the phone is
Atur's. The APK is debug-signed: an unsigned release APK cannot be installed,
and a per-build key would stop Android upgrading in place.

### `--auto` configures itself, and says what to expect first

```
threads    -t 4 to generate, -tb 20 to prefill
batch      -b 512 tokens per prefill block, from 5.5 GiB free
io         direct, bypassing the page cache -- the model does not fit
expect     about 1.42 tok/s -- 1.02 GiB per token at 2.07 GiB/s
```

Measured on that run: **1.51 tok/s**. Within 6%.

It said 4.25 at first, because a token's expert slice was computed as the pool
divided by the layer count — a different quantity that looks plausible and is
3x too small.

The read speed is never measured automatically; the benchmark writes a file
larger than RAM. `chaos-probe --bandwidth` writes what it measured and
`--auto` reads it. With nothing there it states the byte count and names the
command that supplies the rest — a guessed disk speed times a real byte count
is a confident number with nothing behind it.

The I/O mode had to move *before* the container is opened: `Model::open_split`
reads `CHAOS_IO` as it opens each shard, so deciding afterwards printed
"buffered" over a model already opened with direct I/O.

### Every generated image was upside down

Atur's 1024x1024 render — fifty steps, six and a half hours — came out a
coherent, photorealistic portrait, **inverted**. One vertical flip in an image
viewer turns it into a normal photograph.

**Ideogram 4's vertical axis runs bottom-up.** Latent row 0 is the *bottom* of
the picture and takes the highest mRoPE `y`; Chaos numbered it top-down.

Every internal check passed while this was wrong. `try-orientation` was written
to catch exactly this and proved the autoencoder does not flip in either
direction; the token layout matched the position table. All true, all
consistent, and all blind to the one path that matters — **a latent from the
denoiser never passes through the encoder**, so the mismatch cancels in every
test that starts from a real image and in none that starts from noise.

Settled by measurement rather than argument, on a real photograph at four noise
levels. Bottom-up wins **twelve of twelve**:

| σ | cos(v) top-down | bottom-up | cos(−L) top-down | bottom-up |
|---|---|---|---|---|
| 0.90 | 0.7920 | **0.8130** | 0.1974 | **0.2357** |
| 0.70 | 0.7729 | **0.8095** | 0.3285 | **0.3759** |
| 0.50 | 0.7086 | **0.7822** | 0.3346 | **0.4251** |
| 0.30 | 0.6067 | **0.7087** | 0.3522 | **0.4383** |

`cos(−L)` is whether the model can see the image at all: up 24% at low noise.
`x0` error falls on every row. `CHAOS_TOPDOWN_Y=1` repeats the comparison.

This changes nothing about prompt adherence — colour and scene follow, an
object's form may still not. It changes which way up the result is.


### The window says what it is doing

Atur, watching an image render: *"now no model is load in app but image is in
creation lol wtf is that also the progress of image creation is type logs not a
bar progress and monitor do not show anything while i run a image creation"*.

Three symptoms, one omission: `chaos-draw` was a child process the IMAGE page
knew about and nothing else did.

- **The strip reported "no model running"** through ten minutes of a process
  reading 5.26 GiB per step. It is the one surface on every page; it now names
  the draw, the size and the step, with a bar.
- **A progress bar**, on the page and in the strip. The log stays — it carries
  the seconds per step and the time left, which a bar cannot — but a bar is what
  answers *how far along is this*. It reads the step out of what `chaos-draw`
  already prints, so there is one source of truth rather than two.
- **MONITOR has a DRAWING section**: prompt, size, phase, step, elapsed.
- **The IMAGE page says it needs no loaded model.** Drawing opens its own four
  files; CHAT needs a model loaded and this does not, and surprising-and-stated
  is a design where surprising-and-silent is a bug.

### A draw is costed before you start it

Atur chose 1024×1024 at 50 steps and was ninety minutes into a **six and a half
hour** render before any number appeared. The size drop-down said "slow", which
is not a quantity.

The page now says *"about 6.5 hours on this machine"* beside the controls, in
red past an hour, updating as the selection changes. Measured here — 1024×1024
is 4096 latent tokens at 235 s a pass — and **guidance is counted**: it runs the
denoiser twice per step, which is the difference between over-lunch and
overnight.

| | 4 steps | 20 steps | 50 steps |
|---|---|---|---|
| 256×256 | 2m | 10m | 25m |
| 512×512 | 8m | 39m | 1.6h |
| 1024×1024 | 32m | 2.6h | **6.5h** |

### A crash that is not a panic now leaves a note

A Rust panic writes `chaos-app-crash.log` and shows a box. An access violation
does neither — no Rust code runs and the process simply disappears, which is
what "it crashed and there was nothing" looks like from outside. Chaos now
records the fault code and address, then lets Windows Error Reporting have its
turn rather than swallowing it.


## [0.0.15] — 2026-08-21

### The uninstall button asks first

**It had no confirmation at all.** One click and every binary was gone — no
dialog, no undo, and nothing on screen beforehand saying what was about to
happen. It sits on the same screen as UPDATE, which is where somebody who came
to update presses it by mistake and is then left with no Chaos and no idea why.

That is not hypothetical. It happened to Atur.

Now it names what will be removed, says the models are **kept** and where they
are, and offers *"Press No if you meant to update instead."*

### Fixed

- **UNINSTALL is not offered when nothing is installed.** Atur's question was
  exactly right — *"if chaos already in system why uninstall, if not why update
  and not install"*. The primary button already names what it will do (INSTALL
  on an empty prefix, UPDATE over something older); a second button beside it
  that could only do nothing was noise.
- **The installer could hang on a running Chaos.** Closing its window and
  waiting used a plain cross-process `SendMessageW`, which blocks until the
  other window handles it — and a Chaos busy loading a 7 GiB model is not
  pumping messages. The installer would sit there painting nothing for as long
  as the load took, which from the outside is a setup where nothing works. Five
  seconds and it carries on.


## [0.0.14] — 2026-08-21

### The window draws

`chaos-draw` has shipped since v0.0.12 and only a terminal could reach it. The
rail has an **IMAGE** page: a prompt, a size, a number of steps, and DRAW. The
picture lands in `%USERPROFILE%\.chaos\images`; the log carries the real
progress, because this is minutes of work and a spinner would be a lie.

**Spawned, not linked.** A denoiser pass reads 5.26 GiB and an exhausted ggml
arena aborts the process it is in — which, linked, would be the window.

### Fixed

- **The uninstall button, twice over.** The setup window ignored `--prefix` and
  filled its box with the default, so `chaos-setup --prefix D:\Chaos` would
  uninstall `%LOCALAPPDATA%\Chaos` instead — a bug that removes the wrong
  directory, and it did while being found. And neither install nor uninstall
  closed a **running** Chaos: since v0.0.12 closing the window only hides it, so
  `chaos-app.exe` was still locked. Both now ask the app to quit through its own
  Exit, so a loaded model unloads rather than being killed with 7 GiB resident.
- **The taskbar icon was stretched.** The small window icon was hard-coded to
  16×16 while a 125% display asks for 20. Measured with `WM_GETICON`: 16 before,
  20 after. The icon file already carried a 20px entry.
- **An id collision that fired the wrong handler.** The IMAGE page was numbered
  from 601, where `IDM_TRAY_OPEN` and `IDM_TRAY_EXIT` already lived; menu ids
  match first, so the size drop-down meant *quit the application*. Nothing
  failed to compile. The test that exists for this read a hand-written list of
  menu ids nobody had extended — it derives them from the source now.


## [0.0.13] — 2026-08-20

### The workspace has a shape now

`core/` `cli/` `network/` `gui/`, which is what the Rust book's workspace
chapter shows. Fourteen crates used to sit in one flat `crates/` that said
nothing about which of them to read first.

**Crate names did not change** — `core/gguf` is still the `chaos-gguf` package —
so `cargo build -p chaos-gguf`, every binary name and every `use chaos_gguf::`
mean exactly what they did. `chaos-run` and `chaos-serve` became their own
crates, because a `cli/` and a `network/` with nothing in them would be
decoration.

### Fixed

- **Two console windows flashed before the app appeared.** `probe_machine()` ran
  twice during startup and each probe spawns `nvidia-smi`, a console program —
  so Windows made a console for each. Suppressed with `CREATE_NO_WINDOW`, and
  the probe is cached besides: its answer cannot change between two calls a few
  milliseconds apart.
- **"ideogram models not loaded and nothing works."** They load fine —
  `chaos-draw` was checked end to end while investigating. What the app *said*
  when you pressed LOAD was written before the pipeline existed: *"Image
  generation is being built."* It ships in the release that message was printed
  by. All four image entries now name the command that runs them.
- **Nine of eleven binaries did not answer `--version`**, and several treated the
  flag as a filename — `chaos-model-info --version` reported "cannot find the
  file specified", and `chaos-gpubench --version` started benchmarking. All of
  them answer now, checked before any argument is taken as a path, with a test
  that walks every `src/bin/*.rs` in the workspace.


## [0.0.12] — 2026-08-20

### The app updates itself

Atur: *"users can get the most updated release when they connect to the internet
from the app — an updating flow, not every time go and download a new setup. For
all apps and exports we need."*

- **Chaos asks once at startup whether a newer release exists**, and says nothing
  unless one does. If one does, it offers to fetch the installer and hand over.
  `Help ▸ Check for updates` asks on demand and always answers; `Help ▸ Install
  update…` does the download. `CHAOS_NO_UPDATE_CHECK=1` turns the automatic
  check off.
- **`chaos-run --update` does the same from a terminal**, because the window is
  one of twelve binaries a release ships and a CLI user should not have to open
  a GUI to learn that a release exists.
- **One update updates everything.** The installer carries the whole payload —
  the window, `chaos-run`, `chaos-serve`, all of it — so there is nothing to do
  per binary. Your models are not touched.
- The decision logic is in `chaos_model::release` and is tested against
  **GitHub's actual response**, kept as a fixture: `0.0.9` must not look newer
  than `0.0.11`, and a macOS tarball must not be downloaded onto Windows.

### Chaos stays running when you close the window

Atur: *"chaos run in background well when app closed, that chaos must be in
small bar in every device and show there as running to the user; now chaos
always run in background and just finish work with exit button."*

- **The X hides the window.** The model stays loaded, the endpoint stays up, and
  Chaos moves to the notification area. A model can take four minutes to load,
  and throwing that away because somebody closed a window is the wrong default.
- **The icon says what is running** — hover it for *"Chaos — qwen3-4b is
  running"*. Background running you cannot see is indistinguishable from an app
  you forgot to close, and an engine holding 7 GiB with nothing on screen is a
  bug this app has had once already.
- **Right-click it** for Open, Stop *model*, and Exit. **Exit is now the only
  thing that stops the engine** — not the X, not the taskbar's close.
- The first close explains itself with a notification, once per run.
- **One Chaos at a time.** This became necessary the moment closing stopped
  quitting: with the window hidden, double-clicking the shortcut would have
  started a second engine holding a second model's worth of memory, with the
  first one invisible. A second launch now finds the first, brings its window
  back and stops.

On Windows 11 a new tray icon starts behind the `^`; that is where the system
puts every one of them, and pinning it is a Windows setting rather than
something an application can do for itself. `docs/APP.md` says where.

### Pointing an agent at Chaos, written down

The endpoint and the API key were already there; what was missing was the page
that says how to use them. `docs/APP.md` now has the base URL, the key, and what
to put in the "OpenAI-compatible endpoint" box of Hermes, Claude Code, Continue,
Aider or Zed — plus the fact that, because the window keeps the engine alive in
the background, an agent can go on using it with no Chaos window on screen.

### Five builds, eight assets

Two platforms that had nothing at all:

- **Intel Macs.** `macos-latest` is Apple Silicon, so every macOS release so far
  was arm64 only.
- **arm64 Linux** — a Raspberry Pi, an Ampere box, an arm64 VM.

The updater picks by **architecture as well as operating system** now. An arm64
tarball on an Intel Mac is the same silent failure as the wrong OS one level
down: it downloads, it unpacks, and then nothing runs. A test pins the five
names against the five the release workflow builds, because the updater matches
by string equality — a matrix entry renamed in one place and not the other is an
updater that says "no installer for this platform" forever, on that platform
only, with nothing in a log.

### Images ship

`chaos-draw` is a **binary** now, not an example held back behind a comment that
said it would become one "when there is evidence, not before". The evidence:
the autoencoder round-trips real photographs at **36.09–40.89 dB** with the check
ablated three ways first, the text encoder answers `" Paris"` at **logit 22.58**,
the denoiser scores **0.85 velocity cosine** against a real latent, and at
1024×1024 the output is photorealistic — skin texture, individual hairs,
catchlights.

```
chaos-pull ideogram-4 ideogram-4-uncond qwen3-vl-8b flux2-vae
chaos-draw "a red apple on a white table" --grid 64 --steps 20 -o apple.png
```

**What is still imperfect is object form**, and `--help` says so rather than
leaving it to be discovered after two hours: colour and scene follow the prompt,
a named object may come out the wrong shape. Structured, JSON-shaped prompts
condition about three times as strongly as a bare phrase, which is what these
models were trained on.

It ships in every artifact, so a release now carries twelve binaries.

### The icon, drawn from the vector at every size

`make-ico.py` renders `assets/logo.svg` at each of the nine sizes Windows asks
for. It now does so with **8 subsamples instead of 3** — that is the number of
grey levels an antialiased edge can take, and this mark is two dozen rays about
one pixel wide, so nine steps was what "blocky" looked like — and with a **4%
inset rather than 8%**, which at 16px is the difference between 13 and 15 pixels
of drawing.

### The window, after Atur used it

Six reports, each one a real defect:

- **The rail lit every page at once.** Invalidating the parent window does not
  repaint owner-drawn children, so each rail item lit itself when clicked and
  nothing ever un-lit the last one — click all four and all four are
  highlighted. `View ▸` and `Ctrl+1..4` were worse: they changed the page and
  left the rail pointing at the old one.
- **Drop-downs opened too narrow to read.** A Win32 drop-down is exactly as wide
  as its box unless told otherwise, so *"Processor (the GPU is not used here
  yet)"* opened as *"Processor (the GPU is not used her…"*. The open list is now
  measured against its longest option, capped to the screen.
- **BROWSE sat on top of the sentence explaining the models folder.** Two
  layout walkers stepped by different amounts; they now share one.
- **The mark in the rail was rough.** It is a sun of two dozen fine rays around
  an eye, drawn at 44px with 16 levels of antialiasing — about one level per ray
  edge. Now 64px at 64 levels, scan-converted from outlines and cached.
- The models list gives the name its own column, so a narrow window eats a
  measurement rather than the end of `Qwen3-VL-8B-Instruct-Q4_K_M`. The
  **AVAILABLE** tab has columns too, and it was worse there — drawn as one
  string, "needs 16.5 GB - slow, re-reads" came out "needs 16.5 GB - sl…",
  cutting the one word the row exists to say. The list also got wider, and the
  resident requirement is now printed **only when it differs from the download
  size**: on a dense model they are the same number, so twenty rows out of
  twenty-seven were spending a column to repeat themselves.
- The window is DPI-aware (per-monitor v2) and opens centred in the work area
  rather than off the corner of a scaled display.

## [0.0.11] — 2026-08-20

### Chaos draws images

The whole path now runs: **Qwen3-VL-8B** turns the prompt into conditioning,
two copies of **Ideogram 4** — the conditional model and a separately trained
unconditional twin, 5.26 GiB each — denoise a latent, and the **FLUX.2**
autoencoder turns it into pixels. Four files, 16.7 GB.

**It follows the prompt's colour and scene and gets an object's form wrong.**
Said plainly, because a diffusion pipeline that is subtly wrong produces a
plausible picture: at 512×512 *"a red apple on a white table"* drew red on white
surfaces beside a wooden shelf, and not an apple. The foundation is verified;
the pictures are not good yet.

What *is* verified, and how:

- **The text encoder** answers *" Paris"* after "The capital of France is" at
  logit 22.58 — so its attention, rotary positions, per-head QK norm,
  grouped-query broadcasting and causal mask are all right.
- **The denoiser** is scored without looking at a picture. A rectified-flow
  model at noise level `sigma` must predict `noise - latent`, and **both terms
  are known** because the autoencoder's encoder — verified to 36 dB by round trip
  in v0.0.10 — turns a real photograph into a real latent. Cosine against the
  truth is **0.85 at 512×512**.

Scoring against each half separately — "can it see the noise" against "can it
see the image" — is what found three bugs a picture could not have told apart.
**Two of them are in the reference implementation**: `stable-diffusion.cpp`
reads `1.f / 128.f` as an attention scale when it is an F16 overflow guard that
cancels exactly, and it never applies the autoencoder's own latent
normalisation. Being faithful to a reference and being right are not the same
thing.

### 1024×1024 stops being impossible

Chaos runs models larger than memory by streaming them; the image path did not.
Both halves built each graph in a context whose arena allocates every tensor and
frees none, so they paid for every intermediate ever written while the live set
is a handful. Decoding 768×768 wanted **29.5 GiB** and aborted the process after
an hour of denoising.

`ggml_gallocr` plans a graph and gives the same buffer to tensors whose
lifetimes do not overlap. The *device* path here always did this; the host path
never did.

| | before | after |
|---|---|---|
| decode 256×256 | 3.69 GiB | **0.20 GiB** |
| decode 1024×1024 | 52 GiB | **3.4 GiB** |
| denoise 1024×1024, per layer | 14.6 GiB | **2.0 GiB** |

**Bit-identical, not merely close**: 0 of 196,608 pixels differ, and the
denoiser's velocity score is unchanged to four decimals.

### The app says how far a model has loaded

It said "loading — a large model takes a while" and then nothing, which on a
144 GB container is minutes of a window that looks broken. Now:

```
loading qwen3-4b  62%  ·  1.7 GB of 2.8 GB resident  ·  340 MB/s  ·  3s left
```

Measured from the server's working set against the catalogue's resident figure,
the same way the download bar is measured from bytes on disk — no protocol, no
parsing, and it survives a server started with no console. **The bar stops at 99
until the server answers**, because the tail of a load is not weights and a full
bar over a busy app is a lie. A model the catalogue does not know gets bytes and
**no percentage**: a denominator taken from the file size would report V4-Flash
as 5% loaded for its whole load.

### Fixed

- **`chaos-pull` accepted a corrupt download.** A resume the CDN did not honour
  made curl append the whole file to the partial one; the result was 478,535,680
  bytes **too large**, `saturating_sub` floored the remainder to zero, and it
  reported "already complete". The container passed every structural check —
  being too *big* means every tensor offset is readable — and produced NaN at
  block 31 of 36. Now refused by name, and the size is verified after
  downloading.
- An image too large to decode is refused **in 34 milliseconds, before anything
  is loaded**, naming the largest size that works — rather than aborting inside
  ggml after an hour of denoising.


## [0.0.10] — 2026-08-20

### Chaos decodes images, and the autoencoder is verified rather than eyeballed

`chaos-image::vae` builds the FLUX.2 autoencoder as a ggml graph — both halves,
248 tensors, and **no transposes anywhere**, because a contiguous PyTorch
`[OC, IC, KH, KW]` convolution weight *is* ggml's `[KW, KH, IC, OC]` read
backwards. The decoder turns a latent into pixels; the **encoder exists so the
decoder could be checked without a reference implementation**.

Encode a real photograph, take the distribution's mean, decode it, compare. The
two halves are separately trained weights over one shared latent space, so
neither can compensate for a bug in the other. Four 256×256 photographs:
**36.09, 36.29, 36.49 and 40.89 dB**.

That is only worth printing because the check was ablated first. Against the
same input, baseline 36.09 dB:

| deliberate error | PSNR |
|---|---|
| `group_norm` without its per-channel scale and shift | 16.77 dB |
| downsampler padded symmetrically instead of `(0,1,0,1)` | 14.60 dB |
| mid-block attention skipped | 31.93 dB |
| convolution kernels not dimension-reversed | ggml aborts |

Three of the four still produce a recognisable picture, and all three would have
passed "it looks like a photograph" — which is this project's oldest hazard.

**The denoiser is not started**, so Chaos cannot yet generate an image from a
prompt; it can decode a latent into one. The text encoder and the unconditional
twin are fetchable but not yet fetched.

New ggml bindings, each exercised in `try-vae-ops` against a hand-computed
answer before going near the suite: `new_f32_4d`, `upscale_nearest`, `pad`,
`cont_2d`, and a `Tensor::ne` accessor. `ggml_pad` puts its zeros at the far end
of each dimension only, which is exactly diffusers' asymmetric downsample
padding.

### The file has 251 tensors and the graphs name 248

The remainder is a BatchNorm — `bn.running_mean`, `bn.running_var`,
`bn.num_batches_tracked` — holding the **latent normalisation**, which is what
earlier autoencoders did with a scalar `scaling_factor`. It is **128-wide, not
32**: the patchified channel count, 32 latent channels times a 2×2 patch, which
is exactly what the denoiser consumes.

A round trip never touches it, because encode and decode are inverses whatever
the normalisation is — which is what makes the round trip a fair test of this
port and *not* a test of the interface to the denoiser. Recorded because leaving
it out is the next chance to produce a confident, plausible, wrong image.

### `usable without ggml` had been red, and was reported as green

The check failed from the commit that added `try-vae-ops` onwards: `cargo clippy
--all-targets` builds examples, and that example names `Context`, which is
`#[cfg(have_ggml)]`. Four of five checks green was read as green. The gated
*tests* in that crate use `#![cfg(have_ggml)]` on the whole file; an example
cannot, because a file with no `main` does not compile at all, so the cfg goes on
the function with a stub that says what to set.

### Fixed

- The new autoencoder tests panicked when the 336 MB file was absent, which is
  what they were written to do and which turned three CI jobs red — CI runs
  `cargo test --workspace -- --ignored` on three platforms that do not have it.
  They skip now, the way the V4-Flash set does. Because that is the hazard this
  project has already been burned by, the skip prints **"SKIPPED, NOTHING WAS
  CHECKED"** and names the file, and `CHAOS_REQUIRE_VAE=1` turns absence back
  into a failure.

### Note on 0.0.9

There is no v0.0.9 release. The version was bumped in `Cargo.toml` and the
installer was exercised locally, but the tag was never pushed, so v0.0.8 was the
newest release until this one. Everything that section would have contained ships
here.


### Qwen3.6-27B loads, generates, and is wrong — in llama.cpp too

Found while checking that every installed model runs. It exits 0 and prints
`ทัน ทัน ทัน ทัน ทัน ทัน`. Qwen3.5-0.8B — the same architecture at 24 layers
instead of 64 — is byte-identical to llama.cpp at three prompt lengths, so
`qwen35` is verified on the shape it was diffed against and not on this one.

llama.cpp fails on the same file too — `333333` — and the per-layer sums agree to
five significant figures through layer 5 before both overflow to NaN. **That is a
clue, not a defence**: reproducing a wrong answer precisely is still a wrong
answer, and this engine is judged on whether it replies correctly. What the
agreement buys is knowing where not to look. Chaos warns before generating, and
the warning names no other project.

The first sweep of all twelve models recorded "twelve of twelve" because every
one exited 0 and the outputs were never read. That is this project's own
documented hazard walked into by a test that checked liveness instead of
correctness. The sweep now requires the word `Paris` in the continuation of
"The capital of France is".

Ruled out by measurement, not reasoning: the key-head broadcast (the 27B has 16
key heads and 48 value heads where the 0.8B has 16 and 16, so a missing
broadcast would be invisible there — a new test calls the fused op at a 2:6
ratio and proves it broadcasts on its own), and every tensor shape in both
containers against what the config computes. Open, with the next three
experiments written down, in `backlog/qwen35-27b-is-wrong.md`.

### Loading a second model sent your messages to the first one

The window never stopped the running server before starting the next. So
loading a second model spawned a second `chaos-serve` while the first still
held the port: the new process died with `os error 10048`, the readiness check
took its 200 from the **old** server, and the window said the new model was
ready while every message went to the old weights. The dead child's handle
overwrote the live one, so nothing could kill the orphan either — it kept its
memory, which on a 15.7 GiB machine running a 144 GB model is the difference
between 0.45 tok/s and nothing at all.

Both halves were reproduced before the fix and re-checked after. The window now
stops the server first, waits for the port to be released, watches the process
rather than only the port, and shows the server's own reason when one exits —
that sentence used to be written to a console the app does not have.

### Four settings that did nothing

`chaos-serve`'s argument parser swallowed anything it did not recognise. It had
no `-ngl`, no `-c`, no `--auto` and no `--force`, and the app sent all four — so
the GPU list, the context list and the AUTO and FORCE toggles were decorations
for three releases.

- An unknown flag is an error now, in `chaos-serve`, `chaos-probe`,
  `chaos-pull` and `chaos-model-info`. A flag is a promise.
- `-c` works, and only ever *lowers* the limit: a ceiling above what the engine
  can hold is a promise broken mid-request.
- `--force` works, and says on every start that the architecture is unverified.
- `--auto` sizes the expert cache from the memory actually free.
- `-ngl` is refused **by name, with the reason**: the server binds weights
  straight into host memory rather than through the runner's device loader. The
  GPU list now says the window runs on the processor and that `chaos-run -ngl
  99` uses the card. One true option beats three that do nothing.

### The GPU on V4-Flash was accepted and ignored

`--device` and `-ngl` on the streaming expert path changed nothing and printed
nothing, so "I turned the GPU on and it does no work" was an accurate
description of the program. `chaos-run` now says so, with the reason. Not built
rather than not wanted: V4-Flash's always-read set is 7.38 GiB against 5.11 GiB
free on this card, and the only measured figure for a streaming MoE on this
device is **4.3x slower** than the host path.

### The uninstall could not finish

Three causes, each sufficient on its own:

- **A dialog outlived the helper's patience.** An uninstall started from inside
  the install folder stages a copy of itself in `%TEMP%`, because a running
  executable keeps its own file open. The parent then showed a modal and only
  quit once dismissed — while the helper retried for ten seconds and gave up.
  The helper now waits for its parent by process handle and reports the result
  itself.
- **`version.txt` was never removed.** The install wrote it; the uninstall
  removed the manifest and the log and not this, so the folder could never be
  deleted and a stale version file was left claiming Chaos was installed. Found
  on a real machine, not in a test. Both ends now read one list.
- **Add/Remove Programs opened the installer.** `UninstallString` carried no
  arguments, so Windows Settings showed the welcome screen with INSTALL as its
  primary button. It now asks once and removes. `QuietUninstallString` is there
  too, for `winget uninstall`.

### An update looked exactly like a first install

The upgrade line existed and was only ever written into the report — *after* the
install had run. The welcome screen now reads the chosen folder first and says
"Chaos 0.0.8 is installed here. This updates it to 0.0.9.", with the button
relabelled UPDATE, or REINSTALL for the same version. It re-reads when the path
is edited.

### Eight of ten binaries had no icon, and Explorer kept showing the old one

`chaos-app` and `chaos-setup` each carried a private copy of the same `windres`
work, so the other four crates that build binaries had nothing to include:
`chaos-run.exe`, `chaos-serve.exe`, `chaos-pull.exe`, `chaos-probe.exe`,
`gguf-info.exe` and the benches all shipped with the blank Windows default. One
`chaos-build` crate now holds it — a build dependency, linked into nothing.
Verified by extracting the icon from all ten executables: one hash, ten
binaries.

And the file being right was not enough. Explorer caches an icon by path and
does not re-read a file overwritten in place, so an upgrade kept showing the
previous version's icon. The installer now tells the shell otherwise, which is
what every installer does at the end of a copy.

### The mark was too small to read

The icon's art filled the middle 68% of its tile; at 32px that leaves the sun's
rays one pixel wide and the eye invisible. Now 84%. The app's rail mark went
from 32px to 44 in a 208px rail, and the installer's welcome mark from 68px to
96 in a 900px window, where it had less presence than the wordmark beneath it.
Nothing was resampled to get there — the art is filtered from the 256px master
at whatever size is asked for.

### The logo lost its eyes

**An SVG path is one region, however many subpaths it has.** With no
`fill-rule` the default is nonzero, so a subpath winding the other way is a
*hole* — and one path in this mark is a near-white shape whose hole is what the
eyes show through. The generator emitted every subpath as its own shape and
filled each independently, so the hole was filled too. 43 paths producing 44
polygons was the tell, and `assets/logo.png` — rendered by a different tool that
collects a path's edges before scanning — had always shown the eyes. Two
renderers of the same art disagreeing was the whole bug.

### Release assets are named one way

v0.0.8 shipped a mix of `chaos-` and `Chaos-`, and the AppImage was the only
asset that did not say which platform it was for. Now one prefix, one version
format, and platform and architecture on everything — with the `.deb` keeping
Debian's mandated `name_version_arch.deb`, because an installer that will not
install is not tidier. The README lists all five downloads with what to do with
each, and says plainly that the window is Windows-only.

### Mojo and xtool, asked and answered

Both in `research/mojo-and-macos-packaging-2026-08-19.md`.

**Mojo** is genuinely open source now — Apache 2.0, compiler included, CUDA,
HIP and Metal. It is also a language, not a memory manager: deciding which of
256 experts stay resident is policy, and that policy is what this project *is*.
What it could replace is ggml's kernels, and the measured numbers say that is
not where the time goes — the routed expert arithmetic is under 5% of a
V4-Flash token, which is 1.56 s of disk read. It would also be the first
external toolchain here, and Modular is not accepting compiler contributions
until the end of 2026.

**`xtool`** builds *iOS* apps with SwiftPM. This is a Rust project and the ask
was macOS, so it is the wrong tool rather than a tool used wrongly. A macOS
build needs the Apple SDK on Apple hardware — which the release workflow
already has, on a `macos-latest` runner. The `.deb` and the AppImage already
existed too, verified in CI with a real `dpkg -i` and a PATH check.

### The Qwen3.5 forward pass is implemented and diffed against llama.cpp

`qwen35` — Qwen3.5, Qwen3.6 **and Qwen3.8**, all one architecture — now runs
end to end. Developed against `Qwen3.5-0.8B-Q8_0` because it is the same
architecture at 24 layers instead of 64, so a wrong step shows up in a second
rather than a minute.

**Verified, not asserted.** Every layer's output compared against
`llama-eval-callback` on the same container and prompt, by value *and* by sum
over all five prompt tokens:

```
l_out-0   llama -0.3452   chaos -0.345155    sum -4.384898 / -4.384897
l_out-3   llama -0.2049   chaos -0.204932    (the first attention layer)
l_out-23  llama  0.3155   chaos  0.315493    sum 26.697807 / 26.697754
```

**All 24 layers agree**, and the sampled token ids are llama.cpp's exactly:
`" Paris"`, `"."`, `"
"`, `"The"`, `" capital"`, `" of"`.

The sums matter as much as the values: the delta net is recurrent, so a state
carried wrongly leaves token 0 perfect and every token after it wrong. Comparing
first rows alone said the port was correct while the answer was still garbage.

What it took:

- **The gated delta net** (`crates/chaos-arch/src/qwen35.rs`) — 18 of the 0.8B's
  24 layers, 48 of the 27B's 64. Projections, a rolling depthwise convolution,
  `l2_norm` on q and k, `ggml_gated_delta_net`, a gated norm, an output
  projection. The state is host-side like the KV cache, so no in-graph cache
  writes.
- **mRoPE** for the attention layers, and **four position values per token** —
  ggml asserts `ne[2] * 4 == b->ne[0]` and aborts otherwise.
- **The fused query/gate projection**, interleaved *per head*: head 0's query,
  head 0's gate, head 1's query. Two contiguous halves would take the queries of
  the first heads and the gates of the last, silently.
- **The `qwen35` pre-tokenizer** — one digit at a time, and combining marks
  belonging to the word rather than the punctuation beside it. Checked against
  `llama-tokenize`.

### Three detections that read a familiar name and got the wrong architecture

All three found by the layer diff, all three now testing for what they mean
rather than for one tensor name:

| flag | what went wrong |
|---|---|
| `parallel_residual` | absence of `ffn_norm` meant "one norm, parallel block". `qwen35` has two norms and calls the second `post_attention_norm`, so the FFN consumed `attn_norm(x)` — the value attention had already seen. The symptom was `attn_post_norm-0` coming out **identical to llama.cpp's `attn_norm-0`**, which is what named the bug. |
| `post_norms` | presence of `post_attention_norm` meant Gemma. Gemma's post-norms are a *pair*; demanding the missing `post_ffw_norm` refused a loadable model. |
| `fused_qkv` | presence of `attn_qkv` meant Phi-3. On `qwen35` that tensor is the delta net's input projection, and attention layers have no such tensor at all. |

### The bug is found: a tensor read back that was never computed

The attention layers' **output gate** is a *sibling* view of the `attn_q` matmul,
not an ancestor of q, k or v — so a graph rooted at those three never computed
it, and `to_vec_f32` returned whatever the reused scratch arena held. The
previous layer's leftovers.

That explains every symptom:

- Layers 0–2 are recurrent and have no gate, so they matched llama.cpp exactly;
  layer 3, the first attention layer, was wrong.
- Any extra compute anywhere changed the leftovers and so changed the answer —
  which is why `CHAOS_DUMP_LAYERS=1` looked like it *fixed* the model.
- Bisecting the debug computes pointed at the dense-FFN pass, three phases away
  from the actual fault. The trigger and the bug were in different places.

The fix is one line of intent: **the gate joins q, k and v as a root of both
`realize_graph` and `run`.** `qwen35.rs` carries a textual regression test for
it, because the failure is structural — it lives in which tensors `stream.rs`
hands to the compute.

### Verified, with the instrumentation off

| check | result |
|---|---|
| all 24 layers, by value and by sum | match `llama-eval-callback` |
| 1-token prompt `"Paris"` | `", France.\nThe first time I"` — identical |
| 5-token prompt | `" Paris.\nThe capital of France is"` — identical |
| 22-token prompt | `"</think>\n\n The sky appears"` — identical |
| debug dump on vs off | **same output** |

The three prompt lengths are not decoration: the fused delta rule takes a
different path at one token than at many, so both regimes are covered.

**`qwen35` is in `VERIFIED_ARCHITECTURES` and `RUNNABLE_ARCHS`** — Qwen3.5,
Qwen3.6 and Qwen3.8 all report it, and `chaos-run` and `chaos-serve` take them
without `--force`. `qwen35moe` stays out: no MoE container of the family has been
run, and its refusal now names what is *untested* rather than what is
unimplemented.

### Three detections that read a familiar name and got the wrong architecture

All three found by the layer diff, all three now testing for what they mean
rather than for one tensor name:

| flag | what went wrong |
|---|---|
| `parallel_residual` | absence of `ffn_norm` meant "one norm, parallel block". `qwen35` has two norms and calls the second `post_attention_norm`, so the FFN consumed `attn_norm(x)` — the value attention had already seen. The symptom was `attn_post_norm-0` coming out **identical to llama.cpp's `attn_norm-0`**, which is what named the bug. |
| `post_norms` | presence of `post_attention_norm` meant Gemma. Gemma's post-norms are a *pair*; demanding the missing `post_ffw_norm` refused a loadable model. |
| `fused_qkv` | presence of `attn_qkv` meant Phi-3. On `qwen35` that tensor is the delta net's input projection, and attention layers have no such tensor at all. |

### Not finished, and the reason is worth reading

**`CHAOS_DUMP_LAYERS=1` changes the answer.** With it set the sampled ids are
llama.cpp's; without it they are wrong. Same binary, same prompt, `--temp 0`.

So the layer diff above is real *and* insufficient: the debug pass computes extra
tensors in the same arenas, which changes when a buffer is written, and the plain
path reads one at the wrong moment. **A model whose output depends on whether it
is being observed is not verified**, so `qwen35` is out of
`VERIFIED_ARCHITECTURES` and `RUNNABLE_ARCHS` — it was briefly added during this
work and has been taken back out.

Ruled out by measurement: thread count (1, 2 and 4 all identically wrong),
`--force`, computing the layer twice, `scores` as an explicit graph root, `cont`
copies of both carried states, and a `cont` copy of the convolution's input. The
delta net's *own* debug pass makes no difference either — so the trigger is one
of the three extra computes in the **attention** and dense-FFN phases, and the
first suspect is the strided fused query/gate views. The bisect and the next step
are written down in `docs/graph/backlog/qwen35-gated-delta-net.md`.

`CHAOS_DUMP_LAYERS=1` prints the residual stream per layer in
`llama-eval-callback`'s own format and names, which is how all of the above was
found.

### Qwen3.8-27B is in the catalogue, and it is Qwen3.6's architecture

Read from the container, not inferred: `general.architecture qwen35`, 866
tensors, 51 metadata keys, 17.9 GB dense. Upstream it is
`Qwen3_5ForConditionalGeneration` with a vision tower and a separate 928 MB
`mmproj`. Asking for the newer model does not route around the gated delta net —
it adds to it. A test asserts 3.6 and 3.8 agree on the architecture.

### The delta rule is one ggml op, and it is bound

`ggml_gated_delta_net` performs the whole chunked delta scan and returns the
carried state with the scores. It is present in the pinned `ggml-base.a`, and it
is now bound along with `ggml_ssm_conv`, `ggml_l2_norm`, `ggml_rope_multi` and
the 4-D view/reshape/cont/repeat helpers.

**Three numeric tests rather than a compile check**, because a wrong FFI
declaration mis-reads its arguments and returns confident numbers: `l2_norm`
takes four 2s to 0.5 where `rms_norm` would give 1.0; `ssm_conv` sums its rolling
window to 10 then 14; `gated_delta_net` returns `S*H*T*N + S*S*H*N` finite values
with the state moved off zero.

**Qwen3.8 does not run yet** — the layer wiring, the recurrent state cache and
the llama.cpp diff are still to come.

## [0.0.8] — 2026-08-19

Every dropdown on the settings page was two pixels tall. A model in its own
folder was invisible. A download that stopped half way looked finished. All
three were reported by Atur in one message, all three are fixed, and all three
now have a test.

### The settings dropdowns open

**Measured, on this machine: the dropped list was 32 pixels tall in 0.0.7 and
is 238 in 0.0.8.** A combo box in Win32 is sized by the height of its *dropped
list*, not of its closed box, and `layout` was passing it the row height. The
options were all there — `CB_GETCOUNT` reported three to six of them — and the
list they opened into had two pixels of room after the closed control took its
share. Clicking one looked exactly like a control that ignores the mouse.

`metric::COMBO_VISIBLE` now sizes them for eight rows, and the constant carries
the reason so nobody sets it back.

### Models are found where models are actually kept

- **The `models folder` setting is read by the engine.** It was written to
  `settings.txt` by the app and consulted by nothing — the worst of the three
  possible behaviours, because it looked like it worked. `chaos_model::find`
  reads it now, so `chaos-run` and `chaos-serve` honour it too.
- **Several folders, separated by `;`.** A 144 GB container does not live beside
  a 2 GB one. Both `CHAOS_MODELS` and the setting take a list, in the platform's
  own separator.
- **One level of subfolders is searched.** A five-shard model lives in its own
  directory; a scan that stopped at the top level reported "no models installed"
  with 145 GB plainly there. This is how DeepSeek-V4-Flash became visible in the
  app without moving a byte.
- The search order holds each directory exactly once. `Vec::dedup` removes only
  *adjacent* duplicates, so the cache directory used to appear twice.

### An unfinished download says so

A truncated `.gguf` has a valid header — the header is written first — so it
listed beside models that work and failed several seconds into a load, in the
engine's words. Three of the models on Atur's machine were in that state.

`chaos_gguf::Gguf::expected_file_bytes` reads the container's own tensor index
and returns where the last tensor ends; a file shorter than that is *provably*
truncated, with no catalogue, no network and no guess involved. Every shard is
checked separately.

- **MODELS** marks such a row `(unfinished)`.
- **LOAD** refuses, naming the shortfall, and points at DOWNLOAD — which
  resumes.
- `chaos-run` and `chaos-serve` refuse the same way, before anything is bound:
  `the download did not finish -- phi-4-Q4_K_M.gguf is 2.48 GB short of the
  8.89 GB its own index requires (72% written)`.

### Ideogram 4 is in the catalogue, and listed as an image model

It is open-weight — a 9.3B diffusion transformer, GGUF conversions and all — so
"we cannot get it" was never true. What is true is that generating an image
needs a sampler loop, a separate text encoder and a VAE, and Chaos is a token
loop with none of the three. The container says as much itself: **458 tensors
and zero metadata keys**, so there is no `general.architecture` to dispatch on.

Listed, sized from the repository, and refused with the reason. The route in, if
it is ever wanted, is `docs/graph/backlog/image-generation-ideogram-4.md`.

### Why Qwen3.5/3.6 cannot run, stated properly

The old refusal blamed interleaved multimodal RoPE. That is real and it is the
smaller half. Read from the container itself: `full_attention_interval 4` with
64 blocks means **48 of the 64 layers are a gated delta net with recurrent
state**, carrying `ssm_conv1d`, `ssm_a`, `ssm_alpha`, `ssm_beta`, `ssm_dt.bias`
and `ssm_norm` — and a KV cache cannot stand in for a carried state.

The refusal now says that, and `docs/graph/backlog/qwen35-gated-delta-net.md`
writes down the four pieces of work and how they get verified.

### Phi-4 runs: the `dbrx` pre-tokenizer

Phi-4 downloaded completely and then refused to load — `tokenizer.ggml.pre =
"dbrx" is not implemented`. Its expression is `llama3`'s byte for byte, and
llama.cpp says so in a comment above it.

**It is still a separate variant rather than another name in the `llama-bpe`
arm**, because llama.cpp's `llama3` branch also sets `ignore_merges` and
`add_bos` and its `dbrx` branch sets neither. Phi-4 declares no
`tokenizer.ggml.add_bos_token`, so aliasing it would have prepended a BOS from
the default and shifted every position by one — silently, the way tokenizer bugs
always fail here.

Checked against `llama-tokenize` on the container: six cases, exact ids, and
eleven tokens with no BOS for the test sentence. `chaos-run phi-4 --chat` then
answers *"The capital of France is Paris."*

**`smaug-bpe` shares llama.cpp's arm and is still refused**, because there is no
Smaug container here to check it against. Identical in the source is not the
same as verified.

### Verified by running them

Every model on Atur's machine was run through the **installed** v0.0.8 — what
he has, not what the repository has.

**Ten of the eleven generate correct text**: Llama-3.2 1B and 3B,
Qwen2.5-Coder-7B, Qwen3-4B, Qwen3-8B, Qwen3-14B, Gemma-3 4B, 12B and 27B, and
Phi-4. **DeepSeek-V4-Flash generates too** — *" Paris."*, 0.340 tok/s, 5 prompt
tokens in 8.5 s — from `C:\Projects\models\v4flash`, without a byte being
moved.

The eleventh is Qwen3.6-27B, refused by architecture with the reason. Nothing
crashed and nothing hung.

## [0.0.7] — 2026-08-19

The window redesigned around four pages, an installer that looks like one, and
a mark drawn from vectors. Every resident figure in the catalogue is now
measured rather than estimated — including the one this project's headline
claim rests on.

### The app is four pages, not one panel

Atur's verdict on 0.0.6 was exact: *"why is all click in one slot"*. Everything
had been on one screen because the window grew a control at a time.

- **CHAT · MODELS · MONITOR · SETTINGS**, reachable from a navigation rail, a
  real **menu bar**, or `Ctrl+1`..`Ctrl+4`. One page owns the screen.
- A **strip on every page** carries what is running, its endpoint, its
  throughput and STOP.
- **A page per model** rather than a row: status, endpoint, context, threads,
  cache, uptime, tokens served.
- **SETTINGS exposes all nine fields** the file holds; the old window showed
  three. The five that need knowledge are **dropdowns generated from your
  machine** — core count, free memory, whether a GPU exists — each option
  carrying one line on what it costs, and "Measured" always first.
- Light and dark, following Hermes' desktop palette with Atur's `#0000F2`.

### The installer looks like an installer

Navy `#0d2f86`, cream `#ffe6cb`, a tracked display-serif wordmark and one
bordered action — Hermes' own installer values, read from its source. The
install is **twenty named steps on a worker thread** with a progress bar, ticks
and per-step timings, instead of a frozen window that returned a paragraph when
it was over. It also has a panic hook and writes `%TEMP%\chaos-setup.log`.

### The mark is geometry

`assets/logo.svg` ships as **44 polygons, 11,094 points**, filled by a scanline
rasteriser at whatever size is wanted. No bitmap anywhere. The app icon is a
real rounded square with an alpha channel — it was RGB before, so a rounded
corner had to be filled with something and any fill read as a square.

### Added

- **An API key that is actually checked.** `chaos-serve --api-key` requires
  `Authorization: Bearer` on `/v1/*` and answers 401 in the shape an OpenAI
  client expects. Off by default; `/health` is never gated. Generated from the
  system CSPRNG, not the clock.
- **Model → Test the connection** makes the three requests an agent makes and
  reports each. `docs/AGENTS.md` carries recipes for Hermes, aider and the
  OpenAI-compatible provider every editor extension has.
- **Download progress**: percent, bytes, rate and time left, measured from the
  files on disk so a resumed fetch reports correctly.
- **Linux packages**: a `.deb` and an AppImage, built from coreutils alone and
  verified in CI by installing the deb, running `chaos-run` off PATH, and
  executing `chaos-probe` out of the AppImage.
- **`tools/gguf-always-read.py`** reads a container's tensor table over HTTP and
  computes its resident set exactly, without downloading the model.

### Fixed

- **A model the engine cannot run was started anyway.** Qwen3.6 showed a green
  dot while the server had already exited, so the next message failed with
  "connection actively refused". The architecture is read from the container's
  header before a server is started, and the refusal says why.
- **A dead engine went on being reported as running.**
- **The window flickered once a second** — the timer repaint covered the child
  controls. `WS_CLIPCHILDREN`, on both windows.
- **`DrawTextW` with an empty string killed the installer.** `Vec::as_ptr` on an
  empty vector is dangling and Windows dereferences it; the report has blank
  lines in it.
- A worker thread cannot show a message box owned by the UI thread — it did
  nothing at all.

### Corrected

- **V4-Flash's resident set was `7_925_000_000`, a round guess.** Measured
  across all five shards: **7,920,157,020**. The estimate was lucky to 0.06%,
  and it is the number the project's headline rests on.
- Qwen3-30B-A3B was `1_000_000_000`; measured **997,554,176**. A test now
  rejects any Mixture-of-Experts resident figure that looks rounded.

### Known limits

- **Qwen 3.5/3.6 (`qwen35`, `qwen35moe`) cannot run.** They are hybrid
  architectures — recurrent state plus multi-token prediction — not plain
  transformers. They are listed in the catalogue and refuse with the reason.
- The menu bar stays light in dark mode; Windows draws it and
  `SetPreferredAppMode` measurably does nothing on 10.0.26200.
- MONITOR cannot show streamed bytes or cache residency; the engine measures
  them but does not report them over the socket.

## [0.0.6] — 2026-08-18

The app in 0.0.5 could not be used. This release is that, fixed, plus what a
model runner has to show.

### Fixed — the app died on the first click

**Clicking INSTALLED or AVAILABLE killed the process instantly**, with no
window, no message and no log.

`WM_CTLCOLORLISTBOX` borrows the window's state, and the code that filled the
list held a mutable borrow while calling `SendMessageW` — which dispatches that
message *synchronously*. A `RefCell` double borrow, and `panic = "abort"` turns
a double borrow into immediate process death. It could never have worked.

Six places had that shape. Three were found by a test written afterwards rather
than by clicking, and one of them was **UNLOAD** — the button that frees the
model's memory.

Two more, in the same family:

- **The GUI uninstall removed nothing.** It launched the helper and stayed open,
  so the helper could not delete the folder the window was running from and gave
  up. The window now exits after reporting.
- **Closing the window left the engine running**, holding every resident byte —
  7 GiB for V4-Flash — with nothing left to stop it from. Closing Chaos now
  stops the model.

**A crash now says something.** Under `abort` with no console there was nothing
at all; the app writes `%TEMP%\chaos-app-crash.log` and shows a message box
naming it.

### Added — icons

`chaos-setup.exe`, `chaos-app.exe` and the window itself carry the logo, at
**nine sizes from 16 to 256**, each rendered from `assets/logo.svg` at its own
resolution. Windows downsamples one large icon badly and this mark is thin
radiating lines.

### Added — what the app was missing

- **The endpoint.** `running <model> -> http://127.0.0.1:8231/v1`, so a coding
  agent can be pointed at it. There is no API key: the server binds localhost
  only.
- **DELETE**, which removes *every shard*. Deleting one file of a five-shard
  container would leave 120 GB of unusable data and report success.
- **Live memory**, free of total.
- **Settings that persist**, in `%USERPROFILE%\.chaos\settings.txt` — outside
  the install, so upgrading or uninstalling never takes them. Unknown keys are
  preserved, so an older build cannot discard a newer one's preferences.
- **A sidebar that scales with the window.** At a fixed width the model rows
  were clipped mid-word and the fit verdict — the number that decides whether a
  model runs — could not be read.

### Added — the installer says what it did

Install and uninstall end with a report rather than the window closing: what was
written, where, what was removed, and what was deliberately kept. Running a newer
setup over an older install names the upgrade.

### Added — `docs/APP.md`

A manual for the window, including a section on what it does not do yet.

### Changed

- CI starts `chaos-app`, waits, and fails if it exited or left a crash log —
  and checks no `chaos-serve` survives it. An app that only *builds* is what
  shipped 0.0.5.
- The documented test count is checked against the suite that actually ran.

### Known

**Windows SmartScreen will warn** that the publisher is unknown. That is what
Windows says about every unsigned application; choose *More info → Run anyway*.
Signing needs a certificate, which is a purchase, not a patch.

One model runs at a time; there is no per-model window yet.


## [0.0.5] — 2026-08-18

### Fixed — uninstalling actually uninstalled

**In 0.0.4, uninstalling from Add/Remove Programs removed nothing and reported
success.** A running executable cannot delete the directory it lives in, and the
installer copies itself into `bin` so Windows has something to launch — so the
uninstaller ran from inside the very folder it was trying to remove, the delete
failed, and the code said "uninstalled" anyway.

Three things were wrong and all three are fixed:

- The uninstaller now **re-runs from a copy in the temp directory**, spawned
  **detached** so the original can exit. Waiting for it was the first attempt and
  it failed the same way: the parent stayed alive holding the file open for the
  whole of the child's run.
- The staged copy **retries for ten seconds** rather than trying once, because
  the process that asked for the uninstall may still be exiting.
- It **checks before reporting.** If files remain it says how many and where,
  instead of claiming success over a full directory.

Also: the silent log no longer recreates the folder it just deleted. A clean
uninstall used to leave an empty `Chaos` directory containing one file that said
it had uninstalled.

Verified from the default prefix, the path that was broken: install 17 files,
uninstall from the installed copy, everything gone in one second, PATH and
registry clean.


## [0.0.4] — 2026-08-18

### Added — one file to install everything

**`Chaos-0.0.4-Setup.exe`.** Download it, run it, press INSTALL. 23 MB with every
binary inside: no archive to unpack, no PowerShell, no toolchain, no network, no
administrator rights. It installs per-user, puts Chaos on your PATH, creates the
models folder, adds a Start Menu entry and registers in Add/Remove Programs.

Built without NSIS, WiX, Inno or MSI tooling, because every one of them would
have to be installed on the build machine before a release could be cut, and this
project has no dependencies. A Windows install turns out to be a window, a file
copy, a PATH entry, a shortcut and one registry key.

**Uninstalling never touches your models.** They live outside the install folder
on purpose, and a test enforces it — the failure mode is deleting a 155 GB
download.

Silent mode for scripting and CI: `/S`, `/S --uninstall`, `--prefix <dir>`.
**Wait for it** — the installer is a window-subsystem binary, so a shell gets
control back immediately and never sees an exit code:

```powershell
$p = Start-Process Chaos-0.0.4-Setup.exe -ArgumentList '/S' -Wait -PassThru
$p.ExitCode
```

### Added — Chaos as a window

**`chaos-app`**, a native Win32 application. Not a browser in a frame: a real
window, drawn with GDI, in two colours and nothing between them. Pick a model,
LOAD it, chat with it, UNLOAD it — and unloading genuinely frees the memory,
because the engine runs as a child process rather than inside the window.

INSTALLED and AVAILABLE tabs, a DOWNLOAD button, and settings for cache, threads
and port.

### Added — a browser interface for `chaos-serve`

`GET /` now serves a chat page, self-contained in the binary: no CDN, no font, no
script fetched from anywhere. An offline machine gets the whole interface.

### Added — 13 models to fetch, up from 2

Qwen3 4B/8B/14B/32B, Gemma-3 4B/12B/27B, Llama-3.2 1B/3B, Qwen2.5-Coder-7B and
Phi-4, alongside DeepSeek-V4-Flash and Qwen3-30B-A3B. Every repository, filename
and byte count was read from the Hugging Face API and verified to resolve before
being added.

**Each entry states what must stay resident, not just what it downloads**, and
that is the number the fit verdict uses:

```
v4flash    155.1 GB    7.38 GiB resident   -> streams on a 16 GB machine
qwen3-32b   19.8 GB   18.40 GiB resident   -> does not
```

A dense model has no routed experts, so nothing streams and the whole file has to
fit. Sorting by download size would have called the 155 GB model impossible and
the 20 GB one easy, which is backwards.

### Changed

- `chaos-iobench` and `chaos-gpubench` now ship in the release, so the
  measurements this project publishes can be reproduced by anyone who downloads
  it.
- The release workflow builds and tests the installer, and fails if it embedded
  nothing.

### Fixed

- The documented test count is now checked by CI against the suite that actually
  ran. It had gone stale three times — 566, 570, 575 — each caught only by
  someone noticing.


## [0.0.3] — 2026-08-16

### Changed — the project is now called `chaos`

`bigtea-run` is `chaos-run`, and so are the other ten binaries. Crate names,
`--help` text, info lines, environment variables (`BIGTEA_*` → `CHAOS_*`), the
workflows and all 105 documents were renamed in one pass — 1,623 occurrences
across 184 files, none left. The git remote is deliberately untouched; the
`repository`/`homepage` URLs point at the new name and start resolving when the
repository is renamed.

**This is a breaking change for anything scripted against the old names**, which
is what a `0.0.x` minor is for.

### Added — running it no longer starts with a path

**`chaos-run <name>`.** Every command used to begin with an absolute path to a
`.gguf` file, which on Windows means something like
`C:\Users\you\.chaos\models\Qwen3-30B-A3B-Q4_K_M.gguf` typed by hand, and for a
five-shard container it means knowing which shard to name. Now any unique part
of a name resolves:

```
chaos-run                                   # lists the models you have
chaos-run qwen3 "The capital of France is"  # runs Qwen3-30B-A3B-Q4_K_M.gguf
chaos-run deepseek "..."                    # opens shard 1 of 5, automatically
```

An existing path still wins, so nothing that worked before changes. An ambiguous
name lists the candidates rather than guessing a 144 GB read, and an unknown one
lists what *is* available rather than leaving the user to go looking.
`chaos-serve` has the same lookup from the same code, so the two cannot disagree
about where models live.

Searched in order: `CHAOS_MODELS`, `~/.chaos/models` (which `install.ps1`
creates), the download cache `chaos-pull` writes to, and `./models`. **Two of
those already existed and pointed at different places** — where a model lived
depended on how it had arrived — which is exactly the kind of thing a first-time
user should never have to learn.

### Added

- **A startup logo**: the name, then the logo centred beneath it, then the
  version. Rasterised offline into 3 KB of committed luminance bytes and printed
  with Unicode half-blocks, two pixels to a cell. Cropped to the artwork rather
  than the SVG's canvas, which had been carrying a wide white margin into every
  render. No SVG parser, no
  image decoder, no build script, no dependency — the workspace still has zero.
  It sizes itself to the terminal and is skipped for `NO_COLOR`,
  `CHAOS_NO_BANNER`, `--log-disable`, a terminal too small, and any stdout or
  stderr that is not a terminal.
- **`scripts/install.ps1`** — Windows install and in-place upgrade. Copies the
  binaries to `%LOCALAPPDATA%\Chaos\bin`, adds it to the *user* PATH exactly
  once, and creates `%USERPROFILE%\.chaos\models`. Re-running upgrades and
  removes binaries the new version no longer ships; `-Uninstall` reverses it and
  never touches the models directory. It refuses rather than half-upgrading when
  a binary is running. Shipped inside the Windows archive and smoke-tested in
  the release workflow, on the unpacked archive, in the shape a user meets it.
- `chaos-serve` and `chaos-pull` are now in the release archives. They were
  built and never packaged.

### Corrected

**V4-Flash is at parity with llama.cpp, not far behind it.** The published
figures — prefill 1.62x behind, generation 3-4x behind — date from 2026-08-07 and
no longer reproduce. Three alternating pairs in one session, both engines at
their defaults:

| DeepSeek-V4-Flash | Chaos | llama.cpp |
|---|---:|---:|
| prefill, ms per prompt token | **1640** | 1679 |
| generation, tok/s | **0.394** | 0.39 |

The warm-up run, discarded, read llama.cpp at 0.23 tok/s — which would have made
this a 1.7x lead. It is not one. See
[`where-we-stand-vs-llamacpp-2026-08-16.md`](docs/graph/research/where-we-stand-vs-llamacpp-2026-08-16.md).

**The parallel-experts optimisation does not transfer to V4-Flash, and the
ceiling is measured: the entire routed expert arithmetic is under 5% of a
token.** A token is 67% expert-slice read, 17% block compute, 16% routing. The
block's single `compute` had been folded into the residual of the phase table,
which is why that split had never been written down.

### Corrected — earlier

**The hot expert set is per-prompt, so it cannot be pinned.** v0.0.2's routing
figures were all scored *in-sample on a single prompt*. Re-measured on eight
prompts across four subjects, with the token-id-routed layers 0-2 excluded and a
uniform-router null at matched sample size:

| published in 0.0.2 | measured |
|---|---|
| top-64 = 97.8% of selections | **90.5%** in-sample, **53.7%** on a prompt the set was not chosen from |
| 33.6 tok/s disk floor at 34.27 GiB | **1.60 tok/s** |
| 20 tok/s needs a ~48 GiB desktop | unsupported — needs a 96.3% hit rate; a pinned cache gives 76.7% at 68.5 GiB |
| chi-square 7805 | not a valid statistic — generation re-runs prefill per token, so the prompt was counted once per pass (1282 → 5464 → 11469 for 1, 4, 8 passes, with coverage unmoved) |

The skew itself is real and reproduced on every prompt: top-8 of 256 takes
34.6–52.0% of selections against a uniform null of 6.8–7.4%. What does not hold
is *transfer* — across subjects a pinned hot set scores 37.5% against 25.0% for
caching at random. See
[`routing-skew-is-per-prompt-2026-08-08.md`](docs/graph/research/routing-skew-is-per-prompt-2026-08-08.md).

### Added

- `CHAOS_ROUTING_DUMP=<path>` writes raw `pass,layer,expert,count` rows, so two
  runs can be compared offline and passes are not conflated.
- `tools/routing/` — the prompts, capture script and analysis behind the above.
- `STATUS.md` — one canonical statement of where the project stands and what
  remains, so any session can resume without reconstructing it.

### Planned

Everything the previous list named is done — the KV cache, the downloader, the
OpenAI-compatible server, quant selection from the probe and prebuilt binaries.
What replaces it, in the order the measurements justify:

- **The tok/s-versus-RAM frontier for a 144 GB model.** Nobody has published it,
  and only an engine that owns residency can sweep it — `mmap` cannot be told to
  use exactly N GiB.
- **Verify the GPU tier.** `--device`, `-ngl`, `-ot` and `--op-offload` all work
  on Vulkan, and the device path fails 1 of 8 parity prompts where the CPU path
  fails none. Shown to be arithmetic rather than wiring, but unproven either way.
- **More architectures.** 13 of llama.cpp's 141 have been diffed against it.
- **Not** 20 tok/s on V4-Flash. That is closed by measurement rather than
  deferred: it needs 79 MB/token and the model reads 3288.

## [0.0.2] — 2026-08-07

Findings, a retraction, and the measurement that changes the project's direction.

> **⚠ Superseded 2026-08-08.** Every routing figure in this entry was scored
> in-sample on one prompt and four of them are wrong — see **Corrected** under
> [Unreleased](#unreleased). The entry is left as released rather than rewritten.

### Added

- `CHAOS_ROUTING=1` prints how often each expert of each layer is actually
  selected, and what the hot set would cost to keep resident.

### Discovered

**DeepSeek-V4-Flash's router is violently skewed.** Every speed estimate this
project ever made assumed it spread evenly over 256 experts:

| top-N per layer | share of selections | resident cost |
|---:|---:|---:|
| 1 | 12.1% | 0.54 GiB |
| 8 | 52.9% | 4.28 GiB |
| 16 | 70.4% | 8.57 GiB |
| 64 | **97.8%** | 34.27 GiB |

Uniform routing would give top-16 = 6.2%; measured 70.4%, chi-square 7805 against
uniform's ~255. With a hot-set cache, bytes read per token fall from 3.21 GiB to
**72 MiB** — a 33.6 tok/s disk floor, against a 27 tok/s compute floor.

**20 tok/s for a 144 GB model is a cache-sizing problem, not a physics
violation**, and it needs roughly a **48 GiB desktop** rather than the ~150 GiB
previously claimed. On a 15.7 GiB laptop the same arithmetic implies ~1.3 tok/s,
about 4x llama.cpp. Neither is measured yet; both are arithmetic on measurements
that are. See
[`routing-skew-changes-everything.md`](docs/graph/research/routing-skew-changes-everything.md).

### Retracted

**v0.0.1's claim that Chaos leads llama.cpp on DeepSeek-V4-Flash.** It claimed
3.0x faster load and 1.20x faster prefill. Both were false: Chaos's numbers were
measured fresh and llama.cpp's were copied from a two-day-old document taken under
different free-RAM conditions, so the engines were never run back to back. Run
back to back, twice:

| | Chaos | llama.cpp |
|---|---:|---:|
| load | 10.0s | 10.5s |
| prefill, per prompt token | 2440 ms | **1503 ms** |
| generation | 0.064 tok/s | **0.21–0.31 tok/s** |

**Chaos leads on nothing on this model.** It remains ahead on Qwen3-30B-A3B
prefill at 565 and 2206 tokens, measured back to back.

## [0.0.1] — 2026-08-07

Performance. DeepSeek-V4-Flash prefill is **2.2x** faster than v0.0.0 and
generation **1.83x**, with every one of the 14 oracle tests still matching
llama.cpp's element sums.

### Changed

- **One graph evaluation per block instead of 24.** `Context::compute` evaluates
  a tensor's *entire ancestor graph*, so calling it on every intermediate does
  not merely dispatch more work — it **re-does** the work, once per call, and
  pays a graph build and a threadpool cycle each time. A value is now computed
  only where the CPU must read it. Worth **1.9x**, and invisible on a long
  prefill because the matmuls there are large enough to bury it.
- **A layer's three expert tensors are read in one parallel batch.** Four
  readers, jobs distributed one slice at a time so each reader gets an equal
  share of the bytes. Parallel reads had been tried and reverted twice before;
  the difference is batch size — per-tensor groups are 6 slices at generation
  time, and the thread spawns cost more than the queue depth buys.
- `CHAOS_THREADS` selects the thread count per graph evaluation, and
  `CHAOS_BLOCK_TIMING` now reports each phase of a block separately.

### Performance

DeepSeek-V4-Flash, same machine, both engines' command lines and outputs in
[`v4flash-vs-llamacpp-2026-08-07.md`](docs/graph/research/v4flash-vs-llamacpp-2026-08-07.md):

> **⚠ Retracted the same day.** This section originally claimed 3.0x faster load
> and 1.20x faster prefill. Both were wrong: Chaos's numbers were fresh and
> llama.cpp's were copied from a two-day-old document taken under different
> free-RAM conditions, so the two engines were never run back to back. Corrected
> figures, measured back to back twice:

| | Chaos | llama.cpp | |
|---|---:|---:|:--|
| load | 10.0s | 10.5s | parity |
| prefill, per prompt token | 2440 ms | **1503 ms** | llama.cpp 1.62x faster |
| generation | 0.064 tok/s | **0.21-0.31 tok/s** | llama.cpp 3-4x faster |

**Chaos leads on nothing on this model.** The speedups below are real and
measured against Chaos's own previous version; they simply did not close the gap.

A single-token forward pass costs **4.0s**. That is what one step of a KV-cached
loop will cost — 0.25 tok/s — and it is the number to plan against, because the
0.077 above is an artefact of re-running the whole sequence for each token.

### Fixed

- macOS: process enumeration read `/proc`, which does not exist there, so the
  "close these apps to free RAM" advice silently did nothing. Falls back to `ps`.
- macOS: Accelerate framework was never linked, though ggml's cmake enables it by
  default and calls vDSP.
- macOS: OpenMP was demanded unconditionally; AppleClang ships none.
  `CHAOS_GGML_OPENMP` overrides the per-platform default.
- The documented `cmake` line built **shared** ggml libraries, so a new user
  following the README got no `.a` archives at all.
- `chaos-arch` now fails with one actionable message when ggml is missing,
  instead of a wall of unresolved imports.
- Declared MSRV was 1.74 while the code used a 1.82 API. Now 1.82.

## [0.0.0] — 2026-08-07

First public release. The engine works and is verified; the product around it is
not built yet. See [README](README.md#status) for what is and is not there.

### Added

- **Runs Mixture-of-Experts models several times larger than RAM, on CPU.**
  Always-read weights stay resident; routed experts stream from disk as routing
  selects them.
- **DeepSeek-V4-Flash (`deepseek4`) support** — 43 blocks, hyper-connections,
  three kinds of compressed attention (raw, compressed-sparse, heavily
  compressed), hash routing on the first three layers and biased top-k routing on
  the other 40. Verified element-by-element against llama.cpp on all 43 blocks
  plus the output head.
- **Qwen3 and Qwen3-MoE support**, with a frequency-gated expert cache.
- **Cache-bypassing direct I/O** (`FILE_FLAG_NO_BUFFERING` / `O_DIRECT`), falling
  back to buffered reads and *reporting* that it did rather than pretending.
- **Zero-copy expert reads.** `SkewedBuf` deliberately misaligns the destination
  buffer to match the file's sector residue, because GGUF pads tensor data to 32
  bytes rather than to a disk sector — so a conventionally aligned buffer can
  never receive a direct transfer. 0.80 → 1.58 GiB/s, with 0.09% of bytes copied
  instead of 300%.
- **Residency with a hard budget**, which reports what did not fit, what
  re-reading it costs per token, and which processes to close to fix it.
- `chaos-run` — prefill and generation.
- `chaos-probe` — RAM, disk, GPU, and what to close.
- `chaos-model-info` — fit prediction and tok/s estimate before running.
- `chaos-meta`, `gguf-info`, `chaos-loadbench` — container and I/O inspection.
- 157 unit tests and 16 container-backed tests.

### Performance

Measured on one machine (15.7 GiB RAM, NVMe at 2.55 GB/s, 20 threads). Both
engines produce identical, correct output; llama.cpp is measured with a warm page
cache. Full command lines and outputs in
[`head-to-head-llamacpp-2026-08-05.md`](docs/graph/research/head-to-head-llamacpp-2026-08-05.md).

Qwen3-30B-A3B Q4_K_M prefill, Chaos / llama.cpp:

| tokens | Chaos | llama.cpp |
|---:|---:|---:|
| 565 | **27.64** | 23.55 |
| 2206 | **36.60** | 33.59 |
| 4395 | 38.40 | 40.25 |
| 8775 | 34.88 | 35.01 |
| 4395 (`-b 4096`) | **43.61** | 40.25 |

### Known limitations

- **Generation is slower than llama.cpp.** DeepSeek-V4-Flash: 0.077 tok/s against
  0.45, because the V4-Flash path has no KV cache yet and each token re-runs the
  whole sequence. Qwen3-30B-A3B: 1.07 against 2.16, about 2x. On V4-Flash
  **Chaos leads on nothing** — see the retraction above. It is ahead only on
  Qwen3-30B-A3B prefill at 565 and 2206 tokens.
- **Linux and macOS build and pass the unit tests in CI, but no model has been
  run on either.** macOS additionally has no direct-I/O path — `F_NOCACHE` needs
  an `fcntl` after opening and is not written yet — so it falls back to buffered
  reads and the page-cache problems this design exists to avoid.
- No model downloader; bring your own `.gguf`.
- No server or API.
- No prebuilt binaries; ggml must be built first and `GGML_LIB_DIR` set.
- Only `qwen3`, `qwen3moe` and `deepseek4` architectures run. Others parse as
  containers but will not execute.

### Retracted

- **"llama.cpp cannot run models larger than RAM."** It can, with `--no-repack`.
  This claim survived several days on a misattributed error string because nobody
  ran the opposing command. It is retracted in writing, and the project now
  requires a competitor's exact command line and output before any competitive
  claim is citable.

[Unreleased]: https://github.com/aturzone/Chaos/compare/v0.0.21...HEAD
[0.0.21]: https://github.com/aturzone/Chaos/releases/tag/v0.0.21
[0.0.20]: https://github.com/aturzone/Chaos/releases/tag/v0.0.20
[0.0.19]: https://github.com/aturzone/Chaos/releases/tag/v0.0.19
[0.0.18]: https://github.com/aturzone/Chaos/releases/tag/v0.0.18
[0.0.17]: https://github.com/aturzone/Chaos/releases/tag/v0.0.17
[0.0.16]: https://github.com/aturzone/Chaos/releases/tag/v0.0.16
[0.0.15]: https://github.com/aturzone/Chaos/releases/tag/v0.0.15
[0.0.14]: https://github.com/aturzone/Chaos/releases/tag/v0.0.14
[0.0.13]: https://github.com/aturzone/Chaos/releases/tag/v0.0.13
[0.0.12]: https://github.com/aturzone/Chaos/releases/tag/v0.0.12
[0.0.11]: https://github.com/aturzone/Chaos/releases/tag/v0.0.11
[0.0.10]: https://github.com/aturzone/Chaos/releases/tag/v0.0.10
[0.0.8]: https://github.com/aturzone/Chaos/releases/tag/v0.0.8
[0.0.7]: https://github.com/aturzone/Chaos/releases/tag/v0.0.7
[0.0.6]: https://github.com/aturzone/Chaos/releases/tag/v0.0.6
[0.0.5]: https://github.com/aturzone/Chaos/releases/tag/v0.0.5
[0.0.4]: https://github.com/aturzone/Chaos/releases/tag/v0.0.4
[0.0.3]: https://github.com/aturzone/Chaos/releases/tag/v0.0.3
[0.0.2]: https://github.com/aturzone/Chaos/releases/tag/v0.0.2
[0.0.1]: https://github.com/aturzone/Chaos/releases/tag/v0.0.1
[0.0.0]: https://github.com/aturzone/Chaos/releases/tag/v0.0.0
