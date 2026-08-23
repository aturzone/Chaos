//! What the app knows about the models on the machine.
//!
//! The discovery itself is `chaos_model::find`, shared with `chaos-run` and
//! `chaos-serve` so the three cannot disagree about where models live -- that
//! disagreement was a real bug once, when the installer created one directory
//! and the downloader wrote to another.

use std::path::PathBuf;

/// What a container is *for*.
///
/// **A chat model and an image model in one flat list is a real confusion**,
/// not a cosmetic one: Atur pressed DRAW with no image model installed and
/// pressed LOAD on a denoiser, and in both cases the list had told him they
/// were the same kind of thing. Atur: *"list of model better management and
/// sort and structured for users"*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A language model. It loads, and you talk to it.
    Chat,
    /// One of the four parts an image needs. It does not load and there is no
    /// token loop to run on it -- it is used by `chaos-draw` on the IMAGE page.
    ImagePart,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Chat => "chat",
            Kind::ImagePart => "image",
        }
    }
}

/// What a file on disk is for, from its name.
///
/// Asks `chaos_model::image` rather than keeping a second list of image-model
/// names here: two lists that must agree is how the installer and the
/// downloader once ended up writing to different directories.
pub fn kind_of(path: &std::path::Path) -> Kind {
    let named = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if chaos_model::image::role_of(named).is_some() {
        Kind::ImagePart
    } else {
        Kind::Chat
    }
}

/// How the list is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Name,
    /// Largest first: the question a size sort answers is "what is eating the
    /// disk", and that is the top of the list, not the bottom.
    Size,
    /// Chat models first, then image parts, each by name.
    Kind,
}

impl Sort {
    pub const ALL: [Sort; 3] = [Sort::Name, Sort::Size, Sort::Kind];
    pub fn label(self) -> &'static str {
        match self {
            Sort::Name => "by name",
            Sort::Size => "by size",
            Sort::Kind => "by what it is",
        }
    }
}

/// Which kinds the list is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Chat,
    Image,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::All, Filter::Chat, Filter::Image];
    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "everything",
            Filter::Chat => "chat models",
            Filter::Image => "image models",
        }
    }
    pub fn admits(self, k: Kind) -> bool {
        match self {
            Filter::All => true,
            Filter::Chat => k == Kind::Chat,
            Filter::Image => k == Kind::ImagePart,
        }
    }
}

/// Which entries to show, in which order, as indices into `entries`.
///
/// **Indices, not a copy.** The list box holds rows and the rest of the window
/// holds entries; something has to map a clicked row back to the model it
/// names, and a filtered list that forgot to do that would load the wrong
/// model -- silently, because both are valid containers.
pub fn arrange(entries: &[Entry], search: &str, sort: Sort, filter: Filter) -> Vec<usize> {
    let needle = search.trim().to_lowercase();
    let mut idx: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| filter.admits(e.kind))
        .filter(|(_, e)| needle.is_empty() || e.label.to_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect();
    match sort {
        Sort::Name => idx.sort_by_key(|&i| entries[i].label.to_lowercase()),
        Sort::Size => idx.sort_by_key(|&i| std::cmp::Reverse(entries[i].bytes.unwrap_or(0))),
        Sort::Kind => idx.sort_by_key(|&i| {
            (
                // Chat first: it is what most people came for.
                entries[i].kind != Kind::Chat,
                entries[i].label.to_lowercase(),
            )
        }),
    }
    idx
}

/// A model as the list shows it.
pub struct Entry {
    pub label: String,
    pub path: PathBuf,
    /// Total bytes across every shard, or `None` if it could not be measured.
    pub bytes: Option<u64>,
    /// Why this container cannot be loaded, if it cannot.
    ///
    /// **A half-downloaded model looks exactly like a finished one in a list**
    /// -- right name, right extension, valid header -- and the only way a user
    /// found out was to press LOAD and read whatever the engine said three
    /// seconds later. Two of the models on this machine were in that state.
    pub incomplete: Option<String>,
    /// Whether this is something to chat with or a part of an image pipeline.
    pub kind: Kind,
}

/// Human-readable size, at the precision the number deserves.
///
/// Two decimals below 10, one below 100, none above: "144 GB" is more useful
/// than "144.42 GB", and "9.34 GB" is more useful than "9 GB".
pub fn human_size(bytes: u64) -> String {
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

/// Where the app puts what it downloads.
///
/// The same `~/.chaos/models` the installer creates and `find` searches first
/// after `CHAOS_MODELS`, so a download appears in the list without any
/// configuration. Getting this wrong once already cost a release: the installer
/// made one directory and the downloader wrote to another.
pub fn default_dir() -> PathBuf {
    // The *first* place `chaos_model::find` looks, so a download lands where
    // the list will show it. Asking `find` rather than re-deriving the order is
    // the point: the two disagreeing is the bug this doc comment describes, and
    // a second copy of the rule is how it came back.
    chaos_model::find::model_dirs()
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("models"))
}

