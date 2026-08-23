//! Which image models are installed, and what each one is missing.
//!
//! # Why this is not `find::list`
//!
//! A language model is one container. **An image model is four**: a denoiser, a
//! separately trained unconditional twin of it for classifier-free guidance, a
//! text encoder that turns the prompt into hidden states, and an autoencoder
//! that turns the final latent into pixels. Listing those four as four models —
//! which is what `find::list` does, because they are four `.gguf` and
//! `.safetensors` files in the models directory — is the confusion behind
//! Atur's *"now i run to draw a image without select any model!! wtf is that"*.
//!
//! So this module groups them. The unit a user chooses is the **denoiser**;
//! everything else is a supporting part found by its role, and the text encoder
//! and autoencoder are shared between denoisers rather than owned by one.
//!
//! # Why the filename, and not the header
//!
//! `ideogram4-Q4_0.gguf` has **458 tensors and zero metadata keys** — no
//! `general.architecture`, no name, nothing. It is a bag of weights for another
//! engine's sampler, and there is no header field to dispatch on. The filename
//! is not a heuristic that could be replaced by reading the file properly; for
//! the denoisers it is the only signal that exists.
//!
//! That is also why this module is cheap: it is `read_dir` and string matching,
//! with no file ever opened. It runs on a tab switch and must stay that way.

use std::path::PathBuf;

/// One of the four parts a picture needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The diffusion transformer. This is the part a user chooses.
    Denoiser,
    /// Its unconditional twin, for classifier-free guidance.
    Uncond,
    /// The language model that turns the prompt into conditioning.
    TextEncoder,
    /// The autoencoder that turns the final latent into pixels.
    Autoencoder,
}

impl Role {
    /// What to call it in a sentence a user reads.
    pub fn label(self) -> &'static str {
        match self {
            Role::Denoiser => "denoiser",
            Role::Uncond => "unconditional twin",
            Role::TextEncoder => "text encoder",
            Role::Autoencoder => "autoencoder",
        }
    }

    /// The command that fetches one, so a missing part is an instruction rather
    /// than a complaint.
    pub fn how_to_get(self, family: &str) -> String {
        match self {
            Role::Denoiser => format!("chaos-pull {family}"),
            Role::Uncond => format!("chaos-pull {family}-uncond"),
            Role::TextEncoder => "chaos-pull qwen3-vl-8b".to_string(),
            Role::Autoencoder => "chaos-pull flux2-vae".to_string(),
        }
    }
}

/// A denoiser and the parts found to go with it.
#[derive(Debug, Clone)]
pub struct ImageModel {
    /// What the user picks it by: the denoiser's file stem, e.g.
    /// `ideogram4-Q4_0`.
    pub name: String,
    /// The family the denoiser belongs to, e.g. `ideogram4`. Two quants of one
    /// model share a family and so share an unconditional twin's naming.
    pub family: String,
    pub denoiser: PathBuf,
    pub uncond: Option<PathBuf>,
    pub text_encoder: Option<PathBuf>,
    pub autoencoder: Option<PathBuf>,
    /// Bytes across every part that was found.
    pub bytes: u64,
}

impl ImageModel {
    /// The roles with nothing to fill them.
    ///
    /// **Said before a run, not at step three of five.** Six hours into a draw
    /// is the wrong moment to discover the autoencoder was never downloaded.
    pub fn missing(&self) -> Vec<Role> {
        let mut m = Vec::new();
        if self.uncond.is_none() {
            m.push(Role::Uncond);
        }
        if self.text_encoder.is_none() {
            m.push(Role::TextEncoder);
        }
        if self.autoencoder.is_none() {
            m.push(Role::Autoencoder);
        }
        m
    }

    /// Whether a picture can actually be made with this.
    pub fn ready(&self) -> bool {
        self.missing().is_empty()
    }

    /// One line for a list: the name, then whether it can run.
    pub fn summary(&self) -> String {
        let missing = self.missing();
        if missing.is_empty() {
            format!("{} -- ready, {}", self.name, human(self.bytes))
        } else {
            let names: Vec<&str> = missing.iter().map(|r| r.label()).collect();
            format!("{} -- needs the {}", self.name, names.join(" and the "))
        }
    }
}

