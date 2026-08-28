//! Is this container the file it was when it arrived?
//!
//! §4e wrote the case for this down. Four kinds of broken container were tried:
//! zero bytes, random bytes and a truncated file all fail precisely and exit 1.
//! **Four kilobytes of zeros written into the tensor data loads, exits 0 and
//! answers fluently and differently.** There was no checksum: `download` verifies
//! the four magic bytes, and `chaos-pull-corrupt-resume` records the adjacent
//! failure, where a resumed download ends up *too large* and passes every check.
//!
//! # The manifest, and why it sits beside the models
//!
//! One line per file, in `.chaos-sha256` in the same directory as the container:
//!
//! ```text
//! <64 hex chars>  <size in bytes>  <file name>
//! ```
//!
//! Beside the models rather than in the install directory, for the reason the
//! settings file is: an upgrade replaces the install, and the record of what a
//! 144 GB download hashed to must not go with it.
//!
//! **The size is recorded next to the hash on purpose.** It is free, it is checked
//! first, and it catches the corrupt-resume case in milliseconds instead of
//! minutes — a file of the wrong length cannot be the right file, and there is no
//! need to read 144 GB to know it.
//!
//! # Trust on first use, stated as such
//!
//! Recording a hash the first time a file is seen proves only that it has not
//! changed *since then*. That is worth having — bit-rot, a bad copy and a
//! half-finished resume all become visible — and it is not the same as knowing the
//! publisher's digest. When a publisher's value is known, `expect` takes it and
//! the answer means something stronger. The distinction is in the output, not just
//! in this comment.

use crate::sha256::Sha256;
use std::io::Read;
use std::path::{Path, PathBuf};

/// What the manifest says about one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub sha256: String,
    pub bytes: u64,
    pub name: String,
}

/// The verdict for one container.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Hash and size match what was recorded.
    Matches { sha256: String, bytes: u64 },
    /// Nothing was recorded, so this reading became the record.
    Recorded { sha256: String, bytes: u64 },
    /// **The size differs.** Cheap to detect and conclusive.
    WrongSize { expected: u64, found: u64 },
    /// Same size, different contents: rot, or a bad write.
    WrongHash { expected: String, found: String },
}

impl Verdict {
    /// Whether the file may be trusted.
    pub fn ok(&self) -> bool {
        matches!(self, Verdict::Matches { .. } | Verdict::Recorded { .. })
    }
}

/// Where the manifest for a container lives.
pub fn manifest_for(file: &Path) -> PathBuf {
    file.parent()
        .unwrap_or(Path::new("."))
        .join(".chaos-sha256")
}

/// Parse a manifest's text. Unreadable lines are skipped rather than fatal.
///
/// **A damaged manifest must not stop a model loading.** It is a record, not the
/// thing being recorded, and the failure mode of being strict here is refusing to
/// run over a stray blank line.
pub fn parse(text: &str) -> Vec<Record> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(size)) = (parts.next(), parts.next()) else {
            continue;
        };
        let name = parts.collect::<Vec<_>>().join(" ");
        if name.is_empty() || hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let Ok(bytes) = size.parse::<u64>() else {
            continue;
        };
        out.push(Record {
            sha256: hash.to_ascii_lowercase(),
            bytes,
            name,
        });
    }
    out
}

/// Render records back to manifest text, one per line, sorted by name.
pub fn render(records: &[Record]) -> String {
    let mut sorted: Vec<&Record> = records.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = String::from(
        "# Chaos container digests. One line per file: sha256, bytes, name.\n\
         # Written by `chaos pull` and `chaos verify`. Safe to delete: the next\n\
         # verify records afresh, which then means \"unchanged since then\".\n",
    );
    for r in sorted {
        out.push_str(&format!("{}  {}  {}\n", r.sha256, r.bytes, r.name));
    }
    out
}

/// Read the manifest beside `file`, if there is one.
pub fn read_manifest(file: &Path) -> Vec<Record> {
    std::fs::read_to_string(manifest_for(file))
        .map(|t| parse(&t))
        .unwrap_or_default()
}

