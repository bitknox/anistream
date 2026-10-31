//! The store's tables, and how a database gets from any earlier version of them to this one.
//!
//! Each migration is a list of statements that takes the database from one version to the next.
//! They run when a store opens, in order, each one in a single batch with a row in `migrations`
//! that records it. The row's primary key is the version, so when two instances open the same
//! database at once, the second to insert a version fails, its batch rolls back whole, and it
//! carries on from what the first one wrote. That needs nothing but the atomic batch every
//! [`Database`] offers: SQLite, Postgres and D1 alike.
//!
//! Migrations only go forward. A store refuses a database whose version is newer than any it
//! knows, rather than guess what a newer store changed; the way back from that is to deploy the
//! newer version again, or to restore a backup.
//!
//! A database from before migrations existed has the tables and no `migrations` table. It reads
//! as version 0, and the first migration, whose statements all say `IF NOT EXISTS`, records it as
//! version 1 without changing anything in it.
//!
//! Adding a migration: append a list to [`MIGRATIONS`]. Never edit one that has shipped, since
//! stores that ran it will not run it again. Keep each statement in the subset SQLite, Postgres
//! and D1 share, with `{int}` where a 64-bit integer column goes and `{bytes}` for a byte column.

use crate::{
    db::{Database, DbError, Dialect, Statement, Value},
    store::StoreError,
};

/// Every migration, in order. Migration `n` (counting from one) takes the database to version `n`.
pub const MIGRATIONS: &[&[&str]] = &[
    // 1: the tables as they were when versions began.
    &[
        "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        "CREATE TABLE IF NOT EXISTS members (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, role TEXT NOT NULL,
            joined_at {int} NOT NULL, removed_at {int})",
        "CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY, member_id TEXT NOT NULL, label TEXT NOT NULL,
            token_hash TEXT NOT NULL UNIQUE, created_at {int} NOT NULL, last_seen {int},
            revoked_at {int})",
        "CREATE TABLE IF NOT EXISTS invites (
            id TEXT PRIMARY KEY, code_hash TEXT NOT NULL UNIQUE, kind TEXT NOT NULL,
            name TEXT NOT NULL, role TEXT NOT NULL, member_id TEXT, created_by TEXT,
            created_at {int} NOT NULL, expires_at {int} NOT NULL, used_at {int}, used_by TEXT,
            cancelled_at {int})",
        "CREATE TABLE IF NOT EXISTS admin_links (
            code_hash TEXT PRIMARY KEY, member_id TEXT NOT NULL, created_at {int} NOT NULL,
            expires_at {int} NOT NULL, used_at {int})",
        "CREATE TABLE IF NOT EXISTS sessions (
            token_hash TEXT PRIMARY KEY, member_id TEXT NOT NULL, created_at {int} NOT NULL,
            expires_at {int} NOT NULL)",
        "CREATE TABLE IF NOT EXISTS plugins (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, version TEXT NOT NULL,
            allowed_hosts TEXT NOT NULL, translations TEXT NOT NULL, sha256 TEXT NOT NULL,
            size {int} NOT NULL, published_at {int} NOT NULL)",
        "CREATE TABLE IF NOT EXISTS uploads (
            id TEXT PRIMARY KEY, member_id TEXT NOT NULL, size {int} NOT NULL,
            created_at {int} NOT NULL)",
        "CREATE TABLE IF NOT EXISTS failures (
            key TEXT PRIMARY KEY, window_start {int} NOT NULL, attempts {int} NOT NULL)",
        // Open joins per address, counted like failures but over their own window.
        "CREATE TABLE IF NOT EXISTS open_joins (
            key TEXT PRIMARY KEY, window_start {int} NOT NULL, attempts {int} NOT NULL)",
    ],
    // 2: open-join links are off for every store. Turning one on now needs an operator contact,
    // so a store that was open before that rule closes, and its owner opens it again once there
    // is a contact. The devices that joined keep working.
    &["DELETE FROM meta WHERE key = 'open_code'"],
];

/// The version this code takes a database to.
pub const VERSION: u32 = MIGRATIONS.len() as u32;

