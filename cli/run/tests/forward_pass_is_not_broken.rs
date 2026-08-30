//! Does the engine still compute what it computed yesterday?
//!
//! **This is the test the project did not have, and it is the one it needed
//! most.** `CLAUDE.md` states the failure mode plainly: *"A wrong forward pass
//! produces fluent nonsense, never a crash."* The only defence against it was
//! the `--ignored` container tests, and **CI ran them with no model on disk**.
//! The workflow's own comment admitted it: *"This proves the skip path works,
//! not that the tests pass."* So every green run was compatible with a forward
//! pass that had silently regressed.
//!
//! # Four layers, and each one's sensitivity is measured rather than assumed
//!
//! Corruption was written into a copy of the model at its midpoint and the
//! answer to *"The capital of France is"* read back, on 2026-08-31:
//!
//! | zeroed | what it answered | caught by |
//! |---|---|---|
//! | 4 KiB | byte-identical to intact | **nothing — nothing to catch** |
//! | 1 MiB | "Paris. It is the most populous city in **the world**" | the golden only |
//! | 16 MiB | "the capital of the **French language**" | tripwire and golden |
//!
//! That table is why there are two output checks rather than one. **The
//! substring tripwire alone would have passed the 1 MiB case** — and it would
//! have passed §4e's own demonstration too, whose corrupt answer also contained
//! the word *Paris*. A test that only looks for the right word is a test that
//! agrees with a broken engine, and finding that out cost one experiment.
//!
//! The 4 KiB row is worth keeping for the opposite reason: it is *not* a hole in
//! this test. Four kilobytes in 397 MB landed in weights this prompt does not
//! exercise, and no output check can see a difference that does not exist.
//! **Corruption is `chaos verify`'s job** — it hashes the container and says so
//! in milliseconds. This file's job is regression: did *our code* change what we
//! compute.
//!
//! 1. **The golden** — byte-exact stdout, per platform, under `goldens/`. The
//!    sharp instrument, and the one that is not portable: five platform and
//!    architecture combinations run this suite, and a different SIMD path can
//!    flip a token near a probability tie without anything being wrong. So it
//!    runs **only where a golden for this platform exists**, and says clearly
//!    when there is none rather than inventing a pass.
//! 2. **The tripwire** — `Paris`, `Pacific`, `H2O`, `east`. Portable, chosen far
//!    from any tie, and it catches the gross breakage on every platform.
//! 3. **Determinism** — two identical runs agree. Greedy decoding is a
//!    deterministic function of the weights, so a run that disagrees with itself
//!    is a race or an uninitialised read, and neither is visible in an answer.
//! 4. **Thread-invariance** — `-t 1` and `-t 4` agree byte-for-byte. Verified to
//!    hold on 2026-08-31 at `-t` 1, 4 and 8. This is the one that catches a
//!    reduction written so its result depends on how the work was split.
//!
//! # Running it
//!
//! ```text
//! CHAOS_TEST_MODEL=/path/to/Qwen2-0.5B-Instruct-Q4_K_M.gguf cargo test --release
//! ```
//!
//! With no `CHAOS_TEST_MODEL` this skips and says so. **CI sets
//! `CHAOS_REQUIRE_MODEL_TESTS=1`, which turns the skip into a failure** — the
//! whole point being that a gate nobody runs is not a gate.
//!
//! To record a golden on a new platform, run with `CHAOS_RECORD_GOLDEN=1`. Do
//! that **only** after establishing the arithmetic is right on that platform;
//! recording a golden from a broken engine freezes the breakage in.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The prompts, and the word the answer has to contain.
///
/// Verified against `Qwen2-0.5B-Instruct-Q4_K_M` on 2026-08-31. They are **not**
/// a general knowledge test: at 0.5B this model says the capital of Japan is
/// Kyoto and that two plus two equals three. What is asserted is that the engine
/// reproduces what this model computes when the arithmetic is right, and these
/// four are the ones it gets right with room to spare.
const CHECKS: &[(&str, &str)] = &[
    ("The capital of France is", "Paris"),
    ("The largest ocean on Earth is the", "Pacific"),
    ("The chemical symbol for water is", "H2O"),
    ("The sun rises in the", "east"),
];

