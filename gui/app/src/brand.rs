//! The book, and the reader, served to a browser by this process.
//!
//! # Why the window serves them itself
//!
//! **Atur, testing v0.0.31: *"the book of QR code for Core mode is not
//! available!!! that book where is it!!"*** He was right, and there were two
//! separate reasons for it.
//!
//! The window used to reach the art the only way it could: `ShellExecute` on
//! `http://<this node>/qr`, a route on the child `chaos-serve`. That makes the
//! brand pages **a feature of a loaded model**. Open the app, turn the dial to
//! CORE, press the button that shows a book — and the browser says the site
//! cannot be reached, because no model is loaded and so no server is running.
//! The art has nothing to do with inference and should not wait on 7 GiB of
//! weights.
//!
//! The second reason is subtler and made the *reader* useless even with a model
//! loaded. A camera needs a secure context, and a LAN address is not one:
//! `http://192.168.1.20:8080/scan` gets `getUserMedia` refused by every
//! browser. Only `https://`, or **loopback**, counts. In CORE mode the window
//! handed the reader precisely the address that cannot work — its own LAN
//! address, correct for the mark and wrong for the reader.
//!
//! So this module binds `127.0.0.1` on an ephemeral port and answers two paths.
//! Loopback is a secure context, so the camera opens; it needs no model, so the
//! book opens from a cold start; and the address the *mark* encodes is passed in
//! separately, because `chaos_grimoire` injects it and the page prefers an
//! injected endpoint over the origin it was served from. That precedence is not
//! incidental — `resolveEndpoint` refuses to infer a loopback endpoint, on the
//! grounds that it is useless to the one person who matters, the one holding
//! another device.
//!
//! # What it is not
//!
//! Not a web server. It binds loopback only, answers exactly two fixed paths
//! with two strings already in memory, and touches no file. There is no path
//! traversal to defend against because there are no paths — a request is either
//! one of two constants or a 404.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Mutex, OnceLock};

pub use chaos_grimoire::Page;

/// What the server hands out, replaced each time a page is opened.
///
/// The mark's QR depends on the address the user is currently showing and on
/// the window's theme, both of which change while the app is open, so the bytes
/// are rebuilt per open rather than once at startup.
#[derive(Default)]
struct Pages {
    mark: String,
    scry: String,
}

fn pages() -> &'static Mutex<Pages> {
    static P: OnceLock<Mutex<Pages>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(Pages::default()))
}

/// The port the page server listens on, or 0 before it has started.
fn port() -> &'static Mutex<u16> {
    static P: OnceLock<Mutex<u16>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(0))
}

/// The path each page answers on, matching the node's own routes so the two are
/// one thing to learn rather than two.
fn route(which: Page) -> &'static str {
    match which {
        Page::Mark => "qr",
        Page::Scry => "scan",
    }
}

/// Build both pages for the given route and theme, and return the loopback URL
/// that shows `which`.
///
/// `endpoint` is the address **another machine** would use to reach this node —
/// what the mark encodes. `None` leaves the page to fall back to the project's
/// own URL, which is what it shows before a role is chosen.
///
/// Starting the listener is idempotent: the first call binds, later calls reuse
/// the port. An error means loopback could not be bound at all, which is worth
/// reporting rather than papering over.
pub fn open(which: Page, endpoint: Option<&str>, theme: Option<&str>) -> std::io::Result<String> {
    let host = chaos_grimoire::Host { endpoint, theme };
    {
        let mut p = pages().lock().expect("pages");
        p.mark = chaos_grimoire::mark(host);
        p.scry = chaos_grimoire::scry(host);
    }
    let p = ensure_server()?;
    Ok(format!("http://127.0.0.1:{p}/{}", route(which)))
}

/// Bind loopback and start answering, once per process.
fn ensure_server() -> std::io::Result<u16> {
    let mut guard = port().lock().expect("port");
    if *guard != 0 {
        return Ok(*guard);
    }
    // Port 0: the operating system picks a free one. A fixed port would collide
    // with the engine, with a second copy of this app, or with whatever else
    // the machine happens to be running.
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
    let listener = TcpListener::bind(addr)?;
    let bound = listener.local_addr()?.port();
    std::thread::Builder::new()
        .name("chaos-brand".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                // One at a time, deliberately: a browser opens a handful of
                // requests for one page and each is answered in microseconds
                // from a string already in memory. A thread per connection
                // would be a pool to get wrong for no gain.
                let _ = answer(stream);
            }
        })?;
    *guard = bound;
    Ok(bound)
}

