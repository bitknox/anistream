//! A store's records and files, and every rule about them.
//!
//! Written against [`Database`] and [`Blobs`] so the same rules run on SQLite and a directory, on
//! Postgres and S3, and on D1 and R2. Nothing here reads the clock — every operation is told what
//! time it is — because there is no clock to read in some of the places this runs.
//!
//! The database offers single statements and atomic batches, nothing more, so every decision is
//! made by one statement: an invite is claimed by an `UPDATE … RETURNING` that only matches an
//! unused one, a version is published by an upsert that only applies to a new version, and an
//! open join adds its member only while the code it came with is the store's open code. Two
//! requests racing for the same invite cannot both win, whatever runs underneath.
//!
//! A member who joined with the open link has no name: that is what makes them anonymous, and
//! every name anyone is given otherwise is one to 64 characters. Nothing else about them is kept,
//! and their row goes with their last device.

use crate::{
    blobs::{BlobError, Blobs},
    crypto::{self, PublicKey, SigningKey},
    db::{Database, DbError, Dialect, Row, Statement, Value},
    inspect::{Inspector, differs},
    invite::Invite,
    manifest::{self, PluginInfo},
    protocol::{
        DeviceRow, INVITE_LIFETIME_SECS, Index, IndexEntry, InviteKind, MAX_PLUGIN_BYTES,
        Member, MemberRow, OPEN_DEVICES_LISTED, OpenState, PART_SIZE, PROTOCOL, PendingInvite,
        RESERVED_IDS, Role, SignedIndex, StoreInfo, is_valid_id, is_valid_version,
    },
    sql,
};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{0}")]
    Setup(String),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Blob(#[from] BlobError),
    #[error("{0}")]
    Crypto(#[from] crypto::CryptoError),
    /// The invite was used, cancelled, has expired or never existed. One error for all four, so
    /// a guess learns nothing about which.
    #[error("this invite was used, cancelled or has expired")]
    InviteUnusable,
    /// A sign-in link for the admin page that was used, has expired, or is not an owner's.
    #[error("this sign-in link was already used or has expired — open the admin page again")]
    LinkUnusable,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    InvalidPlugin(String),
    #[error("{0}")]
    BadRequest(String),
    /// The store does not let anyone join, or not with that code any more: closed, or opened
    /// again since with a new link. One error for both, like [`Self::InviteUnusable`].
    #[error(
        "this link no longer lets anyone join. Ask the store's owner for a new link or an invite"
    )]
    OpenUnusable,
    /// Too many devices joined with the open link from one address lately.
    #[error("too many devices joined from this address lately. Wait an hour and try again")]
    TooManyJoins,
}

/// How stale `last_seen` may get before a request refreshes it. Writing it on every request
/// would turn each index poll into a database write for a figure nobody needs to the second.
const SEEN_GRANULARITY_SECS: i64 = 10 * 60;

/// How long a sign-in link for the admin page works. Long enough to switch to a browser.
pub const ADMIN_LINK_SECS: i64 = 10 * 60;

/// How long an admin page session lasts. A working day, then sign in again from the app.
pub const SESSION_SECS: i64 = 12 * 60 * 60;

/// How long an unfinished upload is kept before its parts are thrown away.
const UPLOAD_SECS: i64 = 60 * 60;

/// Failed attempts allowed per address per window — redeeming, signing in, claiming.
///
/// Not what keeps any of them safe — every code is 128 random bits, and guessing one is not a
/// matter of attempts per minute — but it keeps a script hammering an endpoint from costing
/// anything.
pub const ALLOWED_FAILURES: i64 = 10;
pub const FAILURE_WINDOW_SECS: i64 = 10 * 60;

/// Devices allowed to join with the open link per address per window. A household joining
/// together fits; a script filling the members table does not. IPv6 addresses count by their
/// /64, which is what one connection is usually handed.
pub const OPEN_JOINS_ALLOWED: i64 = 10;
pub const OPEN_JOIN_WINDOW_SECS: i64 = 60 * 60;

/// A store name nobody chose.
pub const DEFAULT_NAME: &str = "Plugin store";

fn schema(dialect: Dialect) -> Vec<String> {
    // SQLite's INTEGER is 64-bit; Postgres's is not.
    let int = match dialect {
        Dialect::Sqlite => "INTEGER",
        Dialect::Postgres => "BIGINT",
    };
    [
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
    ]
    .iter()
    .map(|statement| statement.replace("{int}", int))
    .collect()
}

/// What an invite is for.
#[derive(Debug, Clone, Copy)]
pub struct NewInvite<'a> {
    pub kind: InviteKind,
    /// Who it is for, as the owner's member list will show them.
    pub name: &'a str,
    pub role: Role,
    /// The member a device invite adds a device to.
    pub member: Option<&'a str>,
    pub created_by: Option<&'a str>,
    pub expires_at: i64,
}

impl<'a> NewInvite<'a> {
    /// Someone new.
    pub fn member(
        name: &'a str,
        role: Role,
        created_by: Option<&'a str>,
        expires_at: i64,
    ) -> Self {
        Self { kind: InviteKind::Member, name, role, member: None, created_by, expires_at }
    }

    /// Another device for an existing member, asked for by that member.
    pub fn device(member: &'a Member, expires_at: i64) -> Self {
        Self {
            kind: InviteKind::Device,
            name: &member.name,
            role: member.role,
            member: Some(&member.id),
            created_by: Some(&member.id),
            expires_at,
        }
    }
}

/// A store, open on its database and file storage.
pub struct Store<D, B> {
    db: D,
    blobs: B,
    key: SigningKey,
}

impl<D, B> std::fmt::Debug for Store<D, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("key", &self.key).finish_non_exhaustive()
    }
}

