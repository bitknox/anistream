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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NoticeKind {
    Info,
    Alert,
}

impl Notice {
    pub fn info(text: impl Into<String>) -> Self {
        Self { kind: NoticeKind::Info, text: text.into() }
    }

    pub fn alert(text: impl Into<String>) -> Self {
        Self { kind: NoticeKind::Alert, text: text.into() }
    }
}
