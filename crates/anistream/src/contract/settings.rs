//! Settings: asked for, put in effect and written inline by the engine, said back whole, and
//! acted on by the engine side when they move sources.
//!
//! A write is synchronous in `dispatch`, so two in a row land in the order they were made and
//! the second sees the first. The settings as they then stand always come back, so a refused
//! write is said with its setting put back. Only a write that landed is followed by
//! `SettingsSaved`, and by `SourcesReordered` or `SourcesChanged` when it touched the chain.

use anistream_core::{
    config::{Config, LeakAction},
    ids::AnilistId,
    media::Translation,
    settings::SettingValue,
};
use anistream_model::{
    command::Command,
    event::Event,
    protection::{FailedCheck, TorrentProtection},
    settings::{SettingKey, SettingOverride, SettingsView},
    show::ShowSettingKey,
};
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_testkit::sources;

use super::toasts;
use crate::harness::{Harness, Options};

/// A file that does not load: the app runs on the defaults and says so.
const BROKEN: &str = "[playback]\nqualty = 720\n";

fn order(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

/// A new source order, as `K` and `J` on Providers ask for one.
fn reorder(h: &mut Harness, to: &[&str]) {
    h.send(Command::SetSetting {
        key: SettingKey::SourceOrder,
        value: SettingValue::Strings(order(to)),
    });
}

fn settings(update: &Event) -> &SettingsView {
    match update {
        Event::Settings(view) => view,
        other => panic!("not the settings: {other:?}"),
    }
}

/// A row's value, as the settings say it.
fn shown(view: &SettingsView, key: SettingKey) -> &str {
    &view.rows.iter().find(|row| row.key == key).expect("a row").value
}

/// What the file holds now, read as startup reads it.
fn on_disk(h: &Harness) -> Config {
    Config::load(h.paths()).unwrap_or_default()
}

fn provider_ids(update: &Event) -> Vec<String> {
    match update {
        Event::Providers(rows) => rows.iter().map(|row| row.id.clone()).collect(),
        other => panic!("not the provider rows: {other:?}"),
    }
}

#[tokio::test]
async fn an_ordinary_write_lands_in_the_file_and_is_read_back() {
    let mut h = Harness::new().await;
    let job = h.send(Command::SetSetting {
        key: SettingKey::AutoNext,
        value: SettingValue::Bool(false),
    });
    let updates = h.settle().await;

    assert!(job.is_none(), "too quick to wait on");
    assert_eq!(h.trace(), ["Settings", "Toast", "SettingsSaved"]);
    let view = settings(&updates[0]);
    assert_eq!(shown(view, SettingKey::AutoNext), "off");
    assert!(!on_disk(&h).playback.auto_next);
    assert_eq!(view.refused, None);
    assert_eq!(toasts(&updates), ["info: saved · auto-play next"]);
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(file, "[playback]\nauto_next = false\n");
    let row = h.app().setting_rows().into_iter().find(|row| row.key == SettingKey::AutoNext);
    assert_eq!(row.unwrap().value, "off", "the screen shows the file");
}

#[tokio::test]
async fn a_setting_with_no_row_is_saved_without_a_word() {
    // The volume the player remembers: no front end asks for it, the player says it.
    let mut h = Harness::new().await;
    h.player_says(Event::PlaybackVolume(80.0));
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["PlaybackVolume", "Settings", "SettingsSaved"]);
    assert_eq!(on_disk(&h).playback.persisted_volume, Some(80.0));
    assert!(toasts(&updates).is_empty());
}

#[tokio::test]
async fn a_speed_the_player_remembers_is_saved_without_a_word() {
    let mut h = Harness::new().await;
    h.player_says(Event::PlaybackSpeed(1.25));
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["PlaybackSpeed", "Settings", "SettingsSaved"]);
    assert_eq!(settings(&updates[1]).speed, Some(1.25));
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(file, "[playback]\npersisted_speed = 1.25\n");
}

