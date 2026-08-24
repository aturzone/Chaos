# The engine builds and runs on Android, and aborts at exit (2026-08-24)

**"Phase B is blocked" was wrong.** `android-app.md` said running models on the
phone was blocked because `dl.google.com` 404s this network. The Google host is
unreachable; **the NDK is on the same Tencent mirror the SDK came from**, which
was never tested. The conclusion had been drawn from one host without trying the
mirror already in use for everything else.

```
android-ndk-r26d-windows.zip   665,022,840 bytes
sha1  c7ea35ffe916082876611da1a6d5618d15430c29   <- matches Google's manifest
```

The manifest is itself mirrored, so this proves the transfer, not the
provenance — the same standing caveat as the SDK, and the same policy: **local
toolchain for building, CI ships from Google's own repositories.**

## What it took

Less than expected. **Every crate already type-checked for
`aarch64-linux-android` with no source change** — `chaos-probe`, `chaos-gguf`,
`chaos-tokenizer`, `chaos-model`, `chaos-arch`, `chaos-ggml`, `chaos-grammar`,
`chaos-jinja`, `chaos-plan`, `chaos-io`. `core/probe` was the crate the backlog
expected to need work and it needed none: its unix branch reads `/proc/meminfo`,
which Android has.

All the work was in `core/ggml/build.rs`, and every item was a link flag that is
right elsewhere and wrong here:

| symptom | cause |
|---|---|
| `unable to find library -lgomp` | the NDK has no libgomp; its OpenMP is libomp, and ggml is built with OpenMP off for this target |
| `unable to find library -lpthread` | bionic has no separate libpthread — pthreads are inside libc |
| `unable to find library -lstdc++` | the NDK ships LLVM's libc++ |
| a page of `undefined symbol: operator new`, `__cxa_guard_acquire`, `std::__ndk1::mutex::lock()` | naming nothing was also wrong: rustc passes `-nodefaultlibs`, so the clang driver links no C++ runtime implicitly. `libc++_static.a` has to be named **and** its directory searched |
| `could not find native static library c++_static` | the archive lives in the NDK sysroot, which rustc does not search |

**Two mistakes inside the fix, both worth keeping.**

- The directory is asked of the compiler (`clang -print-file-name=`) rather than
  hard-coded, the way the GNU runtime directory already was. But
  **`Command::new` cannot execute a `.cmd`** and the NDK's Windows compiler is
  exactly that — so the call failed, the helper returned `None`, nothing was
  linked, and the failure looked like ggml missing `operator new`. Batch
  wrappers go through `cmd /c` now.
- The helper then read `CC_aarch64_linux_android` **by name**, so it silently
  returned `None` for `x86_64-linux-android` — the emulator's ABI — and the same
  page of undefined symbols came back on a target that had just been proven to
  work. It derives the variable from `TARGET` now.

## It runs

`x86_64-linux-android`, on an Android 34 emulator:

```
$ adb shell /data/local/tmp/chaos-run --version
chaos-run 0.0.18

$ adb shell /data/local/tmp/chaos-probe --quick
os         android (x86_64)
cpu        4 threads
ram        2.4 GiB total, 1.6 GiB available   [/proc/meminfo]
disk       0.0 GiB free of 0.8 GiB   (.)
gpu        none detected
```

**`core/probe` reads a phone correctly with no changes at all.** That is the
crate the plan expected to be the obstacle.

## Two bugs, and running it is the only way either would have appeared

### 1. Every binary aborts at exit — SIGABRT, exit code 134

```
chaos-run 0.0.18
FORTIFY: pthread_mutex_destroy called on a destroyed mutex (0x59643766e5b0)
```

The work completes and the output is correct; the process then aborts. It
happens on `--version` and `--help`, and on `chaos-probe` too, so it is **not
specific to the runner or to a model being loaded**.

**Android's libc is stricter than the ones this has run on.** bionic's FORTIFY
checks that a mutex is not destroyed twice; glibc and Windows tolerate it
silently, so the same double-destroy has presumably been happening everywhere
and nothing said so. Not diagnosed further yet: what is known is that it is at
teardown, it is reproducible, and it would abort on a real phone.

### 2. The models directory resolves to `/.chaos/models`

```
no models found. Put a .gguf file in:
  /.chaos/models
```

`$HOME` is not set in an Android shell, so the home-relative path collapses to
the filesystem root. On a phone the app's own files directory is the right
answer, and it has to be passed in rather than derived from the environment.

## What is still not done

Both of those, and then the part that makes it a feature rather than a
milestone: a JNI entry point the Kotlin side can call, model files on the
device, and a way to choose a model that fits **this** phone — Atur's "a
powerful phone or a simple phone" — which `chaos-probe` can now answer, because
it reads the device correctly.
