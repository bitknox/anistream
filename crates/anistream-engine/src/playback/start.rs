//! Starting a playback: the plays a front end asks for, each with a fresh control channel and the
//! window handed over, the stream found behind the eyecatch, and the settings it runs under.

use std::sync::Arc;

use anistream_core::{config::Config, ids::AnilistId};
use anistream_model::{
    command::PlayerCommand, episodes::EpisodeRow, event::Event, notice::Notice,
    playback::PlaybackId,
};
use tokio::sync::{mpsc, watch};

use crate::{
    cache::{PlayHints, PlayKind},
    jobs::JobGuard,
    outbox::{Outbox, Played},
    playback::{
        self,
        autonext::{Follow, Staged},
        handoff::{Handoff, successor, take_controls},
        link::{PlayerOpener, opener},
        resolve::{Resolved, resolve_for_playback},
        shaders::shader_dir,
        slot::PlayerSlot,
    },
    services::Services,
    show_settings,
    state::EngineState,
    titles::episodes::{EpisodeLoad, load_episodes},
    tracking::commands::signed_in_tracker_ids,
};

/// What a play of `episode` of `id` takes from the engine's record of the titles: the name the
/// window being handed over announces it by, and what the player's Up Next card and Episodes
/// menu show. See [`TitleCache::hints`]. A staged play's name is remembered for the episode
/// that follows it, which goes by the same name.
///
/// [`TitleCache::hints`]: crate::cache::TitleCache::hints
fn hints_for(
    id: AnilistId,
    episode: &str,
    kind: PlayKind,
    skip_filler: bool,
    engine: &mut EngineState,
) -> PlayHints {
    let hints = engine.cache.hints(id, episode, skip_filler, kind);
    if kind == PlayKind::Staged {
        engine.cache.announce(id, hints.title.clone());
    }
    hints
}

/// What this play opens its player with, from the config in effect: see [`opener`].
///
/// No player handle is kept across the session: every play builds its own from the config in
/// effect, which is the live one the Settings screen edits, as the front end last said. A
/// handle held across the session is exactly how a turned-off upscaler kept running.
pub(crate) fn opener_for(
    engine: &EngineState,
    services: &Services,
) -> Option<Arc<dyn PlayerOpener>> {
    let hosted = services.hosted_player.then(|| -> Arc<dyn PlayerOpener> {
        Arc::clone(&services.hosted) as Arc<dyn PlayerOpener>
    });
    opener(engine.config.effective(), &services.paths, &services.player_theme, hosted)
}

/// Whether a play goes to a Syncplay party when nobody asked for one: when the config sends
/// every play there, on the desktop, which is where Syncplay runs.
fn party_by_default(config: &Config) -> bool {
    cfg!(feature = "desktop") && config.syncplay.enabled
}

/// Play an episode from whichever source the resolution ladder chooses, staged for Now Playing
/// as the engine's record of the titles names it, which the front end is told. Refused, with
/// nothing staged, where this build has no player to open.
pub fn play(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    let billing = named(id, &episode, engine);
    let staged = play_staged(id, episode, None, billing, opener, job, engine, services, tx);
    let _ = tx.send(staged);
}

/// Play the release the user picked in the Sources overlay.
#[allow(clippy::too_many_arguments)]
pub fn play_source(
    id: AnilistId,
    episode: String,
    provider_id: String,
    source_id: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    let billing = named(id, &episode, engine);
    let pick = Some((provider_id, source_id));
    let staged = play_staged(id, episode, pick, billing, opener, job, engine, services, tx);
    let _ = tx.send(staged);
}

