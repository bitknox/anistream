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

    /// Move `config.toml` aside so the next start reads the defaults: to `config.damaged-<date>`
    /// beside it, with a count after it when that is taken, and kept there. For an app whose
    /// start failed on a file nobody on the device can edit. Answers where the file went, or
    /// `None` when there was none.
    pub fn set_config_aside(&self) -> std::io::Result<Option<PathBuf>> {
        if !self.config_file.exists() {
            return Ok(None);
        }
        let stem = format!("config.damaged-{}", jiff::Timestamp::now().strftime("%Y-%m-%d"));
        let aside = (1..)
            .map(|n| if n == 1 { stem.clone() } else { format!("{stem}-{n}") })
            .map(|aside| self.config_file.with_file_name(aside))
            .find(|aside| !aside.exists())
            .expect("a free name");
        std::fs::rename(&self.config_file, &aside)?;
        Ok(Some(aside))
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
// Comparable, so a front end holding a copy can tell when its copy has moved and pass it on.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The language the terminal app speaks, as a tag from `anistream_i18n::languages()` such as
    /// `da` or `pt-BR`. Unset (or `"system"`) follows the system's language. `--lang` overrides it
    /// for one run, and a language the build lacks shows English.
    pub language: Option<String>,
    pub theme: ThemeConfig,
    pub playback: PlaybackConfig,
    pub providers: ProvidersConfig,
    pub trackers: TrackersConfig,
    pub downloads: DownloadsConfig,
    pub presence: PresenceConfig,
    pub network: NetworkConfig,
    pub updates: UpdatesConfig,
    pub syncplay: SyncplayConfig,
    pub notifications: NotificationsConfig,
    pub meta: MetaConfig,
    pub controller: ControllerConfig,
    /// Keybinding overrides, `action = "key"`. The help overlay is generated from the
    /// resolved map so it can never drift from what the keys actually do.
    pub keys: BTreeMap<String, String>,
}

/// Why a configuration could not be used, and where in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProblem {
    /// The line at fault, counting from 1, when the problem is in one place. A rule broken
    /// across settings — torrents on with no VPN mode — has none.
    pub line: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for ConfigProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl ConfigProblem {
    fn from_toml(src: &str, error: &toml::de::Error) -> Self {
        // The parser's own rendering quotes the source over several lines, which reads well in
        // a terminal and not at all in a one-line notice. The line number is what a person
        // needs to find it.
        let line = error.span().map(|span| {
            let before = src.as_bytes().get(..span.start).unwrap_or(src.as_bytes());
            1 + before.iter().filter(|&&b| b == b'\n').count()
        });
        Self { line, message: error.message().trim().to_owned() }
    }
}

impl Config {
    /// Parse from a TOML string, then validate.
    pub fn from_toml(src: &str) -> Result<Self, crate::Error> {
        Self::parse(src).map_err(|problem| crate::Error::Config(problem.to_string()))
    }

    /// [`Self::from_toml`], keeping where the problem is apart from what it is.
    pub fn parse(src: &str) -> Result<Self, ConfigProblem> {
        let cfg: Self = toml::from_str(src).map_err(|e| ConfigProblem::from_toml(src, &e))?;
        cfg.validate().map_err(|e| ConfigProblem {
            line: None,
            message: match e {
                crate::Error::Config(message) => message,
                other => other.to_string(),
            },
        })?;
        warn_unknown(&unknown_keys(src, &cfg));
        Ok(cfg)
    }

    /// Load from disk, falling back to defaults when the file does not exist.
    ///
    /// A missing config is normal on first run and must not be an error; a *malformed*
    /// one is, because silently ignoring it could mean silently ignoring a VPN setting.
    pub fn load(paths: &Paths) -> Result<Self, crate::Error> {
        Self::read(paths).map_err(|problem| {
            crate::Error::Config(format!("{}, {problem}", paths.config_file.display()))
        })
    }