impl<D: Database, B: Blobs> Store<D, B> {
    /// Open a store, creating its tables and its key on first use.
    ///
    /// `url` and `name` replace what is recorded when given. The key is kept in the database, so
    /// a backup of the database is a backup of the store's identity.
    pub async fn open(
        db: D,
        blobs: B,
        url: Option<&str>,
        name: Option<&str>,
    ) -> Result<Self, StoreError> {
        let dialect = db.dialect();
        let statements: Vec<Statement> =
            schema(dialect).iter().map(|s| Statement::new(s, Vec::new())).collect();
        db.batch(&statements).await?;

        // Generated once. Two instances starting together both try; the first insert wins and
        // both read back the same key.
        let fresh = SigningKey::generate()?;
        db.batch(&[render(
            dialect,
            sql!(
                "INSERT INTO meta (key, value) VALUES ('signing_key', ?1)
                 ON CONFLICT(key) DO NOTHING",
                fresh.encode()
            ),
        )])
        .await?;
        let stored = db
            .query("SELECT value FROM meta WHERE key = 'signing_key'", &[])
            .await?
            .first()
            .map(|row| row.text(0))
            .transpose()?
            .ok_or_else(|| StoreError::Setup("the store's key was not saved".into()))?;
        let key = SigningKey::parse(&stored)?;

        let store = Self { db, blobs, key };
        if let Some(url) = url {
            store.set_meta("url", url).await?;
        }
        match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => store.set_meta("name", name).await?,
            None => {
                store
                    .exec(sql!(
                        "INSERT INTO meta (key, value) VALUES ('name', ?1)
                         ON CONFLICT(key) DO NOTHING",
                        DEFAULT_NAME
                    ))
                    .await?;
            }
        }
        Ok(store)
    }

    pub fn key(&self) -> &SigningKey {
        &self.key
    }

    pub fn blobs(&self) -> &B {
        &self.blobs
    }

    // --- plumbing ---------------------------------------------------------------------------

    async fn query(&self, statement: Statement) -> Result<Vec<Row>, StoreError> {
        let statement = render(self.db.dialect(), statement);
        Ok(self.db.query(&statement.sql, &statement.params).await?)
    }

    async fn exec(&self, statement: Statement) -> Result<u64, StoreError> {
        Ok(self.batch(vec![statement]).await?.first().copied().unwrap_or(0))
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<u64>, StoreError> {
        let dialect = self.db.dialect();
        let statements: Vec<Statement> =
            statements.into_iter().map(|s| render(dialect, s)).collect();
        Ok(self.db.batch(&statements).await?)
    }

    async fn meta(&self, key: &str) -> Result<Option<String>, StoreError> {
        let rows = self.query(sql!("SELECT value FROM meta WHERE key = ?1", key)).await?;
        Ok(rows.first().map(|row| row.text(0)).transpose()?)
    }

    async fn set_meta(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.exec(sql!(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            key,
            value
        ))
        .await?;
        Ok(())
    }

    // --- identity ---------------------------------------------------------------------------

    pub async fn name(&self) -> String {
        self.meta("name").await.ok().flatten().unwrap_or_else(|| DEFAULT_NAME.into())
    }

    /// The address recorded for the store, if one was given.
    pub async fn url(&self) -> Option<String> {
        self.meta("url").await.ok().flatten().filter(|url| !url.is_empty())
    }

    pub async fn info(&self) -> StoreInfo {
        StoreInfo {
            protocol: PROTOCOL,
            name: self.name().await,
            key: self.key.public().encode(),
            claimable: false,
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.key.public()
    }

    // --- invites ----------------------------------------------------------------------------

    /// Create an invite and return it with its link. The code exists only in the return value.
    pub async fn create_invite(
        &self,
        new: NewInvite<'_>,
        base: &str,
        now: i64,
    ) -> Result<(String, Invite), StoreError> {
        let NewInvite { kind, name, role, member, created_by, expires_at } = new;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(StoreError::BadRequest(
                "an invite needs a name of 1 to 64 characters".into(),
            ));
        }
        let id = crypto::new_id()?;
        let code = crypto::new_code()?;
        self.exec(sql!(
            "INSERT INTO invites (id, code_hash, kind, name, role, member_id, created_by,
                                  created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            id.as_str(),
            crypto::hash_secret(&code),
            kind.as_str(),
            name,
            role.as_str(),
            member,
            created_by,
            now,
            expires_at
        ))
        .await?;
        Ok((id, Invite { base: base.to_owned(), code, key: self.key.public(), open: false }))
    }

    /// The default expiry for an invite created `now`.
    pub fn default_expiry(now: i64) -> i64 {
        now + INVITE_LIFETIME_SECS
    }

    pub async fn has_owner(&self) -> Result<bool, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT EXISTS(SELECT 1 FROM members WHERE role = 'owner' AND removed_at IS NULL)"
            ))
            .await?;
        Ok(rows.first().map(|r| r.bool(0)).transpose()?.unwrap_or(false))
    }

    /// An owner invite, while nobody has claimed the store yet.
    ///
    /// Earlier unclaimed owner invites are cancelled first, so only the link produced most
    /// recently works — an old terminal's scrollback is not a way in.
    pub async fn fresh_owner_invite(
        &self,
        base: &str,
        now: i64,
    ) -> Result<Option<Invite>, StoreError> {
        if self.has_owner().await? {
            return Ok(None);
        }
        self.exec(sql!(
            "UPDATE invites SET cancelled_at = ?1
             WHERE role = 'owner' AND created_by IS NULL AND used_at IS NULL
               AND cancelled_at IS NULL",
            now
        ))
        .await?;
        let new = NewInvite::member("Owner", Role::Owner, None, Self::default_expiry(now));
        let (_, invite) = self.create_invite(new, base, now).await?;
        Ok(Some(invite))
    }

    /// Spend an invite: create the member it was for (or find them, for a device invite) and give
    /// this device a token.
    ///
    /// The invite is claimed by one statement that only matches an unused, unexpired invite whose
    /// maker is still a member, so two devices racing for one invite cannot both get it.
    pub async fn redeem(
        &self,
        code: &str,
        device_label: &str,
        now: i64,
    ) -> Result<(String, Member), StoreError> {
        if !crypto::is_code(code) {
            return Err(StoreError::InviteUnusable);
        }
        let new_member = crypto::new_id()?;
        let claimed = self
            .query(sql!(
                "UPDATE invites SET used_at = ?1, used_by = COALESCE(member_id, ?2)
                 WHERE code_hash = ?3 AND used_at IS NULL AND cancelled_at IS NULL
                   AND expires_at > ?1
                   AND (created_by IS NULL OR EXISTS (
                       SELECT 1 FROM members m
                       WHERE m.id = invites.created_by AND m.removed_at IS NULL))
                 RETURNING kind, name, role, member_id",
                now,
                new_member.as_str(),
                crypto::hash_secret(code)
            ))
            .await?;
        let row = claimed.first().ok_or(StoreError::InviteUnusable)?;
        let kind = InviteKind::parse(&row.text(0)?);
        let role = Role::parse(&row.text(2)?).unwrap_or_default();

        let token = crypto::new_token()?;
        let label: String = device_label.trim().chars().take(64).collect();
        let label = if label.is_empty() { "device".to_owned() } else { label };
        let device = sql!(
            "INSERT INTO devices (id, member_id, label, token_hash, created_at, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            crypto::new_id()?,
            String::new(),
            label.as_str(),
            crypto::hash_secret(&token),
            now
        );
        let with_member = |mut statement: Statement, member: &str| {
            statement.params[1] = Value::from(member);
            statement
        };

        let member = match (kind, row.opt_text(3)?) {
            (Some(InviteKind::Device), Some(member_id)) => {
                let rows = self
                    .query(sql!(
                        "SELECT name, role FROM members WHERE id = ?1 AND removed_at IS NULL",
                        member_id.as_str()
                    ))
                    .await?;
                // The member was removed after asking for the invite: it dies with them.
                let found = rows.first().ok_or(StoreError::InviteUnusable)?;
                let member = Member {
                    name: found.text(0)?,
                    role: Role::parse(&found.text(1)?).unwrap_or_default(),
                    id: member_id,
                };
                self.exec(with_member(device, &member.id)).await?;
                member
            }
            (Some(InviteKind::Member), _) => {
                let member = Member { id: new_member, name: row.text(1)?, role };
                self.batch(vec![
                    sql!(
                        "INSERT INTO members (id, name, role, joined_at) VALUES (?1, ?2, ?3, ?4)",
                        member.id.as_str(),
                        member.name.as_str(),
                        role.as_str(),
                        now
                    ),
                    with_member(device, &member.id),
                ])
                .await?;
                member
            }
            _ => return Err(StoreError::InviteUnusable),
        };
        Ok((token, member))
    }

    /// Whether `code` is an invite that would join, checked as [`Self::redeem`] checks it but
    /// spending nothing: for a preview of what it leads to.
    pub async fn check_invite(&self, code: &str, now: i64) -> Result<(), StoreError> {
        if !crypto::is_code(code) {
            return Err(StoreError::InviteUnusable);
        }
        let found = self
            .query(sql!(
                "SELECT 1 FROM invites
                 WHERE code_hash = ?1 AND used_at IS NULL AND cancelled_at IS NULL
                   AND expires_at > ?2
                   AND (created_by IS NULL OR EXISTS (
                       SELECT 1 FROM members m
                       WHERE m.id = invites.created_by AND m.removed_at IS NULL))",
                crypto::hash_secret(code),
                now
            ))
            .await?;
        if found.is_empty() { Err(StoreError::InviteUnusable) } else { Ok(()) }
    }

    /// The owners' names, the longest-standing first: who the store says runs it.
    pub async fn owner_names(&self) -> Result<Vec<String>, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT name FROM members WHERE role = 'owner' AND removed_at IS NULL
                 ORDER BY joined_at, id"
            ))
            .await?;
        Ok(rows.iter().map(|row| row.text(0)).collect::<Result<_, _>>()?)
    }

    pub async fn pending_invites(&self, now: i64) -> Result<Vec<PendingInvite>, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT id, name, role, kind, expires_at FROM invites
                 WHERE used_at IS NULL AND cancelled_at IS NULL AND expires_at > ?1
                 ORDER BY created_at",
                now
            ))
            .await?;
        rows.iter()
            .map(|row| {
                Ok(PendingInvite {
                    id: row.text(0)?,
                    name: row.text(1)?,
                    role: Role::parse(&row.text(2)?).unwrap_or_default(),
                    kind: InviteKind::parse(&row.text(3)?).unwrap_or(InviteKind::Member),
                    expires_at: row.int(4)?,
                })
            })
            .collect()
    }

    pub async fn cancel_invite(&self, id: &str, now: i64) -> Result<(), StoreError> {
        let changed = self
            .exec(sql!(
                "UPDATE invites SET cancelled_at = ?1
                 WHERE id = ?2 AND used_at IS NULL AND cancelled_at IS NULL",
                now,
                id
            ))
            .await?;
        if changed == 0 {
            return Err(StoreError::NotFound("no pending invite with that id".into()));
        }
        Ok(())
    }

    // --- open joining -----------------------------------------------------------------------

    /// The open link's code while anyone may join, `None` while only invited people may.
    ///
    /// Kept as it is rather than as a digest, unlike every other code: the owner copies the link
    /// again whenever they like, and the code grants nothing but what the link openly offers.
    pub async fn open_code(&self) -> Result<Option<String>, StoreError> {
        Ok(self.meta("open_code").await?.filter(|code| crypto::is_code(code)))
    }

    /// The open link, built on `base`, while the store has one.
    pub async fn open_link(&self, base: &str) -> Result<Option<Invite>, StoreError> {
        Ok(self.open_code().await?.map(|code| Invite {
            base: base.to_owned(),
            code,
            key: self.key.public(),
            open: true,
        }))
    }

    /// Whether `code` is the open link's: how a request from a client that predates open links,
    /// which takes the link for an invite, is told apart from a spent invite.
    pub async fn is_open_code(&self, code: &str) -> Result<bool, StoreError> {
        Ok(self.open_code().await?.is_some_and(|open| codes_match(&open, code)))
    }

    /// Let anyone with the link join, or only people the owners invite.
    ///
    /// Opening makes a new link, so a link from an earlier time the store was open stays dead:
    /// switching to invites only and back is also how an owner replaces a link that went further
    /// than they meant. Opening a store that is already open keeps its link. Closing stops new
    /// joins and leaves the devices that joined, which the owner removes when they choose.
    pub async fn set_open(&self, open: bool) -> Result<(), StoreError> {
        if open {
            self.exec(sql!(
                "INSERT INTO meta (key, value) VALUES ('open_code', ?1)
                 ON CONFLICT(key) DO NOTHING",
                crypto::new_code()?
            ))
            .await?;
        } else {
            self.exec(sql!("DELETE FROM meta WHERE key = 'open_code'")).await?;
        }
        Ok(())
    }

    /// Whether `code` would join now, checked as [`Self::join_open`] checks it but adding no
    /// one: for a preview of what the link leads to.
    pub async fn check_open(&self, code: &str) -> Result<(), StoreError> {
        if crypto::is_code(code) && self.is_open_code(code).await? {
            Ok(())
        } else {
            Err(StoreError::OpenUnusable)
        }
    }

    /// Whether `client` has had as many devices join from it lately as it may.
    pub async fn joins_blocked(&self, client: &str, now: i64) -> Result<bool, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT window_start, attempts FROM open_joins WHERE key = ?1",
                join_key(client)
            ))
            .await?;
        let Some(row) = rows.first() else { return Ok(false) };
        Ok(now - row.int(0)? < OPEN_JOIN_WINDOW_SECS && row.int(1)? >= OPEN_JOINS_ALLOWED)
    }

    /// Join with the open link: a new member with no name, one device, and its token.
    ///
    /// The member is added by one statement that only adds them while `code` is still the store's
    /// open code, so a join racing the owner closing the store, or opening it again with a new
    /// link, does not get in on the old one. The device and the count for `client` go in the same
    /// batch, so a join is all of them or none.
    pub async fn join_open(
        &self,
        code: &str,
        client: &str,
        now: i64,
    ) -> Result<(String, Member), StoreError> {
        self.check_open(code).await?;
        if self.joins_blocked(client, now).await? {
            return Err(StoreError::TooManyJoins);
        }
        let member = Member { id: crypto::new_id()?, name: String::new(), role: Role::Member };
        let token = crypto::new_token()?;
        // Each value is cast to its column's type: Postgres cannot infer a parameter's type from
        // the column an `INSERT … SELECT` fills.
        let changed = self
            .batch(vec![
                sql!(
                    "INSERT INTO members (id, name, role, joined_at)
                     SELECT CAST(?1 AS TEXT), '', 'member', CAST(?2 AS BIGINT)
                     WHERE EXISTS (SELECT 1 FROM meta
                                   WHERE key = 'open_code' AND value = CAST(?3 AS TEXT))",
                    member.id.as_str(),
                    now,
                    code
                ),
                sql!(
                    "INSERT INTO devices (id, member_id, label, token_hash, created_at, last_seen)
                     SELECT CAST(?1 AS TEXT), CAST(?2 AS TEXT), '', CAST(?3 AS TEXT),
                            CAST(?4 AS BIGINT), CAST(?4 AS BIGINT)
                     WHERE EXISTS (SELECT 1 FROM members WHERE id = CAST(?2 AS TEXT))",
                    crypto::new_id()?,
                    member.id.as_str(),
                    crypto::hash_secret(&token),
                    now
                ),
                sql!(
                    "INSERT INTO open_joins (key, window_start, attempts) VALUES (?1, ?2, 1)
                     ON CONFLICT(key) DO UPDATE SET
                        window_start = CASE WHEN ?2 - open_joins.window_start >= ?3
                                            THEN ?2 ELSE open_joins.window_start END,
                        attempts = CASE WHEN ?2 - open_joins.window_start >= ?3
                                        THEN 1 ELSE open_joins.attempts + 1 END",
                    join_key(client),
                    now,
                    OPEN_JOIN_WINDOW_SECS
                ),
                sql!(
                    "DELETE FROM open_joins WHERE window_start < ?1",
                    now - OPEN_JOIN_WINDOW_SECS
                ),
            ])
            .await?;
        if changed.first().copied().unwrap_or(0) == 0 {
            // Closed, or a new link made, between the check and the join.
            return Err(StoreError::OpenUnusable);
        }
        Ok((token, member))
    }

    /// Who can join, the link while anyone can, and the devices that joined with a link.
    pub async fn open_state(&self, base: &str) -> Result<OpenState, StoreError> {
        let count = self
            .query(sql!(
                "SELECT COUNT(*) FROM devices d JOIN members m ON m.id = d.member_id
                 WHERE d.revoked_at IS NULL AND m.removed_at IS NULL AND m.name = ''"
            ))
            .await?;
        let devices = count.first().map(|row| row.int(0)).transpose()?.unwrap_or(0);
        let rows = self
            .query(sql!(
                "SELECT d.id, d.created_at, d.last_seen
                 FROM devices d JOIN members m ON m.id = d.member_id
                 WHERE d.revoked_at IS NULL AND m.removed_at IS NULL AND m.name = ''
                 ORDER BY d.created_at DESC, d.id LIMIT ?1",
                i64::from(OPEN_DEVICES_LISTED)
            ))
            .await?;
        let device_list = rows
            .iter()
            .map(|row| {
                Ok(DeviceRow {
                    id: row.text(0)?,
                    label: String::new(),
                    created_at: row.int(1)?,
                    last_seen: row.opt_int(2)?,
                })
            })
            .collect::<Result<_, StoreError>>()?;
        let link = self.open_link(base).await?;
        Ok(OpenState {
            open: link.is_some(),
            link: link.map(|invite| invite.link()),
            devices: u32::try_from(devices.max(0)).unwrap_or(u32::MAX),
            device_list,
        })
    }

    /// Remove every member who joined with a link, and so every device they joined on: the
    /// devices are revoked and the members' rows deleted, together. Answers how many devices
    /// stopped working. Whether anyone can join is left as it is.
    pub async fn remove_anonymous(&self, now: i64) -> Result<u64, StoreError> {
        let changed = self
            .batch(vec![
                sql!(
                    "UPDATE devices SET revoked_at = ?1
                     WHERE revoked_at IS NULL AND member_id IN (
                         SELECT id FROM members WHERE name = '')",
                    now
                ),
                sql!("DELETE FROM members WHERE name = ''"),
            ])
            .await?;
        Ok(changed.first().copied().unwrap_or(0))
    }

    // --- members and devices ----------------------------------------------------------------

    /// Who a token belongs to, if it is still good.
    pub async fn authenticate(
        &self,
        token: &str,
        now: i64,
    ) -> Result<Option<(Member, crate::protocol::Device)>, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT m.id, m.name, m.role, d.id, d.label, d.last_seen
                 FROM devices d JOIN members m ON m.id = d.member_id
                 WHERE d.token_hash = ?1 AND d.revoked_at IS NULL AND m.removed_at IS NULL",
                crypto::hash_secret(token)
            ))
            .await?;
        let Some(row) = rows.first() else { return Ok(None) };
        let member = Member {
            id: row.text(0)?,
            name: row.text(1)?,
            role: Role::parse(&row.text(2)?).unwrap_or_default(),
        };
        let device = crate::protocol::Device { id: row.text(3)?, label: row.text(4)? };
        if row.opt_int(5)?.is_none_or(|seen| now - seen >= SEEN_GRANULARITY_SECS) {
            self.exec(sql!(
                "UPDATE devices SET last_seen = ?1 WHERE id = ?2",
                now,
                device.id.as_str()
            ))
            .await?;
        }
        Ok(Some((member, device)))
    }

    /// Revoke a device: the one a request came from, as it leaves. An anonymous member goes
    /// with their last device, in the same batch.
    pub async fn revoke_device(&self, device_id: &str, now: i64) -> Result<(), StoreError> {
        self.batch(revoke(device_id, now)).await?;
        Ok(())
    }

    /// Revoke one device of anyone's, for an owner. A member with a name stays; an anonymous one
    /// goes with their last device, in the same batch.
    pub async fn revoke_any_device(&self, device_id: &str, now: i64) -> Result<(), StoreError> {
        let changed = self.batch(revoke(device_id, now)).await?;
        if changed.first().copied().unwrap_or(0) == 0 {
            return Err(StoreError::NotFound("no device with that id".into()));
        }
        Ok(())
    }

    pub async fn members(&self) -> Result<Vec<MemberRow>, StoreError> {
        let devices = self
            .query(sql!(
                "SELECT d.member_id, d.id, d.label, d.created_at, d.last_seen
                 FROM devices d JOIN members m ON m.id = d.member_id
                 WHERE d.revoked_at IS NULL AND m.removed_at IS NULL AND m.name <> ''
                 ORDER BY d.created_at"
            ))
            .await?;
        let mut by_member: std::collections::HashMap<String, Vec<DeviceRow>> =
            Default::default();
        for row in &devices {
            by_member.entry(row.text(0)?).or_default().push(DeviceRow {
                id: row.text(1)?,
                label: row.text(2)?,
                created_at: row.int(3)?,
                last_seen: row.opt_int(4)?,
            });
        }
        // Anonymous members are counted, never listed: see `open_state`.
        let rows = self
            .query(sql!(
                "SELECT id, name, role, joined_at FROM members
                 WHERE removed_at IS NULL AND name <> '' ORDER BY joined_at, id"
            ))
            .await?;
        rows.iter()
            .map(|row| {
                let id = row.text(0)?;
                let device_list = by_member.remove(&id).unwrap_or_default();
                Ok(MemberRow {
                    last_seen: device_list.iter().filter_map(|d| d.last_seen).max(),
                    devices: device_list.len() as u32,
                    device_list,
                    id,
                    name: row.text(1)?,
                    role: Role::parse(&row.text(2)?).unwrap_or_default(),
                    joined_at: row.int(3)?,
                })
            })
            .collect()
    }

    /// Change a member's role. Not your own: an owner making themselves a member could leave a
    /// store with nobody to run it. Not an anonymous member's either, who has no name to run it by.
    pub async fn set_role(&self, id: &str, role: Role, by: &str) -> Result<(), StoreError> {
        if id == by {
            return Err(StoreError::Forbidden(
                "an owner cannot change their own role — another owner can".into(),
            ));
        }
        let changed = self
            .exec(sql!(
                "UPDATE members SET role = ?1 WHERE id = ?2 AND removed_at IS NULL AND name <> ''",
                role.as_str(),
                id
            ))
            .await?;
        if changed == 0 {
            return Err(StoreError::NotFound("no member with that id".into()));
        }
        Ok(())
    }

    /// Change the name a member is shown by. An anonymous member has none to change.
    pub async fn rename(&self, id: &str, name: &str) -> Result<(), StoreError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(StoreError::BadRequest("a name is 1 to 64 characters".into()));
        }
        let changed = self
            .exec(sql!(
                "UPDATE members SET name = ?1 WHERE id = ?2 AND removed_at IS NULL AND name <> ''",
                name,
                id
            ))
            .await?;
        if changed == 0 {
            return Err(StoreError::NotFound("no member with that id".into()));
        }
        Ok(())
    }

    /// The owner a command-line sign-in link is for: the one named, or the first to join.
    pub async fn owner_named(&self, name: Option<&str>) -> Result<Option<Member>, StoreError> {
        Ok(self
            .members()
            .await?
            .into_iter()
            .filter(|m| m.role == Role::Owner)
            .find(|m| name.is_none_or(|name| m.name.eq_ignore_ascii_case(name.trim())))
            .map(|m| Member { id: m.id, name: m.name, role: m.role }))
    }

    /// Remove a member, revoke every device they have, and cancel every invite that would bring
    /// them, or anyone they chose, back in.
    pub async fn remove_member(&self, id: &str, by: &str, now: i64) -> Result<(), StoreError> {
        if id == by {
            return Err(StoreError::Forbidden(
                "an owner cannot remove themselves — a store always keeps one".into(),
            ));
        }
        let changed = self
            .batch(vec![
                sql!(
                    "UPDATE members SET removed_at = ?1 WHERE id = ?2 AND removed_at IS NULL",
                    now,
                    id
                ),
                sql!(
                    "UPDATE devices SET revoked_at = ?1
                     WHERE member_id = ?2 AND revoked_at IS NULL",
                    now,
                    id
                ),
                sql!(
                    "UPDATE invites SET cancelled_at = ?1
                     WHERE (member_id = ?2 OR created_by = ?2)
                       AND used_at IS NULL AND cancelled_at IS NULL",
                    now,
                    id
                ),
                sql!("DELETE FROM sessions WHERE member_id = ?1", id),
                // An anonymous member is not kept as removed: nothing of them is kept at all.
                sql!("DELETE FROM members WHERE id = ?1 AND name = ''", id),
            ])
            .await?;
        if changed.first().copied().unwrap_or(0) == 0 {
            return Err(StoreError::NotFound("no member with that id".into()));
        }
        Ok(())
    }

    // --- the admin page ---------------------------------------------------------------------

    /// A one-time link that signs a browser in as `member`, who must be an owner.
    ///
    /// The code travels in the link's fragment, so it never reaches the server's logs or a
    /// proxy's, and the page trades it for a session cookie the moment it loads.
    pub async fn create_admin_link(
        &self,
        member: &str,
        base: &str,
        now: i64,
    ) -> Result<(String, i64), StoreError> {
        let rows = self
            .query(sql!(
                "SELECT EXISTS(SELECT 1 FROM members
                               WHERE id = ?1 AND role = 'owner' AND removed_at IS NULL)",
                member
            ))
            .await?;
        if !rows.first().map(|r| r.bool(0)).transpose()?.unwrap_or(false) {
            return Err(StoreError::Forbidden("only the store's owner can do that".into()));
        }
        let code = crypto::new_code()?;
        let expires_at = now + ADMIN_LINK_SECS;
        self.exec(sql!(
            "INSERT INTO admin_links (code_hash, member_id, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4)",
            crypto::hash_secret(&code),
            member,
            now,
            expires_at
        ))
        .await?;
        Ok((format!("{base}/admin/#login={code}"), expires_at))
    }

    /// Spend a sign-in link for a session token.
    pub async fn open_session(
        &self,
        code: &str,
        now: i64,
    ) -> Result<(String, Member), StoreError> {
        if !crypto::is_code(code) {
            return Err(StoreError::LinkUnusable);
        }
        let claimed = self
            .query(sql!(
                "UPDATE admin_links SET used_at = ?1
                 WHERE code_hash = ?2 AND used_at IS NULL AND expires_at > ?1
                 RETURNING member_id",
                now,
                crypto::hash_secret(code)
            ))
            .await?;
        let member_id = claimed.first().ok_or(StoreError::LinkUnusable)?.text(0)?;
        let rows = self
            .query(sql!(
                "SELECT name FROM members
                 WHERE id = ?1 AND role = 'owner' AND removed_at IS NULL",
                member_id.as_str()
            ))
            .await?;
        let name = rows.first().ok_or(StoreError::LinkUnusable)?.text(0)?;
        let token = crypto::new_token()?;
        // Old sessions and links are swept here rather than on a timer: signing in is rare, and
        // this keeps both tables to what could still be used.
        self.batch(vec![
            sql!(
                "INSERT INTO sessions (token_hash, member_id, created_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4)",
                crypto::hash_secret(&token),
                member_id.as_str(),
                now,
                now + SESSION_SECS
            ),
            sql!("DELETE FROM sessions WHERE expires_at <= ?1", now),
            sql!("DELETE FROM admin_links WHERE expires_at <= ?1", now),
        ])
        .await?;
        Ok((token, Member { id: member_id, name, role: Role::Owner }))
    }

    /// The owner a session belongs to, if it is still good — unexpired, and its owner still an
    /// owner. Demoting or removing someone ends their sessions with no further step.
    pub async fn session(&self, token: &str, now: i64) -> Result<Option<Member>, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT m.id, m.name FROM sessions s JOIN members m ON m.id = s.member_id
                 WHERE s.token_hash = ?1 AND s.expires_at > ?2
                   AND m.role = 'owner' AND m.removed_at IS NULL",
                crypto::hash_secret(token),
                now
            ))
            .await?;
        rows.first()
            .map(|row| Ok(Member { id: row.text(0)?, name: row.text(1)?, role: Role::Owner }))
            .transpose()
    }

    pub async fn close_session(&self, token: &str) -> Result<(), StoreError> {
        self.exec(sql!(
            "DELETE FROM sessions WHERE token_hash = ?1",
            crypto::hash_secret(token)
        ))
        .await?;
        Ok(())
    }

    // --- failed attempts --------------------------------------------------------------------

    /// Whether `key` — an address, usually — has failed too often lately.
    pub async fn blocked(&self, key: &str, now: i64) -> Result<bool, StoreError> {
        let rows = self
            .query(sql!("SELECT window_start, attempts FROM failures WHERE key = ?1", key))
            .await?;
        let Some(row) = rows.first() else { return Ok(false) };
        Ok(now - row.int(0)? < FAILURE_WINDOW_SECS && row.int(1)? >= ALLOWED_FAILURES)
    }

    /// Count a failed attempt. Kept in the database rather than in memory, because where a store
    /// runs as a function there is no memory that outlives one request.
    pub async fn fail(&self, key: &str, now: i64) -> Result<(), StoreError> {
        self.batch(vec![
            sql!(
                "INSERT INTO failures (key, window_start, attempts) VALUES (?1, ?2, 1)
                 ON CONFLICT(key) DO UPDATE SET
                    window_start = CASE WHEN ?2 - failures.window_start >= ?3
                                        THEN ?2 ELSE failures.window_start END,
                    attempts = CASE WHEN ?2 - failures.window_start >= ?3
                                    THEN 1 ELSE failures.attempts + 1 END",
                key,
                now,
                FAILURE_WINDOW_SECS
            ),
            // Expired windows go as new failures arrive, so the table holds only what counts.
            sql!("DELETE FROM failures WHERE window_start < ?1", now - FAILURE_WINDOW_SECS),
        ])
        .await?;
        Ok(())
    }

    // --- plugins and the index --------------------------------------------------------------

    pub async fn plugins(&self) -> Result<Vec<IndexEntry>, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT id, name, version, allowed_hosts, translations, sha256, size, published_at
                 FROM plugins ORDER BY id"
            ))
            .await?;
        rows.iter().map(|row| entry_from(row, 0)).collect()
    }

    pub async fn plugin(&self, id: &str) -> Result<Option<IndexEntry>, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT id, name, version, allowed_hosts, translations, sha256, size, published_at
                 FROM plugins WHERE id = ?1",
                id
            ))
            .await?;
        rows.first().map(|row| entry_from(row, 0)).transpose()
    }

    /// The current index sequence.
    pub async fn sequence(&self) -> u64 {
        self.meta("sequence").await.ok().flatten().and_then(|s| s.parse().ok()).unwrap_or(0)
    }

    /// The current index, signed.
    ///
    /// The sequence and the plugins are read by one statement, so they are the same moment's: read
    /// apart, a publish landing between them would sign one number over two different lists.
    /// Deterministic for an unchanged store: `issued_at` is when the plugin set last changed.
    pub async fn signed_index(&self) -> Result<SignedIndex, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT (SELECT value FROM meta WHERE key = 'sequence'),
                        (SELECT value FROM meta WHERE key = 'index_updated_at'),
                        p.id, p.name, p.version, p.allowed_hosts, p.translations, p.sha256,
                        p.size, p.published_at
                 FROM (SELECT 1 AS anchor) AS a LEFT JOIN plugins p ON 1 = 1
                 ORDER BY p.id"
            ))
            .await?;
        let first = rows.first();
        let sequence = first.map(|r| r.opt_int(0)).transpose()?.flatten().unwrap_or(0).max(0);
        let issued_at = first.map(|r| r.opt_int(1)).transpose()?.flatten().unwrap_or(0);
        let mut plugins = Vec::new();
        for row in &rows {
            if row.opt_text(2)?.is_some() {
                plugins.push(entry_from(row, 2)?);
            }
        }
        let index = Index {
            protocol: PROTOCOL,
            store: self.name().await,
            key: self.key.public().encode(),
            sequence: sequence as u64,
            issued_at,
            plugins,
            owners: self.owner_names().await?,
            open: self.open_code().await?.is_some(),
        };
        let index = serde_json::to_string(&index)
            .map_err(|e| StoreError::Setup(format!("encoding the index: {e}")))?;
        Ok(SignedIndex { signature: self.key.sign_index(&index), index })
    }

    /// What a plugin is, decided the best way available.
    ///
    /// With a WebAssembly runtime, by asking the plugin — and a manifest section it carries must
    /// agree. Without one, from the manifest section, which then has to be there.
    async fn identify(
        &self,
        wasm: &[u8],
        inspector: Option<&dyn Inspector>,
    ) -> Result<PluginInfo, StoreError> {
        match inspector {
            Some(inspector) => {
                let asked = inspector.inspect(wasm).await.map_err(StoreError::InvalidPlugin)?;
                if let Ok(Some(carried)) = manifest::read(wasm)
                    && let Some(problem) =
                        differs(&carried, &asked.id, &asked.version, &asked.allowed_hosts)
                {
                    return Err(StoreError::InvalidPlugin(format!(
                        "the plugin's manifest section disagrees with the plugin itself: {problem}"
                    )));
                }
                Ok(asked)
            }
            None => manifest::read(wasm)
                .map_err(|e| StoreError::InvalidPlugin(e.to_string()))?
                .ok_or_else(|| {
                    StoreError::InvalidPlugin(
                        "this store cannot run plugins, so a plugin has to carry its manifest — \
                         publish it from anistream, or run `anistream plugin stamp` on it first"
                            .into(),
                    )
                }),
        }
    }

    /// Publish a plugin from its bytes.
    ///
    /// Publishing the version already published is accepted when the bytes are the same — a
    /// retried upload — and refused when they differ, because a published version never changes:
    /// clients verify a download against the digest in the index they already hold.
    pub async fn publish(
        &self,
        wasm: &[u8],
        inspector: Option<&dyn Inspector>,
        now: i64,
    ) -> Result<IndexEntry, StoreError> {
        self.publish_with_digest(wasm, inspector, None, now).await
    }

    /// [`Self::publish`], taking the digest as given when one is. See
    /// [`crate::protocol::PublishRequest::sha256`] for when that is right.
    pub async fn publish_with_digest(
        &self,
        wasm: &[u8],
        inspector: Option<&dyn Inspector>,
        digest: Option<&str>,
        now: i64,
    ) -> Result<IndexEntry, StoreError> {
        if wasm.len() as u64 > MAX_PLUGIN_BYTES {
            return Err(StoreError::InvalidPlugin(format!(
                "plugins are limited to {} MiB",
                MAX_PLUGIN_BYTES / 1024 / 1024
            )));
        }
        let info = self.identify(wasm, inspector).await?;
        if !is_valid_id(&info.id) {
            return Err(StoreError::InvalidPlugin(format!(
                "the plugin's id {:?} must be lowercase letters, digits, - and _",
                info.id
            )));
        }
        if RESERVED_IDS.contains(&info.id.as_str()) {
            return Err(StoreError::InvalidPlugin(format!(
                "{} is the name of one of anistream's own sources — give the plugin another id",
                info.id
            )));
        }
        if !is_valid_version(&info.version) {
            return Err(StoreError::InvalidPlugin(format!(
                "the plugin's version {:?} must be letters, digits, ., -, + and _",
                info.version
            )));
        }

        let entry = IndexEntry {
            id: info.id.clone(),
            name: if info.name.trim().is_empty() { info.id.clone() } else { info.name.clone() },
            version: info.version.clone(),
            allowed_hosts: info.allowed_hosts.clone(),
            translations: info.translations.clone(),
            sha256: match digest {
                Some(digest) => {
                    let digest = digest.trim().to_ascii_lowercase();
                    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(StoreError::BadRequest(
                            "that is not a SHA-256 digest".into(),
                        ));
                    }
                    digest
                }
                None => crypto::sha256_hex(wasm),
            },
            size: wasm.len() as u64,
            published_at: now,
        };
        let previous = self.plugin(&entry.id).await?;
        if let Some(previous) = &previous
            && previous.version == entry.version
        {
            return if previous.sha256 == entry.sha256 {
                Ok(previous.clone())
            } else {
                Err(version_conflict(&entry))
            };
        }

        // Named by content, so a file is never overwritten in place: a racing publish of the same
        // version writes a different key, and the row decides which one counts.
        let key = blob_key(&entry);
        self.blobs.put(&key, wasm.to_vec()).await?;
        let mut statements = vec![sql!(
            "INSERT INTO plugins (id, name, version, allowed_hosts, translations, sha256,
                                          size, published_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(id) DO UPDATE SET
                        name = excluded.name, version = excluded.version,
                        allowed_hosts = excluded.allowed_hosts,
                        translations = excluded.translations, sha256 = excluded.sha256,
                        size = excluded.size, published_at = excluded.published_at
                     WHERE plugins.version <> excluded.version",
            entry.id.as_str(),
            entry.name.as_str(),
            entry.version.as_str(),
            serde_json::to_string(&entry.allowed_hosts).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&entry.translations).unwrap_or_else(|_| "[]".into()),
            entry.sha256.as_str(),
            entry.size,
            entry.published_at
        )];
        statements.extend(bump_sequence(now));
        let changed = self.batch(statements).await?;
        if changed.first().copied().unwrap_or(0) == 0 {
            // Someone else published this version between the check and the write.
            let _ = self.blobs.delete(&key).await;
            return Err(version_conflict(&entry));
        }
        if let Some(previous) = previous {
            let _ = self.blobs.delete(&blob_key(&previous)).await;
        }
        Ok(entry)
    }

    /// Stop offering a plugin. Members who installed it keep it; it stops updating.
    pub async fn unpublish(&self, id: &str, now: i64) -> Result<(), StoreError> {
        let previous = self
            .plugin(id)
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("{id} is not published here")))?;
        let mut statements = vec![sql!(
            "DELETE FROM plugins WHERE id = ?1 AND sha256 = ?2",
            id,
            previous.sha256.as_str()
        )];
        statements.extend(bump_sequence(now));
        let changed = self.batch(statements).await?;
        if changed.first().copied().unwrap_or(0) == 0 {
            return Err(StoreError::NotFound(format!("{id} is not published here")));
        }
        let _ = self.blobs.delete(&blob_key(&previous)).await;
        Ok(())
    }

    /// The bytes of the version the index offers, and only that version: an older file that has
    /// not been cleaned up yet is not something a client should be able to fetch.
    pub async fn plugin_bytes(
        &self,
        id: &str,
        version: &str,
    ) -> Result<Option<(IndexEntry, Vec<u8>)>, StoreError> {
        let Some(entry) = self.plugin(id).await?.filter(|e| e.version == version) else {
            return Ok(None);
        };
        Ok(self.blobs.get(&blob_key(&entry)).await?.map(|bytes| (entry, bytes)))
    }

    /// A published version's entry, while that version is the one offered.
    pub async fn published(
        &self,
        id: &str,
        version: &str,
    ) -> Result<Option<IndexEntry>, StoreError> {
        Ok(self.plugin(id).await?.filter(|e| e.version == version))
    }

    /// `len` bytes of a published plugin from `start`, read without the rest of it.
    pub async fn plugin_range(
        &self,
        entry: &IndexEntry,
        start: u64,
        len: u64,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.blobs.get_range(&blob_key(entry), start, len).await?)
    }

    // --- uploads in parts -------------------------------------------------------------------

    /// Start receiving a plugin of `size` bytes, in parts of [`PART_SIZE`].
    pub async fn begin_upload(
        &self,
        member: &str,
        size: u64,
        now: i64,
    ) -> Result<(String, u64), StoreError> {
        if size == 0 || size > MAX_PLUGIN_BYTES {
            return Err(StoreError::InvalidPlugin(format!(
                "plugins are 1 byte to {} MiB",
                MAX_PLUGIN_BYTES / 1024 / 1024
            )));
        }
        // Uploads left unfinished an hour ago are thrown away as new ones start.
        let stale = self
            .query(sql!(
                "SELECT id, size FROM uploads WHERE created_at < ?1",
                now - UPLOAD_SECS
            ))
            .await?;
        for row in &stale {
            let (id, size) = (row.text(0)?, row.int(1)?.max(0) as u64);
            for part in 0..parts_for(size) {
                let _ = self.blobs.delete(&part_key(&id, part)).await;
            }
            self.exec(sql!("DELETE FROM uploads WHERE id = ?1", id.as_str())).await?;
        }
        let id = crypto::new_id()?;
        self.exec(sql!(
            "INSERT INTO uploads (id, member_id, size, created_at) VALUES (?1, ?2, ?3, ?4)",
            id.as_str(),
            member,
            size,
            now
        ))
        .await?;
        Ok((id, parts_for(size)))
    }

    async fn upload(&self, id: &str, member: &str, now: i64) -> Result<u64, StoreError> {
        let rows = self
            .query(sql!(
                "SELECT size FROM uploads WHERE id = ?1 AND member_id = ?2 AND created_at >= ?3",
                id,
                member,
                now - UPLOAD_SECS
            ))
            .await?;
        let row = rows
            .first()
            .ok_or_else(|| StoreError::NotFound("no such upload — start it again".into()))?;
        Ok(row.int(0)?.max(0) as u64)
    }

    /// Store one part. Every part is [`PART_SIZE`] bytes except the last, which is what is left.
    pub async fn put_part(
        &self,
        id: &str,
        member: &str,
        index: u64,
        bytes: Vec<u8>,
        now: i64,
    ) -> Result<(), StoreError> {
        let size = self.upload(id, member, now).await?;
        let parts = parts_for(size);
        if index >= parts {
            return Err(StoreError::BadRequest(format!("this upload has {parts} parts")));
        }
        let expected =
            if index + 1 == parts { size - PART_SIZE * (parts - 1) } else { PART_SIZE };
        if bytes.len() as u64 != expected {
            return Err(StoreError::BadRequest(format!(
                "part {index} should be {expected} bytes and is {}",
                bytes.len()
            )));
        }
        self.blobs.put(&part_key(id, index), bytes).await?;
        Ok(())
    }

    /// Put an upload's parts back together and publish the plugin they make.
    pub async fn finish_upload(
        &self,
        id: &str,
        member: &str,
        inspector: Option<&dyn Inspector>,
        digest: Option<&str>,
        now: i64,
    ) -> Result<IndexEntry, StoreError> {
        let size = self.upload(id, member, now).await?;
        let mut wasm = Vec::with_capacity(size as usize);
        for part in 0..parts_for(size) {
            let bytes = self.blobs.get(&part_key(id, part)).await?.ok_or_else(|| {
                StoreError::BadRequest(format!("part {part} of this upload never arrived"))
            })?;
            wasm.extend_from_slice(&bytes);
        }
        let outcome = self.publish_with_digest(&wasm, inspector, digest, now).await;
        // The parts go whether or not it published: a plugin that did not load will not load on
        // a second try with the same bytes.
        for part in 0..parts_for(size) {
            let _ = self.blobs.delete(&part_key(id, part)).await;
        }
        self.exec(sql!("DELETE FROM uploads WHERE id = ?1", id)).await?;
        outcome
    }

    // --- claiming ---------------------------------------------------------------------------

    /// Claim a store nobody owns yet with the setup code it was deployed with. Answers with an
    /// owner invite, which is what the owner pastes into the app.
    pub async fn claim(
        &self,
        given: &str,
        configured: &str,
        base: &str,
        now: i64,
    ) -> Result<(Invite, i64), StoreError> {
        if !codes_match(given, configured) {
            return Err(StoreError::Forbidden("that is not this store's setup code".into()));
        }
        match self.fresh_owner_invite(base, now).await? {
            Some(invite) => Ok((invite, Self::default_expiry(now))),
            None => Err(StoreError::Conflict(
                "this store already has an owner — ask them for an invite".into(),
            )),
        }
    }
}

