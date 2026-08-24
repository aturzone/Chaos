# The plan for v0.1.0 — everything Atur asked for, ordered

Written 2026-08-22 at his request: *"brainstorm to write a fully plan for all
last prompt i told you + this new version with android app too in exports"*.

**The rule this document exists to enforce**: nothing here gets quietly dropped.
If an item is not done when the release is cut, the release notes say which and
why. An item is done when its check passes, not when it feels finished.

---

## Part 0 — What is already fixed, so it is not re-litigated

These landed on `ticket/r73-drawing-visible` and are waiting for release:

| | evidence |
|---|---|
| **Every picture was upside down** | Ideogram 4's vertical axis runs bottom-up; velocity harness, 12 of 12 |
| **Every picture was identical** | seed was hard-coded 42; now random per draw and logged |
| The window said "no model running" while drawing | strip and MONITOR report the draw |
| Progress was a wall of log lines | a real bar, on the page and in the strip |
| No idea how long a draw would take | "about 6.5 hours on this machine", red past an hour |
| A crash that was not a panic left nothing | `SetUnhandledExceptionFilter` writes the fault address |

Between them these account for most of *"always same image, always low quality,
always broken"*. The pictures were correct, inverted, and identical.

---

## Part 1 — The image generator, properly

**Target 2 on the roadmap.** Orientation and seed are done. What remains:

- [x] **Model selection.** Done. `chaos_model::image` groups a denoiser with its twin, text encoder and autoencoder; the IMAGE page chooses among them and DRAW refuses with none installed. Atur: *"why image generator do not have select model
      options??"* The four files are hard-coded. The page needs a chooser over
      *installed* image models, the way MODELS lists text models, with the four
      roles named (denoiser, unconditional twin, text encoder, autoencoder) so
      a missing one is a sentence rather than a failure at step three.
- [x] **Quality below 1024 — measured, and it is the model.** The velocity
      cosine at light noise runs 0.858 / 0.898 / 0.918 / 0.934 / 0.941 across
      grids 16, 24, 32, 40, 48 — monotonic, and 2.4x worse in error terms at 16
      than at 48, before a single sampler step. **Do not go looking for a
      sampler bug**; that was the obvious next move and this removes it. The
      size labels now say what was measured.
      `research/small-images-are-the-model-2026-08-24.md`.
- [ ] **More steps at small grids**, which is the one lever left this side of
      the model: if each step's direction is 2.4x worse, more of them may
      partly compensate. Untried. The app defaults to 20 regardless of size.
- [x] **Prompt adherence — measured, and the advice was aimed at the wrong
      thing.** The plan said to offer the JSON shape rather than document it. A
      button that did exactly that was written, measured, and **not shipped**:
      over eight latents an empty structured frame conditions at **0.9x** against
      a bare phrase, which is nothing. What earns **11.3x** is the sentences —
      lighting, background, layout, a palette. The published "3x" claim came
      from one latent and was wrong in both directions; it is corrected in the
      README, `APP.md`, `chaos-draw --help` and the source node.
      `research/prompt-shape-does-nothing-2026-08-24.md`.
- [ ] **Object form**, which is the part of adherence that remains: a named
      object may still come out the wrong shape even when colour and scene
      follow. No instrument for it yet.
- [x] **`--keep-latent`.** Done, and on by default from the window. A 128x128 draw took 21 s; `--from-latent` re-decoded it in **0.7 s** to a byte-identical PNG. Six hours of correct denoising was thrown away
      because the PNG was written and the latent discarded. A latent is a few MB;
      keeping it means a re-decode is seconds instead of a night.
- [x] **A CFG control.** Done: the IMAGE page offers guidance 2/4/6 and "no
      guidance -- half the time", and the time estimate follows it. Guidance
      runs a second 5.26 GiB denoiser on every step, so it is exactly a factor
      of two, and the estimate assumed it was always on.
- [ ] **A negative prompt.** *Not done, deliberately.* This pipeline's guidance
      uses a separately trained unconditional twin fed **no text at all**, not
      an empty prompt, so a negative prompt means running the *conditional*
      model on the negative text instead -- a real change to which weights run
      on every step. Worth doing, and not worth shipping unmeasured: the
      quality harness that would judge it does not exist yet.

## Part 2 — The model list, properly managed

