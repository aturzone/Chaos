//! Pointing Claude Code at this node: the settings, in one place.
//!
//! # Why this is a module rather than a `format!` in the button handler
//!
//! It was a `format!` in the button handler. **The same five settings exist in
//! four places** — the USE WITH CLAUDE CODE button, `scripts/claude-chaos.cmd`,
//! `scripts/claude-chaos.sh`, and `docs/CLAUDE-CODE.md` — and nothing tied them
//! together. Atur asked for the setup to work *"without any problem"* and to be
//! tested for sure; a button that hands Claude Code a different tool set than
//! the documented wrapper is exactly the kind of problem that shows up as "it
//! works from the terminal but not from the app".
//!
//! The button cannot be exercised by `scripts/run-through.ps1` — it opens a
//! modal folder dialog, which stops the message loop, so the run-through lists
//! it rather than pressing it. That makes the part *around* the modal worth
//! extracting: what is decided, and what command is built. Those are pure
//! functions, they are tested here, and they are checked against the shipped
//! wrapper so the two cannot drift.
//!
//! # The two settings that decide whether it works at all
//!
//! **`--tools`.** Claude Code's default twenty-eight tool definitions are
//! 40,255 tokens before the user types anything, against a 32,768-token context
//! on every model that runs here. Six brings it to 11,706. Measured:
//! `research/claude-code-against-a-chaos-node-2026-09-07.md`.
//!
//! **`ANTHROPIC_MODEL`.** A name from Claude Code's own catalogue, not the local
//! model's. The name is validated **client-side before any request is sent**, so
//! an unknown one gives `[claude-code:unrecognized_model]` and exit 0 with the
//! node never seeing a byte. The node ignores the name and answers with
//! whatever it has loaded.

/// The tools Claude Code is given. Six, not twenty-eight — see the module note.
pub const TOOLS: &str = "Read,Write,Edit,Bash,Glob,Grep";

/// The context to tell Claude Code about.
///
/// Without this it assumes 200k and compacts against a number the node cannot
/// hold. It is not the node's `-c`: this is what the *client* believes.
pub const CONTEXT: u32 = 16_384;

/// A model name from Claude Code's catalogue, because the name is validated
/// before a request is sent.
pub const MODEL: &str = "claude-opus-5";

/// Claude Code's own config directory, kept apart from a normal `claude`.
///
/// Pointing Claude Code at a local model must not disturb the history,
/// settings or authentication of the real one.
pub const CONFIG_DIR: &str = "%USERPROFILE%\\.claude-chaos";

/// The `cmd /k` script the button runs, for a node on `port`.
///
/// `/k` rather than `/c` so the window survives the turn: the answer stays
/// readable, and an error the user never sees is the same as no error at all.
/// The two echoed lines are there because the first minutes look like a hang —
/// one turn is minutes of prefill on a CPU machine.
pub fn terminal_script(port: u16) -> String {
    format!(
        "set CLAUDE_CONFIG_DIR={CONFIG_DIR} && \
         set ANTHROPIC_BASE_URL=http://127.0.0.1:{port} && \
         set ANTHROPIC_API_KEY=chaos && \
         set ANTHROPIC_MODEL={MODEL} && \
         set CLAUDE_CODE_MAX_CONTEXT_TOKENS={CONTEXT} && \
         echo Claude Code is talking to Chaos on port {port}. && \
         echo A turn takes minutes on a CPU machine. That is the model, not a hang. && \
         echo. && \
         claude --tools {TOOLS}"
    )
}

/// The command that installs Claude Code, shown to the user before it runs.
pub const INSTALL: &str = "npm install -g @anthropic-ai/claude-code";

/// Why the button will not proceed, or `None` if it will.
///
/// **A node with no model is the confusing failure**, not a missing one: the
/// button hands Claude Code an address, and an address with nothing behind it
/// makes Claude Code look broken. Kept as a pure function so the refusal and
/// its wording are testable without a window.
pub fn refusal(installed: bool, model_loaded: bool) -> Option<&'static str> {
    if !installed {
        return Some("Claude Code is not installed");
    }
    if !model_loaded {
        return Some("load a model first -- Claude Code needs a running node");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variable Claude Code reads has to be in the script. A missing
    /// `ANTHROPIC_BASE_URL` sends the turn to Anthropic's API on the user's own
    /// account, which is the one failure here that costs money.
    #[test]
    fn the_script_sets_everything_claude_code_reads() {
        let s = terminal_script(8231);
        for want in [
            "CLAUDE_CONFIG_DIR=",
            "ANTHROPIC_BASE_URL=http://127.0.0.1:8231",
            "ANTHROPIC_API_KEY=",
            "ANTHROPIC_MODEL=claude-opus-5",
            "CLAUDE_CODE_MAX_CONTEXT_TOKENS=16384",
            "claude --tools Read,Write,Edit,Bash,Glob,Grep",
        ] {
            assert!(s.contains(want), "the script does not set {want}: {s}");
        }
        // The port is the node's, wherever it moved to.
        assert!(terminal_script(9999).contains("127.0.0.1:9999"));
        // And it is said in words too, because the wait looks like a hang.
        assert!(s.contains("not a hang"));
    }

    /// **The button and the shipped wrapper must agree.** They are two doors to
    /// one thing, and a user who is told in the docs that six tools are used,
    /// then gets twenty-eight from the button, meets a context overflow with no
    /// explanation. Read from the file rather than restated here, so editing
    /// one without the other fails.
    #[test]
    fn the_button_and_the_wrapper_agree() {
        let cmd = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/claude-chaos.cmd"
        ))
        .expect("scripts/claude-chaos.cmd is missing");
        let sh = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/claude-chaos.sh"
        ))
        .expect("scripts/claude-chaos.sh is missing");

        for (name, src) in [("claude-chaos.cmd", &cmd), ("claude-chaos.sh", &sh)] {
            assert!(
                src.contains(TOOLS),
                "{name} does not use the same tool set as the button ({TOOLS})"
            );
            assert!(
                src.contains(&CONTEXT.to_string()),
                "{name} does not use the same context as the button ({CONTEXT})"
            );
            assert!(
                src.contains(MODEL),
                "{name} does not use the same model name as the button ({MODEL})"
            );
            assert!(
                src.contains(".claude-chaos"),
                "{name} does not keep its own config directory"
            );
        }
    }

    /// And the document a person is pointed at has to say the same numbers.
    #[test]
    fn the_documentation_says_the_same_thing() {
        let doc = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/CLAUDE-CODE.md"
        ))
        .expect("docs/CLAUDE-CODE.md is missing");
        assert!(doc.contains(TOOLS), "the doc names a different tool set");
        assert!(
            doc.contains(&CONTEXT.to_string()),
            "the doc names a different context"
        );
        assert!(
            doc.contains("/v1/messages"),
            "the doc does not name the endpoint that makes this work"
        );
    }

    /// The order of the two refusals matters: not-installed is checked first,
    /// because "load a model" is unhelpful advice to someone who has no client.
    #[test]
    fn it_refuses_for_the_first_reason_that_applies() {
        assert_eq!(refusal(false, false), Some("Claude Code is not installed"));
        assert_eq!(refusal(false, true), Some("Claude Code is not installed"));
        assert_eq!(
            refusal(true, false),
            Some("load a model first -- Claude Code needs a running node")
        );
        assert_eq!(refusal(true, true), None);
    }

    /// The install line is the one the user is shown and the one that runs.
    #[test]
    fn the_install_command_is_the_real_one() {
        assert_eq!(INSTALL, "npm install -g @anthropic-ai/claude-code");
    }
}
