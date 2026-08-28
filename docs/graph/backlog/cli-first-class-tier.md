---
topic: the command line as a first-class tier, not a lesser one
status: mostly done — 6 of 7 items closed and measured, 1 declared not built
links:
  - v0-0-3-the-complete-version.md
  - ../research/secure-context-decision-2026-08-28.md
---

# The CLI is a tier, not an afterthought

Atur, twice: **someone may run Chaos on a headless server and connect from their
own machine, and the CLI must not be a lesser tier.** §3 of the plan lists what
that means. This is the scoreboard, with what was measured.

## The scoreboard

| | item | state |
|---|---|---|
| 1 | print the route, and print it as a QR in the terminal | **done** before this round (`chaos-qr`) |
| 2 | start and stop a node as a managed process | **done** — `chaos start` / `stop` / `status`, pid file, log |
| 3 | pick model, device, threads, context, cache from one place | **done** — one settings file, read by both tiers |
| 4 | report what `/status` serves, without curl | **done** — `chaos status`, over a client written for it |
| 5 | connect to another node and use it | **done** — `chaos connect <route> "prompt"`, streamed |
| 6 | read a code: `chaos scan <image.png>` | **NOT BUILT, declared** — see below |
| 7 | one binary or many | **decided: `chaos <subcommand>`, every old name kept** |

Plus §3's closing ask — installable anywhere there is a terminal, with
completions — which is partly measured and partly not; see the last section.

## What the shared settings file fixed

The plan's complaint was exact: *"the app has a settings file the CLI cannot
read."* It could not, because `Settings` lived inside `gui/app`. It now lives in
**`core/config` (`chaos-config`)** and `gui/app` re-exports it, so every existing
`settings::` call site in the window is unchanged while the command line reads the
same `~/.chaos/settings.txt`.

`chaos start` builds the server's flags with `Settings::serve_args` — **the same
function the window calls**, not a second list that could drift. `chaos config`
prints the resolved settings and the exact `chaos-serve` line they produce, with
the key redacted.

## Measured, on a real node

A throwaway `USERPROFILE` was used throughout, so none of this touched the real
profile. Model: `Llama-3.2-1B-Instruct-Q4_K_M`.

```
chaos start Llama-3.2-1B      node starting, pid 23660 -- up in ~1 s
chaos status                  local node   pid 23660
                              reachable    127.0.0.1:8231
                                model        Llama-3.2-1B-Instruct
                                route        http://127.0.0.1:8231
                                context      2048
                                off loopback false
chaos connect 127.0.0.1:8231 "Name one colour. Answer in three words."
                              Red
chaos start (again)           refuses: a node is already running (pid 23660)
chaos stop                    stopped pid 23660
chaos stop (again)            no node was started from this machine
stale pid file (999999)       "pid 999999 is gone. Clearing the pid file."
```

Every one of `status`, `connect` and the readiness check goes through
`core/http`, written for this: **no `curl` anywhere on those paths.** Model
downloads still shell out to curl, deliberately — that needs `https`, and TLS is
the one thing this client does not do.

### Two bugs the measurement caught, which reading would not have

- **`chaos start` reported success over a node that had already died.** The first
  run printed `node starting, pid 852` for a server that exited immediately
  because it could not find the model, leaving a pid file, no node, and nothing on
  screen pointing at the log that explained it. `start` now waits 600 ms and
  reports the exit code and the log's tail. 600 ms catches the two failures that
  are immediate — wrong model name, port in use — without waiting on a load that
  legitimately takes minutes.
- **The first liveness check was wrong on Windows in a way that looked right.**
  `OpenProcess` succeeds on an *exited* process while any handle to it is still
  open, and the parent was holding one — so `alive(pid)` returned true for a dead
  child and the check passed. Fixed twice over: the post-spawn check asks
  `Child::try_wait`, which owns the handle and cannot be fooled, and `alive()`
  itself now reads `GetExitCodeProcess` and requires `STILL_ACTIVE`.

