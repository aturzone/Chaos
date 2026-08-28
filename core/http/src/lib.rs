//! Just enough HTTP/1.1 to talk to a Chaos node, and nothing more.
//!
//! **Why this exists at all.** Atur's plan for the command-line tier asks for two
//! things that need a client: *"report what `/status` serves, **without curl**"*
//! and *"connect to another node and use it"*. The workspace had neither — model
//! downloads shell out to `curl`, and `chaos-worker` speaks its own binary
//! framing over a raw socket. A tool that cannot report a node's status without a
//! second program installed is not a first-class tier.
//!
//! **Plain HTTP, deliberately, and this is a documented limit rather than an
//! oversight.** There is no TLS here: implementing it would mean a certificate
//! store and a crypto dependency, in a project whose defining property is that it
//! has no dependencies and downloads nothing. A Chaos node serves a LAN over
//! plain `http`, which is the same reason the browser reader cannot open a camera
//! against one — see the secure-context decision in
//! `docs/graph/research/secure-context-decision-2026-08-28.md`. Anything needing
//! `https` should keep shelling out to `curl`, which is what
//! `chaos_model::download` does and will go on doing.
//!
//! No dependencies, no ggml: `std::net` and a byte parser.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// A URL, split into the parts a request needs.
#[derive(Debug, PartialEq, Eq)]
pub struct Url {
    pub host: String,
    pub port: u16,
    /// Always begins with `/`.
    pub path: String,
}

impl Url {
    /// Parse `host:port/path`, with or without an `http://` in front.
    ///
    /// **A bare `host:port` is accepted on purpose.** It is what a person reads
    /// off the CHAOS page and types into a phone, so it is what they will type
    /// here; requiring a scheme would be a papercut with no upside. `https://`
    /// is rejected loudly rather than silently downgraded — a caller who asked
    /// for TLS and got plaintext is worse off than one who got an error.
    pub fn parse(s: &str) -> Result<Url, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("no address given".into());
        }
        if let Some(rest) = s.strip_prefix("https://") {
            return Err(format!(
                "https is not supported: this client speaks plain HTTP only, and \
                 a Chaos node serves plain HTTP. Try http://{rest}"
            ));
        }
        let rest = s.strip_prefix("http://").unwrap_or(s);
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        if authority.is_empty() {
            return Err(format!("{s:?} has no host"));
        }
        // A bracketed IPv6 literal keeps its colons.
        let (host, port) = if let Some(end) = authority.strip_prefix('[') {
            match end.find(']') {
                Some(i) => {
                    let h = &end[..i];
                    let after = &end[i + 1..];
                    let p = after.strip_prefix(':').unwrap_or("8080");
                    (h.to_string(), p)
                }
                None => return Err(format!("{authority:?} opens a [ and never closes it")),
            }
        } else {
            match authority.rsplit_once(':') {
                Some((h, p)) => (h.to_string(), p),
                None => (authority.to_string(), "8080"),
            }
        };
        let port: u16 = port
            .parse()
            .map_err(|_| format!("{port:?} is not a port number"))?;
        if host.is_empty() {
            return Err(format!("{s:?} has no host"));
        }
        Ok(Url {
            host,
            port,
            path: path.to_string(),
        })
    }

    /// `host:port`, as a socket address string.
    pub fn authority(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// What came back.
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn connect(url: &Url, timeout: Duration) -> Result<TcpStream, String> {
    let authority = url.authority();
    // Resolve first, so a name that does not exist says so rather than timing
    // out: they are different problems and the message should say which.
    let mut addrs = authority
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve {authority}: {e}"))?;
    let addr = addrs
        .next()
        .ok_or_else(|| format!("{authority} resolved to no addresses"))?;
    let stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("cannot reach {authority}: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("cannot set a read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|e| format!("cannot set a write timeout: {e}"))?;
    stream.set_nodelay(true).ok();
    Ok(stream)
}

fn send(
    url: &Url,
    method: &str,
    body: Option<&str>,
    bearer: Option<&str>,
    accept: &str,
    timeout: Duration,
) -> Result<BufReader<TcpStream>, String> {
    let stream = connect(url, timeout)?;
    let mut w = stream
        .try_clone()
        .map_err(|e| format!("cannot split the socket: {e}"))?;
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nAccept: {accept}\r\n\
         User-Agent: chaos\r\nConnection: close\r\n",
        url.path,
        url.authority()
    );
    // **The key is a header and never anything else.** Not a query parameter,
    // where it would land in logs and in shell history.
    if let Some(k) = bearer {
        if !k.is_empty() {
            head.push_str(&format!("Authorization: Bearer {k}\r\n"));
        }
    }
    if let Some(b) = body {
        head.push_str("Content-Type: application/json\r\n");
        head.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    head.push_str("\r\n");
    w.write_all(head.as_bytes())
        .map_err(|e| format!("cannot send the request: {e}"))?;
    if let Some(b) = body {
        w.write_all(b.as_bytes())
            .map_err(|e| format!("cannot send the body: {e}"))?;
    }
    w.flush().map_err(|e| format!("cannot flush: {e}"))?;
    Ok(BufReader::new(stream))
}

/// Read the status line and headers, leaving the reader at the body.
///
/// Returns the status and whether the body is chunked, plus a content length if
/// one was given.
fn read_head(r: &mut BufReader<TcpStream>) -> Result<(u16, bool, Option<usize>), String> {
    let mut line = String::new();
    r.read_line(&mut line)
        .map_err(|e| format!("no reply: {e}"))?;
    if line.is_empty() {
        return Err("the connection closed before it answered".into());
    }
    let status: u16 = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("cannot read a status from {line:?}"))?;

    let mut chunked = false;
    let mut length = None;
    loop {
        let mut h = String::new();
        let n = r
            .read_line(&mut h)
            .map_err(|e| format!("cannot read headers: {e}"))?;
        if n == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        let lower = h.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("transfer-encoding:") {
            chunked = v.contains("chunked");
        } else if let Some(v) = lower.strip_prefix("content-length:") {
            length = v.trim().parse::<usize>().ok();
        }
    }
    Ok((status, chunked, length))
}

