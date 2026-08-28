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
