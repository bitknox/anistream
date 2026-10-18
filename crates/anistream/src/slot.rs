//! The player window kept between episodes.
//!
//! A playback ends at the end of one episode, but the window should not: the next episode, or
//! the one the viewer picked from the player's menu, loads into the same window, fullscreen and
//! all. So a finished playback that expects a successor *parks* its player here, showing the
//! loading card, and the next playback claims it instead of spawning a new process.
//!
//! Nothing guarantees a successor arrives — resolution can fail, or the viewer can walk away —
//! so a parked window that nobody claims within [`PARK_LIMIT`] is closed, and a playback that
//! knows it will not need it (a failed resolution, a watch party) releases it at once.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anistream_core::{ids::AnilistId, media::Translation};
use anistream_player::{MpvSession, PlaybackEvent, TrackPolicy, TrackPreferences};
use tokio::sync::mpsc::UnboundedReceiver;

/// How long a parked window waits for the next playback.
///
/// Generous, because finding a torrent's peers for the next episode can take most of a minute,
/// and closing the window just before the stream arrives would be the worst of both.
const PARK_LIMIT: Duration = Duration::from_secs(120);

/// A live player and the events it is reporting.
pub struct Player {
    pub session: MpvSession,
    pub events: UnboundedReceiver<PlaybackEvent>,
    /// The track policy travels with the process rather than the episode: mpv carries an
    /// explicit track id and the subtitle delay from one file into the next, so whatever
    /// chooses tracks has to know what the previous file left behind.
    pub tracks: TrackPolicy,
    /// Whose memory `tracks` holds — what the next file's leftovers are stored under.
    pub tracks_for: Option<(AnilistId, Translation)>,
}

impl Player {
    pub fn new(session: MpvSession, events: UnboundedReceiver<PlaybackEvent>) -> Self {
        let preferences = TrackPreferences { translation: Translation::Sub, language: None };
        Self { session, events, tracks: TrackPolicy::new(preferences, None), tracks_for: None }
    }
}

/// At most one parked player, shared by every playback the task runner starts.
#[derive(Clone, Default)]
pub struct PlayerSlot {
    parked: Arc<Mutex<Option<(u64, Player)>>>,
    generation: Arc<AtomicU64>,
}

impl PlayerSlot {
    /// Keep a player for whichever playback comes next.
    pub fn park(&self, player: Player) {
        let token = self.generation.fetch_add(1, Ordering::Relaxed);
        let replaced = self.lock().replace((token, player));
        if let Some((_, old)) = replaced {
            tokio::spawn(old.session.shutdown());
        }

        // The backstop. Matched on the token so a timer from an earlier park cannot close a
        // window parked after it.
        let slot = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(PARK_LIMIT).await;
            let expired = {
                let mut parked = slot.lock();
                if parked.as_ref().is_some_and(|(t, _)| *t == token) {
                    parked.take()
                } else {
                    None
                }
            };
            if let Some((_, player)) = expired {
                tracing::info!("no playback claimed the waiting player; closing it");
                player.session.shutdown().await;
            }
        });
    }

    /// Claim the parked player, if there is one and the viewer has not closed it meanwhile.
    pub fn take(&self) -> Option<Player> {
        let (_, mut player) = self.lock().take()?;
        if !player.session.is_alive() {
            return None;
        }
        // What arrived while it waited belongs to no playback: the tail of the last file, keys
        // pressed on the loading card.
        while player.events.try_recv().is_ok() {}
        Some(player)
    }

    /// Close the parked player: the playback it was waiting for is not coming.
    pub fn release(&self) {
        if let Some((_, player)) = self.lock().take() {
            tokio::spawn(player.session.shutdown());
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<(u64, Player)>> {
        // A poisoned lock means a panic mid-swap; the value is still a valid Option.
        self.parked.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
