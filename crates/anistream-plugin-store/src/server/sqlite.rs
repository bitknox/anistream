//! SQLite, for a store with a disk: the reference server's default, and every container
//! platform that gives a service a volume.

use std::{path::Path, sync::Mutex};

use anistream_store_core::db::{Database, DbError, Dialect, Row, Statement, Value};
use rusqlite::{Connection, types::ValueRef};

pub struct SqliteDb {
    conn: Mutex<Connection>,
}

impl SqliteDb {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path).map_err(error)?;
        Self::configure(conn)
    }

    /// A database that lives as long as the value, for tests.
    pub fn in_memory() -> Result<Self, DbError> {
        Self::configure(Connection::open_in_memory().map_err(error)?)
    }

    fn configure(conn: Connection) -> Result<Self, DbError> {
        // WAL, so `anistream store invite` and `publish` can write while the server runs.
        conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(error)?;
        conn.pragma_update(None, "journal_mode", "WAL").map_err(error)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned lock means a query panicked mid-way. SQLite rolled the statement back, so
        // the connection is still sound and refusing every later request would be worse.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn error(e: rusqlite::Error) -> DbError {
    DbError(e.to_string())
}

fn params(values: &[Value]) -> Vec<rusqlite::types::Value> {
    values
        .iter()
        .map(|value| match value {
            Value::Null => rusqlite::types::Value::Null,
            Value::Int(i) => rusqlite::types::Value::Integer(*i),
            Value::Text(t) => rusqlite::types::Value::Text(t.clone()),
            Value::Bytes(b) => rusqlite::types::Value::Blob(b.clone()),
        })
        .collect()
}

#[async_trait::async_trait]
impl Database for SqliteDb {
    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }

    async fn query(&self, sql: &str, values: &[Value]) -> Result<Vec<Row>, DbError> {
        // Inline rather than on a blocking thread: these are small queries on a local file, and
        // the lock is never held across an await.
        let conn = self.conn();
        let mut statement = conn.prepare(sql).map_err(error)?;
        let columns = statement.column_count();
        let mut rows =
            statement.query(rusqlite::params_from_iter(params(values))).map_err(error)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(error)? {
            let mut values = Vec::with_capacity(columns);
            for i in 0..columns {
                values.push(match row.get_ref(i).map_err(error)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(i) => Value::Int(i),
                    ValueRef::Real(f) => Value::Int(f as i64),
                    ValueRef::Text(t) => Value::Text(String::from_utf8_lossy(t).into_owned()),
                    ValueRef::Blob(b) => Value::Bytes(b.to_vec()),
                });
            }
            out.push(Row(values));
        }
        Ok(out)
    }

    async fn batch(&self, statements: &[Statement]) -> Result<Vec<u64>, DbError> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(error)?;
        let mut changed = Vec::with_capacity(statements.len());
        for statement in statements {
            let n = tx
                .execute(&statement.sql, rusqlite::params_from_iter(params(&statement.params)))
                .map_err(error)?;
            changed.push(n as u64);
        }
        tx.commit().map_err(error)?;
        Ok(changed)
    }
}
