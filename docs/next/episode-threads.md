# Episode discussions

Written 24 October 2026, to build after the parity and website phase. Follows `docs/native-ux.md`.

After an episode, viewers often want to see what other people thought of it. AniList has a forum where, by convention, a thread is opened for each episode of an airing show, titled "[Spoilers] Title - Episode N Discussion". This feature finds that thread for the episode just watched and shows its comments in the app, with spoilers for later episodes kept back, and lets a signed-in viewer reply and like.

Better than Hayase because ours opens the one thread for the episode in front of you, not a list of every thread the show has, holds it back until you've watched the episode, and shows it in a native sheet or pane beside the player rather than a page inside a web view.

## Compared with Hayase

Hayase has a forum view per show: it asks AniList for every thread tagged with the show, and opens a thread's comments in a page of its own. Comment text goes through a markdown renderer and an HTML sanitiser into a shadow root.

Where it falls short:

- There is no episode matching, so the viewer hunts through the list for the episode's thread, past threads for later episodes whose titles are spoilers by themselves.
- There is no spoiler guard: a thread for episode 9 shows to someone on episode 3.
- Failures read "Ooops!" with nothing to try.
- As HTML, every image in a comment loads as soon as the comment appears, which costs data, tells the image's host who is reading, and shows what people post without asking.

What we have that it doesn't: the engine's AniList client with its shared rate budget (`budget_left`, `rate_limit_wait`), the viewer's progress from the library to guard spoilers with, `description_spans`, which already turns AniList's spoiler markup into spans, and native sheets and panes on every platform.

## How AniList's threads work

Checked against the live API on 24 October 2026:

- Threads are made by users, not AniList. A regular poster opens one per episode for most airing shows, in the Release Discussion category (id 5), tagged with the show's media id (`mediaCategories`). Older shows and shows few people watch often have none.
- `Page { threads(mediaCategoryId: $media, categoryId: 5, search: "Episode N", sort: [REPLY_COUNT_DESC]) }` finds candidates. The search matches loosely, so a candidate is kept only when its title matches `Episode\s+0*N\s+Discussion` (case-insensitive) and N is not followed by another digit. Among those, the one with the most replies wins, which settles duplicates.
- `Page { threadComments(threadId: $id, page: $p) }` returns top-level comments, each with `childComments` as a JSON tree, `likeCount`, `isLiked`, `user { name avatar }`, `createdAt` and `comment` in AniList's markdown.
- `SaveThreadComment(threadId, parentCommentId, comment)` posts or edits, `DeleteThreadComment(id)` deletes, and `ToggleLikeV2(id, type: THREAD_COMMENT)` likes. All three need the viewer's AniList token, which the app already holds when signed in.
- Reading needs no token.
- The rate limit is about 30 requests a minute at the time of writing (90 nominally, degraded since 2025). The engine's client already shares one budget across everything it asks.

## Behaviour

### Where it opens

