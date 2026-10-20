//! Settings: a front end's asks for a change, carried out on the config the engine owns; what
//! the player changes on its own; and a setting written to the file, named as the Settings
//! screen names it, with the work the write leaves the engine side.

use anistream_core::{config::Config, media::Translation, settings::SettingValue};
use anistream_model::{
    event::Event,
    notice::Notice,
    settings::{self, SettingKey},
    sources::only_reorders,
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

/// Step `key` through its values, toward `delta`'s sign, by the Settings screen's rules, and
/// write what that makes of it: [`set_setting`]. A setting no step changes — typed, read-only,
/// the order — is left alone, and nothing is said.
pub fn step_setting(
    key: SettingKey,
    delta: i64,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let config = &engine.config;
    let Some(value) = settings::step(config.effective(), config.saved(), key, delta) else {
        return;
    };
    set_setting(key, value, engine, services, tx);
}

/// Put `value` in effect for `key` and write it, then say what the file holds, as
/// [`Event::Settings`]. A value the setting does not take, or a setting the file is the one
/// place to change, is left alone, and nothing is said.
///
/// In effect before it is written, so the work reads it the moment it is asked for; put back
/// by [`ConfigState::write`](crate::config::ConfigState::write) if the write is refused.
pub fn set_setting(
    key: SettingKey,
    value: SettingValue,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some((table, file_key)) = key.path() else { return };
    if !settings::put(engine.config.effective_mut(), key, &value) {
        tracing::debug!(?key, ?value, "not a value this setting takes; nothing written");
        return;
    }
    write_setting(table, file_key, value, Answer::View, engine, services, tx);
}

/// Sub or dub for this session only: in effect at once, and the file left as it is, so the next
/// launch goes by the saved choice. The Settings row shows it beside that choice.
pub fn set_translation(translation: Translation, engine: &mut EngineState, tx: &Outbox) {
    engine.config.effective_mut().playback.translation = translation;
    let _ = tx.send(Event::Settings(engine.config.view()));
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
    value: SettingValue,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    write_setting(table, key, value, Answer::OnDisk, engine, services, tx);
}

/// How a write's outcome is said. Temporary, while a front end that keeps a config of its own
/// still asks for writes with `SaveSetting`.
#[derive(Clone, Copy)]
enum Answer {
    /// What the file holds, whole, for that front end to keep: `SettingWritten`.
    OnDisk,
    /// The settings as the engine has them: [`Event::Settings`].
    View,
}

/// Write a setting the config already has in effect, and say how that went: what the file then
/// holds, the "saved" or "not saved" notice, and the work the write leaves the engine side.
fn write_setting(
    table: &'static [&'static str],
    key: &'static str,
    value: SettingValue,
    answer: Answer,
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
    let _ = tx.send(match answer {
        Answer::OnDisk => Event::SettingWritten {
            on_disk: Box::new(engine.config.saved().clone()),
            failed: written.is_err().then_some((table, key)),
        },
        Answer::View => Event::Settings(settings::SettingsView {
            refused: written.as_ref().err().and_then(|_| SettingKey::at(table, key)),
            ..engine.config.view()
        }),
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
/// way to the front end, which is told the settings after it. `tx` is asked for only when there
/// is something to say.
///
/// The speed and the volume are remembered whatever the config says, and saved only when it
/// asks for them to be: `persist_speed`, `persist_volume`. Sub or dub picked in the player is
/// for this session, as `t` is, and not saved.
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
    let saved = match event {
        Event::PlaybackSpeed(speed) => {
            playback.persisted_speed = Some(*speed);
            playback.persist_speed.then_some(("persisted_speed", V::Float(*speed)))
        }
        Event::PlaybackVolume(volume) => {
            playback.persisted_volume = Some(*volume);
            playback.persist_volume.then_some(("persisted_volume", V::Float(*volume)))
        }
        Event::PlayerUpscaling(name) => {
            let Some(mode) = anistream_core::config::Upscaling::from_name(name) else { return };
            playback.upscaling = mode;
            Some(("upscaling", V::Str(name.clone())))
        }
        Event::PlayerToneMapping(curve) => {
            playback.tone_mapping.clone_from(curve);
            Some(("tone_mapping", V::Str(curve.clone())))
        }
        Event::PlayerNightMode(on) => {
            playback.night_mode = *on;
            Some(("night_mode", V::Bool(*on)))
        }
        Event::PlayerTranslation(translation) => {
            playback.translation = *translation;
            None
        }
        _ => return,
    };
    let tx = tx();
    match saved {
        Some((key, value)) => {
            write_setting(&["playback"], key, value, Answer::View, engine, services, &tx);
        }
        // Kept for this session only: in effect, and the file left as it is.
        None => {
            let _ = tx.send(Event::Settings(engine.config.view()));
        }
    }
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
