//! Configuration, loaded from `~/.config/anistream/config.toml`.
//!
//! Two principles run through this module. First, every volatile thing is *addressable
//! from config* — provider order, emulation profile, dataset URLs — so a dead source or
//! a rotated fingerprint is a config edit rather than a release. Second, anything with a
//! privacy or safety consequence **fails closed**: see [`VpnConfig`] and
//! [`Config::validate`], where torrenting is unreachable until a VPN mode is chosen and
//! `mode = "none"` requires an explicit acknowledgement that cannot be reached by
//! omission.

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::media::Translation;

/// Filesystem locations, resolved once at startup.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self, crate::Error> {
        let dirs = directories::ProjectDirs::from("", "", "anistream")
            .ok_or_else(|| crate::Error::Config("cannot determine home directory".into()))?;
        Ok(Self {
            config_file: dirs.config_dir().join("config.toml"),
            data_dir: dirs.data_dir().to_path_buf(),
            cache_dir: dirs.cache_dir().to_path_buf(),
        })
    }

    pub fn database(&self) -> PathBuf {
        self.data_dir.join("anistream.db")
    }

    pub fn image_cache(&self) -> PathBuf {
        self.cache_dir.join("img")
    }

    pub fn plugin_dir(&self) -> PathBuf {
        self.config_file.parent().unwrap_or(&self.data_dir).join("plugins")
    }

    /// Where the mpv IPC socket lives.
    ///
    /// Under the cache dir rather than the data dir: a stale socket left by a crash is
    /// disposable, and putting it somewhere backed up would be wrong.
    pub fn runtime_dir(&self) -> PathBuf {
        self.cache_dir.join("run")
    }
}

/// Root configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: ThemeConfig,
    pub playback: PlaybackConfig,
    pub providers: ProvidersConfig,
    pub trackers: TrackersConfig,
    pub downloads: DownloadsConfig,
    pub presence: PresenceConfig,
    pub network: NetworkConfig,
    pub updates: UpdatesConfig,
    pub syncplay: SyncplayConfig,
    /// Keybinding overrides, `action = "key"`. The help overlay is generated from the
    /// resolved map so it can never drift from what the keys actually do.
    pub keys: BTreeMap<String, String>,
}

impl Config {
    /// Parse from a TOML string, then validate.
    pub fn from_toml(src: &str) -> Result<Self, crate::Error> {
        let cfg: Self = toml::from_str(src)
            .map_err(|e| crate::Error::Config(format!("invalid TOML: {e}")))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Load from disk, falling back to defaults when the file does not exist.
    ///
    /// A missing config is normal on first run and must not be an error; a *malformed*
    /// one is, because silently ignoring it could mean silently ignoring a VPN setting.
    pub fn load(paths: &Paths) -> Result<Self, crate::Error> {
        match std::fs::read_to_string(&paths.config_file) {
            Ok(src) => Self::from_toml(&src),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let cfg = Self::default();
                cfg.validate()?;
                Ok(cfg)
            }
            Err(e) => Err(crate::Error::Config(format!(
                "cannot read {}: {e}",
                paths.config_file.display()
            ))),
        }
    }

    /// Reject configurations that are unsafe or self-contradictory.
    pub fn validate(&self) -> Result<(), crate::Error> {
        self.providers.torrent.validate()?;
        self.trackers.validate()?;
        if !(0.05..=1.0).contains(&self.playback.commit_threshold) {
            return Err(crate::Error::Config(format!(
                "playback.commit_threshold must be between 0.05 and 1.0, got {}",
                self.playback.commit_threshold
            )));
        }
        Ok(())
    }
}

/// Offline downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DownloadsConfig {
    /// Where finished files go. `None` leaves them where the torrent session wrote them,
    /// in the cache directory. Ignored while `keep_seeding` is on — a seeding torrent
    /// needs its file where the session put it.
    ///
    /// Worth being explicit about: unlike everything else anistream writes, these are files the
    /// user will want to find, so a cache directory is a poor default and a configurable one is the
    /// point.
    pub directory: Option<String>,
    /// Mux sidecar subtitle files into the video when a download finishes.
    ///
    /// On by default because a release with separate `.ass` files plays without subtitles anywhere
    /// the folder structure is not preserved — copying the file to a phone, for instance. The remux
    /// is stream-copy only, so it costs seconds and re-encodes nothing.
    pub merge_subtitles: bool,
    /// A command run after each download finishes — after any move and subtitle merge,
    /// with `ANISTREAM_PATH`, `ANISTREAM_TITLE` and `ANISTREAM_EPISODE` in its
    /// environment. Refreshing a Jellyfin library or renaming to a house scheme are the
    /// intended uses; failures are logged, never fatal.
    pub on_complete: Option<String>,
    /// Keep seeding after a download completes.
    ///
    /// Off by default, and that is a privacy choice rather than a bandwidth one: seeding advertises
    /// you as a source for as long as it runs, and doing that unattended is not something to opt
    /// somebody into silently.
    ///
    /// Streaming is different, deliberately: while an episode plays (and for as long as the
    /// app stays open), its torrent uploads to peers over the guarded connection — watching
    /// already gives back. This key only decides whether a *finished download* keeps doing so.
    pub keep_seeding: bool,
}