/// Play what a playback led to, which the engine starts on its own, in `opener`'s player: staged
/// as `follow` says, under the name the playback before it went by. Returns what it was staged
/// as, for the engine to say right after the event that led to it.
pub(crate) fn play_following(
    follow: Follow,
    opener: Arc<dyn PlayerOpener>,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Event {
    let Follow { id, episode, title, episode_title } = follow;
    let billing = (title, episode_title);
    play_staged(id, episode, None, billing, opener, job, engine, services, tx)
}

/// What a play the front end asked for is staged as: the name the engine's record of the titles
/// gives it, and the episode's own name from the table it holds when that is this title's.
fn named(id: AnilistId, episode: &str, engine: &EngineState) -> (String, Option<String>) {
    (engine.cache.name_for(id, PlayKind::Staged), engine.cache.episode_title(id, episode))
}

/// Stage a playback for Now Playing as `title`, with `episode_title`: the name the episode
/// after it goes by too, and the engine's record of what Now Playing shows. Returns what the
/// front end is told of it, for the caller to say in its place.
fn stage(
    played: &Played,
    title: String,
    episode_title: Option<String>,
    engine: &mut EngineState,
) -> Event {
    engine.cache.announce(played.id, title.clone());
    engine.staging.staged(Staged {
        serial: played.serial,
        id: played.id,
        episode: played.episode.clone(),
        title: title.clone(),
        episode_title: episode_title.clone(),
    });
    Event::PlaybackStaged {
        id: played.id,
        title,
        episode: played.episode.clone(),
        episode_title,
        // What the player starts at when it remembers a speed, and Now Playing until it says.
        speed: engine.config.effective().playback.persisted_speed.unwrap_or(1.0),
    }
}

/// Play an episode staged for Now Playing as `billing` says, its title and its episode's own
/// name, from the release `pick` names or whichever the resolution ladder chooses. Returns what
/// it was staged as.
#[allow(clippy::too_many_arguments)]
fn play_staged(
    id: AnilistId,
    episode: String,
    // `(provider_id, source_id)` when the user picked an exact release in the Sources overlay.
    pick: Option<(String, String)>,
    (title, episode_title): (String, Option<String>),
    opener: Arc<dyn PlayerOpener>,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Event {
    let Services { paths, sync, prefetch, store, .. } = services;
    let played = engine.next_playback(id, &episode);
    let staged = stage(&played, title, episode_title, engine);
    // What the show sets for itself, over the config in effect: sub or dub, quality, what is
    // skipped.
    let for_show = show_settings::for_show(engine.config.effective(), store, id);
    let hints =
        hints_for(id, &episode, PlayKind::Staged, for_show.playback.skip_filler, engine);
    let EngineState { player_tx, finding, .. } = engine;
    let config = &for_show;
    // A fresh channel per playback. The one playing gives up its window only once
    // this one has something to play — see `Handoff`.
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, &hints, config));
    let stopped = watch_for_stop(finding);
    // Never from the streams found ahead for a release picked by hand: the pick names its
    // own. Otherwise the same fresh channel, the same session-replacement semantics.
    let prefetched = match pick {
        None => prefetch.take(id, &episode, config.playback.translation),
        Some(_) => None,
    };
    // Tracker ids rather than trackers: playback only needs to know which queues to
    // append to, and passing the set would couple the player to the sync layer.
    let tracker_ids = signed_in_tracker_ids(sync);
    if hints.episodes.is_empty() {
        fill_episodes(id, &episode, services, player_tx);
    }
    spawn_playback(
        id,
        episode,
        pick,
        party_by_default(config),
        prx,
        stopped,
        handoff,
        config,
        services,
        opener,
        tracker_ids,
        hints,
        shader_dir(config, paths),
        prefetched,
        job,
        &tx.for_playback(played),
    );
    staged
}

/// Play an episode for a Syncplay party rather than in a session of its own.
pub fn play_party(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    // Syncplay is a desktop program: without it there is nothing to hand a party to.
    if !cfg!(feature = "desktop") {
        let _ = tx.send(Event::Toast(Notice::alert(playback::NO_PARTY)));
        drop(job);
        return;
    }
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    let Services { paths, sync, store, .. } = services;
    let played = engine.next_playback(id, &episode);
    let for_show = show_settings::for_show(engine.config.effective(), store, id);
    let hints = hints_for(id, &episode, PlayKind::Party, for_show.playback.skip_filler, engine);
    let EngineState { player_tx, finding, .. } = engine;
    let config = &for_show;
    // Same resolution as Play; the handoff at the end is the only difference.
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, &hints, config));
    let stopped = watch_for_stop(finding);
    let prefetched = None;
    let tracker_ids = signed_in_tracker_ids(sync);
    spawn_playback(
        id,
        episode,
        None,
        true,
        prx,
        stopped,
        handoff,
        config,
        services,
        opener,
        tracker_ids,
        hints,
        shader_dir(config, paths),
        prefetched,
        job,
        &tx.for_playback(played),
    );
}

