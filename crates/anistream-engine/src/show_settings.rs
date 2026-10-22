//! Settings for one show: its sheet read, a setting set for it or let go of, and the show put
//! back on Settings; what a play of it runs under; and the counts of shows that set each
//! setting, which the Settings rows say.
//!
//! In line, on the actor, as a provider preference is: one small indexed read or write, and a
//! sheet answered from a task could arrive after the change that followed it.

use anistream_core::{
    config::Config, ids::AnilistId, media::Translation, settings::SettingValue,
};
use anistream_model::{
    event::Event,
    notice::Notice,
    settings::{SettingKey, SettingOverride},
    show::{self, RememberedTracks, ShowSettingKey, ShowValues},
};
use anistream_store::Store;

use crate::{outbox::Outbox, services::Services, state::EngineState};

/// What show `id` sets for itself, as the database keeps it: nothing, when it cannot be read.
/// A play goes ahead on Settings rather than not at all.
pub(crate) fn own(store: &Store, id: AnilistId) -> ShowValues {
    match store.show_settings(id) {
        Ok(kept) => show::read(&kept),
        Err(error) => {
            tracing::warn!(%id, %error, "could not read a show's own settings");
            Vec::new()
        }
    }
}

/// The config a play of show `id` runs under: `config`, the one in effect, with what the show
/// sets for itself put in it.
pub(crate) fn for_show(config: &Config, store: &Store, id: AnilistId) -> Config {
    let mut config = config.clone();
    show::apply(&mut config, &own(store, id));
    config
}

/// How many shows set each setting for themselves, the source they keep among them, as the
/// Settings rows say it. Nothing, when the database cannot count them.
pub(crate) fn counts(store: &Store) -> Vec<SettingOverride> {
    let counted = store.show_setting_counts().unwrap_or_else(|error| {
        tracing::warn!(%error, "could not count the shows' own settings");
        Vec::new()
    });
    counted
        .into_iter()
        .filter_map(|(name, shows)| {
            let key = match name.as_str() {
                "source" => SettingKey::SourceOrder,
                name => ShowSettingKey::named(name)?.setting(),
            };
            Some(SettingOverride { key, shows })
        })
        .collect()
}

/// How many shows switch new-episode notifications on for themselves: none, when the database
/// cannot say.
pub(crate) fn notify_switched_on(store: &Store) -> u32 {
    let name = ShowSettingKey::NotifyNewEpisodes.name();
    match store.shows_setting(name, "true") {
        Ok(shows) => shows.len() as u32,
        Err(error) => {
            tracing::warn!(%error, "could not count the shows switched on for new episodes");
            0
        }
    }
}

/// Count the shows' own settings again into the settings the engine says.
pub(crate) fn count(engine: &mut EngineState, store: &Store) {
    engine.config.set_overridden(counts(store), notify_switched_on(store));
}

/// Count the shows' own settings again and say the settings with the new counts.
pub(crate) fn recount(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    count(engine, &services.store);
    let _ = tx.send(Event::Settings(engine.config.view()));
}

/// Say show `id`'s settings sheet.
pub fn load_show_settings(
    id: AnilistId,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let store = &services.store;
    let remembered = [Translation::Sub, Translation::Dub]
        .into_iter()
        .filter_map(|translation| {
            let memory = store.track_memory(id, translation).ok().flatten()?;
            RememberedTracks::of(translation, &memory)
        })
        .collect();
    let source = store.provider_preference(id).ok().flatten();
    let view = show::view(id, engine.config.effective(), &own(store, id), remembered, source);
    let _ = tx.send(Event::ShowSettings(view));
}

/// Have show `id` set `key` to `value`, or follow Settings again, then say its sheet and the
/// settings with their counts. A value the setting does not take is refused in words.
pub fn set_show_setting(
    id: AnilistId,
    key: ShowSettingKey,
    value: Option<SettingValue>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let text = match &value {
        Some(value) => match show::encode(key, value) {
            Some(text) => Some(text),
            None => {
                tracing::debug!(?key, ?value, "not a value this setting takes");
                let refusal =
                    format!("not saved: {} is not a {}", words(value), key.setting().label());
                let _ = tx.send(Event::Toast(Notice::alert(refusal)));
                return;
            }
        },
        None => None,
    };
    let written = services.store.set_show_setting(
        id,
        key.name(),
        text.as_deref(),
        anistream_store::now(),
    );
    if let Err(error) = written {
        let _ = tx.send(Event::Toast(Notice::alert(format!("not saved: {error}"))));
    }
    load_show_settings(id, engine, services, tx);
    recount(engine, services, tx);
}

/// Put show `id` back on Settings: what it sets, the source it keeps and the tracks
/// remembered for it, forgotten together. Then say its sheet and the settings.
pub fn reset_show_settings(
    id: AnilistId,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if let Err(error) = services.store.reset_show(id) {
        let _ = tx.send(Event::Toast(Notice::alert(format!("not reset: {error}"))));
    }
    load_show_settings(id, engine, services, tx);
    recount(engine, services, tx);
}

/// A value as a refusal names it.
fn words(value: &SettingValue) -> String {
    match value {
        SettingValue::Str(text) => format!("\u{201c}{text}\u{201d}"),
        SettingValue::Int(number) => number.to_string(),
        SettingValue::Float(number) => number.to_string(),
        SettingValue::Bool(on) => if *on { "on" } else { "off" }.into(),
        SettingValue::Strings(items) => items.join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_001);

    #[test]
    fn a_play_runs_under_what_its_show_sets_and_the_rest_of_settings() {
        let store = Store::open_in_memory().unwrap();
        let config = Config::default();
        assert_eq!(for_show(&config, &store, SHOW), config, "a show that sets nothing");

        store.set_show_setting(SHOW, "translation", Some("dub"), 1).unwrap();
        store.set_show_setting(SHOW, "skip_filler", Some("true"), 1).unwrap();
        // Kept by some other build, and passed over.
        store.set_show_setting(SHOW, "quality", Some("best"), 1).unwrap();
        let played = for_show(&config, &store, SHOW);
        assert_eq!(played.playback.translation, Translation::Dub);
        assert!(played.playback.skip_filler);
        assert_eq!(played.playback.quality, config.playback.quality);
        assert_eq!(for_show(&config, &store, AnilistId::new(900_002)), config, "its own alone");
    }

    #[test]
    fn the_counts_go_under_the_rows_the_shows_set() {
        let store = Store::open_in_memory().unwrap();
        store.set_show_setting(SHOW, "skip_opening", Some("false"), 1).unwrap();
        store
            .set_show_setting(AnilistId::new(900_002), "skip_opening", Some("true"), 1)
            .unwrap();
        store.set_provider_preference(SHOW, "yamabiko", 1).unwrap();
        store.set_show_setting(SHOW, "retired", Some("true"), 1).unwrap();
        assert_eq!(
            counts(&store),
            [
                SettingOverride { key: SettingKey::SkipOpening, shows: 2 },
                SettingOverride { key: SettingKey::SourceOrder, shows: 1 },
            ]
        );
    }
}
