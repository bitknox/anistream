//! Schema and forward-only migrations.
//!
//! The table layout encodes two decisions from the design that are hard to retrofit, so
//! they are here from the first migration rather than added later:
//!
//! 1. **`watch_event` is an append-only log, and `watch_progress` is a projection of it.**
//!    History is richer than any tracker can represent — position, duration watched, which
//!    provider served it, which translation — and none of that survives a round-trip
//!    through AniList. So local is the source of truth and sync is derived from it, not
//!    the other way around.
//!
//! 2. **`sync_outbox` is a durable table, not an in-memory queue.** Progress recorded
//!    while offline has to survive a process kill, otherwise "watched on a plane" silently
//!    loses episodes.
//!
//! `mapping_override` is deliberately separate from `mapping_resolution`: a dataset
//! refresh clears cached resolutions but must never touch a correction the user made by
//! hand.

use rusqlite::Connection;

/// Ordered migrations. Append only — never edit an existing entry, since it may already
/// have run on a user's database.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial",
        r#"
    -- Cached AniList responses, keyed by media id.
    CREATE TABLE media_cache (
        anilist_id   INTEGER PRIMARY KEY,
        payload      TEXT    NOT NULL,
        fetched_at   INTEGER NOT NULL
    );

    -- Materialised ID mapping, merged from the upstream datasets on refresh.
    -- Parsed once per refresh rather than on every launch: re-reading ~15 MB of JSON at
    -- startup would be pure waste.
    CREATE TABLE mapping (
        anilist_id      INTEGER PRIMARY KEY,
        mal_id          INTEGER,
        kitsu_id        INTEGER,
        anidb_id        INTEGER,
        tvdb_id         INTEGER,
        tmdb_id         INTEGER,
        title_normal    TEXT,
        -- INTEGER, not TEXT: SQLite's TEXT affinity silently coerces a written integer
        -- into a string, so reading it back as a number then fails at runtime.
        episode_offset  INTEGER,
        source          TEXT    NOT NULL
    );
    CREATE INDEX idx_mapping_mal   ON mapping(mal_id);
    CREATE INDEX idx_mapping_title ON mapping(title_normal);

    -- Episode redirection rules from anime-relations, for fansub continuous numbering.
    CREATE TABLE episode_relation (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        anilist_id    INTEGER NOT NULL,
        src_from      INTEGER NOT NULL,
        src_to        INTEGER NOT NULL,
        dst_anilist   INTEGER NOT NULL,
        dst_from      INTEGER NOT NULL,
        dst_to        INTEGER NOT NULL
    );
    CREATE INDEX idx_relation_anilist ON episode_relation(anilist_id);

    -- Refresh bookkeeping for each RefreshableDataset: the stored ETag makes the
    -- steady-state check a 304 with no body.
    CREATE TABLE dataset_state (
        name        TEXT PRIMARY KEY,
        url         TEXT    NOT NULL,
        etag        TEXT,
        fetched_at  INTEGER,
        item_count  INTEGER,
        last_error  TEXT
    );

    -- Rung 1 of the resolution ladder: a correction the user made by hand.
    -- Always wins, never expires, and survives every dataset refresh.
    CREATE TABLE mapping_override (
        anilist_id   INTEGER NOT NULL,
        provider_id  TEXT    NOT NULL,
        provider_key TEXT    NOT NULL,
        created_at   INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, provider_id)
    );

    -- Rung 2: a previously successful automatic resolution, with the rung that produced
    -- it so low-confidence matches can be re-examined later.
    CREATE TABLE mapping_resolution (
        anilist_id   INTEGER NOT NULL,
        provider_id  TEXT    NOT NULL,
        provider_key TEXT    NOT NULL,
        confidence   REAL    NOT NULL,
        rung         INTEGER NOT NULL,
        resolved_at  INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, provider_id)
    );

    -- Append-only watch log. The source of truth.
    CREATE TABLE watch_event (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        anilist_id    INTEGER NOT NULL,
        episode       TEXT    NOT NULL,
        position_secs REAL    NOT NULL,
        duration_secs REAL,
        watched_secs  REAL    NOT NULL DEFAULT 0,
        provider_id   TEXT,
        translation   TEXT,
        completed     INTEGER NOT NULL DEFAULT 0,
        at            INTEGER NOT NULL
    );
    CREATE INDEX idx_event_title ON watch_event(anilist_id, at DESC);
    CREATE INDEX idx_event_at    ON watch_event(at DESC);

    -- Fast-read projection for the CONTINUE rail and resume prompt.
    CREATE TABLE watch_progress (
        anilist_id     INTEGER PRIMARY KEY,
        last_episode   TEXT    NOT NULL,
        last_position  REAL    NOT NULL,
        last_duration  REAL,
        episodes_done  INTEGER NOT NULL DEFAULT 0,
        updated_at     INTEGER NOT NULL
    );
    CREATE INDEX idx_progress_recent ON watch_progress(updated_at DESC);

    -- Durable per-tracker outbox. Survives a process kill; drains on reconnect.
    CREATE TABLE sync_outbox (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        tracker_id  TEXT    NOT NULL,
        op          TEXT    NOT NULL,
        anilist_id  INTEGER NOT NULL,
        created_at  INTEGER NOT NULL,
        attempts    INTEGER NOT NULL DEFAULT 0,
        next_retry  INTEGER NOT NULL DEFAULT 0,
        last_error  TEXT
    );
    CREATE INDEX idx_outbox_ready ON sync_outbox(tracker_id, next_retry);

    -- Per-episode extras: aniskip intervals and filler/recap flags.
    CREATE TABLE episode_meta (
        anilist_id  INTEGER NOT NULL,
        episode     TEXT    NOT NULL,
        op_start    REAL,
        op_end      REAL,
        ed_start    REAL,
        ed_end      REAL,
        is_filler   INTEGER NOT NULL DEFAULT 0,
        is_recap    INTEGER NOT NULL DEFAULT 0,
        fetched_at  INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, episode)
    );
    "#,
    ),
    (
        "0002_sync_state",
        r#"
    -- Small key/value store for cursors that are not worth a table of their own.
    -- The tracker sync uses it for the last successful pull time per tracker, which is the
    -- baseline last-write-wins compares a local edit against.
    CREATE TABLE app_state (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    -- Titles for ids we have seen, so a sync conflict can name the show rather than showing a
    -- bare AniList id. Separate from `media_cache` because it has no TTL: a title is stable,
    -- and this is a lookup table rather than a cache.
    CREATE TABLE title_index (
        anilist_id INTEGER PRIMARY KEY,
        title      TEXT NOT NULL,
        seen_at    INTEGER NOT NULL
    );
    "#,
    ),
    (
        "0002_downloads",
        r#"
    -- The download queue, and it has to be a table rather than in-memory state.
    --
    -- A download outlives the process that started it in the only sense that matters: the *files*
    -- are still on disk after a restart, and a queue that forgot about them would either re-fetch
    -- from zero or leave orphans nobody can find. librqbit can resume a partial torrent, so the
    -- magnet and the target path are the two things worth persisting.
    CREATE TABLE download (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        anilist_id   INTEGER NOT NULL,
        episode      TEXT    NOT NULL,
        title        TEXT    NOT NULL,
        magnet       TEXT    NOT NULL,
        -- queued · active · paused · done · failed
        state        TEXT    NOT NULL,
        -- Where the finished file landed. Null until it is known, which is after metadata arrives.
        path         TEXT,
        downloaded   INTEGER NOT NULL DEFAULT 0,
        total        INTEGER NOT NULL DEFAULT 0,
        -- Null unless `state = 'failed'`. Kept so the screen can say *why* rather than just that.
        error        TEXT,
        created_at   INTEGER NOT NULL,
        updated_at   INTEGER NOT NULL,
        -- One row per episode: asking twice is not two downloads, and without this a held-down key
        -- would queue the same file repeatedly.
        UNIQUE(anilist_id, episode)
    );
    CREATE INDEX idx_download_state ON download(state, created_at);
    "#,
    ),
];

