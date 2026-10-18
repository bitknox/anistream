//! Wiring playback to history.
//!
//! The policy lives in [`anistream_player::PlaybackTracker`], which is pure and tested. This is
//! the part that talks to the world: it spawns mpv, feeds its events to the tracker, and turns
//! the resulting actions into rows in the local database.
//!
//! History is written **before** anything is pushed anywhere. Local is the source of truth, so
//! if a tracker push fails the watch is still recorded and the outbox can retry.

use std::time::Duration;

use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
    traits::PlaybackRequest,
};
use anistream_net::HttpClient;
use anistream_player::{
    Action, Choice, ExternalPlayer, Mpv, MpvSession, PlaybackEvent, PlaybackTracker,
    ScriptEvent, SkipInterval, SkipKind, companion, skip,
};
use anistream_store::{Store, WatchEvent};
use anistream_ui::app::{PlayerCommand, Toast, Update};
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::slot::{Player, PlayerSlot};
/// How long mpv may go without reporting a position before we say something.
///
/// Generous, because a torrent legitimately takes a while to find peers and fill a buffer — the
/// measured happy path is half a second, so twenty is far past "slow" and firmly in "wrong".
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(20);

/// Everything one playback needs to know about what it is playing.
#[derive(Debug, Clone)]
pub struct PlaybackContext {
    pub anilist_id: AnilistId,
    pub mal_id: Option<u32>,
    pub episode: String,
    pub title: String,
    pub translation: Translation,
    /// Where to resume from, if the local history says so.
    pub resume_at: Option<f64>,
    /// The episode's advertised runtime in seconds, from AniList's per-episode minutes.
    ///
    /// A hint, not a truth: it exists for the aniskip fallback, which needs a plausible
    /// length when the length-agnostic query hits that API's end-of-episode 500.
    pub runtime_secs: Option<u32>,
    /// Speed carried over from the previous episode.
    pub speed: Option<f64>,
    /// Volume carried over from the previous session.
    pub volume: Option<f64>,
    /// What plays after this, when the episode table knows. The Up Next card and the decision
    /// to keep the window open for it both hang off this; `None` means this is the end.
    pub next: Option<NextEpisode>,
    /// The episode table, for the player's Episodes menu. Empty when it was not loaded.
    pub episodes: Vec<EpisodeEntry>,
}

/// The episode that follows, as the Up Next card names it.
#[derive(Debug, Clone, PartialEq)]
pub struct NextEpisode {
    pub number: String,
    pub title: Option<String>,
}

impl NextEpisode {
    pub fn label(&self) -> String {
        format!("Episode {}", self.number)
    }
}

/// One row of the player's Episodes menu.
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeEntry {
    pub number: String,
    pub title: Option<String>,
    pub watched: bool,
    pub filler: bool,
}

impl PlaybackContext {
    pub fn episode_label(&self) -> String {
        format!("Episode {}", self.episode)
    }

    /// The title the window and the player's top bar show.
    pub fn window_title(&self) -> String {
        format!("{} · {}", self.title, self.episode_label())
    }

    fn episode_info(&self) -> companion::EpisodeInfo {
        companion::EpisodeInfo {
            number: self.episode.clone(),
            label: self.episode_label(),
            title: self
                .episodes
                .iter()
                .find(|e| e.number == self.episode)
                .and_then(|e| e.title.clone()),
            show: self.title.clone(),
            translation: match self.translation {
                Translation::Dub => "dub",
                _ => "sub",
            },
        }
    }
}

/// Download a stream's external subtitles and point it at the local copies.
///
/// **A stream's headers are its subtitles' headers.** A provider returns whatever a track needs
/// alongside the stream it belongs to, and getting that right is the provider's job, not this
/// one's. So nothing is guessed here: each track is requested with exactly what the stream
/// declared, through the emulated client, so a subtitle behind the same protection as its video
/// is fetched the same way the video would be.
///
/// **What this buys over handing the URL to mpv** is that a track which cannot actually be
/// retrieved is never offered. A provider may list a track that its chosen stream's headers do
/// not open — listings are often scoped to the episode rather than to the individual stream — and
/// mpv would accept that URL, fail quietly, and leave an empty track for the user to cycle onto.
/// Fetching first turns that into a track that is simply absent.
///
/// Files land beside the IPC socket, in a directory the player already owns.
async fn localise_subtitles(http: &HttpClient, mpv: &Mpv, stream: &mut Stream) {
    let soft = stream.subtitles.iter().filter(|s| !s.hard).count();
    if soft == 0 {
        return;
    }

    let dir = mpv.scratch_dir().join("subs");
    if tokio::fs::create_dir_all(&dir).await.is_err() {
        return;
    }

    // Exactly what the stream declared, because that is what the subtitle host wants too.
    let headers = stream.headers.clone();
    let (headers, dir) = (&headers, &dir);

    // Several tracks at once: playback waits on this, and a stream listing a dozen languages
    // was a dozen round trips back to back before mpv could open. `buffered` keeps the tracks
    // in their listed order, which is the order mpv offers them in, and the bound keeps a long
    // list from opening a connection per language against the subtitle host.
    let tracks = std::mem::take(&mut stream.subtitles).into_iter().enumerate();
    let localised = futures::stream::iter(tracks).map(|(index, mut subtitle)| async move {
        // Already burned in, or already a local path — nothing to fetch either way.
        if subtitle.hard || !subtitle.url.starts_with("http") {
            return Some(subtitle);
        }

        let mut request = http.emulated().get(&subtitle.url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }

        let fetched = match request.send().await {
            Ok(response) if response.status().is_success() => response.bytes().await.ok(),
            Ok(response) => {
                tracing::debug!(url = %subtitle.url, status = %response.status(), "subtitle refused");
                None
            }
            Err(error) => {
                tracing::debug!(url = %subtitle.url, %error, "subtitle fetch failed");
                None
            }
        };

        // Dropped rather than passed along. mpv would request it with the same headers and get
        // the same refusal, so keeping it only buys the user an empty track to cycle onto.
        let Some(body) = fetched.filter(|b| !b.is_empty()) else {
            tracing::debug!(language = %subtitle.language, "subtitle dropped: could not be fetched");
            return None;
        };

        // The extension is what tells mpv which parser to use — `.vtt` and `.ass` are both
        // common and are not interchangeable. The source's own claim wins, because an
        // API-shaped URL (`/subtitles?id=…`) carries no extension to read; the URL is the
        // fallback, and `vtt` the guess of last resort.
        let plausible = |ext: &String| {
            !ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric())
        };
        let extension = subtitle
            .format
            .as_deref()
            .map(|format| format.trim_start_matches('.').to_ascii_lowercase())
            .filter(plausible)
            .or_else(|| {
                subtitle
                    .url
                    .rsplit('/')
                    .next()
                    .and_then(|name| name.rsplit_once('.'))
                    .map(|(_, ext)| ext.to_ascii_lowercase())
                    .filter(plausible)
            })
            .unwrap_or_else(|| "vtt".into());
        // Indexed rather than hashed: two tracks in the same language would otherwise collide
        // and the second would silently replace the first.
        let path = dir.join(format!("{index}-{}.{extension}", sanitise(&subtitle.language)));

        match tokio::fs::write(&path, &body).await {
            Ok(()) => {
                tracing::debug!(language = %subtitle.language, path = %path.display(), "subtitle cached");
                subtitle.url = path.to_string_lossy().into_owned();
            }
            // The fetch worked and only the write failed, so the remote URL is still good.
            Err(error) => tracing::debug!(%error, "could not cache subtitle; using its url"),
        }
        Some(subtitle)
    });
    stream.subtitles =
        localised.buffered(SUBTITLE_FETCHES).filter_map(std::future::ready).collect().await;
}

