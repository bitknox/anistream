//! The torrent stack as the settings stand it up and down: the tasks that run beside a torrent
//! session once the VPN guard passes, the session lent to the sources that name torrents, and
//! the source chain rebuilt when a setting changes which sources there are.

use anistream_model::Msg;
use std::{sync::Arc, time::Duration};

use anistream_core::config::Config;
use anistream_model::{event::Event, notice::Notice, sources::GuardView};
use anistream_providers::vpn::VpnGuard;
use anistream_store::Store;
use tokio::sync::{Notify, mpsc};

use crate::{
    SharedConfig, background::prune, data, downloads, outbox::Outbox, services::Services,
    sources, sources::vpn::vpn_watch,
};

/// How long a bring-up waits for the session it replaces to stand down.
const TEARDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// Everything torrent playback runs in the background: the download manager and the
/// VPN guard's re-verification loop.
///
/// Grouped so they can be stopped and started as one. Playback the user has just switched
/// off must stop *seeding*, not merely stop being offered — and that means the tasks
/// holding its session have to end with it.
#[derive(Default)]
pub struct TorrentTasks {
    /// Waiting for the VPN guard to pass, before there is a session.
    bring_up: Option<tokio::task::JoinHandle<()>>,
    /// Which bring-up the current one is. A result from an earlier one — the settings changed
    /// while it waited — describes playback that has since been switched off or replaced.
    generation: u64,
    downloads: Option<tokio::task::JoinHandle<()>>,
    watcher: Option<tokio::task::JoinHandle<()>>,
    seeding: Option<tokio::task::JoinHandle<()>>,
    /// Keeps the stream cache within its limit while the session writes to it.
    cache: Option<tokio::task::JoinHandle<()>>,
    session: Option<Arc<anistream_providers::torrent::TorrentSession>>,
    /// The session a settings change let go of, still standing down: the next bring-up waits
    /// for it, so the old traffic stops before new traffic starts, without the actor waiting.
    teardown: Option<tokio::task::JoinHandle<()>>,
    /// Brings the guard's next check forward, for Sources' Check Now: whichever waits on the
    /// guard, the bring-up before a session or the watcher beside one, checks at once.
    check: Arc<Notify>,
}

impl TorrentTasks {
    fn start(
        guard: &Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
        store: &Store,
        config: &SharedConfig,
        tx: &Outbox,
        check: &Arc<Notify>,
    ) -> Self {
        let Some((guard, session)) = guard.clone() else {
            // No session means no downloads, which is correct rather than a limitation:
            // every downloadable source here is a torrent.
            return Self { check: Arc::clone(check), ..Self::default() };
        };

        let downloads =
            downloads::spawn(store.clone(), session.clone(), Arc::clone(config), tx.clone());
        downloads::publish_now(store, tx);

        let _ = tx.send(Event::Vpn(Some(sources::guard_view(&guard.state()))));
        let watcher =
            tokio::spawn(vpn_watch(guard, session.clone(), tx.clone(), Arc::clone(check)));
        let seeding =
            tokio::spawn(seeding_watch(session.clone(), Arc::clone(config), tx.clone()));
        let cache = tokio::spawn(prune::torrent_cache_watch(
            session.clone(),
            store.clone(),
            Arc::clone(config),
        ));
        Self {
            bring_up: None,
            generation: 0,
            downloads: Some(downloads),
            watcher: Some(watcher),
            seeding: Some(seeding),
            cache: Some(cache),
            session: Some(session),
            teardown: None,
            check: Arc::clone(check),
        }
    }

