//! The wire format. `docs/plugin-stores.md` is the prose version of this file.

use serde::{Deserialize, Serialize};

/// The protocol version this crate speaks.
pub const PROTOCOL: u32 = 1;

/// The largest plugin a client downloads or a server accepts.
///
/// The JavaScript reference plugin is about 12 MiB, so this leaves room for a heavier runtime
/// without letting a store fill someone's disk.
pub const MAX_PLUGIN_BYTES: u64 = 32 * 1024 * 1024;

/// How much of a plugin moves in one request, uploading or downloading.
///
/// Small enough for every platform a store runs on — serverless functions cap a request or a
/// response at 4.5 MB — so no request anywhere carries more than this.
pub const PART_SIZE: u64 = 4 * 1024 * 1024;

/// How long an invite lasts unless the owner says otherwise.
pub const INVITE_LIFETIME_SECS: i64 = 7 * 24 * 60 * 60;

/// `GET /v1/store`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreInfo {
    pub protocol: u32,
    pub name: String,
    /// The Ed25519 public key, unpadded base64url.
    pub key: String,
    /// Whether nobody owns the store yet and it was deployed with a setup code, so `POST
    /// /v1/setup` can claim it. Left out otherwise.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub claimable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Member,
    Owner,
}

impl Role {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Owner => "owner",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "member" => Some(Self::Member),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub label: String,
}

/// `POST /v1/redeem`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemRequest {
    pub invite: String,
    pub device: String,
}

/// `POST /v1/preview`: what an invite leads to, before it is spent. Answered with the
/// [`SignedIndex`], which holding the invite entitles one to see.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRequest {
    pub invite: String,
}

/// `POST /v1/open/preview` and `POST /v1/open/join`: the code from a store's open link, which
/// anyone holding the link may use as often as they like while the store lets anyone join.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenJoinRequest {
    pub code: String,
}

/// The code is what lets anyone in, so it never reaches a log line through `{:?}`.
impl std::fmt::Debug for OpenJoinRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenJoinRequest(«redacted»)")
    }
}

/// `PUT /v1/open`: who can join. `false`, the default, is only people the owners invite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSetting {
    pub open: bool,
}

/// `GET /v1/open`, and the answer to every change of it: who can join, the link anyone can join
/// with while it is open, and the devices that joined that way.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpenState {
    /// Whether anyone with the link can join. Otherwise only people the owners invite can.
    pub open: bool,
    /// `{base}/join/{code}?open#{key}`, while the store is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// How many devices that joined with a link still work here, whether or not the store is
    /// still open.
    pub devices: u32,
    /// The most recently joined of them, newest first, up to [`OPEN_DEVICES_LISTED`]. Their
    /// labels are empty: nothing about who joined is kept.
    #[serde(default)]
    pub device_list: Vec<DeviceRow>,
}

/// How many anonymous devices [`OpenState::device_list`] lists at most.
pub const OPEN_DEVICES_LISTED: u32 = 100;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedeemResponse {
    pub token: String,
    pub member: Member,
}

/// The token is a credential, so it never reaches a log line through `{:?}`.
impl std::fmt::Debug for RedeemResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedeemResponse")
            .field("token", &"«redacted»")
            .field("member", &self.member)
            .finish()
    }
}

/// `GET /v1/me`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub member: Member,
    pub device: Device,
}

/// `POST /v1/invites`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InviteRequest {
    /// Another device for whoever is asking.
    Device,
    /// Someone new. Owners only.
    Member {
        name: String,
        #[serde(default)]
        role: Role,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InviteCreated {
    pub id: String,
    pub link: String,
    pub expires_at: i64,
}

/// `GET /v1/members`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemberList {
    pub members: Vec<MemberRow>,
    pub invites: Vec<PendingInvite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRow {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub joined_at: i64,
    pub last_seen: Option<i64>,
    pub devices: u32,
    /// Each device, for the admin page. Absent from servers that predate it.
    #[serde(default)]
    pub device_list: Vec<DeviceRow>,
}

/// One device of a member, for the owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRow {
    pub id: String,
    pub label: String,
    pub created_at: i64,
    pub last_seen: Option<i64>,
}