fn read_body(
    r: &mut BufReader<TcpStream>,
    chunked: bool,
    length: Option<usize>,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    if chunked {
        loop {
            let mut size_line = String::new();
            if r.read_line(&mut size_line).map_err(io)? == 0 {
                break;
            }
            let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or(""), 16)
                .map_err(|_| format!("bad chunk size {size_line:?}"))?;
            if size == 0 {
                break;
            }
            let mut chunk = vec![0u8; size];
            r.read_exact(&mut chunk).map_err(io)?;
            body.extend_from_slice(&chunk);
            let mut crlf = [0u8; 2];
            let _ = r.read_exact(&mut crlf);
        }
    } else if let Some(n) = length {
        body = vec![0u8; n];
        r.read_exact(&mut body).map_err(io)?;
    } else {
        // `Connection: close` and no length: read to the end.
        r.read_to_end(&mut body).map_err(io)?;
    }
    Ok(body)
}

fn io(e: std::io::Error) -> String {
    format!("cannot read the body: {e}")
}

/// GET a URL and read the whole answer.
pub fn get(url: &str, timeout: Duration) -> Result<Response, String> {
    let u = Url::parse(url)?;
    let mut r = send(&u, "GET", None, None, "application/json", timeout)?;
    let (status, chunked, length) = read_head(&mut r)?;
    let body = read_body(&mut r, chunked, length)?;
    Ok(Response { status, body })
}

