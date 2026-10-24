//! Schema versions and migrations, on SQLite. `tests/postgres.rs` runs the same steps on
//! Postgres, and `deploy/test/journeys.sh` opens a D1 database from before migrations.

use std::sync::Arc;

use anistream_store_core::{
    blobs::Blobs,
    db::Database,
    protocol::Role,
    schema::{self, MIGRATIONS, VERSION},
    sql,
    store::{NewInvite, Store, StoreError},
};

use super::{files::LocalBlobs, sqlite::SqliteDb};

const NOW: i64 = 1_800_000_000;
const BASE: &str = "https://plugins.example";

/// The tables exactly as stores made them before schema versions, with no record of migrations.
const BEFORE_VERSIONS: &[&str] = &[
    "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    "CREATE TABLE members (id TEXT PRIMARY KEY, name TEXT NOT NULL, role TEXT NOT NULL,
        joined_at INTEGER NOT NULL, removed_at INTEGER)",
    "CREATE TABLE devices (id TEXT PRIMARY KEY, member_id TEXT NOT NULL, label TEXT NOT NULL,
        token_hash TEXT NOT NULL UNIQUE, created_at INTEGER NOT NULL, last_seen INTEGER,
        revoked_at INTEGER)",
    "CREATE TABLE invites (id TEXT PRIMARY KEY, code_hash TEXT NOT NULL UNIQUE,
        kind TEXT NOT NULL, name TEXT NOT NULL, role TEXT NOT NULL, member_id TEXT,
        created_by TEXT, created_at INTEGER NOT NULL, expires_at INTEGER NOT NULL,
        used_at INTEGER, used_by TEXT, cancelled_at INTEGER)",
    "CREATE TABLE plugins (id TEXT PRIMARY KEY, name TEXT NOT NULL, version TEXT NOT NULL,
        allowed_hosts TEXT NOT NULL, translations TEXT NOT NULL, sha256 TEXT NOT NULL,
        size INTEGER NOT NULL, published_at INTEGER NOT NULL)",
    "INSERT INTO meta (key, value) VALUES ('name', 'From before versions')",
    "INSERT INTO members (id, name, role, joined_at) VALUES ('m1', 'Alex', 'owner', 1)",
];

type TestStore = Store<Arc<dyn Database>, Box<dyn Blobs>>;

async fn open_on(
    db: Arc<dyn Database>,
    dir: &std::path::Path,
) -> Result<TestStore, StoreError> {
    Store::open(db, Box::new(LocalBlobs::new(dir)) as Box<dyn Blobs>, Some(BASE), None).await
}

async fn run(db: &dyn Database, statements: &[&str]) {
    let batch: Vec<_> = statements
        .iter()
        .map(|s| anistream_store_core::db::Statement::new(s, vec![]))
        .collect();
    db.batch(&batch).await.unwrap();
}

async fn count(db: &dyn Database, sql: &str) -> i64 {
    db.query(sql, &[]).await.unwrap()[0].int(0).unwrap()
}

#[tokio::test]
async fn a_new_store_records_the_schema_version_it_was_made_at() {
    let dir = tempfile::tempdir().unwrap();
    let db: Arc<dyn Database> = Arc::new(SqliteDb::in_memory().unwrap());
    open_on(db.clone(), dir.path()).await.unwrap();
    assert_eq!(schema::version(db.as_ref()).await.unwrap(), VERSION);
    let by =
        db.query("SELECT applied_by FROM migrations WHERE version = 1", &[]).await.unwrap();
    assert_eq!(by[0].text(0).unwrap(), env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn a_store_from_before_versions_is_brought_up_to_date_and_keeps_everything() {
    let dir = tempfile::tempdir().unwrap();
    let db: Arc<dyn Database> = Arc::new(SqliteDb::in_memory().unwrap());
    run(db.as_ref(), BEFORE_VERSIONS).await;
    assert_eq!(schema::version(db.as_ref()).await.unwrap(), 0);

    let store = open_on(db.clone(), dir.path()).await.unwrap();
    assert_eq!(schema::version(db.as_ref()).await.unwrap(), VERSION);
    assert_eq!(store.name().await, "From before versions");
    assert!(store.has_owner().await.unwrap());
    // The tables added after it, such as the open joins, are there and work.
    let new = NewInvite::member("Sam", Role::Member, Some("m1"), NOW + 60);
    let (_, invite) = store.create_invite(new, BASE, NOW).await.unwrap();
    store.redeem(&invite.code, "laptop", NOW).await.unwrap();
    store.set_open(true).await.unwrap();
    assert_eq!(store.members().await.unwrap().len(), 2);
}

#[tokio::test]
async fn opening_again_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let db: Arc<dyn Database> = Arc::new(SqliteDb::open(&path).unwrap());
    let key = open_on(db.clone(), dir.path()).await.unwrap().public_key();
    let again = open_on(db.clone(), dir.path()).await.unwrap();
    assert_eq!(again.public_key(), key);
    assert_eq!(count(db.as_ref(), "SELECT COUNT(*) FROM migrations").await, i64::from(VERSION));
}

#[tokio::test]
async fn a_database_from_a_newer_store_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let db: Arc<dyn Database> = Arc::new(SqliteDb::in_memory().unwrap());
    open_on(db.clone(), dir.path()).await.unwrap();
    let newer = i64::from(VERSION) + 1;
    db.batch(&[sql!(
        "INSERT INTO migrations (version, applied_by) VALUES (?1, '99.0.0')",
        newer
    )])
    .await
    .unwrap();

    let error = open_on(db.clone(), dir.path()).await.unwrap_err().to_string();
    assert!(error.contains(&format!("schema version {newer}")), "{error}");
    assert!(error.contains("newer than this version"), "{error}");
    // Nothing was changed on the way to refusing.
    assert_eq!(schema::version(db.as_ref()).await.unwrap(), VERSION + 1);
}

