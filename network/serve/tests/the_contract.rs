//! What the server promises, checked against what it does.
//!
//! **`network/serve` had no `tests/` directory at all** — §4d names it as a thin
//! spot, and it was half right. Twenty-two inline tests already cover request
//! parsing, SSE framing, the embedding shape and the loopback rule, so *those*
//! were never unasserted. What was unasserted is the pair of contracts that span
//! two files each, which is exactly where nobody looks:
//!
//! 1. **the usage block against the route table** — a route implemented and
//!    undocumented is a feature nobody finds; a route documented and unimplemented
//!    is a 404 with the server's own help pointing at it;
//! 2. **`/status`'s keys against the keys `chaos status` reads** — the CLI parses
//!    that JSON by name, so renaming a field there breaks a command in another
//!    crate, silently, with no compiler anywhere in the way.
//!
//! Source checks rather than a socket: starting a server needs a model on disk,
//! and a test that skips itself when the model is absent would assert nothing on
//! the machine most likely to break these.

use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn root() -> PathBuf {
    crate_dir()
        .ancestors()
        .nth(2)
        .expect("network/serve is two levels below the workspace root")
        .to_path_buf()
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn lib_rs() -> String {
    read(&crate_dir().join("src/lib.rs"))
}

/// Every `("METHOD", "/path")` arm in the file.
fn implemented_routes(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '(' && i + 1 < bytes.len() && bytes[i + 1] == '"' {
            let rest: String = bytes[i..].iter().take(80).collect();
            // ("GET", "/path")
            if let Some(close) = rest.find(')') {
                let inner = &rest[1..close];
                let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
                if parts.len() == 2
                    && parts[0].starts_with('"')
                    && parts[1].starts_with("\"/")
                    && parts[1].ends_with('"')
                {
                    let m = parts[0].trim_matches('"').to_string();
                    let p = parts[1].trim_matches('"').to_string();
                    if matches!(m.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "HEAD") {
                        out.push((m, p));
                    }
                }
            }
        }
        i += 1;
    }
    out.sort();
    out.dedup();
    out
}

/// Every `GET /path` or `POST /path` the usage block prints.
fn documented_routes(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in src.lines() {
        // Only the usage block's own lines, which are println! of a route table.
        if !line.contains("println!") {
            continue;
        }
        for method in ["GET", "POST"] {
            if let Some(at) = line.find(method) {
                let after = &line[at + method.len()..];
                let path: String = after
                    .trim_start()
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '"' && *c != '\\')
                    .collect();
                if path.starts_with('/') {
                    out.push((method.to_string(), path));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// **Everything the help promises must exist, and everything that exists and a
/// person would call must be in the help.**
///
/// Two internal routes are exempt and named here rather than pattern-matched, so
/// adding a third is a deliberate act: `/favicon.ico`, which every browser asks
/// for and which exists only to keep a normal request out of the error log, and
/// `/mark`, an alias of the documented `/qr`.
#[test]
fn the_usage_block_and_the_route_table_agree() {
    let src = lib_rs();
    let implemented = implemented_routes(&src);
    let documented = documented_routes(&src);

    assert!(
        implemented.len() >= 8,
        "found only {} routes, so the scan is broken, not the server: {implemented:?}",
        implemented.len()
    );
    assert!(
        documented.len() >= 6,
        "found only {} documented routes, so the scan is broken: {documented:?}",
        documented.len()
    );

    // Documented but absent: the help points at a 404.
    for route in &documented {
        assert!(
            implemented.contains(route),
            "the usage block documents {} {} and nothing serves it",
            route.0,
            route.1
        );
    }

    // Implemented but undocumented: a working endpoint nobody can find. This is
    // how `POST /v1/completions` and `POST /v1/embeddings` went unlisted while
    // being implemented *and* unit-tested.
    let exempt: [(&str, &str); 2] = [("GET", "/favicon.ico"), ("GET", "/mark")];
    for route in &implemented {
        if exempt.iter().any(|(m, p)| *m == route.0 && *p == route.1) {
            continue;
        }
        assert!(
            documented.contains(route),
            "{} {} works and the server's own --help does not mention it. Add it \
             to the usage block, or add it to `exempt` above with a reason.",
            route.0,
            route.1
        );
    }
}

/// **`/status`'s field names are an API another crate parses.**
///
/// `chaos status` reads them by name out of the JSON. Renaming one here breaks
/// that command with no compile error and no test failure anywhere else — the
/// field simply stops appearing in the output, which reads as "the node did not
/// report it" rather than as a bug.
#[test]
fn status_json_still_carries_every_field_the_cli_reads() {
    let serve = lib_rs();
    let cli = read(&root().join("cli/chaos/src/main.rs"));

    // The keys `chaos status` asks `field()` for, taken from its own table.
    let mut wanted: Vec<&str> = Vec::new();
    for line in cli.lines() {
        let t = line.trim();
        // ("label", "key"),
        if !t.starts_with('(') || !t.contains(',') {
            continue;
        }
        if let Some(second) = t.split(',').nth(1) {
            let key = second.trim().trim_end_matches(')').trim_matches('"');
            if !key.is_empty()
                && key.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && key.contains('_')
                || matches!(key, "model" | "route")
            {
                wanted.push(match key {
                    k if !k.is_empty() => Box::leak(k.to_string().into_boxed_str()),
                    _ => continue,
                });
            }
        }
    }
    wanted.sort();
    wanted.dedup();
    assert!(
        wanted.len() >= 5,
        "found only {wanted:?} keys in the CLI, so this scan is broken"
    );

    let status_fn = serve
        .split("fn status_json")
        .nth(1)
        .expect("network/serve no longer has a status_json");
    let body: String = status_fn.chars().take(1200).collect();

    for key in &wanted {
        assert!(
            body.contains(&format!("\"{key}\"")),
            "`chaos status` reads {key:?} out of /status and status_json no longer \
             produces it. Renaming a field here breaks that command silently."
        );
    }
}