    /// [`Self::load`], with the problem kept apart from the file it is in — for an interface
    /// that names the file its own way.
    pub fn read(paths: &Paths) -> Result<Self, ConfigProblem> {
        match std::fs::read_to_string(&paths.config_file) {
            Ok(src) => Self::parse(&src),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let cfg = Self::default();
                cfg.validate()
                    .map_err(|e| ConfigProblem { line: None, message: e.to_string() })?;
                Ok(cfg)
            }
            Err(e) => {
                Err(ConfigProblem { line: None, message: format!("cannot be read: {e}") })
            }
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

/// The keys in `src` that `cfg`, read from it, has no place for, as dotted paths: settings of a
/// newer version, or of an older one that this version dropped.
///
/// Such keys are read past rather than refused, since the apps and the terminal app share one
/// file and each release can be ahead of or behind the others, and a setting written with
/// [`crate::settings::write_keys`] leaves them where they are. The VPN table stays strict: a
/// misspelt key there would leave torrents unprotected without a word.
pub fn unknown_keys(src: &str, cfg: &Config) -> Vec<String> {
    let (Ok(given), Ok(known)) = (src.parse::<toml::Table>(), toml::Table::try_from(cfg))
    else {
        return Vec::new();
    };
    let mut unknown = Vec::new();
    unknown_in(&given, &known, "", &mut unknown);
    unknown
}

fn unknown_in(given: &toml::Table, known: &toml::Table, at: &str, unknown: &mut Vec<String>) {
    for (key, value) in given {
        let path = if at.is_empty() { key.clone() } else { format!("{at}.{key}") };
        match (value, known.get(key)) {
            (_, None) => unknown.push(path),
            (toml::Value::Table(given), Some(toml::Value::Table(known))) => {
                unknown_in(given, known, &path, unknown);
            }
            _ => {}
        }
    }
}

/// Said in the log once a process, however often the file is read.
fn warn_unknown(unknown: &[String]) {
    static WARNED: std::sync::Once = std::sync::Once::new();
    if !unknown.is_empty() {
        WARNED.call_once(|| {
            tracing::warn!(keys = ?unknown, "config keys this version doesn't know are kept and ignored");
        });
    }
}

/// Offline downloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// Streaming is different, deliberately: while an episode plays, its torrent uploads
    /// to peers over the guarded connection — watching already gives back. The session
    /// keeps the previous, current and next episode's swarms alive and retires older ones
    /// (each live swarm holds dozens of sockets, and file descriptors are finite). This key
    /// only decides whether a *finished download* keeps seeding.
    pub keep_seeding: bool,
    /// Which version of an episode a download takes: the one playback would stream, or the
    /// best a source offers. Read by the native apps' Downloads settings and by every download
    /// queued, the terminal's included.
    pub quality: DownloadQuality,
    /// How many of the next episodes of each show being watched are kept downloaded, `0` for
    /// none, where a show keeps no number of its own. Set from the native apps' Downloads
    /// settings, and acted on by the engine wherever it runs, the terminal's included: it queues
    /// what is missing as the viewer moves on, and deletes nothing.
    pub keep_next: u32,
    /// Whether downloads run over a cellular connection. Off by default, so a phone does not
    /// spend a data plan on its own. Only the native apps know which network they are on; the
    /// terminal keeps it as it is.
    pub on_cellular: bool,
}

/// Which version of an episode a download takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadQuality {
    /// The quality playback asks for, `playback.quality`: what the episode would stream at.
    #[default]
    Standard,
    /// The best version a source offers, and more space.
    Higher,
}

impl DownloadQuality {
    /// The vertical resolution a download ranks its streams toward, with `playback` being
    /// what playback asks for: [`crate::stream::Stream::quality_rank`] puts the highest first
    /// when nothing is too high.
    pub const fn resolution(self, playback: u32) -> u32 {
        match self {
            Self::Standard => playback,
            Self::Higher => u32::MAX,
        }
    }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// The richer presence: Discord's "Watching" with the cover, a progress bar, a link to the
    /// show on AniList and whether it is paused. Off by default, as each part of it is one more
    /// thing a third party is told or fetches. Ignored when `show_title` is off, since every part
    /// of it names the show.
    pub rich: bool,
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
        Self { enabled: false, client_id: None, show_title: true, rich: false }
    }
}

impl Default for DownloadsConfig {
    fn default() -> Self {
        Self {
            directory: None,
            merge_subtitles: true,
            keep_seeding: false,
            on_complete: None,
            quality: DownloadQuality::Standard,
            keep_next: 0,
            on_cellular: false,
        }
    }
}

/// Real-time upscaling, chosen as a look rather than a shader list: the chain itself adapts to
/// the source, the display, the battery and how the GPU is keeping up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Upscaling {
    /// mpv's own scalers.
    Off,
    /// ArtCNN: faithful line work and texture, nothing restyled.
    #[default]
    Natural,
    /// Anime4K: crisper lines and heavier cleanup. The old `anime4k-fast` and
    /// `anime4k-quality` settings read as this; how heavy a chain runs is now decided by the
    /// budget rather than by the setting.
    #[serde(alias = "anime4k-fast", alias = "anime4k-quality")]
    Sharp,
}

impl Upscaling {
    pub const ALL: [Self; 3] = [Self::Off, Self::Natural, Self::Sharp];

