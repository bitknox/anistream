//! Settings: written inline, read back, and acted on by the loop when they move sources.
//!
//! A write is synchronous in `dispatch`, so two in a row land in the order they were made and
//! the second sees the first. What the file holds afterwards always comes back, so a refused
//! write can put its row back. Only a write that landed is followed by `SettingsSaved`, and by
//! `SourcesReordered` or `SourcesChanged` when it touched the chain.

use anistream_core::settings::SettingValue;
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_testkit::sources;
use anistream_ui::app::{Task, Update};

use super::toasts;
use crate::harness::{Harness, Options, names};

/// A file that does not load: the app runs on the defaults and says so.
const BROKEN: &str = "[playback]\nqualty = 720\n";

fn order(entries: &[&str]) -> Vec<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

/// A new source order, as `K` and `J` on Providers make one: in effect at once, then saved.
fn reorder(h: &mut Harness, to: &[&str]) {
    h.app_mut().config.providers.order = order(to);
    h.send(Task::SaveSetting {
        table: &["providers"],
        key: "order",
        value: SettingValue::List(order(to)),
    });
}

fn written(update: &Update) -> (&anistream_core::config::Config, Option<(&[&str], &str)>) {
    match update {
        Update::SettingWritten { on_disk, failed } => (on_disk, *failed),
        other => panic!("not a settings write: {other:?}"),
    }
}

fn provider_ids(update: &Update) -> Vec<String> {
    match update {
        Update::Providers(rows) => rows.iter().map(|row| row.id.clone()).collect(),
        other => panic!("not the provider rows: {other:?}"),
    }
}

#[tokio::test]
async fn an_ordinary_write_lands_in_the_file_and_is_read_back() {
    let mut h = Harness::new().await;
    let job = h.send(Task::SaveSetting {
        table: &["playback"],
        key: "auto_next",
        value: SettingValue::Bool(false),
    });
    let updates = h.settle().await;

    assert!(job.is_none(), "too quick to wait on");
    assert_eq!(names(&updates), ["SettingWritten", "Toast", "SettingsSaved"]);
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
    h.send(Task::SaveSetting {
        table: &["playback"],
        key: "persisted_speed",
        value: SettingValue::Float(1.25),
    });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["SettingWritten", "SettingsSaved"]);
    assert_eq!(written(&updates[0]).0.playback.persisted_speed, Some(1.25));
}

#[tokio::test]
async fn writes_in_a_row_land_in_order_and_each_sees_the_last() {
    let mut h = Harness::new().await;
    for (key, on) in [("auto_next", false), ("skip_opening", false), ("auto_next", true)] {
        h.send(Task::SaveSetting { table: &["playback"], key, value: SettingValue::Bool(on) });
    }
    let updates = h.settle().await;

    let reads: Vec<(bool, bool)> = updates
        .iter()
        .filter(|update| matches!(update, Update::SettingWritten { .. }))
        .map(|update| {
            let on_disk = written(update).0;
            (on_disk.playback.auto_next, on_disk.playback.skip_opening)
        })
        .collect();
    assert_eq!(reads, [(false, true), (false, false), (true, false)]);
    let saved = names(&updates).iter().filter(|name| **name == "SettingsSaved").count();
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
        names(&updates),
        ["SettingWritten", "Toast", "SettingsSaved", "Providers", "SourcesReordered"]
    );
    assert_eq!(toasts(&updates), ["info: saved · source order"]);
    assert_eq!(provider_ids(&updates[3]), ["alt", "fixture"]);
    assert_eq!(h.registry().ids(), ["alt", "fixture"], "the live chain, not a rebuilt one");

    reorder(&mut h, &["fixture", "alt", "torrent", "plugins"]);
    let updates = h.settle().await;
    assert_eq!(provider_ids(&updates[3]), ["fixture", "alt"]);
}

#[tokio::test]
async fn an_order_that_takes_a_source_away_rebuilds_the_chain() {
    let mut h = Harness::new().await;
    reorder(&mut h, &["torrent"]);
    let updates = h.settle().await;

    // The rebuild's reports come after the change that asked for it.
    assert_eq!(
        names(&updates),
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
    assert!(matches!(&updates[4], Update::Vpn(None)));
    let Update::ProviderNote(note) = &updates[5] else { unreachable!() };
    assert_eq!(note, "torrents are off — the session was stopped");
    // Rebuilt from the config, which describes no source the fixture one could be.
    assert!(provider_ids(&updates[6]).is_empty());
    assert!(h.registry().is_empty());
}

#[tokio::test]
async fn a_write_the_config_refuses_leaves_the_file_alone_and_alerts() {
    let mut h = Harness::new().await;
    // A source-defining key, refused by validation: no rebuild follows.
    h.send(Task::SaveSetting {
        table: &["providers", "torrent"],
        key: "enabled",
        value: SettingValue::Bool(true),
    });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["SettingWritten", "Toast"]);
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
    h.send(Task::SaveSetting {
        table: &["playback"],
        key: "auto_next",
        value: SettingValue::Bool(false),
    });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["SettingWritten", "Toast"]);
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
    assert_eq!(names(&updates), ["SettingWritten", "Toast", "Providers"]);
    assert_eq!(provider_ids(&updates[2]), ["fixture"]);
    assert_eq!(std::fs::read_to_string(&h.paths().config_file).unwrap(), BROKEN);
}
