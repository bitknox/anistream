//! Local SQLite store.
//!
//! **This is the source of truth for watch state.** Trackers are a projection of what is
//! here, never the reverse — which is what lets anistream work with no account, no
//! network, and no tracker configured at all.
//!
//! # Blocking
//!
//! `rusqlite` is synchronous and these methods block. The UI must never call them
//! directly on the event-loop thread; wrap them in `tokio::task::spawn_blocking`. The
//! store is [`Clone`] and cheap to clone, so handing a copy to a blocking task is the
//! intended pattern.

pub mod airing_times;
pub mod cache;
pub mod dataset;
pub mod download;
pub mod episode_notice;
pub mod history;
pub mod library_status;
pub mod mapping;
pub mod outbox;
pub mod party_room;
pub mod preference;
pub mod schema;
pub mod server_health;
pub mod show_setting;
pub mod state;
pub mod stats;
pub mod track_memory;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::Connection;

pub use dataset::{DatasetState, Mapping, MappingInput};
pub use download::{Download, DownloadState};
pub use episode_notice::EpisodeNotice;
pub use history::{MIN_RESUME_SECS, Progress, RESUME_CEILING, WatchEvent, is_complete};
pub use mapping::{ResolutionRung, ResolvedMapping};
pub use outbox::OutboxEntry;
pub use party_room::PartyRoom;
pub use preference::ReleasePin;
pub use server_health::{ServerFailure, ServerHealth, ServerKey};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("could not create data directory {path}: {source}")]
    DataDir {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("serialising {what}: {source}")]
    Encode {
        what: &'static str,
        #[source]
        source: serde_json::Error,
    },

    /// The connection mutex was poisoned by a panic in another thread. Surfaced rather
    /// than papered over, because it means some earlier operation died mid-write.
    #[error("store lock poisoned — a previous operation panicked")]
    Poisoned,

    /// A row that must exist and does not — a write reporting success and then not being there.
    #[error("{0} is missing")]
    Missing(String),

    /// An import file this build does not understand.
    ///
    /// Refused rather than read optimistically: a newer export could mean anything, and guessing
    /// at it risks writing wrong progress into someone's history.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Handle to the local database.
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Store")
    }
}

impl StoreError {
    /// Whether this says the file itself is damaged, as a file that is not a database or has
    /// corrupt pages does, rather than that it could not be got at.
    pub fn is_damage(&self) -> bool {
        use rusqlite::ErrorCode::{DatabaseCorrupt, NotADatabase};
        matches!(
            self,
            Self::Sqlite(rusqlite::Error::SqliteFailure(e, _))
                if matches!(e.code, NotADatabase | DatabaseCorrupt)
        )
    }
}

/// Move the database at `path` aside: to `<name>.damaged-<date>`, with a count after it when that
/// is taken. Any journal SQLite left beside it goes with it, since one left behind would be read
/// into the new database.
fn set_aside(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().map_or_else(|| "anistream.db".into(), |n| n.to_string_lossy());
    let stem = format!("{name}.damaged-{}", today());
    let aside = (1..)
        .map(|n| if n == 1 { stem.clone() } else { format!("{stem}-{n}") })
        .map(|aside| path.with_file_name(aside))
        .find(|aside| !aside.exists())
        .expect("a free name");
    let moved = |from: &Path, to: &Path| {
        std::fs::rename(from, to)
            .map_err(|source| StoreError::DataDir { path: from.display().to_string(), source })
    };
    moved(path, &aside)?;
    for journal in ["-wal", "-shm", "-journal"] {
        let from = PathBuf::from(format!("{}{journal}", path.display()));
        if from.exists() {
            moved(&from, &PathBuf::from(format!("{}{journal}", aside.display())))?;
        }
    }
    Ok(aside)
}

/// Today's date in UTC, as `2026-10-30`.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    // Days to a civil date, after Howard Hinnant's `civil_from_days`.
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