/// Does this filename fill a role, and for which family?
///
/// Returns the role and the family key. The family is what pairs a denoiser
/// with its twin: `ideogram4-Q4_0` and `ideogram4_uncond-Q4_0` are both
/// `ideogram4`, so the twin is found without either file naming the other.
pub fn role_of(file_name: &str) -> Option<(Role, String)> {
    let lower = file_name.to_ascii_lowercase();

    // The autoencoder first: it is the only `.safetensors` here, and testing it
    // before the `.gguf` rules keeps those rules from having to exclude it.
    if lower.ends_with(".safetensors") {
        return lower
            .contains("vae")
            .then(|| (Role::Autoencoder, "flux2".to_string()));
    }
    if !lower.ends_with(".gguf") {
        return None;
    }
    let stem = &lower[..lower.len() - ".gguf".len()];

    // **The twin is tested before the denoiser**, because its name contains the
    // denoiser's. `ideogram4_uncond-Q4_0` starts with `ideogram4`, so the other
    // order classifies every twin as a denoiser and the guidance pass silently
    // uses the conditional model twice.
    if let Some(cut) = stem.find("_uncond") {
        return Some((Role::Uncond, stem[..cut].to_string()));
    }

    // A text encoder is a language model, named as one. Qwen3-VL is what the
    // Ideogram pipeline was trained against; the check is deliberately narrow,
    // because calling an arbitrary chat model a text encoder produces a picture
    // that is wrong rather than an error that is clear.
    if stem.starts_with("qwen3-vl") {
        return Some((Role::TextEncoder, "qwen3vl".to_string()));
    }

    // A denoiser, by family. Table-driven so a new one is a line here rather
    // than a new branch; the containers carry no metadata to dispatch on.
    for family in DENOISER_FAMILIES {
        if stem.starts_with(family) {
            return Some((Role::Denoiser, (*family).to_string()));
        }
    }
    None
}

/// Denoiser families this engine has a sampler for.
///
/// Deliberately a closed list. An unrecognised `.gguf` is a language model far
/// more often than it is a diffusion transformer, and offering one as a
/// denoiser produces hours of work and a grey rectangle.
const DENOISER_FAMILIES: &[&str] = &["ideogram4"];

