---
topic: the documented Linux build was impossible — cmake names static archives `libggml-base.a` and the build scripts only knew `ggml-base.a`. Fixed, and a model ran on Linux for the first time.
status: resolved
links:
  - ../backlog/lts-parity-criteria.md
  - ../reference/hard-won-facts.md
---

# Linux could not build from the README

`CONTRIBUTING.md` says no model has ever been run on Linux, and
`lts-parity-criteria` counts it as a v0.0.29 item. It turns out that was not for
want of trying: **following the README on Linux could not work.**

## The bug

cmake names static archives differently per platform. MinGW on Windows emits
`ggml-base.a`; GCC on Linux emits **`libggml-base.a`**. Both
`core/arch/build.rs` and `core/ggml/build.rs` looked only for `{name}.a`:

```rust
.filter(|name| !dir.join(format!("{name}.a")).exists())
```

So a Debian container that had just built ggml successfully —

```text
/build/ggml/ggml/src/libggml-base.a
/build/ggml/ggml/src/libggml-cpu.a
/build/ggml/ggml/src/libggml.a
```

— was told:

```text
GGML_LIB_DIR is /build/ggml/ggml/src, but it does not contain:
  ggml-base.a, ggml-cpu.a, ggml.a
```

**naming three files that the instructions in that very message cannot
produce.** `core/ggml/build.rs` then compounds it: it copies `{name}.a` to
`lib{name}.a` when staging, on the assumption that the source never has the
prefix — so even past the check, the copy would look for the wrong file.

## Why CI never caught it

CI builds ggml itself and **stages the archives, stripping the prefix**, before
setting `GGML_LIB_DIR`:

```bash
find llama.cpp/build \( -name 'libggml*.a' -o -name 'ggml*.a' \) -print | while read -r f; do
  cp "$f" "ggml-libs/$(basename "$f" | sed 's/^lib//')"
done
```

That `sed 's/^lib//'` is the workaround, and it is exactly what kept the bug
invisible: **CI's Linux job has never exercised the path a user takes.** The
green build and the broken instructions coexisted because the two do different
things.

This is the same shape as the `--ignored` gap found the same day — CI running a
variant of the real thing and reporting on the variant.

## The fix

One helper in each build script:

```rust
fn archive(dir: &Path, name: &str) -> Option<PathBuf> {
    let plain = dir.join(format!("{name}.a"));
    if plain.exists() { return Some(plain); }
    let prefixed = dir.join(format!("lib{name}.a"));
    prefixed.exists().then_some(prefixed)
}
```

Either spelling is accepted; staging still writes `lib{name}.a`, which is what
the GNU linker wants. The failure message now names both. Windows still builds —
checked before and after.

## And then a model ran on Linux

Debian 12 (bookworm), GCC, Rust 1.98.0, in a container with 20 cores and
7.6 GiB of RAM. `chaos-probe` works: it reads `/proc/meminfo`, reports
`os linux (x86_64)`, and detects no GPU.

**Qwen2-0.5B, on Linux:**

```text
The capital of France is Paris. It is the most populous city in France and the most populous city in
generated  16 tokens in 0.3s (50.01 tok/s)
threads    generation used 12
```

**DeepSeek-V4-Flash, 144 GB, on Linux:**

```text
resident   loaded 108 tensors, 3.90 GiB of 3.90 GiB budget in 28.3s
           3.48 GiB did not fit and will be re-read every token
           ~11.9s of each, at a measured 0.29 GiB/s on these tensors
           nothing large is closeable; this model needs more RAM than this machine has
generate   3 tokens in 68.9s (0.044 tok/s, 23.0s per token)
```

**It runs.** 0.044 tok/s is not a Linux number, it is a 7.6 GiB number: the
container has less than half the trunk's 7.38 GiB, so most of the always-read set
is re-read from disk on every token — and the engine says so precisely, including
that closing things would not help. That is the residency report doing its job on
a platform it had never run on.

## And the whole suite passes on Linux

Not only a model. The same container, with `clippy` and `rustfmt` added to the
toolchain:

```text
cargo test --release --workspace     991 passed, 0 failed, 42 ignored
cargo clippy --workspace --all-targets -- -D warnings     0 errors
cargo fmt --all --check                                   clean
```

**Identical to Windows**, test for test. That is a stronger statement than "it
builds": the counts match, so nothing is being skipped on one platform and run on
the other.

## The one number worth a second look

**Qwen2-0.5B measured 50.01 tok/s on Linux against 32.00 on Windows** in the same
day's five-model table. That is 1.56x, and it is **not a controlled comparison**:
the thread tuner chose **12** generation threads on Linux and **2** on Windows, on
the same 20-core machine. Two different configurations, so the gap could be the
platform, the tuner, or both.

It is recorded here as an observation and not a claim. Settling it means running
the same thread count on both, which is a measurement nobody has taken — and it
sits beside the other tuner finding from the same day, that it leaves about 4% on
the table at two very different context lengths.

## What this closes, and what it does not

- **v0.0.29: "run a model on Linux"** — done, twice, with the output above, and
  the full suite, clippy and fmt pass there with the same counts as Windows.
- **The Linux tarball, `.deb` and AppImage are still unexecuted.** They cannot be
  downloaded on this network: `release-assets.githubusercontent.com` resolves and
  accepts a TCP connection in 0.13 s and then **resets during TLS**, while
  `api.github.com` answers 200 in 0.34 s. So the assets themselves remain
  untested, and what is verified is the source they are built from.
- **macOS remains untested**, and nothing on this machine can change that.