/// Subtitle tracks fetched at once.
const SUBTITLE_FETCHES: usize = 4;

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

/// Reduce a language label to something safe to put in a filename.
fn sanitise(language: &str) -> String {
    let cleaned: String = language
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();
    if cleaned.is_empty() { "sub".into() } else { cleaned }
}

/// Fetch skip intervals for an episode.
///
/// Keyed on MAL id, so a title the mapping layer could not resolve simply has no skip data —
/// the prompt is absent rather than broken.
pub async fn fetch_skips(
    http: &HttpClient,
    mal_id: Option<u32>,
    episode: &str,
    runtime_secs: Option<u32>,
) -> Vec<SkipInterval> {
    let Some(mal_id) = mal_id else {
        tracing::debug!("no mal id; skip data unavailable for this title");
        return Vec::new();
    };
    let Ok(number) = episode.trim().parse::<u32>() else {
        return Vec::new();
    };

    // Length 0 first, because it returns every submission regardless of which encode it
    // was timed against. It has one measured failure: aniskip answers 500 when a stored
    // interval runs to the exact end of the episode — an ending that fades on the last
    // frame, common for currently-airing shows. The retry asks again with the advertised
    // runtime, which that server handles; its tight length-tolerance is the price, and it
    // costs nothing here since the alternative was no data at all.
    let attempts = [Some(0), runtime_secs];
    for length in attempts.into_iter().flatten() {
        match http.plain().get(skip::query_url(mal_id, number, length)).send().await {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();
                let intervals = skip::parse(&body);
                tracing::info!(count = intervals.len(), length, "skip intervals loaded");
                return intervals;
            }
            // aniskip returns 404 for titles nobody has submitted times for, which is
            // the norm — only a server error is worth the second ask.
            Ok(response) if !response.status().is_server_error() => return Vec::new(),
            _ => {}
        }
    }
    Vec::new()
}

/// The settings one playback runs under, read from the config when it starts.
#[derive(Debug, Clone)]
pub struct PlaySettings {
    pub threshold: f64,
    pub auto_skip: bool,
    /// Put the opening and ending on the seek bar where the stream has no chapters of its own,
    /// from `playback.mark_chapters`.
    pub mark_chapters: bool,
    /// Open mpv fullscreen, from `playback.fullscreen`.
    pub fullscreen: bool,
    pub subtitle_language: Option<String>,
    /// Whether a finished episode rolls on to the next, from `playback.auto_next`.
    pub auto_next: bool,
    pub night_mode: bool,
    pub upscaling: anistream_core::config::Upscaling,
    /// Tracker ids to queue progress against when the episode completes.
    pub tracker_ids: Vec<String>,
    pub presence: anistream_core::config::PresenceConfig,
    pub syncplay: anistream_core::config::SyncplayConfig,
    /// Whether this play joins the party — from the `y` key, or `[syncplay] enabled`
    /// sending every play there.
    pub party: bool,
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
        /// Nothing was ever shown.
        no_frames: bool,
    },
    /// The viewer picked another stream from the menu, or accepted the stall prompt's offer.
    Switch { to: usize, at: f64 },
    /// A newer playback is taking over this window.
    Handover,
}

/// The audio filter night mode adds, labelled so removing it leaves the viewer's own alone.
///
/// `dynaudnorm` lifts quiet passages and holds loud ones down over a short window, which is
/// what keeps dialogue audible at a low volume without the opening arriving at full blast.
const NIGHT_FILTER: &str = "@anistream-night:lavfi=[dynaudnorm=f=150:g=15:p=0.6:m=8]";

/// How long a mid-episode stall lasts before the player offers another stream.
const STALL_PROMPT: Duration = Duration::from_secs(8);

