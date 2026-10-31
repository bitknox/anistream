//! Your media through the engine: folders, the scan, matching, Keep unmatched and removal.
//! Folders are temporary directories of empty files, and AniList is the testkit's fake.

use std::path::Path;

use anistream_core::ids::AnilistId;
use anistream_model::{
    command::Command,
    event::Event,
    local::{GroupState, LocalCommand, LocalEvent, LocalView, NotMatchedReason},
};
use anistream_testkit::fixtures::invented;

use crate::harness::Harness;

const KNOWN: AnilistId = AnilistId::new(960_001);

async fn harness() -> Harness {
    let h = Harness::new().await;
    h.server().add_catalogue(vec![invented(
        KNOWN.get(),
        "FINISHED",
        12,
        (2030, 4, 6),
        &[],
        &[],
    )]);
    h
}

fn touch(root: &Path, rel: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"").unwrap();
}

fn view(updates: &[Event]) -> LocalView {
    updates
        .iter()
        .rev()
        .find_map(|u| match u {
            Event::Local(LocalEvent::View(v)) => Some(v.clone()),
            _ => None,
        })
        .expect("the view")
}

async fn scanned(h: &mut Harness, command: LocalCommand) -> LocalView {
    h.send(Command::Local(command));
    let updates =
        h.settle_until(|e| matches!(e, Event::Local(LocalEvent::View(v)) if !v.scanning)).await;
    view(&updates)
}

async fn settled(h: &mut Harness, command: LocalCommand) -> LocalView {
    h.send(Command::Local(command));
    view(&h.settle().await)
}

fn media() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "Invented 960001 [Release] (1080p)/Invented 960001 - 02.mkv");
    touch(dir.path(), "Invented 960001 [Release] (1080p)/Invented 960001 - 01.mkv");
    touch(dir.path(), "Quiet Orchard Days - 03.mp4");
    dir
}

fn add(dir: &tempfile::TempDir) -> LocalCommand {
    LocalCommand::AddFolder { path: dir.path().to_string_lossy().into_owned() }
}

#[tokio::test]
async fn a_folder_is_scanned_and_its_series_matched_by_name() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    assert_eq!(v.folders.len(), 1);
    assert!(v.folders[0].available);
    assert_eq!(v.folders[0].files, 3);
    let known = v.groups.iter().find(|g| g.name == "Invented 960001").expect("the series");
    assert_eq!(known.state, GroupState::Matched);
    assert_eq!(known.anilist, Some(KNOWN));
    assert_eq!(known.files, 2);
    let unknown = v.groups.iter().find(|g| g.name == "Quiet Orchard Days").expect("loose file");
    assert_eq!(unknown.state, GroupState::NotMatched);
    assert_eq!(unknown.reason, Some(NotMatchedReason::NoResult));
}

#[tokio::test]
async fn keeping_unmatched_makes_a_title_of_its_own_that_a_rescan_leaves_alone() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    let group = v.groups.iter().find(|g| g.state == GroupState::NotMatched).unwrap().id.clone();
    let v = settled(&mut h, LocalCommand::Keep { group: group.clone() }).await;
    let kept = v.groups.iter().find(|g| g.id == group).unwrap();
    assert_eq!(kept.state, GroupState::Kept);
    assert!(kept.anilist.unwrap().is_own(), "never an AniList id");
    let v = scanned(&mut h, LocalCommand::Scan).await;
    let kept = v.groups.iter().find(|g| g.id == group).unwrap();
    assert_eq!(kept.state, GroupState::Kept);
}

#[tokio::test]
async fn choosing_a_match_confirms_it() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    let group = v.groups.iter().find(|g| g.state == GroupState::NotMatched).unwrap().id.clone();
    let v =
        settled(&mut h, LocalCommand::Choose { group: group.clone(), anilist: KNOWN }).await;
    let chosen = v.groups.iter().find(|g| g.id == group).unwrap();
    assert_eq!(chosen.state, GroupState::Matched);
    assert_eq!(chosen.anilist, Some(KNOWN));
}

#[tokio::test]
async fn a_folder_that_is_not_there_is_unavailable_and_keeps_what_was_found() {
    let dir = media();
    let mut h = harness().await;
    scanned(&mut h, add(&dir)).await;
    let path = dir.path().to_path_buf();
    std::fs::rename(&path, path.with_extension("away")).unwrap();
    let v = scanned(&mut h, LocalCommand::Scan).await;
    assert!(!v.folders[0].available);
    assert_eq!(v.groups.len(), 2, "its series are remembered while the drive is out");
    std::fs::rename(path.with_extension("away"), &path).unwrap();
}

#[tokio::test]
async fn removing_a_folder_forgets_it_and_leaves_its_files() {
    let dir = media();
    let mut h = harness().await;
    scanned(&mut h, add(&dir)).await;
    let v = settled(
        &mut h,
        LocalCommand::RemoveFolder { path: dir.path().to_string_lossy().into_owned() },
    )
    .await;
    assert!(v.folders.is_empty() && v.groups.is_empty());
    assert!(dir.path().join("Quiet Orchard Days - 03.mp4").exists());
}
