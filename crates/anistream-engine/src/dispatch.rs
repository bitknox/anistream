//! Routing: each command to whatever carries it out, what the engine side does on the way of an
//! event, and the work it leaves itself.
//!
//! The engine's actor drives these, behind `Engine::send` and the engine's event stream:
//! [`dispatch`] for each command, [`receive`] for each event on its way to the front end, and
//! [`handle_internal`] for each message the engine side left itself. No front end calls them.

use anistream_meta::Meta;
use anistream_model::{command::Command, event::Event, notice::Notice};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::playback::start::{
    control_playback, control_player, play, play_local, play_party, play_source,
};
use crate::{
    artwork,
    catalog::{
        calendar::{load_calendar, load_calendar_past},
        detail::{load_detail, open_external, play_trailer},
        lists::{hide_from_continue, load_continue, load_seasonal},
        search::search,
    },
    data,
    downloads::{
        self,
        commands::{
            download_cancel, download_clear_completed, download_delete, download_episode,
            download_many, download_pause,
        },
    },
    jobs::JobGuard,
    outbox::{Internal, Outbox},
    playback::{prefetch::prefetch, resolve::load_sources},
    plugin_stores::tasks::{StoreTaskContext, store_task},
    protection::set_torrent_protection,
    services::Services,
    settings::{set_setting, set_translation, step_setting},
    show_settings::{load_show_settings, reset_show_settings, set_show_setting},
    sources::{
        self,
        commands::{check_providers, disable_plugin},
        torrent::reload_sources,
    },
    state::EngineState,
    titles::{
        episodes::{list_episodes, set_watched},
        matching::{clear_match, fix_match, manual_search, set_provider_preference},
    },
    tracking::{
        self,
        commands::{
            connect, connect_kitsu, load_library, load_title_language, set_score, set_status,
            sync_now,
        },
    },
};

/// What a task run by [`spawn`] is lent: its own clones of the handles it reads, and of the
/// outbox it answers on.
pub struct Lent {
    pub meta: Meta,
    pub store: Store,
    pub registry: ProviderRegistry,
    /// The plugin stores joined, which say where each plugin came from.
    pub stores: crate::plugin_stores::Stores,
    pub tx: Outbox,
}

/// Run a task off the UI thread, once the AniList rate limit lets it, and let it report back
/// through the channel.
fn spawn<W, F>(job: Option<JobGuard>, services: &Services, tx: &Outbox, work: W)
where
    W: FnOnce(Lent) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let meta = services.meta.clone();
    let store = services.store.clone();
    let registry = services.registry.clone();
    let stores = services.stores.clone();
    let tx = tx.clone();

    tokio::spawn(async move {
        // Say so before waiting, not after. At thirty requests a minute a burst of navigation can
        // drain the budget, and the token bucket then delays every request — which presented as an
        // indefinite loading indicator and looked exactly like a hang. As a note on the job it
        // counts down and goes away with the work, where a status line kept saying "12s" after.
        if let Some(wait) = meta.rate_limit_wait().await
            && wait.as_millis() > 400
        {
            match &job {
                Some(job) => job.note("AniList limit", Some(wait)),
                // No job to note it on. A toast, which goes when it is done: as a status it
                // stayed, still counting "12s", after the wait was over.
                None => {
                    let _ = tx.send(Event::Toast(Notice::info(format!(
                        "waiting {}s on the AniList rate limit",
                        wait.as_secs().max(1)
                    ))));
                }
            }
        }
        work(Lent { meta, store, registry, stores, tx }).await;
    });
}