/// Play the best stream of a resolved list and record what happens.
///
/// `streams` is ordered best-first. A stream that dies without ever producing a frame is not
/// the end: the next one is tried, in the same window, because a provider can only rank what it
/// cannot verify — whether segments actually decode is knowable only here, in front of a player.
/// A stream that *played* and then ended never falls over to another, which would replay what
/// was watched.
///
/// The window is borrowed rather than owned. It comes from `slot` when the previous episode left
/// one open, and goes back there when this episode ends with a successor expected — or when a
/// newer playback takes over mid-episode — so a binge is one window from start to finish.
///
/// Returns once playback has ended. Runs as its own task so the UI keeps rendering.
#[allow(clippy::too_many_arguments)]
pub async fn play(
    mut streams: Vec<Stream>,
    context: PlaybackContext,
    store: Store,
    http: HttpClient,
    mpv: Mpv,
    settings: PlaySettings,
    slot: PlayerSlot,
    tx: mpsc::UnboundedSender<Update>,
    mut commands: mpsc::UnboundedReceiver<PlayerCommand>,
) {
    let Some(first) = streams.first() else {
        slot.release();
        let _ = tx.send(Update::PlaybackEnded { watched: false });
        return;
    };

    // A licensed stream cannot be decoded locally, so it is handed to whatever is licensed to
    // play it. No progress is tracked, because we never see any.
    if first.kind == StreamKind::ExternalDeepLink {
        use anistream_core::traits::Player as _;
        slot.release();
        let player = ExternalPlayer::new();
        let request = PlaybackRequest { title: context.title.clone(), ..Default::default() };
        let update = match player.play(first, request).await {
            Ok(()) => Update::Toast(Toast::info("opened in your browser")),
            Err(e) => Update::Toast(Toast::alert(format!("could not open: {e}"))),
        };
        let _ = tx.send(update);
        return;
    }

    // A watch party is a shared session: Syncplay owns the player and the pacing, so no
    // private history is recorded — the room pausing must not read as you abandoning the
    // episode. The torrent session stays alive underneath, serving the loopback URL.
    if settings.party {
        slot.release();
        hand_to_syncplay(&settings.syncplay, &first.url, &tx);
        let _ = tx.send(Update::PlaybackEnded { watched: false });
        return;
    }

    let skips =
        fetch_skips(&http, context.mal_id, &context.episode, context.runtime_secs).await;

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

        // Before mpv opens the file, so the tracks are on disk by the time it asks for them.
        if !localised[index] {
            localise_subtitles(&http, &mpv, &mut streams[index]).await;
            localised[index] = true;
        }
        let mut stream = streams[index].clone();

        // HLS goes through the mender: segments arrive dressed as other things often enough
        // that checking is worth a loopback hop, and a healthy stream passes through
        // byte-for-byte. Held for the whole attempt — dropping it would kill the proxy mid
        // playback — and replaced on the next, so a stream that failed leaves nothing behind.
        let mut menders = crate::mend::Menders::default();
        if stream.kind == StreamKind::Hls {
            stream.url = menders
                .route(&http, &stream.url, &stream.headers, &mender_token(&context, attempt))
                .await;
        }
        attempt += 1;

        let request = PlaybackRequest {
            title: context.window_title(),
            start_at: resume_at,
            subtitle_language: settings.subtitle_language.clone(),
            speed: context.speed,
            volume: context.volume,
            dub: context.translation == Translation::Dub,
            fullscreen: settings.fullscreen,
        };

        // Named per attempt — during failover the header would otherwise credit the stream
        // that just failed.
        let _ = tx.send(Update::ActiveProvider(stream.provider_id.clone()));

        // A window: the one this playback already has, the one the last episode left open,
        // or a new one.
        if player.as_ref().is_none_or(|p| !p.session.is_alive()) {
            player = slot.take();
            run.night_applied = false;
        }
        if player.is_none() {
            let _ = tx.send(Update::Status("starting mpv…".into()));
            match mpv.spawn(&request).await {
                Ok((session, events)) => player = Some(Player { session, events }),
                Err(e) if last => {
                    let _ = tx.send(Update::Toast(Toast::alert(format!("mpv: {e}"))));
                    // The player never existed, but the UI has Now Playing up and a control
                    // surface pointed at it. Saying so is what takes them down.
                    let _ = tx.send(Update::PlaybackEnded { watched: false });
                    let _ = tx.send(Update::Status(String::new()));
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

        run.publish(&current.session, &streams, index).await;
        if let Err(e) = current.session.load(&stream, &request).await {
            tracing::warn!(error = %e, "could not hand the stream to mpv");
        }
        let _ = tx
            .send(Update::Status(format!("playing {} ep {}", context.title, context.episode)));
        // mpv is already at the resume point by now — `start` handled it. Saying so is what
        // keeps it from looking like the episode began in the wrong place.
        if let Some(position) = resume_at {
            let _ = tx.send(Update::Resumed { position });
        }

        let outcome = run.file(current, &stream, &streams, index, &mut commands).await;

        match outcome {
            FileOutcome::Switch { to, at } => {
                tracing::info!(from = index, to, at, "switching stream in place");
                index = to;
                resume_at = Some(at);
                // A switch the viewer asked for is a fresh start for failover: the stream they
                // left may be the one to come back to.
                tried = vec![false; total];
            }
            FileOutcome::Handover => {
                if let Some(player) = player.take() {
                    let _ = player
                        .session
                        .send(companion::state(
                            "status",
                            &companion::Status::loading("Opening the next episode…", None),
                        ))
                        .await;
                    // Silence what was playing while the next stream is found.
                    let _ = player.session.stop().await;
                    slot.park(player);
                }
                return;
            }
            FileOutcome::Ended { watched, complete, exited, stopped, no_frames } => {
                let reported = match (&player, no_frames) {
                    (Some(p), true) => p.session.diagnostics().most_relevant().await,
                    _ => None,
                };

                if no_frames && !stopped && !exited && !last {
                    // The point of carrying alternates: whether segments actually decode is
                    // knowable only in front of a player, so a stream that never showed a frame
                    // fails over — in the same window — unless the viewer stopped it themselves,
                    // which is an instruction, not a failure.
                    tracing::warn!(url = %stream.url, ?reported, "no frames; trying the next stream");
                    let _ = tx.send(Update::Toast(Toast::info(format!(
                        "no video from that stream — trying {} of {total}",
                        attempt + 1
                    ))));
                    index = next_untried(&tried, index);
                    continue;
                }

                // Keep the window for the next episode when one is coming: auto-next is on, the
                // episode table knows what is next, and the viewer did not say no.
                let rolls_on = complete
                    && settings.auto_next
                    && !run.cancel_next
                    && !exited
                    && context.next.is_some();
                if let Some(player) = player.take() {
                    if rolls_on && let Some(next) = &context.next {
                        let _ = player
                            .session
                            .send(companion::state(
                                "status",
                                &companion::Status::loading(
                                    format!("Up next · {}", next.label()),
                                    Some("Finding sources…".into()),
                                ),
                            ))
                            .await;
                        slot.park(player);
                    } else {
                        player.session.shutdown().await;
                    }
                }

                if no_frames {
                    let message = match reported {
                        Some(line) => format!("mpv could not play this: {line}"),
                        None => {
                            "mpv exited without playing anything, and said nothing about why"
                                .into()
                        }
                    };
                    tracing::warn!(%message, url = %stream.url, "playback produced no frames");
                    let _ = tx.send(Update::Toast(Toast::alert(message)));
                }

                // Exactly one ending per playback, sent only now that this attempt is known to
                // be the last: the tracker's verdict when it reached one, and otherwise the
                // honest `false` for a session that never played.
                let _ = tx.send(Update::PlaybackEnded { watched: watched.unwrap_or(false) });
                let _ = tx.send(Update::Status(String::new()));
                return;
            }
        }
    }
}

/// The next stream not yet tried, wrapping around; the current one when all have been.
fn next_untried(tried: &[bool], current: usize) -> usize {
    (1..=tried.len())
        .map(|step| (current + step) % tried.len())
        .find(|&i| !tried[i])
        .unwrap_or(current)
}

/// Start a Syncplay party on a stream.
fn hand_to_syncplay(
    syncplay: &anistream_core::config::SyncplayConfig,
    url: &str,
    tx: &mpsc::UnboundedSender<Update>,
) {
    // Syncplay's no-gui mode refuses to start without a room, so asking first beats
    // spawning a process whose refusal we would have to fish out of its stderr.
    let Some(room) = syncplay.room.clone().filter(|r| !r.trim().is_empty()) else {
        let _ = tx.send(Update::Toast(Toast::alert(
            "syncplay needs a room — set syncplay.room in config.toml",
        )));
        return;
    };

    let mut command = std::process::Command::new(&syncplay.binary);
    command.arg("--no-gui");
    command.args(["--host", &syncplay.server]);
    command.args(["--name", &syncplay.name]);
    command.args(["--room", &room]);
    command.arg(url);
    // The TUI owns the terminal. One line of child stderr — Syncplay greets with a
    // Python deprecation warning — scribbles straight over the alternate screen, so
    // the party speaks through toasts and nothing else.
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::null());

    match command.spawn() {
        Ok(mut child) => {
            let _ = tx
                .send(Update::Toast(Toast::info(format!("handed to syncplay — room {room}"))));
            // With stdio silenced, a failed start would otherwise be a party that
            // just never happens. Watch the exit instead: quiet on success, a toast
            // naming the failure whenever it dies unhappy.
            let tx = tx.clone();
            tokio::task::spawn_blocking(move || {
                if let Ok(status) = child.wait()
                    && !status.success()
                {
                    let _ = tx.send(Update::Toast(Toast::alert(format!(
                        "syncplay exited with {status} — check server and room in config.toml"
                    ))));
                }
            });
        }
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!(
                "syncplay would not start: {e} — is it installed?"
            ))));
        }
    }
}

