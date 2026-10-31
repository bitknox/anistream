//! Your media: folders on this computer, read for video files and matched to AniList.
//!
//! What is kept: the folders and the groups found in them, as JSON in the database. Nothing is
//! written to the folders. A group is the files of one series. Matching goes folder name first
//! through AniList's search at the same confidence the sources use; a match below
//! [`SURE`] waits in Needs a look until the viewer confirms it, and only a sure or confirmed
//! match plays or reaches a tracker. Files the viewer keeps unmatched become a title with its own
//! metadata, which no tracker ever sees.

mod scan;
pub mod source;

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anistream_core::ids::AnilistId;
use anistream_meta::Meta;
use anistream_model::{
    event::Event,
    local::{
        FolderRow, GroupState, LocalCommand, LocalEvent, LocalGroup, LocalView,
        NotMatchedReason,
    },
};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use serde::{Deserialize, Serialize};

use crate::{outbox::Outbox, services::Services};

const KEY: &str = "local:state";

/// The confidence at which a match is acted on without asking.
pub const SURE: f64 = 0.85;

/// The most name lookups one scan makes, because AniList allows 30 requests a minute and the
/// rest of the app shares them. Groups left over are matched on the next scan.
const LOOKUPS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Standing {
    Matched,
    NeedsLook,
    NotMatched,
    Kept,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub episode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub folder: String,
    pub name: String,
    pub files: Vec<FileEntry>,
    pub standing: Standing,
    /// Said by the viewer: a rescan never changes it.
    #[serde(default)]
    pub decided: bool,
    pub anilist: Option<AnilistId>,
    pub matched_title: Option<String>,
    #[serde(default)]
    pub offline: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    folders: Vec<String>,
    /// Folders that could not be read at the last scan.
    #[serde(default)]
    unavailable: Vec<String>,
    groups: Vec<Group>,
    /// Own-metadata ids handed out so far.
    #[serde(default)]
    own: u32,
}

/// The handles Your media needs, cheap to clone.
#[derive(Clone)]
pub struct Local {
    store: Store,
    scanning: Arc<Mutex<bool>>,
}

impl Local {
    pub fn new(store: Store) -> Self {
        Self { store, scanning: Default::default() }
    }

    fn state(&self) -> State {
        self.store
            .get_meta_text(KEY)
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, state: &State) {
        if let Ok(text) = serde_json::to_string(state) {
            let _ = self.store.set_meta_text(KEY, &text);
        }
    }

    fn change(&self, change: impl FnOnce(&mut State)) {
        let mut state = self.state();
        change(&mut state);
        self.save(&state);
    }

    pub fn groups(&self) -> Vec<Group> {
        self.state().groups
    }

    fn view(&self) -> LocalView {
        let state = self.state();
        let scanning = *self.scanning.lock().unwrap_or_else(|e| e.into_inner());
        LocalView {
            folders: state
                .folders
                .iter()
                .map(|path| FolderRow {
                    path: path.clone(),
                    files: state
                        .groups
                        .iter()
                        .filter(|g| &g.folder == path)
                        .map(|g| g.files.len() as u32)
                        .sum(),
                    available: !state.unavailable.contains(path),
                })
                .collect(),
            groups: state.groups.iter().map(row).collect(),
            scanning,
        }
    }

    fn publish(&self, tx: &Outbox) {
        let _ = tx.send(Event::Local(LocalEvent::View(self.view())));
    }

    /// Put the source in the chain, where `order` places it. Replaces the one there.
    pub fn put_source(&self, registry: &ProviderRegistry, order: &[String]) {
        registry.remove(source::ID);
        if self.state().groups.is_empty() {
            return;
        }
        registry.put(Arc::new(source::LocalSource::new(self.groups())));
        registry.rank_by(|p| crate::sources::rank(p, order));
    }
}

