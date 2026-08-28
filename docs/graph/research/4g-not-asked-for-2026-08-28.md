---
topic: §4g — the seven things nobody asked for, checked and decided
status: resolved — two gaps fixed, three already sound, two decisions left with Atur
links:
  - ../backlog/v0-0-3-the-complete-version.md
  - 4e-production-readiness-2026-08-28.md
  - 4f-open-source-readiness-2026-08-28.md
---

# §4g: things not asked for that belong in the plan

Seven items. **Two were real gaps and are fixed, three were already sound and are
now measured rather than assumed, and two are decisions that change behaviour and
so belong to Atur.**

## 1. Security: what a node exposes without the key

§4g states the tension exactly — *"`/qr` now hands out the machine's LAN address
to anyone who loads it, and `/status` names the model. That is a feature **and** an
exposure."* Measured against a running node with a key configured:

| request, no key | result |
|---|---|
| `GET /v1/models` | **401** — gated, as §4d's tests now assert |
| `GET /status` | **200** — model name, context limit and ceiling, route, uptime, verified-architecture count |
| `GET /health` | **200** — model name, context limit |
| `GET /qr` | **200** — the page, encoding this node's route |

So on a node bound to `0.0.0.0` — which is what choosing CORE does — **anyone who
can reach the port learns which model you run, its context size and the node's
address, without the key.**

**Decision taken: leave them open, and state it.** They are how a device
discovers a node; `chaos status`, the browser page and the phone all read
`/status`, and gating it would break `chaos status` against a remote node, which
sends no key on that request today. The exposure is now written down in
`SECURITY.md` with the exact payloads, so it is a documented trade rather than a
surprise, and the mitigation is named: bind loopback (the default) and reach the
node over SSH.

**Reversed by Atur, 2026-08-28, and done in v0.0.23.** `/status` and `/health`
are now behind the key — but gated on the **peer** rather than the bind address,
which is the better rule and the one that keeps the window's own `/health` probe
on `127.0.0.1` working. This machine is never gated; the network needs the key;
`chaos status` sends it. Measured from this machine's LAN address against a node
bound to it: **`/status` 401, `/health` 401, `/v1/models` 401, `/qr` 200** without
the key, and 200 with it. The mark stays open because a stranger's phone has no
key and scanning it is the point.

## 2. No telemetry — was true and was stated nowhere

Chaos sends nothing anywhere. It was worth checking whether anyone could *know*
that:

```
$ grep -ril "telemetry" README.md STATUS.md CLAUDE.md SECURITY.md CONTRIBUTING.md docs/
docs/graph/backlog/v0-0-3-the-complete-version.md
```

**The only occurrence in the entire repository was the plan asking for it.** A
person evaluating Chaos had no way to learn it, and this is a project whose
defining property is that it downloads nothing — the one claim most worth making.

**Fixed in both places a reader looks**: the README, next to the platform note,
and a new section in `SECURITY.md`. Both state that the single request Chaos makes
on its own is the window's update check against a static JSON file, disabled by
`CHAOS_NO_UPDATE_CHECK`.

## 3. Reproducible builds and an SBOM — the SBOM is one line

```
$ grep -c "^\[\[package\]\]" Cargo.lock
22
$ ...names, none of which is third-party
chaos-android chaos-app chaos-arch chaos-build chaos-cli chaos-config chaos-ggml
chaos-gguf chaos-grammar chaos-http chaos-image chaos-io chaos-jinja chaos-model
chaos-plan chaos-probe chaos-qr chaos-run chaos-serve chaos-setup chaos-tokenizer
chaos-worker
```

**Twenty-two packages, twenty-two of them in this repository. Zero third-party
dependencies.** So the supply chain is this workspace plus a statically linked
ggml built from a named llama.cpp commit. An SBOM would be that sentence; a
CycloneDX file would add process and no information. **Stated in the README and
`SECURITY.md` rather than generated.**

