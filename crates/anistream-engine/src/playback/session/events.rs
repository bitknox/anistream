//! One file's playback, from the load to its end: mpv's events and the viewer's controls
//! interleaved, fed to the tracker and the track policy, with the first frame, a stall and
//! the picture watched as it plays.

use std::time::Duration;

use anistream_core::stream::Stream;
use anistream_model::{
    Msg,
    command::PlayerCommand,
    event::Event,
    hosted::TrackKind as TrackChoice,
    notice::{Notice, NoticeHint},
};
use anistream_playback::{
    Action, Choice, PlaybackEvent, PlaybackTracker, ScriptEvent, SkipInterval, SkipKind,
    TrackInput,
};
use anistream_store::ServerFailure;
use tokio::sync::mpsc;

use super::{
    FileOutcome, Run,
    companion_state::fetch_still,
    controls::{aimed, apply_command},
    presence::{Playhead, Rpc},
    skips::{offered, skip_target},
    stats::{Playing, StatsWatch, sampled},
    subtitles::LateSubtitles,
    upscaling::read_source,
};
use crate::playback::{
    link::{LinkError, Overlay, PlayerLink},
    seat::{SeatCue, SeatReport},
    shaders::{self, Look},
    slot::Player,
};

/// How long mpv may go without reporting a position before we say something.
///
/// Generous, because a torrent legitimately takes a while to find peers and fill a buffer — the
/// measured happy path is half a second, so twenty is far past "slow" and firmly in "wrong".
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a mid-episode stall lasts before the player offers another stream.
const STALL_PROMPT: Duration = Duration::from_secs(8);

/// How often the battery is asked about during playback.
const BATTERY_POLL: Duration = Duration::from_secs(60);

/// How often a party hears where the player is while it plays. A pause, a resume or a new speed
/// is said at once.
const SEAT_REPORTS: Duration = Duration::from_millis(200);

