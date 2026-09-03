//! `chaos-qr` -- print a QR code in a terminal.
//!
//! **The reason this is a binary and not a library call.** Someone runs Chaos
//! on a headless server and wants to reach it from a phone. There is no window
//! to show the mark in and no browser to open it with, so the route has to
//! come out of the terminal they are already looking at -- over SSH, in a
//! `systemd` log, in a `tmux` pane. `chaos-qr --route 8080` prints it.

use chaos_qr::{encode, Level, Render};

/// Write `qr.html` and `scan.html` into `dir`, self-contained.
///
/// **Here because this binary needs no C toolchain.** The same two files can be
/// emitted by `chaos-serve --emit-pages`, and that is what the Android release
/// used to call -- which meant every release compiled a **host** llama.cpp, a
/// second full cmake, purely so a binary that writes two HTML files could link
/// ggml. `chaos_grimoire` is string assembly with no ggml reference in it, and
/// `chaos-qr` is already the brand tier's terminal half, so the emitter belongs
/// on a binary that builds anywhere.
///
/// No endpoint is baked in: a file on disk has no idea which node will show it,
/// and the host that loads it passes one -- `?endpoint=` for the Android
/// WebView, `window.CHAOS_ENDPOINT` for anything embedding it.
fn emit_pages(dir: &std::path::Path) -> std::io::Result<()> {
    use chaos_grimoire::{page, Host, Page};
    std::fs::create_dir_all(dir)?;
    for (name, which) in [("qr", Page::Mark), ("scan", Page::Scry)] {
        let file = dir.join(format!("{name}.html"));
        let html = page(which, Host::default());
        std::fs::write(&file, &html)?;
        println!("wrote {} ({} bytes)", file.display(), html.len());
    }
    Ok(())
}

fn usage() {
    println!("usage: chaos-qr <text>            print any text as a QR code");
    println!("       chaos-qr --route [port]    print this machine's Chaos route");
    println!("       chaos-qr --emit-pages <dir>  write qr.html and scan.html");
    println!();
    println!("  --ecc L|M|Q|H   error correction (default Q: photographed off a screen)");
    println!("  --quiet N       margin in modules (default 4; the specification's minimum)");
    println!("  --ascii         two `#` per module -- no Unicode, no colour");
    println!("  --unicode       half-blocks, no colour. Assumes a LIGHT terminal");
    println!("  --invert        with --unicode, for a dark terminal");
    println!("  --plain         just print the text, no code");
    println!();
    println!("Without a flag the code is drawn in explicit black-on-white colour,");
    println!("because a QR is dark-on-light by definition and leaving the ground to");
    println!("the terminal produces an inverted code on half the machines there are.");
    println!();
    println!("Byte mode, versions 1-6: 74 bytes at level Q, 154 at level L.");
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() || argv.iter().any(|a| a == "-h" || a == "--help") {
        usage();
        return;
    }

    let mut level = Level::Q;
    let mut quiet = 4usize;
    let mut how = Render::Ansi;
    let mut invert = false;
    let mut plain = false;
    let mut route_port: Option<u16> = None;
    let mut want_route = false;
    let mut text: Option<String> = None;

    // **Before anything treats an argument as the text to encode.** This
    // writes two files and exits; it is how the Android APK carries the brand
    // pages without a second copy of the wrapping logic.
    if let Some(i) = argv.iter().position(|a| a == "--emit-pages") {
        let dir = match argv.get(i + 1) {
            Some(d) if !d.starts_with('-') => std::path::PathBuf::from(d),
            _ => {
                eprintln!("chaos-qr: --emit-pages wants a directory to write into");
                std::process::exit(2);
            }
        };
        if let Err(e) = emit_pages(&dir) {
            eprintln!("chaos-qr: --emit-pages {}: {e}", dir.display());
            std::process::exit(1);
        }
        return;
    }

    let mut i = 0usize;
    while i < argv.len() {
        let a = argv[i].as_str();
        match a {
            "--ecc" | "-e" => {
                i += 1;
                match argv.get(i).and_then(|s| Level::parse(s)) {
                    Some(l) => level = l,
                    None => die(&format!(
                        "--ecc wants one of L, M, Q, H; got {:?}",
                        argv.get(i).map(String::as_str).unwrap_or("nothing")
                    )),
                }
            }
            "--quiet" | "-q" => {
                i += 1;
                match argv.get(i).and_then(|s| s.parse::<usize>().ok()) {
                    // A margin over 20 is almost certainly a typo for the text,
                    // and it would print a screenful of blank lines.
                    Some(n) if n <= 20 => quiet = n,
                    _ => die("--quiet wants a number of modules, 0 to 20"),
                }
            }
            "--ascii" => how = Render::Ascii,
            "--unicode" => how = Render::Unicode,
            "--invert" => invert = true,
            "--plain" => plain = true,
            "--route" => {
                want_route = true;
                // The port is optional and positional, so only consume the next
                // argument when it actually looks like one.
                if let Some(p) = argv.get(i + 1).and_then(|s| s.parse::<u16>().ok()) {
                    route_port = Some(p);
                    i += 1;
                }
            }
            // **`--version` on every binary, because a person checks it first.**
            // `gguf-info` had this exact gap and it was fixed there: the flag was
            // being opened as a model, so `--version` reported "cannot find the
            // file specified". Here it was refused as an unknown flag, which is
            // tidier and just as wrong -- ten of the eleven shipped binaries
            // answered it and this one did not.
            "--version" | "-V" => {
                println!("chaos-qr {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            other if other.starts_with("--") => die(&format!("unknown flag {other}")),
            other => text = Some(other.to_string()),
        }
        i += 1;
    }

    if how == Render::Unicode && invert {
        how = Render::UnicodeInverted;
    }

    let payload = if want_route {
        let port = route_port.unwrap_or(8080);
        let (addr, loopback) = chaos_probe::net::reachable_address("0.0.0.0");
        if loopback {
            eprintln!(
                "warning: no route off this machine was found, so this is a loopback\n\
                 address. Nothing else can reach it -- check the network is up."
            );
        }
        format!("http://{addr}:{port}")
    } else {
        match text {
            Some(t) => t,
            None => {
                usage();
                return;
            }
        }
    };

    if plain {
        println!("{payload}");
        return;
    }

    match encode(&payload, level) {
        Ok(code) => {
            print!("{}", code.render(how, quiet));
            println!("{payload}");
            println!(
                "version {}, level {:?}, {}x{} modules",
                code.version, code.level, code.size, code.size
            );
        }
        Err(e) => die(&e.to_string()),
    }
}

fn die(message: &str) -> ! {
    eprintln!("chaos-qr: {message}");
    std::process::exit(2);
}
