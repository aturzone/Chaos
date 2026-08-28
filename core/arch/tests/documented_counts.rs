//! The numbers the documents quote are the numbers the code has.
//!
//! **A sentence is not a mechanism.** `README.md` said *"every one of the 13
//! architectures was diffed against llama.cpp at 8 prompts"* while
//! `VERIFIED_ARCHITECTURES` held fourteen. Nothing caught it, because nothing was
//! looking — the same shape of drift that `scripts/check-test-count.sh` exists to
//! stop for the test count, found during §4b's claim audit.
//!
//! This is the mechanism for the architecture count.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("core/arch is two levels below the workspace root")
        .to_path_buf()
}

fn read(name: &str) -> String {
    let p = root().join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// **The verified-architecture count in the README is the list's real length.**
///
/// The README's sentence is the one a reader believes, and it is the one that
/// went stale. Written as a search for the phrase rather than a line number so
/// moving the paragraph does not break the check.
#[test]
fn the_readme_quotes_the_real_number_of_verified_architectures() {
    let n = chaos_arch::VERIFIED_ARCHITECTURES.len();
    let readme = read("README.md");

    let needle = format!("every one of the {n} architectures was diffed");
    assert!(
        readme.contains(&needle),
        "VERIFIED_ARCHITECTURES has {n} entries, and README.md does not say so.\n\
         Looked for: {needle:?}\n\
         Update the README in the same commit as the list, or this drifts again."
    );

    // And no *other* count may be claimed alongside it, which is how the stale
    // sentence survived: the right number appeared elsewhere in the file.
    for wrong in [n.saturating_sub(1), n + 1] {
        let stale = format!("every one of the {wrong} architectures was diffed");
        assert!(
            !readme.contains(&stale),
            "README.md still claims {wrong} architectures somewhere: {stale:?}"
        );
    }
}

/// **The binary count in `CLAUDE.md` is the number of `[[bin]]` targets.**
///
/// Third count to drift in one day, after the test count and the architecture
/// count: the file said seventeen, then eighteen, while the workspace had
/// nineteen. Counted from the manifests, which is the only place the answer
/// lives.
#[test]
fn claude_md_quotes_the_real_number_of_binaries() {
    let mut n = 0usize;
    let mut names = Vec::new();
    for bucket in ["core", "cli", "gui", "network", "android"] {
        let dir = root().join(bucket);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let manifest = e.path().join("Cargo.toml");
            let Ok(text) = std::fs::read_to_string(&manifest) else {
                continue;
            };
            let mut in_bin = false;
            for line in text.lines() {
                let t = line.trim();
                if t == "[[bin]]" {
                    in_bin = true;
                } else if t.starts_with('[') && t != "[[bin]]" {
                    in_bin = false;
                } else if in_bin {
                    if let Some(rest) = t.strip_prefix("name = ") {
                        names.push(rest.trim_matches('"').to_string());
                        n += 1;
                        in_bin = false;
                    }
                }
            }
        }
    }
    assert!(
        n > 10,
        "found only {n} binaries, so the walk is wrong, not the docs"
    );
    let claude = read("CLAUDE.md");
    let word = match n {
        17 => "Seventeen",
        18 => "Eighteen",
        19 => "Nineteen",
        20 => "Twenty",
        21 => "Twenty-one",
        _ => "",
    };
    assert!(
        !word.is_empty(),
        "{n} binaries and no word for it; extend this test's spelling table"
    );
    let needle = format!("**{word} binaries, not five**");
    assert!(
        claude.contains(&needle),
        "there are {n} binaries ({}) and CLAUDE.md does not say so.
         Looked for: {needle:?}",
        names.join(", ")
    );
}