    /// The name the config file uses.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Natural => "natural",
            Self::Sharp => "sharp",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// Whose interface the player wears. See [`PlaybackConfig::player_ui`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlayerUi {
    #[default]
    Anistream,
    Mpv,
}

/// Watch parties, on a Syncplay server.
///
/// The engine speaks Syncplay itself and keeps its own player with the room. An app starts a
/// party on purpose, from its form; these are the terminal's way in, and the defaults the form
/// opens with. A party writes no private history but the episodes it finishes: the room pausing
/// or jumping is never kept as where you left off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncplayConfig {
    /// Every play from the terminal joins the party set below, not only those `y` starts. The
    /// apps start parties when asked, and this does nothing there.
    pub enabled: bool,
    /// `host:port` of the server the terminal's parties use. Empty until chosen: there is no
    /// default server, since a public one is run by people the viewer may not know.
    pub server: String,
    /// The room the terminal's parties join. When unset, a new one with a name nobody can guess,
    /// shared through its invite link.
    pub room: Option<String>,
    /// The name shown to the room.
    pub name: String,
    /// Servers used for a party before, the last first: what the form offers.
    pub recent_servers: Vec<String>,
    /// Reserved. Accepted so existing configs keep parsing, but has no effect since the engine
    /// speaks Syncplay itself and no longer starts the Syncplay program.
    pub binary: String,
}

impl Default for SyncplayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server: String::new(),
            room: None,
            name: "anistream".into(),
            recent_servers: Vec::new(),
            binary: "syncplay".into(),
        }
    }
}

impl SyncplayConfig {
    /// How many servers [`SyncplayConfig::recent_servers`] keeps.
    pub const RECENT: usize = 5;

    /// The servers used before with `server` first, as the next party's form should offer them.
    pub fn used(&self, server: &str) -> Vec<String> {
        let server = server.trim();
        let mut recent = vec![server.to_owned()];
        recent.extend(
            self.recent_servers
                .iter()
                .filter(|s| !s.trim().eq_ignore_ascii_case(server) && !s.trim().is_empty())
                .cloned(),
        );
        recent.truncate(Self::RECENT);
        recent
    }
}

/// The airing digest: what aired for shows in your history while the app was closed. And the
/// native apps' notifications of new episodes, which are off until asked for.
///
/// A digest, never a stream of pings — one line on launch, and only when there is
/// something to say. The tool must not get in the way of the person using it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationsConfig {
    /// Say on launch how many new episodes aired for shows you watch.
    pub airing_digest: bool,
    /// Send the same line to the desktop as well. Off by default — the toast already
    /// says it, and a notification about the app you just opened is noise.
    pub desktop: bool,
    /// Let a native app notify you of each new episode of the shows you follow, from its
    /// background check. Off until you turn it on.
    pub episodes: bool,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self { airing_digest: true, desktop: false, episodes: false }
    }
}

/// A game controller, in the apps that take one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ControllerConfig {
    /// Drive the app with a connected controller. On unless switched off.
    pub enabled: bool,
    /// Swap A and B, for a controller laid out as Nintendo's are.
    pub swap_face: bool,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self { enabled: true, swap_face: false }
    }
}

/// The metadata failover, and how its titles are shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetaConfig {
    /// Fall back to Tenrai — MyAnimeList's mirror — when AniList cannot answer, before
    /// resorting to cached data. Fresh first, always; the ladder only descends on failure.
    pub fallback: bool,
    /// Which form of each title is shown: AniList's own choice for the signed-in account, or
    /// English or romaji whoever is signed in. Every front end shows what this says.
    pub title_language: TitleChoice,
    /// Show titles AniList marks adult in Search and the lists, with its adult genre and tags
    /// among the filters. Off unless asked for.
    pub adult: bool,
    /// Episode discussions: look up an episode's AniList forum thread once it counts as watched,
    /// and open it in the app. On unless switched off.
    pub discussions: bool,
    /// Load the images in discussion comments inline. Off, each is a line that opens it in the
    /// browser, and the app asks nothing of the hosts people post images on.
    pub discussion_images: bool,
    /// Calendar shows only the shows on the person's list. Off, it opens on everything airing;
    /// the choice is remembered.
    pub calendar_mine: bool,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            fallback: true,
            title_language: TitleChoice::default(),
            adult: false,
            discussions: true,
            discussion_images: false,
            calendar_mine: false,
        }
    }
}

/// Which form of a title to show, as `[meta] title_language` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitleChoice {
    /// As the signed-in AniList account reads AniList: romaji, English or native. English
    /// first when nobody is signed in, which is how titles were always shown.
    #[default]
    Anilist,
    English,
    Romaji,
}

