//! The command line for the Chaos server. The server itself is the
//! library beside this, because the Android app runs it in-process.

use chaos_serve::*;
use std::process::ExitCode;

fn main() -> ExitCode {
    // **Before anything treats an argument as a path.** Without this,
    // `chaos-serve --version` reported "cannot find the file specified" -- the
    // flag was being opened as a model. `--version` is how a person checks
    // whether an update landed, so it has to answer on whichever binary they
    // happen to type.
    if std::env::args()
        .skip(1)
        .any(|a| a == "--version" || a == "-V")
    {
        println!("chaos-serve {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }
    let mut path = String::new();
    let mut port = 8080u16;
    let mut cache_gib = 0f64;
    let mut api_key: Option<String> = None;
    // **Loopback until somebody says otherwise.** Opening an inference server
    // to a network is a decision, not a default.
    let mut host = String::from("127.0.0.1");
    let mut context: Option<usize> = None;
    let mut force = false;
    let mut auto = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                port = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(8080);
                i += 2;
            }
            "--cache" => {
                cache_gib = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                i += 2;
            }
            // Read by `configured_threads`, which every graph evaluation calls.
            // Set here rather than threaded through `serve` because the engines
            // are constructed several call-frames down.
            "-t" | "--threads" => {
                if let Some(t) = args.get(i + 1).and_then(|v| v.parse::<usize>().ok()) {
                    std::env::set_var("CHAOS_THREADS", t.to_string());
                }
                i += 2;
            }
            "-tb" | "--threads-batch" => {
                if let Some(t) = args.get(i + 1).and_then(|v| v.parse::<usize>().ok()) {
                    std::env::set_var("CHAOS_THREADS_BATCH", t.to_string());
                }
                i += 2;
            }
            // **Optional, and off unless asked for.** The server binds
            // `127.0.0.1` only, so a key is not what keeps a stranger out --
            // what keeps them out is that there is no route in. It is here
            // because many OpenAI-compatible clients insist on sending one and
            // some refuse to work without a value, and because a shared machine
            // is a real thing.
            // Where to listen. `0.0.0.0` makes the endpoint reachable from a
            // phone on the same Wi-Fi, which is the whole point of the Android
            // client -- and the moment it leaves loopback, the api key stops
            // being optional. See `require_key_off_loopback`.
            "--host" => {
                if let Some(v) = args.get(i + 1) {
                    host = v.to_string();
                }
                i += 2;
            }
            "--api-key" => {
                api_key = args
                    .get(i + 1)
                    .map(|v| v.to_string())
                    .filter(|v| !v.is_empty());
                i += 2;
            }
            // The context limit. **The app has had a control for this since
            // the settings page existed and it did nothing**: the flag was
            // swallowed by the catch-all below, along with every other flag
            // this parser did not know.
            "-c" | "--context" => {
                context = args.get(i + 1).and_then(|v| v.parse::<usize>().ok());
                i += 2;
            }
            // Run an architecture that has not been diffed against llama.cpp.
            //
            // The runner has had this and the server refused to, on the grounds
            // that a client cannot see that an answer is unsound. That reasoning
            // still holds for a stranger's client -- and it does not hold for the
            // person who typed the flag, who is the only one who can pass it. It
            // is off by default and it says what it is doing.
            "--force" => {
                force = true;
                i += 1;
            }
            // Size the expert cache from what the machine actually has free.
            //
            // The runner's `--auto` also picks a device and a layer split; that
            // half has nowhere to go here yet and says so rather than being
            // quietly dropped. What is left is the number that matters most on
            // the streaming path, and it was a flag the app had been sending
            // into a void.
            "--auto" => {
                auto = true;
                i += 1;
            }
            other => {
                // **An unknown flag is an error now.** It used to fall into the
                // model-path slot and vanish, which is how `-ngl`, `-c`,
                // `--auto` and `--force` all came to be accepted and ignored:
                // the app passed four settings the server had never heard of and
                // nothing anywhere said so. A flag is a promise.
                if other.starts_with('-') && other.len() > 1 {
                    eprintln!("chaos-serve: unknown option {other:?}");
                    if let Some(why) = declined(other) {
                        eprintln!("             {why}");
                    }
                    eprintln!("             chaos-serve --help lists what this build accepts");
                    return ExitCode::from(2);
                }
                if path.is_empty() {
                    path = other.to_string();
                }
                i += 1;
            }
        }
    }
    // With no model named and exactly one on the machine, serve that one.
    // There is no ambiguity to resolve and nothing to warn about, and it is what
    // makes a double-click launcher possible -- a shortcut cannot know the name
    // of a file the user has not put there yet. Two or more, and it still lists
    // them and stops, because picking one silently would load the wrong model
    // for minutes before saying so.
    if path.is_empty() {
        let found = chaos_model::find::list();
        if found.len() == 1 {
            eprintln!("model      {} (the only one found)", found[0].label);
            path = found[0].path.to_string_lossy().into_owned();
        } else {
            usage();
            return ExitCode::from(2);
        }
    }

    // The same name lookup the runner has, so `chaos-serve qwen3` works and the
    // two binaries cannot disagree about where models live.
    let path = match chaos_model::find::resolve(&path) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(e) => {
            eprintln!("chaos-serve: {}", chaos_model::find::explain(&path, &e));
            return ExitCode::from(2);
        }
    };
    // The same refusal the runner makes: a truncated download is recognised
    // from the container's own index before anything is bound.
    if let Some(why) = chaos_model::complete::why_incomplete(std::path::Path::new(&path)) {
        eprintln!("chaos-serve: {why}");
        eprintln!("             run `chaos-pull` again -- it resumes.");
        return ExitCode::from(2);
    }
    // **Checked before the model loads, not after.** A four-minute load
    // followed by "refusing to start" is the same refusal delivered at the
    // worst possible moment.
    if let Some(why) = refuse_to_start(&host, api_key.as_deref()) {
        eprintln!("{why}");
        std::process::exit(2);
    }
    match serve(&path, &host, port, cache_gib, api_key, context, force, auto) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("chaos-serve: {e}");
            ExitCode::FAILURE
        }
    }
}
