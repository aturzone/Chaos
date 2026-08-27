# Chaos — a runner for models larger than RAM

> **Read `STATUS.md` first.** It is the canonical statement of where the project
> is, the honest scoreboard, and what remains in order. Update it in the same
> commit as anything that moves a number or closes a task.

A Rust inference runner whose job is running models that do *not* fit in memory:
the always-read weights stay resident, routed experts stream from disk per token.
Borrows `ggml` for arithmetic; owns memory, residency, streaming and the token
loop. **Proven**: Qwen3-30B-A3B (17.28 GiB) generates correct text on a 15.7 GiB
machine, and DeepSeek-V4-Flash (144 GB) does too.

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
# was once reported for a file whose two GPU tests never ran once.
cargo test --release          # 909 tests
cargo test --release --test deepseek4_forward -- --ignored   # 19 V4-Flash, needs the container
cargo build --release
./target/release/chaos-run <name-or-path> "prompt" -n 16   # bare `chaos-run` lists models
./target/release/chaos-probe --quick          # RAM/disk/GPU + what to close
./target/release/chaos-model-info <m.gguf> --budget 8   # fit + tok/s prediction
```

Windows needs the **GNU** Rust toolchain plus MSYS2 mingw64 on PATH, and
`.cargo/config.toml`'s `link-self-contained=no` must stay.

## Layout — `core/` `cli/` `network/` `gui/`, per the Rust book

**Directories moved, crate names did not.** `core/gguf` is the `chaos-gguf`
package, so `-p chaos-gguf` and `use chaos_gguf::` are unchanged.

`core/`: `build` build-script helpers · `gguf` container parsing · `ggml` FFI
(graph, zero-copy weight binding) · `io` cache-bypassing aligned reads · `probe`
hardware + RAM reclaim · `plan` prediction + residency policy · `model` sharded
resolution, partial reads, catalogue, release checks · `tokenizer` byte-level
BPE · `grammar` constrained decoding + the workspace's JSON parser · `jinja` chat
templates · `arch` architectures + streaming forward pass · `image` PNG,
safetensors, FLUX.2 VAE, the sampler.

`cli/run` chaos-run · `network/serve` chaos-serve · `gui/app` the window ·
`gui/setup` the installer. Benchmarks stay beside the crate they measure.

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
- Sync audit at phase boundaries only, not per commit.
- Keep this file under ~2000 tokens; tell Atur to prune rather than letting it
  bloat. It reached 3,308 words once — the overflow is now
  `reference/hard-won-facts.md`.

## Roadmap — Atur's targets, in his order

**This list is the agreement. Do not let a session end having quietly dropped an
item; if one is not done, say which and why.**

- [ ] **1. V4-Flash at 20 tok/s on THIS machine.** *Excluded by Atur, and
  measured as out of reach here.* A token is 1.56 s of expert read plus 0.84 s
  that never touches the disk, so with **every** expert resident this CPU tops
  out at **1.19 tok/s**. 20 tok/s also needs 67.7 GB/s to the experts, which is
  a GPU-memory specification. **Do not report progress without a measurement.**
- [x] **2. The image generator, with model selection.** Orientation, seed,
  model selection, `--keep-latent` and a guidance control are done. **Open**:
  quality below 1024, prompt adherence, and a negative prompt — which needs a
  real pipeline change (the twin is fed no text, not an empty prompt) and a
  quality harness to judge it.
- [x] **3. The model list.** Sort, filter, search, a kind per row, and no
  stall: 1584 ms → 10.8 ms per switch.
- [x] **4. R6 — self-configuration.** `--auto` picks device, cache, threads,
  prefill block and I/O mode, and predicts tok/s: 1.42 predicted against 1.51
  measured on Qwen3-30B-A3B.
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

**v0.0.17 released 2026-08-24**: nine assets. The **Android app has been run**
— on an emulator, against a real `chaos-serve` — and running it found four
defects a build never would. `chaos-worker` holds experts and answers with
activations, measured at 38x in favour of sending the work to the weights.
1024x1024 encodes. Two claims retracted: the JSON prompt shape (0.9x, it is the
sentences) and "4 steps is five times faster" (a 56-level grey band).
**Install → update → uninstall verified on this machine from the published
files, models byte-identical** — `scripts/install-update-uninstall.ps1`.
`STATUS.md` is the scoreboard; `backlog/the-plan-v0-1-0.md` is the queue.

**Three instruments, kept because they are how the above was measured**:
`scripts/poke-app.ps1` (click a control, time the UI thread, check a layout for
overlaps), `scripts/run-through.ps1` (every control, every page, one
transcript), `tools/check-logo-centred.py` (decode the shipped `.ico`, report
the margins). A screen grab is uniform black in a session with no composited
display, which is why these read rectangles rather than pixels.

**Against llama.cpp, measured 2026-08-16 with both engines alternating**
(`where-we-stand-vs-llamacpp-2026-08-16.md`): **parity on everything that
streams** — V4-Flash prefill 1640 against 1679 ms/prompt token and generation
0.394 against 0.39, Qwen3-30B parity on both phases. Behind 1.20-1.27x on the
dense path when both sides are hand-tuned; ahead 1.23x out of the box, because we
measure the machine and llama.cpp uses a fixed default. **The old "V4-Flash
prefill 1.62x behind, generation 3-4x behind" is retracted**, and so is
"generation ~2x behind" on Qwen3-30B. Do not replace either with a claimed lead:
the ranges overlap.

**20 tok/s on V4-Flash is closed from both sides, with numbers**
(`v4flash-ram-frontier-2026-08-16.md`). Bytes: 20 tok/s needs 79 MB/token and it
reads 3288 (`v4flash-has-no-slack-2026-08-10.md`). Time: **a token is 1.56 s of
expert read plus 0.84 s that never touches the disk, so with EVERY expert
resident this CPU tops out at 1.19 tok/s** — the fixed cost alone is 17x over a
50 ms budget, and `-t` 2/4/8/16 confirms it is a floor, not a knob left wrong.
**The measured frontier**: 16 GB 0.42 (measured), 64 GB 0.55, 128 GB 0.93,
160 GB 1.19 — **holding the whole 144 GB model in RAM is worth 2.9x, not 48x.**
20 tok/s also needs 67.7 GB/s to the experts, so it is a GPU-memory
specification. Do not quote a GPU V4-Flash figure: resident-in-VRAM is untested
and the only measured number is 4.3x *slower* on streaming MoE.

1. **The frontier on a machine with real memory.** The curve above is this
   laptop's left-hand edge; the two numbers worth bringing back are `F` on a
   bigger CPU and `F` with the model resident on a real GPU, because the whole
   question reduces to them. Prompt ready: `backlog/bigger-machine-prompt.md`.
2. **A bigger machine**, measured rather than predicted. The 5090 box is 32 GiB
   VRAM + 64 GiB RAM, where Qwen3-30B-A3B (17.3 GiB) fits **entirely in VRAM** —
   that is the demo, not V4-Flash. 96 GiB of fast memory against 144 GiB of model
   is ~67% resident there against ~11% here. Check `--auto` picks sensibly
   without the user knowing any flags.
3. **Verify the GPU tier.** `--device`, `-ngl`, `-ot`, `--op-offload` and
   `ggml_backend_sched` are all bound on Vulkan; what is *not* done is
   verification — the device path fails 1 of 8 parity prompts where the CPU path
   fails none, which is arithmetic rather than wiring.
4. Finish R5/T1-T5 of `lts-0-0-0.md`: quant selection, self-configuration.

**Dead ends, measured, do not re-propose**: expert factorisation, contextual
sparsity, a pinned hot set, expert-read/compute overlap (1.03x), `--op-offload`
(19% slower), `mul_mat_id` batching on the streaming path, and porting
parallel-experts to V4-Flash (its whole routed arithmetic is under 5% of a
token).

## Compact Instructions

If auto-compacted, preserve ONLY: open decisions, the work in progress, files
modified this session, unresolved questions for Atur. Discard tool output,
committed file contents, and dead ends.
