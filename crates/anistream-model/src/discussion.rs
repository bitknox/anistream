//! An episode's discussion: the AniList forum thread its viewers opened for it, with its
//! comments as the engine read them, and the viewer's own replies on their way.
//!
//! The engine finds the thread by its title, reads the comments' markdown into blocks of spans,
//! and holds a later episode's thread back while spoiler protection is on. A front end draws
//! what [`DiscussionView`] says and sends commands back: it renders no markdown or HTML, and it
//! asks AniList nothing itself.

use anistream_core::ids::AnilistId;

use crate::Msg;

/// The discussion open in a sheet or pane, whole, said again on every change: see
/// [`Event::Discussion`](crate::event::Event::Discussion).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DiscussionView {
    /// The title, which says which discussion this is, for a view that holds one per title.
    pub media: AnilistId,
    pub episode: u32,
    /// The thread's title with "[Spoilers]" left out: empty until the thread is found.
    pub title: String,
    /// Every comment in the thread, replies included, as AniList counts them.
    pub comment_count: u32,
    /// Open on AniList: the thread, or the show's forum page when there is no thread.
    pub url: String,
    /// The comments read so far, top-level ones in AniList's order, oldest first.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub comments: Vec<CommentView>,
    pub state: DiscussionState,
    /// Whether the viewer can post, reply and like: signed in to AniList, online, and a thread
    /// to post to. Signed out, the field's place says "Sign in with AniList to reply".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub can_post: bool,
    /// Whether anyone is signed in to AniList, online or not: the line in the field's place.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub signed_in: bool,
    /// Whether images in comments load inline, as Settings says: off, each is a line that opens
    /// it in the browser.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub load_images: bool,
}

/// Where the discussion stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum DiscussionState {
    /// Being read. Nothing shows for 600 ms, then placeholder comments.
    #[default]
    Loading,
    /// Read, with more pages to come when `more`: [`Command::MoreComments`] asks for the next.
    ///
    /// [`Command::MoreComments`]: crate::command::Command::MoreComments
    Ready { more: bool },
    /// AniList's budget is spent for now: the read waits this long, then goes on by itself.
    /// The placeholders stay, with the countdown.
    RateLimited { seconds: u32 },
    /// Nobody opened a thread for this episode on AniList.
    NoThread,
    /// The read failed: Try again sends [`Command::OpenDiscussion`] again.
    ///
    /// [`Command::OpenDiscussion`]: crate::command::Command::OpenDiscussion
    Failed,
    /// Offline: the comments shown are the ones read `age_minutes` ago, or there are none and
    /// the sheet says "You're offline" with Try again. Posting waits for the network.
    Offline { age_minutes: u32 },
}

/// One comment.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CommentView {
    /// AniList's id, or one the engine made up for a comment still on its way, until AniList
    /// answers with its own.
    pub id: u32,
    pub author: String,
    /// The author's avatar, shown round at 32 pt.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub avatar: Option<String>,
    /// When it was written, in Unix seconds, for a front end that words its own age.
    pub created_at: i64,
    /// How long ago, worded by the engine: "2 h".
    pub age: Msg,
    /// The text, in paragraphs.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub body: Vec<CommentBlock>,
    pub like_count: u32,
    /// Whether the viewer likes it, drawn amber.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub liked: bool,
    /// The viewer's own, which offers Edit and Delete.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub own: bool,
    pub pending: PostState,
    /// Under a top-level comment, its replies, one step in. Under a reply, the replies to it and
    /// theirs, flattened oldest first, which the front end shows only once the viewer opens
    /// "3 more replies".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub replies: Vec<CommentView>,
    /// How many replies the "3 more replies" line counts: those in `replies` of a reply, and
    /// none for a top-level comment, whose replies show.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hidden_replies: u32,
    /// The comment's own text as it was written, for Edit to start from.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub markdown: String,
}

/// Where the viewer's own comment, edit or deletion stands with AniList.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PostState {
    /// On AniList, as anyone reads it.
    #[default]
    Posted,
    /// Sent, and not yet answered: shown dimmed.
    Sending,
    /// AniList refused it or could not be reached: dimmed, with "Didn't post. Try again" under
    /// it and the text kept. Sending the same text again tries again.
    Failed,
}

/// One paragraph of a comment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CommentBlock {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spans: Vec<CommentSpan>,
    /// A quote of another comment, drawn set in.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub quote: bool,
}

/// A run of text and how it is set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CommentSpan {
    pub text: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub bold: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub italic: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub strike: bool,
    /// Marked a spoiler by its author: a muted bar reading "Spoiler" until tapped. Neighbouring
    /// spoiler spans are one spoiler.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spoiler: bool,
    /// A link, opened in the system browser and never previewed or fetched by the app.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub link: Option<String>,
    /// An image or video, shown as a line, "Image" or "Video", that opens `link`. `text` is
    /// empty: the line's words are the front end's.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub media: Option<MediaKind>,
}

/// What a comment embeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MediaKind {
    /// `img(…)`: loaded inline only when Settings says so.
    Image,
    /// `webm(…)` or `youtube(…)`: always a line that opens it.
    Video,
}