impl TitleChoice {
    /// As the file writes it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Anilist => "anilist",
            Self::English => "english",
            Self::Romaji => "romaji",
        }
    }

    /// Read back what [`Self::name`] wrote.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Anilist, Self::English, Self::Romaji].into_iter().find(|c| c.name() == name)
    }
}

/// The update check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
    /// Set `false` to suppress the eyecatch wipe and all other motion.
    pub motion: bool,
    /// Reserved. Read so existing configs keep it, but currently has no effect: every glyph in the UI is plain
    /// Unicode, and requiring a patched font is both a portability hazard and one of the
    /// clearest templated-TUI tells.
    pub nerd_font: bool,
    /// When a title's trailer plays by itself, muted, in place of its banner: on any network
    /// (`always`), off cellular and Low Data Mode (`wifi-only`), or never (`off`). The native
    /// apps play them; the terminal keeps the setting and plays none.
    pub trailers: TrailerAutoplay,
    /// Whether a cover the pointer rests on grows into a card with the title's details and,
    /// where `trailers` lets one play, its trailer. The Mac's grids have it; the terminal keeps
    /// the setting and has no pointer to rest.
    pub hover_cards: bool,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Adaptive,
            motion: true,
            nerd_font: false,
            trailers: TrailerAutoplay::Always,
            hover_cards: true,
        }
    }
}

/// When trailers play by themselves. See [`ThemeConfig::trailers`]. Whatever it says, Low Data
/// Mode, Reduce Motion, power saving and a screen reader still hold a trailer back, and Play
/// trailer plays one because the viewer asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrailerAutoplay {
    /// On any network but a constrained one.
    #[default]
    Always,
    /// Only on a network that is neither cellular nor constrained.
    WifiOnly,
    Off,
}

impl TrailerAutoplay {
    pub const ALL: [Self; 3] = [Self::Always, Self::WifiOnly, Self::Off];

    /// The name the config file uses.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::WifiOnly => "wifi-only",
            Self::Off => "off",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.name() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Inherit the terminal's background and set foreground colours only, so anistream
    /// sits naturally beside the user's other tools. The palette variant is chosen by
    /// querying the real background over OSC 11.
    #[default]
    Adaptive,
    /// Paint the dusk-indigo ground for a fully controlled look.
    Immersive,
    /// The terminal's own ground with the palette for a light one, for a terminal that does not
    /// answer the background query.
    Light,
    /// The terminal's own ground with the palette for a dark one.
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// Offer a skip button while the opening or ending plays, where the skip is not automatic.
    ///
    /// With this, `skip_opening` and `mark_chapters` all off, nothing uses the opening and
    /// ending times, so aniskip is not asked for them.
    pub skip_button: bool,
    /// How many seconds Up Next counts down before the next episode plays, `0` for no
    /// countdown: the card waits for Play now. The native apps' player reads it; the terminal's
    /// companion keeps its own ten seconds.
    pub up_next_countdown: u32,
    /// Blur the stills of unwatched episodes after the next one; titles and synopses stay. On by
    /// default, and a show can say otherwise. The native apps draw it; the terminal shows no
    /// stills.
    pub spoiler_protection: bool,
    /// Put the opening and ending on the seek bar as chapter markers, where the stream carries
    /// no chapters of its own.
    ///
    /// A stream a provider hands over is almost always HLS, which cannot carry chapters at all,
    /// so the bar is blank in exactly the case where the skip times are known. A file that does
    /// declare chapters keeps them untouched.
    pub mark_chapters: bool,
    /// Start mpv fullscreen. The viewer can still leave it with mpv's own `f` key;
    /// this only chooses how the window opens.
    pub fullscreen: bool,
    /// Real-time upscaling: `natural` (ArtCNN), `sharp` (Anime4K) or `off`.
    ///
    /// The shaders ship inside the binary, so this is a choice of look rather than a scavenger
    /// hunt. Which networks actually run is decided per file: nothing heavy when the source
    /// already fills the display, a lighter chain on battery, and one step lighter whenever
    /// frames start dropping. HDR sources only get chroma work.
    pub upscaling: Upscaling,
    /// Whose interface the player wears.
    ///
    /// `anistream` brings its own: uosc with seekbar thumbnails, the opening and ending marked
    /// on the timeline, the skip button and in-player menus, in the app's colours and font. It
    /// still reads your `mpv.conf` but does not load the scripts in your mpv folder. `mpv` leaves
    /// your own setup as it is and adds only the companion script that the skip button, Up Next
    /// and the menus need.
    pub player_ui: PlayerUi,
    /// Even out loud and quiet passages, so dialogue stays audible at low volume without the
    /// opening being loud. One audio filter, toggled from the player's menu as well.
    pub night_mode: bool,
    /// How HDR video is mapped to an SDR display. `auto` is mpv's own choice, which is right
    /// for most screens; the player's menu offers the alternatives while HDR is playing.
    pub tone_mapping: String,
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
            upscaling: Upscaling::Natural,
            player_ui: PlayerUi::Anistream,
            night_mode: false,
            tone_mapping: "auto".into(),
            skip_opening: true,
            mark_chapters: true,
            skip_button: true,
            skip_filler: false,
            up_next_countdown: 10,
            spoiler_protection: true,
            fullscreen: false,
            players: vec!["mpv".into(), "external".into()],
            mpv_binary: "mpv".into(),
            mpv_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginsConfig {
    /// Linear-memory ceiling per plugin, in mebibytes.
    pub memory_mb: usize,
    /// Wall-clock budget for one plugin call.
    ///
    /// Enforced by epoch interruption, which can stop a guest mid-loop — so this is a real bound
    /// rather than a hope that the plugin checks a flag.
    pub deadline_secs: u64,
    /// Per-plugin settings, keyed by plugin id: `[providers.plugins.settings.<id>]`.
    ///
    /// Free-form string pairs the host never interprets; a plugin reads them one key at a time
    /// through the lent `config-get`. This is how a source that needs an account or an API key
    /// receives it without the plugin gaining any filesystem access — and a plugin must work
    /// with all of them unset, because most users will never write this table.
    pub settings:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self { memory_mb: 64, deadline_secs: 20, settings: Default::default() }
    }
}

