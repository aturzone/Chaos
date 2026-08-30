//! A flag whose **value** is nonsense is refused, not swallowed.
//!
//! **`chaos-run model.gguf "hi" -n notanumber` used to load the model and
//! generate eight tokens.** The value parsed with `.parse().ok()` and fell back
//! to the default, so a typo in a number was indistinguishable from not passing
//! the flag at all: the run went ahead, at the wrong setting, and said nothing.
//! **Forty-three flags did this.**
//!
//! The project already counts it a defect for a flag *name* to be swallowed —
//! *"182 of 182 recognised, 17 declined with a written reason, 0 unrecognised"*
//! is a headline number in the README. A flag whose **value** is swallowed is
//! the same failure one level down, and it is the worse of the two: having the
//! name accepted is exactly what convinces you the value was accepted too.
//!
//! Found by writing this file. The argument loop had no tests, which is how a
//! systematic gap in the most-typed binary in the workspace stayed invisible.
//!
//! # What is deliberately still allowed
//!
//! **A flag with no value at all takes its default.** `-n` at the end of the
//! line is a different mistake, and refusing it is a separate decision with its
//! own blast radius. This file pins today's behaviour so the decision is made on
//! purpose rather than drifted into.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// `chaos-run`, beside the test binary rather than on `PATH`.
fn chaos_run() -> PathBuf {
    let exe = std::env::current_exe().expect("no current_exe");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("test binary is not under target/<profile>/deps");
    let name = if cfg!(windows) {
        "chaos-run.exe"
    } else {
        "chaos-run"
    };
    let p = dir.join(name);
    assert!(
        p.is_file(),
        "{} is not there. Build it first: cargo build --release --bin chaos-run",
        p.display()
    );
    p
}

fn run(args: &[&str]) -> Output {
    Command::new(chaos_run())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("could not run chaos-run: {e}"))
}

/// A path that is definitely not a model, so nothing can load even if the
/// argument loop lets the run proceed.
///
/// **The point of using one**: if a bad value were still swallowed, the binary
/// would get past the arguments and fail later with a *different* message. The
/// two failures are told apart by what they say, not by the exit code — both are
/// 2 — which is the same trap as "an exit code is not a diff".
const NOT_A_MODEL: &str = "definitely-not-a-model-anywhere.gguf";

#[test]
fn a_nonsense_number_is_refused_by_name() {
    // One per parsing shape in the argument loop: a default-bearing scalar, a
    // float, an optional that means "not set" when absent, one that is filtered
    // after parsing, and one of the long spellings.
    for (flag, value) in [
        ("-n", "notanumber"),
        ("--temp", "abc"),
        ("-t", "xyz"),
        ("-tb", "1e"),
        ("-c", "abc"),
        ("--seed", "nope"),
        ("--top-k", "many"),
        ("--n-predict", "lots"),
    ] {
        let out = run(&[NOT_A_MODEL, "hi", flag, value]);
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{flag} {value} did not exit 2.\nstdout:\n{}\nstderr:\n{err}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(
            err.contains(flag),
            "the refusal for {flag} {value} does not name the flag: {err}"
        );
        assert!(
            err.contains(value),
            "the refusal for {flag} {value} does not quote the value: {err}"
        );
        assert!(
            err.contains("wants a number"),
            "{flag} {value} failed for some other reason: {err}"
        );
        // **It must be clear nothing happened.** The whole complaint about the
        // old behaviour was a run that proceeded; saying so is what closes it.
        assert!(
            err.contains("Nothing was loaded"),
            "the refusal for {flag} does not say the run did not start: {err}"
        );
    }
}

/// **A good value still gets through**, which is the half a refusal can break.
///
/// The model does not exist, so this must fail — but on the *model*, not on the
/// arguments. That is the distinction the test is making.
#[test]
fn a_good_value_is_not_refused() {
    let out = run(&[NOT_A_MODEL, "hi", "-n", "2", "-t", "4", "-c", "512"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !err.contains("wants a number"),
        "a valid set of numbers was refused: {err}"
    );
    assert!(
        err.contains("no model") || err.contains(NOT_A_MODEL),
        "expected it to get as far as looking for the model: {err}"
    );
}

/// **A flag with no value at all still takes its default** — pinned on purpose.
#[test]
fn a_missing_value_is_left_alone() {
    let out = run(&[NOT_A_MODEL, "hi", "-n"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !err.contains("wants a number"),
        "a trailing -n was refused, which is a behaviour change this file exists \
         to make deliberate rather than accidental: {err}"
    );
}

/// The two things the argument loop already did right, kept honest.
#[test]
fn version_answers_and_an_unknown_flag_is_named() {
    let v = run(&["--version"]);
    assert_eq!(v.status.code(), Some(0));
    let text = String::from_utf8_lossy(&v.stdout);
    assert!(text.starts_with("chaos-run "), "{text}");

    let u = run(&["--nonexistent-flag"]);
    assert_eq!(u.status.code(), Some(2));
    let err = String::from_utf8_lossy(&u.stderr);
    assert!(
        err.contains("--nonexistent-flag"),
        "an unknown flag is not named back: {err}"
    );
}

/// With no arguments it explains itself rather than doing nothing.
#[test]
fn no_arguments_prints_usage() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("usage:"), "{err}");
}
