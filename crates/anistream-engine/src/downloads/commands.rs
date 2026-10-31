//! What a front end asks of downloads: an episode queued, or several, and a download started
//! now, paused, cancelled, deleted, or cleared away once finished.

use anistream_core::ids::AnilistId;
use anistream_model::{Cause, Msg, event::Event, notice::Notice};

use crate::{
    downloads, jobs::JobGuard, outbox::Outbox, services::Services, state::EngineState,
};

/// Why title `id` cannot be downloaded at all, said before any source is asked: none is set up,
/// or none could list it when its episodes were last loaded, so they are AniList's alone.
fn refused(id: AnilistId, engine: &EngineState, services: &Services) -> Option<Notice> {
    let registry = &services.registry;
    if registry.is_empty() && !registry.is_arriving() {
        return Some(Notice::alert(crate::playback::resolve::explained(
            crate::playback::resolve::no_sources(),
        )));
    }
    engine.ahead.unlisted(id).then(|| {
        Notice::alert(crate::playback::resolve::explained(Cause::NoSourceCouldListShow))
    })
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
            Ok((row, downloads::Queued::New)) => {
                Event::Toast(Notice::info(Msg::NoticeDownloadQueued {
                    title: row.title,
                    episode: row.episode,
                }))
            }
            Ok((row, downloads::Queued::Existing)) => {
                Event::Toast(Notice::info(Msg::NoticeDownloadAlreadyQueued {
                    title: row.title,
                    episode: row.episode,
                }))
            }
            Err(refusal) => Event::Toast(Notice::alert(crate::playback::resolve::explained(
                refusal.into_cause(),
            ))),
        };
        let _ = tx.send(update);
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
        let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeDownloadStartingNow {
            title: row.title,
            episode: row.episode,
        })));
    }
}

/// Cancel a download.
pub fn download_cancel(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    cancel(store, id);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeDownloadCancelled)));
}

/// Remove download `id`'s row, and the part of its file fetched so far.
///
/// A running download's file is the manager's to remove: it notices the row is gone on its next
/// poll and drops the torrent with its file. Deleting the torrent from here would race with that
/// loop. One paused or waiting may have no torrent in this process, as after a restart, so
/// nothing else would remove its file: that goes here, unless another row names the same file.
/// A finished download's file is kept; deleting it is [`download_delete`].
fn cancel(store: &anistream_store::Store, id: i64) {
    use anistream_store::DownloadState;
    let rows = store.downloads().unwrap_or_default();
    let row = rows.iter().find(|d| d.id == id).cloned();
    let _ = store.remove_download(id);
    let Some(row) = row else { return };
    let idle = matches!(
        row.state,
        DownloadState::Paused | DownloadState::Queued | DownloadState::Failed
    );
    let Some(path) = row.path.filter(|_| idle) else { return };
    if rows.iter().any(|d| d.id != id && d.path.as_deref() == Some(path.as_str())) {
        return;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => tracing::info!(row = id, "cancelled download's partial file removed"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(row = id, error = %e, "cancelled download's partial file kept")
        }
    }
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
            Ok(()) => Notice::info(Msg::NoticeDownloadFileDeleted),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Notice::info(Msg::NoticeDownloadRemovedFileGone)
            }
            Err(e) => {
                Notice::alert(Msg::NoticeDownloadRemovedFileKept { reason: e.to_string() })
            }
        },
        None => Notice::info(Msg::NoticeDownloadRemoved),
    };
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(note));
}

/// Clear the finished downloads from the list.
pub fn download_clear_completed(services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    let cleared = store.clear_completed_downloads().unwrap_or(0);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeDownloadsCleared {
        count: cleared as i64,
    })));
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_store::{DownloadState, Store};

    /// A row for `episode` with part of `file` fetched, in `state`, the file in `dir`.
    fn part(
        store: &Store,
        dir: &std::path::Path,
        (episode, file): (&str, &str),
        state: DownloadState,
    ) -> (i64, std::path::PathBuf) {
        let row = store
            .enqueue_download(
                AnilistId::new(900_002),
                episode,
                "The Library Above the Clouds",
                "magnet:?xt=urn:btih:0",
            )
            .unwrap();
        let path = dir.join(file);
        std::fs::write(&path, b"part of an episode").unwrap();
        store
            .update_download_progress(row.id, 18, 100, Some(&path.display().to_string()))
            .unwrap();
        store.set_download_state(row.id, state).unwrap();
        (row.id, path)
    }

    #[test]
    fn cancelling_a_paused_download_removes_what_it_fetched() {
        let (store, dir) = (Store::open_in_memory().unwrap(), tempfile::tempdir().unwrap());
        let (paused, paused_file) =
            part(&store, dir.path(), ("3", "Episode 03.mkv"), DownloadState::Paused);
        let (waiting, waiting_file) =
            part(&store, dir.path(), ("4", "Episode 04.mkv"), DownloadState::Queued);
        cancel(&store, paused);
        cancel(&store, waiting);
        assert!(store.downloads().unwrap().is_empty());
        assert!(!paused_file.exists(), "a paused download's partial file outlived its row");
        assert!(!waiting_file.exists(), "a waiting download's partial file outlived its row");
    }

    #[test]
    fn cancelling_keeps_a_finished_file_and_one_another_row_names() {
        let (store, dir) = (Store::open_in_memory().unwrap(), tempfile::tempdir().unwrap());
        let (done, done_file) =
            part(&store, dir.path(), ("5", "Episode 05.mkv"), DownloadState::Done);
        cancel(&store, done);
        assert!(done_file.exists(), "removing a finished row from the list deleted its file");

        let (first, shared) =
            part(&store, dir.path(), ("6", "Season pack.mkv"), DownloadState::Paused);
        let (_second, _) =
            part(&store, dir.path(), ("7", "Season pack.mkv"), DownloadState::Queued);
        cancel(&store, first);
        assert!(shared.exists(), "a file another row names went with the first");
    }
}
