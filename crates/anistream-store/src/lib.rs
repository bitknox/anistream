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

pub mod dataset;
pub mod download;
pub mod history;
pub mod mapping;
pub mod outbox;
pub mod schema;
pub mod state;
pub mod stats;

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use rusqlite::Connection;

pub use dataset::{DatasetState, Mapping, MappingInput};
pub use download::{Download, DownloadState};
pub use history::{MIN_RESUME_SECS, Progress, RESUME_CEILING, WatchEvent, is_complete};
pub use mapping::{ResolutionRung, ResolvedMapping};
pub use outbox::OutboxEntry;

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
