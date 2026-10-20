//! What crosses: the commands a native front end sends, the events it reads, and the rows they
//! carry, each mapped to and from the model's own.
//!
//! Each variant here stands for one of the model's, and means what it means: widening the surface
//! adds variants, and leaves these as they are. What has no variant yet crosses by its name, so
//! a front end can at least see that it happened.

use std::fmt::{self, Write};

use anistream_model::{
    catalog::{Content, ContentFor, Entry, SearchFilters},
    command::Command,
    event::Event,
    notice::NoticeKind,
};

/// Where the engine keeps its files: directories the app chooses, such as its Application
/// Support and Caches directories on macOS, or its container's on iOS.
///
/// Paths as strings, which every foreign language has. Each is created when it is missing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiDirs {
    /// Where `config.toml` is, and the plugins.
    pub config: String,
    /// Where the database is.
    pub data: String,
    /// Where what can be fetched again is kept: covers, logs, the player's leftovers.
    pub cache: String,
}

/// What a front end asks of the engine: the model's `Command`, so far as it crosses.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiCommand {
    /// Search the catalogue for `query`, with no filters: answered with an
    /// [`FfiEvent::Content`] for [`FfiView::Search`] with the same query.
    Search { query: String },
    /// What you were watching, from local history: answered with an [`FfiEvent::Content`] for
    /// [`FfiView::Continue`].
    LoadContinue,
}

impl From<FfiCommand> for Command {
    fn from(command: FfiCommand) -> Self {
        match command {
            FfiCommand::Search { query } => {
                Command::Search { query, filter: SearchFilters::default() }
            }
            FfiCommand::LoadContinue => Command::LoadContinue,
        }
    }
}

/// What the engine says happened: the model's `Event`, so far as it crosses.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiEvent {
    /// A list, for the view that asked for it.
    Content { view: FfiView, content: FfiContent },
    /// Something to tell the user, and whether it is a problem. How long it stays up is the
    /// front end's call.
    Notice { text: String, alert: bool },
    /// The work a command started has ended, whether it succeeded or not: `job` is the number
    /// [`FfiEngine::send`](crate::FfiEngine::send) returned for it.
    JobDone { job: u64 },
    /// An event that does not cross yet, by the name of its variant in the model.
    Other { name: String },
}

impl From<Event> for FfiEvent {
    fn from(event: Event) -> Self {
        match event {
            Event::Content { view, content } => {
                Self::Content { view: view.into(), content: content.into() }
            }
            Event::Toast(notice) => {
                Self::Notice { text: notice.text, alert: notice.kind == NoticeKind::Alert }
            }
            Event::JobDone(job) => Self::JobDone { job: job.get() },
            other => Self::Other { name: variant_name(&other) },
        }
    }
}

/// Which list an [`FfiEvent::Content`] answers.
///
/// Requests are not cancelled when the user moves on, so the answer for a view just left can land
/// after the switch. A front end shows a list only while the view it was asked for is still on
/// screen.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiView {
    /// The Continue row.
    Continue,
    /// A search, by its query as sent, so a slow answer to an earlier search cannot replace the
    /// results of a later one.
    Search { query: String },
    /// A view no command here asks for yet, by the name of its variant in the model.
    Other { name: String },
}

impl From<ContentFor> for FfiView {
    fn from(view: ContentFor) -> Self {
        let other = |name: &str| Self::Other { name: name.to_owned() };
        match view {
            ContentFor::Continue => Self::Continue,
            ContentFor::Search { query, .. } => Self::Search { query },
            ContentFor::Seasonal => other("Seasonal"),
            ContentFor::Calendar => other("Calendar"),
            ContentFor::Library(_) => other("Library"),
        }
    }
}

/// What a list holds.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiContent {
    /// Nothing to show: the Continue row with nothing left part-watched.
    Empty,
    /// On its way.
    Loading,
    /// The titles, in the order the list has them.
    Titles { titles: Vec<FfiTitle> },
    /// Why the list could not be fetched, to show in its place. Never an empty list with no
    /// explanation: when something breaks, the user must be able to see what.
    Failed { reason: String },
}

impl From<Content> for FfiContent {
    fn from(content: Content) -> Self {
        match content {
            Content::Empty => Self::Empty,
            Content::Loading => Self::Loading,
            Content::Entries(entries) => {
                Self::Titles { titles: entries.into_iter().map(FfiTitle::from).collect() }
            }
            Content::Failed(reason) => Self::Failed { reason },
        }
    }
}

/// A title as a list shows it: the first of the model's `Entry` fields to cross.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTitle {
    /// AniList's id, the key for a title everywhere in the engine.
    pub id: u32,
    /// The title to show: the English one when there is one, else the romaji.
    pub title: String,
    /// The romaji, when the title shown is the English one and the two differ: a subheading.
    pub secondary: Option<String>,
}