**Target 3.** Atur: *"list of model better management and sort and structured
for users"*, and *"when i switch between available and installed models,
installed models load with lag and make problem"*.

- [x] **The tab-switch stall.** Done. Reproduced first, as the plan required: **1584 ms** per switch to INSTALLED, of which 1885/1889 was `why_incomplete` re-reading every container's header. Cached against length and mtime, and moved to a worker. Now **10.8 ms**. Reproduce first — it may be `rescan()` doing
      disk I/O on the UI thread on every switch. If so, cache and refresh in the
      background; the list must never block on a spinning `read_dir`.
- [x] **Sort and group.** Done: by name, by size, by what it is; and a kind per row, so an image part is recognisable before LOAD is pressed. LOAD greys out for one. By size, by name, by whether it fits this machine, and
      **by what it is for** — a chat model and an image model in one flat list is
      the confusion behind *"why can I draw without loading a model"*.
- [x] **Search.** Done. Measured in the window: "qwen" 11 rows, "GEMMA" 6, "q4_k_m" 29, each under 11 ms.
- [~] **Download problems.** Atur: *"download models have problem"* — the
      specific failure is still not known, and `chaos-pull` already handles
      resume and the too-large-file trap. One thing found while looking: **the
      catalogue said nothing about what was already downloaded**, so a model
      fetched ten minutes ago looked identical to one never fetched. The
      autoencoder was worse — `flux2-vae.safetensors` is not a GGUF, so it
      never appeared on INSTALLED either: downloaded, and invisible in both
      lists. AVAILABLE now marks what is on disk. Whether that was his
      complaint is unknown; **ask before assuming it is closed.**
- [x] **CHAT and IMAGE follow the model.** Both page headers now say what the
      page can do *at this moment* — which model is loaded, how many image
      models are installed, and what to do if the answer is none — instead of a
      fixed sentence describing the page in general. Whether the thing in front
      of you will work is the question, and it has a specific answer.

## Part 3 — The logo, everywhere, correctly

Atur: *"that svg logo must be in center of app icon and logo and everywhere we
use it; sometimes it is not exactly in center … you must change svg file size
for each place and export an image icon exactly with svg for that place"*.

- [x] **Audit every place the mark is drawn**: the `.ico` at nine sizes, the rail
      at 64px, the setup window at 96 and 56, the terminal banner, the README.
- [x] **Centre it by ink, not by canvas.** It already was, in `ink_box`. The defect was elsewhere: `make-ico.py` floored an odd margin, putting **four of nine sizes** a pixel left and a pixel high. `ink_box` already finds the drawing's
      bounding box; the centring must use that, because an SVG's canvas and its
      ink are not the same rectangle and that is exactly how a mark ends up
      looking off-centre while being mathematically centred.
- [x] **Render per place at that place's exact pixel size** from the vector —
      never scale a raster. This is already true of the `.ico` and the rail; the
      audit is to find the places where it is not.
- [x] **A test that the mark's ink is centred** at eight sizes, ablated with a deliberate two-pixel shift. Plus `tools/check-logo-centred.py`, which measures the shipped `.ico`: **4 of 9 exact before, 8 of 9 after**. to within a pixel at each size,
      so this cannot drift back.

## Part 4 — Android, in every release

**Target 5.** Full audit in `backlog/android-app.md`. Atur has now answered the
question that node asks:

> *"we see devices as a resource with chaos … yeah we run a model in there,
> more simple and smaller models"*

**So it is both, and in this order:**

- [x] **Phase A — a client.** Done and building in CI: an 887 KB APK with manifest, dex, resources and all ten icons. **Never run** — see `android-app.md` for why the SDK cannot be installed here. Kotlin against `/v1/chat/completions`, talking to a
      Chaos on the PC. No NDK, no ggml, no Rust on the device. This makes the
      *big* models usable from the phone, which local inference never can, and it
      is the shortest path to an `.apk` that does something real.
- [ ] **Phase B — small models locally.** 1B–4B quantised, fully resident.
      Needs the NDK, ggml for `aarch64-linux-android`, and the Rust core
      cross-compiled. `core/probe` is the only crate with a platform assumption
      that has to change.
- [x] **Phase C — the phone as a worker.** Decided: rejected for now, reason recorded. Rejected for now, and the reason is
      in `devices-as-resources.md`: Wi-Fi latency and battery make a phone a poor
      member of a layer loop. Revisit only with measurements.

