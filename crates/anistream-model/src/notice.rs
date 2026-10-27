//! Notices: what the work tells the user beyond the answer it was asked for, and whether it is a
//! problem.

use crate::Msg;

/// Something to tell the user, and whether it is a problem.
///
/// What the work sends, with nothing in it about how a screen shows it: how long a notice stays
/// up is the front end's call, made in the terminal by its `Toast`, from the kind.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Notice {
    pub kind: NoticeKind,
    /// The English rendering of [`msg`](Self::msg), for a front end that does not render
    /// messages itself yet. Removed once the last one does.
    pub text: String,
    /// What the user can do about it, for the front end to say with its own keys or buttons.
    /// The text reads whole without it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hint: Option<NoticeHint>,
    /// What the notice says, as a catalogue message for the front end to render in the person's
    /// language. Always set by the engine; `None` only on a notice a front end made itself, which
    /// reads as its `text`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub msg: Option<Msg>,
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
    pub fn new(kind: NoticeKind, msg: Msg) -> Self {
        Self { kind, text: msg.english(), hint: None, msg: Some(msg) }
    }

    pub fn info(msg: Msg) -> Self {
        Self::new(NoticeKind::Info, msg)
    }

    pub fn alert(msg: Msg) -> Self {
        Self::new(NoticeKind::Alert, msg)
    }

    /// A notice whose words are still written in the engine. Being converted to catalogue messages;
    /// each use is a sentence the catalogue does not have yet.
    pub fn raw_info(text: impl Into<String>) -> Self {
        Self::info(Msg::verbatim(text))
    }

    /// See [`Notice::raw_info`].
    pub fn raw_alert(text: impl Into<String>) -> Self {
        Self::alert(Msg::verbatim(text))
    }

    /// What the notice says: its message, or for one a front end made itself, its text.
    pub fn message(&self) -> Msg {
        self.msg.clone().unwrap_or_else(|| Msg::verbatim(self.text.clone()))
    }

    /// The same notice, with what can be done about it.
    pub fn with_hint(self, hint: NoticeHint) -> Self {
        Self { hint: Some(hint), ..self }
    }
}
