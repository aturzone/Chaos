//! What the app remembers between runs.
//!
//! **The implementation moved to `chaos-config` and this is the door to it.**
//! The file was never graphical: `~/.chaos/settings.txt` is what the window
//! writes and what the command line now reads, and one file with two parsers
//! would be one parser too many. Atur's plan asks for exactly this -- *"pick a
//! model, set device, threads, context, cache from one place"*.
//!
//! Kept as a module rather than replaced by an import so that every
//! `settings::Settings`, `settings::Role` and `settings::new_key()` in this crate
//! still resolves. Nothing here has a body.

pub use chaos_config::{new_key, path, Mode, Role, Settings};
