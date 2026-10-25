//! Wiring playback to history.
//!
//! The policy lives in [`anistream_playback::PlaybackTracker`], which is pure and tested. This
//! is the part that talks to the world: it opens a player, feeds its events to the tracker, and
//! turns the resulting actions into rows in the local database.
//!
//! The player is whatever the [`Machine`] opens, through a [`PlayerLink`]: mpv's child process
//! on the desktop. So this runs in any build; without the `desktop` feature there is no player
//! to open yet, and nothing starts a playback.
//!
//! History is written **before** anything is pushed anywhere. Local is the source of truth, so
//! if a tracker push fails the watch is still recorded and the outbox can retry.
//!
//! # Layout
//!
//! The playback run is here: the play a front end asks for, where it goes when it cannot play
//! in mpv, and the streams it tries in one window until one plays. One file's events, the
//! viewer's controls, what the companion shows, the picture, what is written down of the watch,
//! skipping, the subtitles fetched ahead and the Discord presence each have a file of their own.

use std::sync::Arc;

use anistream_core::{
    media::Translation,
    stream::{Stream, StreamKind},
    traits::PlaybackRequest,
};
use anistream_model::{
    command::PlayerCommand,
    event::Event,
    notice::Notice,
    playback::{Ending, Successor},
};
// What the episode table says of the episodes around this one: the front end sends them ahead
// of a play, so they are the model's, and still named from here for whatever plays.
pub use anistream_model::playback::{EpisodeEntry, NextEpisode};
use anistream_net::HttpClient;
use anistream_playback::{SkipInterval, TrackAction, TrackPreferences, companion};
use anistream_store::Store;
use tokio::sync::mpsc;

use crate::{
    host::Host,
    outbox::Outbox,
    playback::{
        PlaybackContext,
        health::Servers,
        link::{LinkError, Overlay, PlayerLink, PlayerOpener},
        picture::HdrKind,
        power::PowerSource,
        seat::{PartyHub, Seat, SeatReport},
        shaders::Look,
        slot::{Player, PlayerSlot},
    },
};

mod companion_state;
mod controls;
mod events;
mod presence;
#[cfg(test)]
mod scripted;
mod skips;
mod subtitles;
mod timeline;
mod upscaling;
mod watch;

pub use skips::{SkipTimes, offered};
pub use upscaling::PictureState;

use companion_state::fetch_still;
use subtitles::{SUBTITLE_WAIT, localise_subtitles, sanitise};
use watch::remember_tracks;

