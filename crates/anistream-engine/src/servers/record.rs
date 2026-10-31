//! The saved servers, as the database keeps them.

use anistream_core::ids::{AnidbId, AnilistId, ExternalIds, TmdbId, TvdbId};
use anistream_jellyfin::{SeriesMatch, model::Session};
use anistream_model::servers::{CheckedServer, LibraryChoice, ServerRow};
use anistream_store::Store;
use serde::{Deserialize, Serialize};

const KEY: &str = "servers:list";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub name: String,
    pub address: String,
    pub version: String,
    pub user_id: String,
    pub user_name: String,
    #[serde(default)]
    pub libraries: Vec<LibraryRecord>,
    /// Whether the person has chosen libraries yet. Until then none is read.
    #[serde(default)]
    pub libraries_chosen: bool,
    #[serde(default = "yes")]
    pub report: bool,
    #[serde(default)]
    pub signed_out: bool,
    #[serde(default)]
    pub series: u32,
    #[serde(default)]
    pub found: Vec<Found>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub ticked: bool,
}

/// One series and the AniList entry it was matched to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Found {
    pub series: String,
    pub name: String,
    pub anilist: u32,
    #[serde(default)]
    pub anidb: Option<u32>,
    #[serde(default)]
    pub tvdb: Option<u32>,
    #[serde(default)]
    pub tmdb: Option<u32>,
    #[serde(default)]
    pub season: Option<u32>,
}

impl Record {
    pub fn new(checked: &CheckedServer, session: &Session) -> Self {
        Self {
            id: checked.id.clone(),
            name: checked.name.clone(),
            address: checked.address.clone(),
            version: checked.version.clone(),
            user_id: session.user_id.clone(),
            user_name: session.user_name.clone(),
            libraries: Vec::new(),
            libraries_chosen: false,
            report: true,
            signed_out: false,
            series: 0,
            found: Vec::new(),
        }
    }

    pub fn matches(&self) -> Vec<SeriesMatch> {
        self.found
            .iter()
            .map(|f| SeriesMatch {
                series: f.series.clone(),
                name: f.name.clone(),
                anilist: AnilistId::new(f.anilist),
                ids: ExternalIds {
                    anilist: Some(AnilistId::new(f.anilist)),
                    mal: None,
                    anidb: f.anidb.map(AnidbId::new),
                    tvdb: f.tvdb.map(TvdbId::new),
                    tmdb: f.tmdb.map(TmdbId::new),
                },
                season: f.season,
            })
            .collect()
    }

    pub fn set_matches(&mut self, matches: Vec<SeriesMatch>, series: u32) {
        self.series = series;
        self.signed_out = false;
        self.found = matches
            .into_iter()
            .map(|m| Found {
                series: m.series,
                name: m.name,
                anilist: m.anilist.get(),
                anidb: m.ids.anidb.map(|i| i.get()),
                tvdb: m.ids.tvdb.map(|i| i.get()),
                tmdb: m.ids.tmdb.map(|i| i.get()),
                season: m.season,
            })
            .collect();
    }

    pub fn row(&self) -> ServerRow {
        ServerRow {
            id: self.id.clone(),
            name: self.name.clone(),
            address: self.address.clone(),
            version: self.version.clone(),
            user: self.user_name.clone(),
            libraries: self
                .libraries
                .iter()
                .map(|l| LibraryChoice {
                    id: l.id.clone(),
                    name: l.name.clone(),
                    kind: l.kind.clone(),
                    ticked: l.ticked,
                })
                .collect(),
            series: self.series,
            matched: self.found.len() as u32,
            report: self.report,
            signed_out: self.signed_out,
        }
    }
}

pub fn load(store: &Store) -> Vec<Record> {
    store
        .get_meta_text(KEY)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(store: &Store, records: &[Record]) {
    if let Ok(text) = serde_json::to_string(records) {
        let _ = store.set_meta_text(KEY, &text);
    }
}

/// Add a server, or replace the one with the same id and keep what was chosen on it.
pub fn upsert(store: &Store, mut record: Record) {
    let mut records = load(store);
    match records.iter_mut().find(|r| r.id == record.id) {
        Some(old) => {
            record.libraries = std::mem::take(&mut old.libraries);
            record.libraries_chosen = old.libraries_chosen;
            record.report = old.report;
            record.found = std::mem::take(&mut old.found);
            record.series = old.series;
            *old = record;
        }
        None => records.push(record),
    }
    save(store, &records);
}

pub fn update(store: &Store, id: &str, change: impl FnOnce(&mut Record)) {
    let mut records = load(store);
    if let Some(record) = records.iter_mut().find(|r| r.id == id) {
        change(record);
        save(store, &records);
    }
}

pub fn remove(store: &Store, id: &str) {
    let mut records = load(store);
    records.retain(|r| r.id != id);
    save(store, &records);
}

/// Mika's Jellyfin as a scene shows it: signed in, the anime library read, three series and two
/// of them matched. Made-up; there is no server behind the address.
pub fn seed_fixture(store: &Store) {
    let checked = CheckedServer {
        id: "0f1e2d3c".into(),
        name: "Mika's Jellyfin".into(),
        version: "10.9.11".into(),
        address: "http://192.168.1.20:8096".into(),
        quick_connect: true,
    };
    let session =
        Session { token: String::new(), user_id: "u-mika".into(), user_name: "mika".into() };
    let mut record = Record::new(&checked, &session);
    record.libraries = vec![
        LibraryRecord {
            id: "lib-anime".into(),
            name: "Anime Shows".into(),
            kind: Some("tvshows".into()),
            ticked: true,
        },
        LibraryRecord {
            id: "lib-films".into(),
            name: "Films".into(),
            kind: Some("movies".into()),
            ticked: false,
        },
    ];
    record.libraries_chosen = true;
    record.series = 3;
    record.found = vec![
        Found {
            series: "s-1".into(),
            name: "Stardust Wayfarer".into(),
            anilist: 900_001,
            anidb: None,
            tvdb: None,
            tmdb: None,
            season: None,
        },
        Found {
            series: "s-2".into(),
            name: "Lantern Harbour".into(),
            anilist: 900_002,
            anidb: Some(424_242),
            tvdb: None,
            tmdb: None,
            season: None,
        },
    ];
    upsert(store, record);
}
