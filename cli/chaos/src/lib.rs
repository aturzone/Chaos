//! `chaos` — the one command a person types, and the parts of it worth testing.
//!
//! **Why a front door.** Seventeen binaries ship, and Atur's plan names the
//! consequence: *"A person who installs this types `chaos`. **Decide** whether
//! `chaos <subcommand>` becomes the front door with the rest as internal
//! binaries, and if so do it as one deliberate change with the old names kept
//! working."*
//!
//! **Decided: yes, and every old name keeps working.** `chaos run` runs
//! `chaos-run`, with its arguments passed through untouched — so every script,
//! every line of documentation, the installer's file list and
//! `chaos_model::release::asset_for_platform` all keep meaning what they meant.
//! Nothing is renamed and nothing is hidden; a name is *added*.
//!
//! Four subcommands are not a pass-through, because they did not exist:
//! `start`, `stop` and `status` manage a node as a background process, which is
//! what somebody over SSH needs and what `chaos-serve` — foreground, one
//! terminal — could never be; and `connect` talks to another node's endpoint.
//!
//! **The settings file is the shared one.** `chaos start` builds the server's
//! flags with `chaos_config::Settings::serve_args`, the same function the window
//! calls, so the port, key, cache, threads and context a person set in the app
//! are the ones a node started from a terminal uses. That was the other half of
//! the plan's complaint: *"the app has a settings file the CLI cannot read"*.

use std::path::{Path, PathBuf};

/// A subcommand that is just another binary under a friendlier name.
pub struct Alias {
    /// What the person types after `chaos`.
    pub verb: &'static str,
    /// The binary it becomes, without any platform extension.
    pub binary: &'static str,
    /// One line, for the help.
    pub blurb: &'static str,
}

/// Every pass-through, in the order the help lists them.
///
/// **Ordered by what a new user needs first**, not alphabetically: fetch a model,
/// run it, look at the machine. The help is the only documentation most people
/// will read.
pub const ALIASES: &[Alias] = &[
    Alias {
        verb: "run",
        binary: "chaos-run",
        blurb: "run a model on a prompt, here in this terminal",
    },
    Alias {
        verb: "serve",
        binary: "chaos-serve",
        blurb: "serve the OpenAI API in the foreground (see `start` for a node)",
    },
    Alias {
        verb: "pull",
        binary: "chaos-pull",
        blurb: "fetch a model, or list what can be fetched",
    },
    Alias {
        verb: "draw",
        binary: "chaos-draw",
        blurb: "draw a picture from a prompt",
    },
    Alias {
        verb: "probe",
        binary: "chaos-probe",
        blurb: "what this machine has, and what to close before a big run",
    },
    Alias {
        verb: "fit",
        binary: "chaos-model-info",
        blurb: "whether a container fits here, and how fast it will be",
    },
    Alias {
        verb: "meta",
        binary: "chaos-meta",
        blurb: "what a container says about itself",
    },
    Alias {
        verb: "qr",
        binary: "chaos-qr",
        blurb: "draw a route as a scannable code in this terminal",
    },
    Alias {
        verb: "worker",
        binary: "chaos-worker",
        blurb: "hold experts in this machine's memory for a CORE to use",
    },
];

/// The subcommands `chaos` implements itself.
pub const OWN: &[(&str, &str)] = &[
    (
        "start",
        "start a node in the background, with a log and a pid file",
    ),
    ("stop", "stop the node this machine started"),
    (
        "status",
        "what the node is doing -- locally and over HTTP, no curl",
    ),
    (
        "connect",
        "talk to another machine's node: chaos connect <route> \"prompt\"",
    ),
    // **The front door had no way to update, and it is the documented door.**
    // `chaos-run --update` has done the whole job since v0.0.13; a second
    // updater in this crate would be a second one to keep correct.
    (
        "update",
        "check for a newer Chaos and install it -- no manual download",
    ),
    (
        "config",
        "the settings every tier reads, and where they come from",
    ),
    (
        "completions",
        "shell completions: bash, zsh, fish or powershell",
    ),
    (
        "verify",
        "hash a model and check it is the file it was when it arrived",
    ),
];