/// Revoke one device, and delete its member when the member is anonymous and this was their
/// last working device: one batch, so no member with no name is left with no way in.
fn revoke(device_id: &str, now: i64) -> Vec<Statement> {
    vec![
        sql!(
            "UPDATE devices SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
            now,
            device_id
        ),
        sql!(
            "DELETE FROM members
             WHERE name = '' AND id IN (SELECT member_id FROM devices WHERE id = ?1)
               AND NOT EXISTS (SELECT 1 FROM devices d
                               WHERE d.member_id = members.id AND d.revoked_at IS NULL)",
            device_id
        ),
    ]
}

/// Two codes compared as digests: equal-length, so the comparison's timing says nothing about
/// where a guess went wrong.
fn codes_match(a: &str, b: &str) -> bool {
    crypto::hash_secret(a) == crypto::hash_secret(b)
}

/// What open joins from `client` are counted under: a digest of the address, or of an IPv6
/// address's /64, which one connection is usually handed whole. A digest, so the table never
/// holds an address as it is; and nothing ties a count to the device that joined.
fn join_key(client: &str) -> String {
    crypto::hash_secret(&join_address(client))
}

/// The address an open join is counted by, before it is digested. Anything that is not an
/// address is counted as it is.
fn join_address(client: &str) -> String {
    match client.trim().parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V6(ip)) => match ip.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let [a, b, c, d, ..] = ip.segments();
                format!("{a:x}:{b:x}:{c:x}:{d:x}::/64")
            }
        },
        Ok(ip) => ip.to_string(),
        Err(_) => client.trim().to_owned(),
    }
}