impl Run<'_> {
    /// Play one file to its end, feeding the tracker and answering the viewer.
    pub(super) async fn file(
        &mut self,
        player: &mut Player,
        stream: &Stream,
        streams: &[Stream],
        index: usize,
        commands: &mut mpsc::UnboundedReceiver<PlayerCommand>,
        // Subtitle tracks still being fetched as the file opened, added as each arrives.
        mut late: Option<LateSubtitles>,
    ) -> FileOutcome {
        let context = self.context;
        let settings = self.settings;
        let tx = self.tx;
        let link = player.link.as_ref();
        // The file is loading: the time to its first frame is counted from here.
        self.servers.begin();

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
        // The seek bar's marks as last said, and the pace for what the player holds.
        let mut marked: Option<Vec<SkipInterval>> = None;
        let mut buffered = super::timeline::Buffered::default();
        let clock = tokio::time::Instant::now();
        let mut picture_tick = tokio::time::interval(Duration::from_secs(1));
        // The governor keeps its steps across episodes in this window, but its evidence is
        // per file.
        player.picture.governor.new_file();
        self.hdr = None;

        // The watch party's seat, when there is a party: this file's while it plays, taken at
        // the start or the moment a party opens.
        let mut seat = self.seat.take().or_else(|| settings.party.seat());
        let mut parties = settings.party.changes();
        parties.borrow_and_update();
        let mut parties_open = true;
        self.in_party = seat.is_some();
        if let Some(seat) = &seat {
            seat.report(self.seat_playing(None));
        }
        let mut seat_sent: Option<tokio::time::Instant> = None;
        let mut seat_length = false;

        // Whether the file has loaded, which a track has to wait for before it can be added.
        let mut loaded = false;
        let mut waiting: Vec<anistream_core::stream::Subtitle> = Vec::new();
        // Stats for nerds, open as the window left it.
        let mut stats = StatsWatch::new(player.stats);
        // What the stats are made from beside the player's figures, as it stands now.
        macro_rules! playing {
            () => {
                Playing {
                    picture: &player.picture,
                    source: source.as_ref(),
                    stream,
                    streams,
                    index,
                    paused: tracker.is_paused(),
                    buffering,
                }
            };
        }

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
                        quit: link.has_quit(),
                        no_frames: !ever_played,
                    },
                },
                // Only armed until the first frame arrives, so this cannot fire mid-episode
                // during a legitimate pause — mpv keeps reporting `time-pos` while paused.
                _ = tokio::time::sleep(FIRST_FRAME_TIMEOUT), if !ever_played && !warned => {
                    // How to stop it is the front end's to say: the terminal's `x`.
                    let notice = Notice::alert(Msg::NoticePlayerNotPlaying {
                        seconds: FIRST_FRAME_TIMEOUT.as_secs() as i64,
                    });
                    let _ = tx.send(Event::Toast(notice.with_hint(NoticeHint::StopPlayback)));
                    // Deliberately not a stop. mpv may still be buffering a slow torrent, and
                    // killing it would turn a wait into a failure; the user now knows and can
                    // stop it.
                    warned = true;
                    self.servers.late();
                    continue;
                }
                fetched = async { still_fetch.as_mut().expect("guarded").await },
                    if still_fetch.is_some() =>
                {
                    still_fetch = None;
                    self.still = fetched.ok().flatten();
                    if let Some(next) = self.up_next() {
                        let _ = link.overlay(next).await;
                    }
                    continue;
                }
                _ = picture_tick.tick() => {
                    // What the player holds and the pace kept back, now due: when nothing
                    // more arrives, as once the whole file is held.
                    let now = clock.elapsed().as_secs_f64();
                    if let Some(ranges) = buffered.due(now, tracker.is_paused()) {
                        let _ = link.overlay(Overlay::Buffered(ranges)).await;
                    }
                    let picture = &mut player.picture;
                    let mut changed = false;
                    // Battery about once a minute: cheap, but a subprocess on macOS.
                    if picture.battery_checked.is_none_or(|t| t.elapsed() >= BATTERY_POLL) {
                        picture.battery_checked = Some(tokio::time::Instant::now());
                        let power = std::sync::Arc::clone(&self.power);
                        let on = tokio::task::spawn_blocking(move || power.on_battery())
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
                        self.apply_picture(&mut player.picture, link, source.as_ref(), streams, index, true).await;
                    }
                    // The TV's summary, said as it changes; the open panel says it with its rows.
                    if !stats.is_open() {
                        let said = self.stats(&stats, &playing!(), false);
                        if stats.summary_changed(&said) {
                            let _ = link.overlay(Overlay::Stats(said)).await;
                        }
                    }
                    continue;
                }
                // The open panel: the player asked for its figures, then the rows said.
                _ = tokio::time::sleep_until(stats.due().unwrap_or_else(tokio::time::Instant::now)),
                    if stats.is_open() =>
                {
                    if stats.turn() {
                        let _ = link.sample(sampled()).await;
                    } else {
                        let said = self.stats(&stats, &playing!(), true);
                        stats.summary_changed(&said);
                        let _ = link.overlay(Overlay::Stats(said)).await;
                    }
                    continue;
                }
                _ = stall_tick.tick(), if stalled_since.is_some() => {
                    let held = stalled_since.map(|t| t.elapsed()).unwrap_or_default();
                    if held >= STALL_PROMPT && !stall_dismissed {
                        let _ = link.overlay(Overlay::Stall { seconds: held.as_secs() }).await;
                        stall_shown = true;
                    }
                    continue;
                }
                // A subtitle track that arrived after the file opened: given to the player now,
                // or as the file loads, which it has to be loaded for.
                subtitle = async {
                    match late.as_mut() {
                        Some(late) => late.next().await,
                        None => None,
                    }
                }, if late.is_some() => {
                    match subtitle {
                        Some(subtitle) if loaded => add_subtitle(link, &subtitle).await,
                        Some(subtitle) => waiting.push(subtitle),
                        None => late = None,
                    }
                    continue;
                }
                cue = async { seat.as_mut().expect("guarded").cue().await }, if seat.is_some() => {
                    match cue {
                        Some(cue) => apply_cue(link, cue).await,
                        // The party is over: this plays on as a private session, from here.
                        None => {
                            seat = None;
                            self.in_party = false;
                        }
                    }
                    continue;
                }
                opened = parties.changed(), if seat.is_none() && parties_open => {
                    if opened.is_err() {
                        parties_open = false;
                        continue;
                    }
                    seat = settings.party.seat();
                    self.in_party = seat.is_some();
                    if let Some(seat) = &seat {
                        seat.report(self.seat_playing(tracker.duration()));
                        seat_length = tracker.duration().is_some();
                        if ever_played {
                            seat.report(SeatReport::Sample {
                                position: tracker.position(),
                                paused: tracker.is_paused(),
                                speed: tracker.speed(),
                            });
                        }
                    }
                    continue;
                }
                command = commands.recv(), if self.controls_open => {
                    match command {
                        Some(PlayerCommand::Handover(successor)) => {
                            // Flush what was watched before the window changes hands.
                            self.record_actions(&mut tracker, &PlaybackEvent::Ended { complete: false }, stream, &mut committed, &mut ending).await;
                            let _ = tx.send(Event::EpisodeLeft {
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
                            let _ = link.overlay(Overlay::Episode(info)).await;
                            if let Some(next) = self.up_next() {
                                let _ = link.overlay(next).await;
                            }
                            let menus = self.menus(streams, index);
                            let _ = link.overlay(Overlay::Menus(menus)).await;
                            continue;
                        }
                        Some(PlayerCommand::Step(delta)) => {
                            self.step(delta, link, true).await;
                            continue;
                        }
                        // A pick in a hosted player's own menus: the engine selects it and
                        // keeps it, rather than waiting to read it back.
                        Some(PlayerCommand::ChooseTrack { kind, id }) => {
                            let kind = match kind {
                                TrackChoice::Audio => anistream_playback::tracks::TrackKind::Audio,
                                TrackChoice::Subtitle => anistream_playback::tracks::TrackKind::Sub,
                            };
                            for action in player.tracks.viewer_chose(kind, id) {
                                self.apply_track(action, link).await;
                            }
                            continue;
                        }
                        Some(PlayerCommand::SubtitleDelay { seconds }) => {
                            let now = std::time::Instant::now();
                            for action in player.tracks.viewer_delay(seconds, now) {
                                self.apply_track(action, link).await;
                            }
                            continue;
                        }
                        Some(PlayerCommand::Stats(open)) => {
                            player.stats = open;
                            stats.open(open);
                            if !open {
                                let said = self.stats(&stats, &playing!(), false);
                                let _ = link.overlay(Overlay::Stats(said)).await;
                            }
                            continue;
                        }
                        // Read as the panel stands, so the copy is what the viewer sees.
                        Some(PlayerCommand::CopyStats { about }) => {
                            let said = self.stats(&stats, &playing!(), true);
                            let text = crate::playback::stats::copy_text(&said.rows, &about);
                            let _ = tx.send(Event::Clipboard { text });
                            continue;
                        }
                        // The server this is read from was bound again: the same stream, opened
                        // again where it plays, as a switch in place would.
                        Some(PlayerCommand::Reload) => {
                            break FileOutcome::Switch { to: index, at: tracker.position() };
                        }
                        // What the viewer did in a hosted player's own interface: read as the
                        // companion's word for the same is.
                        Some(command) => match from_the_player(command) {
                            Ok(script) => PlaybackEvent::Script(script),
                            Err(command) => {
                                stopped |= matches!(command, PlayerCommand::Stop);
                                let position = tracker.position();
                                if let Some(command) =
                                    aimed(command, loaded, position, tracker.duration())
                                {
                                    apply_command(link, command).await;
                                }
                                continue;
                            }
                        },
                        // The UI is going away. mpv keeps playing until it ends on its own.
                        None => {
                            self.controls_open = false;
                            continue;
                        }
                    }
                }
            };

            // The stats take their figures from what the player says; what is read for them
            // alone goes no further.
            stats.observe(&event);
            if matches!(event, PlaybackEvent::Sampled { .. }) {
                continue;
            }

            // Tracks first, and for every event: the policy reads the clock off each one.
            let track_input = match &event {
                PlaybackEvent::TrackList(list) => Some(TrackInput::TrackList(
                    anistream_playback::tracks::parse_track_list(list),
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
                    self.apply_track(action, link).await;
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
                            link,
                            source.as_ref(),
                            streams,
                            index,
                            false,
                        )
                        .await;
                    }
                    continue;
                }
                PlaybackEvent::Display { width, height, compute, .. } => {
                    player.picture.display = Some(shaders::Display {
                        width: *width,
                        height: *height,
                        compute: *compute,
                    });
                    self.apply_picture(
                        &mut player.picture,
                        link,
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
                // The renderer gave up on a hook of the chain: the chain does not run as chosen,
                // so it steps down as the governor steps it, once for the chain that failed. A
                // hook of a chain that has been replaced since is late news, and changes nothing;
                // one no shader here describes cannot be placed in a chain, and eases only once.
                PlaybackEvent::HookDisabled(hook) => {
                    let picture = &mut player.picture;
                    let running = |file: &str| {
                        picture.applied.iter().any(|path| {
                            std::path::Path::new(path).file_name().is_some_and(|n| n == file)
                        })
                    };
                    let ours = match shaders::hook_file(hook) {
                        Some(file) => running(file),
                        None => !picture.applied.is_empty() && picture.failed.is_none(),
                    };
                    let failed = ours && picture.failed.as_ref() != Some(&picture.applied);
                    if failed && picture.governor.step_down().is_some() {
                        tracing::warn!(%hook, "the renderer disabled a shader hook; easing");
                        picture.failed = Some(picture.applied.clone());
                        self.apply_picture(
                            &mut player.picture,
                            link,
                            source.as_ref(),
                            streams,
                            index,
                            true,
                        )
                        .await;
                    }
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
                        player.picture.failed = None;
                        self.apply_picture(
                            &mut player.picture,
                            link,
                            source.as_ref(),
                            streams,
                            index,
                            false,
                        )
                        .await;
                        let _ = tx.send(Event::PlayerUpscaling(id.clone()));
                    }
                    continue;
                }
                PlaybackEvent::Script(ScriptEvent::Choose(Choice::ToneMapping(id))) => {
                    let _ = link.set("tone-mapping", id.clone().into()).await;
                    self.tone_mapping = id.clone();
                    let _ = link.overlay(Overlay::Menus(self.menus(streams, index))).await;
                    let _ = tx.send(Event::PlayerToneMapping(id.clone()));
                    continue;
                }
                PlaybackEvent::Script(script) => {
                    if let Some(outcome) = self
                        .answer(script, link, &mut tracker, &mut last_skip, streams, index)
                        .await
                    {
                        break outcome;
                    }
                    match script {
                        ScriptEvent::StallWait => {
                            stall_dismissed = true;
                            let _ = link.overlay(Overlay::StallCleared).await;
                        }
                        ScriptEvent::StallSwitch if streams.len() > 1 => {
                            let stalled = ServerFailure::Stalled;
                            self.servers.failed(self.store, &streams[index], stalled);
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
                    // A cast took them with the file: its tracks are fixed at the load.
                    if !link.subtitles_with_load()
                        && let Err(e) =
                            add_subtitles(link, stream, settings.subtitle_language.as_deref())
                                .await
                    {
                        tracing::debug!(error = %e, "could not attach external subtitles");
                    }
                    loaded = true;
                    for subtitle in waiting.drain(..) {
                        add_subtitle(link, &subtitle).await;
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
                            let _ = link.overlay(Overlay::StallCleared).await;
                        }
                    }
                }
                PlaybackEvent::Cached(ranges) => {
                    let now = clock.elapsed().as_secs_f64();
                    if let Some(ranges) =
                        buffered.offer(ranges.clone(), now, tracker.is_paused())
                    {
                        let _ = link.overlay(Overlay::Buffered(ranges)).await;
                    }
                    continue;
                }
                // Our own `loadfile` ending the file it replaced; nothing to record.
                PlaybackEvent::Replaced => continue,
                PlaybackEvent::Progress { position, duration } => {
                    self.maybe_prefetch(*position, *duration);
                    if !ever_played {
                        ever_played = true;
                        let _ = link.overlay(Overlay::Playing).await;
                        self.servers.served(self.store, &streams[index]);
                    }
                    // Mirror position to the UI so Now Playing can render without querying mpv —
                    // but only when the rendered value would actually change. mpv reports
                    // `time-pos` about thirty times a second (measured), and every update redraws
                    // the terminal.
                    let whole = position.floor();
                    if whole != shown_second || duration.is_some() != shown_duration {
                        shown_second = whole;
                        shown_duration = duration.is_some();
                        let _ = tx.send(Event::Playback {
                            position: *position,
                            duration: *duration,
                            paused: tracker.is_paused(),
                        });
                        // Rides the same whole-second throttle, and the presence paces itself
                        // beyond it: Discord rate-limits updates, and a bar that is still right
                        // needs none.
                        rpc.observe(Playhead {
                            position: *position,
                            duration: *duration,
                            speed: tracker.speed(),
                            paused: tracker.is_paused(),
                            buffering,
                        })
                        .await;
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
                            let _ = link.seek(to, true).await;
                            let _ = link.overlay(Overlay::Skipped { kind, from, to }).await;
                            last_skip = Some(kind);
                        } else if settings.skip_button {
                            // On the video, not the terminal: the viewer is looking at it.
                            let _ = link.overlay(Overlay::SkipOffer { kind, to }).await;
                        } else {
                            // No button wanted, and no skip on its own: nothing to say.
                            continue;
                        }
                        let _ = tx.send(Event::SkipAvailable { kind: offered(kind), to });
                    }
                    Action::ClearSkip => {
                        let _ = link.overlay(Overlay::SkipCleared).await;
                        let _ = tx.send(Event::SkipCleared);
                    }
                    other => {
                        let watched_secs = tracker.watched_secs();
                        self.act(
                            other,
                            watched_secs,
                            link,
                            stream,
                            &mut committed,
                            &mut ending,
                        )
                        .await;
                    }
                }
            }

            // The party hears where the player is: a pause, a resume or a new speed at once, and
            // the position as it plays, a few times a second. Not before the first frame, when
            // the position is only where the player was asked to start.
            if let Some(seat) = &seat
                && ever_played
            {
                if !seat_length && tracker.duration().is_some() {
                    seat_length = true;
                    seat.report(self.seat_playing(tracker.duration()));
                }
                let now = tokio::time::Instant::now();
                let due = match event {
                    PlaybackEvent::Paused(_) | PlaybackEvent::Speed(_) => true,
                    PlaybackEvent::Progress { .. } => {
                        seat_sent.is_none_or(|sent| now.duration_since(sent) >= SEAT_REPORTS)
                    }
                    _ => false,
                };
                if due {
                    seat_sent = Some(now);
                    seat.report(SeatReport::Sample {
                        position: tracker.position(),
                        paused: tracker.is_paused(),
                        speed: tracker.speed(),
                    });
                }
            }

            // A pause, a stall or a new speed is said at once, where the position has not moved
            // to bring the next tick: the tracker has just taken it in.
            if matches!(
                event,
                PlaybackEvent::Paused(_)
                    | PlaybackEvent::Speed(_)
                    | PlaybackEvent::Buffering(_)
            ) {
                rpc.observe(Playhead {
                    position: tracker.position(),
                    duration: tracker.duration(),
                    speed: tracker.speed(),
                    paused: tracker.is_paused(),
                    buffering,
                })
                .await;
            }

            // The file's own chapters arrive once it is playing, and outrank aniskip's times.
            // Up Next and the look-ahead for the next episode both hang off where the ending
            // starts, so they move with it.
            if tracker.skips() != self.skips.as_slice() {
                self.skips = tracker.skips().to_vec();
                if let Some(next) = self.up_next() {
                    let _ = link.overlay(next).await;
                }
            }
            // The seek bar marks the same segments: said for the file once, then as they change.
            if marked.as_deref() != Some(tracker.skips()) {
                marked = Some(tracker.skips().to_vec());
                let _ = link.overlay(Overlay::Timeline(tracker.skips().to_vec())).await;
            }
            // Play carrying on says what the player came to hold while paused.
            if matches!(event, PlaybackEvent::Paused(false))
                && let Some(ranges) = buffered.resumed(clock.elapsed().as_secs_f64())
            {
                let _ = link.overlay(Overlay::Buffered(ranges)).await;
            }

            if let PlaybackEvent::Ended { complete } = event
                && file_ended
            {
                // `end-file` for a quit arrives before the socket closes, so the window is
                // already as good as gone when mpv says it is quitting.
                break FileOutcome::Ended {
                    watched: ending,
                    complete,
                    exited: !link.is_alive() || link.has_quit(),
                    stopped,
                    quit: link.has_quit(),
                    no_frames: !ever_played,
                };
            }
        };

        // An unfinished fetch carries over to the next stream this playback tries, and the seat
        // with it.
        self.still_fetch = still_fetch;
        self.seat = seat;
        // Cleared before anything else, so quitting never leaves a stale "watching" up.
        rpc.finish().await;
        let _ = link.overlay(Overlay::SkipCleared).await;
        outcome
    }
}