/// The Discord application a presence appears under when none is configured.
///
/// Shipped as a plain constant, and that is fine: a Discord client id is public by construction — it
/// travels in every rich-presence handshake and every client that has one embeds it. There is
/// nothing here to protect.
///
/// What it *does* decide is the name on the user's profile, because Discord displays the
/// application's name. So this is an editorial choice rather than a credential, and
/// `presence.client_id` overrides it for anyone who would rather appear under their own app.
///
/// It also cannot be invented. Discord refuses an unknown id at handshake time, so a made-up value
/// would leave the feature silently doing nothing — the exact failure mode this project spends most
/// of its effort eliminating.
pub const DEFAULT_PRESENCE_CLIENT_ID: &str = "1531267760959783025";

/// Discord rich presence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PresenceConfig {
    /// Off by default, and that is the only defensible default: this is the one feature that
    /// publishes what you are watching to a third party, so it has to be asked for.
    pub enabled: bool,
    /// The Discord application whose name and artwork the presence appears under.
    ///
    /// Falls back to [`DEFAULT_PRESENCE_CLIENT_ID`] when unset. A Discord client id is public by
    /// construction — it travels in the IPC handshake and every rich-presence client ships one — so
    /// there is nothing to protect here. What the id actually decides is the *name on your profile*:
    /// Discord shows the application's name, so this is an editorial choice more than a credential.
    /// Override it to appear under your own app instead.
    pub client_id: Option<String>,
    /// Show the title, or only that something is playing.
    ///
    /// For anyone who wants the "watching anime" presence without broadcasting *which* anime, which
    /// is a completely reasonable thing to want.
    pub show_title: bool,
}

impl PresenceConfig {
    /// The application to connect as, or `None` if there is genuinely none.
    ///
    /// Resolved here rather than at each call site: the fallback and the "empty string means unset"
    /// rule both have to hold everywhere, and a second copy of that logic is a second chance to get
    /// it wrong.
    pub fn resolved_client_id(&self) -> Option<&str> {
        self.client_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .or(Some(DEFAULT_PRESENCE_CLIENT_ID))
            .filter(|id| !id.trim().is_empty())
    }
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self { enabled: false, client_id: None, show_title: true }
    }
}

impl Default for DownloadsConfig {
    fn default() -> Self {
        Self { directory: None, merge_subtitles: true, keep_seeding: false, on_complete: None }
    }
}

/// Real-time upscaling modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Upscaling {
    #[default]
    Off,
    /// Anime4K mode A with the medium networks — fine on integrated graphics.
    Anime4kFast,
    /// Anime4K mode A with the large networks — wants a discrete GPU.
    Anime4kQuality,
}

/// Watch parties via Syncplay.
///
/// A handoff, not a session: Syncplay owns the player it launches, so progress is not
/// recorded while a party is on — a shared watch is not private history. Turn it on for
/// the evening, off after. Syncplay's own config decides which player it drives.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SyncplayConfig {
    /// Hand playback to Syncplay instead of a private mpv session.
    pub enabled: bool,
    /// `host:port` of the Syncplay server everyone in the room uses.
    pub server: String,
    /// The room to join. Required before a party can start — Syncplay's no-gui mode
    /// refuses an empty room, so anistream asks for this up front rather than spawning
    /// a process that immediately dies.
    pub room: Option<String>,
    /// The name shown to the room.
    pub name: String,
    /// The Syncplay executable, a path or a name on `PATH`.
    pub binary: String,
}

impl Default for SyncplayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server: "syncplay.pl:8995".into(),
            room: None,
            name: "anistream".into(),
            binary: "syncplay".into(),
        }
    }
}