/// Every image model discoverable in `dirs`, best-formed first.
///
/// The text encoder and autoencoder are shared: whichever is found is offered
/// to every denoiser, because there is one of each and it is not owned by any
/// particular one of them.
pub fn installed(dirs: &[PathBuf]) -> Vec<ImageModel> {
    let mut denoisers: Vec<(String, String, PathBuf)> = Vec::new();
    let mut unconds: Vec<(String, PathBuf)> = Vec::new();
    let mut text_encoder: Option<PathBuf> = None;
    let mut autoencoder: Option<PathBuf> = None;

    for dir in dirs {
        // One level down as well, because a big model lives in its own folder
        // -- the same rule `find::scan_into` follows, and for the same reason.
        let mut roots = vec![dir.clone()];
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    roots.push(p);
                }
            }
        }
        for root in roots {
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            for e in entries.flatten() {
                let path = e.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some((role, family)) = role_of(name) else {
                    continue;
                };
                let stem = name.rsplit_once('.').map_or(name, |(s, _)| s).to_string();
                match role {
                    Role::Denoiser => denoisers.push((stem, family, path)),
                    Role::Uncond => unconds.push((family, path)),
                    // First found wins. There is one of each on a machine, and
                    // preferring the earlier search directory is the same
                    // precedence `model_dirs` already establishes.
                    Role::TextEncoder => {
                        text_encoder.get_or_insert(path);
                    }
                    Role::Autoencoder => {
                        autoencoder.get_or_insert(path);
                    }
                }
            }
        }
    }

    let mut out: Vec<ImageModel> = denoisers
        .into_iter()
        .map(|(name, family, denoiser)| {
            let uncond = unconds
                .iter()
                .find(|(f, _)| *f == family)
                .map(|(_, p)| p.clone());
            let mut m = ImageModel {
                name,
                family,
                denoiser,
                uncond,
                text_encoder: text_encoder.clone(),
                autoencoder: autoencoder.clone(),
                bytes: 0,
            };
            m.bytes = [
                Some(&m.denoiser),
                m.uncond.as_ref(),
                m.text_encoder.as_ref(),
                m.autoencoder.as_ref(),
            ]
            .into_iter()
            .flatten()
            .filter_map(|p| std::fs::metadata(p).ok().map(|md| md.len()))
            .sum();
            m
        })
        .collect();

    // Ready ones first, then by name: the list's job is to put a usable choice
    // under the cursor without anybody reading it.
    out.sort_by(|a, b| {
        b.ready()
            .cmp(&a.ready())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out.dedup_by(|a, b| a.name == b.name);
    out
}

/// The model to use when the user has not said, or `None` if there is none.
///
/// The first ready one. **Not the first one found**: offering a denoiser whose
/// autoencoder is missing as the default means the default cannot draw.
pub fn best(dirs: &[PathBuf]) -> Option<ImageModel> {
    installed(dirs).into_iter().find(ImageModel::ready)
}

/// One image model by name, matched on the denoiser's stem.
pub fn by_name(dirs: &[PathBuf], name: &str) -> Option<ImageModel> {
    let want = name.to_ascii_lowercase();
    let all = installed(dirs);
    all.iter()
        .find(|m| m.name.to_ascii_lowercase() == want)
        // A prefix is enough for a person typing at a prompt: `ideogram4` finds
        // `ideogram4-Q4_0` when there is only one of them.
        .or_else(|| {
            all.iter()
                .find(|m| m.name.to_ascii_lowercase().starts_with(&want))
        })
        .cloned()
}

fn human(bytes: u64) -> String {
    const K: f64 = 1000.0;
    let b = bytes as f64;
    if b >= K * K * K {
        format!("{:.1} GB", b / (K * K * K))
    } else if b >= K * K {
        format!("{:.0} MB", b / (K * K))
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The twin must not be read as a denoiser.** Its name contains the
    /// denoiser's, so a `starts_with` test in the wrong order classifies it as
    /// one -- and the failure is silent: guidance runs the conditional model
    /// twice, producing a picture that is merely worse rather than an error.
    #[test]
    fn the_unconditional_twin_is_not_a_denoiser() {
        assert_eq!(
            role_of("ideogram4_uncond-Q4_0.gguf"),
            Some((Role::Uncond, "ideogram4".into()))
        );
        assert_eq!(
            role_of("ideogram4-Q4_0.gguf"),
            Some((Role::Denoiser, "ideogram4".into()))
        );
    }

    /// The pair share a family, which is how the twin is found without either
    /// file naming the other.
    #[test]
    fn a_denoiser_and_its_twin_share_a_family() {
        let (_, a) = role_of("ideogram4-Q4_0.gguf").unwrap();
        let (_, b) = role_of("ideogram4_uncond-Q4_0.gguf").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn the_other_two_roles_are_recognised() {
        assert_eq!(
            role_of("flux2-vae.safetensors").map(|r| r.0),
            Some(Role::Autoencoder)
        );
        assert_eq!(
            role_of("Qwen3-VL-8B-Instruct-Q4_K_M.gguf").map(|r| r.0),
            Some(Role::TextEncoder)
        );
    }

    /// A chat model is not a denoiser and must not be offered as one: doing so
    /// costs hours and produces a grey rectangle rather than an error.
    #[test]
    fn an_ordinary_language_model_fills_no_role() {
        assert_eq!(role_of("Qwen3-14B-Q4_K_M.gguf"), None);
        assert_eq!(role_of("gemma-3-27b-it-Q4_K_M.gguf"), None);
        assert_eq!(role_of("phi-4-Q4_K_M.gguf"), None);
        assert_eq!(role_of("lora.safetensors"), None);
        assert_eq!(role_of("notes.txt"), None);
    }

    /// A model missing a part says which part, and how to get it.
    #[test]
    fn a_missing_part_is_named_with_its_command() {
        let m = ImageModel {
            name: "ideogram4-Q4_0".into(),
            family: "ideogram4".into(),
            denoiser: "d.gguf".into(),
            uncond: Some("u.gguf".into()),
            text_encoder: None,
            autoencoder: None,
            bytes: 0,
        };
        assert!(!m.ready());
        assert_eq!(m.missing(), vec![Role::TextEncoder, Role::Autoencoder]);
        assert!(m.summary().contains("text encoder"));
        assert!(m.summary().contains("autoencoder"));
        assert_eq!(
            Role::Autoencoder.how_to_get("ideogram4"),
            "chaos-pull flux2-vae"
        );
        assert_eq!(
            Role::Uncond.how_to_get("ideogram4"),
            "chaos-pull ideogram4-uncond"
        );
    }

    /// Discovery over a real directory: four files in, one model out.
    #[test]
    fn four_files_group_into_one_model() {
        let dir = std::env::temp_dir().join("chaos-image-installed-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "ideogram4-Q4_0.gguf",
            "ideogram4_uncond-Q4_0.gguf",
            "Qwen3-VL-8B-Instruct-Q4_K_M.gguf",
            "flux2-vae.safetensors",
            // Noise that must not become a model, or a part of one.
            "Qwen3-14B-Q4_K_M.gguf",
            "readme.txt",
        ] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }

        let all = installed(std::slice::from_ref(&dir));
        assert_eq!(all.len(), 1, "four files are one model, not four");
        let m = &all[0];
        assert_eq!(m.name, "ideogram4-Q4_0");
        assert!(m.ready(), "{}", m.summary());
        assert!(m.uncond.is_some());
        assert!(m.text_encoder.is_some());
        assert!(m.autoencoder.is_some());
        assert!(best(std::slice::from_ref(&dir)).is_some());
        assert_eq!(
            by_name(std::slice::from_ref(&dir), "ideogram4").map(|m| m.name),
            Some("ideogram4-Q4_0".into())
        );
        assert!(by_name(std::slice::from_ref(&dir), "nothing-like-this").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A denoiser with no autoencoder must not be the default.** The default
    /// is what a user gets by pressing DRAW without reading anything, and one
    /// that cannot draw is worse than an empty list.
    #[test]
    fn the_default_is_a_model_that_can_actually_draw() {
        let dir = std::env::temp_dir().join("chaos-image-installed-partial");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ideogram4-Q4_0.gguf"), b"x").unwrap();

        let all = installed(std::slice::from_ref(&dir));
        assert_eq!(all.len(), 1);
        assert!(!all[0].ready());
        assert!(
            best(std::slice::from_ref(&dir)).is_none(),
            "a model missing three parts is not a default"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
