//! What can go wrong talking to a Jellyfin server, as the add flow and the server page word it.

/// A failure, named by what the person should be told. The engine turns each into one message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JellyfinError {
    /// Nothing answered at the address.
    #[error("can't reach {address}")]
    Unreachable { address: String },
    /// Something answered, and it is not a Jellyfin server.
    #[error("not a Jellyfin server")]
    NotJellyfin,
    /// The server is older than 10.8, which Quick Connect and the current auth header need.
    #[error("Jellyfin {version} is too old")]
    TooOld { version: String },
    /// The user name or password was refused.
    #[error("wrong user name or password")]
    BadCredentials,
    /// Quick Connect is off on this server.
    #[error("Quick Connect is off")]
    QuickConnectOff,
    /// The Quick Connect code ran out.
    #[error("the Quick Connect code ran out")]
    QuickConnectExpired,
    /// An `http://` address that is not on a private network. The password would travel unprotected.
    #[error("plain HTTP outside the local network")]
    PlainHttp,
    /// The server's certificate is not one this device trusts.
    #[error("untrusted certificate for {host}")]
    UntrustedCertificate { host: String },
    /// The server no longer accepts the saved sign-in.
    #[error("signed out by the server")]
    SignedOut,
    /// The address could not be read as one.
    #[error("not an address")]
    BadAddress,
    /// The server answered with an error status.
    #[error("the server answered {status}")]
    Status { status: u16 },
    /// The server's answer was not the shape Jellyfin answers in.
    #[error("unexpected answer: {0}")]
    Decode(String),
}

pub type Result<T> = std::result::Result<T, JellyfinError>;
