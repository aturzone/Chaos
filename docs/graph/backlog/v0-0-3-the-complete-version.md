# v0.0.3 — the complete version

**This file is a prompt.** Paste it into a fresh session after `/init`. It is
the whole agreement for the next stretch of work: what exists, what is
half-built, what has never been built, and what "done" means for each.

Read `STATUS.md` first, then `CLAUDE.md`, then this. Where they disagree, they
are right and this is stale — say so rather than following it.

Atur's words for the target: **a professional, complete version with no gap and
no miss.** Where something cannot be done, this plan says so out loud rather
than leaving it quiet — and so should you.

---

## 0. The rule that matters more than anything below

**Do not report progress without a measurement.** This project has a history of
claims that did not survive checking, and every one of them looked fine at the
time:

- "12 of 12 models work" — came from exit codes, not from output.
- A brand colour "that does not exist" — came from reading an **untracked
  leftover** file. `crates/` still sits untracked in at least one worktree and
  is *not* the real tree. Run `git ls-files` before concluding a file is
  authoritative.
- A QR grid that rendered perfectly and no scanner would touch — two clobbered
  timing modules out of 1089, found only by a bit-for-bit diff.
- "The reader works: 15/15 across 4, 6 and 8 pixels per module" — true, and it
  covered the wrong range. It failed above ten pixels per module, which is what
  happens the moment a phone is close enough for the code to fill the frame.
  Three separate causes; see `STATUS.md` 2026-08-27.
- "20 tok/s on V4-Flash" — closed from both sides with numbers. Do not reopen it
  without new hardware.

An exit code is not a diff. A render is not a scan. A frame rate measured before
the particle count settles is not a frame rate. **A sweep in multiples of 45
degrees is not a rotation test.** When you finish a task, say what you measured
and paste the command.

---

## 1. Where the work stands

### Done and verified — the brand tier

`assets/grimoire/` holds two self-contained pages and they now reach every tier
of the product from one source:

- **`grimoire.html`** — a book bearing the Chaos mark, burning inside a rune
  circle, which turns its own fourteen leaves and opens onto a QR code cut from
  **this node's route**. Both themes. Brand blue `#0000F2`, from
  `android/app/src/main/res/values/colors.xml`.
- **`scanner.html`** — the same circle as a viewfinder, with a complete QR
  detector behind it, because `BarcodeDetector` is absent on desktop Windows
  (measured) and on iOS.
- **`chaos_arch::grimoire`** wraps both into real documents, splices in
  `fonts.css` in place of the font `<link>`s, and injects
  `window.CHAOS_ENDPOINT`. A test asserts the assembled page has no fetchable
  external reference: **0 `<link>` elements, 6 font faces loaded, 0 network
  requests**, measured in a browser.
- **`chaos-serve`** serves `GET /qr`, `GET /scan`, `GET /status`, and
  `--emit-pages <dir>` writes the wrapped documents for a host that embeds them.
- **Desktop** `gui/app`: SHOW THE MARK / READ A CODE on the CHAOS page,
  `shell_open` of the node's URL with the window's theme. **Stated decision**:
  the window is Win32 with GDI painting and no webview; it opens the served page
  rather than re-drawing it, so every tier shows the same bytes.
- **Android**: `BrandActivity`, a WebView over `https://chaos.invalid` that the
  app answers itself in `shouldInterceptRequest` — bundled asset first, then the
  node. That fictitious `https` origin is what makes a **secure context**, which
  a camera requires and a LAN node over plain http can never be.
- **CLI, with no GUI at all**: `core/qr` and the `chaos-qr` binary draw a
  scannable code in a terminal. `chaos-serve` prints its own route as one at
  startup when the node is reachable (`CHAOS_QR=1`/`0` overrides).

**The evidence, because this is the part that counts.** See `STATUS.md`
2026-08-27 for the numbers; the short form:

- The Rust encoder is **bit-for-bit identical** to the page's JavaScript one
  across nine payloads spanning versions 1-6 and multi-byte UTF-8; every grid
  also goes through `assets/grimoire/decode_qr.py` (written from the reading
  side) and must return its exact payload with all syndromes zero.
- Where `python-qrcode` differs it is a **mask choice**, and by a third scoring
  of ISO 18004's rules ours is better every time (311 v 416, 334 v 436,
  296 v 325).
- The reader: **210 of 210** across 7 scales and 30 angles; the mark's own
  rendered artwork read from 1600 px down to 120 px wide; blur to 3.0 px;
  contrast to `#606060` on `#9a9a9a`; quiet zone to zero; **0 false positives on
  40 noise frames**; 22 ms median at the camera path's working size.
- `chaos-qr`'s own terminal output, parsed back and decoded independently:
  exact payload, every syndrome zero.
- **909 tests pass** (`cargo test --release`), up from 889.

### Not verified, and you should fix that first