/// Play a downloaded episode from disk, in the full playback session all the same.
#[allow(clippy::too_many_arguments)]
pub fn play_local(
    id: AnilistId,
    episode: String,
    title: String,
    path: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some(opener) = opener_for(engine, services) else {
        return playback::refuse(job, tx);
    };
    let Services { paths, http, store, sync, .. } = services;
    let played = engine.next_playback(id, &episode);
    let for_show = show_settings::for_show(engine.config.effective(), store, id);
    let hints =
        engine.cache.hints(id, &episode, for_show.playback.skip_filler, PlayKind::Staged);
    // Named by the download it plays: the Downloads screen says what its rows are called.
    let staged =
        stage(&played, title.clone(), engine.cache.episode_title(id, &episode), engine);
    let _ = tx.send(staged);
    let EngineState { player_tx, finding, .. } = engine;
    // Nothing to find: it opens at once, and a Stop is its session's from the start.
    *finding = None;
    let config = &for_show;
    // No provider walk and no torrent session — the file is on disk — but the full
    // playback session all the same: a downloaded episode records history, resumes,
    // and syncs exactly like a streamed one. It used to go straight to mpv with no
    // context, which made downloads second-class in every way that matters.
    // Nothing to look up, so nothing can fail before it plays.
    // Named by the download, as it is staged.
    let hints = PlayHints { title: title.clone(), ..hints };
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, &hints, config));
    let PlayHints { next, episodes, cover_url, banner_url, .. } = hints;
    handoff.now();
    let tracker_ids = signed_in_tracker_ids(sync);
    if episodes.is_empty() {
        fill_episodes(id, &episode, services, player_tx);
    }
    let shaders = shader_dir(config, paths);
    let (config, store, http, machine, tx) = (
        config.clone(),
        store.clone(),
        http.clone(),
        machine(opener, services),
        tx.for_playback(played),
    );
    tokio::spawn(async move {
        let _job = job;
        let playback = config.playback.clone();
        let stream =
            anistream_core::stream::Stream::new(path, anistream_core::stream::StreamKind::Mp4);
        // Best-effort, exactly as the streaming path treats them.
        let resume_at = store
            .resume_position(id, &episode)
            .inspect_err(|e| tracing::warn!(error = %e, "could not read resume position"))
            .unwrap_or(None);
        let mal_id = store.mapping_for(id).ok().flatten().and_then(|m| m.mal_id);
        let context = playback::PlaybackContext {
            anilist_id: id,
            mal_id,
            episode,
            title,
            translation: playback.translation,
            resume_at,
            // A local file's real length is knowable, but the hint only serves
            // the aniskip fallback and a lie would filter its answers.
            runtime_secs: None,
            speed: playback.persist_speed.then_some(playback.persisted_speed).flatten(),
            volume: playback.persist_volume.then_some(playback.persisted_volume).flatten(),
            next,
            episodes,
            cover_url,
            banner_url,
        };
        let settings = playback::PlaySettings {
            shader_dir: shaders,
            ..play_settings(&config, tracker_ids, party_by_default(&config))
        };
        playback::play(vec![stream], context, store, http, machine, settings, tx, prx).await;
    });
}

/// What a playback runs on: `opener`'s player, and the engine's slot, host and power source.
fn machine(opener: Arc<dyn PlayerOpener>, services: &Services) -> playback::Machine {
    playback::Machine {
        opener,
        slot: services.slot.clone(),
        host: services.host.clone(),
        power: services.power.clone(),
    }
}

/// Steer whatever is playing, through the controls of the playback started last.
///
/// A Stop is that playback's whether or not it has opened yet: one still finding its stream
/// reads its controls only once it plays, so it is told at once as well, and opens nothing.
pub fn control_player(command: PlayerCommand, engine: &EngineState) {
    let EngineState { player_tx, finding, .. } = engine;
    if command == PlayerCommand::Stop
        && let Some(finding) = finding
    {
        finding.send_replace(true);
    }
    if let Some(sender) = player_tx.as_ref() {
        let _ = sender.send(command);
    }
}

/// Steer `playback`, when it is the playback started last, which holds the controls. A command
/// aimed at one another has followed since is let go: a Play Now or an Undo that a hosted
/// player sent as its episode rolled on to the next was meant for the episode it showed.
///
/// But for a Stop while the playback started last is still finding its stream. The viewer
/// stopped what was on screen, which was that one's starting screen, or the episode it is
/// about to take the window from: the player it would open is what they turned down.
pub fn control_playback(playback: PlaybackId, command: PlayerCommand, engine: &EngineState) {
    let current = PlaybackId::new(engine.playbacks);
    let finding = engine.finding.as_ref().is_some_and(|finding| !finding.is_closed());
    if current != playback && !(command == PlayerCommand::Stop && playback < current && finding)
    {
        tracing::debug!(
            playback = playback.get(),
            current = engine.playbacks,
            ?command,
            "a command for a playback that is no longer current"
        );
        return;
    }
    if current != playback {
        tracing::info!(
            playback = playback.get(),
            current = engine.playbacks,
            "a stop for the playback before, while this one finds its stream: stopping it"
        );
    }
    control_player(command, engine);
}

