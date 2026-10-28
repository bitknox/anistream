//! AniList's forum threads, as the episode discussions read and write them.
//!
//! A test makes up the threads it needs with [`FakeAniList::add_thread`]; there are none in
//! the fixtures. Comments are kept in AniList's own wire shape, replies nested under
//! `childComments`, so what the fake answers is what the client decodes. Posting, editing,
//! deleting and liking change the fake's copy, as they would AniList's, so a test can read a
//! thread back after writing to it.

use serde_json::{Value, json};

use super::{AnilistId, FakeAniList, MAX_PER_PAGE, Operation, Reply, State, fixtures, lock};

/// One made-up thread.
#[derive(Debug, Clone)]
pub(super) struct FakeThread {
    id: u32,
    media: u32,
    title: String,
    comments: Vec<Value>,
}

/// Where comments the fake makes up are numbered from, clear of any a test writes.
const FIRST_POSTED: u64 = 700_000;

/// One comment in AniList's wire shape, for [`FakeAniList::add_thread`]: by a made-up user,
/// written `created_at`, with `replies` under it in the same shape.
pub fn comment(
    id: u32,
    author: &str,
    text: &str,
    created_at: i64,
    replies: Vec<Value>,
) -> Value {
    json!({
        "id": id,
        "comment": text,
        "likeCount": 0,
        "isLiked": false,
        "createdAt": created_at,
        "user": {
            "id": 800_000 + u64::from(id),
            "name": author,
            "avatar": { "medium": format!("{}avatar/{author}.png", fixtures::ART_HOST) },
        },
        "childComments": replies,
    })
}

impl FakeAniList {
    /// Open a thread in the anime forum of `media`, titled `title`, holding `comments` made
    /// with [`comment`]. Its id is returned.
    pub fn add_thread(&self, media: AnilistId, title: &str, comments: Vec<Value>) -> u32 {
        let mut state = lock(&self.state);
        let id = 60_000 + state.threads.len() as u32;
        state.threads.push(FakeThread {
            id,
            media: media.get(),
            title: title.to_owned(),
            comments,
        });
        id
    }

    /// The top-level comments of thread `id` as the fake now holds them, replies nested.
    pub fn thread_comments(&self, id: u32) -> Vec<Value> {
        lock(&self.state)
            .threads
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.comments.clone())
            .unwrap_or_default()
    }
}