impl Run<'_> {
    /// What the party is told plays: the episode and its title, from which the party writes the
    /// name the room sees, never the stream's address.
    fn seat_playing(&self, duration: Option<f64>) -> SeatReport {
        let context = self.context;
        SeatReport::Playing {
            id: context.anilist_id,
            episode: context.episode.clone(),
            name: context.title.clone(),
            duration,
        }
    }
}

/// Do what the party asks of the player.
async fn apply_cue(link: &dyn PlayerLink, cue: SeatCue) {
    let done = match cue {
        SeatCue::Pause(paused) => link.set("pause", paused.into()).await,
        SeatCue::Seek(to) => link.seek(to, true).await,
        SeatCue::Speed(speed) => link.set("speed", speed.into()).await,
        SeatCue::Notice(text) => {
            let level = anistream_playback::companion::Level::Info;
            link.overlay(Overlay::Toast { text, level }).await
        }
    };
    if let Err(e) = done {
        tracing::debug!(error = %e, "the player did not take what the party asked");
    }
}

/// What the viewer did in a player's own interface, when `command` says so, as mpv's companion
/// would have said it; any other command back as it was.
fn from_the_player(command: PlayerCommand) -> Result<ScriptEvent, PlayerCommand> {
    use anistream_model::hosted::PlayerChoice;
    Ok(match command {
        PlayerCommand::TakeSkip => ScriptEvent::Skip,
        PlayerCommand::UndoSkip { to } => ScriptEvent::UndoSkip { to },
        PlayerCommand::PlayNext => ScriptEvent::PlayNext,
        PlayerCommand::CancelNext => ScriptEvent::CancelNext,
        PlayerCommand::StallSwitch => ScriptEvent::StallSwitch,
        PlayerCommand::StallWait => ScriptEvent::StallWait,
        PlayerCommand::NightMode(on) => ScriptEvent::Choose(Choice::NightMode(on)),
        PlayerCommand::Upscaling(id) => ScriptEvent::Choose(Choice::Upscaling(id)),
        PlayerCommand::Choose(choice) => ScriptEvent::Choose(match choice {
            PlayerChoice::Episode(number) => Choice::Episode(number),
            PlayerChoice::Source(id) => Choice::Source(id),
            PlayerChoice::Translation(translation) => {
                Choice::Translation(translation.as_str().to_owned())
            }
            PlayerChoice::ToneMapping(id) => Choice::ToneMapping(id),
        }),
        other => return Err(other),
    })
}

/// Attach one external subtitle track that arrived late to the loaded file.
async fn add_subtitle(link: &dyn PlayerLink, subtitle: &anistream_core::stream::Subtitle) {
    let language = (!subtitle.language.is_empty()).then(|| subtitle.language.clone());
    if let Err(e) = link.add_subtitle(&subtitle.url, language.clone(), language).await {
        tracing::debug!(error = %e, "could not attach a late subtitle");
    }
}

/// Attach a stream's external subtitle tracks to the loaded file, the preferred language first:
/// with nothing better to go on, a player selects the first track it is given.
async fn add_subtitles(
    link: &dyn PlayerLink,
    stream: &Stream,
    subtitle_language: Option<&str>,
) -> Result<(), LinkError> {
    for subtitle in
        anistream_playback::protocol::subtitle_order(&stream.subtitles, subtitle_language)
    {
        let language = (!subtitle.language.is_empty()).then(|| subtitle.language.clone());
        link.add_subtitle(&subtitle.url, language.clone(), language).await?;
    }
    Ok(())
}
