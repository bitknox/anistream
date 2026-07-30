//! Wiring playback to history.
//!
//! The policy lives in [`anistream_player::PlaybackTracker`], which is pure and tested. This is
//! the part that talks to the world: it spawns mpv, feeds its events to the tracker, and turns
//! the resulting actions into rows in the local database.
//!
//! History is written **before** anything is pushed anywhere. Local is the source of truth, so
//! if a tracker push fails the watch is still recorded and the outbox can retry.

use std::{sync::Arc, time::Duration};

use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
    traits::PlaybackRequest,
};
use anistream_net::HttpClient;
use anistream_player::{
    Action, ExternalPlayer, Mpv, MpvSession, PlaybackEvent, PlaybackTracker, SkipInterval, skip,
};
use anistream_store::{Store, WatchEvent};
use anistream_ui::app::{PlayerCommand, Toast, Update};
use tokio::sync::mpsc;
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
    /// Speed carried over from the previous episode.
    pub speed: Option<f64>,
    /// Volume carried over from the previous session.
    pub volume: Option<f64>,
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
    let mut kept = Vec::with_capacity(stream.subtitles.len());

    for (index, mut subtitle) in std::mem::take(&mut stream.subtitles).into_iter().enumerate() {
        // Already burned in, or already a local path — nothing to fetch either way.
        if subtitle.hard || !subtitle.url.starts_with("http") {
            kept.push(subtitle);
            continue;
        }

        let mut request = http.emulated().get(&subtitle.url);
        for (name, value) in &headers {
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
            continue;
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
        kept.push(subtitle);
    }

    stream.subtitles = kept;
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
) -> Vec<SkipInterval> {
    let Some(mal_id) = mal_id else {
        tracing::debug!("no mal id; skip data unavailable for this title");
        return Vec::new();
    };
    let Ok(number) = episode.trim().parse::<u32>() else {
        return Vec::new();
    };

    match http.plain().get(skip::query_url(mal_id, number)).send().await {
        Ok(response) if response.status().is_success() => {
            let body = response.text().await.unwrap_or_default();
            let intervals = skip::parse(&body);
            tracing::info!(count = intervals.len(), "skip intervals loaded");
            intervals
        }
        // aniskip returns 404 for titles nobody has submitted times for, which is the norm.
        _ => Vec::new(),
    }
}