/// An unguessable-enough path token for one playback attempt's mender.
///
/// Not a security boundary — binding to loopback is — but it keeps an unrelated local process
/// from stumbling onto the proxy, and differs per attempt so a stale connection from a failed
/// stream cannot address the next one's.
fn mender_token(context: &PlaybackContext, attempt: usize) -> String {
    // Random, because the parts identifying the playback were all guessable: the AniList id is
    // the show on screen, the episode a small integer, the attempt almost always zero, and the
    // timestamp a few seconds wide. That is a few thousand candidates against a scannable
    // ephemeral port, and the proxy will fetch any URL named in the path — so a guessable
    // token made it a fetch primitive for anything else running on the machine.
    use rand::Rng;
    let mut bytes = [0_u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    let random: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

    // The identifying part is kept in front of it: a token in a log line should say which
    // playback it belonged to, and it costs nothing next to 128 bits.
    format!("{}-{}-{attempt}-{random}", context.anilist_id.get(), sanitise(&context.episode))
}

/// The settings one playback runs under, read from the config when it starts.
#[derive(Debug, Clone)]
pub struct PlaySettings {
    pub threshold: f64,
    pub auto_skip: bool,
    /// Put the opening and ending on the seek bar where the stream has no chapters of its own,
    /// from `playback.mark_chapters`.
    pub mark_chapters: bool,
    /// Offer a skip button while the opening or ending plays, where the skip is not automatic,
    /// from `playback.skip_button`.
    pub skip_button: bool,
    /// Open mpv fullscreen, from `playback.fullscreen`.
    pub fullscreen: bool,
    pub subtitle_language: Option<String>,
    /// Whether a finished episode rolls on to the next, from `playback.auto_next`.
    pub auto_next: bool,
    pub night_mode: bool,
    /// Keep the viewer's audio and subtitle picks and subtitle delay per series, from
    /// `playback.persist_speed` — which has always promised "speed and track choice".
    pub remember_tracks: bool,
    pub look: Look,
    /// Where the shaders were written. `None` turns runtime upscaling off entirely — the
    /// cache could not be written, or the user passes their own `--glsl-shaders`.
    pub shader_dir: Option<std::path::PathBuf>,
    /// The HDR tone-mapping curve, from `playback.tone_mapping`.
    pub tone_mapping: String,
    /// Tracker ids to queue progress against when the episode completes.
    pub tracker_ids: Vec<String>,
    pub presence: anistream_core::config::PresenceConfig,
    /// Where the watch party is, when there is one: a playback takes a seat at it, at the start
    /// or the moment it opens, and plays with the room until it ends. See [`crate::playback::seat`].
    pub party: PartyHub,
    /// How many seconds a hosted player's Up Next counts down before it plays the next
    /// episode, `0` for none: [`UP_NEXT_COUNTDOWN`] until the config keeps one. The terminal's
    /// companion keeps its own ten.
    pub up_next_countdown: u32,
    /// The viewer's own mpv options, as property and value, for a hosted player to set as it
    /// opens: `playback.mpv_args`, which mpv's own process takes on its command line.
    pub mpv_options: Vec<(String, String)>,
}

/// How many seconds Up Next counts down, the design's ten: what every playback says until the
/// config has a setting for it.
pub const UP_NEXT_COUNTDOWN: u32 = 10;

/// How many episodes in a row play with no input from the viewer before a hosted player asks
/// "Still watching?". The app counts; the engine says how far.
pub const STILL_WATCHING_AFTER: u32 = 3;

/// What a playback runs on beside its stream: the player it opens a window in, the slot a window
/// waits in between episodes, the host that opens a link the player cannot play, and what says
/// whether the machine is on battery.
#[derive(Clone)]
pub struct Machine {
    pub opener: Arc<dyn PlayerOpener>,
    pub slot: PlayerSlot,
    pub host: Arc<dyn Host>,
    pub power: Arc<dyn PowerSource>,
}

#[cfg(feature = "desktop")]
impl Machine {
    /// A playback on the desktop, as the command line and the probes run one: in `mpv`'s
    /// process, with `slot` for its window, links opened by `host`, and the system's word on
    /// the battery.
    pub fn desktop(mpv: anistream_player::Mpv, slot: PlayerSlot, host: Arc<dyn Host>) -> Self {
        Self { opener: Arc::new(mpv), slot, host, power: Arc::new(super::power::SystemPower) }
    }
}

/// How one file's playback came to an end.
enum FileOutcome {
    /// mpv reached the end, the viewer closed it, or the stream failed.
    Ended {
        /// The tracker's verdict, when it reached one.
        watched: Option<bool>,
        /// mpv played to the end of the file.
        complete: bool,
        /// The window is gone: the viewer closed it, or mpv died.
        exited: bool,
        /// The viewer asked for this from the terminal.
        stopped: bool,
        /// mpv quit, rather than failing or dying: the viewer closed the window or pressed q,
        /// or `stopped` asked it to.
        quit: bool,
        /// Nothing was ever shown.
        no_frames: bool,
    },
    /// The viewer picked another stream from the menu, or accepted the stall prompt's offer.
    Switch { to: usize, at: f64 },
    /// A newer playback is taking over this window, for this.
    Handover(Successor),
}

/// Play the best stream of a resolved list and record what happens.
///
/// `streams` is ordered best-first. A stream that dies without ever producing a frame is not
/// the end: the next one is tried, in the same window, because a provider can only rank what it
/// cannot verify — whether segments actually decode is knowable only here, in front of a player.
/// A stream that *played* and then ended never falls over to another, which would replay what
/// was watched.
///
/// The window is borrowed rather than owned. It comes from the machine's slot when the previous
/// episode left one open, and goes back there when this episode ends with a successor expected
/// — or when a newer playback takes over mid-episode — so a binge is one window from start to
/// finish. A new one is opened by the machine's opener.
///
/// Returns once playback has ended. Runs as its own task so the UI keeps rendering.
#[allow(clippy::too_many_arguments)]
pub async fn play(
    mut streams: Vec<Stream>,
    context: PlaybackContext,
    store: Store,
    http: HttpClient,
    machine: Machine,
    settings: PlaySettings,
    tx: Outbox,
    mut commands: mpsc::UnboundedReceiver<PlayerCommand>,
) {
    let Machine { opener, slot, host, power } = machine;
    let Some(first) = streams.first() else {
        slot.release();
        let _ = tx.send(Event::playback_ended(false, Ending::Failed));
        return;
    };

    // A licensed stream cannot be decoded locally, so it is handed to whatever is licensed to
    // play it, through the host: the system's handler for the link. No progress is tracked,
    // because we never see any.
    if first.kind == StreamKind::ExternalDeepLink {
        slot.release();
        tracing::info!(url = %first.url, "handing off to the system handler");
        let update = match host.open_url(&first.url) {
            Ok(()) => Event::Toast(Notice::info("opened in your browser")),
            Err(e) => Event::Toast(Notice::alert(format!("could not open: {e}"))),
        };
        let _ = tx.send(update);
        // Nothing plays here, so Now Playing comes down rather than waiting on a player that
        // was never going to report.
        let _ = tx.send(Event::playback_ended(false, Ending::Elsewhere));
        return;
    }

    let skips = SkipTimes::shared()
        .for_playback(&http, &settings, context.mal_id, &context.episode, context.runtime_secs)
        .await;

    let mut run = Run {
        context: &context,
        settings: &settings,
        store: &store,
        tx: &tx,
        skips,
        // Cleared when the UI drops its sender, which disables that select branch — otherwise
        // a closed channel returns `None` instantly and spins the loop. Outside the attempt
        // loop, because a channel that closed during one stream is just as closed for the next.
        controls_open: true,
        cancel_next: false,
        night_applied: false,
        night: settings.night_mode,
        prefetch_sent: false,
        look: settings.look,
        tone_mapping: settings.tone_mapping.clone(),
        hdr: None,
        episodes: context.episodes.clone(),
        next: context.next.clone(),
        http: http.clone(),
        scratch: opener.scratch_dir().to_path_buf(),
        power,
        still: None,
        // Fetched alongside everything else rather than before it: the card is minutes away
        // and the episode should not wait on a picture.
        still_fetch: context.next.as_ref().and_then(|n| n.thumbnail.clone()).map(|url| {
            tokio::spawn(fetch_still(http.clone(), url, opener.scratch_dir().to_path_buf()))
        }),
        servers: Servers::load(&store, &streams).await,
        seat: None,
        in_party: false,
    };

    // Read once: every stream this playback tries is the same series in the same translation.
    let memory = if settings.remember_tracks {
        let (store, id, translation) = (store.clone(), context.anilist_id, context.translation);
        tokio::task::spawn_blocking(move || store.track_memory(id, translation))
            .await
            .ok()
            .and_then(|read| {
                read.inspect_err(|e| tracing::debug!(error = %e, "no track memory"))
                    .ok()
                    .flatten()
            })
    } else {
        None
    };

    let total = streams.len();
    // Named in the log so "did fallback have anything to work with" is answerable after the
    // fact — a pool of one behaves exactly like the old single-stream path.
    tracing::info!(streams = total, "starting playback");
    let mut tried = vec![false; total];
    let mut localised = vec![false; total];
    let mut index = 0;
    let mut attempt = 0;
    let mut resume_at = context.resume_at;
    let mut player: Option<Player> = None;

    loop {
        tried[index] = true;
        let last = tried.iter().all(|t| *t);

        // Before mpv opens the file, so the tracks are on disk by the time it asks for them,
        // but no longer than a moment: the tracks still on their way follow as they arrive.
        let mut late = None;
        if !localised[index] {
            let scratch = opener.scratch_dir();
            late = localise_subtitles(&http, scratch, &mut streams[index], SUBTITLE_WAIT).await;
            localised[index] = true;
        }
        let mut stream = streams[index].clone();

        // HLS goes through the mender: segments arrive dressed as other things often enough
        // that checking is worth a loopback hop, and a healthy stream passes through
        // byte-for-byte. Held for the whole attempt — dropping it would kill the proxy mid
        // playback — and replaced on the next, so a stream that failed leaves nothing behind.
        let mut menders = crate::playback::mend::Menders::default();
        if stream.kind == StreamKind::Hls {
            let original = stream.url.clone();
            stream.url = menders
                .route(&http, &original, &stream.headers, &mender_token(&context, attempt))
                .await;
            // The seek bar's thumbnailer reads the same stream through its own entry, which
            // answers from what playback already fetched and never waits out a rate limit.
            // Unset when the mender could not start, and thumbfast then opens the stream as
            // the player does.
            stream.thumbnail_url = menders.thumbnail_url(&original);
        }
        attempt += 1;
        tracing::info!(
            attempt,
            of = total,
            provider = %stream.provider_id,
            kind = ?stream.kind,
            quality = ?stream.quality,
            from = %crate::logged::address(&streams[index].url),
            mended = stream.kind == StreamKind::Hls,
            headers = stream.headers.len(),
            "trying a stream"
        );

        let request = PlaybackRequest {
            title: context.window_title(),
            start_at: resume_at,
            subtitle_language: settings.subtitle_language.clone(),
            speed: context.speed,
            volume: context.volume,
            dub: context.translation == Translation::Dub,
            fullscreen: settings.fullscreen,
            mpv_options: settings.mpv_options.clone(),
        };

        // Named per attempt — during failover the header would otherwise credit the stream
        // that just failed.
        let _ = tx.send(Event::ActiveProvider(stream.provider_id.clone()));

        // A window: the one this playback already has, the one the last episode left open,
        // or a new one.
        if player.as_ref().is_none_or(|p| !p.link.is_alive()) {
            player = slot.take();
            run.night_applied = false;
        }
        if player.is_none() {
            let _ = tx.send(Event::Status("starting mpv…".into()));
            match opener.open(&request).await {
                Ok((link, events)) => player = Some(Player::new(link, events)),
                Err(e) if last => {
                    let _ = tx.send(Event::Toast(Notice::alert(format!("mpv: {e}"))));
                    // The player never existed, but the UI has Now Playing up and a control
                    // surface pointed at it. Saying so is what takes them down.
                    let _ = tx.send(Event::playback_ended(false, Ending::Failed));
                    let _ = tx.send(Event::Status(String::new()));
                    return;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "mpv would not start; trying the next stream");
                    index = next_untried(&tried, index);
                    continue;
                }
            }
        }
        let Some(current) = player.as_mut() else { continue };
        // This playback drives the window from here, the one it opened or the one it took.
        current.link.attach(&tx);

        // The track policy belongs to the process; point it at this series before the file
        // loads, and settle what it still owed the previous one.
        let preferences = TrackPreferences {
            translation: context.translation,
            language: settings.subtitle_language.clone(),
        };
        let owed = current.tracks.prepare(preferences, memory.clone());
        let key = (context.anilist_id, context.translation);
        if let Some(previous) = current.tracks_for.replace(key) {
            for action in owed {
                if let TrackAction::Remember(memory) = action {
                    remember_tracks(&store, settings.remember_tracks, previous, memory);
                }
            }
        }

        run.publish(current.link.as_ref(), &streams, index).await;
        if let Err(e) = load(current.link.as_ref(), &stream, &request).await {
            tracing::warn!(error = %e, "could not hand the stream to mpv");
        }
        let _ =
            tx.send(Event::Status(format!("playing {} ep {}", context.title, context.episode)));
        // mpv is already at the resume point by now — `start` handled it. Saying so is what
        // keeps it from looking like the episode began in the wrong place.
        if let Some(position) = resume_at {
            let _ = tx.send(Event::Resumed { position });
            let _ = current.link.overlay(Overlay::Resumed { position }).await;
        }

        let outcome = run.file(current, &stream, &streams, index, &mut commands, late).await;

        match outcome {
            FileOutcome::Switch { to, at } => {
                tracing::info!(from = index, to, at, "switching stream in place");
                index = to;
                resume_at = Some(at);
                // A switch the viewer asked for is a fresh start for failover: the stream they
                // left may be the one to come back to.
                tried = vec![false; total];
            }
            FileOutcome::Handover(successor) => {
                // The playback taking the window takes the seat too.
                if let Some(seat) = run.seat.take() {
                    seat.report(SeatReport::Ended { leave: false });
                }
                if let Some(player) = player.take() {
                    let card = companion::Status::loading(context.handing_to(&successor), None);
                    // Art the record of the titles did not have, this playback's when it is the
                    // same title's.
                    let ours = |art: &Option<String>| {
                        (successor.id == context.anilist_id).then(|| art.clone()).flatten()
                    };
                    let starting = Overlay::Starting {
                        card,
                        id: successor.id,
                        title: successor.title.clone(),
                        episode: successor.episode.clone(),
                        episode_title: None,
                        cover_url: successor
                            .cover_url
                            .clone()
                            .or_else(|| ours(&context.cover_url)),
                        banner_url: successor
                            .banner_url
                            .clone()
                            .or_else(|| ours(&context.banner_url)),
                    };
                    let _ = player.link.overlay(starting).await;
                    // Silence what was playing while the next stream is found.
                    let _ = player.link.stop().await;
                    slot.park(player);
                }
                return;
            }
            FileOutcome::Ended { watched, complete, exited, stopped, quit, no_frames } => {
                let reported = match (&player, no_frames) {
                    (Some(p), true) => p.link.diagnostics().await,
                    _ => None,
                };
                // What the attempt says of its server, written down before anything else is
                // decided, so the next play knows which server to try first.
                if no_frames {
                    let refused = menders.refused(&streams[index].url);
                    run.servers
                        .unplayed(&store, &streams[index], stopped || quit, refused)
                        .await;
                }

                if no_frames && !stopped && !exited && !last {
                    // The point of carrying alternates: whether segments actually decode is
                    // knowable only in front of a player, so a stream that never showed a frame
                    // fails over — in the same window — unless the viewer stopped it or closed
                    // the window themselves, which is an instruction, not a failure.
                    tracing::warn!(
                        from = %crate::logged::address(&streams[index].url),
                        ?reported,
                        "no frames; trying the next stream"
                    );
                    let _ = tx.send(Event::Toast(Notice::info(format!(
                        "no video from that stream — trying {} of {total}",
                        attempt + 1
                    ))));
                    index = next_untried(&tried, index);
                    continue;
                }

                // Keep the window for the next episode when one is coming: auto-next is on, the
                // episode table knows what is next, and the viewer did not say no.
                // Not in a party: the room decides what it watches next, and each member's
                // next episode would be found and started on their own.
                let rolls_on = complete
                    && settings.auto_next
                    && !run.cancel_next
                    && !exited
                    && !run.in_party
                    && run.next.is_some();
                // Stopping the player or closing its window leaves the party; an episode that
                // ended, or failed, stays in it, for the next to play with the room.
                if let Some(seat) = run.seat.take() {
                    seat.report(SeatReport::Ended { leave: stopped || quit });
                }
                let next = run.next.as_ref().filter(|_| rolls_on);
                if let Some(player) = player.take() {
                    if let Some(next) = next {
                        let card = companion::Status::loading(
                            format!("Up next · {}", next.label()),
                            Some("Finding sources…".into()),
                        );
                        let starting = Overlay::Starting {
                            card,
                            id: context.anilist_id,
                            title: context.title.clone(),
                            episode: next.number.clone(),
                            episode_title: next.title.clone(),
                            cover_url: context.cover_url.clone(),
                            banner_url: context.banner_url.clone(),
                        };
                        let _ = player.link.overlay(starting).await;
                        slot.park(player);
                    } else {
                        player.link.shutdown().await;
                    }
                }

                // Not for a player the viewer closed or stopped: quitting before the first frame
                // is a choice, and "mpv exited without playing anything" would blame mpv for it.
                // Both are checked, because a quit that reaches mpv before the file has started
                // ends no file, and only the socket closing says anything happened.
                if no_frames && !stopped && !quit {
                    let message = match reported {
                        Some(line) => format!("mpv could not play this: {line}"),
                        None => {
                            "mpv exited without playing anything, and said nothing about why"
                                .into()
                        }
                    };
                    tracing::warn!(
                        %message,
                        from = %crate::logged::address(&streams[index].url),
                        "playback produced no frames"
                    );
                    let _ = tx.send(Event::Toast(Notice::alert(message)));
                }

                // Exactly one ending per playback, sent only now that this attempt is known to
                // be the last: the tracker's verdict when it reached one, and otherwise the
                // honest `false` for a session that never played. The ending names the next
                // episode only when the window was kept for it, which is what auto-next follows.
                let ending = if complete {
                    Ending::Complete { next: next.map(|n| n.number.clone()) }
                } else if stopped {
                    Ending::Stopped
                } else if exited {
                    Ending::Closed
                } else {
                    Ending::Failed
                };
                let _ = tx.send(Event::playback_ended(watched.unwrap_or(false), ending));
                let _ = tx.send(Event::Status(String::new()));
                return;
            }
        }
    }
}