/// A statement as this dialect writes it.
pub fn statement(dialect: Dialect, sql: &str) -> String {
    let (int, bytes) = match dialect {
        // SQLite's INTEGER is 64-bit; Postgres's is not.
        Dialect::Sqlite => ("INTEGER", "BLOB"),
        Dialect::Postgres => ("BIGINT", "BYTEA"),
    };
    sql.replace("{int}", int).replace("{bytes}", bytes)
}

/// The version a database is at: the highest migration recorded, or 0 for one from before
/// migrations. Creates the record of migrations if it is missing.
pub async fn version<D: Database + ?Sized>(db: &D) -> Result<u32, StoreError> {
    let create = statement(
        db.dialect(),
        "CREATE TABLE IF NOT EXISTS migrations (version {int} PRIMARY KEY, applied_by TEXT NOT NULL)",
    );
    if let Err(error) = db.batch(&[Statement::new(&create, Vec::new())]).await {
        // Postgres can refuse a second instance creating the same table at the same moment.
        // The table is there either way; only an error reading it is a real one.
        return recorded(db).await.map_err(|_| error.into());
    }
    recorded(db).await
}

async fn recorded<D: Database + ?Sized>(db: &D) -> Result<u32, StoreError> {
    let rows = db.query("SELECT COALESCE(MAX(version), 0) FROM migrations", &[]).await?;
    let version = rows.first().map(|row| row.int(0)).transpose()?.unwrap_or(0);
    u32::try_from(version).map_err(|_| DbError(format!("schema version {version}")).into())
}

/// Bring the database up to [`VERSION`]. `by` is recorded with each migration this applies,
/// such as the store's version. Answers the version the database is at afterwards.
pub async fn migrate<D: Database + ?Sized>(db: &D, by: &str) -> Result<u32, StoreError> {
    migrate_with(db, MIGRATIONS, by).await
}

/// [`migrate`] with a list of migrations of one's own, for tests.
pub async fn migrate_with<D: Database + ?Sized>(
    db: &D,
    migrations: &[&[&str]],
    by: &str,
) -> Result<u32, StoreError> {
    let known = migrations.len() as u32;
    let mut at = version(db).await?;
    if at > known {
        return Err(newer(at, known));
    }
    let dialect = db.dialect();
    let record = dialect.render("INSERT INTO migrations (version, applied_by) VALUES (?1, ?2)");
    while at < known {
        let next = at + 1;
        // The record first: an instance that loses the race fails here, before it changes
        // anything, and the batch rolls back.
        let mut batch = vec![Statement::new(
            &record,
            vec![Value::Int(i64::from(next)), Value::Text(by.to_owned())],
        )];
        batch.extend(
            migrations[at as usize]
                .iter()
                .map(|sql| Statement::new(&statement(dialect, sql), Vec::new())),
        );
        match db.batch(&batch).await {
            Ok(_) => {
                tracing::info!(version = next, "store database migrated");
                at = next;
            }
            Err(error) => {
                // Another instance may have got there first. If so, carry on from where it left
                // the database; otherwise this migration failed, and nothing of it was kept.
                let now = recorded(db).await?;
                if now < next {
                    return Err(StoreError::Setup(format!(
                        "could not bring the store's database to schema version {next}: {error}"
                    )));
                }
                if now > known {
                    return Err(newer(now, known));
                }
                at = now;
            }
        }
    }
    Ok(at)
}

fn newer(at: u32, known: u32) -> StoreError {
    StoreError::Setup(format!(
        "the store's database is at schema version {at}, newer than this version of the store, \
         which knows up to {known}. A newer version of the store has used it: deploy that \
         version again, or restore a backup from before it"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_take_each_dialects_types() {
        let sql = "CREATE TABLE t (a {int}, b {bytes})";
        assert_eq!(statement(Dialect::Sqlite, sql), "CREATE TABLE t (a INTEGER, b BLOB)");
        assert_eq!(statement(Dialect::Postgres, sql), "CREATE TABLE t (a BIGINT, b BYTEA)");
    }

    #[test]
    fn every_statement_of_the_first_migration_is_safe_on_tables_that_exist() {
        // Stores from before migrations already have these tables, and run this one.
        for sql in MIGRATIONS[0] {
            assert!(sql.starts_with("CREATE TABLE IF NOT EXISTS"), "{sql}");
        }
        assert_eq!(VERSION, MIGRATIONS.len() as u32);
    }
}