/// POST a JSON body and read the whole answer.
pub fn post_json(
    url: &str,
    body: &str,
    bearer: Option<&str>,
    timeout: Duration,
) -> Result<Response, String> {
    let u = Url::parse(url)?;
    let mut r = send(&u, "POST", Some(body), bearer, "application/json", timeout)?;
    let (status, chunked, length) = read_head(&mut r)?;
    let out = read_body(&mut r, chunked, length)?;
    Ok(Response { status, body: out })
}

/// POST a JSON body and hand each server-sent event's `data:` payload to `on`.
///
/// **This is how a token appears as it is produced rather than at the end.** `on`
/// returning `false` stops reading, which is how a caller cancels. The terminal
/// `[DONE]` sentinel is not passed on: it is framing, not content.
pub fn post_sse(
    url: &str,
    body: &str,
    bearer: Option<&str>,
    timeout: Duration,
    on: &mut dyn FnMut(&str) -> bool,
) -> Result<u16, String> {
    let u = Url::parse(url)?;
    let mut r = send(&u, "POST", Some(body), bearer, "text/event-stream", timeout)?;
    let (status, _chunked, _length) = read_head(&mut r)?;
    if status != 200 {
        // An error body is small and worth showing rather than swallowing.
        let text = read_body(&mut r, false, None).unwrap_or_default();
        let text = String::from_utf8_lossy(&text);
        return Err(format!("the node answered {status}: {}", text.trim()));
    }
    // **Read lines, not chunks.** A chunk boundary falls wherever the server
    // flushed, which is not where an event ends, so framing on chunks drops or
    // splits events. `BufRead::read_line` re-assembles across them.
    loop {
        let mut line = String::new();
        let n = r.read_line(&mut line).map_err(io)?;
        if n == 0 {
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        if !on(data) {
            break;
        }
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A bare `host:port` is the thing people actually type**, because it is
    /// what the CHAOS page shows them.
    #[test]
    fn an_address_can_be_typed_the_way_it_is_read_off_the_page() {
        let u = Url::parse("192.168.1.20:8080").unwrap();
        assert_eq!(u.host, "192.168.1.20");
        assert_eq!(u.port, 8080);
        assert_eq!(u.path, "/");
        assert_eq!(u.authority(), "192.168.1.20:8080");
    }

    #[test]
    fn a_scheme_and_a_path_are_both_optional() {
        for (given, host, port, path) in [
            ("http://127.0.0.1:8231/status", "127.0.0.1", 8231, "/status"),
            ("127.0.0.1:8231/status", "127.0.0.1", 8231, "/status"),
            ("example.local", "example.local", 8080, "/"),
            (
                "http://example.local/v1/models",
                "example.local",
                8080,
                "/v1/models",
            ),
        ] {
            let u = Url::parse(given).unwrap_or_else(|e| panic!("{given}: {e}"));
            assert_eq!(
                (u.host.as_str(), u.port, u.path.as_str()),
                (host, port, path),
                "{given}"
            );
        }
    }

    /// An IPv6 literal is full of the character a port is split on.
    #[test]
    fn a_bracketed_ipv6_literal_keeps_its_colons() {
        let u = Url::parse("[::1]:8080/status").unwrap();
        assert_eq!(u.host, "::1");
        assert_eq!(u.port, 8080);
        assert_eq!(u.path, "/status");
        assert_eq!(u.authority(), "[::1]:8080");
    }

    /// **https must fail loudly.** Silently sending plaintext to somebody who
    /// asked for TLS is the one failure here that could matter.
    #[test]
    fn https_is_refused_rather_than_downgraded() {
        let e = Url::parse("https://example.com/status").unwrap_err();
        assert!(e.contains("https is not supported"), "{e}");
        assert!(
            e.contains("http://example.com/status"),
            "the error does not suggest the fix: {e}"
        );
    }

    #[test]
    fn nonsense_is_rejected_with_a_reason() {
        for bad in ["", "   ", "host:notaport", "[::1/status"] {
            assert!(Url::parse(bad).is_err(), "{bad:?} was accepted");
        }
    }
}