/// Route a command to whatever carries it out.
///
/// One arm for every command, and each names what runs it, so a command added to the
/// protocol does not build until it is routed. What asks AniList or the sources and answers
/// once runs through [`spawn`], behind the AniList limit. Playback runs on its own: it owns
/// the live player's control channel, and replacing that sender is what stops a finished
/// episode's keys steering the next one.
///
/// The actor runs it for each command `Engine::send` hands over, one at a time.
pub(crate) fn dispatch(
    task: Command,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    match task {
        // The lists, the calendar, search and a title's page.
        Command::LoadContinue => spawn(job, services, tx, load_continue),
        Command::LoadSeasonal => spawn(job, services, tx, load_seasonal),
        Command::HideFromContinue { id } => hide_from_continue(id, services, tx),
        Command::LoadCalendar => spawn(job, services, tx, load_calendar),
        Command::LoadCalendarPast { anchor, page } => {
            spawn(job, services, tx, move |lent| load_calendar_past(lent, anchor, page))
        }
        Command::Search { query, filter } => {
            spawn(job, services, tx, move |lent| search(lent, query, filter))
        }
        Command::LoadDetail(id) => spawn(job, services, tx, move |lent| load_detail(lent, id)),
        Command::LoadTitleLanguage => load_title_language(services, tx),
        Command::PlayTrailer { url, title } => {
            play_trailer(url, title, job, engine, services, tx)
        }
        Command::OpenExternal { url } => open_external(url, services, tx),

        // A title's episodes, and the source match they come from.
        Command::LoadEpisodes(id) => {
            spawn(job, services, tx, move |lent| list_episodes(lent, id))
        }
        Command::SetWatched { id, episodes, watched } => {
            set_watched(id, &episodes, watched, engine, services, tx)
        }
        Command::ClearMatch { id } => {
            spawn(job, services, tx, move |lent| clear_match(lent, id))
        }
        Command::FixMatch { id, provider_id, key } => {
            spawn(job, services, tx, move |lent| fix_match(lent, id, provider_id, key))
        }
        Command::ManualSearch { id, query } => {
            manual_search(id, query, job, engine, services, tx)
        }
        Command::SetProviderPreference { id, provider } => {
            set_provider_preference(id, provider, engine, services, tx)
        }

        // Playback.
        Command::Prefetch { id, episode } => prefetch(id, episode, engine, services),
        // Each refused where this build has no player to play in: see `playback::refuse`.
        Command::Play { id, episode } => play(id, episode, job, engine, services, tx),
        Command::PlaySource { id, episode, provider_id, source_id } => {
            play_source(id, episode, provider_id, source_id, job, engine, services, tx)
        }
        Command::PlayParty { id, episode } => {
            play_party(id, episode, job, engine, services, tx)
        }
        Command::PlayLocal { id, episode, title, path } => {
            play_local(id, episode, title, path, job, engine, services, tx)
        }
        Command::LoadSources { id, episode } => {
            load_sources(id, episode, job, engine, services, tx)
        }
        // Nothing to steer where nothing plays: no playback has left its controls.
        Command::Player(command) => control_player(command, engine),
        // The same, aimed: let go when another playback has followed the one it names.
        Command::Control { playback, command } => control_playback(playback, command, engine),
        // Taken in as it is sent by `Engine::send`; one routed here all the same lands.
        Command::PlayerReport { playback, report } => services.hosted.report(playback, report),

        // The sources.
        Command::CheckProviders => spawn(job, services, tx, check_providers),

        // The trackers.
        Command::SyncNow => sync_now(job, services, tx),
        Command::Connect { tracker } => connect(tracker, job, services, tx),
        Command::ConnectKitsu { username, password } => {
            connect_kitsu(username, password, job, services, tx)
        }
        Command::Disconnect { tracker } => {
            tracking::disconnect(&services.sync, &tracker, tx);
            if tracker == "anilist" {
                crate::title_language::signed_out(services, tx);
            }
        }
        Command::SetStatus { id, status } => set_status(id, status, services, tx),
        Command::SetScore { id, score } => set_score(id, score, services, tx),
        Command::ResolveConflict { id, keep_local } => {
            tracking::resolve_conflict(&services.sync, id, keep_local)
        }
        Command::LoadLibrary(segment) => load_library(segment, job, services, tx),

        // Downloads.
        Command::LoadDownloads => downloads::publish_now(&services.store, tx),
        Command::DownloadEpisode { id, episode } => {
            download_episode(id, episode, job, engine, services, tx)
        }
        Command::DownloadMany { id, episodes } => {
            download_many(id, episodes, job, engine, services, tx)
        }
        Command::DownloadPause { id } => download_pause(id, services, tx),
        Command::DownloadCancel { id } => download_cancel(id, services, tx),
        Command::DownloadDelete { id } => download_delete(id, services, tx),
        Command::DownloadClearCompleted => download_clear_completed(services, tx),

        // Settings.
        Command::StepSetting { key, delta } => step_setting(key, delta, engine, services, tx),
        Command::SetSetting { key, value } => set_setting(key, value, engine, services, tx),
        Command::SetTranslation { translation } => set_translation(translation, engine, tx),
        Command::SetTorrentProtection { mode, proxy, on_failure, acknowledged } => {
            set_torrent_protection(mode, proxy, on_failure, acknowledged, engine, services, tx)
        }
        Command::LoadShowSettings { id } => load_show_settings(id, engine, services, tx),
        Command::SetShowSetting { id, key, value } => {
            set_show_setting(id, key, value, engine, services, tx)
        }
        Command::ResetShowSettings { id } => reset_show_settings(id, engine, services, tx),

        // Off the loop, with no job: covers report back on their own.
        Command::FetchArtwork { url } => artwork::spawn_fetch(
            url,
            services.paths.image_cache(),
            services.http.clone(),
            tx.clone(),
        ),

        // The plugin stores.
        Command::LoadStores => {
            store_task(job, engine, services, tx, StoreTaskContext::load_stores)
        }
        Command::PreviewStore { link } => {
            store_task(job, engine, services, tx, move |context| context.preview_store(link))
        }
        Command::JoinStore { link } => {
            store_task(job, engine, services, tx, move |context| context.join_store(link))
        }
        Command::LeaveStore { store } => {
            store_task(job, engine, services, tx, move |context| context.leave_store(store))
        }
        Command::InstallPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.install_plugin(store, plugin)
            })
        }
        Command::RemovePlugin { plugin } => {
            store_task(job, engine, services, tx, move |context| context.remove_plugin(plugin))
        }
        Command::DisablePlugin { id, disabled } => {
            disable_plugin(id, disabled, engine, services, tx)
        }
        Command::CreateInvite { store, name } => {
            store_task(job, engine, services, tx, move |context| {
                context.create_invite(store, name)
            })
        }
        Command::LoadMembers { store } => {
            store_task(job, engine, services, tx, move |context| context.load_members(store))
        }
        Command::RemoveMember { store, member } => {
            store_task(job, engine, services, tx, move |context| {
                context.remove_member(store, member)
            })
        }
        Command::CancelInvite { store, invite } => {
            store_task(job, engine, services, tx, move |context| {
                context.cancel_invite(store, invite)
            })
        }
        Command::LoadLocalPlugins { store } => {
            store_task(job, engine, services, tx, move |context| {
                context.load_local_plugins(store)
            })
        }
        Command::PublishPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.publish_plugin(store, plugin)
            })
        }
        Command::UnpublishPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.unpublish_plugin(store, plugin)
            })
        }
        Command::OpenAdmin { store } => {
            store_task(job, engine, services, tx, move |context| context.open_admin(store))
        }
    }
}