1. **No real camera has ever seen either page.** There is none in the
   environment they were built in, so `getUserMedia` was exercised only down its
   failure paths. **First task: point a phone at the mark on a screen and at
   paper, both themes, several distances, and scan it with the reader.** If the
   mark fails, the module treatment is the thing to back off; if the reader
   fails, `window.__scry` exposes every stage so you can find out which one.
2. **The Android tier has never been built.** `dl.google.com` 404s that whole
   network, so the SDK will not install and release CI is the only build. The
   Kotlin is written against the documented API and unit-tested only where it
   can be (`BrandTest`). Expect to fix it on first run, and run it on an
   emulator at minimum — running the app found four defects a build never would.
3. **`--emit-pages` is wired into release CI but has never run there.**
4. **The desktop buttons compile and are laid out; nobody has clicked them.**
   `scripts/poke-app.ps1` clicks a control and times the UI thread.

### The open question nobody can answer from a desk

**How does the app serve a secure context?** The Android answer is the
intercepted `https://chaos.invalid` origin. The desktop and the plain browser
have no such trick: open `http://192.168.1.20:8080/scan` on a second machine and
the camera will not open, and the page says so rather than failing silently.
Options, none free:

- a self-signed certificate and a trust prompt per device;
- `localhost` only, which is the one non-TLS secure origin, and then the reader
  works on the node itself and nowhere else;
- accept it: the *mark* needs no camera and works everywhere, and the *reader*
  is a phone feature.

**Decide this deliberately and write the decision down.** Whichever way it goes,
the failure message the page shows today is correct and should stay.

### Never built

Everything in sections 3 and 4 below.

---

## 2. iOS

`BarcodeDetector` is absent on Safari, which is why the bundled detector exists
— so the reader *should* work there. It has never been opened on an iPhone.
**Ask Atur whether iOS matters before spending anything on it**, and if it does,
the test is the same one: point it at a code.

---

## 3. The CLI as a first-class tier

Atur is explicit and has said it twice: **someone may run Chaos on a headless
server and connect from their own machine, and the CLI must not be a lesser
tier.** Part of this is done — the route prints as a scannable code — and the
rest is not.

Everything the GUI can do, the CLI must do:

- [x] print the route, and print it as a QR **in the terminal**
- [ ] **start and stop a node as a managed process** — today `chaos-serve` runs
      in the foreground and that is all. A person over SSH needs
      `chaos start` / `chaos stop` / `chaos status`, a pid file, and a log they
      can tail.
- [ ] **pick a model, set device, threads, context, cache** from one place. The
      flags exist across `chaos-run` and `chaos-serve` and disagree in places;
      the app has a settings file the CLI cannot read.
- [ ] **report what `/status` serves**, without curl.
- [ ] **connect to another node and use it** — a client mode. `chaos-run
      --connect <route>` does not exist; the Android app and the desktop both
      have one.
- [ ] **read a code**, not just print one: `chaos scan <image.png>`. The
      detector is JavaScript today. A Rust decoder is a real piece of work and
      `core/qr` only encodes — say so rather than half-building it.

**One binary or many?** There are already `chaos-run`, `chaos-serve`,
`chaos-probe`, `chaos-model-info`, `chaos-pull`, `chaos-draw`, `chaos-qr`,
`chaos-worker`, `gguf-info` and three benchmarks. A person who installs this
types `chaos`. **Decide** whether `chaos <subcommand>` becomes the front door
with the rest as internal binaries, and if so do it as one deliberate change
with the old names kept working, not as a drift.

**Installable anywhere there is a terminal.** `cargo install`, a plain binary on
PATH, PowerShell, cmd, bash, zsh, and shell completions for each. Work out
packaging — winget, scoop, homebrew, a tarball, maybe `.deb` — and **write down
what is actually tested versus merely plausible.** A package nobody installed is
not a package.

---

## 4. The heavy analysis

Atur wants a full pass over the project. Each of these is a deliverable with a
written finding in `docs/graph/`, not a vibe. Take them one at a time and finish
one before starting the next.

### 4a. Performance
Where the time and the memory actually go. Measure; do not re-propose anything
in `CLAUDE.md`'s dead-ends list without a number that beats it. The dead ends
are: expert factorisation, contextual sparsity, a pinned hot set, expert-read /
compute overlap (1.03x), `--op-offload` (19% slower), `mul_mat_id` batching on
the streaming path, and porting parallel-experts to V4-Flash.

Untouched ground worth measuring: the prefill path on the dense side (1.20-1.27x
behind llama.cpp hand-tuned), the tokenizer, model load time, and the GUI's
worst blocking call.

### 4b. Features — what is claimed versus what works
Every claim in `STATUS.md` and `README.md` re-checked against a command that
proves it. **Retract what does not survive**, in the same voice the existing
retractions use. There are already several; adding to that list is a good
outcome, not a bad one.

