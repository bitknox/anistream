//! The database a store keeps its records in, as little of one as the store needs.
//!
//! Two operations: a query, and a batch of statements that succeed or fail together. That is
//! what SQLite, Postgres and Cloudflare's D1 all offer without ceremony — D1 in particular has
//! no interactive transactions, only atomic batches — so every rule in [`crate::store`] is
//! written to need nothing more: decisions are single statements (`UPDATE … RETURNING` claims an
//! invite, a guarded upsert publishes), and what has to happen together goes in one batch.
//!
//! The store writes SQL once, with `?1`-style placeholders, in the subset the dialects share;
//! [`Dialect::render`] turns placeholders into `$1` for Postgres.

use crate::MaybeSendSync;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Int(i64),
    Text(String),
    /// A `BLOB` in SQLite, a `BYTEA` in Postgres.
    Bytes(Vec<u8>),
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Self::Int(value.min(i64::MAX as u64) as i64)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&String> for Value {
    fn from(value: &String) -> Self {
        Self::Text(value.clone())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}

/// One row of a result, by column position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row(pub Vec<Value>);

impl Row {
    fn at(&self, column: usize) -> Result<&Value, DbError> {
        self.0.get(column).ok_or_else(|| DbError(format!("no column {column}")))
    }

    /// An integer. Lenient about text, because some drivers hand a Postgres `BIGINT` to JavaScript
    /// as a string rather than lose precision.
    pub fn int(&self, column: usize) -> Result<i64, DbError> {
        self.opt_int(column)?.ok_or_else(|| DbError(format!("column {column} is null")))
    }

    pub fn opt_int(&self, column: usize) -> Result<Option<i64>, DbError> {
        match self.at(column)? {
            Value::Null => Ok(None),
            Value::Int(value) => Ok(Some(*value)),
            Value::Text(text) => text
                .parse()
                .map(Some)
                .map_err(|_| DbError(format!("column {column} is not a number: {text:?}"))),
            Value::Bytes(_) => Err(DbError(format!("column {column} is bytes, not a number"))),
        }
    }

    pub fn bytes(&self, column: usize) -> Result<Vec<u8>, DbError> {
        match self.at(column)? {
            Value::Bytes(bytes) => Ok(bytes.clone()),
            Value::Text(text) => Ok(text.as_bytes().to_vec()),
            _ => Err(DbError(format!("column {column} is not bytes"))),
        }
    }

    pub fn text(&self, column: usize) -> Result<String, DbError> {
        self.opt_text(column)?.ok_or_else(|| DbError(format!("column {column} is null")))
    }

    pub fn opt_text(&self, column: usize) -> Result<Option<String>, DbError> {
        match self.at(column)? {
            Value::Null => Ok(None),
            Value::Text(text) => Ok(Some(text.clone())),
            Value::Int(value) => Ok(Some(value.to_string())),
            Value::Bytes(_) => Err(DbError(format!("column {column} is bytes, not text"))),
        }
    }

    /// A truth value: SQLite answers `EXISTS` with 0 or 1, Postgres with a boolean the adapters
    /// pass on as 0 or 1.
    pub fn bool(&self, column: usize) -> Result<bool, DbError> {
        Ok(self.opt_int(column)?.unwrap_or(0) != 0)
    }
}

/// One statement of a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub sql: String,
    pub params: Vec<Value>,
}

impl Statement {
    pub fn new(sql: &str, params: Vec<Value>) -> Self {
        Self { sql: sql.to_owned(), params }
    }
}

/// A statement with its parameters, converting each one.
#[macro_export]
macro_rules! sql {
    ($sql:expr $(, $param:expr)* $(,)?) => {
        $crate::db::Statement::new($sql, vec![$($crate::db::Value::from($param)),*])
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// SQLite and D1.
    Sqlite,
    Postgres,
}

impl Dialect {
    /// The statement as this dialect writes it: `?1` stays for SQLite and becomes `$1` for
    /// Postgres. Placeholders inside quoted strings are left alone.
    pub fn render(self, sql: &str) -> String {
        if self == Self::Sqlite {
            return sql.to_owned();
        }
        let mut out = String::with_capacity(sql.len());
        let mut quoted = false;
        for ch in sql.chars() {
            match ch {
                '\'' => {
                    quoted = !quoted;
                    out.push(ch);
                }
                '?' if !quoted => out.push('$'),
                _ => out.push(ch),
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("database: {0}")]
pub struct DbError(pub String);

/// A store's database.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait Database: MaybeSendSync {
    fn dialect(&self) -> Dialect;

    /// Run one statement and return its rows. `sql` is already rendered for [`Self::dialect`].
    async fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>, DbError>;

    /// Run statements together: all of them, or none. Returns how many rows each one changed.
    async fn batch(&self, statements: &[Statement]) -> Result<Vec<u64>, DbError>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl<T: Database + ?Sized> Database for Box<T> {
    fn dialect(&self) -> Dialect {
        (**self).dialect()
    }

    async fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>, DbError> {
        (**self).query(sql, params).await
    }

    async fn batch(&self, statements: &[Statement]) -> Result<Vec<u64>, DbError> {
        (**self).batch(statements).await
    }
}

/// One database shared by the store and its files, as [`crate::blobs::SqlBlobs`] does.
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl<T: Database + ?Sized> Database for std::sync::Arc<T> {
    fn dialect(&self) -> Dialect {
        (**self).dialect()
    }

    async fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>, DbError> {
        (**self).query(sql, params).await
    }

    async fn batch(&self, statements: &[Statement]) -> Result<Vec<u64>, DbError> {
        (**self).batch(statements).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_gets_dollar_placeholders_and_quoted_text_is_left_alone() {
        let sql = "SELECT ?1, '?2 stays' FROM t WHERE a = ?2";
        assert_eq!(Dialect::Postgres.render(sql), "SELECT $1, '?2 stays' FROM t WHERE a = $2");
        assert_eq!(Dialect::Sqlite.render(sql), sql);
    }

    #[test]
    fn rows_read_numbers_from_text() {
        let row = Row(vec![Value::Text("42".into()), Value::Null, Value::Int(1)]);
        assert_eq!(row.int(0).unwrap(), 42);
        assert_eq!(row.opt_int(1).unwrap(), None);
        assert!(row.bool(2).unwrap());
        assert!(row.int(1).is_err());
        assert!(row.int(9).is_err());
    }

    #[test]
    fn the_statement_macro_converts_each_parameter() {
        let statement = sql!("x", 1_i64, "a", None::<String>, Some("b"));
        assert_eq!(
            statement.params,
            vec![Value::Int(1), Value::Text("a".into()), Value::Null, Value::Text("b".into())]
        );
    }
}