/// Apply any migrations the database has not yet seen.
pub fn migrate(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migration (
            name       TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let mut applied = 0;
    for (name, sql) in MIGRATIONS {
        let already: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migration WHERE name = ?1)",
            [name],
            |r| r.get(0),
        )?;
        if already {
            continue;
        }
        // Each migration is one transaction: a partial schema is worse than none.
        conn.execute_batch(&format!("BEGIN;\n{sql}\nCOMMIT;"))?;
        conn.execute(
            "INSERT INTO schema_migration (name, applied_at) VALUES (?1, unixepoch())",
            [name],
        )?;
        tracing::info!(migration = name, "applied migration");
        applied += 1;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_once_and_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&conn).unwrap(), MIGRATIONS.len() as u32);
        // Running again must be a no-op, not an error — this is what happens on every
        // subsequent startup.
        assert_eq!(migrate(&conn).unwrap(), 0);
    }

    #[test]
    fn every_expected_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        for table in [
            "media_cache",
            "mapping",
            "episode_relation",
            "dataset_state",
            "mapping_override",
            "mapping_resolution",
            "watch_event",
            "watch_progress",
            "sync_outbox",
            "episode_meta",
        ] {
            let found: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(found, "table {table} missing");
        }
    }

    #[test]
    fn migration_names_are_unique() {
        let mut names: Vec<&str> = MIGRATIONS.iter().map(|(n, _)| *n).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate migration name");
    }
}