/// Every shard of a split container, so the size is the model's and not
/// shard one's. `find` reports the first shard; the rest sit beside it.
fn total_bytes(first: &std::path::Path) -> Option<u64> {
    let name = first.file_name()?.to_str()?;
    let dir = first.parent()?;
    // `-00001-of-00005.gguf` -> count them all. Anything else is one file.
    let Some(idx) = name.rfind("-00001-of-") else {
        return std::fs::metadata(first).ok().map(|m| m.len());
    };
    let stem = &name[..idx];
    let mut total = 0u64;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let n = e.file_name();
        let Some(n) = n.to_str() else { continue };
        if n.starts_with(stem) && n.ends_with(".gguf") {
            if let Ok(m) = e.metadata() {
                total += m.len();
            }
        }
    }
    (total > 0).then_some(total)
}

/// Everything discoverable, ready to display.
pub fn list() -> Vec<Entry> {
    chaos_model::find::list()
        .into_iter()
        .map(|f| Entry {
            bytes: total_bytes(&f.path),
            incomplete: chaos_model::complete::why_incomplete(&f.path),
            kind: kind_of(&f.path),
            label: f.label,
            path: f.path,
        })
        .collect()
}

/// The separator between a row's columns.
///
/// A control character, so it cannot occur in a filename and cannot be typed by
/// accident. The list is owner-drawn and splits on it to right-align the size.
pub const COLUMN_SEP: char = '\u{1}';

/// One line for the list box, its columns joined.
pub fn row(e: &Entry) -> String {
    columns(e).join(&COLUMN_SEP.to_string())
}