/// Torrent source settings.
///
/// Disabled by default. Torrent traffic exposes your IP to every peer in the swarm, so
/// this stays off until a VPN mode has been chosen deliberately.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// Giving back after an episode has been watched.
    pub seeding: SeedingConfig,
    /// How much of what has been streamed stays on disk, in gigabytes.
    ///
    /// A stream's files are deleted when it is let go, unless it is seeding or a download owns
    /// it. What is still on disk past this — from earlier runs, or episodes kept for a rewatch —
    /// goes oldest first, at launch and while the app runs. Downloads count toward it but are
    /// never removed by it. `0` keeps nothing that is not in use.
    pub cache_limit_gb: u32,
    /// Whether torrents run over a cellular connection. Off by default. Only the native apps
    /// know which network they are on; the terminal keeps it as it is.
    pub on_cellular: bool,
}

impl Default for TorrentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            vpn: VpnConfig::default(),
            rss_url: None,
            trackers: Vec::new(),
            curation_url: None,
            max_seeders_age_days: None,
            seeding: SeedingConfig::default(),
            // Room for a dozen episodes at 1080p: enough that resuming or rewatching last
            // night's episodes is instant, small enough that nobody's disk fills over a season.
            cache_limit_gb: 20,
            on_cellular: false,
        }
    }
}

/// Seeding streamed episodes after they stop playing, and a cap on upload.
///
/// An episode uploads to its swarm while it plays either way. These decide what happens after:
/// with `ratio` at 0 a finished episode stops seeding once newer ones push it out of the session,
/// within minutes; above 0 it keeps seeding, while anistream is open, until it has given back
/// that multiple of what it took or `hours` have passed. Off by default for the same reason as
/// `downloads.keep_seeding`: seeding advertises you as a source for as long as it runs, and that
/// is not something to switch on for anyone silently.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SeedingConfig {
    /// Seed each watched episode until it has uploaded this many times what was downloaded.
    /// `0` turns seeding after watching off.
    pub ratio: f64,
    /// The longest any one episode keeps seeding, whatever its ratio.
    pub hours: u32,
    /// Total upload across every torrent, in KiB/s. `0` is unlimited. Worth setting on a slow
    /// uplink, where seeding could otherwise crowd out the requests the playing episode needs.
    pub upload_limit_kib: u32,
}

impl Default for SeedingConfig {
    fn default() -> Self {
        Self { ratio: 0.0, hours: 24, upload_limit_kib: 0 }
    }
}

impl SeedingConfig {
    /// Whether watched episodes keep seeding at all.
    pub fn enabled(&self) -> bool {
        self.ratio > 0.0 && self.hours > 0
    }
}

