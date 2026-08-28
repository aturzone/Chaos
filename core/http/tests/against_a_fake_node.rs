//! The client against a socket, including the failure that shipped for an hour.
//!
//! **A killed node and a finished answer both end in EOF.** `post_sse` returned
//! `Ok` on EOF, so `chaos connect` printed a truncated answer — cut mid-word at
//! "Sevent" when the node was killed four seconds into counting to two hundred —
//! and exited **0**. A script piping that into a file would have kept the
//! fragment and believed it. Found by §4e asking what happens when a network
//! drops mid-stream; the answer was "it lies".
//!
//! These tests are a real `TcpListener` on an ephemeral port rather than a
//! refactor into a mockable trait: the bug was in how the socket's *end* was
//! interpreted, and a fake that cannot be cut off cannot reproduce it.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::time::Duration;

/// Serve one canned response on a throwaway port and return that port.
///
/// `then_cut` closes the socket without writing `[DONE]`, which is what a killed
/// node looks like from the client's side.
fn fake_node(body: &'static str, status_line: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("cannot bind a test port");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            // Read the request head so the client's write does not block.
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(status_line.as_bytes());
            let _ = stream.write_all(body.as_bytes());
            let _ = stream.flush();
            // **Shut the write side down, do not just drop it.** Dropping a
            // socket with unread data in its receive queue makes Windows send an
            // RST, and the client then sees ECONNRESET rather than EOF -- which
            // is a different code path from the one that lied. `shutdown(Write)`
            // sends a FIN, then draining lets the peer finish reading before the
            // handle goes away.
            let _ = stream.shutdown(Shutdown::Write);
            let mut drain = [0u8; 1024];
            while let Ok(n) = stream.read(&mut drain) {
                if n == 0 {
                    break;
                }
            }
        }
    });
    port
}

const SSE_HEAD: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n";

fn chunk(text: &str) -> String {
    format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n")
}

/// A whole answer: every chunk delivered, and `[DONE]` seen.
#[test]
fn a_complete_stream_delivers_every_chunk_and_succeeds() {
    let body: &'static str = Box::leak(
        format!(
            "{}{}{}data: [DONE]\n\n",
            chunk("Hel"),
            chunk("lo"),
            chunk("!")
        )
        .into_boxed_str(),
    );
    let port = fake_node(body, SSE_HEAD);

    let mut seen = String::new();
    let r = chaos_http::post_sse(
        &format!("127.0.0.1:{port}/v1/chat/completions"),
        "{}",
        None,
        Duration::from_secs(5),
        &mut |c| {
            if let Some(t) = pull(c) {
                seen.push_str(&t);
            }
            true
        },
    );
    assert_eq!(r, Ok(200), "a complete stream failed: {r:?}");
    assert_eq!(seen, "Hello!");
}

/// **The regression.** The node vanishes after two chunks and never sends
/// `[DONE]`; that must not read as success.
#[test]
fn a_stream_that_stops_without_done_is_an_error_not_a_short_answer() {
    let body: &'static str =
        Box::leak(format!("{}{}", chunk("Sevent"), chunk("een")).into_boxed_str());
    let port = fake_node(body, SSE_HEAD);

    let mut seen = String::new();
    let r = chaos_http::post_sse(
        &format!("127.0.0.1:{port}/v1/chat/completions"),
        "{}",
        None,
        Duration::from_secs(5),
        &mut |c| {
            if let Some(t) = pull(c) {
                seen.push_str(&t);
            }
            true
        },
    );
    let err = r.expect_err("an unterminated stream reported success");
    assert!(
        err.contains("without finishing"),
        "the error does not say the answer is incomplete: {err}"
    );
    assert!(
        err.contains("2 chunk"),
        "the error does not say how much arrived: {err}"
    );
    // **What arrived is still delivered.** The caller has already printed it; the
    // error explains why it stops, it does not pretend nothing came.
    assert_eq!(seen, "Seventeen");
}

/// A caller that stops early stopped on purpose, and is not a truncation.
#[test]
fn a_caller_that_cancels_is_not_an_error() {
    let body: &'static str =
        Box::leak(format!("{}{}{}", chunk("one"), chunk("two"), chunk("three")).into_boxed_str());
    let port = fake_node(body, SSE_HEAD);

    let mut count = 0;
    let r = chaos_http::post_sse(
        &format!("127.0.0.1:{port}/v1/chat/completions"),
        "{}",
        None,
        Duration::from_secs(5),
        &mut |_| {
            count += 1;
            count < 2 // stop after the first chunk
        },
    );
    assert_eq!(r, Ok(200), "cancelling was reported as a failure: {r:?}");
    assert_eq!(count, 2);
}

/// A refusal must carry the node's own words, not just a number.
#[test]
fn a_rejected_request_reports_the_status_and_the_body() {
    let port = fake_node(
        "{\"error\":{\"message\":\"invalid api key\"}}",
        "HTTP/1.1 401 Unauthorized\r\nContent-Length: 40\r\n\r\n",
    );
    let r = chaos_http::post_sse(
        &format!("127.0.0.1:{port}/v1/chat/completions"),
        "{}",
        Some("wrong"),
        Duration::from_secs(5),
        &mut |_| true,
    );
    let err = r.expect_err("a 401 was reported as success");
    assert!(err.contains("401"), "{err}");
    assert!(
        err.contains("invalid api key"),
        "the node's own message is lost: {err}"
    );
}

/// `get` against a node that answers with a length, and against one that does not.
#[test]
fn get_reads_a_body_with_or_without_a_content_length() {
    let json = "{\"status\":\"ok\",\"model\":\"test\"}";
    for head in [
        "HTTP/1.1 200 OK\r\nContent-Length: 30\r\n\r\n",
        "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n",
    ] {
        let port = fake_node(Box::leak(json.to_string().into_boxed_str()), head);
        let r = chaos_http::get(&format!("127.0.0.1:{port}/status"), Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("{head:?}: {e}"));
        assert_eq!(r.status, 200);
        assert_eq!(r.text(), json, "body differed for {head:?}");
    }
}

/// A tiny reader for the chunk shape, so these tests do not depend on `chaos-cli`.
fn pull(chunk: &str) -> Option<String> {
    let at = chunk.find("\"content\":\"")? + "\"content\":\"".len();
    let rest = &chunk[at..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
