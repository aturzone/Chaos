//! `chaos` — the front door. See `lib.rs` for why it exists and what it promises.

use chaos_cli::{
    alias_for, chat_body, completions, delta_text, field, log_path, pid_path, read_pid, ALIASES,
    OWN,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Ask the operating system about a process, without a dependency.
///
/// **Raw `extern` declarations, like the rest of this workspace.** The window is
/// 6,000 lines of them; a crate to ask whether a pid is alive would be the
/// project's first dependency and it would be for two function calls.
mod os {
    #[cfg(windows)]
    mod imp {
        type Handle = *mut core::ffi::c_void;
        const PROCESS_TERMINATE: u32 = 0x0001;
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
            fn TerminateProcess(handle: Handle, code: u32) -> i32;
            fn GetExitCodeProcess(handle: Handle, code: *mut u32) -> i32;
            fn CloseHandle(handle: Handle) -> i32;
        }

        /// **`OpenProcess` succeeding is not "it is running".** A process object
        /// outlives the process itself while any handle to it is open, so a
        /// still-open handle elsewhere makes an exited process openable. The
        /// exit code is the fact: `STILL_ACTIVE` means running, anything else
        /// means it has gone.
        pub fn alive(pid: u32) -> bool {
            const STILL_ACTIVE: u32 = 259;
            unsafe {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if h.is_null() {
                    return false;
                }
                let mut code: u32 = 0;
                let ok = GetExitCodeProcess(h, &mut code) != 0;
                CloseHandle(h);
                ok && code == STILL_ACTIVE
            }
        }

        pub fn terminate(pid: u32) -> bool {
            unsafe {
                let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
                if h.is_null() {
                    return false;
                }
                let ok = TerminateProcess(h, 0) != 0;
                CloseHandle(h);
                ok
            }
        }
    }

    #[cfg(unix)]
    mod imp {
        extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
        }

        /// Signal 0 asks "could I signal this?" and sends nothing.
        pub fn alive(pid: u32) -> bool {
            unsafe { kill(pid as i32, 0) == 0 }
        }

        /// `SIGTERM`, so the node can close its socket rather than being shot.
        pub fn terminate(pid: u32) -> bool {
            unsafe { kill(pid as i32, 15) == 0 }
        }
    }

    #[cfg(not(any(windows, unix)))]
    mod imp {
        pub fn alive(_pid: u32) -> bool {
            false
        }
        pub fn terminate(_pid: u32) -> bool {
            false
        }
    }

    pub use imp::{alive, terminate};
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verb = args.first().map(String::as_str).unwrap_or("");
    let rest = if args.is_empty() { &[] } else { &args[1..] };

    let code = match verb {
        "" | "help" | "-h" | "--help" => {
            help();
            0
        }
        "-V" | "--version" | "version" => {
            println!("chaos {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "start" => start(rest),
        "stop" => stop(),
        "status" => status(rest),
        "connect" => connect(rest),
        "config" => config(),
        "scan" => {
            // Not a failure of this invocation -- it is a feature that does not
            // exist, and the message is the deliverable.
            eprint!(
                "{}",
                chaos_cli::scan_verdict(rest.first().map(String::as_str))
            );
            2
        }
        "completions" => match rest.first().map(String::as_str) {
            Some(shell) => match completions(shell) {
                Ok(s) => {
                    print!("{s}");
                    0
                }
                Err(e) => {
                    eprintln!("chaos: {e}");
                    2
                }
            },
            None => {
                eprintln!("chaos completions <bash|zsh|fish|powershell>");
                2
            }
        },
        other => match alias_for(other) {
            Some(a) => pass_through(a.binary, rest),
            None => {
                eprintln!("chaos: {other:?} is not a command. `chaos` on its own lists them.");
                2
            }
        },
    };
    std::process::exit(code);
}

fn help() {
    println!(
        "chaos {} -- run models larger than memory",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("  chaos <command> [arguments]");
    println!();
    println!("A model, a prompt, a terminal:");
    for a in ALIASES {
        println!("  {:<12} {}", a.verb, a.blurb);
    }
    println!();
    println!("A node other machines can use:");
    for (verb, blurb) in OWN {
        println!("  {verb:<12} {blurb}");
    }
    println!();
    println!("Every command above is also a binary of its own: `chaos run` and");
    println!("`chaos-run` are the same program with the same arguments, so nothing");
    println!("written against the old names has to change.");
    println!();
    println!("  chaos pull qwen3-4b          a good first model, about 2.5 GB");
    println!("  chaos run qwen3-4b \"hello\"   answer a prompt and exit");
    println!("  chaos start qwen3-4b         a node in the background");
    println!("  chaos status                 what it is doing");
    println!("  chaos connect 192.168.1.20:8080 \"hello\"   use another machine's");
}

/// Where the sibling binaries are: beside this one.
///
/// **Not `PATH`.** An installed Chaos puts every binary in one directory, and
/// resolving a sibling by path means `chaos run` cannot pick up a different
/// `chaos-run` that happens to be earlier on `PATH` — which on a developer's
/// machine, with a release build and a debug build, is a real way to measure the
/// wrong binary.
fn sibling(binary: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("chaos"));
    let dir = exe.parent().map(PathBuf::from).unwrap_or_default();
    let name = if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    };
    let beside = dir.join(&name);
    if beside.exists() {
        beside
    } else {
        // Fall back to `PATH` rather than failing: `cargo run -p chaos-cli` has
        // no siblings, and neither does a half-finished install.
        PathBuf::from(name)
    }
}