impl TorrentConfig {
    /// [`Self::cache_limit_gb`] in bytes.
    pub fn cache_limit_bytes(&self) -> u64 {
        u64::from(self.cache_limit_gb).saturating_mul(1_000_000_000)
    }

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
        // An indexer set is held to being one whether or not torrents are on: the native apps'
        // torrent index is filled in before the source is turned on, and a typo kept until then
        // would be refused only by the switch, which says nothing of the address.
        let indexer = self.rss_url.as_deref().map(str::trim).filter(|url| !url.is_empty());
        if let Some(url) = indexer
            && !url.starts_with("http://")
            && !url.starts_with("https://")
        {
            return Err(crate::Error::Config(
                "providers.torrent.rss_url must be an http or https URL".into(),
            ));
        }
        if !self.enabled {
            return Ok(());
        }
        // No indexer ships with anistream, so an enabled source with no endpoint would be
        // silently inert. Say so instead — in words, and short: it is what the Settings toggle
        // answers with in one toast line, which cut the dotted keys it used to quote before the
        // part that said what to do.
        if indexer.is_none() {
            return Err(crate::Error::Config(
                "torrents need an indexer; set rss_url under [providers.torrent]".into(),
            ));
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
        if self.vpn.on_leak == LeakAction::Warn
            && !self.vpn.i_understand_a_failed_check_keeps_torrents_running
        {
            return Err(crate::Error::Config(
                "providers.torrent.vpn.on_leak is \"warn\": set \
                 i_understand_a_failed_check_keeps_torrents_running = true to confirm, or \
                 choose pause or stop"
                    .into(),
            ));
        }
        Ok(())
    }
}

