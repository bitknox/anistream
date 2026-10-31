//! Media servers: Jellyfin servers the person runs, their libraries, and the sign-in flow.
//!
//! These are the values the add flow and the server page show. The engine fills them in; the apps
//! only draw them. Nothing here holds a token or a password.

/// A server that answered the broadcast on the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FoundServer {
    pub id: String,
    pub name: String,
    /// Where it says to reach it, such as `http://192.168.1.20:8096`.
    pub address: String,
}

/// A server that answered at an address, and what it can do.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CheckedServer {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The address as the engine will keep it, with its scheme and port.
    pub address: String,
    /// Whether Quick Connect is on, so the sign-in step can offer it.
    pub quick_connect: bool,
}

/// A library a server holds, with whether it is read.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LibraryChoice {
    pub id: String,
    pub name: String,
    /// Jellyfin's word for what the library holds: `tvshows`, `movies` and so on.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub kind: Option<String>,
    /// Ticked: its series are matched to AniList and offered as a source. Libraries that look
    /// like anime start ticked.
    pub ticked: bool,
}

/// One saved server, as its row and page show it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ServerRow {
    pub id: String,
    pub name: String,
    pub address: String,
    pub version: String,
    /// Who is signed in.
    pub user: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub libraries: Vec<LibraryChoice>,
    /// How many series the ticked libraries hold, and how many of them are matched to AniList.
    pub series: u32,
    pub matched: u32,
    /// Whether what is watched is reported back to the server.
    pub report: bool,
    /// The server no longer accepts the saved sign-in.
    pub signed_out: bool,
}

/// Which step of the add flow an error belongs to, so it shows in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ServerStep {
    Address,
    SignIn,
    QuickConnect,
    Libraries,
}

/// Why a step failed. The apps word each from their own strings.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ServerFailure {
    /// Nothing answered at the address.
    Unreachable { address: String },
    /// Something answered, and it is not Jellyfin.
    NotJellyfin,
    /// The server is older than Jellyfin 10.8.
    TooOld { version: String },
    /// Wrong user name or password.
    Credentials,
    /// Quick Connect is off on the server.
    QuickConnectOff,
    /// The Quick Connect code ran out.
    QuickConnectExpired,
    /// An `http://` address that is not on the local network.
    PlainHttp,
    /// The server's certificate is not trusted.
    Certificate { host: String },
    /// The server no longer accepts the saved sign-in.
    SignedOut,
    /// The text is not an address.
    BadAddress,
    /// Anything else the server answered.
    Other,
}

/// Everything the engine says about media servers.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ServerEvent {
    /// A server answered the broadcast. Sent as each arrives.
    Found(FoundServer),
    /// The broadcast window closed.
    DiscoveryDone,
    /// The address is a Jellyfin server of a version that works.
    Checked(CheckedServer),
    /// A step failed. The apps word it, in the place the step shows.
    Failed { step: ServerStep, failure: ServerFailure },
    /// Quick Connect began: show the code and wait.
    QuickConnectCode { code: String },
    /// Signed in, and the server is saved.
    SignedIn { server: String },
    /// The libraries to choose from.
    Libraries { server: String, libraries: Vec<LibraryChoice> },
    /// All saved servers, after any change.
    Servers(Vec<ServerRow>),
}

/// What the person does with media servers.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ServerCommand {
    /// Broadcast on the local network for servers. Answered by `Found` and then `DiscoveryDone`.
    Discover,
    /// Check an address as typed. Answered by `Checked` or `Failed`.
    Check { address: String },
    /// Sign in with a user name and password. The password is not kept.
    SignIn { address: String, user: String, password: String },
    /// Begin Quick Connect. Answered by `QuickConnectCode`, then `SignedIn` once approved.
    StartQuickConnect { address: String },
    /// Stop waiting for a Quick Connect approval.
    CancelQuickConnect,
    /// List a saved server's libraries, with anime-looking ones ticked on first sight.
    LoadLibraries { server: String },
    /// Choose the libraries to read, and match their series to AniList.
    SetLibraries { server: String, libraries: Vec<String> },
    /// Match a server's series to AniList again.
    Refresh { server: String },
    /// Turn reporting what is watched back to the server on or off.
    SetReport { server: String, report: bool },
    /// Forget a server and its sign-in.
    Remove { server: String },
    /// Send `Servers`.
    Load,
}