/// Your media as a scene shows it: one folder, a matched series, one that needs a look and one
/// not matched. Made-up; no file exists behind the paths.
pub fn seed_fixture(store: &Store) {
    let group = |name: &str, standing, anilist: Option<u32>, title: Option<&str>| Group {
        id: format!("/Volumes/Media|{}", name.to_lowercase()),
        folder: "/Volumes/Media".into(),
        name: name.into(),
        files: (1..=3)
            .map(|n| FileEntry {
                path: format!("/Volumes/Media/{name}/{name} - 0{n}.mkv"),
                episode: Some(n),
            })
            .collect(),
        standing,
        decided: false,
        anilist: anilist.map(AnilistId::new),
        matched_title: title.map(str::to_owned),
        offline: false,
    };
    let local = Local::new(store.clone());
    local.save(&State {
        folders: vec!["/Volumes/Media".into()],
        unavailable: Vec::new(),
        groups: vec![
            group(
                "Stardust Wayfarer",
                Standing::Matched,
                Some(900_001),
                Some("Stardust Wayfarer"),
            ),
            group(
                "Lantern Harbour",
                Standing::NeedsLook,
                Some(900_002),
                Some("Lantern Harbour Season 2"),
            ),
            group("Quiet Orchard Days", Standing::NotMatched, None, None),
        ],
        own: 0,
    });
}

/// The folders and groups saved in `store`.
pub fn saved(store: &Store) -> Vec<Group> {
    Local::new(store.clone()).groups()
}

fn row(group: &Group) -> LocalGroup {
    LocalGroup {
        id: group.id.clone(),
        name: group.name.clone(),
        files: group.files.len() as u32,
        state: match group.standing {
            Standing::Matched => GroupState::Matched,
            Standing::NeedsLook => GroupState::NeedsLook,
            Standing::NotMatched => GroupState::NotMatched,
            Standing::Kept => GroupState::Kept,
        },
        anilist: group.anilist,
        matched_title: group.matched_title.clone(),
        reason: (group.standing == Standing::NotMatched).then_some(if group.offline {
            NotMatchedReason::Offline
        } else {
            NotMatchedReason::NoResult
        }),
    }
}

/// Run one command. A scan runs as a task.
pub fn command(command: LocalCommand, services: &Services, order: Vec<String>, tx: &Outbox) {
    let local = services.local.clone();
    let (registry, meta, tx) = (services.registry.clone(), services.meta.clone(), tx.clone());
    match command {
        LocalCommand::Load => local.publish(&tx),
        LocalCommand::AddFolder { path } => {
            local.change(|s| {
                if !s.folders.contains(&path) {
                    s.folders.push(path);
                }
            });
            scan_all(local, registry, meta, order, tx);
        }
        LocalCommand::RemoveFolder { path } => {
            local.change(|s| {
                s.folders.retain(|f| f != &path);
                s.unavailable.retain(|f| f != &path);
                s.groups.retain(|g| g.folder != path);
            });
            local.put_source(&registry, &order);
            local.publish(&tx);
        }
        LocalCommand::Scan => scan_all(local, registry, meta, order, tx),
        LocalCommand::Choose { group, anilist } => {
            tokio::spawn(async move {
                let title =
                    meta.media(anilist).await.ok().map(|m| m.title.display().to_owned());
                local.change(|s| {
                    if let Some(g) = s.groups.iter_mut().find(|g| g.id == group) {
                        g.standing = Standing::Matched;
                        g.decided = true;
                        g.anilist = Some(anilist);
                        g.matched_title = title;
                        g.offline = false;
                    }
                });
                local.put_source(&registry, &order);
                local.publish(&tx);
            });
        }
        LocalCommand::Keep { group } => {
            local.change(|s| {
                let next = s.own;
                if let Some(g) = s.groups.iter_mut().find(|g| g.id == group) {
                    let reuse = g.anilist.filter(|id| id.is_own());
                    let id = reuse.unwrap_or_else(|| AnilistId::own(next));
                    g.standing = Standing::Kept;
                    g.decided = true;
                    g.anilist = Some(id);
                    g.matched_title = Some(g.name.clone());
                    g.offline = false;
                    if reuse.is_none() {
                        s.own += 1;
                    }
                }
            });
            local.put_source(&registry, &order);
            local.publish(&tx);
        }
    }
}

