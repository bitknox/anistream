//! A playback's seat at a watch party: what the party tells the player to do, and what the
//! player reports back. Built in every build, so the session needs no feature of its own; only
//! the party behind it is the `party` feature's (see `crate::party`).
//!
//! The party opens the [`PartyHub`] when it starts and every playback takes a seat from it, at
//! the start of each file or the moment a party opens mid-file. A seat lasts until the party
//! ends, when its cues stop and the playback goes on as a private session, or until the
//! playback does, when it says how it ended.

use std::sync::{Arc, Mutex};

use anistream_core::ids::AnilistId;
use tokio::sync::{mpsc, watch};

/// What the party asks of the player.
#[derive(Debug, Clone, PartialEq)]
pub enum SeatCue {
    Pause(bool),
    /// Seek to this position, in seconds.
    Seek(f64),
    Speed(f64),
    /// Show this on the player, as a notice.
    Notice(String),
}

/// What a playback tells the party.
#[derive(Debug, Clone, PartialEq)]
pub enum SeatReport {
    /// This file plays now: the episode, the name the room sees for it, and its length once
    /// known. Said again when the length becomes known.
    Playing { id: AnilistId, episode: String, name: String, duration: Option<f64> },
    /// Where the player is.
    Sample { position: f64, paused: bool, speed: f64 },
    /// The playback ended. `leave` when the viewer stopped it or closed its window, which
    /// leaves the party; an episode that played to its end, or failed, stays in it.
    Ended { leave: bool },
}

/// What reaches the party from its seats.
#[derive(Debug)]
pub enum SeatMessage {
    /// A playback took a seat: its cues go here, in place of any seat before.
    Seated(mpsc::UnboundedSender<SeatCue>),
    Report(SeatReport),
}

/// Where playbacks find the party, when there is one. Cheap to clone; every clone is the same
/// hub.
#[derive(Clone)]
pub struct PartyHub {
    party: Arc<Mutex<Option<mpsc::UnboundedSender<SeatMessage>>>>,
    /// Counts the parties opened, for a playback under way to take a seat at a new one.
    opened: Arc<watch::Sender<u64>>,
}

impl Default for PartyHub {
    fn default() -> Self {
        Self { party: Arc::default(), opened: Arc::new(watch::channel(0).0) }
    }
}

impl std::fmt::Debug for PartyHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PartyHub").field("open", &self.is_open()).finish()
    }
}

impl PartyHub {
    /// A party starts: every playback takes a seat from now on, the one under way too. What
    /// the seats send arrives on the receiver; the party ends for them when it is dropped.
    pub fn open(&self) -> mpsc::UnboundedReceiver<SeatMessage> {
        let (tx, rx) = mpsc::unbounded_channel();
        *self.lock() = Some(tx);
        self.opened.send_modify(|count| *count += 1);
        rx
    }

    /// Whether a party is going.
    pub fn is_open(&self) -> bool {
        self.lock().as_ref().is_some_and(|tx| !tx.is_closed())
    }

    /// A seat at the party, when one is going.
    pub fn seat(&self) -> Option<Seat> {
        let party = self.lock().clone().filter(|tx| !tx.is_closed())?;
        let (cues, cue_rx) = mpsc::unbounded_channel();
        party.send(SeatMessage::Seated(cues)).ok()?;
        Some(Seat { cues: cue_rx, party })
    }

    /// Changes whenever a party opens.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.opened.subscribe()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<mpsc::UnboundedSender<SeatMessage>>> {
        self.party.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// One playback's seat. See the module's documentation.
#[derive(Debug)]
pub struct Seat {
    cues: mpsc::UnboundedReceiver<SeatCue>,
    party: mpsc::UnboundedSender<SeatMessage>,
}

impl Seat {
    pub fn report(&self, report: SeatReport) {
        let _ = self.party.send(SeatMessage::Report(report));
    }

    /// The next cue; `None` once the party is over, or another playback took the seat.
    pub async fn cue(&mut self) -> Option<SeatCue> {
        self.cues.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seats_are_taken_only_while_a_party_is_going() {
        let hub = PartyHub::default();
        assert!(hub.seat().is_none(), "no party, no seat");
        let mut changes = hub.changes();
        let mut party = hub.open();
        assert!(changes.has_changed().unwrap(), "a playback under way hears of it");
        changes.borrow_and_update();

        let mut seat = hub.seat().expect("a party is going");
        let Some(SeatMessage::Seated(cues)) = party.recv().await else { panic!("no seat") };
        seat.report(SeatReport::Sample { position: 1.0, paused: false, speed: 1.0 });
        assert!(matches!(
            party.recv().await,
            Some(SeatMessage::Report(SeatReport::Sample { .. }))
        ));
        cues.send(SeatCue::Pause(true)).unwrap();
        assert_eq!(seat.cue().await, Some(SeatCue::Pause(true)));

        // The party ends: its cues stop, and no seat is taken after it.
        drop(cues);
        drop(party);
        assert_eq!(seat.cue().await, None);
        assert!(!hub.is_open());
        assert!(hub.seat().is_none());
    }
}