fn pass_through(binary: &str, args: &[String]) -> i32 {
    let path = sibling(binary);
    match Command::new(&path).args(args).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("chaos: cannot run {}: {e}", path.display());
            eprintln!("       it should sit beside `chaos` in the same directory.");
            127
        }
    }
}

/// The node this machine started, if it is still running.
fn running_node() -> Option<u32> {
    let text = std::fs::read_to_string(pid_path()).ok()?;
    let pid = read_pid(&text)?;
    os::alive(pid).then_some(pid)
}

fn start(args: &[String]) -> i32 {
    if let Some(pid) = running_node() {
        eprintln!("chaos: a node is already running (pid {pid}).");
        eprintln!("       `chaos status` says what it is doing, `chaos stop` ends it.");
        return 1;
    }
    let cfg = chaos_config::Settings::load();
    // **The model is the one argument this cannot guess.** Everything else comes
    // from the settings file, which is the same file the window writes.
    let Some(model) = args.first() else {
        eprintln!("chaos start <model> [extra flags for chaos-serve]");
        eprintln!();
        eprintln!("The port, key, cache, threads and context come from");
        eprintln!("  {}", chaos_config::path().display());
        eprintln!("which is the file the app writes, so both agree. `chaos config`");
        eprintln!("prints what they currently are.");
        return 2;
    };

    let dir = chaos_cli::state_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("chaos: cannot create {}: {e}", dir.display());
        return 1;
    }
    let log = match std::fs::File::create(log_path()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("chaos: cannot write {}: {e}", log_path().display());
            return 1;
        }
    };
    let errlog = match log.try_clone() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("chaos: cannot share the log handle: {e}");
            return 1;
        }
    };

    // The window's own flags, from the shared settings, plus anything extra the
    // person typed after the model.
    let mut argv = cfg.serve_args(model);
    argv.extend(args[1..].iter().cloned());

    let path = sibling("chaos-serve");
    let mut cmd = Command::new(&path);
    cmd.args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errlog));

    // **Detached, or closing the terminal takes the node with it** -- which is
    // the entire point of `start` over `serve` for somebody on SSH.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("chaos: cannot start {}: {e}", path.display());
            return 127;
        }
    };
    let pid = child.id();
    if let Err(e) = std::fs::write(pid_path(), format!("{pid}\n")) {
        eprintln!("chaos: the node started (pid {pid}) but the pid file did not: {e}");
        eprintln!("       `chaos stop` will not find it.");
        return 1;
    }

    // **A node that dies at once must not be reported as started.** The first
    // run of this command printed "node starting, pid 852" over a server that
    // had already exited because it could not find the model -- leaving a pid
    // file, no node, and nothing on screen pointing at the log that said so.
    // Half a second is enough to catch "wrong model name" and "port in use",
    // which are the two failures that happen immediately, without waiting on a
    // load that legitimately takes minutes.
    // **`try_wait`, not `os::alive`.** On Windows a process object outlives the
    // process while any handle to it is open, and this parent is holding one --
    // so `OpenProcess` succeeds on something that has already exited, and the
    // first version of this check passed over a node that died instantly. std
    // knows the difference because it owns the handle.
    std::thread::sleep(Duration::from_millis(600));
    let exited = matches!(child.try_wait(), Ok(Some(_)));
    if exited {
        let _ = std::fs::remove_file(pid_path());
        let how = match child.try_wait() {
            Ok(Some(st)) => match st.code() {
                Some(c) => format!(" (exit code {c})"),
                None => String::new(),
            },
            _ => String::new(),
        };
        eprintln!("chaos: the node exited immediately{how}. Its last words:");
        eprintln!();
        for line in tail(&log_path(), 12) {
            eprintln!("  {line}");
        }
        eprintln!();
        eprintln!("Full log: {}", log_path().display());
        return 1;
    }

    println!("node starting, pid {pid}");
    println!("  model    {model}");
    println!("  address  http://{}:{}", cfg.role.host(), cfg.port);
    println!("  log      {}", log_path().display());
    println!();
    // **Loading is minutes for a large model**, so this does not claim readiness.
    // It says where to look, and `chaos status` answers when asked.
    println!("Loading can take minutes. `chaos status` asks the node itself.");
    0
}

