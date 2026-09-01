# Contributing to Chaos

Thanks for looking. Chaos is a small project with an unusual amount of written
reasoning behind it, and the fastest way to be useful is to read a little of it
before writing code.

## The one rule that matters most

> **A performance claim is not citable until the competitor's exact command line
> and its output are in a document.**

This exists because the project broke it. "llama.cpp cannot run a model larger
than RAM" survived for days on a misattributed error string, because nobody ran
the opposing command. When someone finally did, llama.cpp ran the 144 GB model
fine with `--no-repack`, and the project's central marketing claim evaporated.

So: run it, paste it, and flag it. A benchmark without the losing command line is
an opinion.

The same applies to your own changes. "This should be faster" is not a result.
The project's record on reasoning ahead of measurement is **nought for four**:

| predicted | measured |
|---|---|
| Expert reads are latency-bound; parallelise them | 14% *slower*, reverted |
| Contextual sparsity is worth 5-10x | V4-Flash experts are 9.1% sparse, not 80% |
| Residency is "likely a large multiple" | 22% |
| Threadpool churn explains the flat thread curve | 1 thread is 4.7x *slower*, not faster |

## Before you start

1. Read [`docs/graph/INDEX.md`](docs/graph/INDEX.md), then **only the two or three
   nodes your task links to**. The graph is large; reading it all is a waste.
2. Read [`CLAUDE.md`](CLAUDE.md). It is the project's short list of facts that
   cost real time to rediscover — things like *ggml aborts the process when its
   arena is exhausted*, and *a wrong tokenizer produces fluent nonsense, never a
   crash*. Each line there is a summary; the full text of all 38 is
   [`docs/graph/reference/hard-won-facts.md`](docs/graph/reference/hard-won-facts.md),
   and **that one is worth reading in full before you propose an optimisation**,
   because about half of its entries are the measurement that killed one.

## Building and testing

```bash
export GGML_LIB_DIR=/path/to/llama.cpp/build/ggml/src
cargo build --release
cargo test --release              # 991 tests
cargo test --release -- --ignored # 42 more, need a real model on disk
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

**A GPU test with no GPU reports as a pass.** Cargo has no third verdict, and a
green "6 passed" was once reported for a file whose two GPU tests never ran. Run

```bash
CHAOS_REQUIRE_GPU=1 cargo test --release -p chaos-ggml
```

to turn every such skip into a failure. With `GGML_LIB_DIR` pointed at a
Vulkan-enabled ggml build, all 14 of them run: 6 in `device_arithmetic`, 6 in
`scheduler`, plus 2 that must be run alone (their own note says why).

The `--ignored` tests read from real GGUF containers and are skipped silently
when the file is absent, so they are safe to run without the model. If you have
one, run them: they are what verify the forward pass against llama.cpp's own
element sums, and they catch the class of bug that produces plausible output.

## Changing what the model computes

**Nothing that changes the arithmetic ships without `scripts/quality-gate.sh`.** It
runs fifty prompts with checkable answers against a recorded baseline, and the bar
depends on the kind of change — Atur's call, 2026-08-31:

- **exact** — the answer must not change at all: **100% byte-identical.** A change
  that only stops recomputing something, or reorders work that cannot reorder a
  sum, goes here.
- **lossy** — the arithmetic is known to change: **>= 95% identical, no checkable
  answer that was right may become wrong, and perplexity within +1%.**

```bash
bash scripts/quality-gate.sh --model M.gguf --record        # before the change
bash scripts/quality-gate.sh --model M.gguf --lever exact   # after it
```

**Validate the gate in both directions before trusting it.** It reported 0 of 50
byte-identical for a change whose output was in fact identical, because it kept the
non-deterministic `generate ... tok/s` line in every recorded answer — so it could
not have passed a comparison of a build against **itself**. It had only ever been
checked against a deliberately corrupted container, where a failure looks like a
failure whichever way it is caused. Both directions are cheap on Qwen2-0.5B:

```bash
bash scripts/quality-gate.sh --model qwen2.gguf --record
bash scripts/quality-gate.sh --model qwen2.gguf --lever exact   # must be 100%
```

**And do not re-record a baseline to make a comparison pass.** The one legitimate
reason to re-record is that the recorded strings were never the model's answers —
say so explicitly when you do.

## What good looks like here

**Correctness is checked, not assumed.** In this domain a wrong forward pass, a
wrong tokenizer, a wrong mask or a wrong cache all produce *fluent, plausible
text* rather than an error. Every numerical change needs a test that would fail
if the numbers were wrong — ideally against an external oracle, which for us is
`llama-eval-callback`'s element sums.

**Comments explain why, not what.** The codebase is written for someone reading
it in a year with no memory of the session that produced it. If something looks
strange, the comment should say what it prevents. If a value was chosen by
measurement, say what was measured.

**Negative results are kept.** If you try something and it does not work, a
commit or a doc node recording *what you measured and why it failed* is more
valuable than silence. Several of this project's best decisions came from
written-down failures.

## Pull requests

- Branch from `main` as `ticket/<short-name>`.
- One concern per PR.
- Say in the description what you measured, on what hardware, and how.
- CI must be green: build and unit tests on Linux, macOS and Windows.

Note that **Linux and macOS build and pass the unit tests in CI, but no model
has been run on either.** Direct I/O is `O_DIRECT` on Linux and
`FILE_FLAG_NO_BUFFERING` on Windows; **macOS has no direct-I/O path at all** and
falls back to buffered reads, because `F_NOCACHE` must be set after opening via
`fcntl` and that is not written yet. If you run Chaos on Linux or macOS — even
if it just fails — that is a genuinely useful bug report.

## Good first issues

- Run it on Linux or macOS and report what happens.
- `chaos-probe` on unusual hardware: report what it gets wrong.
- Documentation: if something in the README did not work, that is a bug.
- Architectures: adding one is well-bounded work with a clear oracle to check
  against. `core/arch/src/qwen3.rs` is the smaller example to copy.

## Reporting a bug

Include: OS, CPU, RAM, the model and quant, the exact command, and the full
output. If it is a *wrong output* bug rather than a crash, say what you expected
and what you got — those are the hardest and most valuable ones.

## Code of conduct

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

Contributions are accepted under the Apache License 2.0, matching the project.
