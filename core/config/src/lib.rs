//! What Chaos remembers between runs, for every tier that has to agree about it.
//!
//! **This crate exists because the window and the command line disagreed.**
//! `Settings` lived inside `gui/app`, so `~/.chaos/settings.txt` was a file the
//! app wrote and no command-line tool could read. Atur's plan states the
//! consequence plainly: *"pick a model, set device, threads, context, cache from
//! one place. The flags exist across `chaos-run` and `chaos-serve` and disagree
//! in places; the app has a settings file the CLI cannot read."*
//!
//! Nothing about that file was ever graphical. It moved here unchanged — same
//! format, same path, same hand-rolled parser, same preserved unknown keys — and
//! `gui/app` now re-exports this crate so every existing `settings::` call site
//! still means what it did.
//!
//! **No dependencies, and no ggml.** This is `std` and a `BTreeMap`, so it
//! builds on a machine that has never compiled a line of C, which is the half of
//! the build CI checks separately.

/// Which way round the palette runs.
///
/// **Here rather than in the app's `theme` module, because it is persisted.**
/// `mode = light` is a line in the settings file, so its spelling and its parser
/// belong beside every other line's. The palette that reads it is still the
/// app's business; the fact that it survives a restart is this crate's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Hermes' default, and this app's: a light ground with near-black text.
    Light,
    Dark,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Mode::Light => Mode::Dark,
            Mode::Dark => Mode::Light,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Mode::Light),
            "dark" => Some(Mode::Dark),
            _ => None,
        }
    }
}

mod settings;

pub use settings::{new_key, path, Role, Settings};

#[cfg(test)]
mod mode_tests {
    use super::*;

    /// A mode survives the file, and an unknown one keeps the default rather
    /// than inventing one.
    #[test]
    fn a_mode_round_trips_and_a_bad_one_does_not_panic() {
        for m in [Mode::Light, Mode::Dark] {
            assert_eq!(Mode::parse(m.as_str()), Some(m));
            assert_eq!(m.toggled().toggled(), m);
            assert_ne!(m.toggled(), m);
        }
        assert_eq!(Mode::parse("LIGHT"), Some(Mode::Light));
        assert_eq!(Mode::parse("  dark  "), Some(Mode::Dark));
        assert_eq!(Mode::parse("chartreuse"), None);
    }
}
