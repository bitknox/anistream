//! The torrent stack as the settings stand it up and down: the tasks that run beside a torrent
//! session once the VPN guard passes, and the source chain rebuilt when a setting changes which
//! sources there are.

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

/// Everything the torrent source runs in the background: the download manager and the
/// VPN guard's re-verification loop.
///
/// Grouped so they can be stopped and started as one. A source the user has just switched
/// off must stop *seeding*, not merely stop being offered — and that means the tasks
/// holding its session have to end with it.
#[derive(Default)]
pub struct TorrentTasks {
    /// Waiting for the VPN guard to pass, before there is a session.
    bring_up: Option<tokio::task::JoinHandle<()>>,
    /// Which bring-up the current one is. A result from an earlier one — the settings changed
    /// while it waited — describes a source that has since been switched off or replaced.
    generation: u64,
    downloads: Option<tokio::task::JoinHandle<()>>,
    watcher: Option<tokio::task::JoinHandle<()>>,
    seeding: Option<tokio::task::JoinHandle<()>>,
    /// Keeps the stream cache within its limit while the session writes to it.
    cache: Option<tokio::task::JoinHandle<()>>,
    session: Option<Arc<anistream_providers::torrent::TorrentSession>>,
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
            check: Arc::clone(check),
        }
    }

    /// Start bringing the torrent source up, when the configuration asks for it.
    ///
    /// In the background: the guard's check is a network request through a tunnel that may
    /// be down, and the first frame must not wait on it. The badge says what the guard found
    /// meanwhile, and the source joins the chain when it passes — see [`Self::arrived`].
    pub(crate) fn bring_up(
        &mut self,
        config: &Config,
        services: &Services,
        tx: &Outbox,
        arrivals: &mpsc::UnboundedSender<(
            u64,
            std::result::Result<sources::TorrentUp, String>,
        )>,
    ) {
        self.generation += 1;
        if !sources::torrent_wanted(config) {
            return;
        }
        let _ = tx.send(Event::Vpn(Some(anistream_model::sources::GuardView::Checking)));
        let generation = self.generation;
        let check = Arc::clone(&self.check);
        let (config, http, paths, store, tx, arrivals) = (
            // With the proxy's password, which only this copy holds.
            crate::protection::with_proxy_password(config, services),
            services.http.clone(),
            services.paths.clone(),
            services.store.clone(),
            tx.clone(),
            arrivals.clone(),
        );
        self.bring_up = Some(tokio::spawn(async move {
            let outcome =
                sources::bring_up_torrent(config, http, paths, store, tx, check).await;
            let _ = arrivals.send((generation, outcome));
        }));
    }

    /// A bring-up finished: put the source in the chain and start what runs beside it, or say
    /// why it will not come up.
    pub(crate) fn arrived(
        &mut self,
        generation: u64,
        outcome: std::result::Result<sources::TorrentUp, String>,
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
                tracing::warn!(%reason, "torrent source not started");
                let _ = tx.send(Event::ProviderNote(reason));
                return;
            }
        };
        let registry = &services.registry;
        let snapshot =
            config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
        sources::place(registry, up.provider, &snapshot);
        if let Some(warning) = up.warning {
            let _ = tx.send(Event::Toast(Notice::info(warning)));
        }
        *vpn_guard = Some((up.guard, up.session));
        let check = Arc::clone(&self.check);
        *self =
            Self { generation, ..Self::start(vpn_guard, &services.store, config, tx, &check) };
        let _ = tx.send(Event::Providers(data::provider_rows(registry, &services.stores)));
        tracing::info!(providers = ?registry.ids(), "torrent source up");
    }

    /// Check where torrent traffic leaves from now, as Sources' Check Now asks, rather than at
    /// the guard's next check: `Checking`, then the verdict, on `Event::Vpn`; `None` there when
    /// torrents are off.
    ///
    /// What waits on the guard checks at once, so a failed check does what the guard's own
    /// does: the watcher halts the session, or a bring-up keeps waiting. With neither, as when
    /// the source could not come up for want of an index, the guard is checked on its own and
    /// nothing else follows from it.
    pub(crate) fn check_now(&self, config: &Config, services: &Services, tx: &Outbox) {
        if self.bring_forward(config, tx) {
            check_alone(&crate::protection::with_proxy_password(config, services), tx);
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

    /// Stop everything and halt the session before letting go of it.
    ///
    /// Halting first is the whole point: dropping the handles would leave librqbit
    /// seeding whatever it holds until the process exits, which for a source the user
    /// just turned off is precisely the traffic they meant to stop.
    async fn stop(&mut self) {
        // A bring-up still waiting for the guard has no session to halt, only a wait to end.
        if let Some(waiting) = self.bring_up.take() {
            waiting.abort();
        }
        if let Some(session) = self.session.take() {
            session.halt().await;
        }
        for task in
            [self.downloads.take(), self.watcher.take(), self.seeding.take(), self.cache.take()]
                .into_iter()
                .flatten()
        {
            task.abort();
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

/// Check the guard `config` describes on its own, with no session or bring-up to act on what it
/// finds, and say the verdict.
fn check_alone(config: &Config, tx: &Outbox) {
    let guard = match VpnGuard::new(config.providers.torrent.vpn.clone()) {
        Ok(guard) => guard,
        Err(reason) => {
            let _ = tx.send(Event::Vpn(Some(GuardView::Unreachable)));
            let _ = tx.send(Event::Toast(Notice::alert(format!("not checked: {reason}"))));
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
pub async fn reload_sources(
    services: &Services,
    torrent_tasks: &mut TorrentTasks,
    vpn_guard: &mut Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
    config: &SharedConfig,
    tx: &Outbox,
    arrivals: &mpsc::UnboundedSender<(u64, std::result::Result<sources::TorrentUp, String>)>,
) {
    // Old traffic stops before new traffic starts. The other order would leave two
    // sessions briefly alive, and one of them is the one being switched off.
    torrent_tasks.stop().await;
    *vpn_guard = None;
    let registry = &services.registry;

    let plugins = registry.of_kind(anistream_core::traits::ProviderKind::Plugin);
    let snapshot = config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
    let rebuilt = sources::build_registry_deferred(&snapshot, &services.http);
    let mut providers = rebuilt.all();
    providers.extend(plugins);
    registry.replace(providers);
    registry.set_disabled(snapshot.providers.disabled.iter().cloned());
    // The plugins were carried across behind the rebuilt sources; a plugin the order names
    // ahead of one of those goes back in front of it.
    sources::arrange(registry, &snapshot);

    torrent_tasks.bring_up(&snapshot, services, tx, arrivals);

    if !sources::torrent_wanted(&snapshot) {
        // No session: say so, and clear a badge that now describes nothing.
        let _ = tx.send(Event::Vpn(None));
    }
    let _ = tx.send(Event::ProviderNote(if sources::torrent_wanted(&snapshot) {
        "sources reloaded — torrents start once the VPN guard passes".into()
    } else {
        "torrents are off — the session was stopped".into()
    }));
    let _ = tx.send(Event::Providers(data::provider_rows(registry, &services.stores)));
    tracing::info!(providers = ?registry.ids(), "source chain reloaded");
}

#[cfg(test)]
mod tests {
    use anistream_core::config::VpnMode;

    use super::*;

    /// Torrents on, protected the way `mode` says, with the invented index they need.
    fn torrents(mode: VpnMode) -> Config {
        let mut config = Config::default();
        let torrent = &mut config.providers.torrent;
        torrent.enabled = true;
        torrent.rss_url = Some("https://index.example/rss".into());
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
        check_alone(&config, &tx);
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
        check_alone(&torrents(VpnMode::Socks5), &tx);
        let mut said = said(&mut rx);
        let Some(Event::Toast(notice)) = said.pop() else { panic!("{said:?}") };
        assert_eq!(badges(said), [Some(GuardView::Unreachable)]);
        assert!(notice.text.starts_with("not checked: socks5 mode requires"), "{notice:?}");
    }
}