/// The update check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UpdatesConfig {
    /// Ask GitHub for the newest release once a day and say so when one exists.
    ///
    /// One HTTPS request to api.github.com, cached for 24 hours. Nothing is ever
    /// downloaded without `anistream --update` being run explicitly.
    pub check: bool,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        Self { check: true }
    }
}

/// Visual configuration. See the "Obi & Silk" direction in the plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    /// Set `false` to suppress the eyecatch wipe and all other motion.
    pub motion: bool,
    /// Reserved. Accepted so existing configs keep parsing (`deny_unknown_fields` would
    /// otherwise reject them), but currently has no effect: every glyph in the UI is plain
    /// Unicode, and requiring a patched font is both a portability hazard and one of the
    /// clearest templated-TUI tells.
    pub nerd_font: bool,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self { mode: ThemeMode::Adaptive, motion: true, nerd_font: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Inherit the terminal's background and set foreground colours only, so anistream
    /// sits naturally beside the user's other tools. The palette variant is chosen by
    /// querying the real background over OSC 11.
    #[default]
    Adaptive,
    /// Paint the dusk-indigo ground for a fully controlled look.
    Immersive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlaybackConfig {
    pub translation: Translation,
    /// Desired vertical resolution. Streams are ranked toward this, preferring a step
    /// down over upscaling.
    pub quality: u32,
    pub subtitle_language: String,
    /// Fraction of runtime after which an episode counts as watched.
    ///
    /// Not zero, deliberately: opening an episode to check the subtitles should not mark
    /// it seen and push progress to a tracker.
    pub commit_threshold: f64,
    pub auto_next: bool,
    /// Carry playback speed and audio/subtitle track choice to the next episode.
    pub persist_speed: bool,
    /// The speed last chosen in the player, carried to the next episode.
    ///
    /// Stored rather than derived so it survives a restart — the small detail that makes a
    /// client feel finished rather than merely functional.
    pub persisted_speed: Option<f64>,
    /// Carry the player volume across sessions.
    pub persist_volume: bool,
    /// The volume last chosen in the player, in mpv's 0–100 scale.
    pub persisted_volume: Option<f64>,
    pub skip_opening: bool,
    pub skip_filler: bool,
    /// Real-time upscaling via Anime4K, applied as mpv shader chains.
    ///
    /// The shaders ship inside the binary (MIT, vendored from bloc97/Anime4K), so this
    /// is a toggle rather than a scavenger hunt. `fast` suits integrated graphics;
    /// `quality` wants a discrete GPU. Worth turning on for 720p batch rips.
    pub upscaling: Upscaling,
    /// Reserved. Accepted so existing configs keep parsing, but currently has no
    /// effect: players are routed by what a stream needs (mpv for media, the handoff
    /// player for external deep links), not by preference order.
    pub players: Vec<String>,
    /// The mpv executable. A path rather than a bare name works too, which matters on systems
    /// where mpv is not on the `PATH` the terminal inherited.
    pub mpv_binary: String,
    /// Extra flags appended to every mpv invocation.
    ///
    /// An escape hatch, not a configuration surface: profiles and hwdec belong in the user's
    /// own `mpv.conf`, which mpv reads anyway.
    pub mpv_args: Vec<String>,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            translation: Translation::Sub,
            quality: 1080,
            subtitle_language: "eng".into(),
            commit_threshold: 0.85,
            auto_next: true,
            persist_speed: true,
            persisted_speed: None,
            persist_volume: true,
            persisted_volume: None,
            upscaling: Upscaling::Off,
            skip_opening: true,
            skip_filler: false,
            players: vec!["mpv".into(), "external".into()],
            mpv_binary: "mpv".into(),
            mpv_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProvidersConfig {
    /// Resolution order. Earlier entries are tried first; failover walks the list.
    pub order: Vec<String>,
    pub disabled: Vec<String>,
    pub torrent: TorrentConfig,
    pub plugins: PluginsConfig,
    /// Base URL of a self-hosted Consumet-shaped API, if you run one.
    pub remote_url: Option<String>,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            // Torrent first, plugins second — the order failover walks.
            //
            // `plugins` after it: a dropped-in `.wasm` is a source you chose to install, so it
            // should be tried, but not ahead of one that is known to work.
            order: vec!["torrent".into(), "plugins".into()],
            disabled: Vec::new(),
            torrent: TorrentConfig::default(),
            plugins: PluginsConfig::default(),
            remote_url: None,
        }
    }
}

