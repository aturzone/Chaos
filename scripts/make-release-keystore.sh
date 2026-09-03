#!/usr/bin/env bash
# Make the one signing key every future Android release must share, and print
# the four secrets to paste into GitHub.
#
# **Why this exists.** v0.0.31 would not install over v0.0.30 -- "App not
# installed" -- and the cause was not the app. The release builds with
# `gradle assembleDebug`; on a developer's machine that signs with
# `~/.android/debug.keystore`, which persists, so upgrades work locally. On a
# fresh CI runner that file does not exist and **Gradle generates a new one,
# with a new key, on every single run**. Every release had a different identity,
# and Android correctly refuses to replace an app with one signed by a stranger.
#
# The build already accepts a real keystore. What is missing is the key itself,
# and it cannot live in this repository: the repository is public, and a
# committed signing key lets anyone build an APK that Android will accept as an
# upgrade over a user's install. So this is the one step that has to be run by
# hand, once, by Atur -- and this script is here so that it is one command
# rather than four from memory.
#
#   bash scripts/make-release-keystore.sh
#
# It asks for a password (twice, keytool's own prompt -- nothing here reads it,
# stores it, or sends it anywhere), writes `chaos-release.keystore`, and prints
# what to put in each of the four repository secrets.
set -eu

OUT="${1:-chaos-release.keystore}"
ALIAS="${CHAOS_KEY_ALIAS:-chaos}"

if ! command -v keytool >/dev/null 2>&1; then
  echo "keytool is not on PATH. It ships with any JDK:" >&2
  echo "  winget install EclipseAdoptium.Temurin.21.JDK    # Windows" >&2
  echo "  sudo apt install default-jdk                     # Debian/Ubuntu" >&2
  exit 2
fi

# **Never overwrite one.** Replacing the key is the same failure as having none:
# every install made with the old one becomes un-upgradeable, permanently.
if [ -e "$OUT" ]; then
  echo "$OUT already exists -- refusing to replace it." >&2
  echo "That file is irreplaceable: a new key cannot upgrade an install made" >&2
  echo "with the old one. If you are certain, move it aside yourself first." >&2
  exit 1
fi

echo "Making a 4096-bit RSA key valid for 10,000 days (about 27 years)."
echo "Use ONE password for both prompts -- the workflow passes the same value"
echo "as the store password and the key password."
echo

keytool -genkeypair -v \
  -keystore "$OUT" \
  -alias "$ALIAS" \
  -keyalg RSA -keysize 4096 -validity 10000

echo
echo "Wrote $OUT"
echo

B64="$OUT.b64"
if base64 -w0 "$OUT" > "$B64" 2>/dev/null; then :; else base64 "$OUT" | tr -d '\n' > "$B64"; fi
echo "Wrote $B64 ($(wc -c < "$B64" | tr -d ' ') characters)"
echo
echo "Now add four repository secrets:"
echo "  https://github.com/aturzone/Chaos/settings/secrets/actions"
echo
echo "  ANDROID_KEYSTORE_BASE64     the whole contents of $B64"
echo "  ANDROID_KEYSTORE_PASSWORD   the password you just chose"
echo "  ANDROID_KEY_ALIAS           $ALIAS"
echo "  ANDROID_KEY_PASSWORD        the same password"
echo
echo "Then BACK UP $OUT somewhere that is not this machine."
echo "Losing it means no future release can ever upgrade an install made with"
echo "it -- the same failure as today, permanently, with no way back except"
echo "uninstalling on every device."
echo
echo "Neither file is committable: both are in .gitignore, and the next release"
echo "after the secrets are set is the first one Android will install over."