    /// Start bringing torrent playback up, when the configuration asks for it.
    ///
    /// In the background: the guard's check is a network request through a tunnel that may
    /// be down, and the first frame must not wait on it. The badge says what the guard found
    /// meanwhile, and the source joins the chain when it passes — see [`Self::arrived`].
    pub(crate) fn bring_up(
        &mut self,
        config: &Config,
        services: &Services,
        tx: &Outbox,
        arrivals: &mpsc::UnboundedSender<(u64, std::result::Result<sources::TorrentUp, Msg>)>,
    ) {
        self.generation += 1;
        if !sources::torrent_wanted(config) {
            return;
        }
        let _ = tx.send(Event::Vpn(Some(anistream_model::sources::GuardView::Checking)));
        let generation = self.generation;
        let check = Arc::clone(&self.check);
        let fixture = services.fixture_exit.clone();
        let teardown = self.teardown.take();
        let (config, paths, store, tx, arrivals) = (
            // With the proxy's password, which only this copy holds.
            crate::protection::with_proxy_password(config, services),
            services.paths.clone(),
            services.store.clone(),
            tx.clone(),
            arrivals.clone(),
        );
        self.bring_up = Some(tokio::spawn(async move {
            // The session this one replaces stands down first, within reason: one that takes
            // longer has paused what it could, and is left to finish on its own.
            if let Some(teardown) = teardown
                && tokio::time::timeout(TEARDOWN_WAIT, teardown).await.is_err()
            {
                tracing::warn!("the previous torrent session was still standing down");
            }
            // A fixture build says what the check finds and starts no session: a session reaches
            // the DHT and the trackers, which no fixture may.
            if fixture.is_some() {
                check_alone(&config, &tx, fixture.as_ref());
                return;
            }
            let outcome =
                sources::bring_up_torrent(config, paths, store, tx, check, fixture).await;
            let _ = arrivals.send((generation, outcome));
        }));
    }

    /// A bring-up finished: lend the session to the sources that name torrents and start what
    /// runs beside it, or say why it will not come up.
    pub(crate) fn arrived(
        &mut self,
        generation: u64,
        outcome: std::result::Result<sources::TorrentUp, Msg>,
        vpn_guard: &mut Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
        services: &Services,
        config: &SharedConfig,
        tx: &Outbox,
    ) {
        if generation != self.generation {
            // Superseded. Its session holds no torrents yet, so letting go of it is enough.
            return;
        }
        self.bring_up = None;
        let up = match outcome {
            Ok(up) => up,
            Err(reason) => {
                tracing::warn!(reason = %reason.english(), "torrent playback not started");
                let _ = tx.send(Event::ProviderNote(reason));
                return;
            }
        };
        let registry = &services.registry;
        let snapshot =
            config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
        sources::lend_torrents(registry, Some(&up.session), &snapshot);
        // The sources that name torrents are usable now: what is on screen was listed without.
        let _ = tx.internal(crate::outbox::Internal::SourcesReordered);
        // Its advice is a terminal command, so an app that hosts its own player is not told.
        match up.warning {
            Some(warning) if !services.hosted_player => {
                let _ = tx.send(Event::Toast(Notice::info(warning)));
            }
            Some(_) => tracing::info!("no os-level kill switch; the app is not told"),
            None => {}
        }
        *vpn_guard = Some((up.guard, up.session));
        let check = Arc::clone(&self.check);
        *self =
            Self { generation, ..Self::start(vpn_guard, &services.store, config, tx, &check) };
        let _ = tx.send(Event::Providers(data::provider_rows(registry, &services.stores)));
        tracing::info!(providers = ?registry.ids(), "torrent playback up");
    }

    /// Check where torrent traffic leaves from now, as Sources' Check Now asks, rather than at
    /// the guard's next check: `Checking`, then the verdict, on `Event::Vpn`; `None` there when
    /// torrents are off.
    ///
    /// What waits on the guard checks at once, so a failed check does what the guard's own
    /// does: the watcher halts the session, or a bring-up keeps waiting. With neither, as when
    /// the session could not start, the guard is checked on its own and nothing else follows
    /// from it.
    pub(crate) fn check_now(&self, config: &Config, services: &Services, tx: &Outbox) {
        if self.bring_forward(config, tx) {
            check_alone(
                &crate::protection::with_proxy_password(config, services),
                tx,
                services.fixture_exit.as_ref(),
            );
        }
    }

