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
    track_memory::{SubtitleChoice, SubtitleMemory, TrackMemory, TrackPrint},
};
use anistream_model::{
    command::Command,
    event::Event,
    protection::{FailedCheck, PROXY_SECRET, TorrentProtection, TorrentProxy},
    settings::{SettingKey, SettingOverride, SettingsView},
    show::{RememberedSubtitles, RememberedTrack, ShowSettingKey, ShowSettingsView},
    sources::GuardView,
    stores::Secret,
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
    assert_eq!(toasts(&updates), ["info: Saved auto-play next."]);
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(file, "[playback]\nauto_next = false\n");
    let row = h.app().setting_rows().into_iter().find(|row| row.key == SettingKey::AutoNext);
    assert_eq!(row.unwrap().value, "Off", "the screen shows the file");
}

/// A native row's value, as the settings say it.
fn native(view: &SettingsView, key: SettingKey) -> &str {
    &view.native_rows.iter().find(|row| row.key == key).expect("a native row").value
}

#[tokio::test]
async fn the_calendar_opens_on_all_and_remembers_the_pick() {
    let mut h = Harness::new().await;
    assert!(!on_disk(&h).meta.calendar_mine, "All, unless the person chose otherwise");

    h.send(Command::StepSetting { key: SettingKey::CalendarMine, delta: 1 });
    let updates = h.settle().await;
    assert_eq!(native(settings(&updates[0]), SettingKey::CalendarMine), "on");
    assert!(on_disk(&h).meta.calendar_mine);
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(file, "[meta]\ncalendar_mine = true\n");

    h.send(Command::SetSetting {
        key: SettingKey::CalendarMine,
        value: SettingValue::Bool(false),
    });
    let updates = h.settle().await;
    assert_eq!(native(settings(&updates[0]), SettingKey::CalendarMine), "off");
    assert!(!on_disk(&h).meta.calendar_mine);
}

#[tokio::test]
async fn a_remembered_calendar_pick_is_read_back_at_startup() {
    let config = "[meta]\ncalendar_mine = true\n";
    let mut h =
        Harness::with(Options { config: Some(config.into()), ..Options::default() }).await;
    h.send(Command::SetSetting { key: SettingKey::AutoNext, value: SettingValue::Bool(false) });
    let updates = h.settle().await;
    assert_eq!(native(settings(&updates[0]), SettingKey::CalendarMine), "on");
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
    assert_eq!(toasts(&updates), ["info: Saved night mode."]);
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
    assert_eq!(
        toasts(&updates),
        ["info: archive is off. It is skipped when looking for episodes."]
    );
    assert_eq!(on_disk(&h).providers.disabled, ["archive"]);
    assert_eq!(h.registry().ids(), ["archive", "fixture"], "in its place");
    assert_eq!(h.registry().usable_ids(), ["fixture"]);

    h.send(Command::DisablePlugin { id: "archive".into(), disabled: false });
    let updates = h.settle().await;
    assert_eq!(toasts(&updates), ["info: archive is on."]);
    assert!(on_disk(&h).providers.disabled.is_empty());
    assert_eq!(h.registry().usable_ids(), ["archive", "fixture"]);

    // The fixture source is no plugin: refused, and nothing written.
    h.send(Command::DisablePlugin { id: "fixture".into(), disabled: true });
    let updates = h.settle().await;
    assert_eq!(h.trace(), ["Toast"]);
    assert_eq!(toasts(&updates), ["alert: fixture is not a plugin that is loaded."]);
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
    assert_eq!(toasts(&updates), ["info: Saved source order."]);
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
    assert_eq!(note.english(), "Torrents are off. The session was stopped.");
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
    assert!(
        said[0].starts_with("alert: Couldn't save that setting. torrents need"),
        "{said:?}"
    );
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
    assert_eq!(toasts(&updates), ["info: Saved torrent protection."]);
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(
        file,
        "[providers.torrent.vpn]\nmode = \"external\"\ni_understand_my_ip_is_exposed = false\n\
         on_leak = \"stop\"\ni_understand_a_failed_check_keeps_torrents_running = false\n"
    );
    assert_eq!(on_disk(&h).providers.torrent.vpn.on_leak, LeakAction::Stop);
}