/// Add or replace one record, preserving the rest.
pub fn write_record(file: &Path, record: Record) -> Result<(), String> {
    let path = manifest_for(file);
    let mut records = read_manifest(file);
    records.retain(|r| r.name != record.name);
    records.push(record);
    std::fs::write(&path, render(&records))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Hash a file, calling `progress` with bytes done so far.
///
/// **Streaming, in 8 MiB reads.** A 144 GB container cannot be held in memory,
/// and the read size is large enough that the hash rather than the syscall is the
/// cost.
pub fn hash_file(file: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<(String, u64), String> {
    let total = std::fs::metadata(file)
        .map_err(|e| format!("cannot stat {}: {e}", file.display()))?
        .len();
    let mut f =
        std::fs::File::open(file).map_err(|e| format!("cannot open {}: {e}", file.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 8 * 1024 * 1024];
    let mut done = 0u64;
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        done += n as u64;
        progress(done, total);
    }
    Ok((h.hex(), done))
}

/// Hash `file` and compare it with what is recorded, or record it.
///
/// `expect` is a publisher's digest when one is known, which makes the answer
/// stronger than trust-on-first-use.
pub fn verify(
    file: &Path,
    expect: Option<&str>,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Verdict, String> {
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // **Size first, because it is free.** A file of the wrong length cannot be the
    // right file, and saying so takes a millisecond rather than minutes.
    let found_size = std::fs::metadata(file)
        .map_err(|e| format!("cannot stat {}: {e}", file.display()))?
        .len();
    let recorded = read_manifest(file).into_iter().find(|r| r.name == name);
    if expect.is_none() {
        if let Some(r) = &recorded {
            if r.bytes != found_size {
                return Ok(Verdict::WrongSize {
                    expected: r.bytes,
                    found: found_size,
                });
            }
        }
    }

    let (hash, bytes) = hash_file(file, progress)?;
    let wanted = expect
        .map(|e| e.trim().to_ascii_lowercase())
        .or_else(|| recorded.as_ref().map(|r| r.sha256.clone()));

    match wanted {
        Some(w) if w == hash => Ok(Verdict::Matches {
            sha256: hash,
            bytes,
        }),
        Some(w) => Ok(Verdict::WrongHash {
            expected: w,
            found: hash,
        }),
        None => {
            write_record(
                file,
                Record {
                    sha256: hash.clone(),
                    bytes,
                    name,
                },
            )?;
            Ok(Verdict::Recorded {
                sha256: hash,
                bytes,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_round_trips() {
        let records = vec![
            Record {
                sha256: "a".repeat(64),
                bytes: 12,
                name: "b.gguf".into(),
            },
            Record {
                sha256: "b".repeat(64),
                bytes: 34,
                name: "a.gguf".into(),
            },
        ];
        let text = render(&records);
        let back = parse(&text);
        assert_eq!(back.len(), 2);
        // Sorted by name on write, so a.gguf comes first.
        assert_eq!(back[0].name, "a.gguf");
        assert_eq!(back[1].name, "b.gguf");
        assert_eq!(back[1].bytes, 12);
    }

    /// **A damaged manifest must not stop a model loading**, so bad lines are
    /// skipped rather than fatal.
    #[test]
    fn rubbish_lines_are_skipped_not_fatal() {
        let text = concat!(
            "# a comment\n",
            "\n",
            "tooshort 12 a.gguf\n",
            "not-hex-not-hex-not-hex-not-hex-not-hex-not-hex-not-hex-not-hexx 12 b.gguf\n",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa notanumber c.gguf\n",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 99\n",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc 42 good.gguf\n",
        );
        let records = parse(text);
        assert_eq!(records.len(), 1, "{records:?}");
        assert_eq!(records[0].name, "good.gguf");
        assert_eq!(records[0].bytes, 42);
    }

    /// A name with a space in it is one name, not two fields.
    #[test]
    fn a_file_name_may_contain_spaces() {
        let line = format!("{}  7  a model with spaces.gguf\n", "d".repeat(64));
        let r = parse(&line);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "a model with spaces.gguf");
    }

    /// An upper-case digest from a publisher must compare equal to ours.
    #[test]
    fn a_digest_is_compared_case_insensitively() {
        let upper = format!("{}  7  x.gguf\n", "AB".repeat(32));
        let r = parse(&upper);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].sha256, "ab".repeat(32));
    }

    #[test]
    fn a_verdict_knows_whether_to_trust_the_file() {
        assert!(Verdict::Matches {
            sha256: "x".into(),
            bytes: 1
        }
        .ok());
        assert!(Verdict::Recorded {
            sha256: "x".into(),
            bytes: 1
        }
        .ok());
        assert!(!Verdict::WrongSize {
            expected: 1,
            found: 2
        }
        .ok());
        assert!(!Verdict::WrongHash {
            expected: "a".into(),
            found: "b".into()
        }
        .ok());
    }
}