    /// Check Now, as far as it is the tasks': say there is no guard, or that it is checking
    /// and have whatever waits on it check now. Whether nothing does, so the guard is checked
    /// on its own.
    fn bring_forward(&self, config: &Config, tx: &Outbox) -> bool {
        if !sources::torrent_wanted(config) {
            let _ = tx.send(Event::Vpn(None));
            return false;
        }
        let _ = tx.send(Event::Vpn(Some(GuardView::Checking)));
        // A bring-up that has already given up, its outcome not yet taken in, waits on nothing.
        let running = |task: &Option<tokio::task::JoinHandle<()>>| {
            task.as_ref().is_some_and(|task| !task.is_finished())
        };
        if running(&self.bring_up) || running(&self.watcher) {
            self.check.notify_one();
            return false;
        }
        true
    }

    /// Stop everything and close the session before letting go of it, as a settings change
    /// asks.
    ///
    /// Closing is the whole point: dropping the handles would leave librqbit seeding whatever
    /// it holds until the process exits, which for playback the user just turned off is
    /// precisely the traffic they meant to stop. The close pauses each torrent in turn, which
    /// on a slow disk takes a while, so it runs in a task of its own rather than on the actor:
    /// the next bring-up waits for it. The watchers go first, so none of them reacts to the
    /// session going, and the guard's halt is not used, since that says "vpn leak".
    fn stop(&mut self) {
        // A bring-up still waiting for the guard has no session to halt, only a wait to end.
        if let Some(waiting) = self.bring_up.take() {
            waiting.abort();
        }
        for task in
            [self.downloads.take(), self.watcher.take(), self.seeding.take(), self.cache.take()]
                .into_iter()
                .flatten()
        {
            task.abort();
        }
        if let Some(session) = self.session.take() {
            let earlier = self.teardown.take();
            self.teardown = Some(tokio::spawn(async move {
                if let Some(earlier) = earlier {
                    let _ = earlier.await;
                }
                session.close().await;
                tracing::info!("torrent session closed for a settings change");
            }));
        }
    }

    /// Stand everything down as the app quits: the watchers first, so none of them reacts to
    /// the session going, then the session, closed in order so it keeps how far each torrent
    /// got. Not [`Self::stop`], whose halt is the guard's answer to a leak and says so in the
    /// log.
    pub(crate) async fn close(&mut self) {
        if let Some(waiting) = self.bring_up.take() {
            waiting.abort();
        }
        for task in
            [self.downloads.take(), self.watcher.take(), self.seeding.take(), self.cache.take()]
                .into_iter()
                .flatten()
        {
            task.abort();
        }
        if let Some(session) = self.session.take() {
            session.close().await;
        }
    }
}

/// Take built-in torrent search out of the config file, as the first launch after its removal
/// does: the index and curation addresses, and `"torrent"` in the source order and the
/// disabled list. The rest of the file is left as it was, and `config` and `saved` lose the
/// same entries, so nothing on screen lists a source that no longer exists.
///
/// The notice to say once, when an index address was set: the person chose one, so they are
/// told where it went. A file that cannot be read or written is left alone, and the stale
/// entries name nothing.
pub(crate) fn retire_torrent_search(
    paths: &anistream_core::config::Paths,
    config: &mut Config,
    saved: &mut Option<Config>,
) -> Option<Msg> {
    let retired = match anistream_core::settings::retire_torrent_search(paths) {
        Ok(retired) => retired,
        Err(error) => {
            tracing::warn!(%error, "torrent search not taken out of the config");
            return None;
        }
    };
    for config in std::iter::once(config).chain(saved.as_mut()) {
        let providers = &mut config.providers;
        providers.order.retain(|id| id != "torrent");
        providers.disabled.retain(|id| id != "torrent");
    }
    if retired.changed {
        tracing::info!(had_index = retired.had_index, "built-in torrent search taken out");
    }
    retired.had_index.then_some(Msg::TorrentSearchMoved)
}

