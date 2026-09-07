//! The book, served to a browser by this process.
//!
//! # Why the window serves it itself
//!
//! **Atur, testing v0.0.31: *"the book of QR code for Core mode is not
//! available!!! that book where is it!!"*** The window used to reach the art
//! the only way it could: `ShellExecute` on `http://<this node>/qr`, a route on
//! the child `chaos-serve`. That makes the book **a feature of a loaded model**
//! — open the app, press the button before pressing LOAD, and the browser says
//! the site cannot be reached. The art has nothing to do with inference and
//! should not wait on 7 GiB of weights.
//!
//! So this binds `127.0.0.1` on an ephemeral port and answers one path. It
//! needs no model, which is the whole point.
//!
//! **A second reason used to apply and no longer does.** The reader — a camera
//! pointed at another node's mark — needed [a secure context], which a LAN
//! address is not, and loopback was how it got one. The reader is gone: Atur,
//! 2026-09-07, *"that book and barcode aren't needed any more, we only use the
//! book"*. Serving from loopback is now justified by the model, not the camera.
//!
//! [a secure context]: https://developer.mozilla.org/docs/Web/Security/Secure_Contexts
//!
//! The address the mark **encodes** is passed in separately, because
//! [`chaos_grimoire`] injects it and the page prefers an injected endpoint over
//! the origin it was served from. That precedence is not incidental —
//! `resolveEndpoint` refuses to infer a loopback endpoint, on the grounds that
//! it is useless to the one person who matters, the one holding another device.
//!
//! # What it is not
//!
//! Not a web server. It binds loopback only, answers exactly one fixed path
//! with a string already in memory, and touches no file. There is no path
//! traversal to defend against because there are no paths — a request is either
//! that one constant or a 404.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Mutex, OnceLock};

/// The assembled page, replaced each time the book is opened.
///
/// The QR depends on the address the user is currently showing and on the
/// window's theme, both of which change while the app is open, so the bytes are
/// rebuilt per open rather than once at startup.
fn page() -> &'static Mutex<String> {
    static P: OnceLock<Mutex<String>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(String::new()))
}

/// The port the page server listens on, or 0 before it has started.
fn port() -> &'static Mutex<u16> {
    static P: OnceLock<Mutex<u16>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(0))
}

/// The path the book answers on, matching the node's own route so the two are
/// one thing to learn rather than two.
const ROUTE: &str = "qr";

/// Build the book for the given route and theme, and return the loopback URL
/// that shows it.
///
/// `endpoint` is the address **another machine** would use to reach this node —
/// what the mark encodes. `None` leaves the page to fall back to the project's
/// own URL, which is what it shows before a role is chosen.
///
/// Starting the listener is idempotent: the first call binds, later calls reuse
/// the port. An error means loopback could not be bound at all, which is worth
/// reporting rather than papering over.
pub fn open(endpoint: Option<&str>, theme: Option<&str>) -> std::io::Result<String> {
    *page().lock().expect("page") = chaos_grimoire::mark(chaos_grimoire::Host { endpoint, theme });
    let p = ensure_server()?;
    Ok(format!("http://127.0.0.1:{p}/{ROUTE}"))
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
    if !wants_the_book(&line) {
        // A named 404, because the alternative is a blank tab with no way to
        // tell it from a page that rendered nothing.
        let body = "<!doctype html><title>Not here</title><p>This is the Chaos \
                    brand page server. It answers <code>/qr</code>.";
        return write!(
            stream,
            "HTTP/1.1 404 Not Found\r\ncontent-type: text/html; charset=utf-8\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
    }
    let body = page().lock().expect("page").clone();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\n\
         content-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Whether a request line asks for the book.
///
/// A pure function of the line so it can be tested without a socket, and so the
/// matching is visible in one place.
fn wants_the_book(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(method) = parts.next() else {
        return false;
    };
    if method != "GET" && method != "HEAD" {
        return false;
    }
    // A browser may append a query string; the page reads `?theme=` and
    // `?endpoint=` itself, so anything after `?` is not ours to interpret.
    let Some(target) = parts.next() else {
        return false;
    };
    let path = target.split(['?', '#']).next().unwrap_or(target);
    matches!(path.trim_start_matches('/'), "qr" | "mark")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The served page is process-global on purpose — one window, one book,
    /// rebuilt whenever the button is pressed — so any test that asserts on it
    /// has to be the only one doing so.
    ///
    /// **Found by the release build, not the debug one.** `cargo test` passed
    /// for as long as the threads happened to interleave kindly; `--release`
    /// scheduled them differently and one test read a page another had already
    /// overwritten. The failure was in the test, not the module — but a test
    /// that passes by timing is not evidence of anything.
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn one_route_and_nothing_else() {
        assert!(wants_the_book("GET /qr HTTP/1.1"));
        assert!(wants_the_book("GET /mark HTTP/1.1"));
        // The page reads its own query string.
        assert!(wants_the_book("GET /qr?theme=dark HTTP/1.1"));
        assert!(!wants_the_book("GET / HTTP/1.1"));
        assert!(!wants_the_book("GET /../secrets HTTP/1.1"));
        assert!(!wants_the_book("GET /scan HTTP/1.1"), "the reader is gone");
        assert!(!wants_the_book("POST /qr HTTP/1.1"));
        assert!(!wants_the_book(""));
    }

    /// **The point of the module**: the URL handed to the browser is loopback,
    /// so no model has to be loaded, while the address the mark *encodes* is
    /// the LAN one another device must reach.
    #[test]
    fn served_from_loopback_and_pointed_at_the_lan() {
        let _serial = one_at_a_time();
        let url = open(Some("http://192.168.1.20:8080"), Some("dark")).expect("bind");
        assert!(url.starts_with("http://127.0.0.1:"), "{url}");
        assert!(url.ends_with("/qr"));

        let book = page().lock().expect("page").clone();
        assert!(book.contains("window.CHAOS_ENDPOINT=\"http://192.168.1.20:8080\";"));
        assert!(book.contains("data-theme=\"dark\""));
    }

    /// Opening twice must not bind twice: a leaked listener per press is a
    /// handle leak in a window that stays open all day.
    #[test]
    fn the_listener_is_bound_once() {
        let _serial = one_at_a_time();
        let a = open(None, None).expect("bind");
        let b = open(None, None).expect("bind");
        assert_eq!(a, b);
    }

    /// End to end over a real socket, because everything above could be right
    /// while the response itself was malformed.
    #[test]
    fn a_browser_gets_a_whole_document() {
        use std::io::Read;
        let _serial = one_at_a_time();
        let url = open(Some("http://10.0.0.5:8080"), None).expect("bind");
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
        s.write_all(b"GET /scan HTTP/1.1\r\nhost: x\r\n\r\n")
            .unwrap();
        let mut got = String::new();
        s.read_to_string(&mut got).expect("read");
        assert!(got.starts_with("HTTP/1.1 404"), "the reader is gone");
    }
}