/// A fresh watch for a Stop that reaches the playback starting now before it opens, kept as the
/// engine's `finding` in place of the one before it. See [`control_player`].
fn watch_for_stop(finding: &mut Option<watch::Sender<bool>>) -> watch::Receiver<bool> {
    let (stop, stopped) = watch::channel(false);
    *finding = Some(stop);
    stopped
}

/// Until a Stop reaches the playback `stopped` watches for; never, once nothing can send one,
/// a newer playback having taken the controls.
async fn stop_asked(stopped: &mut watch::Receiver<bool>) {
    if stopped.wait_for(|stop| *stop).await.is_err() {
        std::future::pending::<()>().await;
    }
}

/// A playback stopped before it opened a player: it says it ended, stopped, and nothing opens.
/// A window parked for it would wait for nothing.
fn stopped_unopened(slot: &PlayerSlot, tx: &Outbox) {
    tracing::info!("stopped while finding its stream; no player opens for it");
    slot.release();
    let _ = tx.send(Event::playback_ended(false, anistream_model::playback::Ending::Stopped));
    // The playback this one took over left its "playing" line up for it to replace.
    let _ = tx.send(Event::Status(String::new()));
}

/// Resolve a stream and start playing it.
///
/// Everything slow happens inside the spawned task, behind the eyecatch: matching the title,
/// walking the provider chain, and reading back where you left off.
#[allow(clippy::too_many_arguments)]
fn spawn_playback(
    id: anistream_core::ids::AnilistId,
    episode: String,
    // `(provider_id, source_id)` when the user picked an exact release in the Sources
    // overlay; `None` lets the resolution ladder choose.
    pick: Option<(String, String)>,
    // Hand the resolved stream to a Syncplay party instead of a private session.
    party: bool,
    commands: mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>,
    // Whether a Stop reached this playback before it opened: see `control_player`.
    mut stopped: watch::Receiver<bool>,
    handoff: Handoff,
    config: &Config,
    services: &Services,
    opener: Arc<dyn PlayerOpener>,
    tracker_ids: Vec<String>,
    // What the player's Up Next card, its Episodes menu and its starting screen show, from the
    // engine's record of the titles.
    hints: PlayHints,
    shader_dir: Option<std::path::PathBuf>,
    // This episode's streams, found ahead while the previous one's ending played.
    prefetched: Option<Resolved>,
    job: Option<JobGuard>,
    tx: &Outbox,
) {
    let (config, meta, store, registry, http, machine, slot, tx) = (
        config.clone(),
        services.meta.clone(),
        services.store.clone(),
        services.registry.clone(),
        services.http.clone(),
        machine(opener, services),
        services.slot.clone(),
        tx.clone(),
    );

    tokio::spawn(async move {
        let playback = config.playback.clone();
        let resolving = async {
            match prefetched {
                Some(found) => {
                    tracing::info!(episode = %episode, "playing from the streams found ahead");
                    Ok(found)
                }
                None => {
                    resolve_for_playback(
                        &meta,
                        &store,
                        &registry,
                        id,
                        &episode,
                        pick,
                        &playback,
                        job.as_ref(),
                        &tx,
                    )
                    .await
                }
            }
        };
        // Stopped while it looks: the viewer closed its starting screen, or stopped the episode
        // it was to take the window from. Read in the controls, it would open the player first.
        let resolved = tokio::select! {
            biased;
            () = stop_asked(&mut stopped) => None,
            resolved = resolving => Some(resolved),
        };
        let Some(resolved) = resolved else {
            // The episode it was to take over stops with it: what the viewer stopped.
            handoff.stop();
            stopped_unopened(&slot, &tx);
            return;
        };
        let context = match resolved {
            Ok(pair) => pair,
            // The episode being watched carries on, and the window with it: only the lookup
            // failed, and closing what was playing over it would punish the viewer for asking.
            Err(reason) if handoff.live() => {
                let _ = tx.send(Event::Toast(Notice::alert(reason.clone())));
                let _ = tx.send(Event::PlaybackKept);
                handoff.keep(commands, reason).await;
                return;
            }
            Err(reason) => {
                // The failure ladder's first rung: name the failure rather than showing an
                // empty screen. Releasing the eyecatch is what the alert does for us.
                let _ = tx.send(Event::Toast(Notice::alert(reason)));
                // A window left open for this episode would wait for nothing.
                slot.release();
                let _ = tx.send(Event::playback_ended(
                    false,
                    anistream_model::playback::Ending::Failed,
                ));
                // The playback this one took over left its "playing" line up for it to replace.
                let _ = tx.send(Event::Status(String::new()));
                return;
            }
        };
        let (streams, mut context) = context;
        let PlayHints { next, episodes, cover_url, banner_url, .. } = hints;
        (context.next, context.episodes) = (next, episodes);
        // The art the title's lookup found, else what its page or a list showed.
        context.cover_url = context.cover_url.or(cover_url);
        context.banner_url = context.banner_url.or(banner_url);
        // Resolved: from here the wait is mpv opening, which Now Playing shows itself, and the
        // session lasts the episode — far too long to hold a "starting" job open for.
        drop(job);
        // Say why this release, so an automatic pick is never a mystery. The active-provider
        // header is sent from playback itself, per attempt, so failover never credits the
        // stream that just failed.
        if let Some(note) = streams.first().and_then(|s| s.pick_note.clone()) {
            let _ = tx.send(Event::Toast(Notice::info(note)));
        }

        let settings =
            playback::PlaySettings { shader_dir, ..play_settings(&config, tracker_ids, party) };
        handoff.complete().await;
        // Stopped while the window was handed over: the one parked for it closes.
        if *stopped.borrow() {
            stopped_unopened(&slot, &tx);
            return;
        }
        // Opening from here: a Stop is the session's, read off its controls.
        drop(stopped);
        playback::play(streams, context, store, http, machine, settings, tx, commands).await;
    });
}