/// Read one request line and write one response.
fn answer(mut stream: TcpStream) -> std::io::Result<()> {
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    let body = match requested(&line) {
        Some(Page::Mark) => pages().lock().expect("pages").mark.clone(),
        Some(Page::Scry) => pages().lock().expect("pages").scry.clone(),
        None => {
            // A named 404, because the alternative is a blank tab with no way
            // to tell it from a page that rendered nothing.
            let body = "<!doctype html><title>Not here</title><p>This is the Chaos \
                        brand page server. It answers <code>/qr</code> and \
                        <code>/scan</code>.";
            return write!(
                stream,
                "HTTP/1.1 404 Not Found\r\ncontent-type: text/html; charset=utf-8\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\n\
         content-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Which page a request line asks for, if either.
///
/// A pure function of the line so it can be tested without a socket, and so the
/// matching is visible in one place.
fn requested(line: &str) -> Option<Page> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    if method != "GET" && method != "HEAD" {
        return None;
    }
    // A browser may append a query string; the pages read `?theme=` and
    // `?endpoint=` themselves, so anything after `?` is not ours to interpret.
    let target = parts.next()?;
    let path = target.split(['?', '#']).next().unwrap_or(target);
    match path.trim_start_matches('/') {
        "qr" | "mark" => Some(Page::Mark),
        "scan" => Some(Page::Scry),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The served pages are process-global on purpose — one window, one pair of
    /// pages, rebuilt whenever a button is pressed — so any test that asserts on
    /// them has to be the only one doing so.
    ///
    /// **Found by the release build, not the debug one.** `cargo test` passed
    /// here for as long as the threads happened to interleave kindly;
    /// `--release` scheduled them differently and
    /// `served_from_loopback_and_pointed_at_the_lan` read a mark that
    /// `a_browser_gets_a_whole_document` had already overwritten with its own
    /// endpoint. The failure was in the test, not the module — but a test that
    /// passes by timing is not evidence of anything.
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn the_two_routes_and_nothing_else() {
        assert_eq!(requested("GET /qr HTTP/1.1"), Some(Page::Mark));
        assert_eq!(requested("GET /mark HTTP/1.1"), Some(Page::Mark));
        assert_eq!(requested("GET /scan HTTP/1.1"), Some(Page::Scry));
        // The pages read their own query string.
        assert_eq!(requested("GET /scan?theme=dark HTTP/1.1"), Some(Page::Scry));
        assert_eq!(requested("GET / HTTP/1.1"), None);
        assert_eq!(requested("GET /../secrets HTTP/1.1"), None);
        assert_eq!(requested("POST /qr HTTP/1.1"), None);
        assert_eq!(requested(""), None);
    }

    /// **The point of the module**: the URL handed to the browser is loopback,
    /// because that is the only origin a camera will open on, while the address
    /// the mark *encodes* is the LAN one.
    #[test]
    fn served_from_loopback_and_pointed_at_the_lan() {
        let _serial = one_at_a_time();
        let url = open(Page::Scry, Some("http://192.168.1.20:8080"), Some("dark")).expect("bind");
        assert!(
            url.starts_with("http://127.0.0.1:"),
            "a camera is refused anywhere but a secure context: {url}"
        );
        assert!(url.ends_with("/scan"));

        let mark = pages().lock().expect("pages").mark.clone();
        assert!(mark.contains("window.CHAOS_ENDPOINT=\"http://192.168.1.20:8080\";"));
        assert!(mark.contains("data-theme=\"dark\""));
    }

    /// Opening twice must not bind twice: a leaked listener per press is a
    /// handle leak in a window that stays open all day.
    #[test]
    fn the_listener_is_bound_once() {
        let _serial = one_at_a_time();
        let a = open(Page::Mark, None, None).expect("bind");
        let b = open(Page::Scry, None, None).expect("bind");
        let port_of = |u: &str| {
            u.trim_start_matches("http://127.0.0.1:")
                .split('/')
                .next()
                .unwrap()
                .to_string()
        };
        assert_eq!(port_of(&a), port_of(&b));
    }

    /// End to end over a real socket, because everything above could be right
    /// while the response itself was malformed.
    #[test]
    fn a_browser_gets_a_whole_document() {
        let _serial = one_at_a_time();
        use std::io::Read;
        let url = open(Page::Mark, Some("http://10.0.0.5:8080"), None).expect("bind");
        let port: u16 = url
            .trim_start_matches("http://127.0.0.1:")
            .split('/')
            .next()
            .unwrap()
            .parse()
            .unwrap();

        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.write_all(b"GET /qr HTTP/1.1\r\nhost: x\r\n\r\n").unwrap();
        let mut got = String::new();
        s.read_to_string(&mut got).expect("read");
        assert!(
            got.starts_with("HTTP/1.1 200 OK"),
            "{}",
            &got[..40.min(got.len())]
        );
        assert!(got.contains("content-type: text/html"));
        assert!(got.contains("<!doctype html>"));
        assert!(got.trim_end().ends_with("</html>"));
        // The fonts travel in the page, so a browser with no route out shows it.
        assert!(got.contains("data:font/woff2;base64,"));

        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.write_all(b"GET /nope HTTP/1.1\r\nhost: x\r\n\r\n")
            .unwrap();
        let mut got = String::new();
        s.read_to_string(&mut got).expect("read");
        assert!(got.starts_with("HTTP/1.1 404"));
    }
}