/// The last `n` non-empty lines of a file, for reporting why something died.
fn tail(path: &std::path::Path, n: usize) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return vec![format!("(cannot read {})", path.display())];
    };
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(n))
        .map(|l| l.trim_end().to_string())
        .collect()
}

fn stop() -> i32 {
    let path = pid_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!(
            "chaos: no node was started from this machine ({} is absent).",
            path.display()
        );
        return 1;
    };
    let Some(pid) = read_pid(&text) else {
        eprintln!(
            "chaos: {} does not contain a pid. Removing it.",
            path.display()
        );
        let _ = std::fs::remove_file(&path);
        return 1;
    };
    if !os::alive(pid) {
        // **A stale pid file is the normal case after a crash or a reboot**, and
        // saying "stopped" about something that was already gone would be a lie.
        println!("no node running: pid {pid} is gone. Clearing the pid file.");
        let _ = std::fs::remove_file(&path);
        return 0;
    }
    if os::terminate(pid) {
        let _ = std::fs::remove_file(&path);
        println!("stopped pid {pid}");
        0
    } else {
        eprintln!("chaos: could not stop pid {pid}. It may belong to another user.");
        1
    }
}

fn status(args: &[String]) -> i32 {
    let cfg = chaos_config::Settings::load();
    // A route may be given, so this reports on another machine's node too.
    let route = args
        .first()
        .cloned()
        .unwrap_or_else(|| format!("127.0.0.1:{}", cfg.port));

    match running_node() {
        Some(pid) => println!("local node   pid {pid}, log {}", log_path().display()),
        None => {
            if pid_path().exists() {
                println!("local node   a pid file is present but the process is gone");
            } else {
                println!("local node   none started from this machine");
            }
        }
    }

    let url = format!("http://{}/status", route.trim_start_matches("http://"));
    // **The key goes with it now.** `/status` is gated when it arrives from off
    // the machine and a key is set, so asking a *remote* node for its status
    // needs one. A local node never needs it -- the server allows loopback -- but
    // sending it costs nothing and one code path is better than two.
    let key = cfg.core_key.clone().or_else(|| cfg.api_key.clone());
    // **No curl.** `chaos_http` is why this crate exists at all.
    match chaos_http::get_with_key(&url, key.as_deref(), Duration::from_secs(4)) {
        Ok(r) if r.status == 200 => {
            let json = r.text();
            println!("reachable    {route}");
            for (label, key) in [
                ("model", "model"),
                ("route", "route"),
                ("uptime (s)", "uptime_seconds"),
                ("context", "context_limit"),
                ("ceiling", "context_ceiling"),
                ("off loopback", "reachable"),
            ] {
                if let Some(v) = field(&json, key) {
                    println!("  {label:<12} {v}");
                }
            }
            if let Some(t) = field(&json, "tokens_per_second") {
                println!("  {:<12} {t}", "last tok/s");
            }
            0
        }
        Ok(r) => {
            eprintln!("chaos: {route} answered {} rather than 200", r.status);
            1
        }
        Err(e) => {
            println!("reachable    no -- {e}");
            // Not an error when nothing was meant to be running: a node that was
            // never started is a state, not a failure.
            if running_node().is_some() {
                println!();
                println!("The process is alive but not answering yet. If a large model is");
                println!("loading this is expected; the log says how far it has got:");
                println!("  {}", log_path().display());
                1
            } else {
                0
            }
        }
    }
}

