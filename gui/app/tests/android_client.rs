//! Rules for the Android client, checked from here because it cannot be built
//! here.
//!
//! # Why these are Rust tests
//!
//! `android/` is not a Cargo workspace member — it is a Gradle project with a
//! different toolchain — and **its toolchain cannot be installed on this
//! machine at all**: `dl.google.com`, the sole distributor of the Android SDK,
//! answers 404 to every request from this network. So the only build the app
//! ever gets is the one in CI, and a mistake that Gradle would catch in ninety
//! seconds instead costs a whole workflow run.
//!
//! These are the mistakes worth catching before that. Each one has already
//! happened.
//!
//! It lives under `gui/` because that is where this workspace keeps the things
//! with windows, and the Android client is the second of them.

use std::path::{Path, PathBuf};

fn android_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("android")
}

fn xml_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("xml") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&android_dir(), &mut out);
    out.sort();
    out
}

/// **`--` is not permitted inside an XML comment.** It is in the spec, and it
/// is exactly what this project's prose uses for an em dash everywhere else —
/// so writing an Android resource in the house style produces a file that
/// `aapt2` refuses.
///
/// This cost a full CI run. The build resolved the Android plugin, found the
/// SDK, compiled the Kotlin and executed eight tasks, and then failed on two
/// comments:
///
/// ```text
/// Failed to compile resource file: .../layout/activity_main.xml
/// Message: The string "--" is not permitted within comments.
/// ```
#[test]
fn no_xml_comment_contains_a_double_hyphen() {
    let files = xml_files();
    assert!(!files.is_empty(), "no Android XML found — did the tree move?");

    for path in &files {
        let text = std::fs::read_to_string(path).expect("readable");
        let mut rest = text.as_str();
        while let Some(start) = rest.find("<!--") {
            let after = &rest[start + 4..];
            let end = after
                .find("-->")
                .unwrap_or_else(|| panic!("{}: an XML comment is never closed", path.display()));
            let body = &after[..end];
            assert!(
                !body.contains("--"),
                "{}: an XML comment contains \"--\", which aapt2 refuses. \
                 Use an en dash. The comment begins: {:?}",
                path.display(),
                body.trim().chars().take(60).collect::<String>()
            );
            rest = &after[end + 3..];
        }
    }
}

/// The launcher icon at every density Android asks for, plus the adaptive
/// foreground.
///
/// **An APK with no icon gets a grey robot**, on the home screen, for ever.
/// `tools/make-android-icons.py` renders these from `assets/logo.svg` at each
/// place's own pixel size — Atur's standing rule — and forgetting to re-run it
/// after the mark changes is silent.
#[test]
fn the_launcher_icon_exists_at_every_density() {
    let res = android_dir().join("app/src/main/res");
    for bucket in ["mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"] {
        for name in ["ic_launcher.png", "ic_launcher_foreground.png"] {
            let p = res.join(format!("mipmap-{bucket}")).join(name);
            assert!(p.exists(), "missing {}", p.display());
            let bytes = std::fs::read(&p).expect("readable");
            assert_eq!(
                &bytes[..8],
                b"\x89PNG\r\n\x1a\n",
                "{} is not a PNG",
                p.display()
            );
        }
    }
    // The adaptive icon references the foreground; a background colour that
    // does not resolve is a build failure rather than a missing pixel.
    let adaptive = res.join("mipmap-anydpi-v26/ic_launcher.xml");
    let text = std::fs::read_to_string(&adaptive).expect("adaptive icon");
    assert!(text.contains("@mipmap/ic_launcher_foreground"));
    assert!(text.contains("@color/accent"));
    let colors = std::fs::read_to_string(res.join("values/colors.xml")).expect("colors");
    assert!(
        colors.contains("name=\"accent\""),
        "the adaptive icon's background colour is not defined"
    );
}

/// Every string the layout references must exist.
///
/// A missing `@string/...` is a resource-linking failure, which is another
/// ninety seconds of CI to be told something a grep answers.
#[test]
fn every_referenced_string_is_defined() {
    let res = android_dir().join("app/src/main/res");
    let strings = std::fs::read_to_string(res.join("values/strings.xml")).expect("strings.xml");

    for file in ["layout/activity_main.xml", "../AndroidManifest.xml"] {
        let path = res.join(file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut rest = text.as_str();
        while let Some(i) = rest.find("@string/") {
            let after = &rest[i + "@string/".len()..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            assert!(
                strings.contains(&format!("name=\"{name}\"")),
                "{}: @string/{name} is used but not defined in strings.xml",
                path.display()
            );
            rest = &after[name.len()..];
        }
    }
}

/// The build declares no dependencies, and that is deliberate.
///
/// The Rust side of this project has none; the phone half arriving with a
/// hundred transitive androidx artifacts would be a different kind of project.
/// It is also what lets the APK build from the SDK and the Kotlin plugin alone.
#[test]
fn the_apk_has_no_dependencies() {
    let build = std::fs::read_to_string(android_dir().join("app/build.gradle.kts"))
        .expect("app/build.gradle.kts");
    assert!(
        build.contains("dependencies {}"),
        "the Android app has grown a dependency — if that is deliberate, this \
         test is the place to say so"
    );
    // **A declaration, not the word.** The first version of this test failed
    // on the comment above `dependencies {}` explaining why there are none --
    // a check that a file does not mention a thing is not a check that it does
    // not use it, and it made the file harder to document for no benefit.
    for line in build.lines().map(str::trim) {
        if line.starts_with("//") {
            continue;
        }
        for verb in ["implementation(", "api(", "compileOnly(", "runtimeOnly("] {
            assert!(
                !line.starts_with(verb),
                "the Android app declares a dependency: {line}"
            );
        }
    }
    // CI rewrites both from the tag; they have to be there to be rewritten.
    assert!(build.contains("versionName = "));
    assert!(build.contains("versionCode = "));
}
