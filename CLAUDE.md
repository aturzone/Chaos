# Chaos — a runner for models larger than RAM

> **Read `STATUS.md` first.** It is the canonical statement of where the project
> is, the honest scoreboard, and what remains in order. Update it in the same
> commit as anything that moves a number or closes a task.

A Rust inference runner whose job is running models that do *not* fit in memory:
the always-read weights stay resident, routed experts stream from disk per token.
Borrows `ggml` for arithmetic; owns memory, residency, streaming and the token
loop. **Runs models far past RAM**: Qwen3-30B-A3B (17.28 GiB) and
DeepSeek-V4-Flash (144 GB) both generate on a 15.7 GiB machine. **"Proven" was
too strong for the first of those** — `qwen3moe` is *not* in
`VERIFIED_ARCHITECTURES` and needs `--force`: its eight-prompt diff came back
1 FAIL, a demonstrated near-tie where llama.cpp produces our exact answer under
`-b 1`. V4-Flash *is* verified. Do not write "proven" for a model the diff has
not passed.

Graph docs live in `/docs/graph/`; read `INDEX.md` first, then only the 2–3 nodes
a task links to.

## Build / test / run

```
# ggml must be built first; point GGML_LIB_DIR at ggml-base.a, ggml-cpu.a, ggml.a
export GGML_LIB_DIR=C:/Projects/llamacpp-unsloth/build/ggml/src   # PowerShell: $env:GGML_LIB_DIR=...
# On Unix add -DCMAKE_POSITION_INDEPENDENT_CODE=ON: `android/jni` is a cdylib
# and links ggml, and a non-PIC archive fails with a page of R_X86_64_PC32
# relocation errors against `stderr` that name neither ggml nor the cdylib.
# GPU work needs build-vulkan/ggml/src instead: THE LINE ABOVE has no Vulkan
# archive, build-vulkan has ggml-vulkan/ggml-vulkan.a and build.rs finds it.
# The GPU tests SKIP rather than fail without a card -- so a green "6 passed"
# was once reported for a file whose two GPU tests never ran once. Fixed:
# CHAOS_REQUIRE_GPU=1 turns every such skip into a failure, and against
# build-vulkan all 14 GPU tests run and pass on this laptop's RTX 3050.
cargo test --release          # 983 tests
cargo test --release --test deepseek4_forward -- --ignored   # 19 V4-Flash, needs the container
cargo test --release -p chaos-qr --test reference_grids identical_to  # crate/file/one test
cargo clippy --workspace --all-targets -- -D warnings   # CI gate: warnings are errors
cargo fmt --all --check                                 # CI gate
cargo build --release
./target/release/chaos-run <name-or-path> "prompt" -n 16   # bare `chaos-run` lists models
./target/release/chaos-probe --quick          # RAM/disk/GPU + what to close
./target/release/chaos-model-info <m.gguf> --budget 8   # fit + tok/s prediction
```

Windows needs the **GNU** Rust toolchain plus MSYS2 mingw64 on PATH, and
`.cargo/config.toml`'s `link-self-contained=no` must stay.

**Twelve of the thirteen CI-checked crates build with no ggml, and CI enforces
both halves**: every
crate but `chaos-arch` builds, tests and lints without `GGML_LIB_DIR`, and
`chaos-arch` must fail with its `GGML_LIB_DIR is not set` message rather than a
wall of unresolved imports. **A full run reports 42 ignored** — they need a
real container on disk and skip silently without one, so a green run is not a
full run. **The test-count comment above is machine-checked**:
`scripts/check-test-count.sh` compares it with STATUS.md, CONTRIBUTING.md and
the README badge,
so keep exactly *one* `# <N> tests` comment in this file, written with real
digits only there — a second occurrence of that pattern anywhere in this file
makes the check compare a two-line string and fail forever.

## Layout — `core/` `cli/` `network/` `gui/`, per the Rust book

**Directories moved, crate names did not.** `core/gguf` is the `chaos-gguf`
package, so `-p chaos-gguf` and `use chaos_gguf::` are unchanged.

