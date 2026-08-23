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

## Why it is built in CI and not here

**`dl.google.com` answers 404 to everything from this network.** Measured
2026-08-23:

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
the `android` job in `release.yml` is where the APK is made.

**The consequence, stated plainly: nothing about this app has been run.** The
build is the only check it has had, and the phone is Atur's. If he wants it
verified before a release, the honest routes are his own VPN exit or a machine
with access — both his call, not something to route traffic through unasked.

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
  builds it, checks it is a zip containing a manifest, dex and resources, and
  uploads it; `publish` waits on it.
- It installs on a real phone, opens, and does what the notes say. **Not yet** —
  see above.
- The release notes say which of the two products it is, and what it will not do.