fn render(dialect: Dialect, statement: Statement) -> Statement {
    Statement { sql: dialect.render(&statement.sql), params: statement.params }
}

/// Move the index on: the sequence, and the time it moved. Goes in the same batch as the change.
fn bump_sequence(now: i64) -> [Statement; 2] {
    [
        sql!(
            "INSERT INTO meta (key, value) VALUES ('sequence', '1')
             ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(meta.value AS INTEGER) + 1 AS TEXT)"
        ),
        sql!(
            "INSERT INTO meta (key, value) VALUES ('index_updated_at', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            now.to_string()
        ),
    ]
}

fn version_conflict(entry: &IndexEntry) -> StoreError {
    StoreError::Conflict(format!(
        "{} {} is already published with different contents — raise its version",
        entry.id, entry.version
    ))
}

fn blob_key(entry: &IndexEntry) -> String {
    format!("plugins/{}/{}/{}.wasm", entry.id, entry.version, entry.sha256)
}

fn part_key(upload: &str, part: u64) -> String {
    format!("uploads/{upload}/{part}")
}

fn parts_for(size: u64) -> u64 {
    size.div_ceil(PART_SIZE).max(1)
}

fn entry_from(row: &Row, at: usize) -> Result<IndexEntry, StoreError> {
    let list = |text: String| serde_json::from_str::<Vec<String>>(&text).unwrap_or_default();
    Ok(IndexEntry {
        id: row.text(at)?,
        name: row.text(at + 1)?,
        version: row.text(at + 2)?,
        allowed_hosts: list(row.text(at + 3)?),
        translations: list(row.text(at + 4)?),
        sha256: row.text(at + 5)?,
        size: row.int(at + 6)?.max(0) as u64,
        published_at: row.int(at + 7)?,
    })
}
