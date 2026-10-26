//! Titles on screen, listed again when the sources they were listed from change.
//!
//! A front end asks for a title's episodes once and keeps the answer, so a source that was
//! switched on, moved up, pinned or installed would otherwise show only after the app was
//! restarted. The engine knows when the sources change, so it lists the titles the viewer
//! has been looking at again and says the new tables, which a front end takes as it takes any
//! table.

use anistream_core::ids::AnilistId;

use crate::{dispatch, outbox::Outbox, services::Services, state::EngineState};

/// How many titles are listed again: the ones the viewer has just been looking at. A page opened
/// later asks for itself and finds the sources as they now are.
const RECENT: usize = 3;

/// The titles whose episodes were listed lately, newest last.
#[derive(Debug, Default)]
pub struct Listed(Vec<AnilistId>);

impl Listed {
    /// Title `id`'s episodes were asked for.
    pub fn note(&mut self, id: AnilistId) {
        self.0.retain(|seen| *seen != id);
        self.0.push(id);
        if self.0.len() > RECENT {
            self.0.remove(0);
        }
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
        relist(id, engine, services, tx);
    }
}

/// Title `id` is listed again, with its dub and episode names asked afresh, when the viewer
/// has been looking at it. One never opened has nothing on screen to bring up to date, and
/// asks for itself when it is.
pub fn relist(id: AnilistId, engine: &mut EngineState, services: &Services, tx: &Outbox) {
    services.prefetch.clear();
    engine.dubs.forget(id);
    engine.names.forget(id);
    if !engine.listed.has(id) {
        return;
    }
    dispatch::list_episodes_again(id, engine, services, tx);
    engine.dubs.ask(id, engine.network.offline, services, tx);
}
