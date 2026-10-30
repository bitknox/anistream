//! Routing: each command to whatever carries it out, what the engine side does on the way of an
//! event, and the work it leaves itself.
//!
//! The engine's actor drives these, behind `Engine::send` and the engine's event stream:
//! [`dispatch`] for each command, [`receive`] for each event on its way to the front end, and
//! [`handle_internal`] for each message the engine side left itself. No front end calls them.

use anistream_meta::Meta;
use anistream_model::{
    Msg, Span, command::Command, event::Event, notice::Notice, show::ShowSettingKey,
};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::playback::start::{control_playback, control_player, play, play_local, play_source};
use crate::{
    artwork,
    catalog::{
        browse,
        calendar::{load_calendar, load_calendar_ahead, load_calendar_past},
        detail::{load_detail, open_external, play_trailer},
        lists::{hide_from_continue, load_continue, load_seasonal},
        recents, search,
        up_next::{self, Ask},
    },
    data,
    downloads::{
        self, ahead,
        commands::{
            download_cancel, download_clear_completed, download_delete, download_episode,
            download_many, download_pause, download_start_now,
        },
    },
    jobs::JobGuard,
    outbox::{Internal, Outbox},
    playback::{prefetch::prefetch, resolve::load_sources},
    plugin_stores::tasks::{StoreTaskContext, store_task},
    protection::set_torrent_protection,
    services::Services,
    settings::{set_setting, set_translation, step_setting},
    show_settings::{
        load_show_settings, reset_show_settings, set_show_setting, set_show_subtitles,
    },
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
            cancel_sign_in, connect, connect_kitsu, finish_sign_in, load_library,
            load_title_language, set_score, set_status, sync_now,
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

/// The handles a task is lent, cloned.
pub(crate) fn lend(services: &Services, tx: &Outbox) -> Lent {
    Lent {
        meta: services.meta.clone(),
        store: services.store.clone(),
        registry: services.registry.clone(),
        stores: services.stores.clone(),
        tx: tx.clone(),
    }
}

/// List title `id`'s episodes again, quietly: no activity row, as a source change is not
/// something the viewer asked a title for.
pub(crate) fn list_episodes_again(
    id: anistream_core::ids::AnilistId,
    announce: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let ticket = engine.listed.ticket(id);
    let table = engine.dubs.table(id);
    let naming = engine.names.naming(id, engine.network.offline, &services.clock);
    spawn(None, services, tx, move |lent| async move {
        list_episodes(lent, id, naming, ticket, announce).await;
        drop(table);
    });
}

/// Run a task off the UI thread, once the AniList rate limit lets it, and let it report back
/// through the channel.
fn spawn<W, F>(job: Option<JobGuard>, services: &Services, tx: &Outbox, work: W)
where
    W: FnOnce(Lent) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    spawn_abortable(job, services, tx, work);
}

