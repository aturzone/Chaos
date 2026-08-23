//! Is this container all the way downloaded?
//!
//! # Why a separate check
//!
//! A `.gguf` on disk looks finished. It has the right name, it has a valid
//! header -- the header is the first thing written -- and it appears in the
//! model list beside models that work. What it does not have is the second half
//! of its weights, because the download was interrupted, and the only way a
//! user finds out is by loading it and reading whatever error the engine
//! happens to produce three seconds later.
//!
//! That happened here: four models were listed as installed while two of them
//! were a tenth of their real size, and the app reported them exactly as it
//! reported the working ones.
//!
//! # How it knows
//!
//! [`chaos_gguf::Gguf::expected_file_bytes`] reads the container's own tensor
//! index and returns where the last tensor ends. A file shorter than that is
//! *provably* truncated -- no catalogue, no network, no guess about what the
//! model was supposed to weigh. A file longer is fine: ggml pads the data
//! section up to the alignment.
//!
//! Every shard is checked separately, because each carries its own index and an
//! interrupted multi-part fetch stops in the middle of exactly one of them.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// Where the header read starts, and where it gives up.
///
/// **Grown rather than fixed, because this runs over every model in the list.**
/// A dense container's header is well under a megabyte; a Qwen tokenizer with
/// 248,000 tokens and a MoE index with thousands of expert tensors is what
/// pushes it into the megabytes. Reading 32 MB of every model on a rescan is
/// most of a second of disk for nothing, so the first attempt reads 4 MB and
/// only a parse failure doubles it.
const HEAD_START: usize = 4 * 1024 * 1024;
const HEAD_MAX: usize = 64 * 1024 * 1024;

/// One shard that is shorter than its own index says it should be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub path: PathBuf,
    /// Bytes on disk now.
    pub have: u64,
    /// Bytes the container's index requires.
    pub want: u64,
}

impl Missing {
    pub fn short_by(&self) -> u64 {
        self.want.saturating_sub(self.have)
    }
}

/// Every shard of the container at `first` that is not fully written.
///
/// Empty means every shard is at least as long as its index requires, which is
/// the strongest statement that can be made without reading the weights
/// themselves -- for *that*, see [`crate::validate`].
///
/// A shard whose header cannot be parsed is **not** reported here. It is either
/// not a GGUF at all or truncated inside the header, and both are the opening
/// error's business; inventing a byte count for it would be a guess.
pub fn missing(first: &Path) -> Vec<Missing> {
    let mut out = Vec::new();
    for shard in crate::discover::discover_shards(first) {
        let Ok(have) = std::fs::metadata(&shard).map(|m| m.len()) else {
            continue;
        };
        let Some(want) = expected_bytes_cached(&shard) else {
            continue;
        };
        if have < want {
            out.push(Missing {
                path: shard,
                have,
                want,
            });
        }
    }
    out
}

/// What has already been read, so a rescan does not read it again.
///
/// **This was 99.8% of a tab switch.** Measured on this machine with 39 models
/// installed: `find::list()` 3.7 ms, `why_incomplete` across all of them
/// **1885 ms** -- because each one opens every shard and parses up to 4 MB of
/// header. The app calls it on every switch between INSTALLED and AVAILABLE, on
/// the UI thread, so the window froze for a second and a half each time. Atur:
/// *"when i switch between available and installed models installed models load
/// with lag and make problem"*.
///
/// **The key is the file's own identity, not its name.** Length and modified
/// time together: a container cannot gain the bytes it was missing, or lose the
/// ones it had, without changing both. A download in flight changes them
/// continuously, which is exactly right -- it is re-read until it stops moving.
/// Caching on the path alone would freeze the verdict of a file being written.
///
/// `None` is cached as eagerly as a length. A file that will not parse costs
/// *more* than one that will -- the read doubles from 4 MB to 64 MB before
/// giving up -- so the failures are the ones most worth remembering.
type Fingerprint = (PathBuf, u64, Option<SystemTime>);

