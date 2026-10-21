//! The engine: the work behind the front ends, with no terminal in it.
//!
//! Everything that talks to the outside world on the interface's behalf lives here, and the
//! routing that hands each command to it: the lists, the calendar, search and a title's page, a
//! title's episodes and their source match, playback and the player, sources and their plugins,
//! sync with the trackers, downloads, artwork, plugin stores, settings and the background jobs.
//! The terminal front end keeps its terminal setup and event loop in the binary. No terminal crate is a dependency, not even transitively, so a front end
//! that is not a terminal can link this; `tools/check-boundaries.sh` holds it to that in CI.
//!
//! A front end drives it through an [`engine::Engine`]: started from [`options::EngineOptions`]
//! and the [`host::Host`] it runs on, sent commands, and read through its
//! [`engine::EventStream`]. Only the command line reaches past it, to [`services`] and the work's
//! own functions, for the one thing it does before it exits.
//!
//! An example or an integration test drives the *real* orchestration here rather than a copy of
//! it. That matters most for [`playback`], where the thing being verified — a torrent stream
//! reaching mpv and the resulting position landing in SQLite — cannot be covered by unit tests
//! at all.
//!
//! # The `desktop` feature
//!
//! On by default, and what the terminal app is built with. It carries what has no story on a
//! phone:
//!
//! - **the mpv child process** each playback runs in, and the check of the player at launch:
//!   `playback::player`, the opener `playback::link::opener` gives a play, and
//!   `anistream-player` beneath them. The session itself, the window kept between episodes and
//!   handed from one to the next, the upscaling shaders and the governor that picks them run
//!   in any build, behind `playback::link::PlayerLink`;
//! - **Syncplay**, which a play can be handed to as a watch party;
//! - **Discord's presence**, said while an episode plays;
//! - **the browser redirect** AniList and MyAnimeList sign in through: a listener on
//!   `127.0.0.1` waiting for the code;
//! - **`osascript` and `notify-send`**, and links opened by the system's own handler: the
//!   `host::DesktopHost`, with `host::desktop_secrets`, the keychain or `0600` files the
//!   desktop keeps tokens in;
//! - **`pmset`**, read for whether a laptop is on battery: `playback::power::SystemPower`.
//!
//! Built without it, the engine has none of these, nor `anistream-player` and `open`, and a
//! front end gets instead:
//!
//! - every play in the player the front end hosts, driven as mpv is through
//!   `playback::link::hosted`: what it is to do comes as `Event::PlayerOps`, what its own
//!   interface shows as `Event::PlayerUi`, and what it reports goes back as
//!   `Command::PlayerReport`. A watch party is answered with an alert, Syncplay being a desktop
//!   program; a front end that turns the hosted player off (`EngineOptions::hosted_player`) has
//!   every play answered with an alert that playback isn't built into it;
//! - the battery as the front end reports it, `playback::power::ReportedPower`, unknown until
//!   it does;
//! - a trailer opened wherever the host opens links, rather than in mpv;
//! - sign-in through the device flow, Simkl's and Trakt's, and Kitsu's credentials, none of
//!   which needs a redirect; AniList and MyAnimeList answered with an alert instead;
//! - tokens kept in the store its host offers, [`host::Host::secrets`], or in `0600` files under
//!   the data directory when it offers none;
//! - links and notifications through its host, as on the desktop.
//!
//! What stays is everything a player of any kind would need: the lists, search and the titles'
//! pages, episodes and their match, the streams found for an episode and ahead of the next one
//! (`LoadSources`, `Prefetch`), sync with the trackers, downloads and the torrent source, the
//! plugins and their stores, settings and artwork. The mending proxy stays too,
//! `playback::mend`: a stream served in disguise is as unplayable for a phone's player as for
//! mpv, and a loopback server runs on a phone as it does on a desktop.
//!
//! So a front end on a phone has to:
//!
//! - implement [`host::Host`] in full: open a link in the browser or the app that handles it,
//!   post a notification, and keep tokens in the platform's keychain through
//!   [`host::Host::secrets`], since files are the fallback and are said to be less;
//! - choose the background work its platform allows, in [`options::Background`]: the system
//!   may not let a sync loop run while the app is away, and may run its own schedule instead;
//! - not offer playback, or a sign-in to AniList or MyAnimeList, until the engine can hand it
//!   a stream to play and a redirect to take.

/// Configuration shared with the long-running background tasks.
///
/// The Settings screen edits the app's own copy, and anything holding a snapshot from
/// launch would keep acting on values the user has already changed — which is how a
/// download folder, a seeding choice or an upscaler stayed put until a restart. Readers
/// take a snapshot per unit of work rather than holding the guard: these are async tasks,
/// and a `std` guard held across an `await` is how a future stops being `Send`.
pub type SharedConfig = std::sync::Arc<std::sync::RwLock<anistream_core::config::Config>>;

pub mod artwork;
pub mod background;
pub mod cache;
pub mod catalog;
pub mod config;
pub mod data;
pub(crate) mod dispatch;
pub mod downloads;
pub mod engine;
pub mod error;
pub mod host;
pub mod jobs;
pub mod logged;
pub mod options;
pub mod outbox;
pub mod playback;
pub mod plugin_stores;
pub mod protection;
pub mod services;
pub mod settings;
pub mod show_settings;
pub mod sources;
pub(crate) mod state;
pub mod titles;
pub mod tracking;
pub mod updates;

/// Now, in seconds since the Unix epoch, by the system clock: what the work dates and ages
/// things by.
pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
