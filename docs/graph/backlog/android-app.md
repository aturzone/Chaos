# An Android app, shipped as `.apk`

**Atur's words**: *"i need android app for now it is really important … also i
need chaos based on phone options run a model for that suggestions"*, and the
`.apk` is to ship with every release alongside the other eight assets.

**Phase A is written and builds in CI.** `android/` holds it. What follows is
the audit that decided its shape, and the measurement that decided where it is
built.

## The decision, and which way it went

**A phone cannot do what this project is for.** Chaos exists to run models that
do not fit in memory by streaming experts from an NVMe at 2.74 GiB/s. A phone
has neither that storage nor that bandwidth, and a 144 GB container is not going
onto it at all.

So the Android app is one of two products, sharing almost no code:

1. **A small-model runner.** 1B–4B, quantised, entirely resident. Real inference
   on the phone, honest about the ceiling.
2. **A client for a Chaos on a PC.** The engine already speaks the OpenAI API;
   over a LAN this is a chat client, and the phone does no inference at all.

Atur answered it — *"we see devices as a resource with chaos … yeah we run a
model in there, more simple and smaller models"* — so it is **both, and (2)
first**. That order is not a compromise: a client makes the *big* models usable
from the phone, which local inference never can, and it needs no NDK.

## What Phase A is

`android/` — one Activity, framework views, **no androidx and no dependencies at
all**. `HttpURLConnection` and `org.json` are in the framework, so the APK
contains this app and nothing else. The Rust side has zero dependencies on
principle; there was no reason for the phone half to arrive with a hundred.

| file | what it is |
|---|---|
| `ChaosClient.kt` | `/v1/models` and streaming `/v1/chat/completions` |
| `MainActivity.kt` | address, key, CONNECT, transcript, SEND |
| `Phone.kt` | this phone's memory, and what it *could* run locally |
| `tools/make-android-icons.py` | the mark from `assets/logo.svg` at all five densities |

`Phone.kt` is the other half of Atur's request — *"chaos based on phone options
run a model for that suggestions"*. It reads the device's memory, budgets half
of it (Android kills an app that asks for everything), and names the largest
model that would fit — while saying plainly that this app does not run models
yet. Saying otherwise would be the worst kind of wrong.

## The server had to change

`chaos-serve` bound `127.0.0.1` only, so nothing on the Wi-Fi could reach it.
`--host` opens that up, and with it comes a rule:

> **The api key stops being optional the moment the socket leaves loopback.**

On `127.0.0.1` a key guards nothing — a caller who can reach it can read the
weights off the disk anyway, and the flag's own doc comment said so. On a LAN
address it is the only thing between the model and every device on the network.
So `--host 0.0.0.0` without `--api-key` **refuses to start**, before the model
loads rather than four minutes after it.

`0.0.0.0` is the trap the loopback test exists for: it reads like "no address"
and means *every* address.

## It runs. Four defects, each found by running it

**2026-08-24, on an Android 34 x86_64 emulator**, against a real `chaos-serve`
on the host with `--host 0.0.0.0 --api-key`:

> **The capital of France is \*\*Paris\*\*.**

The project's own correctness prompt, answered on a phone through this client.
`chaos-serve` logged `GET /v1/models -> 200` and
`POST /v1/chat/completions -> 200 (stream)`. `Phone.kt` reported the device
correctly: *"unknown Android SDK built for x86_64 – 2.4 GB of memory. Could hold
Llama-3.2-1B locally (1.0 GiB) once Chaos runs on Android."*

Every one of these was invisible until it was on a screen:

| what running it showed | why |
|---|---|
| **"Chaos" twice, stacked** | the framework theme draws an ActionBar with the app label, and the layout has its own heading. `Theme.Material.NoActionBar`. |
| **the key field read "API key (required"** | CONNECT sits beside it, so the hint did not fit. Shortened. |
| **a bare `<think>` and `</think>` around the reply** | Qwen3.5 is a reasoning model. **The tags arrive split across streamed pieces** — Qwen3 emits `<`, `think`, `>` as three tokens — so filtering each piece as it arrives sees none of them. `ThinkFilter` accumulates and holds back only a tail that could still be forming a tag. |
| **the address and key were lost** | saved in `onPause`, which **never runs when the process is killed** rather than paused — swiped from recents, or reclaimed. Found by force-stopping and watching the field revert. Now saved on CONNECT as well. |

