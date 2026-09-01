<h1 align="center">Chaos</h1>

<p align="center">
  <img src="assets/logo.png" alt="Chaos" width="180">
</p>

<p align="center">
  <strong>Run models that do not fit in your RAM.</strong><br>
  The always-read weights stay resident; the routed experts stream from disk, per token.
</p>

<p align="center">
  <a href="https://github.com/aturzone/Chaos/releases"><img alt="version" src="https://img.shields.io/github/v/release/aturzone/Chaos?color=orange&label=version"></a>
  <a href="LICENSE"><img alt="licence" src="https://img.shields.io/badge/licence-Apache--2.0-blue"></a>
  <a href="https://github.com/aturzone/Chaos/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/aturzone/Chaos/actions/workflows/ci.yml/badge.svg"></a>
  <img alt="tests" src="https://img.shields.io/badge/tests-991%20passing-brightgreen">
</p>

> **This file carries three things and nothing else**: the progress bars, the
> document map, and tok/s for five fixed models with the machine that produced
> them named. Everything else lives one click away. `scripts/check-readme.sh`
> enforces it, and CI runs that.

---

## Install

One file per platform from [Releases](https://github.com/aturzone/Chaos/releases).

| you have | download |
|---|---|
| **Windows** | `Chaos-vX.Y.Z-windows-x86_64-Setup.exe` — double-click, per-user, no admin rights |
| **Debian / Ubuntu** | `chaos_X.Y.Z_amd64.deb` — `sudo apt install ./chaos_*.deb` |
| **any Linux** | `Chaos-vX.Y.Z-linux-x86_64.AppImage`, or `-linux-arm64.tar.gz` on a Pi |
| **macOS** | `Chaos-vX.Y.Z-macos-arm64.tar.gz` or `-macos-x86_64.tar.gz` |
| **Android** | `Chaos-vX.Y.Z.apk` — a client for a node on your network |

From an archive, copy the binaries yourself:

```bash
sudo install -m 755 Chaos-*/chaos-* /usr/local/bin/ && mkdir -p ~/.chaos/models
```

**No telemetry and no dependencies.** `Cargo.lock` holds 22 packages and all 22 are
crates in this repository, so there is nothing third-party to audit. The one request
Chaos makes on its own is the window's update check against a static JSON file, which
`CHAOS_NO_UPDATE_CHECK` turns off; `chaos update` asks on demand instead.

**The window is Windows-only.** Every platform gets the command-line tools, and
`chaos-serve` speaks the OpenAI API, so any client works everywhere.

## Run

```bash
chaos-pull --list                      # the catalogue, with the size that actually matters
chaos-pull qwen3-4b                    # ~2.5 GB, a good first one
chaos-run                              # lists the models you have
chaos-run qwen3 "The capital of France is" -n 16
chaos-run qwen3 "write a haiku" --auto # reads your machine and configures itself
chaos-probe                            # what can this machine run, and what should you close?
chaos-serve qwen3 --port 8080          # OpenAI API plus a chat page, both compiled in
chaos-draw "a red apple on a white table" -o apple.png
```

You never type a path, and for a model split across five shards you never work out which
shard to open. **Read the second column of `chaos-pull --list`, not the first**: a 155 GB
Mixture-of-Experts model streams on a 16 GB machine; a 20 GB dense one does not, because
a dense container has no routed experts to leave on disk.

**A model whose architecture has never been diffed against llama.cpp is refused by
name** — a wrong forward pass produces fluent nonsense rather than an error, so the
default is refusal and `--force` is the override.

## Speed

**Five fixed models, measured 2026-09-01 in one session, on a machine with nothing
else running.** The set is fixed in `scripts/speed-five.tsv` so this month's table can
be compared with last month's; regenerate the numbers with `bash scripts/speed-five.sh`
and never by hand.

Machine: **i7-13650HX, 14 cores, 15.7 GiB RAM**, SK Hynix NVMe (3.41 GiB/s at queue
depth 8), **CPU only**, nothing else running.

```
model                  container   resident    tok/s   what the number is
DeepSeek-V4-Flash      144.44 GiB   7.38 GiB    0.728   144 GB generating in 15.7 GiB of RAM
Qwen3-30B-A3B           17.28 GiB  17.28 GiB     4.41   a smaller MoE; --force, its diff has 1 FAIL
Qwen3-4B                 2.33 GiB   2.33 GiB     8.27   dense, fits with room to spare
Falcon3-1B               0.98 GiB   0.98 GiB    22.31   dense, small
Qwen2-0.5B               0.37 GiB   0.37 GiB    32.00   the ceiling: nothing to stream, nothing to wait for
```

Three runs each, median reported; four agreed within 6%, and V4-Flash's 25% spread is a
cold page cache on the first run after a build — at a fixed configuration it holds to 1%.

**Your machine is not this machine, and Chaos will say so before you download
anything.** `chaos-model-info <model> --budget <GiB>` predicts tok/s from the resident
set *your* machine would carry; the law behind it, across nine models spanning 23x in
size, is `tok/s ~= 19 / resident GiB`, with `chaos-membench` for the constant.

**Against llama.cpp:** 1.20–1.27x behind on the dense path hand-tuned, 1.23x ahead out
of the box because Chaos measures your machine and llama.cpp uses a fixed default. Both
engines alternating in one session, command lines recorded:
`docs/graph/research/where-we-stand-vs-llamacpp-2026-08-16.md`.

**On the 144 GB streaming model no comparison is published, deliberately**: llama.cpp
ranged **0.16–0.47 tok/s over eight runs of one command line** here while Chaos held to
1%, so best-of would claim 1.70x and worst-of 4.35x and no ratio is honest —
`docs/graph/research/the-v4flash-parity-cell-does-not-reproduce-2026-09-01.md`.

If you want the fastest local inference today, use
[llama.cpp](https://github.com/ggml-org/llama.cpp). Chaos is worth your time if you want
an engine that owns residency and tells you the truth about your machine.

## Progress

**The road to v0.0.30**, the release built to LTS standard. Nothing is tagged until its
gate is green — 23 releases went out in 21 days once, and none of them got a
stabilisation period.

```
  v0.0.24  One truth                     100%  [####################]  merged
  v0.0.25  Guard the binary              100%  [####################]  merged
  v0.0.26  Measure before optimising     100%  [####################]  merged
  v0.0.27  Quality harness, then levers    85%  [#################...]  two levers through
  v0.0.28  Any machine, any model          80%  [################....]  needs other machines
  v0.0.29  Every platform, actually run    55%  [###########.........]  Linux done, macOS not
  v0.0.30  LTS                              8%  [#...................]  2 of 18 parity cells met
```

**Coverage against llama.cpp.** Every bar is a ratio of two counted things, both named;
filled cells are floored, never rounded up. The first four are enforced by tests rather
than counted by hand, which is why they have stopped drifting.

```
CLI flags         91%  [##################..]  165 of llama.cpp's 182
Chat templates    96%  [###################.]  52 of its 54 names
Tokenizers        83%  [################....]  5 of 6 families
Samplers          80%  [################....]  16 of 20
Architectures     10%  [##..................]  14 of the 141 it declares
GPU backends      20%  [####................]  1 of 5, Vulkan only
Browser UI        33%  [######..............]  2 of 6 things a chat UI needs
V4-Flash speed    14%  [##..................]  0.728 of 5 tok/s
```

"Verified" here means diffed, and **every one of the 14 architectures was diffed against
llama.cpp at 8 prompts.** The Vulkan device path is bound but **not** verified: it fails
1 of those 8 where the CPU path fails none.

## Document map

| | |
|---|---|
| `STATUS.md` | the scoreboard. Read it before quoting any number |
| `CHECKLIST.md` | the tick-list: what is done, what is not, and why |
| `CHANGELOG.md` | every release, including the retractions |
| `CONTRIBUTING.md` | how to build and test, and the citation rule |
| `SECURITY.md` | what a running node exposes, measured |
| `docs/graph/INDEX.md` | every working note, one line each — start here |
| `docs/graph/research/` | measurements. Many refuted the idea that motivated them |
| `docs/graph/backlog/` | what is planned, with a definition of done for each |
| `docs/graph/decisions/` | the choices that are settled, and what they superseded |
| `docs/graph/reference/hard-won-facts.md` | read before proposing any optimisation |
| `docs/graph/history/` | superseded, kept rather than deleted |

The code, grouped by what a crate is *for*: `core/` the engine — containers, ggml, I/O,
residency, architectures, images · `cli/` the front door and the runner · `network/` the
server and the worker · `gui/` the Windows window and the installer · `android/` the JNI
bridge · `scripts/` the checks CI runs.

## Licence

Apache-2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE). Chaos distributes no model
weights; models are yours to obtain, under their own licences.
