#!/usr/bin/env bash
# Can a Chaos that is already installed still find today's release?
#
# **This is the question a VM cannot answer cheaply and a unit test cannot
# answer at all.** An updater is the one component whose old versions never get
# fixed: a build from v0.0.12 asks the feed for the asset name *it* computes,
# and nothing in a later release can teach it a different one. So the thing
# worth checking is not "does the current updater work" -- it is "does every
# updater ever shipped still work against the feed as it stands today".
#
# Every tag's `release.rs` is self-contained (no `use`, no `crate::`), so each
# version's own parse/decide/asset-name logic compiles standalone with `rustc`
# and can be pointed at the live feed. That is what this does: one binary per
# released version, built from that version's own source, run against today's
# GitHub answer.
#
# Offline companion: `core/model/tests/the_update_contract.rs`, which holds the
# names to the workflow and runs in CI on every commit. This script needs the
# network and is therefore not a CI gate.
#
#   bash scripts/check-old-updaters.sh            # against the live feed
#   bash scripts/check-old-updaters.sh feed.json  # against a saved feed
#
# Exit 0 if every version with an updater resolves the newest release to a real
# download URL; 1 if any of them would be stranded.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 1

# The updater first appeared in v0.0.12. v0.0.5 to v0.0.11 shipped a window and
# an installer with **no in-app update path at all**, and v0.0.0 to v0.0.4 had
# no window; nothing here can fix those, and pretending to check them would
# report a pass for versions that have nothing to test.
FIRST_WITH_UPDATER="0.0.12"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

feed="${1:-}"
if [ -z "$feed" ]; then
  feed="$work/latest.json"
  echo "fetching the live release feed..."
  if ! curl -sS -H "User-Agent: Chaos" -H "Accept: application/vnd.github+json" \
      "https://api.github.com/repos/aturzone/Chaos/releases/latest" -o "$feed"; then
    echo "could not reach the release feed; pass a saved copy as an argument" >&2
    exit 1
  fi
fi
bytes=$(wc -c < "$feed" | tr -d ' ')
if [ "$bytes" -lt 500 ]; then
  echo "the feed is only ${bytes} bytes -- that is an error page, not a release" >&2
  exit 1
fi
echo "feed: ${bytes} bytes, newest tag $(grep -o '"tag_name":"[^"]*"' "$feed" | head -1)"
echo

# The harness appended after each version's module. Only the three entry points
# every version has ever had are called, so it compiles against all of them.
cat > "$work/harness.rs" <<'RUST'
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let json = std::fs::read_to_string(&a[1]).unwrap();
    let running = Version::parse(&a[2]).unwrap();
    match decide(parse_latest(&json), running) {
        Outcome::Available { version, url } => println!(
            "AVAILABLE {} asset={} url_ok={}",
            version.text(),
            asset_for_platform(&version),
            url.contains("/releases/download/")
        ),
        Outcome::UpToDate(v) => println!("UPTODATE {}", v.text()),
        Outcome::NoAssetForPlatform(v) => {
            println!("NO-ASSET-FOR-PLATFORM {} wanted={}", v.text(), asset_for_platform(&v))
        }
        Outcome::Failed(w) => println!("FAILED {w}"),
    }
}
RUST

fail=0
checked=0
newest=""

for tag in $(git tag --list 'v*' | sort -V); do
  ver="${tag#v}"
  # Skip anything older than the first version that had an updater.
  if [ "$(printf '%s\n%s\n' "$ver" "$FIRST_WITH_UPDATER" | sort -V | head -1)" = "$ver" ] \
     && [ "$ver" != "$FIRST_WITH_UPDATER" ]; then
    printf "%-9s -- no in-app updater in this version\n" "$tag"
    continue
  fi
  src=$(git ls-tree -r --name-only "$tag" \
        | grep -E '(crates/chaos-model|core/model)/src/release\.rs$' | head -1)
  if [ -z "$src" ]; then
    printf "%-9s -- no release module found at this tag\n" "$tag"
    continue
  fi
  safe=$(echo "$ver" | tr '.' '_')
  { git show "$tag:$src"; cat "$work/harness.rs"; } > "$work/v$safe.rs"
  # `env!("CARGO_PKG_VERSION")` is read from rustc's own environment.
  if ! CARGO_PKG_VERSION="$ver" rustc --edition 2021 -O \
        -o "$work/v$safe.exe" "$work/v$safe.rs" 2>"$work/v$safe.err"; then
    printf "%-9s -> WILL NOT BUILD: %s\n" "$tag" \
      "$(grep -m1 '^error' "$work/v$safe.err")"
    fail=1
    continue
  fi
  out=$("$work/v$safe.exe" "$feed" "$ver")
  printf "%-9s -> %s\n" "$tag" "$out"
  checked=$((checked + 1))
  case "$out" in
    AVAILABLE*url_ok=true) : ;;
    UPTODATE*) newest="$tag" ;;
    *) fail=1 ;;
  esac
done

echo
if [ "$checked" -eq 0 ]; then
  echo "nothing was checked -- that is a failure of this script, not a pass" >&2
  exit 1
fi
if [ "$fail" -ne 0 ]; then
  echo "STRANDED: at least one released version cannot reach today's update." >&2
  echo "Anything but AVAILABLE/url_ok=true (or UPTODATE for the newest) means a" >&2
  echo "user on that version is told nothing and must find the release by hand." >&2
  exit 1
fi
echo "OK: ${checked} versions checked, every one of them resolves today's release."
[ -n "$newest" ] && echo "     ${newest} reports itself up to date, which is the newest tag."