/// One playback's state across the streams it tries.
struct Run<'a> {
    context: &'a PlaybackContext,
    settings: &'a PlaySettings,
    store: &'a Store,
    tx: &'a mpsc::UnboundedSender<Update>,
    skips: Vec<SkipInterval>,
    controls_open: bool,
    /// The viewer cancelled Up Next, so the end of this episode is the end.
    cancel_next: bool,
    /// Night mode as the viewer last set it, and whether this window has it applied.
    night: bool,
    night_applied: bool,
}

impl Run<'_> {
    /// Tell the companion what it is showing: the episode, what follows, the menus.
    async fn publish(&mut self, session: &MpvSession, streams: &[Stream], current: usize) {
        let context = self.context;
        let ending = self.skips.iter().find(|s| s.kind == SkipKind::Ending).map(|s| s.start);
        let next = context.next.as_ref().map(|next| companion::NextUp {
            number: next.number.clone(),
            label: next.label(),
            title: next.title.clone(),
            at: ending,
            auto: self.settings.auto_next && !self.cancel_next,
        });
        let commands = [
            companion::state("episode", &context.episode_info()),
            match &next {
                Some(next) => companion::state("next", next),
                None => companion::clear("next"),
            },
            companion::state("menus", &self.menus(streams, current)),
            companion::state("media", &companion::Media::default()),
            companion::state(
                "status",
                &companion::Status::loading(
                    format!("Opening {}", context.episode_label()),
                    None,
                ),
            ),
        ];
        for command in commands {
            let _ = session.send(command).await;
        }
        self.apply_night(session).await;
    }

    fn menus(&self, streams: &[Stream], current: usize) -> companion::Menus {
        let context = self.context;
        companion::Menus {
            episodes: context
                .episodes
                .iter()
                .map(|e| companion::EpisodeItem {
                    number: e.number.clone(),
                    label: format!("Episode {}", e.number),
                    title: e.title.clone(),
                    watched: e.watched,
                    filler: e.filler,
                    current: e.number == context.episode,
                })
                .collect(),
            sources: streams
                .iter()
                .enumerate()
                .map(|(i, s)| companion::SourceItem {
                    id: i.to_string(),
                    label: s.provider_id.clone(),
                    detail: Some(stream_detail(s)),
                    current: i == current,
                })
                .collect(),
            translation: companion::TranslationMenu {
                current: match context.translation {
                    Translation::Dub => "dub",
                    _ => "sub",
                },
                options: vec!["sub", "dub"],
            },
            upscaling: upscaling_menu(self.settings.upscaling),
            tone_mapping: None,
            night_mode: self.night,
        }
    }

    async fn apply_night(&mut self, session: &MpvSession) {
        if self.night == self.night_applied {
            return;
        }
        let _ = session
            .send(anistream_player::protocol::Command::AudioFilter {
                add: self.night,
                filter: if self.night {
                    NIGHT_FILTER.to_owned()
                } else {
                    "@anistream-night".to_owned()
                },
            })
            .await;
        self.night_applied = self.night;
    }

    /// Play one file to its end, feeding the tracker and answering the viewer.
    async fn file(
        &mut self,
        player: &mut Player,
        stream: &Stream,
        streams: &[Stream],
        index: usize,
        commands: &mut mpsc::UnboundedReceiver<PlayerCommand>,
    ) -> FileOutcome {
        let context = self.context;
        let settings = self.settings;
        let tx = self.tx;
        let session = &player.session;

        let mut tracker =
            PlaybackTracker::new(settings.threshold, self.skips.clone(), settings.auto_skip)
                .marking_chapters(settings.mark_chapters);

        // Presence is connected here rather than at startup: it should exist for exactly as
        // long as something is playing, and holding a socket open while idle would claim a
        // session that is not happening. `None` covers both "turned off" and "Discord is not
        // running", which need no distinction.
        let mut rpc = Rpc::start(&settings.presence, context).await;

        // What the UI is currently showing, so a repaint is only asked for when it would differ.
        let mut shown_second = f64::NAN;
        let mut shown_duration = false;
        // Progress is queued once per episode. The tracker emits several completed rows —
        // crossing the threshold, then pausing, then ending — and the outbox would coalesce
        // duplicates anyway, but queueing once keeps the badge honest.
        let mut committed = false;
        // Whether mpv has ever reported a position. A successful load is not playback: mpv can
        // accept the URL and then sit there forever with no data — which is exactly what a
        // stalled torrent looks like, and it produced no error of any kind because nothing was
        // watching for the *absence* of events.
        let mut ever_played = false;
        // Whether the "still nothing after 20s" warning has been said, kept apart from
        // `ever_played` so the warning cannot be mistaken for playback.
        let mut warned = false;
        // An exit the viewer asked for must never fall over to another stream: pressing `x`
        // on a stream that had not started yet means "stop", not "try the next one".
        let mut stopped = false;
        // What the tracker concluded, held back until it is known whether this attempt is the
        // one the viewer keeps.
        let mut ending: Option<bool> = None;
        // The last automatic skip, so Undo knows which segment to stop skipping.
        let mut last_skip: Option<SkipKind> = None;
        // A mid-episode stall: when it began, and whether the viewer said to keep waiting.
        let mut stalled_since: Option<tokio::time::Instant> = None;
        let mut stall_shown = false;
        let mut stall_dismissed = false;
        let mut stall_tick = tokio::time::interval(Duration::from_secs(1));

        let outcome = loop {
            // Controls and events are interleaved rather than polled in turn: a keystroke must
            // not wait on the next position tick, and a position tick must not wait on a
            // keystroke.
            let event = tokio::select! {
                event = player.events.recv() => match event {
                    Some(event) => event,
                    // The channel closing means mpv exited.
                    None => break FileOutcome::Ended {
                        watched: ending,
                        complete: false,
                        exited: true,
                        stopped,
                        no_frames: !ever_played,
                    },
                },
                // Only armed until the first frame arrives, so this cannot fire mid-episode
                // during a legitimate pause — mpv keeps reporting `time-pos` while paused.
                _ = tokio::time::sleep(FIRST_FRAME_TIMEOUT), if !ever_played && !warned => {
                    let _ = tx.send(Update::Toast(Toast::alert(format!(
                        "mpv has not started playing after {}s — the source may have no data, or check `mpv` plays a file on its own",
                        FIRST_FRAME_TIMEOUT.as_secs()
                    ))));
                    // Deliberately not a stop. mpv may still be buffering a slow torrent, and
                    // killing it would turn a wait into a failure; the user now knows and can
                    // press `x`.
                    warned = true;
                    continue;
                }
                _ = stall_tick.tick(), if stalled_since.is_some() => {
                    let held = stalled_since.map(|t| t.elapsed()).unwrap_or_default();
                    if held >= STALL_PROMPT && !stall_dismissed {
                        let _ = session.send(companion::stall(held.as_secs())).await;
                        stall_shown = true;
                    }
                    continue;
                }
                command = commands.recv(), if self.controls_open => {
                    match command {
                        Some(PlayerCommand::Handover) => {
                            // Flush what was watched before the window changes hands.
                            self.record_actions(&mut tracker, &PlaybackEvent::Ended { complete: false }, stream, &mut committed, &mut ending).await;
                            let _ = tx.send(Update::EpisodeLeft {
                                id: context.anilist_id,
                                episode: context.episode.clone(),
                                position: tracker.position(),
                                duration: tracker.duration(),
                                watched: ending.unwrap_or(false),
                            });
                            break FileOutcome::Handover;
                        }
                        Some(command) => {
                            stopped |= matches!(command, PlayerCommand::Stop);
                            apply_command(session, command).await;
                            continue;
                        }
                        // The UI is going away. mpv keeps playing until it ends on its own.
                        None => {
                            self.controls_open = false;
                            continue;
                        }
                    }
                }
            };

            match &event {
                PlaybackEvent::Script(script) => {
                    if let Some(outcome) = self
                        .answer(script, session, &mut tracker, &mut last_skip, streams, index)
                        .await
                    {
                        break outcome;
                    }
                    match script {
                        ScriptEvent::StallWait => {
                            stall_dismissed = true;
                            let _ = session.send(companion::stall_clear()).await;
                        }
                        ScriptEvent::StallSwitch if streams.len() > 1 => {
                            break FileOutcome::Switch {
                                to: (index + 1) % streams.len(),
                                at: tracker.position(),
                            };
                        }
                        _ => {}
                    }
                    continue;
                }
                PlaybackEvent::Loaded => {
                    if let Err(e) = session
                        .add_subtitles(stream, settings.subtitle_language.as_deref())
                        .await
                    {
                        tracing::debug!(error = %e, "could not attach external subtitles");
                    }
                }
                PlaybackEvent::Buffering(held) => {
                    if *held && ever_played {
                        stalled_since.get_or_insert_with(tokio::time::Instant::now);
                    } else if !*held {
                        stalled_since = None;
                        stall_dismissed = false;
                        if std::mem::take(&mut stall_shown) {
                            let _ = session.send(companion::stall_clear()).await;
                        }
                    }
                }
                // Our own `loadfile` ending the file it replaced; nothing to record.
                PlaybackEvent::Replaced => continue,
                PlaybackEvent::Progress { position, duration } => {
                    if !ever_played {
                        ever_played = true;
                        let _ = session
                            .send(companion::state("status", &companion::Status::playing()))
                            .await;
                    }
                    // Mirror position to the UI so Now Playing can render without querying mpv —
                    // but only when the rendered value would actually change. mpv reports
                    // `time-pos` about thirty times a second (measured), and every update redraws
                    // the terminal.
                    let whole = position.floor();
                    if whole != shown_second || duration.is_some() != shown_duration {
                        shown_second = whole;
                        shown_duration = duration.is_some();
                        let _ = tx.send(Update::Playback {
                            position: *position,
                            duration: *duration,
                            paused: tracker.is_paused(),
                        });
                        // Rides the same whole-second throttle: Discord rate-limits presence
                        // updates, and one a second is more than a presence line needs.
                        rpc.update(tracker.is_paused()).await;
                    }
                }
                _ => {}
            }

            let file_ended = matches!(event, PlaybackEvent::Ended { .. });
            for action in tracker.observe(&event) {
                match action {
                    Action::OfferSkip { kind, to } => {
                        if tracker.auto_skips() {
                            let from = tracker.position();
                            let _ = session.seek_to(to).await;
                            let _ = session
                                .send(companion::skipped(kind_id(kind), kind.label(), from, to))
                                .await;
                            last_skip = Some(kind);
                        } else {
                            // On the video, not the terminal: the viewer is looking at it.
                            let _ = session
                                .send(companion::skip_offer(kind_id(kind), kind.label(), to))
                                .await;
                        }
                        let _ = tx.send(Update::SkipAvailable { label: kind.label(), to });
                    }
                    Action::ClearSkip => {
                        let _ = session.send(companion::skip_clear()).await;
                        let _ = tx.send(Update::SkipCleared);
                    }
                    other => {
                        let watched_secs = tracker.watched_secs();
                        self.act(
                            other,
                            watched_secs,
                            session,
                            stream,
                            &mut committed,
                            &mut ending,
                        )
                        .await;
                    }
                }
            }

            if let PlaybackEvent::Ended { complete } = event
                && file_ended
            {
                break FileOutcome::Ended {
                    watched: ending,
                    complete,
                    exited: !session.is_alive(),
                    stopped,
                    no_frames: !ever_played,
                };
            }
        };

        // Cleared before anything else, so quitting never leaves a stale "watching" up.
        rpc.finish().await;
        let _ = session.send(companion::skip_clear()).await;
        outcome
    }

    /// Answer what the viewer did inside the player. `Some` ends the file.
    async fn answer(
        &mut self,
        event: &ScriptEvent,
        session: &MpvSession,
        tracker: &mut PlaybackTracker,
        last_skip: &mut Option<SkipKind>,
        streams: &[Stream],
        index: usize,
    ) -> Option<FileOutcome> {
        let context = self.context;
        let tx = self.tx;
        match event {
            ScriptEvent::Ready { uosc } => {
                tracing::debug!(uosc, "companion script ready");
            }
            ScriptEvent::Next | ScriptEvent::Previous => {
                // The next playback's arrival is what ends this one — the task runner hands
                // this window over to it — so nothing stops here.
                let step = if matches!(event, ScriptEvent::Next) { 1 } else { -1 };
                match context.episode.trim().parse::<i64>().ok().map(|n| n + step) {
                    Some(n) if n >= 1 => {
                        let _ = tx.send(Update::PlayerPlayEpisode(n.to_string()));
                    }
                    _ => {
                        let _ = session.notify("already at the first episode").await;
                    }
                }
            }
            ScriptEvent::PlayNext => {
                if let Some(next) = &context.next {
                    let _ = tx.send(Update::PlayerPlayEpisode(next.number.clone()));
                }
            }
            ScriptEvent::CancelNext => {
                self.cancel_next = true;
                let _ = tx.send(Update::AutoNextCancelled);
            }
            ScriptEvent::Skip => {
                if let Some(to) = tracker.offered_skip().map(|s| s.end) {
                    let _ = session.seek_to(to).await;
                }
            }
            ScriptEvent::UndoSkip { to } => {
                if let Some(kind) = last_skip.take() {
                    tracker.decline(kind);
                }
                let _ = session.seek_to(*to).await;
            }
            ScriptEvent::Choose(choice) => match choice {
                Choice::Episode(number) => {
                    if *number != context.episode {
                        let _ = tx.send(Update::PlayerPlayEpisode(number.clone()));
                    }
                }
                Choice::Source(id) => {
                    if let Ok(to) = id.parse::<usize>()
                        && to < streams.len()
                        && to != index
                    {
                        return Some(FileOutcome::Switch { to, at: tracker.position() });
                    }
                }
                Choice::Translation(value) => {
                    let wanted =
                        if value == "dub" { Translation::Dub } else { Translation::Sub };
                    if wanted != context.translation {
                        let _ = tx.send(Update::PlayerTranslation(wanted));
                    }
                }
                Choice::Upscaling(id) => {
                    let _ = tx.send(Update::PlayerUpscaling(id.clone()));
                }
                Choice::ToneMapping(id) => {
                    let _ = session
                        .send(anistream_player::protocol::Command::SetProperty(
                            "tone-mapping",
                            id.clone().into(),
                        ))
                        .await;
                }
                Choice::NightMode(on) => {
                    self.night = *on;
                    self.apply_night(session).await;
                    let _ = tx.send(Update::PlayerNightMode(*on));
                }
            },
            ScriptEvent::StallSwitch | ScriptEvent::StallWait => {}
        }
        None
    }

    /// Carry out a tracker action that is not about skipping.
    async fn act(
        &self,
        action: Action,
        watched_secs: f64,
        session: &MpvSession,
        stream: &Stream,
        committed: &mut bool,
        ending: &mut Option<bool>,
    ) {
        let context = self.context;
        let tx = self.tx;
        match action {
            Action::Record { position, duration, completed } => {
                let watch = WatchEvent {
                    duration_secs: duration,
                    watched_secs,
                    provider_id: Some(stream.provider_id.clone()),
                    translation: Some(context.translation),
                    completed,
                    ..WatchEvent::new(context.anilist_id, &context.episode, position)
                };
                self.record(watch, completed, committed).await;
            }
            Action::ProbeChapters => {
                if let Err(e) = session.restate_chapters().await {
                    tracing::debug!(error = %e, "could not ask for the chapter list");
                }
            }
            Action::MarkChapters(marks) => {
                // Best effort by design: a player that will not take them plays the
                // episode exactly as it did before, and the skip prompt still works.
                if let Err(e) = session.set_chapters(&marks).await {
                    tracing::debug!(error = %e, "player would not take chapter markers");
                }
            }
            Action::RememberSpeed(speed) => {
                let _ = tx.send(Update::PlaybackSpeed(speed));
            }
            Action::RememberVolume(volume) => {
                let _ = tx.send(Update::PlaybackVolume(volume));
            }
            Action::Finished { watched } => {
                // Recorded, not announced — see `ending`. mpv reports an ending on the way out
                // whether it played or not, so this fires for a dead stream too.
                *ending = Some(watched);
                if watched {
                    tracing::info!(
                        episode = %context.episode,
                        "episode finished and recorded as watched"
                    );
                }
            }
            Action::OfferSkip { .. } | Action::ClearSkip => {}
        }
    }

    /// Feed one event through the tracker purely for its bookkeeping.
    async fn record_actions(
        &self,
        tracker: &mut PlaybackTracker,
        event: &PlaybackEvent,
        stream: &Stream,
        committed: &mut bool,
        ending: &mut Option<bool>,
    ) {
        for action in tracker.observe(event) {
            if let Action::Record { position, duration, completed } = action {
                let watch = WatchEvent {
                    duration_secs: duration,
                    watched_secs: tracker.watched_secs(),
                    provider_id: Some(stream.provider_id.clone()),
                    translation: Some(self.context.translation),
                    completed,
                    ..WatchEvent::new(self.context.anilist_id, &self.context.episode, position)
                };
                self.record(watch, completed, committed).await;
            } else if let Action::Finished { watched } = action {
                *ending = Some(watched);
            }
        }
    }

    /// Write a history row, and queue tracker progress the first time the episode completes.
    async fn record(&self, watch: WatchEvent, completed: bool, committed: &mut bool) {
        // Local first, always. The history row is written before anything is queued for a
        // tracker, so a failed push can never cost you the watch.
        let queue_now = completed && !*committed;
        if queue_now {
            *committed = true;
        }
        let store_handle = self.store.clone();
        let trackers = self.settings.tracker_ids.clone();
        let id = self.context.anilist_id;
        // Blocking sqlite calls: off the async worker so a slow disk cannot stall the event
        // stream.
        let _ = tokio::task::spawn_blocking(move || {
            store_handle.record_event(&watch)?;
            if queue_now {
                anistream_track::sync::queue_progress_for(
                    &store_handle,
                    &trackers,
                    id,
                    anistream_store::now(),
                );
            }
            Ok::<_, anistream_store::StoreError>(())
        })
        .await;

        if queue_now {
            let _ = self.tx.send(Update::ProgressQueued);
        }
    }
}

