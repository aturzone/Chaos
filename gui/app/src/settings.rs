//! What the app remembers between runs.
//!
//! Every field here was a box you had to retype each time the window opened,
//! which is not a settings page -- it is a form. They persist to a small text
//! file beside the models directory.
//!
//! **The format is `key = value`, one per line, and unknown keys are kept.**
//! A settings file that silently drops what it does not recognise makes a
//! downgrade destructive: run an older build once and the newer build's
//! preferences are gone. Parsing is hand-rolled because the workspace has no
//! serialisation crate and this is thirty lines.

use crate::theme::Mode;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Everything the window lets you change.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Expert cache budget in GiB. `None` means "let the engine measure".
    pub cache_gib: Option<f64>,
    /// Generation threads. `None` means measured.
    pub threads: Option<u32>,
    /// Prefill threads, which want the opposite of generation threads.
    pub threads_batch: Option<u32>,
    /// Where the local server listens.
    pub port: u16,
    /// Context cap, in tokens. `None` means the model's own limit.
    pub context: Option<u32>,
    /// Layers to put on the GPU. `None` means none; `Some(99)` means all.
    pub ngl: Option<u32>,
    /// Where models are looked for, overriding the default.
    pub models_dir: Option<String>,
    /// Let the engine choose device, offload and cache from the machine.
    pub auto: bool,
    /// Run an architecture that has not been diffed against llama.cpp.
    pub force: bool,
    /// Light or dark. Persisted because a window that forgets which way round
    /// it is every launch is not a preference, it is a flicker.
    pub mode: Mode,
    /// The key `/v1/*` requires, or `None` for no key at all.
    ///
    /// **Off by default, deliberately.** The server binds `127.0.0.1` only, so
    /// a key is not what keeps a stranger out -- what keeps them out is that
    /// there is no route in. Turning it on by default would also break every
    /// agent already pointed at an existing install. It exists because many
    /// OpenAI-compatible clients insist on sending a key, and because a shared
    /// machine is a real thing.
    pub api_key: Option<String>,
    /// What this machine is to the other machines. See [`Role`].
    pub role: Role,
    /// The CORE this device talks to, as `host:port`, when it is not one.
    pub core_addr: Option<String>,
    /// The key that CORE wants.
    pub core_key: Option<String>,
    /// Keys read but not understood, preserved on write.
    unknown: BTreeMap<String, String>,
}

/// What a machine is to the others.
///
/// **This is the setting the Android app was blocked on.** `chaos-serve`
/// defaults to `127.0.0.1` and the window never passed `--host`, so the server
/// it started could only ever answer itself. A phone on the same Wi-Fi got no
/// route and no error -- Atur: *"when i try connect desktop nothing happen"*.
/// Nothing was broken; nothing was listening on an address a phone can reach.
///
/// Choosing CORE is what opens that route, and it is a choice rather than a
/// default because `0.0.0.0` means **every** network this machine is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    /// Only this machine. The server listens on `127.0.0.1` and there is no
    /// route in from anywhere else.
    #[default]
    Alone,
    /// Holds the models and answers. Other devices connect to it.
    Core,
    /// Lends this machine's memory and cores to a CORE. Runs no token loop of
    /// its own and keeps no conversation.
    Helper,
    /// Uses a CORE elsewhere. Loads nothing here.
    Client,
}