Reproducibility: CI builds ggml from llama.cpp source and points `GGML_LIB_DIR` at
the result — its own comment says this is *"the same steps a contributor follows
by hand, so a green CI means the documented steps work."* What is **not** pinned is
the llama.cpp commit: `git clone --depth 1` takes whatever `master` is that day.
**That was the one thing standing between "builds" and "reproducible", and it is
now done** (v0.0.23): five clone sites across the two workflows fetch one pinned
commit held in each workflow's `env` block, tried for real before committing. It
first appeared to fail with `Filename too long` — that was this machine's
150-character scratchpad path, not the technique.

## 4. Accessibility

**The brand pages, measured** — the plan says these are done and they are:

| | `prefers-reduced-motion` | `aria-*` | `role=` |
|---|---|---|---|
| `grimoire.html` | 2 | 7 | 1 |
| `scanner.html` | 2 | 2 | 1 |

`lang` is absent from both source files and present in every *assembled* page —
`grimoire.rs` emits `<html lang="en">`, so the served document is correct and the
sources are fragments.

**The window: keyboard-reachable, contrast unaudited.** Every button goes through
one helper that passes `WS_TABSTOP`, so tab reaches all of them; five accelerator
entries give `Ctrl+1`..`Ctrl+5` for the rail plus the menu's own. What nobody has
checked is **contrast ratios** in either theme, and there is no screen-reader
story at all for a window built from owner-draw controls — `WM_DRAWITEM` paints
text that no accessibility API is told about. That is a real limitation of the
approach rather than an oversight, and it is worth stating before someone asks.

**A correction to my own first pass**: I counted six `WS_TABSTOP` occurrences and
nearly reported the window as barely keyboard-reachable. Six is the number of
*literal sites*; one of them is inside `button()`, which is called for every
button in the app. Counting occurrences of a constant is not counting the controls
that get it.

## 5. Versioning and upgrade — what happens to someone on an old version

**The updater is verified across adjacent versions**: v0.0.19 → v0.0.20 → v0.0.21
unattended, through the app's own updater, with `~/.chaos/models` byte-identical
at 18 files and 120,933,521,300 bytes (STATUS has the record).

**What is untested is a long jump** — 0.0.2 to current — and that is where the
risk lives, because the settings file has gained keys the whole way. The format
handles it correctly by design, and this is tested: **unknown keys are preserved
on write**, so running an old build once does not destroy a new build's
preferences, and a missing key reads as its default. Today's `mode_chosen` addition
is exactly that case — absent from every existing file, so every upgrader is asked
the mode once more and then remembered.

So the mechanism is sound and the specific jump is unmeasured. One old release and
one virtual machine would settle it.

## 6. Model weight licensing

Already correct, in `NOTICE`: no weights are distributed, they carry their own
licences, *"which Chaos neither grants nor alters"*, and the named models are the
property of their publishers. `chaos-pull` fetches from a host the user names.
Nothing to change.

## 7. Error messages a person can act on

**Mixed, with both ends measured in §4e.** The container errors are exemplary —
they name the byte counts and the expected magic:

```
truncated while reading magic: needed 4 bytes, 0 left
not a GGUF file (magic was 0xe5024024, expected 0x46554747)
```

The port collision is the raw OS string with no advice, and it arrives *after* the
whole model has loaded. `chaos scan`'s refusal and `chaos start`'s dead-node report
are at the good end. **The rule is applied unevenly rather than absent**, and §4e
names the two specific fixes.

## What §4g changes

- **"No telemetry" is now stated where a reader looks** — it was true and
  unfindable, in the one project where it is the headline property.
- **The node's exposure is documented with its exact payloads**, and the decision
  to leave `/status` open is written down with what would reverse it.
- **The SBOM question is answered by a fact**: 22 packages, all first-party.
- ~~Two decisions left with Atur~~ — **both taken and shipped in v0.0.23**: the
  `/status` gate (on the peer, not the bind address) and the pinned llama.cpp
  commit.
- **Two limitations stated rather than discovered**: no contrast audit and no
  screen-reader story for the window; and no tested upgrade path from a distant
  version.