/// Read every folder, then match what is new. The view is sent when the files are found and
/// again when the matching is done.
fn scan_all(
    local: Local,
    registry: ProviderRegistry,
    meta: Meta,
    order: Vec<String>,
    tx: Outbox,
) {
    {
        let mut scanning = local.scanning.lock().unwrap_or_else(|e| e.into_inner());
        if *scanning {
            return;
        }
        *scanning = true;
    }
    tokio::spawn(async move {
        let folders = local.state().folders;
        let mut unavailable = Vec::new();
        let mut found: Vec<(String, Vec<scan::Series>)> = Vec::new();
        for folder in folders {
            let path = folder.clone();
            let read = tokio::task::spawn_blocking(move || scan::scan(Path::new(&path))).await;
            match read {
                Ok(Ok(series)) => found.push((folder, series)),
                _ => unavailable.push(folder),
            }
        }
        // Keep what the viewer decided and what was matched; groups from a folder that could
        // not be read stay as they were.
        local.change(|s| {
            let old = std::mem::take(&mut s.groups);
            s.unavailable = unavailable.clone();
            s.groups =
                old.iter().filter(|g| unavailable.contains(&g.folder)).cloned().collect();
            for (folder, series) in &found {
                for series in series {
                    let id = format!("{folder}|{}", series.name.to_lowercase());
                    let files: Vec<FileEntry> = series
                        .files
                        .iter()
                        .map(|f| FileEntry {
                            path: f.path.to_string_lossy().into_owned(),
                            episode: f.episode,
                        })
                        .collect();
                    let group = match old.iter().find(|g| g.id == id) {
                        Some(known) => Group { files, ..known.clone() },
                        None => Group {
                            id,
                            folder: folder.clone(),
                            name: series.name.clone(),
                            files,
                            standing: Standing::NotMatched,
                            decided: false,
                            anilist: None,
                            matched_title: None,
                            offline: false,
                        },
                    };
                    s.groups.push(group);
                }
            }
        });
        local.put_source(&registry, &order);
        local.publish(&tx);

        let mut lookups = 0;
        for group in local.groups() {
            let wants = group.standing == Standing::NotMatched && !group.decided;
            if !wants || lookups >= LOOKUPS {
                continue;
            }
            lookups += 1;
            let result = crate::titlematch::candidates(&meta, &group.name, None).await;
            local.change(|s| {
                let Some(g) = s.groups.iter_mut().find(|g| g.id == group.id) else { return };
                match &result {
                    Err(()) => g.offline = true,
                    Ok(found) => {
                        g.offline = false;
                        if let Some(best) = found.first() {
                            let clear =
                                found.get(1).is_none_or(|next| best.score - next.score >= 0.05);
                            g.standing = if best.score >= SURE && clear {
                                Standing::Matched
                            } else {
                                Standing::NeedsLook
                            };
                            g.anilist = Some(best.id);
                            g.matched_title = Some(best.title.clone());
                        }
                    }
                }
            });
        }
        *local.scanning.lock().unwrap_or_else(|e| e.into_inner()) = false;
        local.put_source(&registry, &order);
        local.publish(&tx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_that_waits_has_no_reason_but_one_not_matched_does() {
        let mut group = Group {
            id: "a".into(),
            folder: "f".into(),
            name: "n".into(),
            files: Vec::new(),
            standing: Standing::NeedsLook,
            decided: false,
            anilist: None,
            matched_title: None,
            offline: false,
        };
        assert_eq!(row(&group).reason, None);
        group.standing = Standing::NotMatched;
        assert_eq!(row(&group).reason, Some(NotMatchedReason::NoResult));
        group.offline = true;
        assert_eq!(row(&group).reason, Some(NotMatchedReason::Offline));
    }
}