fn connect(args: &[String]) -> i32 {
    let cfg = chaos_config::Settings::load();
    let (route, prompt) = match args {
        [] => (cfg.core_addr.clone().unwrap_or_default(), String::new()),
        [route] => (route.clone(), String::new()),
        [route, rest @ ..] => (route.clone(), rest.join(" ")),
    };
    if route.is_empty() {
        eprintln!("chaos connect <route> \"prompt\"");
        eprintln!();
        eprintln!("The route is what the app's mode page shows, e.g. 192.168.1.20:8080.");
        eprintln!(
            "With no route, the `core_addr` in {} is used.",
            chaos_config::path().display()
        );
        return 2;
    }
    if prompt.is_empty() {
        eprintln!("chaos: nothing to ask. Put the prompt after the route.");
        return 2;
    }
    let base = route.trim_start_matches("http://").trim_end_matches('/');

    // Ask what is loaded, so the body names the model the node actually has
    // rather than a guess the node would have to correct.
    let model = chaos_http::get(&format!("http://{base}/status"), Duration::from_secs(4))
        .ok()
        .filter(|r| r.status == 200)
        .and_then(|r| field(&r.text(), "model"))
        .unwrap_or_else(|| "chaos".to_string());

    // **The key belongs to the CORE, not to this machine's own server.** A
    // CLIENT stores it as `core_key`; that is the one to send.
    let key = cfg.core_key.clone().or_else(|| cfg.api_key.clone());
    let body = chat_body(&model, &prompt, true);
    let url = format!("http://{base}/v1/chat/completions");

    let mut out = std::io::stdout();
    let mut wrote = false;
    let result = chaos_http::post_sse(
        &url,
        &body,
        key.as_deref(),
        // Generous: the first token of a streamed answer can be a whole prefill
        // away, and on a streaming MoE that is seconds per token.
        Duration::from_secs(300),
        &mut |chunk| {
            if let Some(text) = delta_text(chunk) {
                print!("{text}");
                let _ = out.flush();
                wrote = true;
            }
            true
        },
    );
    if wrote {
        println!();
    }
    match result {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("chaos: {e}");
            2
        }
    }
}

fn config() -> i32 {
    let path = chaos_config::path();
    let cfg = chaos_config::Settings::load();
    println!("settings     {}", path.display());
    if !path.exists() {
        println!("             (absent -- these are the defaults)");
    }
    println!();
    println!("  {:<15} {}", "mode", cfg.role.as_str());
    println!("  {:<15} {}", "port", cfg.port);
    println!("  {:<15} {}", "binds", cfg.role.host());
    println!("  {:<15} {}", "endpoint", cfg.endpoint());
    println!(
        "  {:<15} {}",
        "api key",
        match cfg.api_key.as_deref() {
            // **Never printed.** Whether one exists is the useful fact; the key
            // itself belongs on the clipboard, not in a terminal someone screenshots.
            Some(k) if !k.is_empty() => format!("set, {} characters", k.chars().count()),
            _ => "none".into(),
        }
    );
    for (label, v) in [
        ("cache (GiB)", cfg.cache_gib.map(|v| v.to_string())),
        ("threads", cfg.threads.map(|v| v.to_string())),
        ("threads (batch)", cfg.threads_batch.map(|v| v.to_string())),
        ("context", cfg.context.map(|v| v.to_string())),
        ("models dir", cfg.models_dir.clone()),
        ("core address", cfg.core_addr.clone()),
    ] {
        println!("  {label:<15} {}", v.unwrap_or_else(|| "unset".into()));
    }
    println!("  {:<15} {}", "measure", cfg.auto);
    println!("  {:<15} {}", "force", cfg.force);
    println!();
    println!("The window writes this file and every command here reads it, so a");
    println!("node started from a terminal uses the settings chosen in the app.");
    println!();
    println!("`chaos start <model>` would run:");
    println!(
        "  chaos-serve {}",
        redacted(&cfg.serve_args("<model>"), cfg.api_key.as_deref())
    );
    0
}

/// The argv a node would get, with the key blanked.
fn redacted(args: &[String], key: Option<&str>) -> String {
    let mut out: Vec<String> = Vec::with_capacity(args.len());
    let mut skip_next = false;
    for a in args {
        if skip_next {
            out.push("<key>".into());
            skip_next = false;
            continue;
        }
        if a == "--api-key" {
            skip_next = true;
        }
        // Belt and braces: if the key reached the list some other way, it still
        // does not reach the terminal.
        if let Some(k) = key {
            if !k.is_empty() && a == k {
                out.push("<key>".into());
                continue;
            }
        }
        out.push(a.clone());
    }
    out.join(" ")
}