### 4c. Folder structure
`core/` `cli/` `network/` `gui/` per the Rust book, and it mostly holds. Open
items: `core/qr` was added this round and `cli/` still contains one crate while
`core/` contains a dozen binaries. Note that **`crates/` exists untracked and is
not the real tree** — check `git ls-files`.

### 4d. Tests
Coverage of what matters, not line count. The governing fact: **a wrong forward
pass produces fluent nonsense rather than a crash**, so an exit code proves
nothing and a diff against a reference proves everything. Ask of each area: what
would a wrong answer look like, and would anything catch it?

Known thin spots: `network/serve` has no test directory at all — its routing,
its SSE framing and its `/status` shape are unasserted. The GPU tests **skip**
rather than fail without a card, and a green "6 passed" was once reported for a
file whose two GPU tests never ran once.

### 4e. Production readiness
Install, update, uninstall on a clean machine —
`scripts/install-update-uninstall.ps1` does this on Windows and nothing does it
elsewhere. Crash behaviour. What happens when a model file is corrupt, a disk
fills, a network drops mid-stream, two instances start at once.

### 4f. Open-source readiness
Licence and its compatibility with everything vendored — **including the fonts
now embedded in the binary** (OFL 1.1, notices in
`assets/grimoire/fonts/NOTICE`, which the licence requires be preserved and
which a stripped build could lose). CONTRIBUTING, issue and PR templates, a
build a stranger can reproduce, no secrets in history, CI that runs on a fork.

### 4g. Things not asked for that belong in the plan
- **Security.** The node binds a port and serves. Who can reach it? `--api-key`
  gates `/v1/*` and nothing else, deliberately — but `/qr` now hands out the
  machine's LAN address to anyone who loads it, and `/status` names the model.
  That is a feature *and* an exposure. Decide, and document the decision.
- **Model weight licensing**, as distinct from the code's.
- **Versioning and upgrade**: what happens to someone on 0.0.2 when 0.0.3 lands.
- **Reproducible builds and an SBOM**, if this is to be trusted software.
- **Accessibility** in the window and the web UI — keyboard, contrast, reduced
  motion. Both brand pages already honour `prefers-reduced-motion` and the
  reader has an `aria-live` region; the window has neither reviewed.
- **No telemetry**, stated explicitly somewhere a reader will find it, because
  people will ask.
- **Error messages a person can act on** — a rule already applied in the brand
  pages and in `chaos-qr`, and worth applying everywhere.

---

## 5. Working rules, each bought with a mistake

- Push with the token from `C:\Projects\.env` inline in the URL, output
  redacted. Never in git config, never echoed, **never `git push -u`**.
- Verify containment with `git merge-base --is-ancestor <branch> origin/main`
  *before* deleting a branch. After merging, a local `main` with no upstream
  makes `git pull` a silent no-op — fast-forward from `origin/main` explicitly,
  check a file only the merge added, then **re-run the tests on `main` itself**.
- GitHub parses only the **first** issue in `Closes #1, #2, #3`.
- Never bare `git stash` / `git stash pop`: the stack is shared across worktrees
  and other sessions.
- **Windows needs the GNU toolchain and MSYS2 mingw64 on PATH.** Without
  `/c/msys64/mingw64/bin` the build fails with `linker
  x86_64-w64-mingw32-gcc not found` and nothing about it names the cause.
- **An XML comment containing `--` breaks aapt2.** A test catches it; it caught
  it this round.
- The browser pane does not composite a hidden tab, so its screenshot tool times
  out and a canvas nobody drew into looks exactly like a broken renderer. Both
  brand pages expose a test API for this — `window.__grimoire.run(n, ms)` and
  `window.__scry.readCanvas(canvas)` — and they are how everything above was
  measured. **Use them instead of looking.**
- A local HTTP server sends `Last-Modified` and the browser will serve a stale
  page; you will measure code you are not running. Bust the cache.
- `python-qrcode`'s `add_data(data, optimize=0)` is what forces pure byte mode.
  Left on, it splits a URL into mixed-mode segments — a smaller, perfectly valid
  code, and not the one under test. Five of nine payloads looked like a bug
  because of it.
- On Windows a redirected `print` defaults to cp1252, so a non-ASCII payload
  comes back mojibake and looks like a decode failure. `PYTHONIOENCODING=utf-8`.

---

## 6. Definition of done for v0.0.3

A release where:

- the mark and the reader appear in the desktop app, the Android app, the web UI
  and a bare terminal, from one source of truth — **and a real phone has
  scanned both**;
- the CLI does everything the GUI does, installs from a terminal on Windows,
  macOS and Linux, and prints a scannable QR with no GUI present;
- the secure-context question in §1 has a written answer;
- every claim in `STATUS.md` is backed by a command in a doc;
- the analysis in §4 is written down, with each finding acted on or explicitly
  deferred with a reason.

**No gaps and no misses** — Atur's words. If an item cannot be done, the plan
says which and why, out loud, before the session ends.
