//! Notices: what the work tells the user beyond the answer it was asked for, and whether it is a
//! problem.

/// Something to tell the user, and whether it is a problem.
///
/// What the work sends, with nothing in it about how a screen shows it: how long a notice stays
/// up is the front end's call, made in the terminal by its `Toast`, from the kind.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
    /// What the user can do about it, for the front end to say with its own keys or buttons.
    /// The text reads whole without it.
    pub hint: Option<NoticeHint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NoticeKind {
    Info,
    Alert,
}

/// Something the user can do about a notice, named by what it does rather than by the key that
/// does it.
///
/// The work used to say the key itself — "press x to stop", "— Providers, Enter" — which was the
/// terminal's key whatever the front end, and the default one whatever the user had rebound. Each
/// front end now says it its own way: the terminal with its binding, a native app with a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NoticeHint {
    /// Stop what is playing.
    StopPlayback,
    /// Open the plugin stores, behind the Providers screen.
    OpenStores,
    /// Sign in to Kitsu from its row on the Accounts screen, which asks for credentials.
    KitsuCredentials,
}

impl Notice {
    pub fn info(text: impl Into<String>) -> Self {
        Self { kind: NoticeKind::Info, text: text.into(), hint: None }
    }

    pub fn alert(text: impl Into<String>) -> Self {
        Self { kind: NoticeKind::Alert, text: text.into(), hint: None }
    }

    /// The same notice, with what can be done about it.
    pub fn with_hint(self, hint: NoticeHint) -> Self {
        Self { hint: Some(hint), ..self }
    }
}