/// Open `stream` in the player, in place of whatever was playing.
///
/// Everything that belongs to the file travels with it, so the previous file's never leak into
/// this one: headers, resume point, title and track languages as its options, and before them
/// where the seek bar's thumbnails are read, so the thumbnailer sees the right address when it
/// starts on this file. A torrent gives a second address that reads only what is already
/// downloaded, so thumbnails never pull bandwidth away from the playhead. External subtitles are
/// attached once the file has loaded.
async fn load(
    link: &dyn PlayerLink,
    stream: &Stream,
    request: &PlaybackRequest,
) -> Result<(), LinkError> {
    link.overlay(Overlay::ThumbnailUrl(stream.thumbnail_url.clone())).await?;
    let options = anistream_playback::protocol::file_options(
        &stream.headers,
        request.start_at,
        &request.title,
        request.subtitle_language.as_deref(),
        request.dub,
    );
    link.load(&stream.url, options).await
}

/// The next stream not yet tried, wrapping around; the current one when all have been.
fn next_untried(tried: &[bool], current: usize) -> usize {
    (1..=tried.len())
        .map(|step| (current + step) % tried.len())
        .find(|&i| !tried[i])
        .unwrap_or(current)
}

/// What a party is answered with where this build has no watch parties: see the `party`
/// feature.
pub const NO_PARTY: &str = "watch parties aren't built into this front end";

