//! This encoder against the one that ships in the browser page, module for
//! module.
//!
//! **Why a fixture and not a live comparison.** The other encoder is
//! JavaScript inside `assets/grimoire/grimoire.html`; a test that ran it would
//! need node on every machine that runs `cargo test`. So the grids are cut
//! ahead of time by `scripts/qr-fixture.py`, which is where the evidence lives:
//! every grid in `reference-grids.txt` was produced by the page's encoder, put
//! back through `assets/grimoire/decode_qr.py` -- written from the *reading*
//! side, with a Reed-Solomon syndrome check that no shared misunderstanding can
//! satisfy -- and compared against `python-qrcode`.
//!
//! **A difference the generator found, recorded here so nobody rediscovers
//! it.** On three of the nine payloads python-qrcode chooses a different mask.
//! Scored against ISO 18004's four rules by a third implementation, the page's
//! choice is the better one every time: 311 against 416 on `"hi"`, 334 against
//! 436, 296 against 325. Both codes decode -- mask selection is a quality
//! heuristic, not correctness -- but "they differ and it is fine" is a sentence
//! that needs a number after it.
//!
//! A failure here means the Rust port and the shipped page would hand out
//! *different codes for the same route*, which is the one thing this crate
//! exists to avoid.

use chaos_qr::{encode, Level};

const FIXTURE: &str = include_str!("reference-grids.txt");

/// Just enough base64 to read the fixture. The payloads contain spaces and
/// non-ASCII bytes, which is why they are encoded at all; a dependency for
/// twelve lines would be the wrong trade in a crate that has none.
fn unbase64(s: &str) -> Vec<u8> {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for c in s.bytes().filter(|&c| c != b'=') {
        let v = ALPHA.iter().position(|&a| a == c).expect("base64 alphabet") as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

struct Reference {
    payload: String,
    version: usize,
    rows: Vec<String>,
}

fn fixtures() -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    for line in FIXTURE.lines() {
        let line = line.trim_end();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("payload ") {
            let (v, b64) = rest.split_once(' ').expect("payload <version> <base64>");
            out.push(Reference {
                payload: String::from_utf8(unbase64(b64)).expect("utf-8 payload"),
                version: v.parse().expect("a version number"),
                rows: Vec::new(),
            });
        } else {
            out.last_mut().expect("a grid before its rows").rows.push(line.to_string());
        }
    }
    assert!(out.len() >= 9, "the fixture lost entries: {}", out.len());
    out
}

#[test]
fn identical_to_the_encoder_that_ships_in_the_page() {
    let cases = fixtures();
    for case in &cases {
        let code = encode(&case.payload, Level::Q)
            .unwrap_or_else(|e| panic!("{:?} did not encode: {e}", case.payload));
        assert_eq!(
            code.version, case.version,
            "{:?}: version {} here, {} in the page",
            case.payload, code.version, case.version
        );
        assert_eq!(
            code.size,
            case.rows.len(),
            "{:?}: {} rows here, {} in the page",
            case.payload,
            code.size,
            case.rows.len()
        );
        let ours = code.rows();
        let mut differ = 0usize;
        let mut first: Option<(usize, usize)> = None;
        for (y, (a, b)) in ours.iter().zip(case.rows.iter()).enumerate() {
            for (x, (ca, cb)) in a.chars().zip(b.chars()).enumerate() {
                if ca != cb {
                    differ += 1;
                    first.get_or_insert((x, y));
                }
            }
        }
        assert_eq!(
            differ,
            0,
            "{:?}: {differ} of {} modules differ, first at {:?}\n  ours:  {}\n  page:  {}",
            case.payload,
            code.size * code.size,
            first.expect("a first difference"),
            ours[first.unwrap().1],
            case.rows[first.unwrap().1]
        );
    }
    // The fixture is only evidence if it covers the range. Version 1 through 6,
    // and at least one payload that is not ASCII.
    let versions: std::collections::BTreeSet<usize> = cases.iter().map(|c| c.version).collect();
    for v in 1..=6usize {
        assert!(versions.contains(&v), "no version-{v} grid in the fixture");
    }
    assert!(
        cases.iter().any(|c| !c.payload.is_ascii()),
        "nothing in the fixture exercises multi-byte UTF-8"
    );
}