/// Resource ceilings for WASM plugins.
///
/// Exposed because a plugin doing something unusual — decrypting a large payload, say — might
/// legitimately need more room, and the alternative would be recompiling. They are ceilings, not
/// grants: raising them cannot give a plugin a capability, only more of one it already has.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PluginsConfig {
    /// Linear-memory ceiling per plugin, in mebibytes.
    pub memory_mb: usize,
    /// Wall-clock budget for one plugin call.
    ///
    /// Enforced by epoch interruption, which can stop a guest mid-loop — so this is a real bound
    /// rather than a hope that the plugin checks a flag.
    pub deadline_secs: u64,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self { memory_mb: 64, deadline_secs: 20 }
    }
}

/// Torrent source settings.
///
/// Disabled by default. Torrent traffic exposes your IP to every peer in the swarm, so
/// this stays off until a VPN mode has been chosen deliberately.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TorrentConfig {
    pub enabled: bool,
    pub vpn: VpnConfig,
    /// RSS search endpoint, supplied by you.
    ///
    /// anistream ships no indexer and no default: the torrent source stays inert until this
    /// is set. `{query}` is replaced with the URL-encoded search terms; a template without
    /// it gets `q=` appended. The response is expected to be RSS whose items carry seeders
    /// and an info hash.
    pub rss_url: Option<String>,
    /// Trackers added to every magnet, supplied by you.
    ///
    /// Proxy mode disables DHT, so with none of these there is no way to find peers.
    pub trackers: Vec<String>,
    /// Optional curation endpoint, supplied by you: which release of a title is the good
    /// one. `{anilist_id}` is replaced with the AniList id. Unset means raw ranking decides.
    pub curation_url: Option<String>,
    /// Reserved. Accepted so existing configs keep parsing; currently has no effect —
    /// ranking always trusts the feed's live seeder counts.
    pub max_seeders_age_days: Option<u32>,
}

impl TorrentConfig {
    /// Host of the configured indexer, used to keep curated picks on the same service.
    pub fn indexer_host(&self) -> Option<String> {
        let url = self.rss_url.as_deref()?;
        let rest = url.split_once("://")?.1;
        let authority = rest.split(['/', '?', '#']).next()?;
        let host = authority.rsplit('@').next()?.split(':').next()?;
        (!host.is_empty()).then(|| host.to_ascii_lowercase())
    }
}