`core/`: `build` build-script helpers · `gguf` container parsing · `ggml` FFI
(graph, zero-copy weight binding) · `io` cache-bypassing aligned reads · `probe`
hardware + RAM reclaim · `plan` prediction + residency policy · `model` sharded
resolution, partial reads, catalogue, release checks · `tokenizer` byte-level
BPE · `grammar` constrained decoding + the workspace's JSON parser · `jinja` chat
templates · `arch` architectures + streaming forward pass · `image` PNG,
safetensors, FLUX.2 VAE, the sampler · `qr` encoding, so a headless node can
print its own route · `config` the settings file **both tiers read** · `http`
just enough HTTP/1.1 to ask a node for status and stream a completion, no curl,
no TLS.

**`chaos` is the front door**: `cli/chaos` dispatches `chaos run` to
`chaos-run` with arguments untouched — every old binary name still works — and
implements `start`/`stop`/`status` (a node as a background process, pid file,
log), `connect`, `config`, `completions`, and `scan`, which is **NOT BUILT and
says so**. `chaos start` uses `Settings::serve_args`, the window's own function.

`cli/run` chaos-run · `network/serve` chaos-serve · `network/worker`
chaos-worker, which holds experts and answers with activations · `gui/app` the
window · `gui/setup` the installer · `android/jni` the JNI bridge, a cdylib.
Benchmarks stay beside the crate they measure.

**`nav::RAIL_PAGES` is not `nav::PAGES`.** Six pages exist; five have a rail
entry. CHAOS has none — the mode is answered by the launch knob and reported by
the badge at the bottom of the rail, which is also the page's only door. Leaving
a mode (the CHANGE MODE button *or* Escape) asks first, because it unloads the
model and clears the conversation.

**`core/` holds a crate's own tools; `cli/` holds the front door and the
runner.** 11 of the 19 binaries live under `core/` and that is deliberate — a
benchmark or an inspector belongs beside the crate it measures, and `chaos probe`
reaches it without anyone knowing where it lives. Two stated rules collide here
and this is the resolution (§4c).

**Twenty-one binaries, not five** — also `chaos-pull` (fetch a model),
`chaos-draw` (image), `chaos-qr`, `chaos-meta`, `gguf-info` and **eight**
benchmarks. **`grep '^name' */*/Cargo.toml` undercounts by two**: a `src/bin/*.rs`
is a binary with no `[[bin]]` anywhere, which is what hid `chaos-qdbench` and
`chaos-membench`. **A binary in no ship list does not exist**: `chaos-qr` was
absent from all three of release.yml's staging loops, and those two benchmarks
— the ones that measured 30.8 GiB/s and queue depth 2.55x, the two numbers the
whole 5 tok/s argument rests on — shipped nowhere at all, while the Linux
packages also lacked `chaos-draw` and `chaos-worker`.
`every_binary_reaches_every_platform` is now the mechanism, in both directions
and including `make-linux-packages.sh`, which no test had ever read.

**The mark and the reader have one source, served by the node.**
`assets/grimoire/*.html` plus embedded fonts are `include_str!`d by
`chaos_arch::grimoire` and served at `/qr` and `/scan`; desktop and Android open
that route rather than re-drawing it, and `core/qr` prints the same code in a
bare terminal. **Edit the HTML, never a copy**, and keep it fetch-free — a test
asserts 0 external references in the assembled page.

## Traps — **read `docs/graph/reference/hard-won-facts.md` before proposing any
optimisation.** About half its entries are the measurement that killed an
appealing idea. The five that bite most often:

- **ggml aborts rather than returning errors** — exhausted arena, bad device
  index, misaligned pointer. It kills whole test binaries, not one test.
- **ggml `ne[0]` is the fastest dimension**; row-major reading transposes every
  matrix and yields confident nonsense.
- **A wrong forward pass produces fluent nonsense, never a crash.** Only a diff
  against llama.cpp counts, which is what `VERIFIED_ARCHITECTURES` means. An
  exit code is not a diff, and an architecture name is not a shape.
- **Threads are two levers pulling opposite ways**: generation wants 2-4 (`-t`),
  prefill wants all of them (`-tb`), the MoE expert path wants **one**.
- **Only compare within one session**, and `Get-Process` before trusting a
  surprising number — an orphaned benchmark holding 9 GiB looks like a 10x
  regression.