/// Check the guard `config` describes on its own, with no session or bring-up to act on what it
/// finds, and say the verdict.
fn check_alone(
    config: &Config,
    tx: &Outbox,
    fixture: Option<&anistream_providers::vpn::Egress>,
) {
    let guard = match sources::guard_for(config, fixture) {
        Ok(guard) => guard,
        Err(reason) => {
            let _ = tx.send(Event::Vpn(Some(GuardView::Unreachable)));
            let _ = tx.send(Event::Toast(Notice::alert(Msg::TorrentNotChecked { reason })));
            return;
        }
    };
    let tx = tx.clone();
    tokio::spawn(async move {
        let state = guard.verify().await;
        let _ = tx.send(Event::Vpn(Some(sources::guard_view(&state))));
    });
}

/// Keep the session's seeding in line with the settings, retire watched episodes that have
/// given back enough, and tell the Downloads screen what is still seeding.
///
/// Reads the shared config each tick rather than being told about changes: a setting saved on
/// the Settings screen reaches the session within one tick, upload cap included, with no
/// rebuild of the source (which would tear down the episode playing).
async fn seeding_watch(
    session: Arc<anistream_providers::torrent::TorrentSession>,
    config: SharedConfig,
    tx: Outbox,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(10));
    let mut shown: Option<Vec<anistream_model::downloads::SeedRow>> = None;
    loop {
        ticker.tick().await;
        let goal =
            config.read().map(|c| c.providers.torrent.seeding.clone()).unwrap_or_default();
        session.configure_seeding(&goal).await;
        let rows: Vec<anistream_model::downloads::SeedRow> = session
            .reap_seeding()
            .await
            .into_iter()
            .map(|seed| anistream_model::downloads::SeedRow {
                name: seed.name,
                uploaded_bytes: seed.uploaded_bytes,
                ratio: seed.ratio,
                goal: goal.ratio,
                left_secs: seed.left.as_secs(),
            })
            .collect();
        // Only when it changed: an idle screen should not redraw every ten seconds for nothing.
        if shown.as_ref() != Some(&rows) {
            if tx.send(Event::Seeding(rows.clone())).is_err() {
                break;
            }
            shown = Some(rows);
        }
    }
}

/// Rebuild the source chain from the configuration as it now stands.
///
/// Called when a setting redefines what sources exist. The chain is swapped *in place*
/// so every holder of a registry clone sees the change, and the plugins that compiled in
/// the background are carried across — they are not described by config and a rebuild
/// that forgot them would silently drop sources the user installed.
pub fn reload_sources(
    services: &Services,
    torrent_tasks: &mut TorrentTasks,
    vpn_guard: &mut Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
    config: &SharedConfig,
    tx: &Outbox,
    arrivals: &mpsc::UnboundedSender<(u64, std::result::Result<sources::TorrentUp, Msg>)>,
) {
    // Old traffic stops before new traffic starts. The other order would leave two
    // sessions briefly alive, and one of them is the one being switched off. The stop goes on
    // in the background, and the bring-up below waits for it.
    torrent_tasks.stop();
    *vpn_guard = None;
    let registry = &services.registry;
    // The sources that name torrents wait for the next session, not the one going.
    sources::lend_torrents(registry, None, &Config::default());

    let plugins = registry.of_kind(anistream_core::traits::ProviderKind::Plugin);
    let snapshot = config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
    let rebuilt = sources::build_registry_deferred(&snapshot, &services.http);
    let mut providers = rebuilt.all();
    providers.extend(plugins);
    registry.replace(providers);
    registry.set_disabled(snapshot.providers.disabled.iter().cloned());
    registry.retry_down();
    // The plugins were carried across behind the rebuilt sources; a plugin the order names
    // ahead of one of those goes back in front of it.
    sources::arrange(registry, &snapshot);

    torrent_tasks.bring_up(&snapshot, services, tx, arrivals);

    if !sources::torrent_wanted(&snapshot) {
        // No session: say so, and clear a badge that now describes nothing.
        let _ = tx.send(Event::Vpn(None));
    }
    let _ = tx.send(Event::ProviderNote(if sources::torrent_wanted(&snapshot) {
        Msg::SourcesReloadedTorrentsPending
    } else {
        Msg::SourcesReloadedTorrentsOff
    }));
    let _ = tx.send(Event::Providers(data::provider_rows(registry, &services.stores)));
    tracing::info!(providers = ?registry.ids(), "source chain reloaded");
}

