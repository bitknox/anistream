//! What a front end asks of the sources: how each of them is, checked now, and a plugin switched
//! off or back on.

use anistream_core::{settings::SettingValue, traits::ProviderKind};
use anistream_model::{event::Event, notice::Notice};

use crate::{
    data,
    dispatch::Lent,
    now_epoch,
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// Every source's condition, and the metadata ladder's beside them.
pub async fn check_providers(lent: Lent) {
    let Lent { meta, registry, stores, tx, .. } = lent;
    registry.check_all(now_epoch()).await;
    // The metadata ladder rides the same refresh: one screen, one key,
    // every source's condition.
    let _ = tx.send(Event::MetaStatus(data::meta_rows(&meta)));
    let update = Event::Providers(data::provider_rows(&registry, &stores));
    let _ = tx.send(update);
}

/// Switch the plugin `id` off, or back on, without removing it: written to `providers.disabled`
/// and put in effect on the chain at once, where it keeps its place and is not tried while off.
/// Said with the settings, the Providers rows and a notice. Only a plugin: the torrent source is
/// turned off in the settings, where turning it on asks how to protect it.
pub fn disable_plugin(
    id: String,
    disabled: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { registry, paths, stores, .. } = services;
    let Some(plugin) = registry.get(&id).filter(|p| p.manifest().kind == ProviderKind::Plugin)
    else {
        let _ = tx
            .send(Event::Toast(Notice::alert(format!("{id} is not a plugin that is loaded"))));
        return;
    };
    let name = plugin.manifest().display_name.clone();
    let mut off = engine.config.effective().providers.disabled.clone();
    off.retain(|entry| *entry != id);
    if disabled {
        off.push(id);
    }
    engine.config.effective_mut().providers.disabled.clone_from(&off);
    let written =
        engine.config.write(paths, &["providers"], "disabled", SettingValue::Strings(off));
    if written.is_err() {
        // No setting of the screen's to put back by name, so put back the file's list here.
        let saved = engine.config.saved().providers.disabled.clone();
        engine.config.effective_mut().providers.disabled = saved;
    }
    registry.set_disabled(engine.config.effective().providers.disabled.iter().cloned());
    registry.retry_down();
    crate::titles::refresh::sources_changed(engine, services, tx);
    let _ = tx.send(Event::Settings(engine.config.view()));
    let _ = tx.send(Event::Providers(data::provider_rows(registry, stores)));
    let landed = written.is_ok();
    let notice = match written {
        Ok(()) if disabled => {
            Notice::info(format!("{name} is off · skipped when looking for episodes"))
        }
        Ok(()) => Notice::info(format!("{name} is on")),
        Err(anistream_core::Error::Config(reason)) => {
            Notice::alert(format!("not saved: {reason}"))
        }
        Err(other) => Notice::alert(format!("not saved: {other}")),
    };
    let _ = tx.send(Event::Toast(notice));
    // Republished for the background tasks, as any setting written is.
    if landed {
        let _ = tx.internal(Internal::SettingsSaved);
    }
}