/// The skip kind as the companion names it.
const fn kind_id(kind: SkipKind) -> &'static str {
    kind.label()
}

/// A stream's second line in the Sources menu.
fn stream_detail(stream: &Stream) -> String {
    let kind = match stream.kind {
        StreamKind::Hls => "HLS",
        StreamKind::TorrentHttp => "torrent",
        StreamKind::ExternalDeepLink => "external",
        _ => "direct",
    };
    match stream.quality {
        Some(quality) => format!("{quality}p · {kind}"),
        None => kind.to_owned(),
    }
}

/// The upscaling menu for the configured mode.
fn upscaling_menu(mode: anistream_core::config::Upscaling) -> companion::OptionsMenu {
    let option = |id: &str, label: &str, hint: &str| companion::MenuOption {
        id: id.into(),
        label: label.into(),
        hint: Some(hint.into()),
    };
    companion::OptionsMenu {
        current: mode.name().into(),
        options: vec![
            option("off", "Off", "mpv's own scaling"),
            option("anime4k-fast", "Anime4K · fast", "integrated graphics"),
            option("anime4k-quality", "Anime4K · quality", "discrete GPU"),
        ],
    }
}

/// The Discord presence for one playback, if there is one.
///
/// A wrapper rather than an `Option<Presence>` at every call site: presence is decoration, so every
/// operation on it has to be a no-op when it is absent, and threading that check through the
/// playback loop would put five `if let` blocks around something that cannot fail usefully.
struct Rpc {
    inner: Option<anistream_player::Presence>,
    activity: anistream_player::Activity,
}