static EXPECTED: Mutex<Vec<(Fingerprint, Option<u64>)>> = Mutex::new(Vec::new());

/// Above this the cache is emptied rather than searched.
///
/// A linear scan of a few dozen entries is faster than hashing them, and a
/// models directory pointed at a whole drive is the only way to exceed this.
/// Dropping everything is correct if crude: the next scan pays what the first
/// one paid, which is the behaviour before this cache existed.
const CACHE_MAX: usize = 512;

fn fingerprint(path: &Path) -> Fingerprint {
    let m = std::fs::metadata(path).ok();
    (
        path.to_path_buf(),
        m.as_ref().map(|m| m.len()).unwrap_or(0),
        m.and_then(|m| m.modified().ok()),
    )
}

/// `expected_bytes`, answered from memory when the file has not changed.
fn expected_bytes_cached(path: &Path) -> Option<u64> {
    let key = fingerprint(path);
    if let Ok(c) = EXPECTED.lock() {
        if let Some((_, v)) = c.iter().find(|(k, _)| *k == key) {
            return *v;
        }
    }
    let v = expected_bytes(path);
    if let Ok(mut c) = EXPECTED.lock() {
        if c.len() >= CACHE_MAX {
            c.clear();
        }
        c.push((key, v));
    }
    v
}

/// What one shard's own index says its length must be.
///
/// Returns `None` for anything that will not parse at all, which includes a
/// file truncated inside its own header -- that is the opener's error to
/// report, and a length invented for it would be a guess.
fn expected_bytes(path: &Path) -> Option<u64> {
    let on_disk = std::fs::metadata(path).ok()?.len() as usize;
    let mut want = HEAD_START;
    loop {
        let take = want.min(on_disk);
        let mut f = std::fs::File::open(path).ok()?;
        let mut buf = vec![0u8; take];
        let n = read_up_to(&mut f, &mut buf)?;
        buf.truncate(n);
        if let Ok(g) = chaos_gguf::Gguf::parse(&buf) {
            return Some(g.expected_file_bytes());
        }
        // Either the header is longer than what was read, or the file is not a
        // container. Reading the whole of it once distinguishes the two.
        if take >= on_disk || want >= HEAD_MAX {
            return None;
        }
        want *= 2;
    }
}

/// Fill as much of `buf` as the file has. `read` may return short.
fn read_up_to(f: &mut std::fs::File, buf: &mut [u8]) -> Option<usize> {
    use std::io::Read;
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    Some(n)
}

/// The sentence to show a user, or `None` if the container is whole.
///
/// Says what is missing in bytes rather than "corrupt", because the fix is to
/// finish the download and the number is what tells them how long that takes.
pub fn why_incomplete(first: &Path) -> Option<String> {
    let m = missing(first);
    if m.is_empty() {
        return None;
    }
    let short: u64 = m.iter().map(Missing::short_by).sum();
    let want: u64 = m.iter().map(|s| s.want).sum();
    let have: u64 = m.iter().map(|s| s.have).sum();
    let pct = (have * 100).checked_div(want).unwrap_or(0);
    if m.len() == 1 {
        Some(format!(
            "the download did not finish -- {} is {} short of the {} its own index requires ({pct}% written)",
            m[0].path.file_name().unwrap_or_default().to_string_lossy(),
            human(short),
            human(m[0].want),
        ))
    } else {
        Some(format!(
            "the download did not finish -- {} shards are {} short between them ({pct}% written)",
            m.len(),
            human(short),
        ))
    }
}

