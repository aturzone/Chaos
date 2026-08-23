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

- [ ] **Model selection.** Atur: *"why image generator do not have select model
      options??"* The four files are hard-coded. The page needs a chooser over
      *installed* image models, the way MODELS lists text models, with the four
      roles named (denoiser, unconditional twin, text encoder, autoencoder) so
      a missing one is a sentence rather than a failure at step three.
- [ ] **Quality below 1024.** 256 and 512 are visibly worse than a linear
      reading of the size ladder suggests. Measure the velocity cosine at each
      grid before blaming the sampler — this is the instrument that already
      found the flip.
- [ ] **Prompt adherence.** "A photo of a Russian girl in a bikini" produced a
      woman in a business jacket. Colour and scene follow; object form and
      clothing do not. Structured JSON-shaped prompts condition ~3x more
      strongly — the page should offer that shape rather than documenting it.
- [ ] **`--keep-latent`.** Six hours of correct denoising was thrown away
      because the PNG was written and the latent discarded. A latent is a few MB;
      keeping it means a re-decode is seconds instead of a night.
- [ ] **A negative prompt and a CFG control.** Guidance is fixed at the default
      and doubles every render's cost; a user who does not want it should be
      able to say so and halve the time.

## Part 2 — The model list, properly managed

**Target 3.** Atur: *"list of model better management and sort and structured
for users"*, and *"when i switch between available and installed models,
installed models load with lag and make problem"*.

- [ ] **The tab-switch stall.** Reproduce first — it may be `rescan()` doing
      disk I/O on the UI thread on every switch. If so, cache and refresh in the
      background; the list must never block on a spinning `read_dir`.
- [ ] **Sort and group.** By size, by name, by whether it fits this machine, and
      **by what it is for** — a chat model and an image model in one flat list is
      the confusion behind *"why can I draw without loading a model"*.
- [ ] **Search**, once there are more than about twenty rows.
- [ ] **Download problems.** Atur: *"download models have problem"* — needs the
      specific failure before anything is changed. `chaos-pull` already handles
      resume and the too-large-file trap; what is not known is which part he hit.
- [ ] **CHAT and IMAGE follow the model.** His design instinct was right: the
      app should present what the *selected* model can do, rather than two pages
      with different rules about whether anything needs loading.

## Part 3 — The logo, everywhere, correctly

Atur: *"that svg logo must be in center of app icon and logo and everywhere we
use it; sometimes it is not exactly in center … you must change svg file size
for each place and export an image icon exactly with svg for that place"*.

- [ ] **Audit every place the mark is drawn**: the `.ico` at nine sizes, the rail
      at 64px, the setup window at 96 and 56, the terminal banner, the README.
- [ ] **Centre it by ink, not by canvas.** `ink_box` already finds the drawing's
      bounding box; the centring must use that, because an SVG's canvas and its
      ink are not the same rectangle and that is exactly how a mark ends up
      looking off-centre while being mathematically centred.
- [ ] **Render per place at that place's exact pixel size** from the vector —
      never scale a raster. This is already true of the `.ico` and the rail; the
      audit is to find the places where it is not.
- [ ] **A test that the mark's ink is centred** to within a pixel at each size,
      so this cannot drift back.

## Part 4 — Android, in every release

**Target 5.** Full audit in `backlog/android-app.md`. Atur has now answered the
question that node asks:

> *"we see devices as a resource with chaos … yeah we run a model in there,
> more simple and smaller models"*

**So it is both, and in this order:**

- [ ] **Phase A — a client.** Kotlin against `/v1/chat/completions`, talking to a
      Chaos on the PC. No NDK, no ggml, no Rust on the device. This makes the
      *big* models usable from the phone, which local inference never can, and it
      is the shortest path to an `.apk` that does something real.
- [ ] **Phase B — small models locally.** 1B–4B quantised, fully resident.
      Needs the NDK, ggml for `aarch64-linux-android`, and the Rust core
      cross-compiled. `core/probe` is the only crate with a platform assumption
      that has to change.
- [ ] **Phase C — the phone as a worker.** Rejected for now, and the reason is
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

## Part 6 — R6, self-configuration

Read the probe, pick quant, cache size, prefill block and I/O mode for 8/16/48/
128 GiB, **and say the expected tok/s before doing anything**. On a phone this
matters more, not less: the download is somebody's data allowance.

*(R7, R8 and R9 were merged in PR #55. If those numbers mean something else to
Atur, ask before assuming.)*

## Part 7 — Actually run the thing

Atur: *"you must run app and check every function and work and wait, check it is
really work"*. Fair, and earned — several things shipped this week that a single
run would have caught.

- [ ] A written pass over **every** control on every page: what was clicked,
      what happened, whether it matched the label.
- [ ] The crash he reported on the INSTALLED/AVAILABLE tabs is **still not
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