#[tokio::test]
async fn a_speed_is_not_saved_when_the_config_says_not_to_remember_it() {
    let config = "[playback]\npersist_speed = false\n";
    let mut h =
        Harness::with(Options { config: Some(config.into()), ..Options::default() }).await;
    h.player_says(Event::PlaybackSpeed(1.25));
    let updates = h.settle().await;

    // Remembered for the next episode all the same, which the interface used to keep itself.
    assert_eq!(h.trace(), ["PlaybackSpeed", "Settings"]);
    assert_eq!(settings(&updates[1]).speed, Some(1.25));
    assert_eq!(std::fs::read_to_string(&h.paths().config_file).unwrap(), config);
}

#[tokio::test]
async fn a_choice_made_in_the_player_is_saved_however_the_front_end_reads_its_events() {
    // An episode reaches its ending and asks for the next one's streams, and the viewer turns
    // night mode on in the player's menu: two events, which a front end reading its events in
    // batches applies together. The interface saved night mode by queueing the write in its one
    // slot for work, which the ending's prefetch had just taken, so the save was skipped and the
    // next launch came back without night mode. The engine saves it as it passes.
    let mut h = Harness::new().await;
    h.player_says(Event::PrefetchNext { id: AnilistId::new(7), episode: "2".into() });
    h.player_says(Event::PlayerNightMode(true));
    let updates = h.settle_in_one_batch().await;

    let file = std::fs::read_to_string(&h.paths().config_file).unwrap_or_default();
    assert_eq!(file, "[playback]\nnight_mode = true\n", "night mode was not saved");
    assert_eq!(toasts(&updates), ["info: saved · night mode"]);
}

#[tokio::test]
async fn the_session_s_choice_is_told_apart_from_the_file_s() {
    // `t`, for this session: in effect, and the file left alone.
    let mut h = Harness::new().await;
    h.send(Command::SetTranslation { translation: Translation::Dub });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["Settings"]);
    let view = settings(&updates[0]);
    assert_eq!(view.translation, Translation::Dub, "in effect");
    assert_eq!(shown(view, SettingKey::Translation), "sub · dub this session");
    assert!(!h.paths().config_file.exists(), "and not written");

    // The row's own step changes the saved choice, and the session's gives way to it.
    h.send(Command::StepSetting { key: SettingKey::Translation, delta: 1 });
    let updates = h.settle().await;
    assert_eq!(shown(settings(&updates[0]), SettingKey::Translation), "dub");
    assert_eq!(on_disk(&h).playback.translation, Translation::Dub);
}

#[tokio::test]
async fn writes_in_a_row_land_in_order_and_each_sees_the_last() {
    let mut h = Harness::new().await;
    for (key, on) in [
        (SettingKey::AutoNext, false),
        (SettingKey::SkipOpening, false),
        (SettingKey::AutoNext, true),
    ] {
        h.send(Command::SetSetting { key, value: SettingValue::Bool(on) });
    }
    let updates = h.settle().await;

    let reads: Vec<(&str, &str)> = updates
        .iter()
        .filter(|update| matches!(update, Event::Settings(_)))
        .map(|update| {
            let view = settings(update);
            (shown(view, SettingKey::AutoNext), shown(view, SettingKey::SkipOpening))
        })
        .collect();
    assert_eq!(reads, [("off", "on"), ("off", "off"), ("on", "off")]);
    let file = on_disk(&h);
    assert_eq!((file.playback.auto_next, file.playback.skip_opening), (true, false));
    let saved = h.trace().iter().filter(|name| **name == "SettingsSaved").count();
    assert_eq!(saved, 3);
}

