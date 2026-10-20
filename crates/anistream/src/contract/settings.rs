//! Settings: written inline, read back, and acted on by the loop when they move sources.
//!
//! A write is synchronous in `dispatch`, so two in a row land in the order they were made and
//! the second sees the first. What the file holds afterwards always comes back, so a refused
//! write can put its row back. Only a write that landed is followed by `SettingsSaved`, and by
//! `SourcesReordered` or `SourcesChanged` when it touched the chain.

use anistream_core::{ids::AnilistId, settings::SettingValue};
use anistream_model::{command::Command, event::Event};
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_testkit::sources;

use super::toasts;
use crate::harness::{Harness, Options};

/// A file that does not load: the app runs on the defaults and says so.
const BROKEN: &str = "[playback]\nqualty = 720\n";

fn order(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

/// A new source order, as `K` and `J` on Providers make one: in effect at once, then saved.
fn reorder(h: &mut Harness, to: &[&str]) {
    h.app_mut().config.providers.order = order(to);
    h.send(Command::SaveSetting {
        table: &["providers"],
        key: "order",
        value: SettingValue::List(order(to)),
    });
}

fn written(update: &Event) -> (&anistream_core::config::Config, Option<(&[&str], &str)>) {
    match update {
        Event::SettingWritten { on_disk, failed } => (on_disk, *failed),
        other => panic!("not a settings write: {other:?}"),
    }
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
    let job = h.send(Command::SaveSetting {
        table: &["playback"],
        key: "auto_next",
        value: SettingValue::Bool(false),
    });
    let updates = h.settle().await;

    assert!(job.is_none(), "too quick to wait on");
    assert_eq!(h.trace(), ["SettingWritten", "Toast", "SettingsSaved"]);
    let (on_disk, failed) = written(&updates[0]);
    assert!(!on_disk.playback.auto_next);
    assert_eq!(failed, None);
    assert_eq!(toasts(&updates), ["info: saved · auto-play next"]);
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(file, "[playback]\nauto_next = false\n");
    assert!(!h.app().saved_config.playback.auto_next, "the screen shows the file");
}

#[tokio::test]
async fn a_setting_with_no_row_is_saved_without_a_word() {
    let mut h = Harness::new().await;
    h.send(Command::SaveSetting {
        table: &["playback"],
        key: "persisted_speed",
        value: SettingValue::Float(1.25),
    });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["SettingWritten", "SettingsSaved"]);
    assert_eq!(written(&updates[0]).0.playback.persisted_speed, Some(1.25));
}

#[tokio::test]
async fn a_speed_the_player_remembers_is_saved_without_a_word() {
    let mut h = Harness::new().await;
    h.player_says(Event::PlaybackSpeed(1.25));
    h.settle().await;

    assert_eq!(h.trace(), ["PlaybackSpeed", "SettingWritten", "SettingsSaved"]);
    let file = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert_eq!(file, "[playback]\npersisted_speed = 1.25\n");
}

#[tokio::test]
async fn a_speed_is_not_saved_when_the_config_says_not_to_remember_it() {
    let config = "[playback]\npersist_speed = false\n";
    let mut h =
        Harness::with(Options { config: Some(config.into()), ..Options::default() }).await;
    h.player_says(Event::PlaybackSpeed(1.25));
    h.settle().await;

    assert_eq!(h.trace(), ["PlaybackSpeed"]);
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
async fn writes_in_a_row_land_in_order_and_each_sees_the_last() {
    let mut h = Harness::new().await;
    for (key, on) in [("auto_next", false), ("skip_opening", false), ("auto_next", true)] {
        h.send(Command::SaveSetting {
            table: &["playback"],
            key,
            value: SettingValue::Bool(on),
        });
    }
    let updates = h.settle().await;

    let reads: Vec<(bool, bool)> = updates
        .iter()
        .filter(|update| matches!(update, Event::SettingWritten { .. }))
        .map(|update| {
            let on_disk = written(update).0;
            (on_disk.playback.auto_next, on_disk.playback.skip_opening)
        })
        .collect();
    assert_eq!(reads, [(false, true), (false, false), (true, false)]);
    let saved = h.trace().iter().filter(|name| **name == "SettingsSaved").count();
    assert_eq!(saved, 3);
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
        ["SettingWritten", "Toast", "SettingsSaved", "Providers", "SourcesReordered"]
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
            "SettingWritten",
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
    h.send(Command::SaveSetting {
        table: &["providers", "torrent"],
        key: "enabled",
        value: SettingValue::Bool(true),
    });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["SettingWritten", "Toast"]);
    let (on_disk, failed) = written(&updates[0]);
    assert_eq!(failed, Some((&["providers", "torrent"][..], "enabled")));
    assert!(!on_disk.providers.torrent.enabled);
    let said = toasts(&updates);
    assert!(said[0].starts_with("alert: not saved: torrents need"), "{said:?}");
    assert!(!h.paths().config_file.exists(), "nothing was written");
}

#[tokio::test]
async fn a_file_that_does_not_load_is_not_written_over() {
    let mut h =
        Harness::with(Options { config: Some(BROKEN.into()), ..Options::default() }).await;
    assert!(h.app().notice.is_some(), "the app says it is running on the defaults");
    h.send(Command::SaveSetting {
        table: &["playback"],
        key: "auto_next",
        value: SettingValue::Bool(false),
    });
    let updates = h.settle().await;

    assert_eq!(h.trace(), ["SettingWritten", "Toast"]);
    let (on_disk, failed) = written(&updates[0]);
    assert_eq!(failed, Some((&["playback"][..], "auto_next")));
    assert!(on_disk.playback.auto_next, "read back as startup reads it: the defaults");
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
    assert_eq!(h.trace(), ["SettingWritten", "Toast", "Providers"]);
    assert_eq!(provider_ids(&updates[2]), ["fixture"]);
    assert_eq!(std::fs::read_to_string(&h.paths().config_file).unwrap(), BROKEN);
}