impl Store {
    /// Open (creating if needed) and migrate the database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::DataDir {
                path: parent.display().to_string(),
                source,
            })?;
        }
        let conn = Connection::open(path)?;
        schema::migrate(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// [`Self::open`], with a database too damaged to open moved aside and a new one started in
    /// its place: the path it was moved to comes back with the store, for saying where it is.
    ///
    /// Only damage moves it: a file that is not a database or whose pages are corrupt. A file
    /// another process holds locked, or one that could not be read, is said as the error it is
    /// and left where it is.
    pub fn open_or_set_aside(path: impl AsRef<Path>) -> Result<(Self, Option<PathBuf>)> {
        let path = path.as_ref();
        match Self::open(path) {
            Err(e) if e.is_damage() => {
                let aside = set_aside(path)?;
                tracing::warn!(error = %e, aside = %aside.display(), "database damaged; set aside");
                Ok((Self::open(path)?, Some(aside)))
            }
            opened => opened.map(|store| (store, None)),
        }
    }

    /// An ephemeral in-memory database. For tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::migrate(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Run `f` with the connection held.
    pub(crate) fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        f(&guard)
    }

    /// Run `f` inside a transaction, rolling back on error.
    pub(crate) fn with_tx<T>(
        &self,
        f: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let mut guard = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = guard.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_damaged_database_is_set_aside_and_a_new_one_started() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("anistream.db");
        std::fs::write(&path, b"not a database, just the start of a shopping list").unwrap();
        std::fs::write(tmp.path().join("anistream.db-wal"), b"stale journal").unwrap();

        let (store, aside) = Store::open_or_set_aside(&path).unwrap();
        let aside = aside.expect("set aside");
        let name = aside.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("anistream.db.damaged-20"), "{name}");
        assert_eq!(
            std::fs::read(&aside).unwrap(),
            b"not a database, just the start of a shopping list"
        );
        // SQLite lets go of a journal it could not use as the failed open closes, so the new
        // database starts clean either way.
        assert!(store.pending_downloads().unwrap().is_empty(), "a new, working database");
        drop(store);

        // Damaged again the same day: a name of its own, the first kept.
        std::fs::write(&path, b"still not a database").unwrap();
        let _ = std::fs::remove_file(tmp.path().join("anistream.db-wal"));
        let _ = std::fs::remove_file(tmp.path().join("anistream.db-shm"));
        let (_, again) = Store::open_or_set_aside(&path).unwrap();
        let again = again.expect("set aside again");
        assert_eq!(again.file_name().unwrap().to_string_lossy(), format!("{name}-2"));
        assert!(aside.exists());
    }

    #[test]
    fn a_sound_database_is_opened_where_it_is() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("anistream.db");
        drop(Store::open(&path).unwrap());
        let (_, aside) = Store::open_or_set_aside(&path).unwrap();
        assert_eq!(aside, None);
    }

    #[test]
    fn a_database_another_process_holds_is_an_error_and_never_set_aside() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("anistream.db");
        drop(Store::open(&path).unwrap());
        // Another app holding it, as a long write would.
        let holder = Connection::open(&path).unwrap();
        holder.execute_batch("PRAGMA locking_mode = EXCLUSIVE; BEGIN EXCLUSIVE;").unwrap();

        let refused = Store::open_or_set_aside(&path).map(|_| ()).unwrap_err();
        assert!(!refused.is_damage(), "{refused}");
        let left: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("damaged"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
        drop(holder);
    }

    #[test]
    fn opening_in_memory_runs_migrations() {
        let store = Store::open_in_memory().unwrap();
        let tables: i64 = store
            .with_conn(|c| {
                Ok(c.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert!(tables >= 10, "expected the full schema, got {tables} tables");
    }

    #[test]
    fn opening_a_file_creates_missing_parent_directories() {
        // First run has no data directory yet; that must not be an error.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("deeper").join("anistream.db");
        let store = Store::open(&path).unwrap();
        assert!(path.exists());
        drop(store);

        // Reopening an existing database is also fine, and migrations do not re-run.
        let reopened = Store::open(&path).unwrap();
        let applied = reopened.with_conn(|c| Ok(schema::migrate(c)?)).unwrap();
        assert_eq!(applied, 0);
    }

    #[test]
    fn transactions_roll_back_on_error() {
        let store = Store::open_in_memory().unwrap();
        let err = store.with_tx(|tx| {
            tx.execute(
                "INSERT INTO dataset_state (name, url) VALUES ('x', 'https://x.test')",
                [],
            )?;
            Err::<(), _>(StoreError::Poisoned)
        });
        assert!(err.is_err());
        let count: i64 = store
            .with_conn(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM dataset_state", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 0, "failed transaction must leave no rows behind");
    }
}
