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
    /// The store key's fingerprint, in groups of four.
    pub fingerprint: String,
    /// Who this machine joined as.
    pub member: String,
    pub owner: bool,
    pub state: StoreState,
    pub plugins: Vec<StorePluginRow>,
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

/// A store about to be joined, for the confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StorePreview {
    pub link: Secret,
    pub name: String,
    pub url: String,
    pub fingerprint: String,
    pub already_joined: bool,
}

/// A freshly made invite link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct InviteLinkView {
    pub store: String,
    pub link: Secret,
    /// Who it is for. `None` is another device of your own.
    pub for_whom: Option<String>,
    pub expires_at: i64,
}

/// One member of a store, for its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StoreMemberRow {
    pub id: String,
    pub name: String,
    pub owner: bool,
    pub devices: u32,
    pub last_seen: Option<i64>,
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
    pub hosts: Vec<String>,
    /// The version the store already offers, if it offers one.
    pub published: Option<String>,
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