/// [`spawn`], with the handle that stops it: a search a newer one replaces. Stopped while it
/// waits on the limit, it spends nothing.
pub(crate) fn spawn_abortable<W, F>(
    job: Option<JobGuard>,
    services: &Services,
    tx: &Outbox,
    work: W,
) -> tokio::task::AbortHandle
where
    W: FnOnce(Lent) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let Lent { meta, store, registry, stores, tx } = lend(services, tx);

    tokio::spawn(async move {
        // Say so before waiting, not after. At thirty requests a minute a burst of navigation can
        // drain the budget, and the token bucket then delays every request — which presented as an
        // indefinite loading indicator and looked exactly like a hang. As a note on the job it
        // counts down and goes away with the work, where a status line kept saying "12s" after.
        if let Some(wait) = meta.rate_limit_wait().await
            && wait.as_millis() > 400
        {
            match &job {
                Some(job) => job.note(Msg::JobAnilistLimit, Some(wait)),
                // No job to note it on. A toast, which goes when it is done: as a status it
                // stayed, still counting "12s", after the wait was over.
                None => {
                    let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeAnilistLimit {
                        wait: Span::countdown(wait.as_secs().max(1) as i64),
                    })));
                }
            }
        }
        work(Lent { meta, store, registry, stores, tx }).await;
    })
    .abort_handle()
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
/// What "Up next for you" is told of the viewer and the moment.
fn up_next_ask(engine: &EngineState, services: &Services) -> Ask {
    Ask {
        listed: downloads::ahead::listed(services),
        adult: engine.config.effective().meta.adult,
        now: services.clock.now(),
        zone: services.clock.zone(),
    }
}

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
        Command::LoadSeasonal => {
            let adult = engine.config.effective().meta.adult;
            spawn(job, services, tx, move |lent| load_seasonal(lent, adult))
        }
        Command::HideFromContinue { id } => hide_from_continue(id, services, tx),
        Command::LoadUpNext => {
            engine.up_next_queue = Some(up_next::queue_mark(&services.store));
            let ask = up_next_ask(engine, services);
            spawn(job, services, tx, move |lent| up_next::load_up_next(lent, ask))
        }
        Command::RestoreUpNext { id } => {
            let ask = up_next_ask(engine, services);
            spawn(job, services, tx, move |lent| up_next::restore(lent, id, ask))
        }
        Command::DismissUpNext { id } => {
            spawn(job, services, tx, move |lent| up_next::dismiss(lent, id))
        }
        // Each row marked with its dub where a title page found it: see `Entry::airing_dubbed`.
        Command::LoadCalendar | Command::RefreshCalendar => {
            let (dubs, memory) = (engine.dubs.clone(), engine.calendar.clone());
            let clock = services.clock.clone();
            let remembered = matches!(task, Command::LoadCalendar);
            spawn(job, services, tx, move |lent| {
                load_calendar(lent, dubs, memory, clock, remembered)
            })
        }
        Command::LoadCalendarPast { anchor, page, through } => {
            let (dubs, memory) = (engine.dubs.clone(), engine.calendar.clone());
            spawn(job, services, tx, move |lent| {
                load_calendar_past(lent, dubs, memory, anchor, page, through)
            })
        }
        Command::LoadCalendarAhead { anchor, page, through } => {
            let (dubs, memory) = (engine.dubs.clone(), engine.calendar.clone());
            spawn(job, services, tx, move |lent| {
                load_calendar_ahead(lent, dubs, memory, anchor, page, through)
            })
        }
        Command::Search { query } => search::submit(query, job, engine, services, tx),
        Command::SearchLive { query } => search::live(query, engine, services, tx),
        Command::SearchMore { query, page } => {
            let (searches, adult) =
                (engine.search.clone(), engine.config.effective().meta.adult);
            spawn(job, services, tx, move |lent| {
                search::more(lent, searches, query, page, adult)
            })
        }
        Command::LoadBrowse => {
            let (adult, now) = (engine.config.effective().meta.adult, services.clock.now());
            spawn(job, services, tx, move |lent| browse::load_browse(lent, adult, now))
        }
        Command::LoadSearchFacets => {
            let (adult, now) = (engine.config.effective().meta.adult, services.clock.now());
            spawn(job, services, tx, move |lent| browse::load_facets(lent, adult, now))
        }
        Command::LoadRecentSearches => recents::publish(services, tx),
        Command::RememberSearch(query) => recents::remember(query, services, tx),
        Command::ForgetRecentSearch(query) => recents::forget(query, services, tx),
        Command::ClearRecentSearches => recents::clear(services, tx),
        Command::LoadDetail(id) => {
            // Another title's page: the episode names looked up for the one left stop.
            engine.names.leave_for(id);
            spawn(job, services, tx, move |lent| load_detail(lent, id))
        }
        Command::LoadTitleLanguage => load_title_language(services, tx),
        Command::PlayTrailer { url, title } => {
            play_trailer(url, title, job, engine, services, tx)
        }
        Command::OpenExternal { url } => open_external(url, services, tx),

        // An episode's discussion: off the loop, with no job, each answered with the
        // discussion whole. See `discussion`.
        Command::OpenDiscussion { media, episode } => {
            crate::discussion::open(media, episode, engine, services, tx)
        }
        Command::MoreComments => crate::discussion::more(engine, services, tx),
        Command::PostComment { parent, text } => {
            crate::discussion::post(parent, text, engine, services, tx)
        }
        Command::EditComment { id, text } => {
            crate::discussion::edit(id, text, engine, services, tx)
        }
        Command::DeleteComment { id } => crate::discussion::delete(id, engine, services, tx),
        Command::LikeComment { id } => crate::discussion::like(id, engine, services, tx),
        Command::CloseDiscussion => crate::discussion::close(engine),

        // A title's episodes, and the source match they come from.
        // Quietly, with no job, and never in the way of the table or a play: see
        // `titles::dub`.
        Command::LoadDub(id) => engine.dubs.ask(id, engine.network.offline, services, tx),
        Command::LoadEpisodes(id) => {
            engine.listed.note(id);
            // A lookup of the title's dub waits for this.
            let table = engine.dubs.table(id);
            // Named from another source where its own left titles and stills out: see
            // `titles::names`.
            let naming = engine.names.naming(id, engine.network.offline, &services.clock);
            let ticket = engine.listed.ticket(id);
            spawn(job, services, tx, move |lent| async move {
                list_episodes(lent, id, naming, ticket, true).await;
                drop(table);
            })
        }
        Command::SetWatched { id, episodes, watched } => {
            set_watched(id, &episodes, watched, engine, services, tx);
            ahead::top_up(engine, services, tx);
        }
        // A title matched afresh may be another title on the source, so its dub is too.
        Command::ClearMatch { id } => {
            engine.dubs.forget(id);
            engine.names.forget(id);
            spawn(job, services, tx, move |lent| clear_match(lent, id))
        }
        Command::FixMatch { id, provider_id, key } => {
            engine.dubs.forget(id);
            engine.names.forget(id);
            spawn(job, services, tx, move |lent| fix_match(lent, id, provider_id, key))
        }
        Command::ManualSearch { id, query } => {
            manual_search(id, query, job, engine, services, tx)
        }
        Command::SetProviderPreference { id, provider } => {
            engine.dubs.forget(id);
            engine.names.forget(id);
            set_provider_preference(id, provider, engine, services, tx);
            // The pin applies to what is on screen now, not the next time it opens.
            crate::titles::refresh::relist(id, true, engine, services, tx);
        }

        // Playback.
        Command::Prefetch { id, episode } => prefetch(id, episode, engine, services),
        // Each refused where this build has no player to play in: see `playback::refuse`.
        // A play wants the source's time: a dub or episode names lookup running stops for it.
        Command::Play { id, episode, from_start } => {
            engine.dubs.stop();
            engine.names.stop();
            #[cfg(feature = "party")]
            crate::party::join_by_default(engine, services, tx);
            play(id, episode, from_start, job, engine, services, tx)
        }
        Command::PlaySource { id, episode, provider_id, source_id } => {
            engine.dubs.stop();
            engine.names.stop();
            #[cfg(feature = "party")]
            crate::party::join_by_default(engine, services, tx);
            play_source(id, episode, provider_id, source_id, job, engine, services, tx)
        }
        // Watch parties, where this build has them: see `party`.
        #[cfg(feature = "party")]
        Command::PlayParty { id, episode, server, room, name } => {
            engine.dubs.stop();
            engine.names.stop();
            crate::party::play(id, episode, (server, room, name), job, engine, services, tx)
        }
        #[cfg(feature = "party")]
        Command::LoadPartyForm => crate::party::form(engine, tx),
        #[cfg(feature = "party")]
        Command::PartyJoin { server, room, name } => {
            crate::party::join(&server, &room, name.as_deref(), engine, services, tx);
        }
        #[cfg(feature = "party")]
        Command::PartyReady { ready } => crate::party::ready(ready, engine, tx),
        #[cfg(feature = "party")]
        Command::PartyLeave => crate::party::leave(engine, tx),
        #[cfg(not(feature = "party"))]
        Command::PlayParty { .. }
        | Command::LoadPartyForm
        | Command::PartyJoin { .. }
        | Command::PartyReady { .. }
        | Command::PartyLeave => {
            let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticePartyNotBuilt)));
        }
        Command::PlayLocal { id, episode, title, path } => {
            #[cfg(feature = "party")]
            crate::party::join_by_default(engine, services, tx);
            play_local(id, episode, title, path, job, engine, services, tx)
        }
        Command::LoadSourceOptions(id) => {
            crate::sources::options::load_source_options(id, job, engine, services, tx)
        }
        Command::PinRelease { id, provider_id, release_id, title } => {
            crate::sources::options::pin_release(
                id,
                anistream_store::ReleasePin { provider_id, release_id, title },
                engine,
                services,
                tx,
            )
        }
        Command::LoadSources { id, episode } => {
            load_sources(id, episode, job, engine, services, tx)
        }
        // Nothing to steer where nothing plays: no playback has left its controls.
        Command::Player(command) => control_player(command, engine),
        // The same, aimed: let go when another playback has followed the one it names.
        Command::Control { playback, command } => control_playback(playback, command, engine),
        // Where the app is and its network: which torrents wait, and whether keep next runs.
        Command::AppPhase { phase } => {
            crate::phases::app_phase(phase, engine, tx);
            ahead::top_up(engine, services, tx);
        }
        Command::NetworkPath { cellular, constrained, offline } => {
            let network = crate::phases::Network { cellular, constrained, offline };
            services.offline.store(offline, std::sync::atomic::Ordering::Relaxed);
            crate::phases::network_path(network, engine, tx);
            ahead::top_up(engine, services, tx);
        }
        Command::DeviceState {
            reduce_motion,
            autoplay_previews,
            low_power,
            thermal_serious,
            screen_reader,
            low_memory,
        } => {
            let device = anistream_model::trailer::Device {
                reduce_motion,
                autoplay_previews,
                low_power,
                thermal_serious,
                screen_reader,
                low_memory,
            };
            crate::autoplay::device_state(device, engine, tx);
        }
        // Taken in as it is sent by `Engine::send`; one routed here all the same lands.
        Command::PlayerReport { playback, report } => services.hosted.report(playback, report),

        // The sources.
        Command::CheckProviders => spawn(job, services, tx, check_providers),
        Command::CheckConnection => {
            engine.torrent_tasks.check_now(engine.config.effective(), services, tx);
        }

        // The trackers.
        Command::SyncNow => sync_now(job, services, tx),
        Command::Connect { tracker } => connect(tracker, job, services, tx),
        Command::FinishSignIn { tracker, callback } => {
            finish_sign_in(tracker, callback, job, services, tx)
        }
        Command::CancelSignIn { tracker } => cancel_sign_in(tracker, services, tx),
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
        Command::DownloadStartNow { id } => download_start_now(id, services, tx),
        Command::DownloadPause { id } => download_pause(id, services, tx),
        Command::DownloadCancel { id } => download_cancel(id, services, tx),
        Command::DownloadDelete { id } => download_delete(id, services, tx),
        Command::DownloadClearCompleted => download_clear_completed(services, tx),

        // Settings.
        Command::StepSetting { key, delta } => {
            step_setting(key, delta, engine, services, tx);
            if ahead::follows(key) {
                ahead::top_up(engine, services, tx);
            }
        }
        Command::SetSetting { key, value } => {
            set_setting(key, value, engine, services, tx);
            if ahead::follows(key) {
                ahead::top_up(engine, services, tx);
            }
        }
        Command::SetTranslation { translation } => set_translation(translation, engine, tx),
        Command::SetTorrentProtection { mode, proxy, on_failure, acknowledged } => {
            set_torrent_protection(mode, proxy, on_failure, acknowledged, engine, services, tx)
        }
        // Off the loop, with no job: the verdict is the answer.
        Command::CheckTorrentProtection { mode, proxy } => {
            crate::protection::check_torrent_protection(mode, proxy, engine, services, tx)
        }
        Command::LoadShowSettings { id } => load_show_settings(id, engine, services, tx),
        Command::SetShowSetting { id, key, value } => {
            set_show_setting(id, key, value, engine, services, tx);
            if key == ShowSettingKey::KeepNext {
                ahead::top_up(engine, services, tx);
            }
        }
        Command::ResetShowSettings { id } => {
            reset_show_settings(id, engine, services, tx);
            ahead::top_up(engine, services, tx);
        }
        Command::SetShowSubtitles { id, translation, subtitles } => {
            set_show_subtitles(id, translation, subtitles, engine, services, tx)
        }

        // Off the loop, with no job: covers report back on their own.
        Command::FetchArtwork { url } => artwork::spawn_fetch(
            url,
            services.paths.image_cache(),
            services.http.clone(),
            services.artwork.clone(),
            tx.clone(),
        ),

        // What a native app shows outside itself: notifications, and its widgets. Off the loop,
        // with no job, each answered with one event.
        Command::CheckNewEpisodes => crate::new_episodes::check(engine, services, tx),
        Command::WidgetSnapshot => crate::widgets::snapshot(engine, services, tx),
        Command::UpcomingNotices => crate::upcoming::answer(engine, services, tx),
        // Settings' Check Now, off the loop, with no job: the check says where it stands.
        Command::CheckForUpdates => {
            crate::updates::check_now(&services.paths.cache_dir, &services.http, tx)
        }

        // The plugin stores.
        Command::LoadStores => {
            store_task(job, engine, services, tx, StoreTaskContext::load_stores)
        }
        Command::PreviewStore { link } => {
            store_task(job, engine, services, tx, move |context| context.preview_store(link))
        }
        Command::JoinStore { link, label } => {
            store_task(job, engine, services, tx, move |context| {
                context.join_store(link, label)
            })
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
        Command::SetOpenJoin { store, open } => {
            store_task(job, engine, services, tx, move |context| {
                context.set_open_join(store, open)
            })
        }
        Command::RemoveAnonymous { store, device } => {
            store_task(job, engine, services, tx, move |context| {
                context.remove_anonymous(store, device)
            })
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
    // The ladder words its own transitions; these are the three it makes.
    let msg = match notice.as_str() {
        "anilist is unreachable — fresh data via tenrai for now" => {
            Msg::NoticeAnilistUnreachable
        }
        "no metadata source answered — showing what was last cached" => {
            Msg::NoticeMetadataCached
        }
        "anilist is back" => Msg::NoticeAnilistBack,
        _ => Msg::verbatim(notice),
    };
    vec![Event::MetaStatus(data::meta_rows(meta)), Event::Toast(Notice::info(msg))]
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
        // What runs on cellular may have changed with it.
        Internal::SettingsSaved => crate::phases::settings_saved(engine, tx),
        Internal::TopUp => ahead::top_up(engine, services, tx),
        // Only the order changed: the chain as it stands is put in it, with no rebuild.
        Internal::SourcesReordered => {
            sources::arrange(&services.registry, engine.config.effective());
            crate::titles::refresh::sources_changed(engine, services, tx);
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
            );
            crate::titles::refresh::sources_changed(engine, services, tx);
            // Passed on, ahead of what the rebuild reports: the screen says it reloads.
            said.push(Event::SourcesChanged);
        }
    }
    said
}
