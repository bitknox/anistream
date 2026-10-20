//! The torrent stack as the settings stand it up and down: the tasks that run beside a torrent
//! session once the VPN guard passes, and the source chain rebuilt when a setting changes which
//! sources there are.

use std::{sync::Arc, time::Duration};

use anistream_core::config::Config;
use anistream_model::{event::Event, notice::Notice};
use anistream_providers::vpn::VpnGuard;
use anistream_store::Store;
use tokio::sync::mpsc;

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
}

impl TorrentTasks {
    fn start(
        guard: &Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
        store: &Store,
        config: &SharedConfig,
        tx: &Outbox,
    ) -> Self {
        let Some((guard, session)) = guard.clone() else {
            // No session means no downloads, which is correct rather than a limitation:
            // every downloadable source here is a torrent.
            return Self::default();
        };

        let downloads =
            downloads::spawn(store.clone(), session.clone(), Arc::clone(config), tx.clone());
        downloads::publish_now(store, tx);

        let _ = tx.send(Event::Vpn(Some(sources::guard_view(&guard.state()))));
        let watcher = tokio::spawn(vpn_watch(guard, session.clone(), tx.clone()));
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
        let (config, http, paths, store, tx, arrivals) = (
            config.clone(),
            services.http.clone(),
            services.paths.clone(),
            services.store.clone(),
            tx.clone(),
            arrivals.clone(),
        );
        self.bring_up = Some(tokio::spawn(async move {
            let outcome = sources::bring_up_torrent(config, http, paths, store, tx).await;
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
        *self = Self { generation, ..Self::start(vpn_guard, &services.store, config, tx) };
        let _ = tx.send(Event::Providers(data::provider_rows(registry)));
        tracing::info!(providers = ?registry.ids(), "torrent source up");
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
    let _ = tx.send(Event::Providers(data::provider_rows(registry)));
    tracing::info!(providers = ?registry.ids(), "source chain reloaded");
}
