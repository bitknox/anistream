//! Settings: the config the front end says is in effect, and a setting written to the file,
//! named as the Settings screen names it, with the work the write leaves the engine side.

use anistream_core::config::Config;
use anistream_model::{event::Event, notice::Notice, sources::only_reorders};

use crate::{
    data,
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// The config in effect and the file's, as the front end last said. Kept current by the front
/// end, which changes its own in place, until P4.
pub fn take_config(now: Box<Config>, saved: Box<Config>, engine: &mut EngineState) {
    let EngineState { config, saved_config, .. } = engine;
    *config = *now;
    *saved_config = *saved;
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
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, registry, .. } = services;
    let EngineState { saved_config, .. } = engine;
    // Against the file's order, not `config`'s: the reducer has already put the new
    // order in effect there.
    let reordered = (table, key) == (&["providers"][..], "order")
        && matches!(&value, anistream_core::settings::SettingValue::List(order)
            if only_reorders(&saved_config.providers.order, order));
    let written = anistream_core::settings::write_key(paths, table, key, value);
    // Read back whatever the write left, so the Settings screen shows the file rather
    // than its own hopes — and can put a refused setting back. A file that does not
    // load is treated the way startup treats it.
    let _ = tx.send(Event::SettingWritten {
        on_disk: Box::new(Config::load(paths).unwrap_or_default()),
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

/// The name the Settings screen gives the setting stored at `table`.`key`, for saying it was
/// saved. The config key (`commit_threshold`) is the file's vocabulary, not the screen's.
///
/// `None` for what is stored without a row of its own, the remembered speed and volume: saved
/// in passing, and not something anyone asked to have confirmed.
///
/// A table of its own, so the engine side can name a setting without asking the interface,
/// and held to the Settings screen's own rows by a test in the binary, the one place that sees
/// both.
/// P4 moves the settings' ids and labels into the model, and this reads them from there.
pub fn setting_label(table: &[&str], key: &str) -> Option<&'static str> {
    SETTING_LABELS.iter().find(|(t, k, _)| *t == table && *k == key).map(|(_, _, label)| *label)
}

/// Every setting the interface writes, by table and key, with the name its row goes by.
pub const SETTING_LABELS: &[(&[&str], &str, &str)] = &[
    // No row on Settings: `K` and `J` on Providers change it.
    (&["providers"], "order", "source order"),
    (&["theme"], "mode", "theme"),
    (&["theme"], "motion", "motion"),
    (&["playback"], "translation", "translation"),
    (&["playback"], "quality", "quality"),
    (&["playback"], "subtitle_language", "subtitles"),
    (&["playback"], "commit_threshold", "counts as watched at"),
    (&["playback"], "auto_next", "auto-play next"),
    (&["playback"], "skip_opening", "skip opening automatically"),
    (&["playback"], "skip_filler", "skip filler automatically"),
    (&["playback"], "mark_chapters", "mark opening and ending"),
    (&["playback"], "fullscreen", "start fullscreen"),
    (&["playback"], "upscaling", "upscaling"),
    (&["playback"], "player_ui", "player interface"),
    (&["playback"], "night_mode", "night mode"),
    (&["downloads"], "directory", "download folder"),
    (&["downloads"], "merge_subtitles", "merge subtitles in"),
    (&["downloads"], "keep_seeding", "keep seeding"),
    (&["downloads"], "on_complete", "after each download"),
    (&["providers", "torrent"], "enabled", "torrent source"),
    (&["providers", "torrent", "seeding"], "ratio", "seed after watching"),
    (&["providers", "torrent", "seeding"], "hours", "seed for at most"),
    (&["providers", "torrent", "seeding"], "upload_limit_kib", "upload limit"),
    (&["presence"], "enabled", "discord presence"),
    (&["presence"], "show_title", "presence shows the title"),
    (&["syncplay"], "enabled", "syncplay parties"),
    (&["syncplay"], "server", "syncplay server"),
    (&["syncplay"], "room", "syncplay room"),
    (&["updates"], "check", "check for updates"),
];

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