`ThinkFilter` is the one piece of real logic here and the one that cannot be
checked by looking, so it has nine unit tests — including that the result does
not depend on how the stream was chunked, and that an *unterminated* block is
released rather than swallowed, because a truncated stream showing nothing is
indistinguishable from a server that never replied. CI runs them.

**JUnit is the only dependency and it does not ship.** `testImplementation`
never reaches the APK, and an untested state machine over a fragmented stream is
a worse thing to ship than a test-only dependency. The rule in
`gui/app/tests/android_client.rs` allows JUnit by name and nothing else.

## Why CI builds the shipped APK, and how it was built here

**`dl.google.com` answers 404 to everything from this network.** Measured
2026-08-23 and again 2026-08-24 — including through a SOCKS proxy whose exit is
outside Iran, and against the exact URL `developer.android.com` itself links to,
so it is not a matter of guessing filenames:

| host | result |
|---|---|
| `developer.android.com` | 200 |
| `www.google.com` | 200 |
| `repo1.maven.org` (Maven Central) | 206 |
| `services.gradle.org` | 307 |
| `corretto.aws` (JDK 17) | 206 |
| **`dl.google.com/go/go1.21.0.windows-amd64.zip`** — a file that certainly exists | **404** |
| `dl.google.com/android/repository/platform-tools-latest-windows.zip` | 404 |
| `maven.google.com/...` | 301 → `dl.google.com`, then 404 |

The 404 comes from a server identifying itself as `Server: downloads` — Google's
own. It is not the URLs; it is the host, for this network. `dl.google.com` is
the sole distributor of the Android SDK, build-tools, platform jars and
androidx, so **no Android toolchain can be installed on this machine**.

GitHub's runners are not on that network and ship the SDK already installed, so
the `android` job in `release.yml` is where the **shipped** APK is made, from
Google's own repositories.

### The local toolchain, and what it cost in trust

Atur supplied a SOCKS5 proxy and said to use it. It did not help with
`dl.google.com` — but it reached `developer.android.com`, which gave the real
filenames, and from there public mirrors carry everything:

| piece | source |
|---|---|
| JDK 17 | `corretto.aws` — reachable directly |
| Gradle 8.7 | `services.gradle.org` — reachable directly |
| cmdline-tools, platform-tools, build-tools, platform 34, emulator, system image | `mirrors.cloud.tencent.com/AndroidSDK` |
| the Android Gradle Plugin | `maven.aliyun.com/repository/google` |

**Every SDK component was verified against Google's own SHA-1**, taken from the
repository manifest, before being unpacked. A checksum from a mirrored manifest
is not proof against a mirror that tampered with both — it is proof against a
truncated or corrupted transfer, which this project has been bitten by before.

**The mirrors are not in the repository.** `settings.gradle.kts` names
`google()` and `mavenCentral()`, which is what CI resolves against; the
redirection lives in a local init script passed with `-I`. Committing a mirror
would make every build everywhere depend on a third party for the sake of one
machine's network.

So: **fine for building and testing an APK here, and not what a release is
built from.** CI builds the shipped artefact.

A manual unzip leaves no `package.xml`, which is the SDK's own inventory, so
`avdmanager` reported *"emulator package must be installed!"* about an emulator
plainly present. Those files are generated from each component's
`source.properties` rather than hard-coded, so they cannot drift from what was
actually unpacked.

## Still to do

- [ ] **A signing key.** The APK is **debug-signed**, because an unsigned
      release APK cannot be installed and a key generated per build would give
      every release a different identity — Android would then refuse to upgrade
      in place. It is a real APK signed with a key everybody has. A keystore in
      the repository secrets replaces that whenever Atur wants one.
- [ ] **Phase B — small models locally.** NDK, ggml for
      `aarch64-linux-android`, and the Rust core cross-compiled. `core/probe` is
      the only crate with a platform assumption that has to change. Blocked on
      the same 404 above.
- [ ] **Phase C — the phone as a distributed worker.** Rejected for now, reason
      in `devices-as-resources.md`: Wi-Fi latency and battery make a phone a poor
      member of a layer loop. Revisit only with measurements.

## Definition of done

- `Chaos-vX-android-arm64.apk` attached to the release by CI. **Done** — the job
  runs the Kotlin tests, builds it, checks it is a zip containing a manifest,
  dex and resources, and uploads it; `publish` waits on it.
- It installs, opens, and does what the notes say. **Done on an emulator**:
  installs, launches, connects to a real `chaos-serve` over the network, and
  streams an answer. **Not yet on real hardware** — that is Atur's phone and
  nobody else's.
- The release notes say which of the two products it is, and what it will not
  do.
