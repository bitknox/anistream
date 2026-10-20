//! The Discord presence for one playback: connected for exactly as long as something plays,
//! and nothing at all wherever it is turned off or Discord is not running.

use crate::playback::PlaybackContext;

/// The Discord presence for one playback, if there is one.
///
/// A wrapper rather than an `Option<Presence>` at every call site: presence is decoration, so every
/// operation on it has to be a no-op when it is absent, and threading that check through the
/// playback loop would put five `if let` blocks around something that cannot fail usefully.
pub(super) struct Rpc {
    inner: Option<anistream_player::Presence>,
    activity: anistream_player::Activity,
}

impl Rpc {
    pub(super) async fn start(
        config: &anistream_core::config::PresenceConfig,
        context: &PlaybackContext,
    ) -> Self {
        let activity = anistream_player::Activity {
            // Honouring `show_title` here rather than at send time, so a build that leaks it is a
            // visible mistake rather than a missing branch somewhere downstream.
            title: if config.show_title {
                context.title.clone()
            } else {
                "Watching anime".to_string()
            },
            detail: if config.show_title {
                format!("Episode {}", context.episode)
            } else {
                String::new()
            },
            paused: false,
            started_at: Some(anistream_store::now()),
        };

        let inner = match (config.enabled, config.resolved_client_id()) {
            (true, Some(client_id)) => anistream_player::Presence::connect(client_id).await,
            (true, None) => {
                tracing::info!("presence enabled but no client id is configured; skipping");
                None
            }
            _ => None,
        };
        let mut rpc = Self { inner, activity };
        rpc.push().await;
        rpc
    }

    pub(super) async fn update(&mut self, paused: bool) {
        if self.inner.is_none() || self.activity.paused == paused {
            return;
        }
        self.activity.paused = paused;
        self.push().await;
    }

    async fn push(&mut self) {
        let Some(presence) = self.inner.as_mut() else { return };
        if let Err(e) = presence.set(&self.activity).await {
            // Dropped rather than retried: a broken pipe means Discord went away, and reconnecting
            // in a playback loop is effort spent on decoration.
            tracing::debug!(error = %e, "discord presence dropped");
            self.inner = None;
        }
    }

    pub(super) async fn finish(&mut self) {
        if let Some(presence) = self.inner.as_mut() {
            let _ = presence.clear().await;
        }
        self.inner = None;
    }
}