impl TorrentConfig {
    fn validate(&self) -> Result<(), crate::Error> {
        if !self.enabled {
            return Ok(());
        }
        // No indexer ships with anistream, so an enabled source with no endpoint would be
        // silently inert. Say so instead.
        match self.rss_url.as_deref().map(str::trim) {
            None | Some("") => {
                return Err(crate::Error::Config(
                    "providers.torrent.enabled is true but no providers.torrent.rss_url is \
                     set: anistream ships no indexer, so the search endpoint is yours to \
                     supply"
                        .into(),
                ));
            }
            Some(url) if !url.starts_with("http://") && !url.starts_with("https://") => {
                return Err(crate::Error::Config(
                    "providers.torrent.rss_url must be an http or https URL".into(),
                ));
            }
            Some(_) => {}
        }
        match self.vpn.mode {
            VpnMode::Socks5 => {
                if self.vpn.socks_url.as_deref().unwrap_or("").is_empty() {
                    return Err(crate::Error::Config(
                        "providers.torrent.vpn.mode is \"socks5\" but no socks_url is set"
                            .into(),
                    ));
                }
            }
            VpnMode::External => {}
            VpnMode::None => {
                if !self.vpn.i_understand_my_ip_is_exposed {
                    return Err(crate::Error::Config(
                        "providers.torrent.vpn.mode is \"none\": set \
                         i_understand_my_ip_is_exposed = true to confirm, or choose a VPN mode"
                            .into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// How torrent traffic is kept off your own address.
///
/// librqbit offers a SOCKS5 proxy but **no** bind-to-interface option, so real interface
/// binding has to happen outside the process. This config covers what we can enforce and
/// the docs are explicit about the rest, rather than implying a guarantee we cannot make.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VpnConfig {
    pub mode: VpnMode,
    pub socks_url: Option<String>,
    /// Accepted exit network operators, matched case-insensitively as substrings.
    ///
    /// Checked against the egress lookup so the guard asserts *whose* network you are
    /// leaving through, not merely that some address answered. Provider-agnostic: put
    /// whatever your VPN reports here.
    ///
    /// A **list** rather than one value because operators routinely exit through upstream
    /// infrastructure under a different name — Mullvad reports both `Mullvad VPN AB` and
    /// `31173 Services AB`, for instance. Demanding a single string would flag a perfectly
    /// good tunnel as a leak. Any one match is enough.
    pub require_asn_org: Vec<String>,
    /// Optional shortcut for Mullvad, which publishes a definitive `mullvad_exit_ip`
    /// boolean. Stronger than operator matching when available, and simply unused
    /// otherwise — every other provider is covered by `require_asn_org`.
    pub mullvad_exit: bool,
    pub verify_interval_secs: u64,
    pub on_leak: LeakAction,
    /// Required when `mode = "none"`. Named to be impossible to set by accident.
    pub i_understand_my_ip_is_exposed: bool,
}

impl Default for VpnConfig {
    fn default() -> Self {
        Self {
            mode: VpnMode::Socks5,
            socks_url: None,
            require_asn_org: Vec::new(),
            mullvad_exit: false,
            verify_interval_secs: 60,
            on_leak: LeakAction::Pause,
            i_understand_my_ip_is_exposed: false,
        }
    }
}

impl VpnConfig {
    /// Whether DHT must be disabled.
    ///
    /// In proxy mode: yes, always. SOCKS5 UDP-associate is frequently unsupported and
    /// librqbit does not document whether DHT is tunnelled or bypasses the proxy. That is
    /// unverified, so it is treated as a leak. Tracker-only operation costs little as long
    /// as `providers.torrent.trackers` is populated.
    pub const fn must_disable_dht(&self) -> bool {
        matches!(self.mode, VpnMode::Socks5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VpnMode {
    /// Route the torrent session through a SOCKS5 proxy. Natively supported by librqbit.
    #[default]
    Socks5,
    /// Trust an OS-level arrangement — a network namespace, or a provider kill switch.
    /// The egress check still runs, so a broken tunnel is still caught.
    External,
    /// No protection. Requires explicit acknowledgement.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeakAction {
    /// Pause every torrent and raise a blocking alert.
    #[default]
    Pause,
    /// Tear the session down entirely.
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TrackersConfig {
    /// Tracker ids to sync with, e.g. `["anilist"]`.
    pub enabled: Vec<String>,
    pub anilist: AniListAuthConfig,
    pub mal: MalAuthConfig,
    pub simkl: DeviceAuthConfig,
    pub trakt: DeviceAuthConfig,
    /// Where tokens live: `"keychain"` or `"file"`.
    ///
    /// `keychain` is the default and the better choice for a normal install. `file` exists because
    /// macOS keys keychain access on the *binary*, and every `cargo build` produces a new one — so
    /// during development "Always Allow" can never stick and every run prompts for a password. A
    /// `0600` file has no such problem. `ANISTREAM_TOKEN_STORAGE` overrides this for one run.
    pub token_storage: String,
    /// Seconds between outbox drain attempts.
    pub drain_interval_secs: u64,
    /// Seconds between library pulls. Much rarer than a drain: a pull costs a request out of
    /// thirty a minute, while an idle drain costs nothing.
    pub pull_interval_secs: u64,
}

impl Default for TrackersConfig {
    fn default() -> Self {
        Self {
            // Off until the user connects an account. Local history stands alone.
            enabled: Vec::new(),
            anilist: AniListAuthConfig::default(),
            mal: MalAuthConfig::default(),
            simkl: DeviceAuthConfig::default(),
            trakt: DeviceAuthConfig::default(),
            token_storage: "keychain".into(),
            drain_interval_secs: 60,
            pull_interval_secs: 900,
        }
    }
}

/// AniList OAuth settings.
///
/// Measured, not assumed: AniList supports **only** the authorization code grant. The implicit
/// grant returns `unsupported_grant_type` and `code_verifier` is ignored, so PKCE is not an
/// option either — which means a `client_secret` is unavoidable.
///
/// That is why both halves live here. The user registers their own client at
/// <https://anilist.co/settings/developer>; the secret is theirs, in their own config, on their
/// own machine. anistream never embeds one, because a secret inside an open-source binary is
/// not a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AniListAuthConfig {
    pub client_id: Option<String>,
    /// The client secret from the same registration.
    ///
    /// Optional in the type so a half-finished setup loads and reports itself, rather than
    /// failing to parse. [`TrackersConfig::validate`] rejects it when the tracker is enabled.
    pub client_secret: Option<String>,
    /// Loopback port for the OAuth redirect.
    ///
    /// Fixed rather than ephemeral: AniList matches the registered redirect URI exactly, so the
    /// port is part of what the user registers and cannot be chosen at runtime.
    pub redirect_port: u16,
    /// `"loopback"` catches the token automatically; `"paste"` sends you to AniList's PIN page
    /// to copy it by hand, which needs no port and works over SSH.
    pub flow: String,
}

impl Default for AniListAuthConfig {
    fn default() -> Self {
        Self {
            client_id: None,
            client_secret: None,
            redirect_port: 45_617,
            flow: "loopback".into(),
        }
    }
}

/// MyAnimeList OAuth settings.
///
/// Only a client id, and that is a measured fact rather than an oversight: MAL's token endpoint
/// accepts a request carrying just `client_id` and `code_verifier`, so an app registered as type
/// `other` is a public client and PKCE stands in for the secret. AniList refuses the same shape —
/// see [`AniListAuthConfig`] — which is why the two trackers have different config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MalAuthConfig {
    pub client_id: Option<String>,
    /// Loopback port for the redirect. MAL matches the registered URL exactly, so this is part of
    /// what the user registers — the same reason AniList's port is fixed.
    pub redirect_port: u16,
}

/// A tracker signed into with the OAuth device flow.
///
/// One shape for Simkl and Trakt because there is genuinely nothing to differentiate: no redirect
/// URI to register, no port to match. Just the application's id, and a secret where the service
/// insists on one.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceAuthConfig {
    pub client_id: Option<String>,
    /// Trakt requires this on the token exchange; Simkl does not use one at all.
    pub client_secret: Option<String>,
}

impl Default for MalAuthConfig {
    fn default() -> Self {
        // The same port as AniList: only one sign-in runs at a time, and the flow always knows
        // which tracker it started. One number for the user to register twice.
        Self { client_id: None, redirect_port: 45_617 }
    }
}

impl TrackersConfig {
    fn validate(&self) -> Result<(), crate::Error> {
        if self.enabled.iter().any(|t| t == "mal")
            && self.mal.client_id.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(crate::Error::Config(
                "trackers.mal.client_id is required when \"mal\" is enabled — register an app \
                 (type: other) at https://myanimelist.net/apiconfig"
                    .into(),
            ));
        }
        // Same rule for the device-flow trackers, and the same reason: a tracker that cannot
        // possibly authenticate would surface much later as a permanently stuck outbox.
        for (name, config, where_to_register) in [
            ("simkl", &self.simkl, "https://simkl.com/settings/developer"),
            ("trakt", &self.trakt, "https://trakt.tv/oauth/applications"),
        ] {
            if self.enabled.iter().any(|t| t == name)
                && config.client_id.as_deref().unwrap_or("").trim().is_empty()
            {
                return Err(crate::Error::Config(format!(
                    "trackers.{name}.client_id is required when \"{name}\" is enabled — \
                     register an application at {where_to_register}"
                )));
            }
        }
        self.validate_anilist()
    }

    fn validate_anilist(&self) -> Result<(), crate::Error> {
        // Enabling a tracker you cannot possibly authenticate against would show up much later
        // as a mysteriously stuck outbox, so it is rejected at load.
        if self.enabled.iter().any(|t| t == "anilist") {
            let blank =
                |field: &Option<String>| field.as_deref().unwrap_or("").trim().is_empty();
            if blank(&self.anilist.client_id) || blank(&self.anilist.client_secret) {
                return Err(crate::Error::Config(
                    "trackers.anilist needs both client_id and client_secret when \"anilist\" \
                     is enabled — AniList supports only the authorization code grant, so the \
                     secret is required. Register a client at \
                     https://anilist.co/settings/developer"
                        .into(),
                ));
            }
        }
        if !matches!(self.token_storage.as_str(), "keychain" | "file") {
            return Err(crate::Error::Config(format!(
                "trackers.token_storage must be \"keychain\" or \"file\", got {:?}",
                self.token_storage
            )));
        }
        if !matches!(self.anilist.flow.as_str(), "loopback" | "paste") {
            return Err(crate::Error::Config(format!(
                "trackers.anilist.flow must be \"loopback\" or \"paste\", got {:?}",
                self.anilist.flow
            )));
        }
        Ok(())
    }

    /// Whether a tracker id is turned on.
    pub fn is_enabled(&self, tracker_id: &str) -> bool {
        self.enabled.iter().any(|t| t == tracker_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    /// Browser fingerprint to emulate for Cloudflare-fronted providers. Exposed as
    /// config because this is an arms race — rotating it should not need a rebuild.
    pub emulation: String,
    pub timeout_secs: u64,
    /// Requests per minute against AniList. Their observed limit is 30.
    pub anilist_rate_limit: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self { emulation: "chrome".into(), timeout_secs: 20, anilist_rate_limit: 30 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_torrenting_is_off() {
        let cfg = Config::default();
        cfg.validate().unwrap();
        assert!(
            !cfg.providers.torrent.enabled,
            "torrenting must be opt-in, never on by default"
        );
        assert_eq!(cfg.theme.mode, ThemeMode::Adaptive);
        assert!(!cfg.theme.nerd_font);
    }

    #[test]
    fn no_tracker_is_enabled_by_default() {
        // History has to stand alone: the app is fully usable with no account, so sync is
        // something you opt into rather than something you turn off.
        let cfg = Config::default();
        assert!(cfg.trackers.enabled.is_empty());
        assert!(!cfg.trackers.is_enabled("anilist"));
    }

    #[test]
    fn enabling_anilist_without_a_client_id_is_rejected() {
        // AniList has no public client and the code flow needs a secret, so the user has to
        // register their own. Discovering that via a permanently-stuck outbox would be much
        // worse than a load-time error.
        let err = Config::from_toml(
            r#"
            [trackers]
            enabled = ["anilist"]
            "#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("client_id"), "{err}");
        assert!(
            err.contains("anilist.co/settings/developer"),
            "must say where to get one: {err}"
        );
    }

    #[test]
    fn enabling_anilist_with_both_credentials_is_accepted() {
        let cfg = Config::from_toml(
            r#"
            [trackers]
            enabled = ["anilist"]
            [trackers.anilist]
            client_id = "12345"
            client_secret = "shhh"
            "#,
        )
        .unwrap();
        assert!(cfg.trackers.is_enabled("anilist"));
        assert_eq!(cfg.trackers.anilist.redirect_port, 45_617);
    }

    #[test]
    fn an_id_without_a_secret_is_rejected() {
        // AniList supports only the authorization code grant, so an id alone cannot sign in.
        // Measured: `response_type=token` returns `unsupported_grant_type`.
        let err = Config::from_toml(
            r#"
            [trackers]
            enabled = ["anilist"]
            [trackers.anilist]
            client_id = "12345"
            "#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("client_secret"), "{err}");
    }

    #[test]
    fn a_blank_client_id_counts_as_missing() {
        // An empty string in config is a half-finished setup, not a valid id.
        assert!(
            Config::from_toml(
                r#"
                [trackers]
                enabled = ["anilist"]
                [trackers.anilist]
                client_id = "   "
                "#,
            )
            .is_err()
        );
    }

    #[test]
    fn token_storage_accepts_only_the_two_backends() {
        // A typo must be caught at load, not discovered as "somehow I am signed out".
        assert_eq!(Config::default().trackers.token_storage, "keychain");
        for value in ["keychain", "file"] {
            Config::from_toml(&format!("[trackers]\ntoken_storage = \"{value}\"")).unwrap();
        }
        let err =
            Config::from_toml("[trackers]\ntoken_storage = \"vault\"").unwrap_err().to_string();
        assert!(err.contains("keychain"), "{err}");
    }

    #[test]
    fn an_unknown_auth_flow_is_rejected() {
        // A typo would otherwise silently fall back to one flow while the user registered the
        // redirect for the other.
        let err = Config::from_toml(
            r#"
            [trackers.anilist]
            flow = "magic"
            "#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("loopback"), "{err}");
    }

    #[test]
    fn a_client_id_without_enabling_the_tracker_is_fine() {
        // Configuring credentials ahead of turning sync on is a reasonable order to do things.
        Config::from_toml(
            r#"
            [trackers.anilist]
            client_id = "12345"
            "#,
        )
        .unwrap();
    }

    #[test]
    fn enabling_torrents_without_an_indexer_is_rejected() {
        // anistream ships no indexer, so an enabled source with no endpoint would sit
        // there doing nothing. Say so rather than failing silently at search time.
        let err = Config::from_toml(
            r#"
            [providers.torrent]
            enabled = true
            [providers.torrent.vpn]
            mode = "none"
            i_understand_my_ip_is_exposed = true
            "#,
        )
        .expect_err("must not enable a source with nothing to search");
        assert!(err.to_string().contains("rss_url"), "got: {err}");
    }

    #[test]
    fn an_indexer_url_has_to_be_http() {
        let err = Config::from_toml(
            r#"
            [providers.torrent]
            enabled = true
            rss_url = "file:///etc/passwd"
            [providers.torrent.vpn]
            mode = "none"
            i_understand_my_ip_is_exposed = true
            "#,
        )
        .expect_err("only http(s) is transport");
        assert!(err.to_string().contains("http"), "got: {err}");
    }

    #[test]
    fn the_indexer_host_is_extracted_for_matching_curated_picks() {
        let config = TorrentConfig {
            rss_url: Some("https://Indexer.Example:8080/rss?q={query}".into()),
            ..Default::default()
        };
        assert_eq!(config.indexer_host().as_deref(), Some("indexer.example"));
        assert_eq!(TorrentConfig::default().indexer_host(), None);
    }

    #[test]
    fn enabling_torrents_without_a_socks_url_is_rejected() {
        let err = Config::from_toml(
            r#"
            [providers.torrent]
            enabled = true
            rss_url = "https://indexer.example/?q={query}"
            [providers.torrent.vpn]
            mode = "socks5"
            "#,
        )
        .expect_err("must not start with proxy mode and no proxy");
        assert!(err.to_string().contains("socks_url"), "got: {err}");
    }

    #[test]
    fn vpn_mode_none_requires_explicit_acknowledgement() {
        let base = r#"
            [providers.torrent]
            enabled = true
            rss_url = "https://indexer.example/?q={query}"
            [providers.torrent.vpn]
            mode = "none"
        "#;
        let err = Config::from_toml(base).expect_err("must not silently torrent unprotected");
        assert!(err.to_string().contains("i_understand_my_ip_is_exposed"));

        // With the acknowledgement present it is allowed — the point is that it cannot
        // happen by omission, not that it is forbidden.
        let ok = Config::from_toml(&format!("{base}\ni_understand_my_ip_is_exposed = true\n"))
            .unwrap();
        assert_eq!(ok.providers.torrent.vpn.mode, VpnMode::None);
    }

    #[test]
    fn proxy_mode_always_forces_dht_off() {
        // Unverified whether librqbit tunnels DHT over SOCKS5, so we assume it leaks.
        let vpn = VpnConfig::default();
        assert_eq!(vpn.mode, VpnMode::Socks5);
        assert!(vpn.must_disable_dht());

        let external = VpnConfig { mode: VpnMode::External, ..Default::default() };
        assert!(!external.must_disable_dht());
    }

    #[test]
    fn disabled_torrents_skip_vpn_validation() {
        // No VPN configured at all is fine as long as nothing will torrent.
        Config::from_toml("[providers.torrent]\nenabled = false\n").unwrap();
    }

    #[test]
    fn a_presence_client_id_falls_back_but_never_to_an_empty_one() {
        // An empty string must not resolve: Discord refuses an unknown or blank id at handshake, so
        // treating one as configured would make the feature silently do nothing.
        let mut presence = PresenceConfig::default();
        assert_eq!(
            presence.resolved_client_id(),
            Some(DEFAULT_PRESENCE_CLIENT_ID),
            "an unset id falls back to the shipped application"
        );

        // Whitespace must fall through to the default rather than being taken literally: Discord
        // refuses a blank id at handshake, so treating one as configured would silently disable the
        // feature for anyone who cleared the field instead of removing the line.
        presence.client_id = Some("   ".into());
        assert_eq!(presence.resolved_client_id(), Some(DEFAULT_PRESENCE_CLIENT_ID));

        presence.client_id = Some("1234567890".into());
        assert_eq!(presence.resolved_client_id(), Some("1234567890"));
    }

    #[test]
    fn commit_threshold_is_bounded() {
        assert!(Config::from_toml("[playback]\ncommit_threshold = 0.0\n").is_err());
        assert!(Config::from_toml("[playback]\ncommit_threshold = 1.5\n").is_err());
        Config::from_toml("[playback]\ncommit_threshold = 0.85\n").unwrap();
    }

    #[test]
    fn unknown_keys_are_rejected_rather_than_ignored() {
        // A typo in a privacy-relevant key must not be silently discarded.
        let err = Config::from_toml("[providers.torrent.vpn]\nmoed = \"none\"\n").unwrap_err();
        assert!(err.to_string().contains("invalid TOML"), "got: {err}");
    }
}