#[tokio::test]
async fn a_plugin_switched_off_keeps_its_row_and_is_not_tried() {
    use anistream_core::traits::ProviderKind;
    let archive =
        MockProvider::new("archive").with_hits(sources::hits()).kind(ProviderKind::Plugin);
    let registry = ProviderRegistry::new(vec![archive.arc(), sources::provider().arc()]);
    let mut h = Harness::with(Options { registry: Some(registry), ..Options::default() }).await;
    h.send(Command::DisablePlugin { id: "archive".into(), disabled: true });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["Settings", "Providers", "Toast", "SettingsSaved"]);
    assert_eq!(settings(&updates[0]).sources.disabled, ["archive"]);
    let Event::Providers(rows) = &updates[1] else { unreachable!() };
    let row = rows.iter().find(|row| row.id == "archive").expect("its row stays");
    assert!(row.disabled && !row.usable);
    assert_eq!(toasts(&updates), ["info: archive is off · skipped when looking for episodes"]);
    assert_eq!(on_disk(&h).providers.disabled, ["archive"]);
    assert_eq!(h.registry().ids(), ["archive", "fixture"], "in its place");
    assert_eq!(h.registry().usable_ids(), ["fixture"]);

    h.send(Command::DisablePlugin { id: "archive".into(), disabled: false });
    let updates = h.settle().await;
    assert_eq!(toasts(&updates), ["info: archive is on"]);
    assert!(on_disk(&h).providers.disabled.is_empty());
    assert_eq!(h.registry().usable_ids(), ["archive", "fixture"]);

    // The fixture source is no plugin: refused, and nothing written.
    h.send(Command::DisablePlugin { id: "fixture".into(), disabled: true });
    let updates = h.settle().await;
    assert_eq!(h.trace(), ["Toast"]);
    assert_eq!(toasts(&updates), ["alert: fixture is not a plugin that is loaded"]);
}

#[tokio::test]
async fn an_order_that_only_moves_sources_reorders_the_chain_in_place() {
    let alt = MockProvider::new("alt").with_hits(sources::hits());
    let registry = ProviderRegistry::new(vec![sources::provider().arc(), alt.arc()]);
    let mut h = Harness::with(Options { registry: Some(registry), ..Options::default() }).await;
    reorder(&mut h, &["alt", "fixture", "torrent", "plugins"]);
    let updates = h.settle().await;

    // The chain's own rows go up before the reorder itself is applied, as the loop has it.
    assert_eq!(
        h.trace(),
        ["Settings", "Toast", "SettingsSaved", "Providers", "SourcesReordered"]
    );
    assert_eq!(toasts(&updates), ["info: saved · source order"]);
    assert_eq!(provider_ids(&updates[2]), ["alt", "fixture"]);
    assert_eq!(h.registry().ids(), ["alt", "fixture"], "the live chain, not a rebuilt one");

    reorder(&mut h, &["fixture", "alt", "torrent", "plugins"]);
    let updates = h.settle().await;
    assert_eq!(provider_ids(&updates[2]), ["fixture", "alt"]);
}

#[tokio::test]
async fn an_order_that_takes_a_source_away_rebuilds_the_chain() {
    let mut h = Harness::new().await;
    reorder(&mut h, &["torrent"]);
    let updates = h.settle().await;

    // The rebuild's reports come after the change that asked for it.
    assert_eq!(
        h.trace(),
        [
            "Settings",
            "Toast",
            "SettingsSaved",
            "SourcesChanged",
            "Vpn",
            "ProviderNote",
            "Providers"
        ]
    );
    assert!(matches!(&updates[3], Event::Vpn(None)));
    let Event::ProviderNote(note) = &updates[4] else { unreachable!() };
    assert_eq!(note, "torrents are off — the session was stopped");
    // Rebuilt from the config, which describes no source the fixture one could be.
    assert!(provider_ids(&updates[5]).is_empty());
    assert!(h.registry().is_empty());
}

#[tokio::test]
async fn a_write_the_config_refuses_leaves_the_file_alone_and_alerts() {
    let mut h = Harness::new().await;
    // A source-defining key, refused by validation: no rebuild follows.
    h.send(Command::SetSetting { key: SettingKey::Torrents, value: SettingValue::Bool(true) });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["Settings", "Toast"]);
    let view = settings(&updates[0]);
    assert_eq!(view.refused, Some(SettingKey::Torrents));
    assert!(!view.sources.torrent, "put back to what the file holds");
    assert_eq!(shown(view, SettingKey::Torrents), "off");
    let said = toasts(&updates);
    assert!(said[0].starts_with("alert: not saved: torrents need"), "{said:?}");
    assert!(!h.paths().config_file.exists(), "nothing was written");
}