/// A key nobody has to invent.
///
/// **`chaos-serve` refuses `0.0.0.0` with no key**, and it is right to: every
/// device on the network could otherwise use the model and nothing would ask
/// them who they are. But refusing is the wrong thing for a window to do to
/// somebody who just pressed CORE, so the window makes one instead.
///
/// No dependency and no crypto claim: this is unguessable-in-practice, which is
/// what the server checks for -- equality, not strength. 26 characters of
/// base32 from SplitMix64, seeded from the clock and the process id so two
/// machines starting in the same second do not agree.
pub fn new_key() -> String {
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        ^ (std::process::id() as u64).rotate_left(32);
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut out = String::with_capacity(26);
    for _ in 0..26 {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        out.push(ALPHABET[(z % ALPHABET.len() as u64) as usize] as char);
    }
    out
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Alone => "alone",
            Role::Core => "core",
            Role::Helper => "helper",
            Role::Client => "client",
        }
    }
    pub fn parse(v: &str) -> Option<Self> {
        Some(match v.trim().to_ascii_lowercase().as_str() {
            "alone" => Role::Alone,
            "core" => Role::Core,
            "helper" => Role::Helper,
            "client" => Role::Client,
            _ => return None,
        })
    }
    /// What the server should bind for this role.
    ///
    /// Only CORE opens a route off this machine. ALONE still serves, because
    /// the window's own CHAT talks to it -- it just cannot be reached.
    pub fn host(self) -> &'static str {
        match self {
            Role::Core => "0.0.0.0",
            _ => "127.0.0.1",
        }
    }
    /// Whether this role needs a key before the server will start.
    ///
    /// Only CORE, and only because it is the only role that opens a route.
    pub fn needs_key(self) -> bool {
        !matches!(self, Role::Alone | Role::Client | Role::Helper)
    }

    /// The one-line description the CHAOS page shows under each choice.
    pub fn describe(self) -> &'static str {
        match self {
            Role::Alone => "Only this machine. Nothing else can reach it.",
            Role::Core => "Holds the models and answers. Other devices connect here.",
            Role::Helper => "Lends this machine's memory and cores to a CORE.",
            Role::Client => "Uses a CORE elsewhere. Loads nothing here.",
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cache_gib: None,
            threads: None,
            threads_batch: None,
            // Not 8080: that is the first port every other local tool takes,
            // and a collision here looks like the model failing to load.
            port: 8231,
            context: None,
            ngl: None,
            models_dir: None,
            auto: false,
            force: false,
            // Hermes' desktop defaults to light, and so does this.
            mode: Mode::Light,
            api_key: None,
            role: Role::Alone,
            core_addr: None,
            core_key: None,
            unknown: BTreeMap::new(),
        }
    }
}

/// `%USERPROFILE%\.chaos\settings.txt`, beside the models rather than inside
/// the install -- so an upgrade or an uninstall never takes preferences with it.
pub fn path() -> PathBuf {
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join(".chaos").join("settings.txt")
}