/// Migrations of this test's own: the store's, then two more.
fn later() -> Vec<&'static [&'static str]> {
    let mut all = MIGRATIONS.to_vec();
    all.push(&[
        "ALTER TABLE plugins ADD COLUMN notes TEXT",
        "CREATE INDEX plugins_by_name ON plugins (name)",
    ]);
    all.push(&["CREATE TABLE IF NOT EXISTS tags (plugin TEXT NOT NULL, tag TEXT NOT NULL, added_at {int} NOT NULL)"]);
    all
}

#[tokio::test]
async fn later_migrations_run_in_order_once_each() {
    let dir = tempfile::tempdir().unwrap();
    let db: Arc<dyn Database> = Arc::new(SqliteDb::in_memory().unwrap());
    run(db.as_ref(), BEFORE_VERSIONS).await;
    let migrations = later();
    let known = migrations.len() as u32;

    assert_eq!(schema::migrate_with(db.as_ref(), &migrations, "test").await.unwrap(), known);
    assert_eq!(schema::migrate_with(db.as_ref(), &migrations, "test").await.unwrap(), known);
    assert_eq!(count(db.as_ref(), "SELECT COUNT(*) FROM migrations").await, i64::from(known));
    db.batch(&[sql!("UPDATE plugins SET notes = 'x'")]).await.unwrap();
    db.batch(&[sql!("INSERT INTO tags VALUES ('p', 't', ?1)", NOW)]).await.unwrap();

    // The store's own code, which knows fewer, now refuses the database.
    assert!(open_on(db.clone(), dir.path()).await.is_err());
}

#[tokio::test]
async fn a_failed_migration_keeps_nothing_of_itself() {
    let db: Arc<dyn Database> = Arc::new(SqliteDb::in_memory().unwrap());
    let mut migrations = MIGRATIONS.to_vec();
    migrations.push(&[
        "CREATE TABLE half_done (id TEXT)",
        "ALTER TABLE no_such_table ADD COLUMN x TEXT",
    ]);

    let error = schema::migrate_with(db.as_ref(), &migrations, "test").await.unwrap_err();
    assert!(error.to_string().contains("schema version 2"), "{error}");
    assert_eq!(schema::version(db.as_ref()).await.unwrap(), 1);
    let tables = count(
        db.as_ref(),
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'half_done'",
    )
    .await;
    assert_eq!(tables, 0, "the step's first statement was rolled back with the rest");
}

#[tokio::test]
async fn two_instances_opening_one_database_at_once_both_succeed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    // Two connections, as two instances of a store would have.
    let first: Arc<dyn Database> = Arc::new(SqliteDb::open(&path).unwrap());
    let second: Arc<dyn Database> = Arc::new(SqliteDb::open(&path).unwrap());
    run(first.as_ref(), BEFORE_VERSIONS).await;
    let migrations = later();
    let (a, b) = tokio::join!(
        schema::migrate_with(first.as_ref(), &migrations, "a"),
        schema::migrate_with(second.as_ref(), &migrations, "b"),
    );
    let known = migrations.len() as u32;
    assert_eq!((a.unwrap(), b.unwrap()), (known, known));
    assert_eq!(
        count(first.as_ref(), "SELECT COUNT(*) FROM migrations").await,
        i64::from(known)
    );
}
