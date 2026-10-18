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
    ScriptEvent, SkipInterval, SkipKind, TrackAction, TrackInput, TrackPreferences, companion,
    skip,
};
use anistream_store::{Store, WatchEvent};
use anistream_ui::app::{Ending, PlayerCommand, Successor, Toast, Update};
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::{
    picture::{self, Governor, HdrKind},
    power,
    shaders::{self, Look},
    slot::{Player, PlayerSlot},
};
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
    /// The episode's still, by URL, for the Up Next card.
    pub thumbnail: Option<String>,
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

    /// What the window says while it waits for the playback taking it over.
    ///
    /// Worded against this one, because "the next episode" is only true of one of the ways a
    /// window changes hands: another series, a switch to the dub, an episode picked from the
    /// menu, and the previous episode all come through here too.
    pub fn handing_to(&self, successor: &Successor) -> String {
        let number = |episode: &str| episode.trim().parse::<i64>().ok();
        if successor.id != self.anilist_id {
            return format!("Opening {} · Episode {}…", successor.title, successor.episode);
        }
        if successor.episode == self.episode {
            return if successor.translation == self.translation {
                format!("Opening {} again…", self.episode_label())
            } else {
                format!("Switching to the {}…", successor.translation)
            };
        }
        match (number(&self.episode), number(&successor.episode)) {
            (Some(now), Some(then)) if then == now + 1 => "Opening the next episode…".into(),
            (Some(now), Some(then)) if then == now - 1 => {
                "Opening the previous episode…".into()
            }
            _ => format!("Opening Episode {}…", successor.episode),
        }
    }

    fn episode_info(&self, episodes: &[EpisodeEntry]) -> companion::EpisodeInfo {
        companion::EpisodeInfo {
            number: self.episode.clone(),
            label: self.episode_label(),
            title: episodes
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

        let mended = (extension == "vtt")
            .then(|| std::str::from_utf8(&body).ok().and_then(mend_webvtt))
            .flatten();
        if mended.is_some() {
            tracing::debug!(language = %subtitle.language, "subtitle mended");
        }
        let contents = mended.as_deref().map_or(&body[..], str::as_bytes);

        match tokio::fs::write(&path, contents).await {
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

/// A WebVTT file as the source meant it, or `None` when it needs nothing.
///
/// Some sources convert their subtitles to WebVTT from a format that allows more than WebVTT
/// does, and two things come out broken:
///
/// - **A cue with a blank line in it**, an on-screen panel of text laid out as paragraphs. A
///   blank line ends a WebVTT cue, so the paragraphs after it are left as blocks with no timing.
///   The format says to skip such a block. ffmpeg, which mpv reads WebVTT with, stops at it
///   instead, and every cue after it is lost: an episode's subtitles end a few minutes in. Each
///   such block goes back onto the cue before it, after a line holding only a no-break space,
///   which is not blank to a parser and shows as the gap the source had.
/// - **A shape drawn by command**, `m 370 219 l 370 339 595 339`, the box behind that panel. WebVTT
///   has no drawings, so it shows the commands as text. They are dropped, and a cue that was
///   nothing else goes with them.
fn mend_webvtt(text: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n");
    let mut blocks: Vec<String> = Vec::new();
    let mut mended = false;
    for block in text.split("\n\n").map(|b| b.trim_start_matches('\n')) {
        if block.trim().is_empty() {
            continue;
        }
        let mut lines = block.lines();
        let timed = lines.next().is_some_and(|l| l.contains("-->"))
            || lines.next().is_some_and(|l| l.contains("-->"));
        let own_block = ["NOTE", "STYLE", "REGION"].iter().any(|word| block.starts_with(word));
        let kept: Vec<&str> = block.lines().filter(|line| !is_drawing(line)).collect();
        mended |= kept.len() != block.lines().count();
        let block = kept.join("\n");
        match blocks.last_mut() {
            Some(previous) if !timed && !own_block => {
                if !block.trim().is_empty() {
                    previous.push_str("\n\u{a0}\n");
                    previous.push_str(&block);
                }
                mended = true;
            }
            _ => blocks.push(block),
        }
    }
    // A cue whose only text was a drawing is its timing alone now.
    blocks.retain(|block| !block.lines().last().is_some_and(|l| l.contains("-->")));
    mended.then(|| blocks.join("\n\n") + "\n")
}

/// Whether a cue line is a vector drawing — `m`, then drawing commands and coordinates only —
/// once its tags are set aside.
fn is_drawing(line: &str) -> bool {
    let mut text = String::new();
    let mut in_tag = false;
    for c in line.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let mut words = text.split_whitespace();
    if words.next() != Some("m") {
        return false;
    }
    let rest: Vec<&str> = words.collect();
    let number = |w: &str| w.parse::<f64>().is_ok();
    rest.iter().filter(|w| number(w)).count() >= 2
        && rest
            .iter()
            .all(|w| number(w) || matches!(*w, "m" | "n" | "l" | "b" | "s" | "p" | "c"))
}

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
        let _ = tx.send(Update::PlaybackEnded { watched: false, ending: Ending::Failed });
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
        // Nothing plays here, so Now Playing comes down rather than waiting on a player that
        // was never going to report.
        let _ = tx.send(Update::PlaybackEnded { watched: false, ending: Ending::Elsewhere });
        return;
    }

    // A watch party is a shared session: Syncplay owns the player and the pacing, so no
    // private history is recorded — the room pausing must not read as you abandoning the
    // episode. The torrent session stays alive underneath, serving the loopback URL.
    if settings.party {
        slot.release();
        hand_to_syncplay(&settings.syncplay, &first.url, &tx);
        let _ = tx.send(Update::PlaybackEnded { watched: false, ending: Ending::Elsewhere });
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
        prefetch_sent: false,
        look: settings.look,
        tone_mapping: settings.tone_mapping.clone(),
        hdr: None,
        episodes: context.episodes.clone(),
        next: context.next.clone(),
        http: http.clone(),
        scratch: mpv.scratch_dir().to_path_buf(),
        still: None,
        // Fetched alongside everything else rather than before it: the card is minutes away
        // and the episode should not wait on a picture.
        still_fetch: context.next.as_ref().and_then(|n| n.thumbnail.clone()).map(|url| {
            tokio::spawn(fetch_still(http.clone(), url, mpv.scratch_dir().to_path_buf()))
        }),
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
                Ok((session, events)) => player = Some(Player::new(session, events)),
                Err(e) if last => {
                    let _ = tx.send(Update::Toast(Toast::alert(format!("mpv: {e}"))));
                    // The player never existed, but the UI has Now Playing up and a control
                    // surface pointed at it. Saying so is what takes them down.
                    let _ = tx
                        .send(Update::PlaybackEnded { watched: false, ending: Ending::Failed });
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
            FileOutcome::Handover(successor) => {
                if let Some(player) = player.take() {
                    let _ = player
                        .session
                        .send(companion::state(
                            "status",
                            &companion::Status::loading(context.handing_to(&successor), None),
                        ))
                        .await;
                    // Silence what was playing while the next stream is found.
                    let _ = player.session.stop().await;
                    slot.park(player);
                }
                return;
            }
            FileOutcome::Ended { watched, complete, exited, stopped, quit, no_frames } => {
                let reported = match (&player, no_frames) {
                    (Some(p), true) => p.session.diagnostics().most_relevant().await,
                    _ => None,
                };

                if no_frames && !stopped && !exited && !last {
                    // The point of carrying alternates: whether segments actually decode is
                    // knowable only in front of a player, so a stream that never showed a frame
                    // fails over — in the same window — unless the viewer stopped it or closed
                    // the window themselves, which is an instruction, not a failure.
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
                    && run.next.is_some();
                let next = run.next.as_ref().filter(|_| rolls_on);
                if let Some(player) = player.take() {
                    if let Some(next) = next {
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
                    tracing::warn!(%message, url = %stream.url, "playback produced no frames");
                    let _ = tx.send(Update::Toast(Toast::alert(message)));
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
                let _ = tx
                    .send(Update::PlaybackEnded { watched: watched.unwrap_or(false), ending });
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
}

impl Run<'_> {
    /// Tell the companion what it is showing: the episode, what follows, the menus.
    async fn publish(&mut self, session: &MpvSession, streams: &[Stream], current: usize) {
        let context = self.context;
        let next = self.next_up();
        let commands = [
            companion::state("episode", &context.episode_info(&self.episodes)),
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

    /// Ask for the next episode's streams once this one's ending begins.
    ///
    /// Only when the episode will roll on by itself: fetching ahead for a viewer who stops at
    /// the end would be a search, and for a torrent a download, nobody asked for.
    fn maybe_prefetch(&mut self, position: f64, duration: Option<f64>) {
        if self.prefetch_sent || !self.settings.auto_next || self.cancel_next {
            return;
        }
        let Some(next) = &self.next else { return };
        let ending = self.skips.iter().find(|s| s.kind == SkipKind::Ending).map(|s| s.start);
        let at = ending.or_else(|| {
            duration.filter(|d| *d > 2.0 * PREFETCH_LEAD).map(|d| d - PREFETCH_LEAD)
        });
        if at.is_some_and(|at| position >= at) {
            self.prefetch_sent = true;
            tracing::info!(episode = %next.number, "finding the next episode ahead");
            let _ = self.tx.send(Update::PrefetchNext {
                id: self.context.anilist_id,
                episode: next.number.clone(),
            });
        }
    }

    /// Take an episode list that arrived after playback started.
    ///
    /// Only fills what the context left empty: a next episode the episode table already named
    /// is the one the viewer saw, and stays. Returns the next episode's still to fetch, if a
    /// next episode was learned here and has one.
    fn learn_episodes(
        &mut self,
        rows: Vec<anistream_ui::app::EpisodeRow>,
        next: Option<String>,
    ) -> Option<String> {
        if self.episodes.is_empty() {
            self.episodes = rows
                .iter()
                .map(|row| EpisodeEntry {
                    number: row.number.clone(),
                    title: row.title.clone(),
                    watched: row.completed,
                    filler: row.skippable,
                })
                .collect();
        }
        if self.next.is_some() {
            return None;
        }
        let row = rows.into_iter().find(|row| next.as_deref() == Some(row.number.as_str()))?;
        tracing::info!(episode = %row.number, "learned the next episode after starting");
        self.next = Some(NextEpisode {
            number: row.number,
            title: row.title,
            thumbnail: row.thumbnail.clone(),
        });
        row.thumbnail
    }

    /// What the Up Next card shows, as it stands now.
    fn next_up(&self) -> Option<companion::NextUp> {
        let ending = self.skips.iter().find(|s| s.kind == SkipKind::Ending).map(|s| s.start);
        self.next.as_ref().map(|next| companion::NextUp {
            number: next.number.clone(),
            label: next.label(),
            title: next.title.clone(),
            at: ending,
            auto: self.settings.auto_next && !self.cancel_next,
            thumbnail: self.still.clone(),
        })
    }

    fn menus(&self, streams: &[Stream], current: usize) -> companion::Menus {
        let context = self.context;
        companion::Menus {
            episodes: self
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
                .zip(source_details(streams))
                .enumerate()
                .map(|(i, (s, detail))| companion::SourceItem {
                    id: i.to_string(),
                    label: s.provider_id.clone(),
                    detail: Some(detail),
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
            upscaling: shaders::menu(self.look),
            tone_mapping: picture::tone_mapping_menu(self.hdr, &self.tone_mapping),
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
        let mut still_fetch = self.still_fetch.take();
        // The picture: what the file is, and what the GPU is saying about the chain.
        let mut source: Option<shaders::Source> = None;
        let mut video_params = serde_json::Value::Null;
        let mut video_track: Option<serde_json::Value> = None;
        let mut drops = 0_u64;
        let mut seeking = false;
        let mut buffering = false;
        let mut picture_tick = tokio::time::interval(Duration::from_secs(1));
        // The governor keeps its steps across episodes in this window, but its evidence is
        // per file.
        player.picture.governor.new_file();
        self.hdr = None;

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
                        quit: session.has_quit(),
                        no_frames: !ever_played,
                    },
                },
                // Only armed until the first frame arrives, so this cannot fire mid-episode
                // during a legitimate pause — mpv keeps reporting `time-pos` while paused.
                _ = tokio::time::sleep(FIRST_FRAME_TIMEOUT), if !ever_played && !warned => {
                    let _ = tx.send(Update::Toast(Toast::alert(format!(
                        "mpv has not started playing after {}s — press x to stop; the source may have no data, or check `mpv` plays a file on its own",
                        FIRST_FRAME_TIMEOUT.as_secs()
                    ))));
                    // Deliberately not a stop. mpv may still be buffering a slow torrent, and
                    // killing it would turn a wait into a failure; the user now knows and can
                    // press `x`.
                    warned = true;
                    continue;
                }
                fetched = async { still_fetch.as_mut().expect("guarded").await },
                    if still_fetch.is_some() =>
                {
                    still_fetch = None;
                    self.still = fetched.ok().flatten();
                    if let Some(next) = self.next_up() {
                        let _ = session.send(companion::state("next", &next)).await;
                    }
                    continue;
                }
                _ = picture_tick.tick() => {
                    let picture = &mut player.picture;
                    let mut changed = false;
                    // Battery about once a minute: cheap, but a subprocess on macOS.
                    if picture.battery_checked.is_none_or(|t| t.elapsed() >= BATTERY_POLL) {
                        picture.battery_checked = Some(tokio::time::Instant::now());
                        let on = tokio::task::spawn_blocking(power::on_battery)
                            .await
                            .ok()
                            .flatten()
                            .unwrap_or(false);
                        changed |= std::mem::replace(&mut picture.on_battery, on) != on;
                    }
                    // Fed once a second, and only while a chain runs: drops with no shaders
                    // are not ours to fix.
                    if !picture.applied.is_empty() && ever_played {
                        let time = picture.clock.elapsed().as_secs_f64();
                        let held = tracker.is_paused() || buffering || seeking;
                        changed |= picture.governor.sample(time, drops, held).is_some();
                    }
                    if changed {
                        self.apply_picture(&mut player.picture, session, source.as_ref(), streams, index, true).await;
                    }
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
                        Some(PlayerCommand::Handover(successor)) => {
                            // Flush what was watched before the window changes hands.
                            self.record_actions(&mut tracker, &PlaybackEvent::Ended { complete: false }, stream, &mut committed, &mut ending).await;
                            let _ = tx.send(Update::EpisodeLeft {
                                id: context.anilist_id,
                                episode: context.episode.clone(),
                                position: tracker.position(),
                                duration: tracker.duration(),
                                watched: ending.unwrap_or(false),
                            });
                            break FileOutcome::Handover(successor);
                        }
                        Some(PlayerCommand::Episodes { rows, next }) => {
                            if let Some(url) = self.learn_episodes(rows, next)
                                && still_fetch.is_none()
                                && self.still.is_none()
                            {
                                still_fetch = Some(tokio::spawn(fetch_still(
                                    self.http.clone(),
                                    url,
                                    self.scratch.clone(),
                                )));
                            }
                            let info = context.episode_info(&self.episodes);
                            let _ = session.send(companion::state("episode", &info)).await;
                            if let Some(next) = self.next_up() {
                                let _ = session.send(companion::state("next", &next)).await;
                            }
                            let menus = self.menus(streams, index);
                            let _ = session.send(companion::state("menus", &menus)).await;
                            continue;
                        }
                        Some(PlayerCommand::Step(delta)) => {
                            self.step(delta, session, true).await;
                            continue;
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

            // Tracks first, and for every event: the policy reads the clock off each one.
            let track_input = match &event {
                PlaybackEvent::TrackList(list) => Some(TrackInput::TrackList(
                    anistream_player::tracks::parse_track_list(list),
                )),
                PlaybackEvent::Aid(id) => Some(TrackInput::Aid(*id)),
                PlaybackEvent::Sid(id) => Some(TrackInput::Sid(*id)),
                PlaybackEvent::SubDelay(delay) => Some(TrackInput::SubDelay(*delay)),
                PlaybackEvent::Loaded => Some(TrackInput::FileLoaded),
                PlaybackEvent::Ended { .. } | PlaybackEvent::Replaced => {
                    Some(TrackInput::EndFile)
                }
                PlaybackEvent::Progress { .. } => Some(TrackInput::Tick),
                _ => None,
            };
            if let Some(input) = track_input {
                for action in player.tracks.observe(&input, std::time::Instant::now()) {
                    self.apply_track(action, session).await;
                }
            }

            match &event {
                PlaybackEvent::VideoParams(_) | PlaybackEvent::VideoTrack(_) => {
                    match &event {
                        PlaybackEvent::VideoParams(params) => video_params = params.clone(),
                        PlaybackEvent::VideoTrack(track) => video_track = Some(track.clone()),
                        _ => {}
                    }
                    // Only a change in size or kind matters; HDR10+ restates video-params with
                    // fresh scene metadata on every frame.
                    let fresh = read_source(&video_params, video_track.as_ref());
                    if fresh.is_some() && fresh != source {
                        source = fresh;
                        self.hdr = source.and_then(|s| s.hdr);
                        self.apply_picture(
                            &mut player.picture,
                            session,
                            source.as_ref(),
                            streams,
                            index,
                            false,
                        )
                        .await;
                    }
                    continue;
                }
                PlaybackEvent::Display { width, height } => {
                    player.picture.display =
                        Some(shaders::Display { width: *width, height: *height });
                    self.apply_picture(
                        &mut player.picture,
                        session,
                        source.as_ref(),
                        streams,
                        index,
                        false,
                    )
                    .await;
                    continue;
                }
                PlaybackEvent::FrameDrops(count) => {
                    drops = *count;
                    continue;
                }
                PlaybackEvent::Seeking(now) => {
                    seeking = *now;
                    continue;
                }
                PlaybackEvent::Script(ScriptEvent::Choose(Choice::Upscaling(id))) => {
                    if let Some(look) = Look::from_id(id) {
                        self.look = look;
                        // A new look starts at full budget; the governor earns its steps again.
                        player.picture.governor.reset();
                        self.apply_picture(
                            &mut player.picture,
                            session,
                            source.as_ref(),
                            streams,
                            index,
                            false,
                        )
                        .await;
                        let _ = tx.send(Update::PlayerUpscaling(id.clone()));
                    }
                    continue;
                }
                PlaybackEvent::Script(ScriptEvent::Choose(Choice::ToneMapping(id))) => {
                    let _ = session
                        .send(anistream_player::protocol::Command::SetProperty(
                            "tone-mapping",
                            id.clone().into(),
                        ))
                        .await;
                    self.tone_mapping = id.clone();
                    let _ = session
                        .send(companion::state("menus", &self.menus(streams, index)))
                        .await;
                    let _ = tx.send(Update::PlayerToneMapping(id.clone()));
                    continue;
                }
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
                    buffering = *held;
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
                    self.maybe_prefetch(*position, *duration);
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
                        let to = skip_target(to, tracker.duration());
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

            // The file's own chapters arrive once it is playing, and outrank aniskip's times.
            // Up Next and the look-ahead for the next episode both hang off where the ending
            // starts, so they move with it.
            if tracker.skips() != self.skips.as_slice() {
                self.skips = tracker.skips().to_vec();
                if let Some(next) = self.next_up() {
                    let _ = session.send(companion::state("next", &next)).await;
                }
            }

            if let PlaybackEvent::Ended { complete } = event
                && file_ended
            {
                // `end-file` for a quit arrives before the socket closes, so the window is
                // already as good as gone when mpv says it is quitting.
                break FileOutcome::Ended {
                    watched: ending,
                    complete,
                    exited: !session.is_alive() || session.has_quit(),
                    stopped,
                    quit: session.has_quit(),
                    no_frames: !ever_played,
                };
            }
        };

        // An unfinished fetch carries over to the next stream this playback tries.
        self.still_fetch = still_fetch;
        // Cleared before anything else, so quitting never leaves a stale "watching" up.
        rpc.finish().await;
        let _ = session.send(companion::skip_clear()).await;
        outcome
    }

    /// Step to an adjacent episode: N/P in the player, n/N in the terminal.
    ///
    /// The next playback's arrival is what ends this one — the task runner hands this window
    /// over to it — so nothing stops here, and nothing at all happens when the episode list says
    /// there is no such episode: asking for it anyway handed the finale's window to a search
    /// that could only fail, which closed the window on the episode being watched. An episode
    /// list not known yet is no evidence either way, and the step goes ahead.
    async fn step(&self, delta: i64, session: &MpvSession, from_terminal: bool) {
        let target = self.context.episode.trim().parse::<i64>().ok().map(|n| n + delta);
        let listed = |n: i64| {
            self.episodes.is_empty() || self.episodes.iter().any(|e| e.number == n.to_string())
        };
        let refusal = match target {
            Some(n) if n < 1 => Some("Already at the first episode".to_owned()),
            Some(n) if listed(n) => None,
            // "OVA" and "Special" have no neighbour to guess at.
            _ if delta > 0 => Some(match target {
                Some(_) => {
                    format!("No next episode · {} is the last", self.context.episode_label())
                }
                None => "No next episode".to_owned(),
            }),
            _ => Some("No previous episode".to_owned()),
        };
        match (refusal, target) {
            (Some(refusal), _) => {
                // Said where the key was pressed.
                if from_terminal {
                    let _ = self.tx.send(Update::Toast(Toast::info(refusal.to_lowercase())));
                } else {
                    let _ = session.notify(refusal).await;
                }
            }
            (None, Some(n)) => {
                let episode = n.to_string();
                let title = self.episode_title(&episode);
                let _ = self.tx.send(Update::PlayerPlayEpisode { episode, title });
            }
            (None, None) => {}
        }
    }

    /// An episode's name, from the list this playback holds.
    fn episode_title(&self, number: &str) -> Option<String> {
        self.episodes.iter().find(|e| e.number == number).and_then(|e| e.title.clone())
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
                let delta = if matches!(event, ScriptEvent::Next) { 1 } else { -1 };
                self.step(delta, session, false).await;
            }
            ScriptEvent::PlayNext => {
                if let Some(next) = &self.next {
                    let _ = tx.send(Update::PlayerPlayEpisode {
                        episode: next.number.clone(),
                        title: next.title.clone(),
                    });
                }
            }
            // Held here, for this playback only: the next one starts with Up Next as the
            // settings have it, however this episode was left.
            ScriptEvent::CancelNext => self.cancel_next = true,
            ScriptEvent::Skip => {
                if let Some(to) =
                    tracker.offered_skip().map(|s| skip_target(s.end, tracker.duration()))
                {
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
                        let _ = tx.send(Update::PlayerPlayEpisode {
                            episode: number.clone(),
                            title: self.episode_title(number),
                        });
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
                // Handled where the picture state lives, in `file`.
                Choice::Upscaling(_) | Choice::ToneMapping(_) => {}
                Choice::NightMode(on) => {
                    self.night = *on;
                    self.apply_night(session).await;
                    // The menu's toggle offers the opposite of what it was last told, so it has
                    // to be told: left stale, turning night mode off offered "off" again, and
                    // it could never be turned back on.
                    let _ = session
                        .send(companion::state("menus", &self.menus(streams, index)))
                        .await;
                    let _ = tx.send(Update::PlayerNightMode(*on));
                }
            },
            ScriptEvent::StallSwitch | ScriptEvent::StallWait => {}
        }
        None
    }

    /// Choose the upscaling chain for the file as it now stands, and set it if it changed.
    ///
    /// `announce` is for changes the viewer did not ask for — the governor easing, the battery
    /// — which get a short note on the video; a pick from the menu speaks for itself.
    async fn apply_picture(
        &self,
        picture: &mut PictureState,
        session: &MpvSession,
        source: Option<&shaders::Source>,
        streams: &[Stream],
        index: usize,
        announce: bool,
    ) {
        let (Some(source), Some(dir)) = (source, &self.settings.shader_dir) else { return };
        let budget =
            shaders::Budget { on_battery: picture.on_battery, eased: picture.governor.eased() };
        let chain = shaders::choose(self.look, source, picture.display, budget);
        let paths = shaders::paths(&chain, dir);
        if paths != picture.applied && session.set_shaders(&paths).await.is_ok() {
            tracing::info!(label = chain.label, ?budget, "upscaling chain");
            picture.applied = paths;
            if announce {
                let _ = session.notify(format!("Upscaling eased · {}", chain.label)).await;
            }
        }
        let media = companion::Media {
            hdr: source.hdr.map(HdrKind::badge),
            upscaler: chain.upscaler.map(str::to_owned),
        };
        let _ = session.send(companion::state("media", &media)).await;
        let _ = session.send(companion::state("menus", &self.menus(streams, index))).await;
    }

    /// Carry out what the track policy decided.
    async fn apply_track(&self, action: TrackAction, session: &MpvSession) {
        if let TrackAction::Remember(memory) = action {
            let key = (self.context.anilist_id, self.context.translation);
            remember_tracks(self.store, self.settings.remember_tracks, key, memory);
            return;
        }
        if let Some((name, value)) = action.property() {
            let _ = session
                .send(anistream_player::protocol::Command::SetProperty(name, value))
                .await;
        }
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

/// Store a series' track memory, off the async worker.
fn remember_tracks(
    store: &Store,
    enabled: bool,
    (id, translation): (AnilistId, Translation),
    memory: anistream_player::tracks::TrackMemory,
) {
    if !enabled {
        return;
    }
    let store = store.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = store.remember_tracks(id, translation, &memory) {
            tracing::warn!(error = %e, "could not remember track choices");
        }
    });
}

/// How long before the end the next episode is found, when the ending's start is not known.
const PREFETCH_LEAD: f64 = 90.0;

/// How often the battery is asked about during playback.
const BATTERY_POLL: Duration = Duration::from_secs(60);

/// What the window is doing to the picture.
///
/// Kept with the player rather than the episode, like the track policy: a GPU that could not
/// keep up on episode 3 will not on episode 4 either, so the governor's steps carry over, and
/// mpv keeps the chain from one file to the next, so what is applied is the window's.
pub struct PictureState {
    governor: Governor,
    /// The chain mpv is running, as absolute paths; compared before every write.
    applied: Vec<String>,
    display: Option<shaders::Display>,
    on_battery: bool,
    battery_checked: Option<tokio::time::Instant>,
    /// The governor's clock: monotonic, so a pause or a seek cannot move it.
    clock: std::time::Instant,
}

impl Default for PictureState {
    fn default() -> Self {
        Self {
            governor: Governor::new(Governor::DEFAULT_CAP),
            applied: Vec::new(),
            display: None,
            on_battery: false,
            battery_checked: None,
            clock: std::time::Instant::now(),
        }
    }
}

/// The picture's size and kind, from `video-params` — `dw`/`dh`, the size after aspect
/// correction, which is what is actually shown.
fn read_source(
    params: &serde_json::Value,
    track: Option<&serde_json::Value>,
) -> Option<shaders::Source> {
    let dimension = |key: &str| params.get(key)?.as_u64().and_then(|v| u32::try_from(v).ok());
    Some(shaders::Source {
        width: dimension("dw")?,
        height: dimension("dh")?,
        hdr: picture::classify(params, track),
    })
}

/// The size the Up Next still is written at: 16:9, large enough to stay sharp when the card is
/// drawn at HiDPI scale, small enough to write in a few milliseconds.
const STILL: (u32, u32) = (640, 360);

/// Fetch an episode still and write it as raw BGRA for `overlay-add`.
///
/// Cropped to fill 16:9 rather than letterboxed: stills arrive in every shape and the card has
/// one. `None` on any failure — the card simply shows no picture.
async fn fetch_still(
    http: HttpClient,
    url: String,
    dir: std::path::PathBuf,
) -> Option<companion::Still> {
    let response = http.plain().get(&url).send().await.ok()?.error_for_status().ok()?;
    let bytes = response.bytes().await.ok()?;
    tokio::task::spawn_blocking(move || {
        let (width, height) = STILL;
        let image = image::load_from_memory(&bytes).ok()?;
        let mut pixels = image
            .resize_to_fill(width, height, image::imageops::FilterType::Lanczos3)
            .to_rgba8()
            .into_raw();
        // mpv's overlays are BGRA; swap red and blue in place.
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let name = format!("upnext-{}.bgra", anistream_ui::image::stable_hash(&url));
        let path = dir.join(name);
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, &pixels).ok()?;
        Some(companion::Still { path: path.to_string_lossy().into_owned(), width, height })
    })
    .await
    .ok()
    .flatten()
}

/// The skip kind as the companion names it.
const fn kind_id(kind: SkipKind) -> &'static str {
    kind.label()
}

/// How far short of the end a skip lands when its segment runs to the end of the file.
///
/// An ending cut as the release's last chapter ends exactly at the runtime, and aniskip's times,
/// taken against another rip, can end past it. Seeking there left mpv seeking at end-of-file for
/// up to most of a minute, until Up Next fired on its own; landing just short lets the last
/// frames play and the file end the ordinary way, which is what rolls on to the next episode.
const SKIP_SHORT_OF_END: f64 = 1.0;

/// Where a skip to `to` lands in a file `duration` long: `to` itself, unless that is at or past
/// the end. An unknown length is no reason to second-guess the segment.
fn skip_target(to: f64, duration: Option<f64>) -> f64 {
    match duration {
        Some(d) if d > SKIP_SHORT_OF_END => to.min(d - SKIP_SHORT_OF_END),
        _ => to,
    }
}

/// Each stream's second line in the Sources menu, told apart where two would read the same.
///
/// A provider often offers an episode from several servers with nothing else between them, and
/// two rows reading "HLS" under the same provider gave no reason to pick either, nor a way to
/// see which one was playing. The server's name tells them apart, and a number where even that
/// is shared.
fn source_details(streams: &[Stream]) -> Vec<String> {
    let detail = |i: usize, server: bool| {
        let base = stream_detail(&streams[i]);
        match server.then(|| server_name(&streams[i].url)).flatten() {
            Some(name) => format!("{base} · {name}"),
            None => base,
        }
    };
    let alike = |i: usize, j: usize, server: bool| {
        streams[i].provider_id == streams[j].provider_id
            && detail(i, server) == detail(j, server)
    };
    let shared =
        |i: usize, server: bool| (0..streams.len()).any(|j| j != i && alike(i, j, server));
    (0..streams.len())
        .map(|i| {
            if !shared(i, false) {
                return detail(i, false);
            }
            if !shared(i, true) {
                return detail(i, true);
            }
            // In the order they are listed, which is the order they are ranked.
            let nth = (0..i).filter(|&j| alike(i, j, true)).count() + 1;
            format!("{} · {nth}", detail(i, true))
        })
        .collect()
}

/// The part of a stream's host that names its server: `edge.alpha.example` is `alpha`. An
/// address is kept whole; a loopback one names no server, and torrents share it.
fn server_name(url: &str) -> Option<String> {
    let authority = url.split_once("://")?.1.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
    if host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()) {
        return None;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Some(host.to_owned());
    }
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    let name = match labels.as_slice() {
        [] => return None,
        [only] => *only,
        [.., name, _suffix] => *name,
    };
    Some(name.to_owned())
}

/// A stream's second line in the Sources menu, before telling twins apart.
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
        PlayerCommand::Alert(text) => {
            let _ = session.send(companion::toast(&text, companion::Level::Alert)).await;
        }
        // Detaching is purely a UI move — mpv is untouched, which is the whole point.
        PlayerCommand::Detach => {}
        // Handled by the playback loop itself, which owns the window and what it shows.
        PlayerCommand::Handover(_)
        | PlayerCommand::Episodes { .. }
        | PlayerCommand::Step(_) => {}
        PlayerCommand::Stop => {
            let _ = session.quit().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cue_with_a_blank_line_keeps_its_paragraphs_and_the_cues_after_it() {
        let vtt = "WEBVTT\n\n00:01.000 --> 00:02.000\nHello.\n\n\
                   q1\n00:03.000 --> 00:06.000\n<b>Quest</b>\n\nRescue the prisoner,\nquickly.\n\n\
                   00:03.000 --> 00:06.000\n<b>m 0 0 l 200 0 200 71 0 71</b>\n\n\
                   00:04.000 --> 00:05.000\nmore to do\nm 370 219 l 370 339 595 339\n\n\
                   NOTE kept as it is\n\n00:07.000 --> 00:08.000\nGoodbye.\n";
        let mended = mend_webvtt(vtt).expect("the file is mended");
        assert_eq!(
            mended,
            "WEBVTT\n\n00:01.000 --> 00:02.000\nHello.\n\n\
             q1\n00:03.000 --> 00:06.000\n<b>Quest</b>\n\u{a0}\nRescue the prisoner,\nquickly.\n\n\
             00:04.000 --> 00:05.000\nmore to do\n\n\
             NOTE kept as it is\n\n00:07.000 --> 00:08.000\nGoodbye.\n"
        );

        let clean = "WEBVTT\r\n\r\n00:01.000 --> 00:02.000\r\nHello.\r\nmy 2 cents\r\n";
        assert_eq!(mend_webvtt(clean), None, "a sound file is left as it came");
    }

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
            thumbnail_url: None,
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

    fn hls(provider: &str, url: &str) -> Stream {
        Stream { provider_id: provider.into(), ..Stream::new(url, StreamKind::Hls) }
    }

    #[test]
    fn sources_that_would_read_the_same_are_told_apart() {
        let streams = [
            hls("plugin", "https://edge-1.alpha.example/a/master.m3u8"),
            hls("plugin", "https://cdn.beta.example/b/master.m3u8?token=x"),
            hls("plugin", "https://cdn.beta.example/c/master.m3u8"),
            hls("other", "https://cdn.gamma.example/master.m3u8").with_quality(720),
        ];
        assert_eq!(
            source_details(&streams),
            ["HLS · alpha", "HLS · beta · 1", "HLS · beta · 2", "720p · HLS"],
            "a row already unique is left as it was"
        );
        // A loopback address names no server; torrents behind it are numbered.
        let local = [
            hls("torrent", "http://127.0.0.1:4040/s/1"),
            hls("torrent", "http://127.0.0.1:4040/s/2"),
        ];
        assert_eq!(source_details(&local), ["HLS · 1", "HLS · 2"]);
    }

    #[test]
    fn a_skip_that_reaches_the_end_lands_just_short_of_it() {
        // An ending that closes the file, and aniskip's ending for a longer rip.
        assert_eq!(skip_target(1420.0, Some(1420.0)), 1420.0 - SKIP_SHORT_OF_END);
        assert_eq!(skip_target(1507.0, Some(1420.0)), 1420.0 - SKIP_SHORT_OF_END);
        // Anything short of the end, or a length not known yet, is the segment's own end.
        assert_eq!(skip_target(93.2, Some(1420.0)), 93.2);
        assert_eq!(skip_target(1507.0, None), 1507.0);
    }

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