/// Bytes as a person writes them. Local to this module so it stays usable from
/// the command line tools, which do not depend on the app.
fn human(bytes: u64) -> String {
    const K: f64 = 1000.0;
    let b = bytes as f64;
    let (v, unit) = if b >= K * K * K {
        (b / (K * K * K), "GB")
    } else if b >= K * K {
        (b / (K * K), "MB")
    } else if b >= K {
        (b / K, "kB")
    } else {
        return format!("{bytes} B");
    };
    if v < 10.0 {
        format!("{v:.2} {unit}")
    } else if v < 100.0 {
        format!("{v:.1} {unit}")
    } else {
        format!("{v:.0} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_a_container_is_not_called_incomplete() {
        let dir = std::env::temp_dir().join("chaos-complete-test-notgguf");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("x.gguf");
        std::fs::write(&p, b"not a gguf at all").unwrap();
        // Unparseable is the opener's problem, not this module's: it must not
        // invent a length for a file it cannot read.
        assert!(missing(&p).is_empty());
        assert!(why_incomplete(&p).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_reports_nothing_rather_than_panicking() {
        let p = std::env::temp_dir().join("chaos-does-not-exist-9e1f.gguf");
        assert!(missing(&p).is_empty());
    }

    #[test]
    fn the_shortfall_is_the_difference() {
        let m = Missing {
            path: "x".into(),
            have: 900,
            want: 9_000,
        };
        assert_eq!(m.short_by(), 8_100);
    }

    /// A shard longer than its index requires is padding, not a problem.
    #[test]
    fn longer_than_required_is_not_short() {
        let m = Missing {
            path: "x".into(),
            have: 9_000,
            want: 8_000,
        };
        assert_eq!(m.short_by(), 0);
    }

    /// **The cache's whole correctness argument is its key.** A container
    /// cannot gain the bytes it was missing, or lose the ones it had, without
    /// its length or its modified time changing -- so a fingerprint tracking
    /// both is safe to trust. One that tracked only the path would freeze the
    /// verdict of a download still in flight, which is precisely the file a
    /// user most needs the truth about.
    ///
    /// This checks the key, not the timing: a test that asserted "the second
    /// call was faster" would pass on a machine with a warm page cache no
    /// matter what this module did.
    #[test]
    fn the_fingerprint_moves_when_the_file_does() {
        let dir = std::env::temp_dir().join("chaos-complete-test-fingerprint");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("growing.gguf");

        std::fs::write(&p, b"half a download").unwrap();
        let before = fingerprint(&p);

        std::fs::write(&p, b"half a download, and then the rest of it").unwrap();
        let after = fingerprint(&p);

        assert_ne!(
            before, after,
            "a file that grew must not be answered from the cache"
        );
        assert_ne!(before.1, after.1, "length alone separates these two");

        // Untouched, it fingerprints identically -- otherwise the cache would
        // never hit and the 1885 ms this exists to remove would be back.
        assert_eq!(after, fingerprint(&p));

        // A file that is not there at all still yields a key rather than
        // panicking, so a model deleted between the scan and the check is a
        // cache miss and not a crash.
        std::fs::remove_file(&p).unwrap();
        assert_ne!(fingerprint(&p), after);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The cached answer must be the answer, not merely a fast one.
    ///
    /// A file that will not parse is the case worth pinning: it costs *more*
    /// than one that will, because the read doubles from 4 MB to 64 MB before
    /// giving up, so it is both the most valuable thing to remember and the
    /// easiest to get wrong by caching only successes.
    #[test]
    fn a_repeat_call_gives_the_same_verdict() {
        let dir = std::env::temp_dir().join("chaos-complete-test-repeat");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("x.gguf");
        std::fs::write(&p, b"not a gguf at all").unwrap();

        let first = expected_bytes_cached(&p);
        let second = expected_bytes_cached(&p);
        assert_eq!(first, second);
        assert_eq!(first, None, "unparseable has no length to report");
        assert_eq!(missing(&p), missing(&p));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bytes_read_the_way_a_person_writes_them() {
        assert_eq!(human(0), "0 B");
        assert_eq!(human(8_100), "8.10 kB");
        assert_eq!(human(9_000_000_000), "9.00 GB");
    }
}