## Working rules

- Git: remote `github.com/aturzone/Chaos`. Push with the token from
  `C:\Projects\.env` inline in the URL, output redacted — never in git config,
  never echoed, and **never `git push -u`**. Weights stay gitignored.
- Implementation goes on `ticket/<name>` branches + PR. **Claude owns git end to
  end**: merge when CI is green, close what it supersedes, delete the branch,
  prune, leave `main` verified. Docs may go to main.
- **Git hygiene, each rule bought with a mistake.** Verify containment with
  `git merge-base --is-ancestor <branch> origin/main` *before* deleting, never
  from "it was merged". After merging, `git checkout main` is not enough — a
  local `main` with no upstream makes `git pull` a silent no-op; fast-forward
  from `origin/main` explicitly and check a file only the merge added. Then
  **re-run tests on `main` itself**. GitHub parses only the *first* issue in
  `Closes #1, #2, #3`, so give every one its own `closes`.
- **A competitive claim is not citable until the competitor's exact command line
  and its output are in a doc**, from repeats, alternating in one session.
- **`README.md` carries three things and nothing else** (Atur's rule,
  2026-08-31): the progress bars -- the release ladder *and* the coverage block --
  the document map, and tok/s for the **five fixed models** in
  `scripts/speed-five.tsv`, measured in one session on a machine with nothing
  else running, and dated. Refill the table with `scripts/speed-five.sh`, never
  by hand. `scripts/check-readme.sh` enforces the section list, a line cap, the
  five rows and the date; CI runs it. It was 393 lines once.
- Sync audit at phase boundaries only, not per commit.
- Keep this file under ~2000 tokens; tell Atur to prune rather than letting it
  bloat. It reached 3,308 words once — the overflow is now
  `reference/hard-won-facts.md`.

## Roadmap — Atur's targets, in his order

**This list is the agreement. Do not let a session end having quietly dropped an
item; if one is not done, say which and why.**

- [ ] **1. V4-Flash at 20 tok/s on THIS machine.** *Excluded by Atur, and
  measured as out of reach here.* **Re-measured 2026-09-01: 0.728 tok/s**, from
  0.509, after C5e and defaulting the expert cache — both exact, both gated. A
  generated token is now **0.815 s of expert read plus 0.478 s of arithmetic**;
  remove the disk entirely and this machine lands at **1.5–1.8 tok/s**, so the
  arithmetic is the binding constraint and **88% of it is the hyper-connection
  algebra**, never yet costed. The 4.26 tok/s "one-bit expert" figure is a *disk*
  bound and must not be quoted as the ceiling.
  **Do not report progress without a measurement.**
- [x] **2. The image generator, with model selection.** Orientation, seed,
  model selection, `--keep-latent` and a guidance control are done. **Open**:
  quality below 1024, prompt adherence, and a negative prompt — which needs a
  real pipeline change (the twin is fed no text, not an empty prompt) and a
  quality harness to judge it.
- [x] **3. The model list.** Sort, filter, search, a kind per row, and no
  stall: 1584 ms → 10.8 ms per switch.
- [~] **4. R6 — self-configuration.** `--auto` picks device, cache, threads,
  prefill block and I/O mode, and predicts tok/s: 1.42 predicted against 1.51
  measured on Qwen3-30B-A3B — **on the dense path only.** `auto_plan` takes a
  `Qwen3Config` and the deepseek4 dispatch returns before that config exists, so
  `--auto` makes **zero** decisions on V4-Flash. The expert cache now has a
  default there regardless (1.20x); threads, batch, I/O and device do not.
- [x] **5. An Android app, `.apk` with every release.** A *client*; Phase B
  (models on the phone) is blocked — `dl.google.com` 404s this whole network,
  so the SDK cannot be installed here and CI is the only build. **Never run on
  a phone.** `backlog/android-app.md`.
- [ ] **6. Devices as resources — one model, many machines.** An activation is
  **16 KB**, a token's experts are **3.3 GB**, so expert-parallel costs ~66 ms
  of network to replace ~1560 ms of disk. **Send the work to the weights, never
  the weights to the work.** Four machines get single-digit tok/s on V4-Flash,
  not 20 — say so before building. `backlog/devices-as-resources.md`.