impl From<Entry> for FfiTitle {
    fn from(entry: Entry) -> Self {
        Self { id: entry.id.get(), title: entry.title, secondary: entry.secondary }
    }
}

/// Why [`FfiEngine::start`](crate::FfiEngine::start) could not stand the engine up, by what
/// failed, each with words a front end can show as they are.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    /// One of the directories could not be created.
    #[error("{reason}")]
    Dirs { reason: String },
    /// `config.toml` could not be read, or says something unsafe or self-contradictory. Refused
    /// rather than replaced by the defaults, which could mean ignoring a VPN setting.
    #[error("{reason}")]
    Config { reason: String },
    /// The database could not be opened.
    #[error("{reason}")]
    Store { reason: String },
    /// The HTTP client could not be built from the network settings.
    #[error("{reason}")]
    Client { reason: String },
    /// The runtime the engine runs on could not be started.
    #[error("{reason}")]
    Runtime { reason: String },
    /// The engine itself did not start.
    #[error("{reason}")]
    Engine { reason: String },
}

/// The name of `event`'s variant, as its `Debug` begins.
///
/// Read from `Debug` rather than matched by hand, so the model's events can change while only a
/// few of them cross, and P6 can map each to a variant of its own. The writer stops at the end of
/// the name, so an event carrying a cover's bytes costs no more to name than one carrying nothing.
fn variant_name(event: &Event) -> String {
    /// Keeps what is written up to the first character a name cannot have, then refuses the rest.
    struct Name(String);

    impl Write for Name {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            let end =
                text.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(text.len());
            self.0.push_str(&text[..end]);
            if end == text.len() { Ok(()) } else { Err(fmt::Error) }
        }
    }

    let mut name = Name(String::new());
    // Refused once the name is whole: the error is the writer stopping, not a failure.
    let _ = write!(name, "{event:?}");
    name.0
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;
    use anistream_model::{jobs::JobId, notice::Notice};

    use super::*;

    #[test]
    fn a_search_is_sent_with_no_filters() {
        let command = FfiCommand::Search { query: "hoshikuzu".into() };
        assert_eq!(
            Command::from(command),
            Command::Search { query: "hoshikuzu".into(), filter: SearchFilters::default() }
        );
        assert_eq!(Command::from(FfiCommand::LoadContinue), Command::LoadContinue);
    }

    #[test]
    fn a_list_crosses_with_its_view_and_its_titles() {
        let mut entry = Entry::new(AnilistId::new(900_001), "Stardust Wayfarer");
        entry.secondary = Some("Hoshikuzu no Tabibito".into());
        let event = Event::Content {
            view: ContentFor::Search {
                query: "hoshikuzu".into(),
                filter: SearchFilters::default(),
            },
            content: Content::Entries(vec![entry]),
        };
        assert_eq!(
            FfiEvent::from(event),
            FfiEvent::Content {
                view: FfiView::Search { query: "hoshikuzu".into() },
                content: FfiContent::Titles {
                    titles: vec![FfiTitle {
                        id: 900_001,
                        title: "Stardust Wayfarer".into(),
                        secondary: Some("Hoshikuzu no Tabibito".into()),
                    }],
                },
            }
        );
    }

    #[test]
    fn a_list_that_failed_says_why() {
        let event = Event::Content {
            view: ContentFor::Continue,
            content: Content::Failed("no disk".into()),
        };
        assert_eq!(
            FfiEvent::from(event),
            FfiEvent::Content {
                view: FfiView::Continue,
                content: FfiContent::Failed { reason: "no disk".into() },
            }
        );
    }

    #[test]
    fn notices_and_job_ends_cross_as_themselves() {
        assert_eq!(
            FfiEvent::from(Event::Toast(Notice::alert("could not reach AniList"))),
            FfiEvent::Notice { text: "could not reach AniList".into(), alert: true }
        );
        assert_eq!(
            FfiEvent::from(Event::Toast(Notice::info("saved"))),
            FfiEvent::Notice { text: "saved".into(), alert: false }
        );
        assert_eq!(FfiEvent::from(Event::JobDone(JobId::new(3))), FfiEvent::JobDone { job: 3 });
    }

    #[test]
    fn everything_else_crosses_by_its_name() {
        let other = |name: &str| FfiEvent::Other { name: name.into() };
        // A unit variant, a tuple variant and a struct variant: each `Debug` begins differently
        // after the name.
        assert_eq!(FfiEvent::from(Event::SourcesChanged), other("SourcesChanged"));
        assert_eq!(FfiEvent::from(Event::Status("searching".into())), other("Status"));
        let started = Event::JobStarted { job: JobId::new(1), activity: "searching".into() };
        assert_eq!(FfiEvent::from(started), other("JobStarted"));
        let artwork = Event::ArtworkReady {
            url: "https://img.example/cover.jpg".into(),
            bytes: vec![0; 1 << 20],
        };
        assert_eq!(FfiEvent::from(artwork), other("ArtworkReady"));
    }
}