- The episode row's context menu: Discussion. Only on episodes at or before the viewer's progress, unless spoiler guard is off.
- The player's More menu: Discussion, from 85% of the episode on (the point it counts as watched).
- After an episode counts as watched, the episode screen (or the title page's episode list) shows one quiet text row under the episode: "Discussion · 214 comments". It appears only when a thread was found, and only for that episode.

The engine looks the thread up when the episode reaches 85%, so the row and the menu item know whether there is one. Nothing is looked up before.

### Placement

| Platform | Opens as |
| --- | --- |
| Mac | Its own window, 480 × 720 pt, beside the player; one window per thread |
| iPad | A sheet, medium and large detents; in the player, a sheet over the paused video |
| iPhone | A sheet, large detent |
| Android | A modal bottom sheet on phones; a supporting pane at expanded width |
| Windows | A pane on the trailing side of the window, 420 epx |
| Linux | `AdwDialog`, sheet presentation on narrow windows |
| Android TV | Not shown. Reading a forum with a remote is poor, and the TV brief keeps the player clear |
| Terminal | The episode's line in the companion's episode menu opens the thread in the browser |

Opening it from the player pauses playback; closing it doesn't resume, so the viewer chooses.

### The thread

- A header line: the thread's title with "[Spoilers]" left out, the comment count, and Open on AniList as a text button.
- Comments in the order AniList gives top-level ones (oldest first), replies under their parent, indented one step and no deeper; deeper replies collapse into "3 more replies" under the reply they answer.
- Each comment: the author's name and avatar (32 pt, round), the age ("2 h"), the text, and Like with its count as a plain text button, amber when liked. Reply sits next to Like when signed in.
- Text is rendered from engine spans: paragraphs, bold, italic, strike-through, links and spoilers. A spoiler shows as a muted bar with "Spoiler" until tapped, then as text; tapping again hides it.
- Images and videos (`img(…)`, `webm(…)`, `youtube(…)`) show as a text line, "Image" or "Video", that opens it in the browser. A setting can load images inline.
- Links open in the system browser. They are never previewed or fetched by the app.
- Pages of 25 load as the viewer scrolls, with a row of placeholders while the next page comes.

### Posting

- Signed in: a text field at the bottom, "Add a comment", and Post. Reply puts "Replying to name" above the field with a cancel button.
- A posted comment shows at once in its place, dimmed until AniList confirms, then plain. If AniList refuses, it stays dimmed with "Didn't post. Try again" under it and the text kept.
- Own comments get Edit and Delete in their context menu. Delete asks once.
- Not signed in: in place of the field, one line, "Sign in with AniList to reply", which opens the existing sign-in.

### Spoiler guard

With spoiler protection on (the existing setting), Discussion isn't offered for episodes after the viewer's progress, and the engine doesn't fetch their threads. In the player, the item appears only from 85%. Thread titles are never listed, so the guard can't be bypassed by reading a list.

Inside a thread, comments are what people wrote about that episode; the guard can't read them, and the existing spoiler markup is all it honours.

## States

| State | Shown as |
| --- | --- |
| Loading | Nothing for 600 ms, then four placeholder comments (avatar circle and three text lines), then "Still loading from AniList" under the header at 4 s |
| Rate limited | The placeholders stay and the line reads "AniList is busy. Trying again in 20 s", counting down from `rate_limit_wait`. A job note in Messages if it passes a minute |
| No thread | The menu item and the quiet row don't appear. Opened anyway (a stale cache): "No discussion for this episode on AniList" with Open on AniList, which goes to the show's forum page |
| No comments yet | "No comments yet" and, signed in, the field focused |
| Failed | "Couldn't load the discussion" with Try again |
| Offline | The cached comments if any, with "Offline. Showing comments from 2 h ago" as the header's last line; otherwise "You're offline" with Try again. Posting is disabled offline |

## Motion

Sheets and panes use the system's presentation. New comments and a page of comments fade in (fade-in). A spoiler reveals with a 150 ms cross-fade (fade-quick), immediate with Reduce Motion. A posted comment goes from dimmed to plain with fade-quick. The Like count changes in place, with no animation.

## Engine and model changes

```rust
// anistream-meta::anilist
impl Anilist {
    /// The thread for this episode, by the convention's title, or None.
    pub async fn episode_thread(&self, media: AnilistId, episode: u32) -> Result<Option<Thread>>;
    pub async fn thread_comments(&self, thread: u32, page: u32) -> Result<Page<ThreadComment>>;
    pub async fn save_thread_comment(
        &self,
        thread: u32,
        parent: Option<u32>,
        id: Option<u32>,
        text: &str,
    ) -> Result<ThreadComment>;
    pub async fn delete_thread_comment(&self, id: u32) -> Result<bool>;
    pub async fn toggle_comment_like(&self, id: u32) -> Result<bool>;
}

pub struct Thread { pub id: u32, pub title: String, pub reply_count: u32, pub url: String }

pub struct ThreadComment {
    pub id: u32,
    pub author: String,
    pub avatar: Option<String>,
    pub created_at: i64,
    pub markdown: String,
    pub like_count: u32,
    pub liked: bool,
    pub own: bool,
    pub replies: Vec<ThreadComment>,
}

// anistream-model::command
pub enum Command {
    // … as now, plus:
    OpenDiscussion { media: AnilistId, episode: u32 },
    MoreComments,
    PostComment { parent: Option<u32>, text: String },
    EditComment { id: u32, text: String },
    DeleteComment { id: u32 },
    LikeComment { id: u32 },
    CloseDiscussion,
}

// anistream-model::event
pub enum Event {
    // … as now, plus:
    /// Whether an episode has a discussion, once the engine knows.
    DiscussionFound { media: AnilistId, episode: u32, comments: u32 },
    Discussion(DiscussionView),
}

pub struct DiscussionView {
    pub title: String,
    pub comment_count: u32,
    pub url: String,
    pub comments: Vec<CommentView>,
    pub state: DiscussionState,
    pub can_post: bool,
}

pub enum DiscussionState { Loading, Ready { more: bool }, RateLimited { seconds: u32 }, NoThread, Failed, Offline { age_minutes: u32 } }

pub struct CommentView {
    pub id: u32,
    pub author: String,
    pub avatar: Option<String>,
    /// "2 h", worded by the engine.
    pub age: String,
    pub body: Vec<CommentBlock>,
    pub like_count: u32,
    pub liked: bool,
    pub own: bool,
    pub pending: PostState,
    pub replies: Vec<CommentView>,
    pub hidden_replies: u32,
}

pub enum PostState { Posted, Sending, Failed }

pub struct CommentBlock { pub spans: Vec<CommentSpan> }

pub struct CommentSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub spoiler: bool,
    pub link: Option<String>,
    /// An image or video, shown as a line that opens `link`.
    pub media: Option<MediaKind>,
}
```

The engine parses AniList's markdown into `CommentBlock`s, so no app renders markdown or HTML. It reuses the spoiler handling behind `description_spans`.

Caching, in the engine's existing metadata cache:

- A found thread: 1 hour, then refreshed on open.
- No thread, within 2 days of the episode airing: 10 minutes, since the thread may not be up yet.
- No thread, later: 24 hours.
- Comments: kept for offline reading for 7 days, refreshed on every open.

Every request goes through the client's shared budget. Opening a thread costs one request (the cached thread id) or two; each further page, one.

## Settings

| Row | Values | Default | Where |
| --- | --- | --- | --- |
| Episode discussions | On, Off | On | Settings > Library, beside the AniList account |
| Load images in comments | On, Off | Off | The same group, shown when discussions are on |

Spoiler protection, the existing row, also covers discussions; its footnote gains "and discussions for later episodes".

## Privacy and legal

- Reading sends AniList the show's id and the episode number, with no token. Posting and liking use the viewer's AniList token, as the library sync already does.
- Comments are AniList users' words. The app shows them as AniList returns them, attributes each to its author, and links to the thread on AniList. It doesn't keep them beyond the 7-day offline cache.
- Images in comments come from wherever their authors host them. Loading them off by default means the app makes no request to third-party hosts unless the viewer asks.
- Comments may mention where people watched. The app neither filters nor links them in any special way: links open in the browser like any other. anistream hosts no content.
- Moderation is AniList's. The app builds no reporting, blocking or filtering of its own: it posts to AniList, so AniList's rules and moderators apply, and a viewer who wants to report a comment does it on AniList through Open on AniList.

## Size

About two weeks, then the polish pass:

- The AniList calls, the thread match and its tests against recorded responses: two days.
- The markdown-to-spans parser: two days.
- The engine's view, commands, caching and budget: two days.
- The sheet or pane on each app (Mac, iPhone and iPad together, Android, Windows, Linux): four days.

## Decided

Settled on 24 October 2026. Where Johan answered, it says so; every other question took the recommended default.

1. Reading and posting: both from the start, with the viewer's AniList account. Johan's choice. Since everything posts to AniList, there is no moderation surface in the app to build.
2. Images in comments: off by default, a line that opens the browser, with the setting to load them. The default chosen.
3. In the player: the More menu from 85% only. The default chosen.
4. The quiet row after an episode: shown whenever a thread exists. The default chosen.
5. Shows with no thread: nothing shown. The default chosen.

## Sources

- AniList's GraphQL API: <https://docs.anilist.co/>, and its rate limiting: <https://docs.anilist.co/guide/rate-limiting>
- AniList's markdown, including `~!spoilers!~`, `img()` and `youtube()`: <https://anilist.co/forum/thread/6125>
- The Release Discussion category: <https://anilist.co/forum/recent?category=5>
- Hayase's forum, read for this spec: `interface/src/lib/components/ui/forums/Threads.svelte`, and its queries in `interface/src/lib/modules/anilist/queries.ts`.
