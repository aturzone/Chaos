# Support

What this project will fix, what it will keep stable, and what it does not
promise. Written for v0.0.30 — the release being built to LTS standard — and
kept deliberately narrower than a project this size is tempted to claim.

**Security problems do not belong here.** Report those privately:
[SECURITY.md](SECURITY.md).

## What "LTS" means here

Three things, and nothing more:

1. **The surface stops moving.** The CLI's flag names and their meanings, the
   HTTP endpoints and their request and response shapes, the settings file's
   location and format, and the on-disk layout of the model catalogue. A script
   or an editor integration written against an LTS release keeps working against
   every later patch of it.
2. **Correctness bugs get fixed on it**, not only on `main`.
3. **Every claim in the release notes was measured on hardware**, with the
   command line recorded. This project has retracted claims before; the ladder in
   [CHECKLIST.md](CHECKLIST.md) exists because of it.

**It does not mean a support window in years.** Chaos is written by one person.
Promising five years of patches would be the kind of claim this repository spends
its time removing, so the promise is structural instead: **the newest LTS
release is supported until the next LTS release exists**, and that is the whole
of it.

## What is supported

**Numbers live in [STATUS.md](STATUS.md), not here.** It is the canonical
scoreboard, it is machine-checked, and it is updated in the same commit as
anything that moves it — a second copy in this file would be a second copy to go
stale, which is exactly how `SECURITY.md` came to claim v0.0.0 through
twenty-three releases.

What that file records, in the shape it matters here:

- **Architectures** are supported only when they have been **diffed against
  llama.cpp token for token** on eight prompts. Anything else is refused by name
  and needs `--force`, which exists on the CLI and deliberately not on the
  server. A wrong forward pass produces fluent nonsense rather than a crash, so
  "it loaded and generated" is not evidence of support and is never treated as
  such.
- **Tokenizers**: one family is implemented but **not** verified against
  llama.cpp, because no container for it exists on the machine. STATUS.md names
  which. Treat it as unsupported until that changes.
- **Quant types**: whatever the linked ggml can decode. Chaos does not implement
  its own kernels for these and does not claim to.
- **Platforms** are supported when a model has actually been **run** there, not
  when the build is green. `docs/graph/research/` carries a node per platform
  with the output. Windows and Linux qualify; the rest is in the next section.

## What is not supported, said plainly

- **macOS is untested.** The binaries are built and published; nothing on the
  maintainer's hardware can run them. Reports are welcome and cannot be
  reproduced.
- **The `.deb`, the AppImage and the arm64 Linux tarball have never been
  executed by anybody.** What is verified is the source they are built from.
- **The Android app has never run on a phone.** The published APK installs,
  launches and draws its interface on an emulator, then crashes entering a mode
  inside translated code that cannot be attributed to Chaos. A real device
  settles it in one minute and nobody has done it.
- **iOS does not exist**, and is parked deliberately.
- **`chaos scan` is not built** and says so when you run it.
- **The embeddings endpoint answers 501.** The graph returns logits, and faking
  an embedding from them would look right and behave like noise.
- **The HTTP client speaks no TLS.** It is enough HTTP/1.1 to ask a node for
  status and stream a completion, over plain connections, on a network you
  control.
- **No GPU tier is verified.** Vulkan is bound and works; the device path fails
  one of eight parity prompts where the CPU path fails none, which is arithmetic
  rather than wiring. Until that is closed, treat `--device` as experimental.
- **Speed on your machine is not a promise.** The five numbers in the README
  were measured on one laptop with nothing else running, and the project's own
  measurements have swung 19% between identical runs on a busy machine.

## What gets fixed in a patch release

In priority order:

1. **Wrong output.** Arithmetic that disagrees with llama.cpp, a tokenizer that
   splits differently, a chat template that renders differently. This is the
   category the project cares about most, because it is the one a user cannot
   see.
2. **Data loss** — anything that damages a model file, a prompt cache, or a
   settings file.
3. **A crash, a hang, or an abort**, including the ones ggml causes by aborting
   the process rather than returning an error.
4. **Install, update and uninstall** breaking, on a platform listed as
   supported.
5. **A refusal that should have been an answer**: a container Chaos could open
   but declines, an error message that names the wrong cause.

**What does not get fixed in a patch:** performance regressions that are inside
the machine's own measurement noise, new architectures, new flags, new features.
Those go to `main` and ship in the next release.

## Reporting something

Open an issue at [github.com/aturzone/Chaos/issues](https://github.com/aturzone/Chaos/issues).
Two commands make almost any report actionable, and without them most reports
cannot be reproduced:

```
chaos probe --quick                 # the machine: RAM, cores, disk, GPU
gguf-info <your-model.gguf>         # the container: architecture, shapes, dtypes
```

Then the **exact command line** you ran and its **complete output**, including
the lines above the answer — `resident`, `cache`, `threads` and `repacked` say
more about a speed problem than the tok/s figure does.

For a wrong-output report, the most useful thing you can add is **llama.cpp's
answer to the same prompt with the same sampler**, because that is the standard
this project holds itself to.

## Where the reasoning lives

- [STATUS.md](STATUS.md) — the scoreboard: what works, what is measured, what is
  claimed.
- [CHECKLIST.md](CHECKLIST.md) — the release ladder, and every item's evidence.
- [CONTRIBUTING.md](CONTRIBUTING.md) — how to build it and what a change has to
  pass.
- `docs/graph/` — the reasoning, one node per question. Start at `INDEX.md`.