impl Settings {
    pub fn parse(text: &str) -> Self {
        let mut s = Settings::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "cache_gib" => s.cache_gib = v.parse().ok(),
                "threads" => s.threads = v.parse().ok(),
                "threads_batch" => s.threads_batch = v.parse().ok(),
                "port" => s.port = v.parse().unwrap_or(s.port),
                "context" => s.context = v.parse().ok(),
                "ngl" => s.ngl = v.parse().ok(),
                "models_dir" => {
                    s.models_dir = (!v.is_empty()).then(|| v.to_string());
                }
                "auto" => s.auto = truthy(v),
                "force" => s.force = truthy(v),
                "mode" => s.mode = Mode::parse(v).unwrap_or(s.mode),
                "api_key" => s.api_key = (!v.is_empty()).then(|| v.to_string()),
                "role" => s.role = Role::parse(v).unwrap_or(s.role),
                "core_addr" => s.core_addr = (!v.is_empty()).then(|| v.to_string()),
                "core_key" => s.core_key = (!v.is_empty()).then(|| v.to_string()),
                _ => {
                    s.unknown.insert(k.to_string(), v.to_string());
                }
            }
        }
        s
    }

    pub fn render(&self) -> String {
        let mut out =
            String::from("# Chaos settings. Written by chaos-app; safe to edit by hand.\n");
        let opt = |name: &str, v: Option<String>| match v {
            Some(v) => format!("{name} = {v}\n"),
            // Written as an empty value rather than omitted, so the file shows
            // every setting that exists and what it is currently not set to.
            None => format!("{name} =\n"),
        };
        out.push_str(&opt("cache_gib", self.cache_gib.map(|v| format!("{v}"))));
        out.push_str(&opt("threads", self.threads.map(|v| v.to_string())));
        out.push_str(&opt(
            "threads_batch",
            self.threads_batch.map(|v| v.to_string()),
        ));
        out.push_str(&format!("port = {}\n", self.port));
        out.push_str(&opt("context", self.context.map(|v| v.to_string())));
        out.push_str(&opt("ngl", self.ngl.map(|v| v.to_string())));
        out.push_str(&opt("models_dir", self.models_dir.clone()));
        out.push_str(&format!("auto = {}\n", self.auto));
        out.push_str(&format!("force = {}\n", self.force));
        out.push_str(&format!("mode = {}\n", self.mode.as_str()));
        out.push_str(&opt("api_key", self.api_key.clone()));
        out.push_str(&format!("role = {}\n", self.role.as_str()));
        out.push_str(&opt("core_addr", self.core_addr.clone()));
        out.push_str(&opt("core_key", self.core_key.clone()));
        for (k, v) in &self.unknown {
            out.push_str(&format!("{k} = {v}\n"));
        }
        out
    }

    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }

    /// Write, creating the directory. Returns the error text for the status
    /// line rather than swallowing it -- a settings page that cannot save and
    /// does not say so is worse than one that does not exist.
    pub fn save(&self) -> Result<(), String> {
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        std::fs::write(&p, self.render()).map_err(|e| format!("cannot write {}: {e}", p.display()))
    }

    /// Back to measured everything, without touching the view preference or
    /// any key a newer build wrote.
    ///
    /// A method rather than `..Default::default()` at the call site, because
    /// `unknown` is private: struct-update syntax from outside this module
    /// would not compile, and making the field public to allow it would let any
    /// caller drop the keys it exists to preserve.
    pub fn reset_engine(&mut self) {
        let keep_mode = self.mode;
        let keep_unknown = std::mem::take(&mut self.unknown);
        *self = Settings {
            mode: keep_mode,
            unknown: keep_unknown,
            ..Settings::default()
        };
    }

    /// The arguments these settings imply, for `chaos-serve`.
    ///
    /// One place, so the window and any future headless mode cannot disagree
    /// about what a setting means.
    pub fn serve_args(&self, model: &str) -> Vec<String> {
        // **`--host` was never passed, and that is the whole of "the phone
        // cannot reach the desktop".** `chaos-serve` binds 127.0.0.1 unless
        // told otherwise, so every server this window has ever started could
        // answer only this machine. The role decides the address now.
        let mut a = vec![
            model.to_string(),
            "--host".into(),
            self.role.host().to_string(),
            "--port".into(),
            self.port.to_string(),
        ];
        if let Some(c) = self.cache_gib {
            a.push("--cache".into());
            a.push(format!("{c}"));
        }
        if let Some(t) = self.threads {
            a.push("-t".into());
            a.push(t.to_string());
        }
        if let Some(t) = self.threads_batch {
            a.push("-tb".into());
            a.push(t.to_string());
        }
        if let Some(c) = self.context {
            a.push("-c".into());
            a.push(c.to_string());
        }
        // **`-ngl` is deliberately not sent.** `chaos-serve` refuses it -- its
        // dense path binds weights straight into host memory rather than through
        // the runner's device loader, so there is nowhere on a card to put them
        // yet -- and an unknown flag is an error there now rather than something
        // silently swallowed. Sending it would stop the server from starting.
        //
        // It was sent for three releases and did nothing whatsoever, which is
        // the whole reason the flag is refused loudly today. The setting stays
        // in the file so it survives the wiring; the GPU list says what is true.
        let _ = self.ngl;
        if self.auto {
            a.push("--auto".into());
        }
        if self.force {
            a.push("--force".into());
        }
        if let Some(k) = &self.api_key {
            a.push("--api-key".into());
            a.push(k.clone());
        }
        a
    }

    /// The endpoint, and what to send with it.
    ///
    /// One place, so the line the window shows, the string COPY ENDPOINT puts
    /// on the clipboard, and what the chat client actually sends cannot
    /// disagree -- which is precisely how an endpoint panel comes to advertise
    /// something that does not work.
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }
}

