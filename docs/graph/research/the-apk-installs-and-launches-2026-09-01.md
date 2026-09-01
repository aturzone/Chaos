---
topic: the published APK installs, launches and renders its real UI on an emulator — and crashes on entering a mode in translated code that cannot be attributed to Chaos. What a real arm64 device would settle.
status: partly resolved; one question handed to a device
links:
  - android-engine-runs-2026-08-24.md
  - ../backlog/android-app.md
  - ../backlog/lts-parity-criteria.md
---

# The APK installs and launches

`CLAUDE.md` says the Android tier "**Never run on a phone**" and that the SDK
cannot be installed here because `dl.google.com` 404s this network. The second
half is out of date: **there is a complete Android SDK on this machine** at
`C:\Android\sdk`, installed for an unrelated project — `adb 1.0.41`, emulator
`37.0.0`, an `android-34 google_apis x86_64` system image, build-tools 34.0.0,
and an AVD. No NDK.

So the Android tier is testable here, and this is what testing it found.

## What was tested, and with what

The **published** APK, not a local build — `com.aturzone.chaos`, versionCode 21,
versionName **0.0.21**, `native-code: 'arm64-v8a'`, carrying
`lib/arm64-v8a/libchaos_android.so` (3.1 MB) and `libchaos_serve.so` (3.2 MB).

**It is v0.0.21 rather than the current v0.0.23, and that is not a choice.** The
published assets cannot be downloaded on this network: every route ends at
`release-assets.githubusercontent.com`, which resolves, accepts a TCP connection
in 0.13 s and then **resets during TLS**, while `api.github.com` answers 200 in
0.34 s and `github.com` 200. `objects.githubusercontent.com` will not connect at
all. This APK was on disk from an earlier session.

## The emulator's ABI list is the thing that made this possible

```text
ro.product.cpu.abilist = x86_64,arm64-v8a
ro.product.cpu.abi     = x86_64
ro.build.version.sdk   = 34
```

An arm64-only APK on an x86_64 image looks impossible until you read that
`abilist`. API 34's `google_apis` x86_64 image ships **ARM translation**, and the
install went through as `primaryCpuAbi=arm64-v8a` with `ndk_translation` mapped
into the process. The first conclusion drawn here was "arm64-only, so the
emulator cannot run it", and the `abilist` overturned it.

## Installs, launches, renders

```text
adb install Chaos-v0.0.21-android-arm64.apk        Success
adb shell am start -n com.aturzone.chaos/.ModeActivity
ActivityTaskManager: Displayed com.aturzone.chaos/.ModeActivity  +3s71ms
```

And the screenshot shows the real interface, not a blank window: **"WHAT IS THIS
DEVICE?"**, *"Turn the dial. Everything else follows from this."*, the mode dial
with **CLIENT** and **HELPER**, the caption *"CLIENT — Uses a CORE elsewhere.
Loads nothing here."*, and an **ENTER** button. That is the launch flow
`research/desktop-app-broken-2026-08-28` describes, on Android, from a published
artefact.

A **"System UI isn't responding"** dialog appeared over it. That is
`com.android.systemui`, not Chaos, and Chaos's own activity had already displayed
underneath — routine for a translated app on a memory-constrained emulator, and
recorded here so it is not later mistaken for an app fault.

## Then it crashed, and the crash cannot be pinned on Chaos

Tapping through to CLIENT mode:

```text
F libc  : Fatal signal 11 (SIGSEGV), code 1 (SEGV_MAPERR), fault addr 0x0
          in tid 5173 (.aturzone.chaos)
F DEBUG : ABI: 'x86_64'
F DEBUG : backtrace:
F DEBUG :       #00 pc 00000000001e2f63  <anonymous:73c90ab6b000>
```

**One frame, anonymous, no library, no symbol.** That is code the translator
generated at run time, which is where a translated process spends its time. Two
further facts matter:

- **`libchaos_android.so` was never mapped.** Checked with `adb root` against
  `/proc/<pid>/maps` both before and after: the only `chaos` mapping is
  `base.apk` itself. So the engine had not loaded when it died — consistent with
  CLIENT mode's own caption, *"Loads nothing here."*
- **`ndk_translation` was mapped.** The process was arm64 code being translated
  to x86_64 the whole time.

So the honest position: **an emulator running ARM translation is the wrong
instrument for this question.** The crash may be a Chaos bug that only appears
under translation, or a translator bug. Nothing here distinguishes them, and
attributing it to the app would be exactly the sort of confident wrong claim this
repository keeps retracting.

**Not filed as a bug.** Filed as a question with a named experiment.

## What would settle it, in one step

**Enter CLIENT mode on a real arm64 device.** If it crashes there, it is ours and
the backtrace will name a library. If it does not, this was the translator and
the finding is only that the emulator cannot carry the mode transition.

Atur has an Android device and has offered to test; this is the specific thing to
do, and it takes a minute:

```text
install the APK, open it, turn the dial to CLIENT, press ENTER
```

The second, slower option is to build an **x86_64 APK** so the emulator runs
native code. That needs an NDK, which is not on this machine — although the
package already installed here before this session had `primaryCpuAbi=x86_64`, so
some earlier session had one.

## A release-engineering point worth taking either way

`release.yml` builds `-DANDROID_ABI=arm64-v8a` and nothing else. Adding `x86_64`
would cost a few megabytes and buy something the project does not have: **an APK
that can be smoke-tested on an emulator, including in CI**, without a physical
device and without depending on a translation layer to be faithful. Every
GitHub-hosted runner is x86_64.

## What this closes

- **"The APK has never been installed or launched from a published artefact"** —
  closed, with the screenshot and the `Displayed … +3s71ms` line.
- **"Never run on a phone"** — still true. An emulator is not a phone, and this
  node does not pretend otherwise.
- **The engine on Android** — still unverified. It was not loaded at the screen
  that was reached.