/// The engine side's part in an event on its way to the front end, and what it has to say
/// ahead of the event, for the front end to apply first.
///
/// A finished episode's progress goes to the trackers now, not at the next drain tick: up to a
/// minute later, the pushed progress looked simply absent. And the metadata ladder's notice, when
/// it moved. `tx` is asked for only when there is work to answer on it.
///
/// The actor does this with every event before passing it on.
pub(crate) fn receive(
    update: &Event,
    services: &Services,
    tx: impl FnOnce() -> Outbox,
) -> Vec<Event> {
    if matches!(update, Event::ProgressQueued) {
        let (sync, tx) = (services.sync.clone(), tx());
        tokio::spawn(async move { tracking::drain_once(&sync, &tx).await });
    }
    meta_notice(&services.meta)
}

/// What to say when the metadata ladder moved: once per transition, in both directions, since
/// silently recovering would leave the last thing said untrue. The status rows ride along, so
/// the header chip and the Providers screen agree with the toast without waiting for a refresh.
///
/// The actor says it with the events it passes on.
pub(crate) fn meta_notice(meta: &Meta) -> Vec<Event> {
    let Some(notice) = meta.take_notice() else { return Vec::new() };
    vec![Event::MetaStatus(data::meta_rows(meta)), Event::Toast(Notice::info(notice))]
}

/// Do what the engine side left itself to do, and return what it then has to say, for the
/// front end to apply at once, before anything sent since. Reached in the message's own place
/// among the events, after everything sent before it.
///
/// The metadata ladder's notice comes first, as on the way of an event. Then the copy of the
/// config the background tasks read is brought up to the one in effect, whatever the message.
///
/// The actor does this with each message in its inbox, in the message's place.
pub(crate) async fn handle_internal(
    message: Internal,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Vec<Event> {
    let mut said = meta_notice(&services.meta);
    if let Ok(mut shared) = engine.shared_config.write() {
        *shared = engine.config.effective().clone();
    }
    match message {
        Internal::SettingsSaved => {}
        // Only the order changed: the chain as it stands is put in it, with no rebuild.
        Internal::SourcesReordered => {
            sources::arrange(&services.registry, engine.config.effective());
            said.push(Event::Providers(data::provider_rows(
                &services.registry,
                &services.stores,
            )));
        }
        // Awaited rather than spawned: this owns the session and the guard, and two
        // rebuilds racing would leave one of their sessions running with nothing holding
        // it. Quick: the new torrent source comes up behind the interface, as at launch.
        Internal::SourcesChanged => {
            reload_sources(
                services,
                &mut engine.torrent_tasks,
                &mut engine.vpn_guard,
                &engine.shared_config,
                tx,
                &engine.arrivals,
            )
            .await;
            // Passed on, ahead of what the rebuild reports: the screen says it reloads.
            said.push(Event::SourcesChanged);
        }
    }
    said
}