/// `PATCH /v1/members/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberChange {
    /// A new role. Not for yourself.
    #[serde(default)]
    pub role: Option<Role>,
    /// A new name, as the members list shows it. Yours included.
    #[serde(default)]
    pub name: Option<String>,
}

/// `POST /v1/admin/links`: a one-time link that signs a browser in to the admin page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminLink {
    pub link: String,
    pub expires_at: i64,
}

/// `POST /v1/admin/session`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminLogin {
    pub code: String,
}

impl std::fmt::Debug for AdminLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdminLogin(«redacted»)")
    }
}

/// `GET /v1/admin/overview`: everything the admin page draws, in one answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminOverview {
    pub store: StoreInfo,
    pub url: String,
    pub fingerprint: String,
    pub me: Member,
    pub sequence: u64,
    pub plugins: Vec<IndexEntry>,
    pub members: Vec<MemberRow>,
    pub invites: Vec<PendingInvite>,
    /// Who can join, and the devices that joined with the open link. Absent from servers that
    /// predate open joining, which read as invite-only.
    #[serde(default)]
    pub open: OpenState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteKind {
    Member,
    Device,
}

impl InviteKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Device => "device",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "member" => Some(Self::Member),
            "device" => Some(Self::Device),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingInvite {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub kind: InviteKind,
    pub expires_at: i64,
}

/// `GET /v1/index`: the index document as a string, and its signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedIndex {
    pub index: String,
    pub signature: String,
}

/// The document inside a [`SignedIndex`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub protocol: u32,
    pub store: String,
    pub key: String,
    pub sequence: u64,
    pub issued_at: i64,
    pub plugins: Vec<IndexEntry>,
    /// Who runs the store: its owners' names, the longest-standing first, for "Run by …". Like
    /// `store`, it describes the store as it is now and moves no `sequence`, which counts the
    /// plugin list. Left out by a store from before it was, and then read as none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
    /// Whether anyone with the store's link can join it, rather than only people its owners
    /// invite: something a member should know about who else is there. Like `owners`, it
    /// describes the store as it is now and moves no `sequence`. Left out while it is false, and
    /// by a store from before it was.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open: bool,
}

impl Index {
    pub fn plugin(&self, id: &str) -> Option<&IndexEntry> {
        self.plugins.iter().find(|p| p.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub translations: Vec<String>,
    pub sha256: String,
    pub size: u64,
    pub published_at: i64,
}

/// `POST /v1/uploads`: start sending a plugin in parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadRequest {
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadStarted {
    pub id: String,
    pub part_size: u64,
    pub parts: u64,
}

/// `POST /v1/plugins`: publish what an upload carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishRequest {
    pub upload: String,
    /// The plugin's SHA-256, from whoever sent it. A store with CPU to spare computes it anyway;
    /// one that runs under a tight CPU budget, as a Worker on Cloudflare's free plan does, may
    /// take it instead. Members verify their downloads against it either way.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// `POST /v1/setup`: claim a store that has no owner yet, with the code it was deployed with.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRequest {
    pub code: String,
}

impl std::fmt::Debug for ClaimRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClaimRequest(«redacted»)")
    }
}

/// The owner invite a claim produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claimed {
    pub link: String,
    pub expires_at: i64,
}

/// Every error body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
    pub message: String,
}

/// Whether a plugin id is safe to use as a file name and a path segment.
///
/// Checked by the server before it publishes and by the client before it installs, because an id
/// becomes `{id}.wasm` on every member's disk and a `../` in it would be a path traversal into
/// someone's home directory. The same alphabet the plugin settings table can be keyed by.
pub fn is_valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && !id.starts_with(['-', '_'])
}