impl Rpc {
    async fn start(
        config: &anistream_core::config::PresenceConfig,
        context: &PlaybackContext,
    ) -> Self {
        let activity = anistream_player::Activity {
            // Honouring `show_title` here rather than at send time, so a build that leaks it is a
            // visible mistake rather than a missing branch somewhere downstream.
            title: if config.show_title {
                context.title.clone()
            } else {
                "Watching anime".to_string()
            },
            detail: if config.show_title {
                format!("Episode {}", context.episode)
            } else {
                String::new()
            },
            paused: false,
            started_at: Some(anistream_store::now()),
        };

        let inner = match (config.enabled, config.resolved_client_id()) {
            (true, Some(client_id)) => anistream_player::Presence::connect(client_id).await,
            (true, None) => {
                tracing::info!("presence enabled but no client id is configured; skipping");
                None
            }
            _ => None,
        };
        let mut rpc = Self { inner, activity };
        rpc.push().await;
        rpc
    }

    async fn update(&mut self, paused: bool) {
        if self.inner.is_none() || self.activity.paused == paused {
            return;
        }
        self.activity.paused = paused;
        self.push().await;
    }

    async fn push(&mut self) {
        let Some(presence) = self.inner.as_mut() else { return };
        if let Err(e) = presence.set(&self.activity).await {
            // Dropped rather than retried: a broken pipe means Discord went away, and reconnecting
            // in a playback loop is effort spent on decoration.
            tracing::debug!(error = %e, "discord presence dropped");
            self.inner = None;
        }
    }

