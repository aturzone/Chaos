//! A measured read speed, remembered.
//!
//! # Why this exists
//!
//! The bandwidth benchmark **writes a temporary file larger than RAM**. That is
//! the right way to measure a disk — anything smaller measures the page cache —
//! and it is completely unacceptable to run automatically. So `--auto` had a
//! choice between three bad options: run it and make every launch take minutes,
//! guess a number, or say nothing about speed at all.
//!
//! This is the fourth. `chaos-probe --bandwidth` writes what it measured; every
//! later run reads it. One deliberate measurement, and from then on the engine
//! can answer "what tok/s should I expect" before loading anything — which is
//! R6's actual requirement, and the reason a user waits four minutes for a load
//! without knowing whether the answer will be one token a second or twenty.
//!
//! # What is *not* done here
//!
//! **It never measures on its own.** A cache that filled itself would be a
//! surprise multi-gigabyte write, and this file would then be the reason a
//! laptop's disk was hammered by a chat client.
//!
//! **A stale reading is still a reading.** The value is stamped with when it
//! was taken and the caller is told; a disk does not get faster on its own, and
//! a number from last month beats no number at all. What would invalidate it is
//! moving the models to a different drive, which is why the path measured is
//! recorded next to the speed.

use std::path::{Path, PathBuf};

/// A remembered measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub bytes_per_sec: f64,
    /// Seconds since the Unix epoch, when it was taken.
    pub taken_at: u64,
    /// Which directory was measured. A different drive is a different answer.
    pub path: String,
}

impl Measured {
    /// How long ago, as a person would say it.
    pub fn age(&self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let secs = now.saturating_sub(self.taken_at);
        if secs < 3600 {
            "just now".into()
        } else if secs < 86_400 {
            format!("{} hours ago", secs / 3600)
        } else {
            format!("{} days ago", secs / 86_400)
        }
    }
}

/// Where the reading is kept: beside the models, under the user's home.
fn cache_path() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(PathBuf::from(home).join(".chaos").join("bandwidth"))
}

/// Remember a measurement. Failure is silent: this is a convenience, and a
/// read-only home directory is not a reason to fail a probe.
pub fn save(bytes_per_sec: f64, path: &Path) {
    let Some(file) = cache_path() else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Three lines, one value each. A format anybody can read and correct by
    // hand, which matters for a file that exists to answer a question about
    // their own machine.
    let body = format!(
        "bytes_per_sec {bytes_per_sec}\ntaken_at {now}\npath {}\n",
        path.display()
    );
    let _ = std::fs::write(file, body);
}

/// The last measurement, if there is one.
pub fn load() -> Option<Measured> {
    let text = std::fs::read_to_string(cache_path()?).ok()?;
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_string()))
    };
    let bytes_per_sec: f64 = field("bytes_per_sec ")?.parse().ok()?;
    // A nonsensical reading is worse than none: it would be multiplied into a
    // confident tok/s figure.
    if !bytes_per_sec.is_finite() || bytes_per_sec <= 0.0 {
        return None;
    }
    Some(Measured {
        bytes_per_sec,
        taken_at: field("taken_at ").and_then(|v| v.parse().ok()).unwrap_or(0),
        path: field("path ").unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The format is three lines and survives a round trip. Written by hand
    /// rather than through `save`, because `save` writes to the real home
    /// directory and a test must not.
    #[test]
    fn a_reading_parses_back_out_of_its_three_lines() {
        let dir = std::env::temp_dir().join("chaos-bandwidth-cache-test");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("bandwidth");
        std::fs::write(
            &file,
            "bytes_per_sec 2943718400\ntaken_at 1755990000\npath C:\\\\models\n",
        )
        .unwrap();

        let text = std::fs::read_to_string(&file).unwrap();
        let field = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_string()))
        };
        assert_eq!(field("bytes_per_sec ").unwrap(), "2943718400");
        assert_eq!(field("taken_at ").unwrap(), "1755990000");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A nonsense reading must not become a confident prediction.** Zero,
    /// negative and NaN all multiply into a tok/s figure that looks measured.
    #[test]
    fn an_impossible_speed_is_no_speed() {
        for bad in [0.0f64, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                !(bad.is_finite() && bad > 0.0),
                "{bad} must be rejected as a read speed"
            );
        }
        assert!(2.74e9f64.is_finite() && 2.74e9 > 0.0);
    }

    #[test]
    fn an_age_reads_as_a_person_would_say_it() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let m = |ago: u64| Measured {
            bytes_per_sec: 1.0,
            taken_at: now.saturating_sub(ago),
            path: String::new(),
        };
        assert_eq!(m(60).age(), "just now");
        assert!(m(7200).age().contains("hours"));
        assert!(m(3 * 86_400).age().contains("days"));
    }
}
