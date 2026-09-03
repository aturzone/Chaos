---
topic: Android refuses to upgrade Chaos in place because every release is signed with a different throwaway key — the build now accepts a real keystore, and adding it is a one-time step only Atur can do
status: build support done 2026-09-03; the secret is Atur's to add
links:
  - android-app.md
  - ../reference/hard-won-facts.md
---

# Android upgrades need one keystore, not a new one each release

**Reported from a real device, 2026-09-03**: installing v0.0.31 over v0.0.30 gave
*"App not installed"*. Uninstalling first and installing fresh worked.

## Why

`versionCode` was incrementing correctly — CI rewrites it from the tag — so the
only remaining cause of `INSTALL_FAILED_UPDATE_INCOMPATIBLE` is the **signature**.

The release builds with `gradle assembleDebug`. On a developer's machine that
uses `~/.android/debug.keystore`, which persists, so upgrades work locally. **On a
fresh CI runner that file does not exist and Gradle generates a new one** — with a
new random key — on every single run. Every release therefore had a different
identity, and Android correctly refused to replace one with the other.

The workflow's own comment predicted this exact failure:

> *"generating one per run would give every release a different identity and
> Android would refuse to upgrade in place"*

and then relied on the debug key being *"a key everyone has"*, which is true of a
persistent local keystore and false of an ephemeral runner. **A comment that
names the failure is not a check that prevents it.**

## What is built already

`android/app/build.gradle.kts` takes a keystore from Gradle properties or the
environment (`CHAOS_KEYSTORE`, `CHAOS_KEYSTORE_PASSWORD`, `CHAOS_KEY_ALIAS`,
`CHAOS_KEY_PASSWORD`) and signs **both** the release and debug build types with
it — both, because a release signed with one identity and a debug APK signed with
another would still refuse to upgrade across the two.

`release.yml` decodes `ANDROID_KEYSTORE_BASE64` into the runner's temp directory
and passes it through. With no secret the build **still succeeds and still
ships** — a release that refuses to build helps nobody — but it emits a warning
saying the APK cannot be upgraded over.

## The one-time step, which needs Atur

A signing key is not committed to this repository on purpose: it is public, and a
committed key lets anyone build an APK that Android will accept as an upgrade
over a user's install.

**One command**, `scripts/make-release-keystore.sh`, added in v0.0.32 so this is
not four steps from memory:

```bash
bash scripts/make-release-keystore.sh
```

It refuses to overwrite an existing keystore -- replacing the key is the same
failure as having none -- asks `keytool` for a password (nothing in the script
reads, stores or transmits it), writes the keystore and its base64, and prints
what to paste into each of the four secrets. Both files are gitignored.

Then four repository secrets (Settings → Secrets and variables → Actions):

| secret | value |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | the contents of the `.b64` file |
| `ANDROID_KEYSTORE_PASSWORD` | the store password chosen above |
| `ANDROID_KEY_ALIAS` | `chaos` |
| `ANDROID_KEY_PASSWORD` | the key password chosen above |

**Keep the `.keystore` file somewhere safe and backed up.** Losing it means no
future release can ever upgrade an install made with it — the same failure as
today, permanently, with no way back except uninstalling on every device.

## Until then

Every release remains upgrade-incompatible with the last, and the honest thing is
to say so where someone reads before installing rather than after. `SUPPORT.md`
carries it.