/// **The release workflow's three staging lists must agree, and must name real
/// binaries.**
///
/// `chaos-qr` was in *none* of them while the brand tier claimed it "reaches a
/// bare terminal": a binary in no ship list does not exist, and nothing was
/// checking. There are three near-identical `for b in ...` loops -- one Unix, two
/// Windows -- so the live risk is a binary added to one and forgotten in the
/// others, which ships on some platforms and not others.
///
/// This does not decide *which* binaries ought to ship; that is an open question
/// in `research/4c-folder-structure-2026-08-28.md`. It checks only that the lists
/// are consistent with each other and with the manifests.
#[test]
fn the_ship_lists_agree_and_name_binaries_that_exist() {
    let yml = read(".github/workflows/release.yml");
    let mut lists: Vec<Vec<String>> = Vec::new();
    for line in yml.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("for b in ") {
            let names: Vec<String> = rest
                .trim_end_matches("; do")
                .split_whitespace()
                .map(str::to_string)
                .collect();
            if names.len() > 3 {
                lists.push(names);
            }
        }
    }
    assert_eq!(
        lists.len(),
        3,
        "expected three staging loops in release.yml, found {}. If the workflow          changed shape, this test has to change with it.",
        lists.len()
    );

    // Every name must be a real [[bin]] target.
    let mut real = std::collections::HashSet::new();
    for bucket in ["core", "cli", "gui", "network", "android"] {
        let Ok(entries) = std::fs::read_dir(root().join(bucket)) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(text) = std::fs::read_to_string(e.path().join("Cargo.toml")) else {
                continue;
            };
            let mut in_bin = false;
            for line in text.lines() {
                let t = line.trim();
                if t == "[[bin]]" {
                    in_bin = true;
                } else if t.starts_with('[') && t != "[[bin]]" {
                    in_bin = false;
                } else if in_bin {
                    if let Some(rest) = t.strip_prefix("name = ") {
                        real.insert(rest.trim_matches('"').to_string());
                        in_bin = false;
                    }
                }
            }
        }
    }
    for (i, list) in lists.iter().enumerate() {
        for name in list {
            assert!(
                real.contains(name),
                "release.yml staging list {i} ships {name:?}, which is not a                  [[bin]] target anywhere. A rename left this behind."
            );
        }
    }

    // **The two Windows lists must be identical**, and the Unix one may differ
    // only by the Windows-only binaries.
    let windows_only: std::collections::HashSet<&str> = ["chaos-app"].into_iter().collect();
    let set =
        |v: &Vec<String>| -> std::collections::HashSet<String> { v.iter().cloned().collect() };
    let unix = set(&lists[0]);
    for (i, w) in lists[1..].iter().enumerate() {
        let w = set(w);
        let extra: Vec<&String> = w.difference(&unix).collect();
        for e in &extra {
            assert!(
                windows_only.contains(e.as_str()),
                "windows list {} ships {e:?} and the unix list does not; if that                  is deliberate, add it to `windows_only` here",
                i + 1
            );
        }
        let missing: Vec<&String> = unix.difference(&w).collect();
        assert!(
            missing.is_empty(),
            "windows list {} is missing {missing:?}, which the unix list ships",
            i + 1
        );
    }
    let a = set(&lists[1]);
    let b = set(&lists[2]);
    assert_eq!(
        a,
        b,
        "the two windows staging lists disagree: {:?}",
        a.symmetric_difference(&b).collect::<Vec<_>>()
    );
}

/// The list is what the refusal message prints, so it must be non-empty, unique
/// and lower-case -- a duplicate would silently widen what counts as verified.
#[test]
fn the_verified_list_is_a_set_of_plain_names() {
    let list = chaos_arch::VERIFIED_ARCHITECTURES;
    assert!(
        !list.is_empty(),
        "nothing is verified, which cannot be right"
    );
    let mut seen = std::collections::HashSet::new();
    for a in list {
        assert!(seen.insert(*a), "{a} is listed twice");
        assert_eq!(
            *a,
            a.to_lowercase(),
            "{a} is not the container's own spelling"
        );
        assert!(!a.is_empty());
        assert!(
            !a.contains(char::is_whitespace),
            "{a:?} has whitespace, so it can never match an architecture string"
        );
    }
}