/// A row as its parts: the name, then what is known about it.
///
/// **Separate columns, because one string truncates from the wrong end.** Built
/// as `name + "   " + size` and drawn with an ellipsis, a narrow list eats the
/// *end of the name* — and the end of the name is the quantisation, which is
/// the part that tells two copies of a model apart. `Qwen3-VL-8B-Instruct-Q4_K_M`
/// became `Qwen3-VL-8B-Instru…`, so the list stopped answering the one question
/// it exists to answer. Now the name gets the width it needs and the size is
/// right-aligned into its own column.
pub fn columns(e: &Entry) -> Vec<String> {
    let mut v = vec![e.label.clone()];
    if let Some(b) = e.bytes {
        v.push(human_size(b));
    }
    // **Said in the row, not only in a filter.** A filter answers the question
    // for somebody who already knew to ask it; the row answers it for somebody
    // about to press LOAD on an autoencoder.
    if e.kind == Kind::ImagePart {
        v.push("image".to_string());
    }
    // Said in the list, not only on the model's own page: the list is where the
    // choice is made, and "9.00 GB" beside a file holding 911 MB is a lie.
    if e.incomplete.is_some() {
        v.push("(unfinished)".to_string());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A row's columns are separate, so a narrow list eats a measurement rather
    /// than the end of the name.
    #[test]
    fn the_name_is_its_own_column() {
        let e = Entry {
            label: "Qwen3-VL-8B-Instruct-Q4_K_M".into(),
            path: std::path::PathBuf::from("x.gguf"),
            bytes: Some(5_027_785_568),
            incomplete: None,
            kind: Kind::Chat,
        };
        let c = columns(&e);
        assert_eq!(c[0], "Qwen3-VL-8B-Instruct-Q4_K_M", "the name, whole");
        assert_eq!(c[1], "5.03 GB");
        // The separator cannot appear in a filename, so splitting is safe.
        assert!(!c[0].contains(COLUMN_SEP));
        assert_eq!(row(&e).split(COLUMN_SEP).next().unwrap(), c[0]);

        // An unfinished download says so in its own column too.
        let e = Entry {
            incomplete: Some("half".into()),
            ..e
        };
        let c = columns(&e);
        assert_eq!(c.len(), 3);
        assert_eq!(c[2], "(unfinished)");

        // A model whose size could not be measured has one column and no
        // stray separator to split on.
        let e = Entry {
            bytes: None,
            incomplete: None,
            ..e
        };
        assert_eq!(columns(&e).len(), 1);
        assert!(!row(&e).contains(COLUMN_SEP));
    }

    fn entry(label: &str, bytes: u64, kind: Kind) -> Entry {
        Entry {
            label: label.into(),
            path: format!("{label}.gguf").into(),
            bytes: Some(bytes),
            incomplete: None,
            kind,
        }
    }

    /// An image part must be recognisable as one from its row alone.
    #[test]
    fn an_image_part_says_so_in_its_row() {
        let e = entry("ideogram4-Q4_0", 5_643_820_832, Kind::ImagePart);
        assert!(row(&e).contains("image"));
        let e = entry("Qwen3-14B-Q4_K_M", 9_000_000_000, Kind::Chat);
        assert!(!row(&e).contains("image"));
    }

    /// What a file is for comes from `chaos_model::image`, so the app and the
    /// drawing code cannot disagree about which files are image parts.
    #[test]
    fn the_kind_follows_the_filename() {
        use std::path::Path;
        assert_eq!(kind_of(Path::new("m/ideogram4-Q4_0.gguf")), Kind::ImagePart);
        assert_eq!(kind_of(Path::new("m/flux2-vae.safetensors")), Kind::ImagePart);
        assert_eq!(kind_of(Path::new("m/Qwen3-14B-Q4_K_M.gguf")), Kind::Chat);
    }

    /// **The mapping is the whole point.** A filtered list that returned the
    /// clicked *row* rather than the entry it names would load a different
    /// model than the one pointed at -- silently, because both are containers.
    #[test]
    fn arranging_maps_rows_back_to_the_right_models() {
        let e = vec![
            entry("zeta-chat", 1_000, Kind::Chat),
            entry("ideogram4-Q4_0", 9_000, Kind::ImagePart),
            entry("alpha-chat", 5_000, Kind::Chat),
        ];

        // By name, everything.
        let got = arrange(&e, "", Sort::Name, Filter::All);
        assert_eq!(
            got.iter().map(|&i| e[i].label.as_str()).collect::<Vec<_>>(),
            ["alpha-chat", "ideogram4-Q4_0", "zeta-chat"]
        );

        // Largest first: a size sort answers "what is eating the disk".
        let got = arrange(&e, "", Sort::Size, Filter::All);
        assert_eq!(e[got[0]].label, "ideogram4-Q4_0");
        assert_eq!(e[got[2]].label, "zeta-chat");

        // Chat before image parts.
        let got = arrange(&e, "", Sort::Kind, Filter::All);
        assert_eq!(e[got[2]].kind, Kind::ImagePart);

        // Filtered, and the indices still point at the right entries.
        let got = arrange(&e, "", Sort::Name, Filter::Image);
        assert_eq!(got.len(), 1);
        assert_eq!(e[got[0]].label, "ideogram4-Q4_0");

        // Search is case-insensitive and matches anywhere in the name, because
        // the part of a name that tells two copies apart is at the end.
        let got = arrange(&e, "CHAT", Sort::Name, Filter::All);
        assert_eq!(got.len(), 2);
        let got = arrange(&e, "  q4_0 ", Sort::Name, Filter::All);
        assert_eq!(got.len(), 1);
        assert_eq!(e[got[0]].label, "ideogram4-Q4_0");

        // A search matching nothing is empty rather than everything: a filter
        // that falls back to "show all" tells the user their search worked.
        assert!(arrange(&e, "nothing-like-this", Sort::Name, Filter::All).is_empty());
    }

    #[test]
    fn sizes_read_the_way_a_person_would_write_them() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(999), "999 B");
        assert_eq!(human_size(1_500), "1.50 kB");
        assert_eq!(human_size(9_990_000_000), "9.99 GB");
        assert_eq!(human_size(17_300_000_000), "17.3 GB");
        assert_eq!(human_size(144_400_000_000), "144 GB");
    }

    /// The precision must change with the magnitude, or a 144 GB model reads
    /// as "144.42 GB" and a 9 GB one as "9 GB".
    #[test]
    fn precision_falls_as_the_number_grows() {
        assert_eq!(human_size(1_230_000_000).matches('.').count(), 1);
        assert_eq!(human_size(123_000_000_000).matches('.').count(), 0);
    }

    #[test]
    fn a_row_without_a_size_is_still_a_row() {
        let e = Entry {
            label: "qwen3".into(),
            path: "x".into(),
            bytes: None,
            incomplete: None,
            kind: Kind::Chat,
        };
        assert_eq!(row(&e), "qwen3");
    }

    /// The list must say so, because the list is where the choice is made.
    #[test]
    fn an_unfinished_download_says_so_in_the_row() {
        let e = Entry {
            label: "qwen3-14b".into(),
            path: "x".into(),
            bytes: Some(911_499_264),
            incomplete: Some("the download did not finish".into()),
            kind: Kind::Chat,
        };
        assert!(row(&e).contains("unfinished"));
    }
}