fn truthy(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_trip_preserves_everything() {
        let s = Settings {
            cache_gib: Some(6.5),
            threads: Some(4),
            threads_batch: Some(20),
            port: 9001,
            context: Some(4096),
            ngl: Some(99),
            models_dir: Some(r"D:\models".into()),
            auto: true,
            force: true,
            mode: Mode::Dark,
            api_key: Some("deadbeef".into()),
            ..Settings::default()
        };
        assert_eq!(Settings::parse(&s.render()), s);
    }

    #[test]
    fn defaults_round_trip_too() {
        let s = Settings::default();
        assert_eq!(Settings::parse(&s.render()), s);
    }

    /// **A downgrade must not destroy preferences.** An older build that does
    /// not know a key has to write it back untouched, or running it once loses
    /// whatever the newer one stored.
    #[test]
    fn unknown_keys_survive() {
        let s = Settings::parse("port = 1234\nsomething_new = 7\n");
        assert_eq!(s.port, 1234);
        assert!(s.render().contains("something_new = 7"));
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let s = Settings::parse("# a note\n\n   \nport = 4321\n");
        assert_eq!(s.port, 4321);
    }

    /// A corrupt value must not silently become zero -- port 0 binds a random
    /// port and the endpoint the window shows would be a lie.
    #[test]
    fn a_bad_port_keeps_the_default() {
        assert_eq!(Settings::parse("port = banana").port, 8231);
    }

    #[test]
    fn an_empty_value_means_unset() {
        let s = Settings::parse("cache_gib =\nthreads =\n");
        assert!(s.cache_gib.is_none() && s.threads.is_none());
    }

    #[test]
    fn a_missing_file_gives_defaults() {
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn serve_args_carry_only_what_is_set() {
        let s = Settings::default();
        let a = s.serve_args("qwen3");
        assert_eq!(a, vec!["qwen3", "--host", "127.0.0.1", "--port", "8231"]);
    }

    /// The bug Atur reported as *"when i try connect desktop nothing happen"*.
    ///
    /// The window never passed `--host`, so `chaos-serve` took its default of
    /// `127.0.0.1` and the server could answer only this machine. A phone on
    /// the same Wi-Fi had no route and got no error, because nothing was wrong
    /// -- nothing was listening anywhere it could reach.
    #[test]
    fn only_a_core_opens_a_route_off_this_machine() {
        for (role, expect) in [
            (Role::Alone, "127.0.0.1"),
            (Role::Client, "127.0.0.1"),
            (Role::Helper, "127.0.0.1"),
            (Role::Core, "0.0.0.0"),
        ] {
            assert_eq!(role.host(), expect, "{role:?}");
            let s = Settings {
                role,
                ..Settings::default()
            };
            let a = s.serve_args("qwen3");
            let i = a.iter().position(|x| x == "--host").expect("--host passed");
            assert_eq!(a[i + 1], expect, "{role:?} binds {expect}");
        }
    }

    /// `chaos-serve` refuses `0.0.0.0` with no key, so CORE must bring one.
    #[test]
    fn a_core_needs_a_key_and_the_others_do_not() {
        assert!(Role::Core.needs_key());
        for r in [Role::Alone, Role::Client, Role::Helper] {
            assert!(!r.needs_key(), "{r:?}");
        }
    }

    /// A generated key has to be usable and not the same twice.
    #[test]
    fn a_generated_key_is_long_and_not_repeated() {
        let a = new_key();
        assert_eq!(a.len(), 26);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
        // **Not a randomness test.** Two calls in the same process differ
        // because the state advances; this catches a generator that returns a
        // constant, which is the failure that would matter.
        assert_ne!(a, new_key());
    }

    /// A role survives the file, and an unknown one does not lose the setting.
    #[test]
    fn a_role_round_trips_through_the_file() {
        for role in [Role::Alone, Role::Core, Role::Helper, Role::Client] {
            let s = Settings {
                role,
                core_addr: Some("192.168.1.20:8231".into()),
                core_key: Some("abc".into()),
                ..Settings::default()
            };
            let back = Settings::parse(&s.render());
            assert_eq!(back.role, role);
            assert_eq!(back.core_addr.as_deref(), Some("192.168.1.20:8231"));
            assert_eq!(back.core_key.as_deref(), Some("abc"));
        }
        // Garbage keeps the previous value rather than resetting the machine's
        // job to ALONE behind the user's back.
        let s = Settings::parse("role = wharrgarbl\n");
        assert_eq!(s.role, Role::Alone);
    }

    #[test]
    fn serve_args_include_every_setting() {
        let s = Settings {
            cache_gib: Some(8.0),
            threads: Some(4),
            threads_batch: Some(20),
            context: Some(2048),
            ngl: Some(99),
            auto: true,
            force: true,
            ..Settings::default()
        };
        let a = s.serve_args("m").join(" ");
        for expected in [
            "--cache 8",
            "-t 4",
            "-tb 20",
            "-c 2048",
            "--auto",
            "--force",
        ] {
            assert!(a.contains(expected), "{expected} missing from {a}");
        }
        // **And nothing the server refuses.** `-ngl` was sent for three
        // releases and silently dropped; it is a hard error there now, so
        // sending it would stop the server from starting at all.
        assert!(
            !a.contains("-ngl"),
            "chaos-serve refuses -ngl -- sending it kills the server: {a}"
        );
    }

    /// The model always comes first: `chaos-serve` takes it positionally.
    #[test]
    fn the_model_is_the_first_argument() {
        assert_eq!(Settings::default().serve_args("mymodel")[0], "mymodel");
    }

    /// A key reaches the server, or it is a decoration on a page.
    #[test]
    fn a_key_is_passed_to_the_server() {
        let mut s = Settings::default();
        assert!(
            !s.serve_args("m").iter().any(|a| a == "--api-key"),
            "no key is set, so none must be passed"
        );
        s.api_key = Some("abc123".into());
        let a = s.serve_args("m");
        let i = a
            .iter()
            .position(|x| x == "--api-key")
            .expect("no --api-key");
        assert_eq!(a[i + 1], "abc123");
    }

    /// The endpoint is built in one place and follows the port.
    #[test]
    fn the_endpoint_follows_the_port() {
        let s = Settings::parse("port = 9313");
        assert_eq!(s.endpoint(), "http://127.0.0.1:9313/v1");
    }

    /// A reset returns every engine setting to measured, and touches neither
    /// the theme nor a key this build does not understand.
    #[test]
    fn a_reset_keeps_the_theme_and_the_unknown_keys() {
        let mut s = Settings::parse(
            "cache_gib = 9
threads = 12
port = 9999
mode = dark
from_the_future = 7
",
        );
        s.reset_engine();
        assert_eq!(s.cache_gib, None, "cache was not reset");
        assert_eq!(s.threads, None, "threads were not reset");
        assert_eq!(s.port, Settings::default().port, "the port was not reset");
        assert_eq!(s.mode, Mode::Dark, "the reset flipped the lights");
        assert!(
            s.render().contains("from_the_future = 7"),
            "the reset dropped a key a newer build wrote"
        );
    }

    /// Settings live outside the install, so upgrading or uninstalling Chaos
    /// cannot take them with it.
    #[test]
    fn settings_are_not_inside_the_install() {
        let p = path();
        assert!(p.ends_with("settings.txt"));
        assert!(!p
            .to_string_lossy()
            .to_lowercase()
            .contains("localappdata\\chaos\\bin"));
    }
}