/// Ids anistream's own sources already use. A plugin by one of these names would take a built-in
/// source's place in the chain, so a server will not publish one and a client will not install one.
pub const RESERVED_IDS: &[&str] = &["torrent", "remote", "plugins"];

/// Whether a version string is safe to put in a path segment and a file name.
pub fn is_valid_version(version: &str) -> bool {
    (1..=64).contains(&version.len())
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b'_'))
        && !version.starts_with('.')
}

/// Hosts compared the way the sandbox compares them: trimmed, lowercase, as a set.
pub fn host_set(hosts: &[String]) -> std::collections::BTreeSet<String> {
    hosts.iter().map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_that_could_escape_a_directory_are_invalid() {
        for good in ["example", "my-plugin", "plugin_2", "a"] {
            assert!(is_valid_id(good), "{good}");
        }
        for bad in ["", "../evil", "a/b", "Example", "-dash", "dot.ted", "with space", "é"] {
            assert!(!is_valid_id(bad), "{bad}");
        }
        assert!(!is_valid_id(&"a".repeat(65)));
    }

    #[test]
    fn versions_that_could_escape_a_directory_are_invalid() {
        for good in ["1.4.0", "2.0.0-beta.1", "1.0.0+build.5", "v3"] {
            assert!(is_valid_version(good), "{good}");
        }
        for bad in ["", "..", "../1", "1/2", "1 2", ".hidden"] {
            assert!(!is_valid_version(bad), "{bad}");
        }
    }

    #[test]
    fn an_invite_request_is_tagged_by_kind() {
        let device: InviteRequest = serde_json::from_str(r#"{"kind":"device"}"#).unwrap();
        assert_eq!(device, InviteRequest::Device);
        let member: InviteRequest =
            serde_json::from_str(r#"{"kind":"member","name":"Alex"}"#).unwrap();
        assert_eq!(member, InviteRequest::Member { name: "Alex".into(), role: Role::Member });
        let owner: InviteRequest =
            serde_json::from_str(r#"{"kind":"member","name":"Sam","role":"owner"}"#).unwrap();
        assert_eq!(owner, InviteRequest::Member { name: "Sam".into(), role: Role::Owner });
    }

    #[test]
    fn a_redeemed_token_never_prints() {
        let response = RedeemResponse {
            token: "ast1_secret".into(),
            member: Member { id: "1".into(), name: "Alex".into(), role: Role::Member },
        };
        assert!(!format!("{response:?}").contains("secret"));
    }

    #[test]
    fn an_index_says_it_is_open_only_when_it_is() {
        // Left out while false, so an invite-only store signs the same bytes it did before open
        // joining existed, and an index from such a store reads as invite-only.
        let index = Index {
            protocol: PROTOCOL,
            store: "Ours".into(),
            key: String::new(),
            sequence: 1,
            issued_at: 0,
            plugins: Vec::new(),
            owners: Vec::new(),
            open: false,
        };
        let closed = serde_json::to_string(&index).unwrap();
        assert!(!closed.contains("open"), "{closed}");
        assert!(!serde_json::from_str::<Index>(&closed).unwrap().open);
        let open = serde_json::to_string(&Index { open: true, ..index }).unwrap();
        assert!(serde_json::from_str::<Index>(&open).unwrap().open);
    }

    #[test]
    fn an_open_code_never_prints() {
        let request = OpenJoinRequest { code: "k3TzQe9WbA1xLr7uVn2c0g".into() };
        assert!(!format!("{request:?}").contains("k3Tz"));
    }

    #[test]
    fn host_sets_ignore_case_order_and_blanks() {
        let a = host_set(&["API.example".into(), "cdn.example".into(), " ".into()]);
        let b = host_set(&["cdn.example".into(), "api.example ".into()]);
        assert_eq!(a, b);
    }
}