**Toolchain, none of which is installed**: JDK 17, Android SDK + platform-tools,
NDK r26+, Gradle, `rustup target add aarch64-linux-android`. Several GB, over a
connection that has already failed twice this week on GitHub's asset host.

**Definition of done**: `Chaos-vX-android-arm64.apk` attached to the release by
CI, installs on a real phone, opens, and does what the notes say it does.

## Part 5 — Devices as resources

**Target 6**, and the most interesting thing on the list. Full arithmetic in
`backlog/devices-as-resources.md`. The short version:

- Activations are **16 KB**; expert weights are **3.3 GB per token**. Send the
  work to the weights, never the weights to the work.
- Expert-parallel MoE over 1 GbE costs **~66 ms/token** of network to replace
  **~1560 ms** of disk. That is the win.
- **Four machines get single-digit tok/s on V4-Flash, not 20.** Say so before
  building it, not after.
- Order: `chaos-worker` on loopback → expert routing with local fallback →
  **measure and stop** → discovery → tensor-parallel only if the numbers earn it.

**Step one is done and the measurement is in**:
`research/worker-protocol-measured-2026-08-24.md`. A token's expert-parallel
traffic is **4.94 MB**, 39.5 ms of 1 GbE plus **36.0 ms of measured protocol
cost**, replacing **1560 ms** of disk. Activations over a real socket are
**bit-identical** to the local path. The stop is here: the next number is a LAN
round trip on two machines, and nothing should be wired into the forward pass
until somebody has it.

## Part 6 — R6, self-configuration — **done**

Read the probe, pick quant, cache size, prefill block and I/O mode, **and say
the expected tok/s before doing anything**. `--auto` now does all of it:

```
threads    -t 4 to generate, -tb 20 to prefill
batch      -b 512 tokens per prefill block, from 5.5 GiB free
io         direct, bypassing the page cache -- the model does not fit
expect     about 1.42 tok/s -- 1.02 GiB per token at 2.07 GiB/s
```

**Measured on that run: 1.51 tok/s.** Within 6%. It said 4.25 first, because a
token's expert slice was computed as the pool over the layer count -- a
plausible-looking quantity that is 3x too small.

The original text: On a phone this
matters more, not less: the download is somebody's data allowance.

*(R7, R8 and R9 were merged in PR #55. If those numbers mean something else to
Atur, ask before assuming.)*

## Part 7 — Actually run the thing

Atur: *"you must run app and check every function and work and wait, check it is
really work"*. Fair, and earned — several things shipped this week that a single
run would have caught.

- [x] A written pass over **every** control on every page. `scripts/run-through.ps1`: **22 exercised, 6 skipped by policy, worst blocking call 36.9 ms**. DELETE, RESET and BROWSE are listed rather than pressed.: what was clicked,
      what happened, whether it matched the label.
- [x] The crash on the INSTALLED/AVAILABLE tabs: **still not reproduced**, now over 60 rapid switches on the rewritten code -- worst 28.9 ms, process alive and responding. The fault handler writes an address, so the next occurrence is evidence. Original text: **still not
      reproduced** — 20 rounds of real clicks survived. The fault handler now
      writes an address, so the next occurrence is evidence instead of silence.
- [ ] Install → update → uninstall from a real downloaded setup, on the machine,
      with the models directory counted before and after.

---

## Order, and why

1. **Part 2's tab stall and Part 1's model selection** — daily friction, and the
   two things he hits every session.
2. **Part 3, the logo audit** — small, bounded, visible, and he has raised it
   three times.
3. **Part 7, the full run-through** — before any release, not after.
4. **Part 4 Phase A, the Android client** — the shortest path to the `.apk` he
   asked for, and it needs no NDK.
5. **Part 5, `chaos-worker` on loopback** — the protocol is provable on one
   machine; a second machine only changes the numbers.
6. **Part 6** — folds naturally into the worker's assignment logic.
7. **Part 1's remaining quality work** — the longest pole, and the one where
   measurement matters most.

**Not in this release**: 20 tok/s on V4-Flash. Excluded by Atur's own
instruction, and measured as out of reach on this hardware — 1.19 tok/s with
every expert resident, because 0.84 s of every token never touches the disk.