/// What one playback runs under, from the config as it stands now.
pub fn play_settings(
    config: &Config,
    tracker_ids: Vec<String>,
    party: bool,
) -> playback::PlaySettings {
    let playback = &config.playback;
    playback::PlaySettings {
        threshold: playback.commit_threshold,
        auto_skip: playback.skip_opening,
        mark_chapters: playback.mark_chapters,
        skip_button: playback.skip_button,
        fullscreen: playback.fullscreen,
        subtitle_language: Some(playback.subtitle_language.clone()),
        auto_next: playback.auto_next,
        night_mode: playback.night_mode,
        remember_tracks: playback.persist_speed,
        look: playback.upscaling.into(),
        // Filled in by the caller, which knows where the cache is.
        shader_dir: None,
        tone_mapping: playback.tone_mapping.clone(),
        tracker_ids,
        presence: config.presence.clone(),
        syncplay: config.syncplay.clone(),
        party,
        up_next_countdown: playback::UP_NEXT_COUNTDOWN,
        mpv_options: anistream_playback::protocol::option_pairs(&playback.mpv_args),
    }
}

/// Load a title's episode list for a playback the episode table on screen could not describe.
///
/// The table belongs to whichever title is open, so a playback started from Home or Continue
/// had no episode list: the player's Episodes menu was empty and, with no next episode known,
/// there was no Up Next card, no fetching ahead and no keeping the window for the binge.
/// Loaded in the background after playback starts, never in front of it, and handed to the
/// playback through its controls.
fn fill_episodes(
    id: anistream_core::ids::AnilistId,
    episode: &str,
    services: &Services,
    controls: &Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
) {
    let Some(controls) = controls.clone() else { return };
    let (meta, store, registry, episode) = (
        services.meta.clone(),
        services.store.clone(),
        services.registry.clone(),
        episode.to_owned(),
    );
    tokio::spawn(async move {
        match load_episodes(&meta, &store, &registry, id).await {
            Ok(EpisodeLoad::Rows(rows)) if !rows.is_empty() => {
                let next = following(&rows, &episode);
                let _ = controls
                    .send(anistream_model::command::PlayerCommand::Episodes { rows, next });
            }
            Ok(_) => {}
            Err(reason) => tracing::debug!(%reason, "no episode list for the player"),
        }
    });
}

/// The episode after `episode` in a list, when the list has it.
fn following(rows: &[EpisodeRow], episode: &str) -> Option<String> {
    let next = episode.trim().parse::<u32>().ok()?.checked_add(1)?.to_string();
    rows.iter().any(|row| row.number == next).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_following_episode_is_only_offered_when_the_list_has_it() {
        let rows: Vec<EpisodeRow> = ["1", "2", "3"]
            .into_iter()
            .map(|n| EpisodeRow {
                number: n.into(),
                title: None,
                thumbnail: None,
                description: None,
                duration_secs: None,
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: None,
                delayed_from: None,
            })
            .collect();
        assert_eq!(following(&rows, "1").as_deref(), Some("2"));
        assert_eq!(following(&rows, "3"), None, "a finale has no next");
        assert_eq!(following(&rows, "OVA"), None);
    }
}
