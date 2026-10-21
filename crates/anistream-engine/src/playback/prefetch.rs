//! The next episode's streams, found while the current one's ending plays, for the play that
//! follows to take rather than look for them again.

use std::{sync::Arc, time::Duration};

use anistream_core::ids::AnilistId;
use tokio::sync::mpsc;

use crate::{
    outbox::Outbox,
    playback::resolve::{Resolved, resolve_for_playback},
    services::Services,
    state::EngineState,
};

/// How long streams found ahead stay usable. Long enough to cover a viewer who lets the ending
/// and a post-credits scene play out; short enough that a signed stream URL is still good.
const PREFETCH_FRESH: Duration = Duration::from_secs(15 * 60);

/// The next episode's streams, found while the current one's ending plays.
///
/// One entry: only the episode about to play is worth holding, and a newer prefetch replaces
/// an older one. Taken, not copied, by the playback it was for.
#[derive(Clone, Default)]
pub struct Prefetch(Arc<std::sync::Mutex<Option<Prefetched>>>);

struct Prefetched {
    id: anistream_core::ids::AnilistId,
    episode: String,
    translation: anistream_core::media::Translation,
    found: std::time::Instant,
    resolved: Resolved,
}

impl Prefetch {
    pub fn keep(
        &self,
        id: anistream_core::ids::AnilistId,
        episode: &str,
        translation: anistream_core::media::Translation,
        resolved: Resolved,
    ) {
        tracing::info!(%episode, streams = resolved.0.len(), "next episode found ahead");
        let entry = Prefetched {
            id,
            episode: episode.to_owned(),
            translation,
            found: std::time::Instant::now(),
            resolved,
        };
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(entry);
    }

    /// The streams found ahead for exactly this episode, translation and title, if still fresh.
    pub fn take(
        &self,
        id: anistream_core::ids::AnilistId,
        episode: &str,
        translation: anistream_core::media::Translation,
    ) -> Option<Resolved> {
        let mut slot = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // Only a match is taken. Any other play — a show opened in passing, a source picked
        // for a different episode — used to take the entry with it, so the streams found for
        // the episode about to roll on were gone by the time it did.
        let entry = slot.as_ref()?;
        let fresh = entry.found.elapsed() < PREFETCH_FRESH;
        let matches =
            entry.id == id && entry.episode == episode && entry.translation == translation;
        if !fresh {
            *slot = None;
            return None;
        }
        if !matches {
            return None;
        }
        slot.take().map(|entry| entry.resolved)
    }
}

/// Find the next episode's streams while this one's ending plays, for the play that follows to
/// take.
pub fn prefetch(id: AnilistId, episode: String, engine: &EngineState, services: &Services) {
    let Services { meta, store, registry, prefetch, .. } = services;
    let config = engine.config.effective();
    let (meta, store, registry, playback, prefetch) = (
        meta.clone(),
        store.clone(),
        registry.clone(),
        config.playback.clone(),
        prefetch.clone(),
    );
    tokio::spawn(async move {
        // Quiet: the steps of a search nobody is waiting on must not take the status
        // line from the episode playing. Nothing reads this channel.
        let quiet = Outbox::from(mpsc::unbounded_channel().0);
        let found = resolve_for_playback(
            &meta, &store, &registry, id, &episode, None, &playback, None, &quiet,
        )
        .await;
        match found {
            Ok(resolved) => prefetch.keep(id, &episode, playback.translation, resolved),
            Err(reason) => {
                tracing::info!(%reason, "could not find the next episode ahead")
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback;

    #[test]
    fn streams_found_ahead_serve_only_the_episode_they_were_found_for() {
        use anistream_core::{ids::AnilistId, media::Translation};
        let context = |episode: &str| playback::PlaybackContext {
            anilist_id: AnilistId::new(7),
            mal_id: None,
            episode: episode.into(),
            title: "t".into(),
            translation: Translation::Sub,
            resume_at: None,
            runtime_secs: None,
            speed: None,
            volume: None,
            next: None,
            episodes: Vec::new(),
            cover_url: None,
            banner_url: None,
        };
        let found = || (Vec::new(), context("6"));
        let prefetch = Prefetch::default();

        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_some());
        assert!(
            prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none(),
            "taken once"
        );

        // Another episode, the other translation or another title start from scratch.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "8", Translation::Sub).is_none());
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Dub).is_none());
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(9), "6", Translation::Sub).is_none());

        // Too old to trust a signed stream URL.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        if let Some(entry) = prefetch.0.lock().unwrap().as_mut() {
            entry.found = std::time::Instant::now() - PREFETCH_FRESH - Duration::from_secs(1);
        }
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none());

        // Any other play leaves them for the episode they are for: a show opened in passing
        // while the ending ran used to take them with it.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(9), "1", Translation::Sub).is_none());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Dub).is_none());
        assert!(
            prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_some(),
            "still there"
        );
    }
}
