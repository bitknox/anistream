//! Starting a playback: the plays a front end asks for, each with a fresh control channel and the
//! window handed over, the stream found behind the eyecatch, and the settings it runs under.

use anistream_core::{config::Config, ids::AnilistId};
use anistream_model::{
    command::PlayerCommand, episodes::EpisodeRow, event::Event, notice::Notice,
};
use tokio::sync::mpsc;

use crate::{
    cache::{PlayHints, PlayKind},
    jobs::JobGuard,
    outbox::{Outbox, Played},
    playback::{
        self,
        autonext::{Follow, Staged},
        handoff::{Handoff, successor, take_controls},
        player::{player, shader_dir},
        resolve::{Resolved, resolve_for_playback},
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

/// Play an episode from whichever source the resolution ladder chooses, staged for Now Playing
/// as the engine's record of the titles names it, which the front end is told.
///
/// No player handle is kept across the session: every play builds its own from the config in
/// effect, which is the live one the Settings screen edits, as the front end last said. A
/// handle held across the session is exactly how a turned-off upscaler kept running.
pub fn play(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let billing = named(id, &episode, engine);
    let staged = play_staged(id, episode, None, billing, job, engine, services, tx);
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
    let billing = named(id, &episode, engine);
    let pick = Some((provider_id, source_id));
    let staged = play_staged(id, episode, pick, billing, job, engine, services, tx);
    let _ = tx.send(staged);
}

/// Play what a playback led to, which the engine starts on its own: staged as `follow` says,
/// under the name the playback before it went by. Returns what it was staged as, for the
/// engine to say right after the event that led to it.
pub(crate) fn play_following(
    follow: Follow,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Event {
    let Follow { id, episode, title, episode_title } = follow;
    let billing = (title, episode_title);
    play_staged(id, episode, None, billing, job, engine, services, tx)
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
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Event {
    let Services { paths, sync, prefetch, player_theme, store, .. } = services;
    let played = engine.next_playback(id, &episode);
    let staged = stage(&played, title, episode_title, engine);
    // What the show sets for itself, over the config in effect: sub or dub, quality, what is
    // skipped.
    let for_show = show_settings::for_show(engine.config.effective(), store, id);
    let skip_filler = for_show.playback.skip_filler;
    let PlayHints { title, next, episodes } =
        hints_for(id, &episode, PlayKind::Staged, skip_filler, engine);
    let EngineState { player_tx, .. } = engine;
    let config = &for_show;
    // A fresh channel per playback. The one playing gives up its window only once
    // this one has something to play — see `Handoff`.
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, title, config));
    // Never from the streams found ahead for a release picked by hand: the pick names its
    // own. Otherwise the same fresh channel, the same session-replacement semantics.
    let prefetched = match pick {
        None => prefetch.take(id, &episode, config.playback.translation),
        Some(_) => None,
    };
    // Tracker ids rather than trackers: playback only needs to know which queues to
    // append to, and passing the set would couple the player to the sync layer.
    let tracker_ids = signed_in_tracker_ids(sync);
    let extras = (next, episodes);
    if extras.1.is_empty() {
        fill_episodes(id, &episode, services, player_tx);
    }
    spawn_playback(
        id,
        episode,
        pick,
        config.syncplay.enabled,
        prx,
        handoff,
        config,
        services,
        &player(config, paths, player_theme),
        tracker_ids,
        extras,
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
    let Services { paths, sync, player_theme, store, .. } = services;
    let played = engine.next_playback(id, &episode);
    let for_show = show_settings::for_show(engine.config.effective(), store, id);
    let skip_filler = for_show.playback.skip_filler;
    let PlayHints { title, next, episodes } =
        hints_for(id, &episode, PlayKind::Party, skip_filler, engine);
    let EngineState { player_tx, .. } = engine;
    let config = &for_show;
    // Same resolution as Play; the handoff at the end is the only difference.
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, title, config));
    let prefetched = None;
    let tracker_ids = signed_in_tracker_ids(sync);
    let extras = (next, episodes);
    spawn_playback(
        id,
        episode,
        None,
        true,
        prx,
        handoff,
        config,
        services,
        &player(config, paths, player_theme),
        tracker_ids,
        extras,
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
    let Services { paths, http, store, sync, slot, player_theme, .. } = services;
    let played = engine.next_playback(id, &episode);
    let for_show = show_settings::for_show(engine.config.effective(), store, id);
    let skip_filler = for_show.playback.skip_filler;
    let PlayHints { next, episodes, .. } =
        engine.cache.hints(id, &episode, skip_filler, PlayKind::Staged);
    // Named by the download it plays: the Downloads screen says what its rows are called.
    let staged =
        stage(&played, title.clone(), engine.cache.episode_title(id, &episode), engine);
    let _ = tx.send(staged);
    let EngineState { player_tx, .. } = engine;
    let config = &for_show;
    // No provider walk and no torrent session — the file is on disk — but the full
    // playback session all the same: a downloaded episode records history, resumes,
    // and syncs exactly like a streamed one. It used to go straight to mpv with no
    // context, which made downloads second-class in every way that matters.
    // Nothing to look up, so nothing can fail before it plays.
    let (prx, handoff) =
        take_controls(player_tx, successor(id, &episode, title.clone(), config));
    handoff.now();
    let tracker_ids = signed_in_tracker_ids(sync);
    if episodes.is_empty() {
        fill_episodes(id, &episode, services, player_tx);
    }
    let shaders = shader_dir(config, paths);
    let (config, store, http, mpv, tx, slot) = (
        config.clone(),
        store.clone(),
        http.clone(),
        player(config, paths, player_theme),
        tx.for_playback(played),
        slot.clone(),
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
        };
        let settings = playback::PlaySettings {
            shader_dir: shaders,
            ..play_settings(&config, tracker_ids, config.syncplay.enabled)
        };
        playback::play(vec![stream], context, store, http, mpv, settings, slot, tx, prx).await;
    });
}

/// Steer whatever is playing, through the controls of the playback started last.
pub fn control_player(command: PlayerCommand, engine: &EngineState) {
    let EngineState { player_tx, .. } = engine;
    if let Some(sender) = player_tx.as_ref() {
        let _ = sender.send(command);
    }
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
    handoff: Handoff,
    config: &Config,
    services: &Services,
    mpv: &anistream_player::Mpv,
    tracker_ids: Vec<String>,
    // What the player's Up Next card and Episodes menu show, from the engine's record of the
    // titles.
    extras: (Option<playback::NextEpisode>, Vec<playback::EpisodeEntry>),
    shader_dir: Option<std::path::PathBuf>,
    // This episode's streams, found ahead while the previous one's ending played.
    prefetched: Option<Resolved>,
    job: Option<JobGuard>,
    tx: &Outbox,
) {
    let (config, meta, store, registry, http, mpv, slot, tx) = (
        config.clone(),
        services.meta.clone(),
        services.store.clone(),
        services.registry.clone(),
        services.http.clone(),
        mpv.clone(),
        services.slot.clone(),
        tx.clone(),
    );

    tokio::spawn(async move {
        let playback = config.playback.clone();
        let resolved = match prefetched {
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
        (context.next, context.episodes) = extras;
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
        playback::play(streams, context, store, http, mpv, settings, slot, tx, commands).await;
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
            })
            .collect();
        assert_eq!(following(&rows, "1").as_deref(), Some("2"));
        assert_eq!(following(&rows, "3"), None, "a finale has no next");
        assert_eq!(following(&rows, "OVA"), None);
    }
}
