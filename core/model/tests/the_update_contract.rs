//! The names an installed copy of Chaos will ask for, held to what the
//! release workflow actually publishes.
//!
//! **An updater is the one component whose old versions never get fixed.** A
//! build from v0.0.12 computes the asset name *it* was written with, asks the
//! feed for exactly that, and gives up if it is not there. Nothing in a later
//! release can teach it a new name. So renaming a release asset does not break
//! the next update -- it strands every copy already installed, silently, and the
//! only symptom a user sees is that Chaos stops mentioning new versions.
//!
//! `release.yml` says of its matrix: *"`name` is the contract ... and a test
//! pins that list."* **It did not.** The unit tests in `release.rs` compare the
//! function against a fixture written beside it, so they agree with themselves
//! and would go on agreeing after the workflow was renamed. This file is the
//! missing half, and it reads the workflow.
//!
//! Verified once by construction, 2026-08-28: every release from v0.0.12 to
//! v0.0.22 was compiled from its own tag and run against the real feed for
//! v0.0.23. All eleven asked for `Chaos-v0.0.23-windows-x86_64-Setup.exe` and
//! got a working download URL. `scripts/check-old-updaters.sh` repeats that
//! against the live feed; this file is what runs offline, in CI, every commit.

use chaos_model::release::{asset_name, Version, PLATFORMS};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("core/model is two levels below the workspace root")
        .to_path_buf()
}

fn read(name: &str) -> String {
    let p = root().join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// The `name:` of every row of the release matrix, with its archive format.
fn matrix_rows(yml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in yml.lines() {
        let t = line.trim();
        if !t.starts_with("- { os:") {
            continue;
        }
        let field = |key: &str| -> Option<String> {
            let i = t.find(key)? + key.len();
            let rest = t[i..].trim_start();
            let end = rest.find([',', '}']).unwrap_or(rest.len());
            Some(rest[..end].trim().replace('\'', ""))
        };
        if let (Some(n), Some(a)) = (field("name:"), field("archive:")) {
            out.push((n, a));
        }
    }
    out
}

/// **Every platform the updater can ask about is a row of the matrix.**
///
/// Both directions, because either half going missing is the same outage seen
/// from a different side: a platform with no row is a user offered a file that
/// was never built, and a row with no platform is a build nobody is ever told
/// about.
#[test]
fn the_updaters_platforms_are_exactly_the_matrixs_rows() {
    let yml = read(".github/workflows/release.yml");
    let rows = matrix_rows(&yml);
    assert!(
        rows.len() >= 5,
        "found {} matrix rows in release.yml, which cannot be right -- the parse \
         in this file no longer matches the shape of the workflow",
        rows.len()
    );

    let mut mine: Vec<String> = PLATFORMS
        .iter()
        .map(|(os, arch)| format!("{os}-{arch}"))
        .collect();
    let mut theirs: Vec<String> = rows.iter().map(|(n, _)| n.clone()).collect();
    mine.sort();
    theirs.sort();
    assert_eq!(
        mine, theirs,
        "release::PLATFORMS and the matrix in release.yml disagree. An installed \
         copy asks for the name it computes, so a matrix rename strands every \
         version already on a disk rather than only the next one."
    );
}

/// **The asset name each platform gets is the file that platform publishes.**
///
/// A `.tar.gz` name against a row that builds a `zip` is the
/// macOS-tarball-on-Windows failure one level up: the download succeeds, the
/// file is real, and nothing about it is an installer.
#[test]
fn every_asset_name_matches_the_archive_that_row_builds() {
    let yml = read(".github/workflows/release.yml");
    let v = Version(0, 0, 12);
    for (name, archive) in matrix_rows(&yml) {
        let (os, arch) = name
            .split_once('-')
            .unwrap_or_else(|| panic!("matrix row {name:?} is not <os>-<arch>"));
        let asset = asset_name(&v, os, arch);
        if os == "windows" {
            // Windows is the one row whose advertised asset is not the archive:
            // it is the installer, copied out beside it. So its name is checked
            // against that copy rather than against `archive`.
            assert_eq!(asset, "Chaos-v0.0.12-windows-x86_64-Setup.exe");
            assert_eq!(archive, "zip", "the Windows row no longer builds a zip");
            continue;
        }
        assert!(
            asset.ends_with(&format!(".{archive}")),
            "the updater will ask for {asset}, and the {name} row builds a .{archive}"
        );
        assert!(
            asset.contains(&name),
            "the updater will ask for {asset}, which does not carry the row name {name}"
        );
    }
}

/// **The two literals the workflow writes those names with are still there.**
///
/// The archives are `Chaos-${VER}-${{ matrix.name }}` plus an extension, and
/// the installer is copied out under its own name. Both are string-built in
/// shell, so nothing but a search for the literal can tell whether they still
/// agree with this crate.
#[test]
fn the_workflow_still_builds_those_names() {
    let yml = read(".github/workflows/release.yml");
    for needle in [
        r#"DIR="Chaos-${VER}-${{ matrix.name }}""#,
        "Chaos-${VER}-windows-x86_64-Setup.exe",
    ] {
        assert!(
            yml.contains(needle),
            "release.yml no longer contains {needle:?}. If the naming changed, \
             every installed copy of Chaos from v0.0.12 onwards is now asking \
             for a file that does not exist."
        );
    }
}

/// A tag is `v0.0.23`, and the name carries exactly one `v`.
///
/// `Chaos-vv0.0.23-...` and `Chaos-0.0.23-...` are each one character from
/// correct, and each fetches nothing.
#[test]
fn the_version_appears_once_and_with_its_v() {
    let v = Version(1, 2, 3);
    for (os, arch) in PLATFORMS {
        let n = asset_name(&v, os, arch);
        assert!(n.contains("-v1.2.3-"), "{n} does not carry -v1.2.3-");
        assert_eq!(n.matches("1.2.3").count(), 1, "{n} names the version twice");
        assert!(!n.contains("vv"), "{n} doubled the v");
    }
}

/// The Android APK is **not** an asset the desktop updater may pick.
///
/// `linux-arm64` and `android-arm64` differ by one word, and a phone build
/// downloaded onto a Raspberry Pi is a silent no-op of exactly the kind this
/// file exists to prevent.
#[test]
fn no_platform_resolves_to_the_apk() {
    let v = Version(0, 0, 23);
    for (os, arch) in PLATFORMS {
        let n = asset_name(&v, os, arch);
        assert!(!n.contains("android"), "{n} names the phone build");
        assert!(!n.ends_with(".apk"), "{n} is an APK");
    }
}
