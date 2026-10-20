//! Settings: the config the front end says is in effect, and a setting written to the file,
//! named as the Settings screen names it, with the work the write leaves the engine side.

use anistream_core::config::Config;
use anistream_model::{
    event::Event, notice::Notice, settings::SettingKey, sources::only_reorders,
};

use crate::{
    data,
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// The config in effect and the file's, as the front end last said. Kept current by the front
/// end, which changes its own in place, until P4.
pub fn take_config(now: Box<Config>, saved: Box<Config>, engine: &mut EngineState) {
    engine.config.take(*now, *saved);
}

/// Write a setting to the file, and say how that went.
///
/// Written here rather than in the reducer: the reducer has already changed the in-memory
/// config, so the screen is correct whatever the disk does, and this keeps file IO out of a
/// pure function. Synchronous on purpose — one small file, and a settings change that silently
/// lost a race with the next one would be worse than a paused frame.
pub fn save_setting(
    table: &'static [&'static str],
    key: &'static str,
    value: anistream_core::settings::SettingValue,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, registry, .. } = services;
    // Against the file's order, not the one in effect: the new order is in effect already.
    let reordered = (table, key) == (&["providers"][..], "order")
        && matches!(&value, anistream_core::settings::SettingValue::List(order)
            if only_reorders(&engine.config.saved().providers.order, order));
    let written = engine.config.write(paths, table, key, value);
    // What the file holds after the write, so the Settings screen shows the file rather
    // than its own hopes — and can put a refused setting back.
    let _ = tx.send(Event::SettingWritten {
        on_disk: Box::new(engine.config.saved().clone()),
        failed: written.is_err().then_some((table, key)),
    });
    match written {
        Ok(()) => {
            // A toast, named as the Settings screen names it. As a status it stayed
            // in the status line for the rest of the session, over the state it shows,
            // and it said the file's key — "saved commit_threshold". What has no row
            // there, the remembered speed and volume, is saved without a word.
            if let Some(label) = setting_label(table, key) {
                let _ = tx.send(Event::Toast(Notice::info(format!("saved · {label}"))));
            }
            // Republish what the background tasks read, so the next download to
            // finish honours the value that was just written.
            let _ = tx.internal(Internal::SettingsSaved);
            // A setting that redefines what sources exist takes effect now rather
            // than at the next launch. The engine side's loop owns the session and
            // the guard, so it does the work, once what was said above is in; this
            // only says that it is needed. An order that only moves sources is put in
            // effect without a rebuild.
            if reordered {
                let _ = tx.internal(Internal::SourcesReordered);
            } else if redefines_sources(table, key) {
                let _ = tx.internal(Internal::SourcesChanged);
            }
        }
        // The guard reaching the UI: enabling torrents with no VPN configured fails
        // validation here, so no arrow key can write a config that would leak. And a
        // file that did not load at launch is refused by name and line.
        Err(e) => {
            let reason = match e {
                anistream_core::Error::Config(reason) => reason,
                other => other.to_string(),
            };
            let _ = tx.send(Event::Toast(Notice::alert(format!("not saved: {reason}"))));
            // The Providers rows moved when the key was pressed and the chain did not,
            // so the chain's own rows put them back.
            if (table, key) == (&["providers"][..], "order") {
                let _ = tx.send(Event::Providers(data::provider_rows(registry)));
            }
        }
    }
}

/// What the player changed of the config on its own, from its menus or its keys, kept in effect
/// for the next playback and saved as any setting is, as the event that says so passes on its
/// way to the front end. `tx` is asked for only when there is a save to report.
///
/// The speed and the volume are remembered whatever the config says, and saved only when it
/// asks for them to be: `persist_speed`, `persist_volume`.
///
/// The interface used to do this as the event reached it, by queueing the save in its one slot
/// for work, which an ending's prefetch, auto-next and an episode picked in the player fill too.
/// A save that found the slot taken was skipped without a word, and the next launch came back
/// without the change. Here nothing can take its place: the write is made in line, in its turn,
/// whatever the front end does with its events.
pub fn remember(
    event: &Event,
    engine: &mut EngineState,
    services: &Services,
    tx: impl FnOnce() -> Outbox,
) {
    use anistream_core::settings::SettingValue as V;
    let playback = &mut engine.config.effective_mut().playback;
    let (key, value) = match event {
        Event::PlaybackSpeed(speed) => {
            playback.persisted_speed = Some(*speed);
            if !playback.persist_speed {
                return;
            }
            ("persisted_speed", V::Float(*speed))
        }
        Event::PlaybackVolume(volume) => {
            playback.persisted_volume = Some(*volume);
            if !playback.persist_volume {
                return;
            }
            ("persisted_volume", V::Float(*volume))
        }
        Event::PlayerUpscaling(name) => {
            let Some(mode) = anistream_core::config::Upscaling::from_name(name) else { return };
            playback.upscaling = mode;
            ("upscaling", V::Str(name.clone()))
        }
        Event::PlayerToneMapping(curve) => {
            playback.tone_mapping.clone_from(curve);
            ("tone_mapping", V::Str(curve.clone()))
        }
        Event::PlayerNightMode(on) => {
            playback.night_mode = *on;
            ("night_mode", V::Bool(*on))
        }
        _ => return,
    };
    save_setting(&["playback"], key, value, engine, services, &tx());
}

/// The name the Settings screen gives the setting stored at `table`.`key`, for saying it was
/// saved. The config key (`commit_threshold`) is the file's vocabulary, not the screen's.
///
/// `None` for what is stored without a row of its own, the remembered speed and volume: saved
/// in passing, and not something anyone asked to have confirmed.
///
/// Read off the setting's own key, which names it for every front end: the engine had a table
/// of its own, held to the Settings screen's rows by a test.
pub fn setting_label(table: &[&str], key: &str) -> Option<&'static str> {
    SettingKey::at(table, key).map(SettingKey::label)
}

/// Whether writing this key changes which sources exist, and so needs the chain rebuilt.
///
/// Listed rather than inferred: a rebuild tears down a live torrent session, which is far
/// too blunt to trigger on a guess about a key name.
fn redefines_sources(table: &[&str], key: &str) -> bool {
    matches!(
        (table, key),
        (["providers", "torrent"], "enabled")
            | (["providers", "torrent", "vpn"], "mode")
            | (["providers"], "order" | "disabled" | "remote_url")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_source_defining_keys_force_a_rebuild() {
        // A rebuild tears down a live torrent session, so it must not fire on a setting
        // that has nothing to do with which sources exist.
        assert!(redefines_sources(&["providers", "torrent"], "enabled"));
        assert!(redefines_sources(&["providers", "torrent", "vpn"], "mode"));
        assert!(redefines_sources(&["providers"], "order"));

        assert!(!redefines_sources(&["playback"], "upscaling"));
        assert!(!redefines_sources(&["playback"], "quality"));
        assert!(!redefines_sources(&["downloads"], "directory"));
        assert!(!redefines_sources(&["theme"], "mode"));
    }
}
