//! What a playback writes down: the tracker's actions carried out, a history row written
//! before anything is queued for a tracker, progress queued the first time an episode
//! completes, and the series' track choices kept.

use anistream_core::{ids::AnilistId, media::Translation, stream::Stream};
use anistream_model::event::Event;
use anistream_playback::{Action, PlaybackEvent, PlaybackTracker, TrackAction};
use anistream_store::{Store, WatchEvent};

use super::Run;
use crate::playback::link::PlayerLink;

impl Run<'_> {
    /// Carry out what the track policy decided.
    pub(super) async fn apply_track(&self, action: TrackAction, link: &dyn PlayerLink) {
        if let TrackAction::Remember(memory) = action {
            let key = (self.context.anilist_id, self.context.translation);
            remember_tracks(self.store, self.settings.remember_tracks, key, memory);
            return;
        }
        if let Some((name, value)) = action.property() {
            let _ = link.set(name, value).await;
        }
    }

    /// Carry out a tracker action that is not about skipping.
    pub(super) async fn act(
        &self,
        action: Action,
        watched_secs: f64,
        link: &dyn PlayerLink,
        stream: &Stream,
        committed: &mut bool,
        ending: &mut Option<bool>,
    ) {
        let context = self.context;
        let tx = self.tx;
        match action {
            Action::Record { position, duration, completed } => {
                let watch = WatchEvent {
                    duration_secs: duration,
                    watched_secs,
                    provider_id: Some(stream.provider_id.clone()),
                    translation: Some(context.translation),
                    completed,
                    ..WatchEvent::new(context.anilist_id, &context.episode, position)
                };
                self.record(watch, completed, committed).await;
            }
            Action::ProbeChapters => {
                if let Err(e) = link.restate_chapters().await {
                    tracing::debug!(error = %e, "could not ask for the chapter list");
                }
            }
            Action::MarkChapters(marks) => {
                // Best effort by design: a player that will not take them plays the
                // episode exactly as it did before, and the skip prompt still works.
                // A write to `chapter-list` on a loaded file is what puts the markers on the seek
                // bar and under chapter navigation, with no file to write and clean up. Markers
                // past the runtime are dropped rather than refused, so an ending whose boundary
                // runs off the end of a slightly short encode still lands.
                let list: Vec<serde_json::Value> = marks
                    .iter()
                    .map(|(title, time)| serde_json::json!({ "title": title, "time": time }))
                    .collect();
                if let Err(e) = link.set("chapter-list", list.into()).await {
                    tracing::debug!(error = %e, "player would not take chapter markers");
                }
            }
            Action::RememberSpeed(speed) => {
                let _ = tx.send(Event::PlaybackSpeed(speed));
            }
            Action::RememberVolume(volume) => {
                let _ = tx.send(Event::PlaybackVolume(volume));
            }
            Action::Finished { watched } => {
                // Recorded, not announced — see `ending`. mpv reports an ending on the way out
                // whether it played or not, so this fires for a dead stream too.
                *ending = Some(watched);
                if watched {
                    tracing::info!(
                        episode = %context.episode,
                        "episode finished and recorded as watched"
                    );
                }
            }
            Action::OfferSkip { .. } | Action::ClearSkip => {}
        }
    }

    /// Feed one event through the tracker purely for its bookkeeping.
    pub(super) async fn record_actions(
        &self,
        tracker: &mut PlaybackTracker,
        event: &PlaybackEvent,
        stream: &Stream,
        committed: &mut bool,
        ending: &mut Option<bool>,
    ) {
        for action in tracker.observe(event) {
            if let Action::Record { position, duration, completed } = action {
                let watch = WatchEvent {
                    duration_secs: duration,
                    watched_secs: tracker.watched_secs(),
                    provider_id: Some(stream.provider_id.clone()),
                    translation: Some(self.context.translation),
                    completed,
                    ..WatchEvent::new(self.context.anilist_id, &self.context.episode, position)
                };
                self.record(watch, completed, committed).await;
            } else if let Action::Finished { watched } = action {
                *ending = Some(watched);
            }
        }
    }

    /// Write a history row, and queue tracker progress the first time the episode completes.
    async fn record(&self, watch: WatchEvent, completed: bool, committed: &mut bool) {
        // Local first, always. The history row is written before anything is queued for a
        // tracker, so a failed push can never cost you the watch.
        let queue_now = completed && !*committed;
        if queue_now {
            *committed = true;
        }
        let store_handle = self.store.clone();
        let trackers = self.settings.tracker_ids.clone();
        let id = self.context.anilist_id;
        // Blocking sqlite calls: off the async worker so a slow disk cannot stall the event
        // stream.
        let _ = tokio::task::spawn_blocking(move || {
            store_handle.record_event(&watch)?;
            if queue_now {
                anistream_track::sync::queue_progress_for(
                    &store_handle,
                    &trackers,
                    id,
                    anistream_store::now(),
                );
            }
            Ok::<_, anistream_store::StoreError>(())
        })
        .await;

        if queue_now {
            let _ = self.tx.send(Event::ProgressQueued);
        }
    }
}

/// Store a series' track memory, off the async worker.
pub(super) fn remember_tracks(
    store: &Store,
    enabled: bool,
    (id, translation): (AnilistId, Translation),
    memory: anistream_playback::tracks::TrackMemory,
) {
    if !enabled {
        return;
    }
    let store = store.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = store.remember_tracks(id, translation, &memory) {
            tracing::warn!(error = %e, "could not remember track choices");
        }
    });
}
