# v0.0.3 — the complete version

**This file is a prompt.** Paste it into a fresh session after `/init`. It is the
whole agreement for the next stretch of work: what exists, what is half-built,
what has never been built, and what "done" means for each.

Read `STATUS.md` first, then `CLAUDE.md`, then this. Where they disagree, they
are right and this is stale — say so rather than following it.

---

## 0. The rule that matters most

**Do not report progress without a measurement.** This project has a history of
claims that did not survive checking: a "12 of 12 models work" that came from
exit codes, a brand colour "that does not exist" that came from reading an
untracked leftover file, a QR bug found only by a bit-for-bit diff after the
picture looked perfect. An exit code is not a diff. A render is not a scan. A
frame rate measured before the particle count settles is not a frame rate.

When you finish a task, say what you measured and paste the command.

---

## 1. Where the work stands

### Done and verified
- `assets/grimoire/grimoire.html` — the interactive QR mark. One self-contained
  page: a book with the Chaos mark burns inside a rune circle, turns its own
  fourteen leaves, and the writing scorches onto the leaves. Both themes.
  Brand blue `#0000F2` throughout, taken from
  `android/app/src/main/res/values/colors.xml`.
  - QR encoder written from scratch (byte mode, versions 1–6, level Q).
    **Verified**: every Reed-Solomon syndrome zero, and bit-for-bit identical to
    `python-qrcode` including the auto-chosen mask. `assets/grimoire/decode_qr.py`
    is the independent decoder; `assets/grimoire/README.md` documents the method
    and every trap paid for.
  - Endpoint resolved at runtime: `window.CHAOS_ENDPOINT` → `?endpoint=` →
    `location.origin` (when served by the node and not loopback) → fallback.
    Re-cuts the code on network change.
  - 0.6 ms a frame at 1440×810.

### Not verified
- **The rendered code has never been scanned by a camera.** The grid verifies;
  the *rendering* (eased module corners, ink variation, the gutter) does not.
  **First task: point a phone at it and confirm, on both themes, at a few
  distances.** If it fails, the module treatment is the thing to back off.

### Never built
- The QR **scanner** (below).
- Any integration into the desktop app, Android app, or CLI.

---

## 2. The scanner — "a magical QR reader"

A second mode: no book, just the rune circle, and what the circle frames is
read. Point it at a code and it resolves. No square viewfinder — the circle is
the viewfinder.

**Decide these before writing code, and tell Atur the answer:**

1. **Detection, not just decoding.** Reading a QR from a camera frame means
   locating the three finder patterns, correcting perspective, sampling the
   grid, then Reed-Solomon decoding. `BarcodeDetector` does all of it and is
   present in Chrome and Android WebView — and **absent in Safari and on iOS**.
   If iOS must work, that is a full detector to write and a much larger job.
   Ask before assuming.
2. **Camera access needs a secure context** — HTTPS or `localhost`. It will not
   work from a plain `http://192.168.x.x` origin, which is exactly how a node on
   a LAN is reached. Settle how the app serves this before building on it.

**Definition of done**: scans a code shown on another screen and on paper, in
both themes, on at least one Android device and one desktop browser, with the
failure path (no camera, no permission, no `BarcodeDetector`) saying something
useful rather than nothing.

---

## 3. Into every application

The book and the circle are **3-D elements that belong to the product**, not a
web page that happens to exist. Atur's model:

- **Core mode**: the book is live. It carries the node's own route, so another
  machine can scan it and reach this server while the app is running.
- **Otherwise**: the circle alone, as the scanner.
- **On connecting to a server**: that server's book appears for this user too.

### 3a. `chaos-serve` (the obvious first move)
`core/arch/src/ui.rs` already serves a browser UI. Serve the grimoire from it —
that is the case the endpoint logic was designed around, because `location.origin`
then genuinely *is* the route another machine uses, and it changes when the
network does. This single step reaches desktop and Android at once.

Also add the `/status` endpoint the page already polls (model, quant, size,
context, device, tokens per second, prefill), so the node can describe itself.

### 3b. Desktop (`gui/app`)
A Rust Win32 window. Decide honestly whether it hosts a webview or whether the
book is re-implemented natively — and say which, with the reason, before
building. Do not silently pick one.