- [ ] **7. Genuinely better than llama.cpp.** Today: parity on everything that
  streams, 1.20–1.27x behind on the dense path hand-tuned, 1.23x ahead out of
  the box. A claim is not citable until the competitor's exact command line and
  output are in a doc, alternating in one session.

**The full plan, with a definition of done for every item:
`backlog/the-plan-v0-1-0.md`.**

## Next

`STATUS.md` is the scoreboard; `backlog/the-plan-v0-1-0.md` is the queue. Both
are more current than this file — take a number from them, not from here.

**Three instruments, because they are how the GUI is measured at all**:
`scripts/poke-app.ps1` (one control, timed, overlap-checked),
`scripts/run-through.ps1` (every control, every page, one transcript),
`tools/check-logo-centred.py` (margins of the shipped `.ico`). **A screen grab is
uniform black here** — read rectangles, never pixels, and **`IsWindowVisible` is
not "on screen"**: `layout` parks unreachable rail buttons at `(-3200,-3200)`.
`run-through.ps1` now enters a mode first and covers the CHAOS page; it used to
report a clean pass over an app that had never left its launch screen.

**CI logs cannot be read from this machine** — the logs endpoint redirects to an
Azure blob host that does not resolve here. Reproduce a CI failure locally from the
workflow's own commands instead; that found v0.0.22's release failure in one try.

**Retracted, do not requote** (Roadmap 7 has the standing figures;
`where-we-stand-vs-llamacpp-2026-08-16.md` has the method): "V4-Flash prefill
1.62x behind, generation 3-4x behind" and "generation ~2x behind" on Qwen3-30B.
Do not replace them with a claimed lead either — the ranges overlap.

**The measured RAM frontier** (`v4flash-ram-frontier-2026-08-16.md`): 16 GB 0.42
tok/s measured, 64 GB 0.55, 128 GB 0.93, 160 GB 1.19 — **the whole 144 GB model in
RAM is worth 2.9x, not 48x.** Do not quote a GPU V4-Flash figure: resident-in-VRAM
is untested and the only measured number is 4.3x *slower* on streaming MoE.

**Open, none of it blocking**: iOS is parked until everything else is good
(Atur's call); no real camera has seen the mark or the reader; the Android tier
cannot be built here (`dl.google.com` 404s); `chaos scan` is declared NOT BUILT;
zsh and fish completions are generated but never sourced; a long upgrade jump from
0.0.2 is untested; there is no contrast audit or screen-reader story for the
window. **One cheap idea worth taking**: `chaos_arch::grimoire` has zero ggml
references, so moving it to its own crate would let the APK step emit the brand
pages with no C toolchain and delete the host-ggml build that step now needs.

1. **The frontier on a machine with real memory** — this laptop is the curve's
   left-hand edge, so the two numbers worth bringing back are `F` on a bigger
   CPU and `F` with the model resident on a real GPU. Prompt ready:
   `backlog/bigger-machine-prompt.md`. The 5090 box (32 GiB VRAM + 64 GiB RAM)
   fits Qwen3-30B-A3B **entirely in VRAM** — that is the demo, not V4-Flash —
   and it tests whether `--auto` picks sensibly for someone who knows no flags.
2. **Verify the GPU tier.** `--device`, `-ngl`, `-ot`, `--op-offload` and
   `ggml_backend_sched` are all bound on Vulkan; what is *not* done is
   verification — the device path fails 1 of 8 parity prompts where the CPU path
   fails none, which is arithmetic rather than wiring.
3. Finish R5/T1-T5 of `lts-0-0-0.md`: quant selection, self-configuration.

**Dead ends, measured, do not re-propose**: expert factorisation, contextual
sparsity, a pinned hot set, expert-read/compute overlap (1.03x), `--op-offload`
(19% slower), `mul_mat_id` batching on the streaming path, and porting
parallel-experts to V4-Flash (its whole routed arithmetic is under 5% of a
token).

## Compact Instructions

If auto-compacted, preserve ONLY: open decisions, the work in progress, files
modified this session, unresolved questions for Atur. Discard tool output,
committed file contents, and dead ends.