/// A proxy for the check below, with `password` given or left to the one kept.
fn proxy(host: &str, password: Option<&str>) -> TorrentProxy {
    TorrentProxy {
        host: host.into(),
        port: 1080,
        user: Some("mika".into()),
        password: password.map(|password| Secret(password.into())),
    }
}

#[tokio::test]
async fn a_proxy_is_checked_before_it_is_saved_and_nothing_is_written() {
    let mut h = Harness::new().await;
    let mode = TorrentProtection::Proxy;
    h.send(Command::CheckTorrentProtection {
        mode,
        proxy: Some(proxy("10.64.0.1", Some("lantern"))),
    });
    let updates = h.settle_until(|e| matches!(e, Event::ProtectionChecked(_))).await;

    let exit = anistream_providers::vpn::VpnGuard::fixture_egress().describe();
    assert!(
        matches!(&updates[..], [Event::ProtectionChecked(GuardView::Protected { exit: seen })] if *seen == exit),
        "the verdict alone: {updates:?}"
    );
    assert!(!h.paths().config_file.exists(), "nothing was written");
    assert_eq!(h.secrets().get(PROXY_SECRET).unwrap(), None, "nor the password kept");
}

#[tokio::test]
async fn a_failed_check_leaves_the_file_and_the_kept_password_as_they_were() {
    // A rule the fixture's exit does not meet.
    let file = "[providers.torrent.vpn]\nrequire_asn_org = [\"Other Co\"]\n";
    let mut h =
        Harness::with(Options { config: Some(file.into()), ..Options::default() }).await;
    h.secrets().set(PROXY_SECRET, "before").unwrap();
    let mode = TorrentProtection::Proxy;
    h.send(Command::CheckTorrentProtection {
        mode,
        proxy: Some(proxy("10.64.0.2", Some("after"))),
    });
    let updates = h.settle_until(|e| matches!(e, Event::ProtectionChecked(_))).await;

    assert!(
        matches!(&updates[..], [Event::ProtectionChecked(GuardView::Leaking)]),
        "{updates:?}"
    );
    assert_eq!(std::fs::read_to_string(&h.paths().config_file).unwrap(), file);
    assert_eq!(h.secrets().get(PROXY_SECRET).unwrap().as_deref(), Some("before"));

    // A proxy with no address is not checked at all, and the notice says why.
    let empty = TorrentProxy { host: String::new(), port: 0, user: None, password: None };
    h.send(Command::CheckTorrentProtection { mode, proxy: Some(empty) });
    let updates = h.settle_until(|e| matches!(e, Event::Toast(_))).await;
    assert!(
        matches!(
            &updates[..],
            [Event::ProtectionChecked(GuardView::Unreachable), Event::Toast(_)]
        ),
        "{updates:?}"
    );
    assert_eq!(
        toasts(&updates),
        ["alert: Couldn't check the torrent protection. A proxy needs a host and a port."]
    );
    assert_eq!(std::fs::read_to_string(&h.paths().config_file).unwrap(), file);
    assert_eq!(h.secrets().get(PROXY_SECRET).unwrap().as_deref(), Some("before"));
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
async fn a_show_keeps_its_own_download_quality_and_how_many_episodes_to_keep() {
    let mut h = Harness::new().await;
    let id = AnilistId::new(7);
    let higher = Some(SettingValue::Str("higher".into()));
    h.send(Command::SetShowSetting { id, key: ShowSettingKey::DownloadQuality, value: higher });
    h.send(Command::SetShowSetting {
        id,
        key: ShowSettingKey::KeepNext,
        value: Some(SettingValue::Int(3)),
    });
    let updates = h.settle().await;

    let sheet = last_sheet(&updates);
    let row = |key| sheet.rows.iter().find(|row| row.key == key).unwrap();
    let quality = row(ShowSettingKey::DownloadQuality);
    assert_eq!((quality.value.as_str(), quality.default.as_str()), ("higher", "standard"));
    let keep = row(ShowSettingKey::KeepNext);
    assert_eq!(keep.own, Some(SettingValue::Int(3)));
    assert_eq!((keep.value.as_str(), keep.default.as_str()), ("3 episodes", "off"));
    let view =
        settings(updates.iter().rev().find(|u| matches!(u, Event::Settings(_))).unwrap());
    assert_eq!(
        view.overridden,
        [
            SettingOverride { key: SettingKey::DownloadQuality, shows: 1 },
            SettingOverride { key: SettingKey::KeepNextEpisodes, shows: 1 },
        ]
    );
}

#[tokio::test]
async fn a_show_s_subtitles_are_chosen_and_forgotten_from_its_sheet() {
    let mut h = Harness::new().await;
    let id = AnilistId::new(7);
    // As the player prints a track: more than the sheet's words for it.
    let print = |title: &str| TrackPrint {
        lang: Some("eng".into()),
        title: title.into(),
        external: true,
        codec: Some("ass".into()),
        ..TrackPrint::default()
    };
    let memory = |title, audio: &str| TrackMemory {
        subtitles: Some(SubtitleMemory {
            choice: SubtitleChoice::Track(print(title)),
            audio_lang: Some(audio.into()),
        }),
        sub_delay: Some(0.5),
        ..TrackMemory::default()
    };
    h.store().remember_tracks(id, Translation::Sub, &memory("full", "jpn")).unwrap();
    h.store().remember_tracks(id, Translation::Dub, &memory("signs", "eng")).unwrap();
    let words = |title: &str| RememberedSubtitles::Track {
        track: RememberedTrack {
            language: Some("eng".into()),
            title: title.into(),
            forced: false,
            hearing_impaired: false,
        },
    };

    // The dub's signs, chosen for the sub, from the sheet's words for them.
    let subtitles = Some(words("signs"));
    h.send(Command::SetShowSubtitles { id, translation: Translation::Sub, subtitles });
    let updates = h.settle().await;
    assert_eq!(h.trace(), ["ShowSettings"]);
    let sub = |sheet: &ShowSettingsView| {
        sheet.remembered.iter().find(|r| r.translation == Translation::Sub).cloned()
    };
    let kept = sub(last_sheet(&updates)).unwrap();
    assert_eq!((kept.subtitles, kept.sub_delay), (Some(words("signs")), Some(0.5)));
    let stored = h.store().track_memory(id, Translation::Sub).unwrap().unwrap();
    assert_eq!(
        stored.subtitles,
        Some(SubtitleMemory {
            choice: SubtitleChoice::Track(print("signs")),
            audio_lang: Some("jpn".into()),
        }),
        "the player's own print, over the audio the sub plays in"
    );

    // Use default: the subtitles forgotten, the delay kept.
    h.send(Command::SetShowSubtitles { id, translation: Translation::Sub, subtitles: None });
    let kept = sub(last_sheet(&h.settle().await)).unwrap();
    assert_eq!((kept.subtitles, kept.sub_delay), (None, Some(0.5)));

    // Subtitles the show never had are not made up.
    let subtitles = Some(words("commentary"));
    h.send(Command::SetShowSubtitles { id, translation: Translation::Dub, subtitles });
    let updates = h.settle().await;
    assert_eq!(h.trace(), ["Toast", "ShowSettings"]);
    assert_eq!(
        toasts(&updates),
        [
            "alert: Couldn't save that subtitle choice. This show hasn't used those subtitles before."
        ]
    );
    let dub = h.store().track_memory(id, Translation::Dub).unwrap().unwrap();
    assert_eq!(dub, memory("signs", "eng"), "left as it was");
}

fn last_sheet(updates: &[Event]) -> &ShowSettingsView {
    updates
        .iter()
        .rev()
        .find_map(|update| match update {
            Event::ShowSettings(sheet) => Some(sheet),
            _ => None,
        })
        .expect("a show sheet")
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
    assert!(
        said[0].starts_with("alert: Couldn't save that setting. config.toml is left as it is"),
        "{said:?}"
    );
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
