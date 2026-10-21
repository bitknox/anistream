//! Plugin stores: the ones this machine has joined, their plugins and members, and the invites an
//! owner makes. Also [`Secret`], which an invite link crosses in, and a tracker's password too.

/// A plugin store this machine has joined, as the store screens show it.
///
/// Flattened by the binary from the store client's own state, so this crate knows nothing about
/// the protocol — the same arrangement as [`ProviderRow`](crate::sources::ProviderRow).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StoreRow {
    pub id: String,
    pub name: String,
    pub url: String,
    /// The host the store answers at, from its URL: "Run by … · plugins.example".
    pub host: String,
    /// The store key's fingerprint, in groups of four.
    pub fingerprint: String,
    /// Who this machine joined as.
    pub member: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub owner: bool,
    pub state: StoreState,
    /// When its plugin list was last fetched and verified, as a Unix time: "checked 2 min
    /// ago". `None` before the first.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub checked_at: Option<i64>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub plugins: Vec<StorePluginRow>,
    /// Who runs it, the longest-standing owner first: "Run by …". Empty from a store that does
    /// not say.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub owners: Vec<String>,
}

impl StoreRow {
    pub fn updates(&self) -> usize {
        self.plugins
            .iter()
            .filter(|p| matches!(p.status, StorePluginStatus::Update { .. }))
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum StoreState {
    Ready,
    /// The last refresh failed. The plugins listed are the last list that was verified.
    Offline(String),
    /// The store no longer accepts this device.
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StorePluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    /// What the plugin may contact. Shown before anything is installed.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hosts: Vec<String>,
    pub status: StorePluginStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum StorePluginStatus {
    Available,
    Installed,
    /// A newer version is offered, and these are the hosts it adds.
    Update {
        from: String,
        new_hosts: Vec<String>,
    },
    /// A file with this id is in the plugin directory and did not come from a store.
    AddedByHand,
    /// Installed from another store, which keeps it updated.
    OtherStore(String),
}

/// What a pasted invite link leads to, or why it leads nowhere: the answer to a preview, which
/// [`Event::StorePreview`](crate::event::Event::StorePreview) carries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum InvitePreview {
    /// The store it leads to, checked against the key the link carries.
    Store(InvitedStore),
    /// Why it leads nowhere, in words to show in its place.
    Failed { reason: String },
}

/// A store an invite link leads to, about to be joined, for the confirmation.
///
/// Not named for the event that carries it: no variant is named after the type it carries. In
/// the Kotlin bindings a variant is a class nested in its enum's, and hides any type of its name
/// spelled bare there.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct InvitedStore {
    pub link: Secret,
    pub name: String,
    pub url: String,
    /// The host the store answers at, from its URL.
    pub host: String,
    pub fingerprint: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub already_joined: bool,
    /// Who runs it, the longest-standing owner first: "Run by …".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub owners: Vec<String>,
    /// What it offers, each with the hosts it contacts and where it stands here: "4 plugins",
    /// before anything is joined or installed. From the store's signed list, checked against the
    /// key the link carries.
    ///
    /// Both empty from a store that predates the preview, which the link joins all the same.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub plugins: Vec<StorePluginRow>,
}

/// A freshly made invite link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct InviteLinkView {
    pub store: String,
    pub link: Secret,
    /// Who it is for. `None` is another device of your own.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub for_whom: Option<String>,
    pub expires_at: i64,
}

/// One member of a store, for its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StoreMemberRow {
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub owner: bool,
    pub devices: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last_seen: Option<i64>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub you: bool,
}

/// An invite nobody has used yet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StoreInviteRow {
    pub id: String,
    pub name: String,
    pub device: bool,
    pub expires_at: i64,
}

/// A plugin in the local plugin directory that an owner could publish.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LocalPluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hosts: Vec<String>,
    /// The version the store already offers, if it offers one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub published: Option<String>,
}

/// The host a store's URL names, lowercase, without a user, a port or a path: what the store
/// screens name it by beside its name. The URL itself when it names none.
pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let address = authority.rsplit('@').next().unwrap_or(authority);
    let host = match address.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or(bracketed),
        None => address.split(':').next().unwrap_or(address),
    };
    if host.is_empty() { url.to_owned() } else { host.to_ascii_lowercase() }
}

/// A credential in transit.
///
/// Exists for exactly one reason: `Command` derives `Debug` and the task router logs a task
/// when routing goes wrong. A plain `String` there would put a password in the log file;
/// this prints a placeholder instead, everywhere, unconditionally.
///
/// The string itself on the far side of an FFI: the bindings name the type, and keeping it out
/// of a log there is the front end's to do.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub String);

#[cfg(feature = "ffi")]
uniffi::custom_newtype!(Secret, String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(«redacted»)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_is_named_by_its_host() {
        assert_eq!(host("https://Plugins.Example/with/a/path"), "plugins.example");
        assert_eq!(host("https://plugins.example:8443"), "plugins.example");
        assert_eq!(host("http://owner@[fe80::1]:8080/store"), "fe80::1");
        assert_eq!(host("plugins.example"), "plugins.example", "no scheme");
        assert_eq!(host("https://"), "https://", "nothing to name it by");
    }
}
