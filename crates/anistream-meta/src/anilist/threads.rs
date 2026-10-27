//! AniList's forum threads for one episode: finding the thread, reading its comments, and the
//! viewer's own replies, edits, deletions and likes.
//!
//! Threads are opened by AniList's users, not AniList. By convention one is opened for each
//! episode of most airing shows in the Release Discussion category, titled
//! "[Spoilers] Title - Episode N Discussion" and tagged with the show. Older shows and shows few
//! people watch often have none, so finding nothing is an ordinary answer, not a failure.
//!
//! Reading needs no token. Posting, editing, deleting and liking need the viewer's.

use anistream_core::ids::AnilistId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{AniList, AniListError, Page, Result, query};

/// How many top-level comments a page holds.
pub const COMMENTS_PER_PAGE: u32 = 25;

/// One episode's discussion thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    pub id: u32,
    /// As its author titled it, "[Spoilers]" and all: see [`shown_title`].
    pub title: String,
    /// Every comment in it, replies included.
    pub reply_count: u32,
    /// The thread on AniList's site.
    pub url: String,
}

/// One comment, with the replies to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadComment {
    pub id: u32,
    pub author: String,
    /// The author's AniList id, which tells the viewer's own comments apart.
    pub author_id: u32,
    pub avatar: Option<String>,
    /// Unix seconds.
    pub created_at: i64,
    /// In AniList's markdown, as written.
    pub markdown: String,
    pub like_count: u32,
    /// Whether the viewer likes it; always false when nobody is signed in.
    pub liked: bool,
    /// The replies to it, oldest first, each with its own.
    pub replies: Vec<ThreadComment>,
}

impl ThreadComment {
    /// This comment and every reply under it, however deep.
    pub fn count(&self) -> u32 {
        1 + self.replies.iter().map(Self::count).sum::<u32>()
    }
}

/// Where a like stands after a toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liked {
    pub liked: bool,
    pub count: u32,
}

/// Whether `title` is the thread for episode `episode`: "Episode", the number with any zeros
/// before it and no digit after it, and "Discussion", apart from case and spacing.
///
/// "Episode 1 Discussion" is not episode 10's, and "Episode 10 Discussion" is not episode 1's.
pub fn names_episode(title: &str, episode: u32) -> bool {
    let lower = title.to_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("episode") {
        let start = from + at + "episode".len();
        from = start;
        let rest = &lower[start..];
        let spaced = rest.trim_start();
        if spaced.len() == rest.len() {
            continue;
        }
        let digits: String = spaced.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() || digits.parse::<u64>().ok() != Some(u64::from(episode)) {
            continue;
        }
        let after = &spaced[digits.len()..];
        let word = after.trim_start();
        if word.len() < after.len() && word.starts_with("discussion") {
            return true;
        }
    }
    false
}

/// The thread's title as a header shows it: "[Spoilers]" left out, since the episode is one
/// the viewer has watched.
pub fn shown_title(title: &str) -> String {
    let trimmed = title.trim();
    let tag = "[spoilers]";
    let cut = trimmed
        .get(..tag.len())
        .filter(|head| head.eq_ignore_ascii_case(tag))
        .map_or(trimmed, |_| &trimmed[tag.len()..]);
    cut.trim().to_owned()
}

/// The episode's thread among the candidates AniList answered: the most replied of those that
/// name the episode.
pub fn pick_thread(candidates: &Value, episode: u32) -> Option<Thread> {
    candidates
        .as_array()?
        .iter()
        .filter_map(decode_thread)
        .filter(|thread| names_episode(&thread.title, episode))
        .max_by_key(|thread| thread.reply_count)
}

fn decode_thread(node: &Value) -> Option<Thread> {
    let id = u32::try_from(node["id"].as_u64()?).ok()?;
    let title = node["title"].as_str()?.to_owned();
    let url = node["siteUrl"]
        .as_str()
        .map_or_else(|| format!("https://anilist.co/forum/thread/{id}"), str::to_owned);
    Some(Thread { id, title, reply_count: number(&node["replyCount"]), url })
}

/// One comment as AniList answers it, from the query or from a `childComments` tree: `None`
/// for one without an id or an author, such as a deleted account's.
pub fn decode_comment(node: &Value) -> Option<ThreadComment> {
    let id = u32::try_from(node["id"].as_u64()?).ok()?;
    let user = &node["user"];
    let author = user["name"].as_str()?.to_owned();
    let avatar = user["avatar"]["medium"]
        .as_str()
        .or_else(|| user["avatar"]["large"].as_str())
        .filter(|url| !url.is_empty())
        .map(str::to_owned);
    Some(ThreadComment {
        id,
        author,
        author_id: number(&user["id"]),
        avatar,
        created_at: node["createdAt"].as_i64().unwrap_or_default(),
        markdown: node["comment"].as_str().unwrap_or_default().to_owned(),
        like_count: number(&node["likeCount"]),
        liked: node["isLiked"].as_bool().unwrap_or(false),
        replies: replies(&node["childComments"]),
    })
}