/// One playback's state across the streams it tries.
struct Run<'a> {
    context: &'a PlaybackContext,
    settings: &'a PlaySettings,
    store: &'a Store,
    tx: &'a Outbox,
    skips: Vec<SkipInterval>,
    controls_open: bool,
    /// The viewer cancelled Up Next, so the end of this episode is the end.
    cancel_next: bool,
    /// Night mode as the viewer last set it, and whether this window has it applied.
    night: bool,
    night_applied: bool,
    /// Whether the next episode has been asked to be found ahead.
    prefetch_sent: bool,
    /// The upscaling look and HDR curve as the viewer last chose them in the player.
    look: Look,
    tone_mapping: String,
    /// What the current file is, for the tone-mapping menu.
    hdr: Option<HdrKind>,
    /// The next episode's still once written out, and the fetch while it is still under way.
    still: Option<companion::Still>,
    still_fetch: Option<tokio::task::JoinHandle<Option<companion::Still>>>,
    /// The episode list and what follows, from the context or learned once playback started.
    episodes: Vec<EpisodeEntry>,
    next: Option<NextEpisode>,
    /// For fetching a still learned late, and where to write it.
    http: HttpClient,
    scratch: std::path::PathBuf,
    /// What says whether the machine is on battery, which the upscaling chain goes by.
    power: Arc<dyn PowerSource>,
    /// How the servers of the streams tried have fared, and the attempt under way.
    servers: Servers,
    /// The seat at the watch party, between files; the file playing holds it while it plays.
    seat: Option<Seat>,
    /// Whether this playback plays with a party now: what it writes down and whether it rolls
    /// on go by it.
    in_party: bool,
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;

    use super::*;

    fn context_for(episode: &str) -> PlaybackContext {
        PlaybackContext {
            anilist_id: AnilistId::new(1),
            mal_id: None,
            episode: episode.into(),
            title: "A Show".into(),
            translation: Translation::Sub,
            resume_at: None,
            runtime_secs: None,
            speed: None,
            volume: None,
            next: None,
            episodes: Vec::new(),
            cover_url: None,
            banner_url: None,
            episode_total: None,
        }
    }

    #[test]
    fn a_window_changing_hands_says_what_is_coming() {
        let now = context_for("6");
        let to = |id: u32, title: &str, episode: &str, translation| Successor {
            id: AnilistId::new(id),
            title: title.into(),
            episode: episode.into(),
            translation,
            cover_url: None,
            banner_url: None,
        };
        let sub = Translation::Sub;
        assert_eq!(now.handing_to(&to(1, "A Show", "7", sub)), "Opening the next episode…");
        assert_eq!(now.handing_to(&to(1, "A Show", "5", sub)), "Opening the previous episode…");
        assert_eq!(now.handing_to(&to(1, "A Show", "11", sub)), "Opening Episode 11…");
        assert_eq!(
            now.handing_to(&to(1, "A Show", "6", Translation::Dub)),
            "Switching to the dub…"
        );
        assert_eq!(now.handing_to(&to(1, "A Show", "6", sub)), "Opening Episode 6 again…");
        assert_eq!(
            now.handing_to(&to(2, "Another Show", "1", sub)),
            "Opening Another Show · Episode 1…"
        );
    }

    #[test]
    fn a_playback_context_carries_what_history_needs() {
        // Provider and translation are the fields no tracker can supply, and the reason local
        // history is richer than anything that can be synced.
        let context = PlaybackContext {
            anilist_id: AnilistId::new(154_587),
            mal_id: Some(52_991),
            episode: "12".into(),
            title: "Hoshikuzu".into(),
            translation: Translation::Sub,
            resume_at: Some(600.0),
            runtime_secs: Some(1_440),
            speed: Some(1.25),
            volume: Some(80.0),
            next: None,
            episodes: Vec::new(),
            cover_url: None,
            banner_url: None,
            episode_total: None,
        };
        assert_eq!(context.mal_id, Some(52_991));
        assert_eq!(context.resume_at, Some(600.0));
        assert_eq!(context.runtime_secs, Some(1_440));
    }
}
