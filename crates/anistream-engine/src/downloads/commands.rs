//! What a front end asks of downloads: an episode queued, or several, and a download paused,
//! cancelled, deleted, or cleared away once finished.

use anistream_core::ids::AnilistId;
use anistream_model::{event::Event, notice::Notice};

use crate::{
    downloads, jobs::JobGuard, outbox::Outbox, services::Services, state::EngineState,
};

/// Queue one episode for download.
pub fn download_episode(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { meta, store, registry, .. } = services;
    let config = &crate::show_settings::for_show(engine.config.effective(), store, id);
    let (store, registry, meta, tx) =
        (store.clone(), registry.clone(), meta.clone(), tx.clone());
    let wanted = downloads::Wanted::of(config);
    tokio::spawn(async move {
        let _job = job;
        // Resolving needs the provider chain, so this is a task rather than inline — the
        // same walk playback does, which is the point: a download is "play it, to disk".
        let update = match downloads::enqueue(&store, &registry, &meta, id, &episode, wanted)
            .await
        {
            Ok((row, downloads::Queued::New)) => {
                Event::Toast(Notice::info(format!("queued {} ep {}", row.title, row.episode)))
            }
            Ok((row, downloads::Queued::Existing)) => Event::Toast(Notice::info(format!(
                "{} ep {} is already in downloads",
                row.title, row.episode
            ))),
            Err(refusal) => Event::Toast(Notice::alert(refusal.to_string())),
        };
        let _ = tx.send(update);
        let _ = tx.send(Event::Status(String::new()));
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
                    downloads::enqueue(store, registry, meta, id, &episode, wanted).await?;
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

/// Cancel a download.
pub fn download_cancel(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // The row goes first: the manager notices it is gone on its next poll and drops the
    // torrent. Deleting the torrent from here would race with that loop.
    let _ = store.remove_download(id);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::info("download cancelled")));
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
            Ok(()) => Notice::info("download and file deleted"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Notice::info("download removed — file was already gone")
            }
            Err(e) => Notice::alert(format!("removed the entry, not the file: {e}")),
        },
        None => Notice::info("download removed"),
    };
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(note));
}

/// Clear the finished downloads from the list.
pub fn download_clear_completed(services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    let cleared = store.clear_completed_downloads().unwrap_or(0);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::info(format!("cleared {cleared} finished"))));
}