/// A `childComments` tree: an array, sometimes sent as a string of JSON, or null for none.
fn replies(tree: &Value) -> Vec<ThreadComment> {
    let parsed;
    let tree = match tree {
        Value::String(text) => {
            parsed = serde_json::from_str::<Value>(text).unwrap_or(Value::Null);
            &parsed
        }
        other => other,
    };
    let mut replies: Vec<ThreadComment> =
        tree.as_array().into_iter().flatten().filter_map(decode_comment).collect();
    replies.sort_by_key(|reply| (reply.created_at, reply.id));
    replies
}

fn number(value: &Value) -> u32 {
    value.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or_default()
}

/// A page of comments as AniList answers it.
pub fn decode_comment_page(data: &Value) -> Page<ThreadComment> {
    let page = &data["Page"];
    Page {
        items: page["threadComments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(decode_comment)
            .collect(),
        has_next: page["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false),
    }
}

impl AniList {
    /// A client asking with this one's token, against `base`'s budget and endpoint: the
    /// viewer's own writes, counted with everything else the app asks AniList.
    pub fn on_budget_of(&self, base: &AniList) -> AniList {
        AniList {
            http: base.http.clone(),
            limiter: base.limiter.clone(),
            token: self.token.clone(),
            endpoint: base.endpoint.clone(),
        }
    }

    /// The thread for `episode` of `media`, by the convention's title, or `None` when nobody
    /// opened one. One request.
    pub async fn episode_thread(&self, media: AnilistId, episode: u32) -> Result<Option<Thread>> {
        let data: Value = self
            .execute(
                query::EPISODE_THREADS,
                json!({ "media": media.get(), "search": format!("Episode {episode}") }),
            )
            .await?;
        Ok(pick_thread(&data["Page"]["threads"], episode))
    }

    /// Page `page` of a thread's top-level comments, from 1, [`COMMENTS_PER_PAGE`] to a page,
    /// each with its replies. One request.
    pub async fn thread_comments(&self, thread: u32, page: u32) -> Result<Page<ThreadComment>> {
        let data: Value = self
            .execute(
                &query::thread_comments(),
                json!({ "thread": thread, "page": page.max(1), "perPage": COMMENTS_PER_PAGE }),
            )
            .await?;
        if !data["Page"]["threadComments"].is_array() {
            return Err(AniListError::Decode("the page had no comments list".into()));
        }
        Ok(decode_comment_page(&data))
    }

    /// Post `text` to `thread`, as a reply to `parent` when set, or put it in place of comment
    /// `id`'s text when that is set. Answered with the comment as AniList keeps it.
    pub async fn save_thread_comment(
        &self,
        thread: u32,
        parent: Option<u32>,
        id: Option<u32>,
        text: &str,
    ) -> Result<ThreadComment> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let mut variables = serde_json::Map::new();
        variables.insert("thread".into(), thread.into());
        variables.insert("comment".into(), text.into());
        if let Some(parent) = parent {
            variables.insert("parent".into(), parent.into());
        }
        if let Some(id) = id {
            variables.insert("id".into(), id.into());
        }
        let data: Value =
            self.execute(&query::save_thread_comment(), Value::Object(variables)).await?;
        decode_comment(&data["SaveThreadComment"])
            .ok_or_else(|| AniListError::Decode("the saved comment did not come back".into()))
    }

    /// Delete one of the viewer's comments: whether AniList says it is gone.
    pub async fn delete_thread_comment(&self, id: u32) -> Result<bool> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: Value =
            self.execute(query::DELETE_THREAD_COMMENT, json!({ "id": id })).await?;
        Ok(data["DeleteThreadComment"]["deleted"].as_bool().unwrap_or(false))
    }

    /// Like a comment, or take the like back.
    pub async fn toggle_comment_like(&self, id: u32) -> Result<Liked> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: Value = self.execute(query::TOGGLE_COMMENT_LIKE, json!({ "id": id })).await?;
        let node = &data["ToggleLikeV2"];
        let liked = node["isLiked"]
            .as_bool()
            .ok_or_else(|| AniListError::Decode("the like did not come back".into()))?;
        Ok(Liked { liked, count: number(&node["likeCount"]) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_names_its_episode_and_no_other() {
        let title = "[Spoilers] Lantern Harbour - Episode 5 Discussion";
        assert!(names_episode(title, 5));
        assert!(!names_episode(title, 50));
        assert!(!names_episode("[Spoilers] Lantern Harbour - Episode 50 Discussion", 5));
        assert!(!names_episode("[Spoilers] Lantern Harbour - Episode 15 Discussion", 5));
        assert!(names_episode("[SPOILERS] lantern harbour - EPISODE  05  discussion", 5));
        assert!(names_episode("Episode 010 Discussion", 10));
        assert!(!names_episode("Episode 5 Preview", 5));
        assert!(!names_episode("Episode5 Discussion", 5), "the number stands apart");
        assert!(!names_episode("Episode 5Discussion", 5));
        assert!(!names_episode("Episode 5.5 Discussion", 5), "a recap is not the episode");
        // A title naming another episode first still matches the one it names after.
        assert!(names_episode("Episode 4 recap and Episode 5 Discussion", 5));
    }

    #[test]
    fn the_header_leaves_the_spoiler_tag_out() {
        assert_eq!(
            shown_title(" [Spoilers] Lantern Harbour - Episode 5 Discussion"),
            "Lantern Harbour - Episode 5 Discussion"
        );
        assert_eq!(shown_title("Episode 5 Discussion"), "Episode 5 Discussion");
    }

    /// A recorded answer to [`query::EPISODE_THREADS`], names made up.
    fn candidates() -> Value {
        json!([
            { "id": 9001, "title": "[Spoilers] Lantern Harbour - Episode 15 Discussion",
              "replyCount": 400, "siteUrl": "https://anilist.co/forum/thread/9001" },
            { "id": 9002, "title": "[Spoilers] Lantern Harbour - Episode 5 Discussion",
              "replyCount": 214, "siteUrl": "https://anilist.co/forum/thread/9002" },
            { "id": 9003, "title": "Lantern Harbour Episode 5 Discussion (duplicate)",
              "replyCount": 3, "siteUrl": "https://anilist.co/forum/thread/9003" },
            { "id": 9004, "title": "Episode 5 was something", "replyCount": 900 }
        ])
    }

    #[test]
    fn the_most_replied_thread_naming_the_episode_wins() {
        let thread = pick_thread(&candidates(), 5).expect("found");
        assert_eq!(thread.id, 9002);
        assert_eq!(thread.reply_count, 214);
        assert_eq!(thread.url, "https://anilist.co/forum/thread/9002");
        assert_eq!(pick_thread(&candidates(), 7), None, "none names episode 7");
        assert_eq!(pick_thread(&Value::Null, 5), None);
    }

    /// A recorded answer to [`query::thread_comments`], names made up. The replies come as
    /// AniList sends them: a JSON tree, one of them as a string.
    fn page() -> Value {
        json!({ "Page": {
            "pageInfo": { "hasNextPage": true },
            "threadComments": [
                { "id": 1, "comment": "What a ~!twist!~", "likeCount": 4, "isLiked": true,
                  "createdAt": 1_000, "user": { "id": 11, "name": "kumo", "avatar": { "medium": "https://img.example/kumo.png" } },
                  "childComments": [
                    { "id": 3, "comment": "Right?", "likeCount": 0, "isLiked": false, "createdAt": 1_200,
                      "user": { "id": 12, "name": "hoshi", "avatar": { "large": "https://img.example/hoshi.png" } },
                      "childComments": [
                        { "id": 4, "comment": "Agreed", "createdAt": 1_300, "user": { "id": 11, "name": "kumo" } }
                      ] },
                    { "id": 2, "comment": "First reply", "createdAt": 1_100, "user": { "id": 13, "name": "nami" } }
                  ] },
                { "id": 5, "comment": "img(https://img.example/a.png)", "createdAt": 2_000,
                  "user": { "id": 14, "name": "sora", "avatar": { "medium": "" } },
                  "childComments": "[{\"id\": 6, \"comment\": \"nice\", \"createdAt\": 2100, \"user\": {\"id\": 11, \"name\": \"kumo\"}}]" },
                { "id": 7, "comment": "gone", "createdAt": 3_000, "user": null, "childComments": null }
            ]
        }})
    }

    #[test]
    fn a_page_of_comments_decodes_with_its_replies_oldest_first() {
        let page = decode_comment_page(&page());
        assert!(page.has_next);
        let ids: Vec<u32> = page.items.iter().map(|c| c.id).collect();
        assert_eq!(ids, [1, 5], "a comment with no author is left out");
        let first = &page.items[0];
        assert_eq!((first.author.as_str(), first.author_id), ("kumo", 11));
        assert_eq!(first.avatar.as_deref(), Some("https://img.example/kumo.png"));
        assert_eq!((first.like_count, first.liked, first.created_at), (4, true, 1_000));
        let replies: Vec<u32> = first.replies.iter().map(|c| c.id).collect();
        assert_eq!(replies, [2, 3], "oldest first");
        assert_eq!(first.replies[1].replies[0].id, 4, "deeper replies stay under theirs");
        assert_eq!(first.replies[1].avatar.as_deref(), Some("https://img.example/hoshi.png"));
        assert_eq!(first.count(), 4);
        let second = &page.items[1];
        assert_eq!(second.avatar, None, "an empty avatar is none");
        assert_eq!(second.replies.len(), 1, "a tree sent as a string is read too");
    }
}