/// The prompt and length the golden was recorded at. Changing either invalidates
/// every golden on every platform, so they live here as one fact.
const GOLDEN_PROMPT: &str = "The capital of France is";
const GOLDEN_TOKENS: u32 = 24;

/// The model these expectations were recorded against.
const RECORDED_FOR: &str = "qwen2-0.5b";

fn model() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("CHAOS_TEST_MODEL")?);
    if p.is_file() {
        return Some(p);
    }
    panic!(
        "CHAOS_TEST_MODEL is set to {} and that is not a file. A gate pointed at \
         nothing is worse than no gate: it reports success.",
        p.display()
    );
}

/// `chaos-run`, beside the test binary rather than on `PATH`.
///
/// The same rule the `chaos` front door uses: a `chaos-run` elsewhere on `PATH`
/// is a different build, and testing a different build than the one just
/// compiled is a way to pass while broken.
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

fn generate_with(model: &Path, prompt: &str, n: u32, threads: Option<u32>) -> String {
    let mut cmd = Command::new(chaos_run());
    cmd.arg(model)
        .arg(prompt)
        .args(["-n", &n.to_string()])
        // Greedy is already the default; passed anyway so a change of default
        // cannot silently make this test non-deterministic.
        .args(["--temp", "0"])
        .arg("--no-perf");
    if let Some(t) = threads {
        cmd.args(["-t", &t.to_string()]);
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("could not run chaos-run: {e}"));
    assert!(
        out.status.success(),
        "chaos-run exited {:?} on {prompt:?}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    // Line endings differ between the platforms this runs on and say nothing
    // about the arithmetic, so they are normalised before anything is compared.
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

fn generate(model: &Path, prompt: &str, n: u32) -> String {
    generate_with(model, prompt, n, None)
}

/// `<model stem>.<arch>-<os>.txt`, so a golden can never be read on a platform
/// it was not recorded on.
fn golden_path(model: &Path) -> PathBuf {
    let stem = model
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("goldens")
        .join(format!(
            "{stem}.{}-{}.txt",
            std::env::consts::ARCH,
            std::env::consts::OS
        ))
}

/// The skip, made loud, and fatal where it matters.
fn skipped(reason: &str) -> bool {
    if std::env::var_os("CHAOS_REQUIRE_MODEL_TESTS").is_some() {
        panic!(
            "CHAOS_REQUIRE_MODEL_TESTS is set and this test cannot run: {reason}. \
             This is the gate that catches a regressed forward pass, and it is not \
             allowed to skip here."
        );
    }
    eprintln!(
        "SKIPPED: {reason} -- set CHAOS_TEST_MODEL to a GGUF to run it, and \
         CHAOS_REQUIRE_MODEL_TESTS=1 to make this skip a failure"
    );
    true
}

/// Skip unless there is a model *and* it is the one the expectations describe.
fn model_or_skip() -> Option<PathBuf> {
    let m = match model() {
        Some(m) => m,
        None => {
            skipped("CHAOS_TEST_MODEL is not set");
            return None;
        }
    };
    let name = m
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !name.contains(RECORDED_FOR) {
        skipped(&format!(
            "these expectations were recorded against {RECORDED_FOR}, and the model is {name}"
        ));
        return None;
    }
    Some(m)
}

/// **Layer 1: the byte-exact golden, where this platform has one.**
#[test]
fn the_output_is_byte_for_byte_what_it_was() {
    let Some(m) = model_or_skip() else { return };
    let got = generate(&m, GOLDEN_PROMPT, GOLDEN_TOKENS);
    let path = golden_path(&m);

    if std::env::var_os("CHAOS_RECORD_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).expect("cannot make goldens/");
        std::fs::write(&path, got.as_bytes()).expect("cannot write the golden");
        eprintln!("RECORDED {}", path.display());
        return;
    }

    let Ok(want) = std::fs::read_to_string(&path) else {
        // **A stated absence, and deliberately not fatal even under
        // `CHAOS_REQUIRE_MODEL_TESTS`.** A platform with no golden yet is a
        // legitimate state — only `x86_64-windows` has one, because that is
        // where it could be recorded from. Failing here would mean the gate
        // could not be turned on for Linux and macOS at all, which is the
        // opposite of the point. The other three layers still run everywhere.
        //
        // What is *not* acceptable is inventing a pass: that is how the GPU
        // tests once reported six green for six early returns.
        eprintln!(
            "NO GOLDEN for this platform at {}\n  \
             The other three layers still ran. To turn this layer on here, \
             establish the arithmetic is right on this platform and then record \
             it with CHAOS_RECORD_GOLDEN=1.",
            path.display()
        );
        return;
    };
    let want = want.replace("\r\n", "\n");
    assert_eq!(
        got.trim_end(),
        want.trim_end(),
        "the engine no longer produces the text recorded in {}.\n\
         This is a change in what Chaos computes. Measured sensitivity: 1 MiB of \
         zeros in this model moves this string and does NOT move the tripwire, so \
         do not dismiss it as noise. If the change is intended, re-record with \
         CHAOS_RECORD_GOLDEN=1 and say in the commit why the arithmetic is right.",
        path.display()
    );
}

/// **Layer 2: the portable tripwire.**
#[test]
fn the_forward_pass_still_answers_correctly() {
    let Some(m) = model_or_skip() else { return };
    let mut wrong = Vec::new();
    for (prompt, want) in CHECKS {
        let text = generate(&m, prompt, 12);
        // The prompt is echoed before the answer, so look past it.
        let answer = text.split(prompt).nth(1).unwrap_or(&text);
        if !answer.contains(want) {
            wrong.push(format!("{prompt:?} -> {answer:?}, wanted {want:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "the forward pass has changed what this model says.\n  {}\n\n\
         Do not adjust these strings to match the new output without first \
         establishing that the arithmetic is right.",
        wrong.join("\n  ")
    );
}

/// **Layer 3: two identical runs produce identical text.**
///
/// Greedy decoding is a deterministic function of the weights. A run that
/// disagrees with itself is a race or an uninitialised read, and neither shows
/// up in an answer — both runs can be fluent and only one correct.
#[test]
fn generation_is_deterministic() {
    let Some(m) = model_or_skip() else { return };
    let a = generate(&m, CHECKS[0].0, 16);
    let b = generate(&m, CHECKS[0].0, 16);
    assert_eq!(
        a, b,
        "two greedy runs of the same prompt disagreed, which is a race or an \
         uninitialised read rather than a wrong answer"
    );
}

/// **Layer 4: the thread count does not change the answer.**
///
/// How the work is split across threads is an implementation detail, and an
/// answer that depends on it means a reduction whose result depends on its
/// order. Verified to hold on 2026-08-31 at `-t` 1, 4 and 8, byte-identical.
///
/// This is the layer that needs no golden and no model-specific knowledge, so
/// it is the one that keeps working when everything else about this file goes
/// stale.
#[test]
fn the_thread_count_does_not_change_the_answer() {
    let Some(m) = model_or_skip() else { return };
    let one = generate_with(&m, CHECKS[0].0, 16, Some(1));
    let four = generate_with(&m, CHECKS[0].0, 16, Some(4));
    assert_eq!(
        one, four,
        "-t 1 and -t 4 produced different text. The thread count is an \
         implementation detail; an answer that depends on it means a reduction \
         whose result depends on how the work was divided."
    );
}
