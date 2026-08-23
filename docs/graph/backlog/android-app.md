# An Android app, shipped as `.apk`

**Atur's words**: *"i need android app for now it is really important … also i
need chaos based on phone options run a model for that suggestions"*, and the
`.apk` is to ship with every release alongside the other eight assets.

This node exists so the size of the job is written down before anyone promises a
date, and so the first decision — the one everything else depends on — is made
deliberately rather than discovered halfway.

## Nothing for it is installed

Checked on this machine, 2026-08-22:

| needed | present |
|---|---|
| JDK 17 | **no** — `java` is not on PATH |
| Android SDK + platform-tools | **no** — `%LOCALAPPDATA%\Android\Sdk` holds only `avd`, `dl`, `flows` |
| NDK r26+ | **no** |
| Gradle | **no** |
| `aarch64-linux-android` Rust target | **no** — only `x86_64-pc-windows-gnu` |

So the first commit on this is several gigabytes of toolchain, over a connection
that has already failed on GitHub's own asset host twice this week.

## The decision that comes first

**A phone cannot do what this project is for.** Chaos exists to run models that
do not fit in memory by streaming experts from an NVMe at 2.74 GiB/s. A phone
has neither that storage nor that bandwidth, and a 144 GB container is not going
onto it at all.

So the Android app is one of two products, and they share almost no code:

1. **A small-model runner.** 1B–4B, quantised, entirely resident. Real inference
   on the phone, honest about the ceiling. `chaos-model-info`'s prediction and
   the probe's memory reading port directly; the streaming residency policy is
   irrelevant and should not be carried over.
2. **A client for a Chaos on a PC.** The engine already speaks the OpenAI API on
   `127.0.0.1`; over a LAN this is a chat client and an image queue, and the
   phone does no inference at all. Far less work, and it makes the *big* models
   usable from the phone, which the first option never can.

**These are different apps.** Ask Atur which one he means before building
either; "an Android app" is satisfied by both and disappointed by the wrong one.

## What each needs

### Shared

- JDK, SDK, NDK, Gradle, and the two Rust Android targets.
- A signing key, kept out of the repository, and a release-workflow job that
  builds and signs the `.apk`.
- The icon rendered from `assets/logo.svg` at Android's densities (mdpi through
  xxxhdpi, plus the adaptive-icon foreground/background pair) — the same
  per-place-per-size rule the desktop icon now follows.

### 1. The small-model runner

- ggml cross-compiled for `aarch64-linux-android`, both CPU backends.
- `core/gguf`, `core/io`, `core/model`, `core/tokenizer`, `core/jinja`,
  `core/arch` compiled for Android. None of them use Win32; the blockers are
  `core/probe`'s platform module and any path assumptions.
- Storage-access-framework paths: Android will not hand an app a plain
  `~/.chaos/models`.
- A UI. The Win32 window does not port — `gui/app` is `extern "system"`
  declarations against user32 and gdi32. Android needs its own front end.

### 2. The client

- No NDK, no ggml, no Rust on the device at all — a Kotlin app against
  `/v1/chat/completions`.
- Needs `chaos-serve` to bind something other than `127.0.0.1`, which is a
  deliberate change with a security consequence: the API key stops being
  optional the moment the socket leaves the loopback.

## Model suggestions from the phone's own hardware

Atur asked for this specifically. `core/probe` already reads memory and cores;
`chaos-model-info` already predicts tok/s at a stated footprint. The Android
version of R6 is the same idea with a different ceiling: read the phone's RAM,
name the two or three models that will actually run, and **say the expected
tok/s before the download starts** — on a phone that download is the user's
data allowance, so the prediction matters more, not less.

## Definition of done

- `Chaos-vX-android-arm64.apk` attached to the release, built by CI.
- It installs on a real phone, opens, and does the thing option 1 or 2 promises.
- The release notes say which of the two it is, and what it will not do.