#[tokio::test]
async fn a_protection_is_written_as_one_change_and_read_back_as_the_terminal_reads_it() {
    let mut h = Harness::new().await;
    h.send(Command::SetTorrentProtection {
        mode: TorrentProtection::Device,
        proxy: None,
        on_failure: FailedCheck::Stop,
        acknowledged: false,
    });
    let updates = h.settle().await;

    // Torrents are off, so nothing is brought back up.
    assert_eq!(h.trace(), ["Settings", "Toast", "SettingsSaved"]);
    let view = settings(&updates[0]);
    assert_eq!(
        (view.torrent.protection, view.torrent.on_failure),
        (TorrentProtection::Device, FailedCheck::Stop)
    );
    assert_eq!(shown(view, SettingKey::VpnMode), "external", "the terminal's row says it too");
    assert_eq!(toasts(&updates), ["info: saved · torrent protection"]);
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(
        file,
        "[providers.torrent.vpn]\nmode = \"external\"\ni_understand_my_ip_is_exposed = false\n\
         on_leak = \"stop\"\ni_understand_a_failed_check_keeps_torrents_running = false\n"
    );
    assert_eq!(on_disk(&h).providers.torrent.vpn.on_leak, LeakAction::Stop);
}

#[tokio::test]
async fn what_a_show_sets_for_itself_comes_with_its_sheet_and_is_counted_on_the_settings() {
    let mut h = Harness::new().await;
    let id = AnilistId::new(7);
    let value = Some(SettingValue::Str("dub".into()));
    h.send(Command::SetShowSetting { id, key: ShowSettingKey::Translation, value });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["ShowSettings", "Settings"]);
    let Event::ShowSettings(sheet) = &updates[0] else { unreachable!() };
    let row = sheet.rows.iter().find(|row| row.key == ShowSettingKey::Translation).unwrap();
    assert_eq!((row.value.as_str(), row.default.as_str()), ("dub", "sub"));
    let view = settings(&updates[1]);
    assert_eq!(view.overridden, [SettingOverride { key: SettingKey::Translation, shows: 1 }]);
    assert_eq!(shown(view, SettingKey::Translation), "sub", "Settings' own is as it was");
    assert!(!h.paths().config_file.exists(), "the file is Settings', not the show's");
}

#[tokio::test]
async fn check_now_with_torrents_off_says_there_is_no_guard() {
    // The rest is the torrent tasks', which the harness has none of: see `sources::torrent`.
    let mut h = Harness::new().await;
    h.send(Command::CheckConnection);
    let updates = h.settle().await;
    assert_eq!(h.trace(), ["Vpn"]);
    assert!(matches!(&updates[0], Event::Vpn(None)), "{updates:?}");
}

#[tokio::test]
async fn a_file_that_does_not_load_is_not_written_over() {
    let mut h =
        Harness::with(Options { config: Some(BROKEN.into()), ..Options::default() }).await;
    assert!(h.app().notice.is_some(), "the app says it is running on the defaults");
    h.send(Command::SetSetting { key: SettingKey::AutoNext, value: SettingValue::Bool(false) });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["Settings", "Toast"]);
    let view = settings(&updates[0]);
    assert_eq!(view.refused, Some(SettingKey::AutoNext));
    assert_eq!(
        shown(view, SettingKey::AutoNext),
        "on",
        "read back as startup reads it: the defaults"
    );
    let said = toasts(&updates);
    assert!(said[0].starts_with("alert: not saved: config.toml is left as it is"), "{said:?}");
    assert_eq!(std::fs::read_to_string(&h.paths().config_file).unwrap(), BROKEN);
}

#[tokio::test]
async fn a_refused_order_puts_the_provider_rows_back() {
    let mut h =
        Harness::with(Options { config: Some(BROKEN.into()), ..Options::default() }).await;
    reorder(&mut h, &["plugins", "torrent"]);
    let updates = h.settle().await;

    // The rows moved when the key was pressed; the chain did not, so its rows put them back.
    assert_eq!(h.trace(), ["Settings", "Toast", "Providers"]);
    assert_eq!(settings(&updates[0]).sources.order, ["torrent", "plugins"], "the file's order");
    assert_eq!(provider_ids(&updates[2]), ["fixture"]);
    assert_eq!(std::fs::read_to_string(&h.paths().config_file).unwrap(), BROKEN);
}