### 3c. Android (`android/`)
Native client. Same decision, same rule.

### 3d. CLI — **no GUI at all**
Atur is explicit: someone may run Chaos on a headless server and connect from
their own machine. **The CLI must not be a lesser tier.** Everything the GUI can
do, the CLI must do:
- start/stop the node, pick a model, set device/threads/context
- print the route, and print the QR **in the terminal** (half-block or ASCII
  cells — a terminal can render a scannable QR, and this is the natural way to
  share a headless node's address)
- report status the same fields `/status` serves
- connect to another node and use it

**Installable anywhere a terminal is**: `cargo install`, a plain binary on
PATH, PowerShell, cmd, bash, zsh. Work out packaging (winget / scoop / homebrew
/ a tarball) and write down what is actually tested versus merely plausible.

---

## 4. The heavy analysis

Atur wants a full pass over the project. Each of these is a deliverable with a
written finding, not a vibe:

- **Performance** — where the time and memory actually go. Measure; do not
  re-propose anything in the "dead ends" list in `CLAUDE.md` without a number
  that beats it.
- **Features** — what is claimed versus what works. Every claim in `STATUS.md`
  and `README.md` re-checked against a command that proves it. Retract what does
  not survive.
- **Folder structure** — `core/` `cli/` `network/` `gui/` per the Rust book.
  Note that `crates/` exists untracked in at least one worktree and is *not* the
  real tree; reading it caused a wrong conclusion about the brand palette. Check
  `git ls-files` before concluding a file is authoritative.
- **Tests** — coverage of what matters, not line count. The rule that a wrong
  forward pass produces fluent nonsense rather than a crash means an exit code
  proves nothing; a diff against a reference does.
- **Production readiness** — install, update, uninstall, on a clean machine.
  Crash behaviour. What happens when a model file is corrupt, a disk fills, a
  network drops mid-stream.
- **Open-source readiness** — licence and its compatibility with everything
  vendored, CONTRIBUTING, issue/PR templates, a build that a stranger can
  reproduce, no secrets in history, CI that runs on a fork.

### Things not asked for that belong in the plan
- **Security**: the node binds a port and serves. Who can reach it? Is there any
  auth? A QR that hands out your LAN address is a feature *and* an exposure —
  decide deliberately and document the decision.
- **Licensing of the model weights** as distinct from the code.
- **Versioning and upgrade**: what happens to a user on 0.0.2 when 0.0.3 lands.
- **Reproducible builds and an SBOM**, if this is to be trusted software.
- **Accessibility** in the GUI and the web UI — keyboard, contrast, reduced
  motion. The grimoire page already honours `prefers-reduced-motion`.
- **No telemetry**, stated explicitly, because people will ask.
- **Error messages a person can act on** — a rule already applied in the
  grimoire's page and worth applying everywhere.

---

## 5. Working rules that bit somebody already

- Push with the token from `C:\Projects\.env` inline in the URL, redacted in
  output. Never in git config, never echoed, never `git push -u`.
- Verify containment with `git merge-base --is-ancestor` *before* deleting a
  branch. After merging, fast-forward `main` from `origin/main` explicitly and
  re-run the tests on `main` itself.
- GitHub parses only the **first** issue in `Closes #1, #2, #3`.
- Never bare `git stash` / `git stash pop` — the stash stack is shared across
  worktrees and other sessions.
- The browser pane does not composite a hidden tab, so its screenshot tool times
  out and a canvas nobody drew into looks exactly like a broken renderer. The
  grimoire exposes `window.__grimoire.run(n, ms)` for this reason.
- A local HTTP server sends `Last-Modified`; the browser will serve a stale page
  and you will measure code you are not running. Bust the cache.

---

## 6. Definition of done for v0.0.3

A release where:
- the book and the circle appear in the desktop app, the Android app and the
  web UI, from one source of truth;
- the CLI does everything the GUI does, installs from a terminal on Windows,
  macOS and Linux, and prints a scannable QR without a GUI;
- the scanner works, or its limits are documented and surfaced to the user;
- every claim in `STATUS.md` is backed by a command in a doc;
- the analysis in §4 is written down, with the findings acted on or explicitly
  deferred with a reason.

**No gaps and no misses** — Atur's words. Where something cannot be done, the
plan says so out loud rather than leaving it quiet.