// **`chaos scan` was removed in v0.0.34, with the reader it pointed at.**
//
// It existed to answer one question -- "can this read a code?" -- by
// refusing and naming the two readers that did work: the phone app's SCAN
// button and `/scan` in a browser. Atur deleted both (*"that book and
// barcode aren't needed any more, we only use the book"*), so the command
// spent this release directing people to two features that no longer
// exist -- **and a test asserted that it did**, checking the message named
// `/scan`. A refusal that points at a ghost is worse than no command: it
// is wrong rather than merely absent, and it had a check keeping it wrong.
//
// `chaos qr` still draws a code. Nothing here reads one, and `chaos` now
// says `scan` is not a command, which is true.

pub fn alias_for(verb: &str) -> Option<&'static Alias> {
    ALIASES.iter().find(|a| a.verb == verb)
}

/// Where the state a node leaves behind lives.
///
/// **Beside the models and the settings, not in the install directory**, for the
/// same reason the settings file is: an upgrade replaces the install and must not
/// take a running node's pid file with it.
pub fn state_dir() -> PathBuf {
    chaos_config::path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn pid_path() -> PathBuf {
    state_dir().join("node.pid")
}

pub fn log_path() -> PathBuf {
    state_dir().join("node.log")
}

/// What a pid file says, if it says anything usable.
///
/// **A pid file outlives the process it names.** A machine that lost power leaves
/// one behind, and the pid in it may by then belong to something else entirely --
/// so reading one is always "there was a node, and here is whether it is still
/// there", never "a node is running".
pub fn read_pid(text: &str) -> Option<u32> {
    text.trim().lines().next()?.trim().parse().ok()
}

/// The completion script for a shell.
///
/// Generated rather than hand-written per shell, so a subcommand added above
/// appears in all four without anybody remembering to.
pub fn completions(shell: &str) -> Result<String, String> {
    let verbs: Vec<&str> = ALIASES
        .iter()
        .map(|a| a.verb)
        .chain(OWN.iter().map(|(v, _)| *v))
        .chain(["help"])
        .collect();
    let list = verbs.join(" ");
    Ok(match shell {
        "bash" => format!(
            "# chaos completions for bash. Add to ~/.bashrc:\n\
             #   source <(chaos completions bash)\n\
             _chaos() {{\n\
             \x20 local cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n\
             \x20 if [ \"$COMP_CWORD\" -eq 1 ]; then\n\
             \x20   COMPREPLY=($(compgen -W \"{list}\" -- \"$cur\"))\n\
             \x20 else\n\
             \x20   COMPREPLY=($(compgen -f -- \"$cur\"))\n\
             \x20 fi\n\
             }}\n\
             complete -F _chaos chaos\n"
        ),
        "zsh" => format!(
            "# chaos completions for zsh. Add to ~/.zshrc:\n\
             #   source <(chaos completions zsh)\n\
             _chaos() {{\n\
             \x20 if (( CURRENT == 2 )); then\n\
             \x20   compadd {list}\n\
             \x20 else\n\
             \x20   _files\n\
             \x20 fi\n\
             }}\n\
             compdef _chaos chaos\n"
        ),
        "fish" => {
            let mut out = String::from("# chaos completions for fish. Save as:\n#   ~/.config/fish/completions/chaos.fish\n");
            for a in ALIASES {
                out.push_str(&format!(
                    "complete -c chaos -n __fish_use_subcommand -a {} -d '{}'\n",
                    a.verb,
                    a.blurb.replace('\'', "")
                ));
            }
            for (v, d) in OWN {
                out.push_str(&format!(
                    "complete -c chaos -n __fish_use_subcommand -a {v} -d '{}'\n",
                    d.replace('\'', "")
                ));
            }
            out
        }
        "powershell" => format!(
            "# chaos completions for PowerShell. Add to $PROFILE:\n\
             #   chaos completions powershell | Out-String | Invoke-Expression\n\
             Register-ArgumentCompleter -Native -CommandName chaos -ScriptBlock {{\n\
             \x20 param($wordToComplete, $commandAst, $cursorPosition)\n\
             \x20 @({}) | Where-Object {{ $_ -like \"$wordToComplete*\" }} |\n\
             \x20   ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_) }}\n\
             }}\n",
            verbs
                .iter()
                .map(|v| format!("'{v}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => {
            return Err(format!(
                "no completions for {other:?}. Try bash, zsh, fish or powershell."
            ))
        }
    })
}

/// Pull one field out of a flat JSON object, as text.
///
/// **The workspace's own parser, not a hand-rolled scan.** `chaos_grammar::Json`
/// exists and is tested; a second JSON reader in the tree would be one too many.
pub fn field(json: &str, key: &str) -> Option<String> {
    let parsed = chaos_grammar::Json::parse(json).ok()?;
    let chaos_grammar::Json::Obj(entries) = parsed else {
        return None;
    };
    let (_, v) = entries.iter().find(|(k, _)| k == key)?;
    Some(match v {
        chaos_grammar::Json::Str(s) => s.clone(),
        chaos_grammar::Json::Num(n) => {
            if (n.fract()).abs() < f64::EPSILON {
                format!("{n:.0}")
            } else {
                format!("{n:.3}")
            }
        }
        chaos_grammar::Json::Bool(b) => b.to_string(),
        chaos_grammar::Json::Null => "null".into(),
        _ => return None,
    })
}

/// The JSON body for one non-streaming or streaming chat turn.
///
/// Hand-built rather than serialised, because the whole body is three fields and
/// the workspace has no serialisation crate.
pub fn chat_body(model: &str, prompt: &str, stream: bool) -> String {
    chat_body_with(model, prompt, stream, None)
}

/// A chat request, optionally raising the token cap.
///
/// **Without this the node's default silently truncated every long answer.** A
/// request that asks for a paragraph got one that stopped mid-word, with no
/// explanation in the output and no flag anywhere to ask for more -- the answer
/// simply ended, and it read as the model having nothing further to say.
pub fn chat_body_with(model: &str, prompt: &str, stream: bool, max_tokens: Option<u32>) -> String {
    let cap = match max_tokens {
        Some(n) => format!(r#","max_tokens":{n}"#),
        None => String::new(),
    };
    format!(
        r#"{{"model":"{}","stream":{}{}, "messages":[{{"role":"user","content":"{}"}}]}}"#,
        escape(model),
        stream,
        cap,
        escape(prompt)
    )
}

/// Why generation stopped, from a streamed chunk.
///
/// **The node computes this and tests it, and no client ever showed it.** 4e
/// found an answer that stopped at "Sevent" because the default cap expired
/// there: nothing said so, so a truncated answer and a finished one look
/// identical. `"stop"` is the model finishing; `"length"` is the cap, and only
/// one of those is worth telling somebody about.
pub fn finish_reason(chunk: &str) -> Option<String> {
    let chaos_grammar::Json::Obj(top) = chaos_grammar::Json::parse(chunk).ok()? else {
        return None;
    };
    let (_, choices) = top.iter().find(|(k, _)| k == "choices")?;
    let chaos_grammar::Json::Arr(items) = choices else {
        return None;
    };
    let chaos_grammar::Json::Obj(first) = items.first()? else {
        return None;
    };
    let (_, reason) = first.iter().find(|(k, _)| k == "finish_reason")?;
    match reason {
        chaos_grammar::Json::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// Escape a string for a JSON body.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The `delta.content` of one streamed chunk, if it carries any.
///
/// **A chunk with no content is normal**, not an error: the first carries a role
/// and the last carries a finish reason.
pub fn delta_text(chunk: &str) -> Option<String> {
    let chaos_grammar::Json::Obj(top) = chaos_grammar::Json::parse(chunk).ok()? else {
        return None;
    };
    let (_, choices) = top.iter().find(|(k, _)| k == "choices")?;
    let chaos_grammar::Json::Arr(items) = choices else {
        return None;
    };
    let chaos_grammar::Json::Obj(first) = items.first()? else {
        return None;
    };
    let (_, delta) = first.iter().find(|(k, _)| k == "delta")?;
    let chaos_grammar::Json::Obj(fields) = delta else {
        return None;
    };
    let (_, content) = fields.iter().find(|(k, _)| k == "content")?;
    match content {
        chaos_grammar::Json::Str(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every old binary name is still reachable**, which is the promise the
    /// front door was allowed to exist on.
    #[test]
    fn the_front_door_adds_names_and_removes_none() {
        for binary in [
            "chaos-run",
            "chaos-serve",
            "chaos-pull",
            "chaos-draw",
            "chaos-probe",
            "chaos-model-info",
            "chaos-meta",
            "chaos-qr",
            "chaos-worker",
        ] {
            assert!(
                ALIASES.iter().any(|a| a.binary == binary),
                "{binary} has no verb, so `chaos` cannot reach it"
            );
        }
    }

    /// A verb that means two things, or a blurb that says nothing, is a help
    /// page nobody can use.
    #[test]
    fn every_verb_is_unique_and_says_what_it_does() {
        let mut seen = std::collections::HashSet::new();
        for verb in ALIASES
            .iter()
            .map(|a| a.verb)
            .chain(OWN.iter().map(|(v, _)| *v))
        {
            assert!(seen.insert(verb), "{verb} is defined twice");
            assert!(!verb.is_empty());
            assert!(!verb.starts_with('-'), "{verb} looks like a flag");
        }
        for a in ALIASES {
            assert!(a.blurb.len() > 12, "{}'s blurb says nothing", a.verb);
            assert!(
                !a.blurb.ends_with('.'),
                "{}'s blurb ends in a full stop; the others do not",
                a.verb
            );
        }
        assert!(alias_for("run").is_some());
        assert!(alias_for("start").is_none(), "start is not a pass-through");
        assert!(alias_for("wharrgarbl").is_none());
    }

    /// **A pid file outlives its process**, so parsing one must never panic and
    /// must reject what is not a pid.
    #[test]
    fn a_pid_file_is_read_defensively() {
        assert_eq!(read_pid("1234"), Some(1234));
        assert_eq!(read_pid("  1234  \n"), Some(1234));
        assert_eq!(read_pid("1234\nstale junk"), Some(1234));
        for bad in ["", "   ", "not a pid", "-1", "12.5", "99999999999999999999"] {
            assert_eq!(read_pid(bad), None, "{bad:?} was read as a pid");
        }
    }

    #[test]
    fn the_state_files_sit_beside_the_settings() {
        let dir = state_dir();
        assert_eq!(pid_path().parent(), Some(dir.as_path()));
        assert_eq!(log_path().parent(), Some(dir.as_path()));
        assert_eq!(
            chaos_config::path().parent(),
            Some(dir.as_path()),
            "the pid file no longer lives beside settings.txt"
        );
    }

    /// The real `/status` shape, read with the workspace's own parser.
    #[test]
    fn status_fields_are_read_from_the_real_shape() {
        let json = r#"{"status":"ok","model":"qwen3-4b","context_limit":4096,
            "context_ceiling":32768,"route":"http://192.168.1.20:8080","reachable":true,
            "uptime_seconds":42,"verified_architectures":7,
            "last_generation":{"tokens":16,"tokens_per_second":1.512}}"#;
        assert_eq!(field(json, "model").as_deref(), Some("qwen3-4b"));
        assert_eq!(field(json, "status").as_deref(), Some("ok"));
        assert_eq!(field(json, "reachable").as_deref(), Some("true"));
        assert_eq!(field(json, "uptime_seconds").as_deref(), Some("42"));
        assert_eq!(field(json, "context_limit").as_deref(), Some("4096"));
        assert_eq!(
            field(json, "route").as_deref(),
            Some("http://192.168.1.20:8080")
        );
        // A nested object is not a scalar and is not pretended to be one.
        assert_eq!(field(json, "last_generation"), None);
        assert_eq!(field(json, "absent"), None);
        assert_eq!(field("not json at all", "model"), None);
    }

    /// A prompt with a quote in it must not produce a body the node rejects.
    #[test]
    fn a_prompt_is_escaped_into_valid_json() {
        let body = chat_body("qwen3-4b", "say \"hello\"\nand a tab\there", true);
        assert!(chaos_grammar::Json::parse(&body).is_ok(), "{body}");
        assert!(body.contains(r#"\"hello\""#), "{body}");
        assert!(body.contains(r"\n") && body.contains(r"\t"), "{body}");
        assert!(body.contains(r#""stream":true"#));

        let plain = chat_body("m", "hi", false);
        assert!(plain.contains(r#""stream":false"#));
        assert!(chaos_grammar::Json::parse(&plain).is_ok());

        // A backslash is the one that breaks a naive escaper twice over.
        let tricky = chat_body("m", r"C:\Projects\models", true);
        assert!(chaos_grammar::Json::parse(&tricky).is_ok(), "{tricky}");
    }

    /// The chunk shape `chaos-serve` actually streams.
    #[test]
    fn a_streamed_chunk_yields_its_text_and_silence_is_not_an_error() {
        let with = r#"{"choices":[{"delta":{"content":"Hel"},"index":0}]}"#;
        assert_eq!(delta_text(with).as_deref(), Some("Hel"));
        // The first chunk carries a role, the last a finish reason: neither has
        // content, and neither is a failure.
        assert_eq!(
            delta_text(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#),
            None
        );
        assert_eq!(
            delta_text(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#),
            None
        );
        assert_eq!(delta_text("[DONE]"), None);
        assert_eq!(delta_text(""), None);
    }

    /// **No command may point at something that is not there.**
    ///
    /// `chaos scan` refused to decode and named two readers that did work --
    /// the phone app's SCAN button and `/scan` in a browser. Both were deleted
    /// in v0.0.34, and the command went on naming them, with a test asserting
    /// the message contained "/scan". The check kept it wrong. This is the
    /// same check, inverted: nothing this binary prints may name a reader, and
    /// `scan` is no longer a command it claims to have.
    #[test]
    fn nothing_points_at_the_reader_that_was_deleted() {
        assert!(
            !OWN.iter().any(|(v, _)| *v == "scan"),
            "`chaos scan` is listed again; the reader it named is gone"
        );
        assert!(alias_for("scan").is_none(), "scan is not a pass-through");
        let listed: String = OWN
            .iter()
            .map(|(v, d)| {
                format!(
                    "{v} {d}
"
                )
            })
            .chain(ALIASES.iter().map(|a| {
                format!(
                    "{} {}
",
                    a.verb, a.blurb
                )
            }))
            .collect();
        assert!(!listed.contains("/scan"), "the listing names the reader");
        assert!(
            !listed.to_lowercase().contains("read a qr"),
            "the listing still offers to read a code: {listed}"
        );
        // And `chaos qr`, which draws one, is still there -- the encoder was
        // never the part that went.
        assert!(
            OWN.iter().any(|(v, _)| *v == "qr") || alias_for("qr").is_some(),
            "the encoder went with the reader"
        );
    }

    /// **The cap reaches the wire, and its absence leaves the body unchanged.**
    #[test]
    fn a_token_cap_is_sent_only_when_asked_for() {
        let plain = chat_body_with("m", "hi", true, None);
        assert!(!plain.contains("max_tokens"), "{plain}");
        let capped = chat_body_with("m", "hi", true, Some(220));
        assert!(capped.contains(r#""max_tokens":220"#), "{capped}");
        // Still one well-formed object, not two fields glued together.
        assert!(chaos_grammar::Json::parse(&capped).is_ok(), "{capped}");
        assert!(chaos_grammar::Json::parse(&plain).is_ok(), "{plain}");
        // And the old entry point still means what it meant.
        assert_eq!(chat_body("m", "hi", true), plain);
    }

    /// **Why generation stopped is read from the shape the node really sends.**
    ///
    /// The node computes this and tests it, and no client ever showed it: 4e
    /// found an answer that stopped at "Sevent" with nothing saying the cap had
    /// expired, so a truncated answer and a finished one looked identical.
    #[test]
    fn the_finish_reason_is_read_and_a_running_chunk_has_none() {
        let running = r#"{"choices":[{"delta":{"content":" Paris"},"finish_reason":null}]}"#;
        assert_eq!(finish_reason(running), None, "a null is not a reason");
        assert_eq!(delta_text(running).as_deref(), Some(" Paris"));

        let capped = r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#;
        assert_eq!(finish_reason(capped).as_deref(), Some("length"));

        let done = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        assert_eq!(finish_reason(done).as_deref(), Some("stop"));

        // Nothing at all, and rubbish, are both "no reason" rather than a panic.
        assert_eq!(finish_reason("[DONE]"), None);
        assert_eq!(finish_reason("{"), None);
        assert_eq!(finish_reason(r#"{"choices":[]}"#), None);
    }

    /// All four shells, and a refusal that names the four.
    #[test]
    fn completions_exist_for_every_shell_and_name_every_verb() {
        for shell in ["bash", "zsh", "fish", "powershell"] {
            let s = completions(shell).unwrap_or_else(|e| panic!("{shell}: {e}"));
            assert!(!s.is_empty());
            for verb in ALIASES.iter().map(|a| a.verb) {
                assert!(s.contains(verb), "{shell} completions omit {verb}");
            }
            for (verb, _) in OWN {
                assert!(s.contains(verb), "{shell} completions omit {verb}");
            }
        }
        let e = completions("csh").unwrap_err();
        for named in ["bash", "zsh", "fish", "powershell"] {
            assert!(e.contains(named), "the refusal does not name {named}: {e}");
        }
    }
}