/// Answer one of the thread operations, or `None` for any other.
pub(super) fn respond(
    state: &mut State,
    operation: Operation,
    vars: &Value,
    now: i64,
) -> Option<Reply> {
    Some(match operation {
        Operation::EpisodeThreads => {
            let media = vars["media"].as_u64().unwrap_or_default();
            let words = words(vars["search"].as_str().unwrap_or_default());
            let mut found: Vec<&FakeThread> = state
                .threads
                .iter()
                .filter(|t| u64::from(t.media) == media)
                // As loosely as AniList: every word somewhere in the title.
                .filter(|t| {
                    let title = t.title.to_lowercase();
                    words.iter().all(|w| title.contains(w.as_str()))
                })
                .collect();
            found.sort_by_key(|t| std::cmp::Reverse(replies(&t.comments)));
            let threads: Vec<Value> = found
                .into_iter()
                .take(10)
                .map(|t| {
                    json!({
                        "id": t.id,
                        "title": t.title,
                        "replyCount": replies(&t.comments),
                        "siteUrl": format!("https://anilist.co/forum/thread/{}", t.id),
                    })
                })
                .collect();
            Reply::ok(json!({ "Page": { "threads": threads } }))
        }
        Operation::ThreadComments => {
            let id = vars["thread"].as_u64().unwrap_or_default();
            let per = (vars["perPage"].as_u64().unwrap_or(25) as usize).clamp(1, MAX_PER_PAGE);
            let page = vars["page"].as_u64().unwrap_or(1).max(1) as usize;
            let all = state
                .threads
                .iter()
                .find(|t| u64::from(t.id) == id)
                .map(|t| t.comments.as_slice())
                .unwrap_or_default();
            let start = (page - 1) * per;
            let items: Vec<Value> = all.iter().skip(start).take(per).cloned().collect();
            Reply::ok(json!({ "Page": {
                "pageInfo": { "hasNextPage": all.len() > start + per },
                "threadComments": items,
            } }))
        }
        Operation::SaveThreadComment => {
            let text = vars["comment"].as_str().unwrap_or_default().to_owned();
            if let Some(id) = vars["id"].as_u64() {
                let Some(found) =
                    state.threads.iter_mut().find_map(|t| find(&mut t.comments, id))
                else {
                    return Some(Reply::errors(
                        404,
                        "Not Found.",
                        json!({ "SaveThreadComment": null }),
                    ));
                };
                found["comment"] = json!(text);
                let mut answered = found.clone();
                answered.as_object_mut().map(|o| o.remove("childComments"));
                return Some(Reply::ok(json!({ "SaveThreadComment": answered })));
            }
            let thread = vars["thread"].as_u64().unwrap_or_default();
            let Some(index) = state.threads.iter().position(|t| u64::from(t.id) == thread)
            else {
                return Some(Reply::errors(
                    404,
                    "Not Found.",
                    json!({ "SaveThreadComment": null }),
                ));
            };
            state.posted += 1;
            let id = FIRST_POSTED + state.posted;
            let mut made = json!({
                "id": id,
                "comment": text,
                "likeCount": 0,
                "isLiked": false,
                "createdAt": now,
                "user": {
                    "id": fixtures::VIEWER_ID,
                    "name": "fixture-viewer",
                    "avatar": { "medium": format!("{}avatar/fixture-viewer.png", fixtures::ART_HOST) },
                },
                "childComments": [],
            });
            let comments = &mut state.threads[index].comments;
            match vars["parent"].as_u64() {
                Some(parent) => match find(comments, parent) {
                    Some(parent) => children(parent).push(made.clone()),
                    None => {
                        return Some(Reply::errors(
                            404,
                            "Not Found.",
                            json!({ "SaveThreadComment": null }),
                        ));
                    }
                },
                None => comments.push(made.clone()),
            }
            made.as_object_mut().map(|o| o.remove("childComments"));
            Reply::ok(json!({ "SaveThreadComment": made }))
        }
        Operation::DeleteThreadComment => {
            let id = vars["id"].as_u64().unwrap_or_default();
            let deleted = state.threads.iter_mut().any(|t| remove(&mut t.comments, id));
            Reply::ok(json!({ "DeleteThreadComment": { "deleted": deleted } }))
        }
        Operation::ToggleCommentLike => {
            let id = vars["id"].as_u64().unwrap_or_default();
            let Some(found) = state.threads.iter_mut().find_map(|t| find(&mut t.comments, id))
            else {
                return Some(Reply::errors(404, "Not Found.", json!({ "ToggleLikeV2": null })));
            };
            let liked = !found["isLiked"].as_bool().unwrap_or(false);
            let count = found["likeCount"].as_i64().unwrap_or(0) + if liked { 1 } else { -1 };
            found["isLiked"] = json!(liked);
            found["likeCount"] = json!(count.max(0));
            Reply::ok(json!({ "ToggleLikeV2": {
                "id": id, "likeCount": found["likeCount"], "isLiked": liked,
            } }))
        }
        _ => return None,
    })
}

fn words(search: &str) -> Vec<String> {
    search.split_whitespace().map(str::to_lowercase).collect()
}

/// Every comment in a thread, replies included, as AniList counts `replyCount`.
fn replies(comments: &[Value]) -> usize {
    comments.iter().map(|c| 1 + c["childComments"].as_array().map_or(0, |r| replies(r))).sum()
}

fn children(comment: &mut Value) -> &mut Vec<Value> {
    if !comment["childComments"].is_array() {
        comment["childComments"] = json!([]);
    }
    comment["childComments"].as_array_mut().expect("just made an array")
}

fn find(comments: &mut [Value], id: u64) -> Option<&mut Value> {
    for comment in comments {
        if comment["id"].as_u64() == Some(id) {
            return Some(comment);
        }
        if let Some(found) = comment["childComments"].as_array_mut().and_then(|r| find(r, id)) {
            return Some(found);
        }
    }
    None
}

fn remove(comments: &mut Vec<Value>, id: u64) -> bool {
    if let Some(at) = comments.iter().position(|c| c["id"].as_u64() == Some(id)) {
        comments.remove(at);
        return true;
    }
    comments
        .iter_mut()
        .any(|c| c["childComments"].as_array_mut().is_some_and(|r| remove(r, id)))
}
