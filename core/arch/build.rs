//! Fail early, and in one sentence, when `ggml` is missing.
//!
//! `chaos-arch` is the only crate that cannot be built without `ggml`: it is
//! the forward pass, and the forward pass is `ggml` graphs. Every other crate in
//! the workspace builds fine without it, which is deliberate — the container,
//! probe and planning tools are useful on a machine that has never compiled a
//! line of C.
//!
//! Without this file the failure is a wall of `unresolved import
//! chaos_ggml::Context` errors, repeated once per module, which says nothing
//! about the actual problem: an environment variable is not set. That is the
//! first thing a new contributor sees, and "the build exploded" is a worse first
//! impression than "you need one more step, here it is".

use std::path::PathBuf;

const REQUIRED: [&str; 3] = ["ggml-base", "ggml-cpu", "ggml"];

fn main() {
    // Every binary in this crate -- chaos-run, chaos-serve and the benches --
    // shipped with the blank Windows default until the icon logic was shared.
    chaos_build::embed_icon();
    println!("cargo:rerun-if-env-changed=GGML_LIB_DIR");

    let Some(dir) = std::env::var_os("GGML_LIB_DIR").map(PathBuf::from) else {
        fail("GGML_LIB_DIR is not set.");
    };

    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|name| archive(&dir, name).is_none())
        .collect();

    if !missing.is_empty() {
        fail(&format!(
            "GGML_LIB_DIR is {}, but it does not contain: {}",
            dir.display(),
            missing
                .iter()
                .map(|n| format!("{n}.a"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn fail(why: &str) -> ! {
    // `cargo:warning` lines survive into the error output and are not truncated
    // the way a panic message can be, so the instructions arrive intact.
    for line in [
        "".to_string(),
        format!("chaos-arch cannot build: {why}"),
        "".to_string(),
        "  chaos-arch is the inference engine and needs ggml's static libraries.".to_string(),
        "  Every other crate in this workspace builds without them.".to_string(),
        "".to_string(),
        "  1. Build ggml once:".to_string(),
        "       git clone https://github.com/ggml-org/llama.cpp".to_string(),
        "       cmake -S llama.cpp -B llama.cpp/build -DCMAKE_BUILD_TYPE=Release \\".to_string(),
        "         -DBUILD_SHARED_LIBS=OFF   # llama.cpp defaults to shared; we need .a".to_string(),
        "       cmake --build llama.cpp/build --config Release -j".to_string(),
        "".to_string(),
        "  2. Point Chaos at the result (the directory holding ggml-base.a or".to_string(),
        "     libggml-base.a -- cmake names them differently per platform, and".to_string(),
        "     either is accepted):".to_string(),
        "     ggml-cpu.a and ggml.a -- usually llama.cpp/build/ggml/src):".to_string(),
        "       export GGML_LIB_DIR=/path/to/llama.cpp/build/ggml/src".to_string(),
        "       $env:GGML_LIB_DIR = \"C:/path/to/llama.cpp/build/ggml/src\"   # PowerShell"
            .to_string(),
        "".to_string(),
        "  Full instructions: https://github.com/aturzone/Chaos#building".to_string(),
        "".to_string(),
    ] {
        println!("cargo:warning={line}");
    }
    panic!("ggml not found -- see the instructions above");
}

/// The archive for `name`, under either spelling ggml's build might have used.
///
/// **cmake names static archives differently by platform, and Chaos only knew
/// one of them.** MinGW on Windows emits `ggml-base.a`; GCC on Linux emits
/// `libggml-base.a`. Every check and copy here looked for `{name}.a` only, so a
/// user following the README on Linux got
///
/// ```text
/// GGML_LIB_DIR is /path/to/build/ggml/src, but it does not contain:
///   ggml-base.a, ggml-cpu.a, ggml.a
/// ```
///
/// naming three files the instructions in that very message cannot produce.
/// **Building Chaos on Linux by following the README was impossible**, which is
/// the likeliest reason `CONTRIBUTING.md` could say no model had ever been run
/// there. Found 2026-09-01 by doing it, in a Debian container.
///
/// CI never hit this because it stages the archives itself, stripping the prefix
/// with `sed 's/^lib//'` before setting `GGML_LIB_DIR` -- so the workaround that
/// kept CI green is exactly what kept the bug invisible.
fn archive(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let plain = dir.join(format!("{name}.a"));
    if plain.exists() {
        return Some(plain);
    }
    let prefixed = dir.join(format!("lib{name}.a"));
    prefixed.exists().then_some(prefixed)
}
