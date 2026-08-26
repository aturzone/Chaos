---
topic: why the Android app runs the engine as a child process instead of loading it
status: resolved
links:
  - android-engine-runs-2026-08-24.md
  - ../backlog/android-app.md
---

# A Rust cdylib broke bionic's threads, twice, in opposite directions

**The engine was loaded into the app and called through JNI for two releases.**
It worked for everything that did not touch a thread. It broke bionic's
per-thread bookkeeping the moment anything did, and it broke it in two
different places:

```text
creating a thread FROM the library
  Java_..._startServer -> chaos_serve::serve -> StreamingRunner::new
    -> std::thread -> pthread_create -> __init_tcb
       SIGSEGV, code 2 (SEGV_ACCERR)

a thread that had CALLED the library, exiting
  __start_thread -> __pthread_start -> pthread_exit
    -> pthread_key_clean_all
    -> libcrypto thread_local_destructor -> OPENSSL_free
       SIGSEGV, code 1 (SEGV_MAPERR)
```

The second one is the more informative: **the faulting library is Android's own
`libcrypto`, which this project never calls.** A pthread key destructor running
on thread teardown found a pointer it could not free. That is the signature of
something having corrupted the thread's own bookkeeping earlier.

## What was ruled out, so nobody repeats it

| tried | result |
|---|---|
| `stack_size(16 MiB)` on the spawned thread | no change |
| spawning from a **JVM** thread instead, so Rust never calls `pthread_create` | no change — the crash moved deeper, into `StreamingRunner::new`, which spawns its own |
| linking with the NDK's `.cmd` wrapper rather than raw `clang.exe` | the wrapper only adds `--target=`, which was already passed |
| a `PT_TLS` segment too large for bionic's dlopen surplus | **the library has no `PT_TLS` segment at all** (`llvm-readelf -lW`) |
| removing an unsound `std::env::set_var` | **a real bug, fixed** — writing the environment in a threaded process can free a string another thread holds — but not this one |

## What the same code does as an executable

Nothing. `chaos-run` was pushed to the same emulator in v0.0.19 and creates
threads perfectly; `libchaos_serve.so` is an executable shipped in `jniLibs`
and has been running a 1B model on the phone since. **The difference is being
`dlopen`'d into an app process, not the code.**

## The decision

**Android runs the engine as a child process, exactly as the desktop window
does.** `ProcessBuilder` on `nativeLibraryDir/libchaos_serve.so`, with
`android:extractNativeLibs="true"` so the file is really on disk and can be
executed. One architecture across both platforms, one protocol, and the part
that was fighting is gone.

The JNI bridge was then deleted rather than kept for the three small things it
answered, because each is available without native code:

| the bridge answered | Kotlin answers |
|---|---|
| the engine's version | `BuildConfig.VERSION_NAME` |
| what this device is | `Phone.describe`, which asks Android |
| the models present | `File.listFiles()` |

**Keeping a library that corrupts a thread's teardown in order to avoid
`File.listFiles()` would be a bad trade**, and the crash it caused was
unattributable from the log: the app died in `libcrypto`, on a thread that had
already returned from our code, minutes after the call that broke it.

## What this cannot say

- **Why** it happens. The mechanism is not identified — only that it is
  reproducible, that it is specific to the shared-library form, and that the
  five obvious explanations above are not it.
- **Whether a smaller cdylib is safe.** The bridge that only returned strings
  worked for two releases. It may have been corrupting something the whole
  time and nobody made a thread that exited.
- **Anything about a real handset.** All of this is an Android 34 x86_64
  emulator.
