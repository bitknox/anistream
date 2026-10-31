//! The built-in demo source: switched on and off, and where it stands for Home, Search and
//! Sources.
//!
//! It is a native source with no plugin file, no store and no setting but `[sources] demo`. A
//! fresh install has it on; an existing one is offered it. It never writes to a tracker: see
//! `Store::enqueue`.

use anistream_core::{
    demo::{DEMO_ANILIST_ID, DEMO_SOURCE_ID},
    settings::SettingValue,
};
use anistream_model::{
    Msg,
    event::Event,
    notice::{Notice, NoticeHint},
    sources::DemoState,
};

use crate::{
    data,
    outbox::{Internal, Outbox},
    services::Services,
    sources,
    state::EngineState,
};

/// Where the demo stands now.
pub fn state(engine: &EngineState, services: &Services) -> DemoState {
    let on = engine.config.effective().sources.demo;
    let other_source = services.registry.usable_ids().iter().any(|id| id != DEMO_SOURCE_ID);
    DemoState {
        on,
        offer: !on,
        only: on && !other_source,
        played: services.store.progress(DEMO_ANILIST_ID).ok().flatten().is_some(),
    }
}

/// Add the demo to the chain, or take it out, and write the choice. Said with the demo's state,
/// the sources, the settings, and a notice when it was removed.
pub fn set(on: bool, engine: &mut EngineState, services: &Services, tx: &Outbox) {
    let Services { registry, paths, stores, .. } = services;
    if engine.config.effective().sources.demo == on {
        let _ = tx.send(Event::Demo(state(engine, services)));
        return;
    }
    engine.config.effective_mut().sources.demo = on;
    let written = engine.config.write(paths, &["sources"], "demo", SettingValue::Bool(on));
    if let Err(error) = written {
        tracing::warn!(%error, "could not write the demo's choice");
        let message = match error {
            anistream_core::Error::Config(reason) => reason,
            other => other.to_string(),
        };
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeNotSaved { reason: message })));
        let _ = tx.send(Event::Settings(engine.config.view()));
        let _ = tx.send(Event::Demo(state(engine, services)));
        return;
    }
    if on {
        sources::place(
            registry,
            anistream_providers::demo::DemoProvider::shared(),
            engine.config.effective(),
        );
    } else {
        registry.remove(DEMO_SOURCE_ID);
    }
    crate::titles::refresh::sources_changed(engine, services, tx);
    let _ = tx.send(Event::Settings(engine.config.view()));
    let _ = tx.send(Event::Providers(data::provider_rows(registry, stores)));
    let _ = tx.send(Event::Demo(state(engine, services)));
    let _ = tx.internal(Internal::SettingsSaved);
    if !on {
        let notice =
            Notice::info(Msg::SourcesDemoRemoved).with_hint(NoticeHint::UndoRemoveDemo);
        let _ = tx.send(Event::Toast(notice));
    }
}