/// Play a stream and record what happens.
///
/// Returns once playback has ended. Runs as its own task so the UI keeps rendering.
#[allow(clippy::too_many_arguments)]
pub async fn play(
    stream: Stream,
    context: PlaybackContext,
    store: Store,
    http: HttpClient,
    mpv: Mpv,
    threshold: f64,
    auto_skip: bool,
    subtitle_language: Option<String>,
    // Tracker ids to queue progress against when the episode completes.
    tracker_ids: Vec<String>,
    presence: anistream_core::config::PresenceConfig,
    syncplay: anistream_core::config::SyncplayConfig,
    // Whether this play joins the party — from the `y` key, or `[syncplay] enabled`
    // sending every play there.
    party: bool,
    tx: mpsc::UnboundedSender<Update>,
    mut commands: mpsc::UnboundedReceiver<PlayerCommand>,
) {
    // A licensed stream cannot be decoded locally, so it is handed to whatever is licensed to
    // play it. No progress is tracked, because we never see any.
    if stream.kind == StreamKind::ExternalDeepLink {
        use anistream_core::traits::Player;
        let player = ExternalPlayer::new();
        let request = PlaybackRequest { title: context.title.clone(), ..Default::default() };
        let update = match player.play(&stream, request).await {
            Ok(()) => Update::Toast(Toast::info("opened in your browser")),
            Err(e) => Update::Toast(Toast::alert(format!("could not open: {e}"))),
        };
        let _ = tx.send(update);
        return;
    }

    // A watch party is a shared session: Syncplay owns the player and the pacing, so no
    // private history is recorded — the room pausing must not read as you abandoning the
    // episode. The torrent session stays alive underneath, serving the loopback URL.
    if party {
        // Syncplay's no-gui mode refuses to start without a room, so asking first beats
        // spawning a process whose refusal we would have to fish out of its stderr.
        let Some(room) = syncplay.room.clone().filter(|r| !r.trim().is_empty()) else {
            let _ = tx.send(Update::Toast(Toast::alert(
                "syncplay needs a room — set syncplay.room in config.toml",
            )));
            let _ = tx.send(Update::PlaybackEnded { watched: false });
            return;
        };

        let mut command = std::process::Command::new(&syncplay.binary);
        command.arg("--no-gui");
        command.args(["--host", &syncplay.server]);
        command.args(["--name", &syncplay.name]);
        command.args(["--room", &room]);
        command.arg(&stream.url);
        // The TUI owns the terminal. One line of child stderr — Syncplay greets with a
        // Python deprecation warning — scribbles straight over the alternate screen, so
        // the party speaks through toasts and nothing else.
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());

        match command.spawn() {
            Ok(mut child) => {
                let _ = tx.send(Update::Toast(Toast::info(format!(
                    "handed to syncplay — room {room}"
                ))));
                // With stdio silenced, a failed start would otherwise be a party that
                // just never happens. Watch the exit instead: quiet on success, a toast
                // naming the failure whenever it dies unhappy.
                let tx = tx.clone();
                tokio::task::spawn_blocking(move || {
                    if let Ok(status) = child.wait()
                        && !status.success()
                    {
                        let _ = tx.send(Update::Toast(Toast::alert(format!(
                            "syncplay exited with {status} — check server and room in \
                             config.toml"
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
        let _ = tx.send(Update::PlaybackEnded { watched: false });
        return;
    }

    let skips = fetch_skips(&http, context.mal_id, &context.episode).await;

    // Before mpv starts, so the tracks are on disk by the time it reads its arguments.
    let mut stream = stream;
    localise_subtitles(&http, &mpv, &mut stream).await;

    let request = PlaybackRequest {
        title: context.title.clone(),
        start_at: context.resume_at,
        subtitle_language,
        speed: context.speed,
        volume: context.volume,
        dub: context.translation == Translation::Dub,
    };

    let _ = tx.send(Update::Status("starting mpv…".into()));
    let (session, mut events) = match mpv.play(&stream, &request).await {
        Ok(pair) => pair,
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("mpv: {e}"))));
            return;
        }
    };
    let session = Arc::new(session);
    let _ =
        tx.send(Update::Status(format!("playing {} ep {}", context.title, context.episode)));
    // mpv is already at the resume point by now — `--start` handled it. Saying so is what keeps
    // it from looking like the episode began in the wrong place.
    if let Some(position) = context.resume_at {
        let _ = tx.send(Update::Resumed { position });
    }

    // Say the in-player keys exist, once, on mpv's own OSD — bindings nobody announces
    // are bindings nobody finds. One line at session start, then out of the way.
    let _ = session.notify("N next · P previous · S skip").await;

    let mut tracker = PlaybackTracker::new(threshold, skips, auto_skip);

    // Presence is connected here rather than at startup: it should exist for exactly as long as
    // something is playing, and holding a socket open while idle would claim a session that is not
    // happening. `None` covers both "turned off" and "Discord is not running", which need no
    // distinction — see the module docs on why neither is an error.
    let mut presence = Rpc::start(&presence, &context).await;

    // Cleared when the UI drops its sender, which disables that select branch — otherwise a
    // closed channel returns `None` instantly and spins the loop.
    let mut controls_open = true;
    // What the UI is currently showing, so a repaint is only asked for when it would differ.
    let mut shown_second = f64::NAN;
    let mut shown_duration = false;
    // Progress is queued once per episode. The tracker emits several completed rows — crossing
    // the threshold, then pausing, then ending — and the outbox would coalesce duplicates
    // anyway, but queueing once keeps the badge honest.
    let mut committed = false;

    // Whether mpv has ever reported a position. A successful spawn is not playback: mpv can
    // connect its IPC socket, accept the URL and then sit there forever with no data — which is
    // exactly what a stalled torrent or a broken local mpv looks like, and it produced no error
    // of any kind because nothing was watching for the *absence* of events.
    let mut ever_played = false;

    loop {
        // Controls and events are interleaved rather than polled in turn: a keystroke must not
        // wait on the next position tick, and a position tick must not wait on a keystroke.
        let event = tokio::select! {
            event = events.recv() => match event {
                Some(event) => event,
                // The channel closing means mpv exited.
                None => break,
            },
            // Only armed until the first frame arrives, so this cannot fire mid-episode during a
            // legitimate pause — mpv keeps reporting `time-pos` while paused.
            _ = tokio::time::sleep(FIRST_FRAME_TIMEOUT), if !ever_played => {
                let _ = tx.send(Update::Toast(Toast::alert(format!(
                    "mpv has not started playing after {}s — the source may have no data, or check `mpv` plays a file on its own",
                    FIRST_FRAME_TIMEOUT.as_secs()
                ))));
                // Deliberately not a stop. mpv may still be buffering a slow torrent, and killing
                // it would turn a wait into a failure; the user now knows and can press `x`.
                ever_played = true;
                continue;
            }
            command = commands.recv(), if controls_open => {
                match command {
                    Some(command) => {
                        apply_command(&session, command).await;
                        continue;
                    }
                    // The UI is going away. mpv keeps playing until it ends on its own.
                    None => {
                        controls_open = false;
                        continue;
                    }
                }
            }
        };

        // Mirror position to the UI so Now Playing can render without querying mpv — but only
        // when the rendered value would actually change. mpv reports `time-pos` about thirty
        // times a second (measured), and every update redraws the terminal, so forwarding them
        // all would mean thirty full repaints per second to move a clock that ticks once.
        // In-player keys arrive as events and go straight to the reducer — the session
        // itself ends when the reducer starts the next episode's playback.
        if let PlaybackEvent::Remote(command) = &event {
            let delta = match command {
                anistream_player::RemoteCommand::NextEpisode => 1,
                anistream_player::RemoteCommand::PreviousEpisode => -1,
            };
            let _ = tx.send(Update::PlayerStepEpisode(delta));
        }

        if let PlaybackEvent::Progress { position, duration } = &event {
            ever_played = true;
            let whole = position.floor();
            if whole != shown_second || duration.is_some() != shown_duration {
                shown_second = whole;
                shown_duration = duration.is_some();
                let _ = tx.send(Update::Playback {
                    position: *position,
                    duration: *duration,
                    paused: tracker.is_paused(),
                });
                // Rides the same whole-second throttle. Discord rate-limits presence updates, and
                // mpv reports position about thirty times a second — sending each one would be
                // throttled away by Discord anyway, and one update a second is more than a presence
                // line needs.
                presence.update(tracker.is_paused()).await;
            }
        }

        for action in tracker.observe(&event) {
            match action {
                Action::Record { position, duration, completed } => {
                    let watch = WatchEvent {
                        duration_secs: duration,
                        watched_secs: tracker.watched_secs(),
                        provider_id: Some(stream.provider_id.clone()),
                        translation: Some(context.translation),
                        completed,
                        ..WatchEvent::new(context.anilist_id, &context.episode, position)
                    };
                    // Local first, always. The history row is written before anything is queued
                    // for a tracker, so a failed push can never cost you the watch.
                    //
                    // Blocking sqlite calls: off the async worker so a slow disk cannot stall
                    // the event stream.
                    let queue_now = completed && !committed;
                    if queue_now {
                        committed = true;
                    }
                    let store_handle = store.clone();
                    let trackers = tracker_ids.clone();
                    let id = context.anilist_id;
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
                        let _ = tx.send(Update::ProgressQueued);
                    }
                }

                Action::OfferSkip { kind, to } => {
                    if tracker.auto_skips() {
                        let _ = session.seek_to(to).await;
                        let _ = session.notify(format!("skipped {}", kind.label())).await;
                    } else {
                        // On mpv's OSD, not the terminal: the viewer is looking at the video.
                        let _ = session
                            .notify(format!("press S to skip the {}", kind.label()))
                            .await;
                    }
                    let _ = tx.send(Update::SkipAvailable { label: kind.label(), to });
                }

                Action::ClearSkip => {
                    let _ = tx.send(Update::SkipCleared);
                }

                Action::RememberSpeed(speed) => {
                    let _ = tx.send(Update::PlaybackSpeed(speed));
                }

                Action::RememberVolume(volume) => {
                    let _ = tx.send(Update::PlaybackVolume(volume));
                }

                Action::Finished { watched } => {
                    let _ = tx.send(Update::PlaybackEnded { watched });
                    if watched {
                        tracing::info!(
                            episode = %context.episode,
                            "episode finished and recorded as watched"
                        );
                    }
                }
            }
        }
    }

    // mpv exited without ever reporting a position. This was the silent failure: the loop simply
    // broke, `Finished` was never emitted because the tracker had seen nothing to finish, and the
    // eyecatch wiped back to the episode table with no message — indistinguishable from a
    // successful playback that ended instantly. mpv almost always explains itself on stderr, so
    // say what it said rather than inventing a guess.
    // Cleared before anything else, so quitting never leaves a stale "watching" up.
    presence.finish().await;

    if !ever_played {
        let reported = session.diagnostics().most_relevant().await;
        let message = match reported {
            Some(line) => format!("mpv could not play this: {line}"),
            None => "mpv exited without playing anything, and said nothing about why".into(),
        };
        tracing::warn!(%message, url = %stream.url, "playback produced no frames");
        let _ = tx.send(Update::Toast(Toast::alert(message)));
        // Leaves Now Playing rather than stranding the user on a control surface for a session
        // that no longer exists.
        let _ = tx.send(Update::PlaybackEnded { watched: false });
    }

    if let Ok(session) = Arc::try_unwrap(session) {
        session.shutdown().await;
    }
    let _ = tx.send(Update::Status(String::new()));
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
        PlayerCommand::Stop => {
            let _ = session.quit().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_title_without_a_mal_id_simply_has_no_skip_data() {
        // The mapping layer cannot resolve every title, and skip data is decoration.
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        assert!(fetch_skips(&http, None, "1").await.is_empty());
    }

    #[tokio::test]
    async fn a_non_numeric_episode_is_not_looked_up() {
        // "OVA" and "S1" are real episode labels; asking aniskip about them is meaningless.
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        assert!(fetch_skips(&http, Some(52_991), "OVA").await.is_empty());
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
            speed: Some(1.25),
            volume: Some(80.0),
        };
        assert_eq!(context.mal_id, Some(52_991));
        assert_eq!(context.resume_at, Some(600.0));
    }
}