## Item 6: `chaos scan` is not built, and says so

**`core/qr` encodes and does not decode.** The plan's instruction was to *"say so
rather than half-building it"*, and that is what `chaos scan` does: it is listed
in the help as `NOT BUILT`, and running it prints why, plus the two readers that
do work and are measured (210 of 210 across 7 scales and 30 angles) — the phone's
SCAN button and `/scan` in a browser on the node itself.

**Why the asymmetry is real and not laziness.** Encoding is arithmetic with a
known answer, checkable against a reference grid. Decoding a photograph is
thresholding, finder detection, a perspective basis, format and version recovery,
de-masking, and Reed-Solomon *correction* rather than the syndrome check the
encoder's tests use. Every one of those stages fails by returning a plausible
wrong string rather than an error — which is the worst failure shape there is in
this project, and the reason `VERIFIED_ARCHITECTURES` exists at all.

**If it is built, the bar is set by what already exists**: bit-for-bit agreement
with `assets/grimoire/scanner.html`'s detector across the nine reference payloads
in `core/qr/tests/reference-grids.txt`, and every grid round-tripped through
`assets/grimoire/decode_qr.py`, which was written from the reading side. Anything
less is a decoder nobody should trust.

A narrower slice is available and worth noting: **a clean, axis-aligned grid from
a PNG** — no camera, no perspective — is a fraction of the work, and would cover
"read the code I just printed". It still needs de-masking and a bitstream reader,
and it would still have to agree with the nine grids.

## Installable anywhere there is a terminal

**Tested here, on this machine:**

- `cargo install --path cli/chaos --root <dir>` produces a runnable `chaos`
  (`chaos --version` → `chaos 0.0.21`).
- **The front door needs no ggml.** `chaos-cli`, `chaos-config`, `chaos-http` and
  `chaos-grammar` all build with `GGML_LIB_DIR` unset, so `cargo install` works on
  a machine that has never compiled a line of C. CI now checks all four in its
  ggml-free job.
- Completions are generated for **bash, zsh, fish and powershell** from one list,
  so a subcommand added to `ALIASES` or `OWN` appears in all four; an unknown
  shell is refused with the four named. A test asserts every verb appears in every
  shell's script.
- **Two of the four are sourced into their real shell and driven**, which is the
  difference between a script that exists and a completion that works:

  | shell | how it was checked | result |
  |---|---|---|
  | bash | sourced in `bash --noprofile --norc`, `_chaos` called directly | `complete -F _chaos chaos` registered; `st` → `start stop status`; `co` → `connect config completions`; **17 verbs** |
  | powershell | dot-sourced, then asked `TabExpansion2` | `chaos st` → `start stop status`; **17 verbs** |
- `chaos` and `chaos-qr` are now in the release workflow's three staging lists and
  in `make-linux-packages.sh`. **`chaos-qr` was missing from all of them** — the
  brand tier claims it "reaches a bare terminal" and the binary that does it was
  never packaged.

**Plausible, and NOT tested — do not claim these:**

- **zsh and fish completions have never been sourced** — neither shell is
  installed here. Their scripts are generated and their content asserted, and
  that is all. bash and powershell are tested; see the table above.
- winget, scoop and homebrew manifests **do not exist**.
- `make-linux-packages.sh` exists and now names the new binaries, but has not been
  run in this round, and no `.deb` or tarball it produces has been installed
  anywhere.
- The release workflow's staging lists are edited but **the change has not been
  through a tag**, so no published archive yet contains `chaos`.

## What is left

1. **Source the completion scripts in a real bash, zsh and fish.** Cheap, and
   currently the gap between "generated" and "works".
2. **A tag, to prove `chaos` actually ships.** The lists are right in the source;
   nothing has published from them.
3. **`chaos scan`**, if it is wanted, at the bar set above.
4. Package manifests, if Atur wants them — each is only worth writing if someone
   will install from it and report back.
