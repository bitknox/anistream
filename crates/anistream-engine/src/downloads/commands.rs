//! What a front end asks of downloads: an episode queued, or several, and a download started
//! now, paused, cancelled, deleted, or cleared away once finished.

use anistream_core::ids::AnilistId;
use anistream_model::{event::Event, notice::Notice};

use crate::{
    downloads, jobs::JobGuard, outbox::Outbox, services::Services, state::EngineState,
};

/// Why title `id` cannot be downloaded at all, said before any source is asked: none is set up,
/// or none could list it when its episodes were last loaded, so they are AniList's alone.
fn refused(id: AnilistId, engine: &EngineState, services: &Services) -> Option<Notice> {
    let registry = &services.registry;
    if registry.is_empty() && !registry.is_arriving() {
        return Some(Notice::raw_alert(crate::playback::resolve::NO_SOURCES));
    }
    engine
        .ahead
        .unlisted(id)
        .then(|| Notice::raw_alert("no source could list this show, so it can't be downloaded"))
}

/// Queue one episode for download.
pub fn download_episode(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if let Some(notice) = refused(id, engine, services) {
        let _ = tx.send(Event::Toast(notice));
        return;
    }
    let Services { meta, store, registry, .. } = services;
    let config = &crate::show_settings::for_show(engine.config.effective(), store, id);
    let (store, registry, meta, tx) =
        (store.clone(), registry.clone(), meta.clone(), tx.clone());
    let wanted = downloads::Wanted::of(config);
    tokio::spawn(async move {
        let _job = job;
        // Resolving needs the provider chain, so this is a task rather than inline — the
        // same walk playback does, which is the point: a download is "play it, to disk".
        let queued = downloads::enqueue(&store, &registry, &meta, id, &episode, wanted, false);
        let update = match queued.await {
            Ok((row, downloads::Queued::New)) => Event::Toast(Notice::raw_info(format!(
                "queued {} ep {}",
                row.title, row.episode
            ))),
            Ok((row, downloads::Queued::Existing)) => Event::Toast(Notice::raw_info(format!(
                "{} ep {} is already in downloads",
                row.title, row.episode
            ))),
            Err(refusal) => Event::Toast(Notice::raw_alert(refusal.to_string())),
        };
        let _ = tx.send(update);
        let _ = tx.send(Event::Status(None));
        downloads::publish_now(&store, &tx);
    });
}

/// Queue several episodes for download, one after another.
pub fn download_many(
    id: AnilistId,
    episodes: Vec<String>,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if let Some(notice) = refused(id, engine, services) {
        let _ = tx.send(Event::Toast(notice));
        return;
    }
    let Services { meta, store, registry, .. } = services;
    let config = &crate::show_settings::for_show(engine.config.effective(), store, id);
    let (store, registry, meta, tx) =
        (store.clone(), registry.clone(), meta.clone(), tx.clone());
    let wanted = downloads::Wanted::of(config);
    tokio::spawn(async move {
        let _job = job;
        // Sequential enqueues, not parallel: each one is only a resolve + a queue
        // row, and the download manager's own concurrency cap paces the fetches.
        let outcome = downloads::enqueue_each(&episodes, |episode| {
            let (store, registry, meta, tx) = (&store, &registry, &meta, &tx);
            async move {
                let (_, queued) =
                    downloads::enqueue(store, registry, meta, id, &episode, wanted, false)
                        .await?;
                // Each row as it lands, so the Downloads screen fills while the rest
                // resolve rather than saying nothing is queued until the end.
                downloads::publish_now(store, tx);
                Ok(queued)
            }
        })
        .await;
        let _ = tx.send(Event::Toast(outcome.toast()));
    });
}

/// Pause a download, or carry on with one that was paused.
pub fn download_pause(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // Toggled here rather than in the reducer because the authoritative state is the row,
    // and the reducer only has the flattened projection of it.
    if let Ok(Some(row)) = store.downloads().map(|all| all.into_iter().find(|d| d.id == id)) {
        let next = match row.state {
            anistream_store::DownloadState::Paused => anistream_store::DownloadState::Queued,
            anistream_store::DownloadState::Done | anistream_store::DownloadState::Failed => {
                row.state
            }
            _ => anistream_store::DownloadState::Paused,
        };
        let _ = store.set_download_state(id, next);
    }
    downloads::publish_now(store, tx);
}

/// Start a waiting or paused download now: it goes to the front of the queue, and the manager
/// starts it on its next tick, the download latest in the queue waiting again if every place is
/// taken.
pub fn download_start_now(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    let row = store.downloads().ok().and_then(|all| all.into_iter().find(|d| d.id == id));
    let moved = store.download_to_front(id, anistream_store::now()).unwrap_or(false);
    downloads::publish_now(store, tx);
    if let (true, Some(row)) = (moved, row) {
        let _ = tx.send(Event::Toast(Notice::raw_info(format!(
            "starting {} ep {} now",
            row.title, row.episode
        ))));
    }
}

/// Cancel a download.
pub fn download_cancel(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // The row goes first: the manager notices it is gone on its next poll and drops the
    // torrent. Deleting the torrent from here would race with that loop.
    let _ = store.remove_download(id);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::raw_info("download cancelled")));
}

/// Delete a download and its file.
pub fn download_delete(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // Path first, row second: once the row is gone nothing else knows where the
    // file was. A running download's partial file is cleaned up by the manager's
    // own poll, and deleting here as well is a harmless no-op race.
    let path = store
        .downloads()
        .ok()
        .and_then(|all| all.into_iter().find(|d| d.id == id))
        .and_then(|d| d.path);
    let _ = store.remove_download(id);
    let note = match path {
        Some(path) => match std::fs::remove_file(&path) {
            Ok(()) => Notice::raw_info("download and file deleted"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Notice::raw_info("download removed — file was already gone")
            }
            Err(e) => Notice::raw_alert(format!("removed the entry, not the file: {e}")),
        },
        None => Notice::raw_info("download removed"),
    };
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(note));
}

/// Clear the finished downloads from the list.
pub fn download_clear_completed(services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    let cleared = store.clear_completed_downloads().unwrap_or(0);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::raw_info(format!("cleared {cleared} finished"))));
}