/// How torrent traffic is kept off your own address.
///
/// librqbit offers a SOCKS5 proxy but **no** bind-to-interface option, so real interface
/// binding has to happen outside the process. This config covers what we can enforce and
/// the docs are explicit about the rest, rather than implying a guarantee we cannot make.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// Strict, unlike the rest: a key misspelt here would quietly leave torrents unprotected.
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
    /// Required when `on_leak = "warn"`, for the same reason: a failed check then leaves
    /// torrents running on a connection that may be your own.
    pub i_understand_a_failed_check_keeps_torrents_running: bool,
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
            i_understand_a_failed_check_keeps_torrents_running: false,
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
    /// Keep torrents running and say the check failed. Requires
    /// `i_understand_a_failed_check_keeps_torrents_running`, as `mode = "none"` requires its
    /// own acknowledgement.
    Warn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
            // Kitsu is on out of the box because it is the one tracker with nothing to
            // register and nothing to configure — being enabled only makes it *visible*
            // on the Accounts screen, one sign-in away. Nothing is queued and nothing is
            // pulled until credentials exist, so local history still stands alone. Every
            // other tracker needs config anyway, and enabling rides along with that.
            enabled: vec!["kitsu".into()],
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// to copy it by hand, which needs no port and works over SSH. `"implicit"` is the native
    /// apps': their own client through the system's sign-in sheet, with no secret, which the
    /// terminal cannot catch on a loopback port.
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceAuthConfig {
    pub client_id: Option<String>,
    /// Trakt requires this on the token exchange. Simkl wants one only from an app registered
    /// as a server app; a device or command-line app has none.
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
        // The implicit grant is a native app's, with the app's own client and no secret: none
        // is asked of the file, whose client the terminal's code grant would need.
        if self.enabled.iter().any(|t| t == "anilist") && self.anilist.flow != "implicit" {
            let blank =
                |field: &Option<String>| field.as_deref().unwrap_or("").trim().is_empty();
            if blank(&self.anilist.client_id) || blank(&self.anilist.client_secret) {
                return Err(crate::Error::Config(
                    "trackers.anilist needs both client_id and client_secret when \"anilist\" \
                     is enabled — the terminal signs in with the authorization code grant, which \
                     needs the secret. Register a client at \
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
        if !matches!(self.anilist.flow.as_str(), "loopback" | "paste" | "implicit") {
            return Err(crate::Error::Config(format!(
                "trackers.anilist.flow must be \"loopback\", \"paste\" or \"implicit\", got {:?}",
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        }
    }

    #[test]
    fn a_config_set_aside_is_kept_and_the_defaults_read_in_its_place() {
        let dir = std::env::temp_dir().join("anistream-config-set-aside");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = paths_in(&dir);
        assert_eq!(paths.set_config_aside().unwrap(), None, "no file, nothing moved");

        std::fs::write(&paths.config_file, "[playback\nupscaling = ").unwrap();
        assert!(Config::read(&paths).is_err());
        let aside = paths.set_config_aside().unwrap().expect("moved");
        let name = aside.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("config.damaged-20"), "{name}");
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), "[playback\nupscaling = ");
        assert!(!paths.config_file.exists());
        assert_eq!(Config::read(&paths).unwrap(), Config::default());

        // A second the same day takes the next name, and the first stays.
        std::fs::write(&paths.config_file, "junk = [").unwrap();
        let again = paths.set_config_aside().unwrap().expect("moved");
        assert_eq!(again.file_name().unwrap().to_string_lossy(), format!("{name}-2"));
        assert!(aside.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_old_anime4k_settings_still_read_as_sharp() {
        // Existing configs said `anime4k-fast` or `anime4k-quality`; refusing them would turn
        // a working config into a startup error. Both chose Anime4K's look.
        for old in ["anime4k-fast", "anime4k-quality"] {
            let cfg = Config::from_toml(&format!("[playback]\nupscaling = \"{old}\"")).unwrap();
            assert_eq!(cfg.playback.upscaling, Upscaling::Sharp, "{old}");
        }
        let off = Config::from_toml("[playback]\nupscaling = \"off\"").unwrap();
        assert_eq!(off.playback.upscaling, Upscaling::Off, "an explicit off stays off");
        assert_eq!(Config::default().playback.upscaling, Upscaling::Natural);
    }

    #[test]
    fn the_language_is_a_root_key_and_follows_the_system_when_unset() {
        assert_eq!(Config::default().language, None);
        let cfg = Config::from_toml("language = \"da\"\n[theme]\nmotion = false").unwrap();
        assert_eq!(cfg.language.as_deref(), Some("da"));
    }

    #[test]
    fn hover_cards_are_on_until_the_file_turns_them_off() {
        assert!(Config::default().theme.hover_cards);
        let cfg = Config::from_toml("[theme]\nhover_cards = false").unwrap();
        assert!(!cfg.theme.hover_cards);
        // Separate from trailers: one off leaves the other as it was.
        assert_eq!(cfg.theme.trailers, TrailerAutoplay::Always);
    }

    #[test]
    fn trailers_play_always_until_the_file_says_otherwise() {
        assert_eq!(Config::default().theme.trailers, TrailerAutoplay::Always);
        let cfg = Config::from_toml("[theme]\ntrailers = \"wifi-only\"").unwrap();
        assert_eq!(cfg.theme.trailers, TrailerAutoplay::WifiOnly);
        assert!(Config::from_toml("[theme]\ntrailers = \"sometimes\"").is_err());
        for choice in TrailerAutoplay::ALL {
            assert_eq!(TrailerAutoplay::from_name(choice.name()), Some(choice));
        }
    }

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
    fn the_native_apps_settings_default_to_what_their_boards_show() {
        let cfg = Config::default();
        assert_eq!(cfg.playback.up_next_countdown, 10, "Up Next counts down from ten");
        assert!(cfg.playback.spoiler_protection, "on by default");
        assert_eq!(cfg.downloads.quality, DownloadQuality::Standard);
        assert_eq!(cfg.downloads.keep_next, 0, "nothing kept unasked");
        assert!(!cfg.downloads.on_cellular && !cfg.providers.torrent.on_cellular);
        assert_eq!(cfg.providers.torrent.vpn.on_leak, LeakAction::Pause);
        assert_eq!(DownloadQuality::Standard.resolution(720), 720);
        assert_eq!(DownloadQuality::Higher.resolution(720), u32::MAX);
    }

    #[test]
    fn warning_on_a_failed_check_needs_its_own_acknowledgement() {
        // As `mode = "none"` does: a failed check then leaves torrents running.
        let on = r#"
            [providers.torrent]
            enabled = true
            rss_url = "https://indexer.example/rss?q={query}"
            [providers.torrent.vpn]
            mode = "external"
            on_leak = "warn"
        "#;
        let err = Config::from_toml(on).unwrap_err().to_string();
        assert!(err.contains("i_understand_a_failed_check_keeps_torrents_running"), "{err}");
        let acknowledged =
            format!("{on}i_understand_a_failed_check_keeps_torrents_running = true\n");
        let cfg = Config::from_toml(&acknowledged).unwrap();
        assert_eq!(cfg.providers.torrent.vpn.on_leak, LeakAction::Warn);
    }

    #[test]
    fn only_the_zero_config_tracker_is_enabled_by_default() {
        // History has to stand alone: the app is fully usable with no account. Kitsu is
        // the exception in *visibility* only — it needs no registration, so it appears on
        // the Accounts screen out of the box — while sync stays inert until a sign-in,
        // which the enqueue and pull gates on `is_authenticated` guarantee.
        let cfg = Config::default();
        assert_eq!(cfg.trackers.enabled, vec!["kitsu".to_string()]);
        assert!(!cfg.trackers.is_enabled("anilist"));
        assert!(cfg.trackers.is_enabled("kitsu"));
        // And it must validate without any [trackers.kitsu] section existing.
        assert!(cfg.validate().is_ok());
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
    fn the_implicit_flow_asks_the_file_for_no_secret() {
        // A native app's: its own client, through the system's sign-in sheet.
        let cfg = Config::from_toml(
            r#"
            [trackers]
            enabled = ["anilist"]
            [trackers.anilist]
            flow = "implicit"
            "#,
        )
        .expect("no secret needed");
        assert_eq!(cfg.trackers.anilist.flow, "implicit");
        // The code grant still needs both halves.
        let loopback =
            "[trackers]\nenabled = [\"anilist\"]\n[trackers.anilist]\nclient_id = \"1\"\n";
        assert!(Config::from_toml(loopback).unwrap_err().to_string().contains("client_secret"));
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
        // Said for a person, and short enough for the Settings toast at 80 columns.
        let crate::Error::Config(reason) = err else { panic!("not a config error: {err}") };
        assert!(!reason.contains("enabled is true"), "got: {reason}");
        assert!(format!("not saved: {reason}").chars().count() <= 75, "got: {reason}");
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
    fn an_indexer_set_is_checked_with_torrents_off_and_none_is_no_indexer() {
        // Filled in on the native apps' torrent sheet before the source is turned on.
        let off =
            |url: &str| Config::from_toml(&format!("[providers.torrent]\nrss_url = {url:?}\n"));
        let err = off("indexer.example/rss").expect_err("not an address").to_string();
        assert!(err.contains("http or https"), "got: {err}");
        assert!(off("https://indexer.example/rss?q={query}").is_ok());
        // Blank is unset, with torrents off as with them on.
        assert!(off("  ").is_ok());
        let on = "[providers.torrent]\nenabled = true\nrss_url = \" \"\n\
                  [providers.torrent.vpn]\nmode = \"external\"\n";
        let err = Config::from_toml(on).expect_err("no indexer").to_string();
        assert!(err.contains("torrents need an indexer"), "got: {err}");
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
    fn the_stream_cache_is_capped_unless_told_otherwise() {
        // An uncapped cache is how the disk filled, so an unset key must still mean a cap.
        let unset = Config::from_toml("[providers.torrent]\n").unwrap();
        assert_eq!(unset.providers.torrent.cache_limit_gb, 20);
        assert_eq!(unset.providers.torrent.cache_limit_bytes(), 20_000_000_000);
        let set = Config::from_toml("[providers.torrent]\ncache_limit_gb = 5\n").unwrap();
        assert_eq!(set.providers.torrent.cache_limit_bytes(), 5_000_000_000);
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
        assert!(err.to_string().contains("unknown field `moed`"), "got: {err}");
    }

    #[test]
    fn a_key_this_version_does_not_know_is_read_past_and_named() {
        // A newer app's setting, and one an older version had, beside known ones.
        let src = "[theme]\nmode = \"immersive\"\nsparkle = true\n\n[playback]\nqualty = 720\n\n\
                   [future]\nwhatever = 1\n";
        let cfg = Config::parse(src).expect("unknown keys are tolerated");
        assert_eq!(
            cfg.theme,
            Config::from_toml("[theme]\nmode = \"immersive\"\n").unwrap().theme
        );
        let mut unknown = unknown_keys(src, &cfg);
        unknown.sort();
        assert_eq!(unknown, ["future", "playback.qualty", "theme.sparkle"]);
        assert!(unknown_keys("[theme]\nmode = \"immersive\"\n", &cfg).is_empty());
    }

    #[test]
    fn a_problem_names_its_line_and_what_is_wrong_on_one_line() {
        // What the interface shows when the file cannot be used: a person has to find it.
        let typo = Config::parse(
            "[theme]\nmode = \"adaptive\"\n\n[providers.torrent.vpn]\nmoed = 1\n",
        )
        .unwrap_err();
        assert_eq!(typo.line, Some(5), "{typo:?}");
        assert!(typo.message.starts_with("unknown field `moed`"), "{typo:?}");

        let syntax = Config::parse("[playback]\nquality = 1080 ]\n").unwrap_err();
        assert_eq!(syntax.line, Some(2), "{syntax:?}");

        let wrong_type = Config::parse("[playback]\nquality = \"high\"\n").unwrap_err();
        assert_eq!(wrong_type.line, Some(2), "{wrong_type:?}");

        for problem in [&typo, &syntax, &wrong_type] {
            assert!(!problem.to_string().contains('\n'), "{problem}");
        }

        // A rule across settings has no one line to point at, and names the setting instead.
        let rule = Config::parse("[playback]\ncommit_threshold = 3.0\n").unwrap_err();
        assert_eq!(rule.line, None);
        assert!(rule.message.contains("commit_threshold"), "{rule:?}");
    }
}