    async fn finish(&mut self) {
        if let Some(presence) = self.inner.as_mut() {
            let _ = presence.clear().await;
        }
        self.inner = None;
    }
}

/// Translate a UI control into mpv IPC.
///
/// Every failure is swallowed deliberately: mpv may have exited a millisecond ago, and a lost
/// seek is not worth an error toast when the `Ended` event is already on its way.
async fn apply_command(session: &MpvSession, command: PlayerCommand) {
    match command {
        PlayerCommand::TogglePause => {
            let _ = session.pause_toggle().await;
        }
        PlayerCommand::Seek(delta) => {
            let _ = session.seek(delta).await;
        }
        PlayerCommand::SeekTo(position) => {
            let _ = session.seek_to(position).await;
        }
        PlayerCommand::Speed(delta) => {
            let _ = session.nudge_speed(delta).await;
        }
        PlayerCommand::Volume(delta) => {
            let _ = session.nudge_volume(delta).await;
        }
        PlayerCommand::Fullscreen => {
            let _ = session.fullscreen_toggle().await;
        }
        // Detaching is purely a UI move — mpv is untouched, which is the whole point.
        PlayerCommand::Detach => {}
        // Handled by the playback loop itself, which owns the window being handed on.
        PlayerCommand::Handover => {}
        PlayerCommand::Stop => {
            let _ = session.quit().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subtitles_are_fetched_together_and_kept_in_their_listed_order() {
        use anistream_core::stream::Subtitle;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Every track takes 300 ms to answer, so three fetched in turn would take most of a
        // second before mpv could open.
        let host =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let origin = format!("http://{}", host.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = host.accept().await {
                tokio::spawn(async move {
                    let mut request = [0u8; 2048];
                    let n = socket.read(&mut request).await.unwrap_or(0);
                    let head = String::from_utf8_lossy(&request[..n]).into_owned();
                    let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    let response = if path.contains("missing") {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_owned()
                    } else {
                        let body = format!("WEBVTT\n\nNOTE {path}\n");
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    };
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        let track = |language: &str, url: String, hard| Subtitle {
            language: language.into(),
            url,
            hard,
            format: None,
        };
        let mut stream = Stream {
            url: "http://unused.invalid/master.m3u8".into(),
            kind: StreamKind::Hls,
            quality: None,
            headers: Vec::new(),
            subtitles: vec![
                track("en", format!("{origin}/en.vtt"), false),
                track("de", format!("{origin}/missing.vtt"), false),
                track("fr", format!("{origin}/fr.vtt"), false),
                track("ja", "burned-in".into(), true),
            ],
            provider_id: "test".into(),
            download_source: None,
            pick_note: None,
        };
        let scratch = tempfile::tempdir().unwrap();
        let mpv = Mpv::new(scratch.path());
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();

        let started = std::time::Instant::now();
        localise_subtitles(&http, &mpv, &mut stream).await;
        let took = started.elapsed();

        let languages: Vec<_> = stream.subtitles.iter().map(|s| s.language.as_str()).collect();
        assert_eq!(
            languages,
            ["en", "fr", "ja"],
            "listed order, with the refused track dropped"
        );
        for fetched in &stream.subtitles[..2] {
            assert!(
                !fetched.url.starts_with("http"),
                "{} must point at its local copy",
                fetched.language
            );
            assert!(std::fs::read_to_string(&fetched.url).unwrap().starts_with("WEBVTT"));
        }
        assert!(took < Duration::from_millis(600), "tracks were fetched in turn: {took:?}");
    }

    #[tokio::test]
    async fn a_title_without_a_mal_id_simply_has_no_skip_data() {
        // The mapping layer cannot resolve every title, and skip data is decoration.
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        assert!(fetch_skips(&http, None, "1", None).await.is_empty());
    }

    #[tokio::test]
    async fn a_non_numeric_episode_is_not_looked_up() {
        // "OVA" and "S1" are real episode labels; asking aniskip about them is meaningless.
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        assert!(fetch_skips(&http, Some(52_991), "OVA", None).await.is_empty());
    }

    #[test]
    fn a_playback_context_carries_what_history_needs() {
        // Provider and translation are the fields no tracker can supply, and the reason local
        // history is richer than anything that can be synced.
        let context = PlaybackContext {
            anilist_id: AnilistId::new(154_587),
            mal_id: Some(52_991),
            episode: "12".into(),
            title: "Frieren".into(),
            translation: Translation::Sub,
            resume_at: Some(600.0),
            runtime_secs: Some(1_440),
            speed: Some(1.25),
            volume: Some(80.0),
            next: None,
            episodes: Vec::new(),
        };
        assert_eq!(context.mal_id, Some(52_991));
        assert_eq!(context.resume_at, Some(600.0));
        assert_eq!(context.runtime_secs, Some(1_440));
    }
}