#[cfg(test)]
mod tests {
    use anistream_core::config::VpnMode;

    use super::*;

    /// Torrents on, protected the way `mode` says.
    fn torrents(mode: VpnMode) -> Config {
        let mut config = Config::default();
        let torrent = &mut config.providers.torrent;
        torrent.enabled = true;
        torrent.vpn.mode = mode;
        torrent.vpn.i_understand_my_ip_is_exposed = true;
        config
    }

    fn said(rx: &mut mpsc::UnboundedReceiver<Event>) -> Vec<Event> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    /// What `Event::Vpn`s said, in order; anything else fails.
    fn badges(said: Vec<Event>) -> Vec<Option<GuardView>> {
        said.into_iter()
            .map(|event| match event {
                Event::Vpn(view) => view,
                other => panic!("not a badge: {other:?}"),
            })
            .collect()
    }

    #[tokio::test]
    async fn check_now_wakes_what_waits_on_the_guard_or_checks_it_alone() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx: Outbox = tx.into();
        let mut tasks = TorrentTasks::default();

        // Off: there is no guard.
        assert!(!tasks.bring_forward(&Config::default(), &tx));
        assert_eq!(badges(said(&mut rx)), [None]);

        // On, and nothing waits on it: checked on its own.
        let config = torrents(VpnMode::None);
        assert!(tasks.bring_forward(&config, &tx));
        assert_eq!(badges(said(&mut rx)), [Some(GuardView::Checking)]);
        check_alone(&config, &tx, None);
        let verdict = rx.recv().await.into_iter().collect();
        assert_eq!(badges(verdict), [Some(GuardView::Unprotected)]);

        // A bring-up waiting on the guard checks now, and says the verdict itself.
        let check = Arc::clone(&tasks.check);
        let waiting = tokio::spawn(async move { check.notified().await });
        tasks.bring_up = Some(waiting);
        tokio::task::yield_now().await;
        assert!(!tasks.bring_forward(&config, &tx));
        assert_eq!(badges(said(&mut rx)), [Some(GuardView::Checking)]);
        let woken =
            tokio::time::timeout(Duration::from_secs(5), tasks.bring_up.take().unwrap());
        woken.await.expect("woken").unwrap();

        // One that has given up waits on nothing.
        tasks.bring_up = Some(tokio::spawn(async {}));
        tokio::task::yield_now().await;
        while !tasks.bring_up.as_ref().unwrap().is_finished() {
            tokio::task::yield_now().await;
        }
        assert!(tasks.bring_forward(&config, &tx), "checked on its own");
    }

    #[tokio::test]
    async fn a_guard_that_cannot_be_built_is_said_and_not_checked() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx: Outbox = tx.into();
        // A proxy with no address.
        check_alone(&torrents(VpnMode::Socks5), &tx, None);
        let mut said = said(&mut rx);
        let Some(Event::Toast(notice)) = said.pop() else { panic!("{said:?}") };
        assert_eq!(badges(said), [Some(GuardView::Unreachable)]);
        assert!(
            notice.text.starts_with("Couldn't check the connection. socks5 mode requires"),
            "{notice:?}"
        );
    }
}
