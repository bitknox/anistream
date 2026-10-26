//! Titles on screen, listed again when the sources they were listed from change.
//!
//! A front end asks for a title's episodes once and keeps the answer, so a source that was
//! switched on, moved up, pinned or installed would otherwise show only after the app was
//! restarted. The engine knows when the sources change, so it lists the titles the viewer
//! has been looking at again and says the new tables, which a front end takes as it takes any
//! table.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anistream_core::ids::AnilistId;
use anistream_model::event::Event;

use crate::{dispatch, outbox::Outbox, services::Services, state::EngineState};

/// How many titles are listed again: the ones the viewer has just been looking at. A page opened
/// later asks for itself and finds the sources as they now are.
const RECENT: usize = 3;

/// The titles whose episodes were listed lately, newest last, and which listing of each is the
/// current one.
#[derive(Debug, Default)]
pub struct Listed(Vec<AnilistId>, Generations, Generations);

/// Which listing of each title is the newest. A load, a pin and a source change each start one,
/// and a slow earlier one must not land over a newer table.
#[derive(Debug, Clone, Default)]
struct Generations(Arc<Mutex<HashMap<AnilistId, u64>>>);

/// A listing's claim to speak for its title: good until a newer listing of it starts.
#[derive(Debug, Clone)]
pub struct Ticket {
    id: AnilistId,
    n: u64,
    generations: Generations,
}

impl Ticket {
    /// No newer listing of the title has started.
    pub fn current(&self) -> bool {
        self.generations.0.lock().unwrap().get(&self.id) == Some(&self.n)
    }

    /// Send `event` if this listing is still the newest. Returns whether it was sent.
    pub fn send(&self, tx: &Outbox, event: Event) -> bool {
        if !self.current() {
            return false;
        }
        let _ = tx.send(event);
        true
    }

    /// A ticket that is always current, for a caller with no newer listing to lose to.
    #[cfg(test)]
    pub fn standalone(id: AnilistId) -> Self {
        let generations = Generations::default();
        generations.0.lock().unwrap().insert(id, 0);
        Self { id, n: 0, generations }
    }
}

impl Listed {
    /// Title `id`'s episodes were asked for.
    pub fn note(&mut self, id: AnilistId) {
        self.0.retain(|seen| *seen != id);
        self.0.push(id);
        if self.0.len() > RECENT {
            self.0.remove(0);
        }
    }

    /// Start a listing of title `id`, which puts every earlier one out of date.
    pub fn ticket(&self, id: AnilistId) -> Ticket {
        let mut map = self.1.0.lock().unwrap();
        let n = map.entry(id).or_insert(0);
        *n += 1;
        Ticket { id, n: *n, generations: self.1.clone() }
    }

    /// Start a source-options listing of title `id`: only the newest answers, since the
    /// slowest source sets how long one takes and an older one may come back after a newer.
    pub fn options_ticket(&self, id: AnilistId) -> Ticket {
        let mut map = self.2.0.lock().unwrap();
        let n = map.entry(id).or_insert(0);
        *n += 1;
        Ticket { id, n: *n, generations: self.2.clone() }
    }

    pub fn has(&self, id: AnilistId) -> bool {
        self.0.contains(&id)
    }

    pub fn ids(&self) -> Vec<AnilistId> {
        self.0.clone()
    }
}

/// The sources changed: order, which are on, one installed, updated or removed, or the torrent
/// source joined. Whatever was found against the old sources is dropped, and the titles
/// shown are listed again.
pub fn sources_changed(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    // A next episode found ahead was found from the old sources.
    services.prefetch.clear();
    for id in engine.listed.ids() {
        relist(id, false, engine, services, tx);
    }
}

/// Title `id` is listed again, with its dub and episode names asked afresh, when the viewer
/// has been looking at it. One never opened has nothing on screen to bring up to date, and
/// asks for itself when it is.
pub fn relist(
    id: AnilistId,
    announce: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    services.prefetch.clear();
    engine.dubs.forget(id);
    engine.names.forget(id);
    if !engine.listed.has(id) {
        return;
    }
    dispatch::list_episodes_again(id, announce, engine, services, tx);
    engine.dubs.ask(id, engine.network.offline, services, tx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_newest_listing_of_a_title_may_speak() {
        let listed = Listed::default();
        let id = AnilistId::new(1);
        let slow = listed.ticket(id);
        assert!(slow.current());
        let pinned = listed.ticket(id);
        assert!(!slow.current(), "a newer listing started");
        assert!(pinned.current());
        assert!(listed.ticket(AnilistId::new(2)).current());
        assert!(pinned.current(), "another title's listing is no one's to supersede");
        // Options listings are counted apart from episode listings.
        let options = listed.options_ticket(id);
        assert!(pinned.current() && options.current());
    }
}
