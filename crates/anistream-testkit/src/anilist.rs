//! A stand-in for AniList's GraphQL endpoint, on loopback.
//!
//! The documents in `anistream_meta::anilist::query` carry no operation names, so a request
//! is told apart by the part of its document only that query has — its root field and the
//! argument that sets it apart — and answered from [`crate::fixtures`]. Answers are composed
//! per request rather than replayed whole: a search filters by its term, a page by its
//! `page` and `perPage`, a calendar window by its bounds. That is what lets one small
//! catalogue answer every screen without two screens contradicting each other.
//!
//! Beside it sits an empty Tenrai, under [`FakeAniList::tenrai_endpoint`]: every list it is
//! asked for is empty and every title is not found. It is there so the fallback can be switched
//! on, as the default config has it, without any request leaving the machine.
//!
//! Asked to, it also serves the fixture titles' art ([`FakeAniList::serve_art`]), and points
//! every image URL it answers with at itself.
//!
//! HTTP is spoken by hand, as the loopback servers in the app's own tests do: one request per
//! connection, answered with `connection: close` so the client never reuses a socket this side
//! has finished with. That is all reqwest needs, and it keeps a server framework out of the
//! dependency tree.

use std::{
    collections::{HashMap, VecDeque},
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, MutexGuard},
};

use anistream_core::ids::AnilistId;
use anistream_meta::{AniList, Meta, Tenrai};
use anistream_net::HttpClient;
use anistream_store::Store;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedReadHalf},
    task::JoinHandle,
};

use crate::fixtures::{self, FIXTURE_NOW};

/// A request body larger than this is not one the app sends, and is not read.
const MAX_BODY: usize = 1 << 20;

/// AniList's own cap on `perPage`.
const MAX_PER_PAGE: usize = 50;

/// Which of the app's queries a request was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    /// `query::search` or `query::search_filtered`: a text search, or a browse with no term.
    Search,
    /// `query::by_id`: one title, with the detail-only fields.
    Detail,
    /// `query::by_ids`: several titles at once, for the Continue rail.
    Many,
    /// `query::relations`: several titles' relations, two levels deep, for the season walk.
    Relations,
    /// `query::seasonal`.
    Seasonal,
    /// `query::airing_schedule`: broadcasts in a window, for the Calendar.
    Schedule,
    /// `query::LAST_AIRED`.
    LastAired,
    /// `query::AIRED_SINCE`: what aired for the titles followed, for the new-episode check.
    AiredSince,
    /// `query::user_library`.
    Library,
    /// `query::VIEWER`.
    Viewer,
    /// `query::SAVE_PROGRESS`.
    SaveEntry,
    /// `query::DELETE_ENTRY`.
    DeleteEntry,
    /// A request to the empty Tenrai.
    Tenrai,
    /// Anything else. Answered with an error, and recorded, so a test can catch a query the
    /// fake was never taught rather than reading its failure as the app's.
    Unknown,
}

impl Operation {
    /// Tell a GraphQL document apart by what only it contains.
    ///
    /// Mutations and the authenticated queries are checked first: their documents nest the
    /// media fields too, so the media checks would claim them. The single title comes before
    /// the schedules, because it asks for its own broadcasts beside the title.
    pub fn of(document: &str) -> Self {
        let document = document.split_whitespace().collect::<Vec<_>>().join(" ");
        let has = |needle: &str| document.contains(needle);
        if has("DeleteMediaListEntry") {
            Self::DeleteEntry
        } else if has("SaveMediaListEntry") {
            Self::SaveEntry
        } else if has("MediaListCollection") {
            Self::Library
        } else if has("Viewer") {
            Self::Viewer
        } else if has("Media(id:") {
            Self::Detail
        } else if has("airingSchedules(mediaId_in: $ids, airingAt_greater") {
            Self::AiredSince
        } else if has("airingSchedules(mediaId_in") {
            Self::LastAired
        } else if has("airingSchedules(") {
            Self::Schedule
        } else if has("media(id_in:") && has("relations {") {
            Self::Relations
        } else if has("media(id_in:") {
            Self::Many
        } else if has("media(season:") {
            Self::Seasonal
        } else if has("media(search:") {
            Self::Search
        } else {
            Self::Unknown
        }
    }
}

/// One request the fake received.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub operation: Operation,
    /// `/graphql`, or the Tenrai path with its query string.
    pub path: String,
    /// The GraphQL document; empty for a Tenrai request.
    pub query: String,
    /// The GraphQL variables; `Null` for a Tenrai request.
    pub variables: Value,
    /// The `authorization` header, when one was sent.
    pub authorization: Option<String>,
}

/// A failure the fake answers with instead of the fixtures, for the failure paths.
///
/// Applies to GraphQL requests only: the empty Tenrai keeps answering, so a test of the
/// fallback ladder can fail AniList and watch the rungs below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// `429` with `retry-after` and `x-ratelimit-remaining: 0`, as AniList answers a spent
    /// budget. The client holds every later request for that many seconds, so `0` is the
    /// value for a test that means to go on asking.
    RateLimited { retry_after_secs: u64 },
    /// This HTTP status, with an `errors` body naming it.
    Status(u16),
    /// `200` with an `errors` array: GraphQL's own failure shape, and AniList's usual one.
    GraphQl(String),
    /// Read the request and hang up without a word: what the client reads as the network.
    Disconnect,
}

/// AniList's GraphQL endpoint on `127.0.0.1`, answering from the fixtures.
///
/// Stops accepting when dropped.
pub struct FakeAniList {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    accept: JoinHandle<()>,
}

struct State {
    requests: Vec<Request>,
    next: VecDeque<Failure>,
    always: Option<Failure>,
    now: i64,
    /// How far the clock has moved on past `now` without the broadcasts moving with it: see
    /// [`FakeAniList::advance`].
    ahead: i64,
    /// Titles answered in other forms than the fixtures', by id: see [`FakeAniList::set_title`].
    titles: HashMap<u64, Value>,
    /// The viewer's own title language, in place of the fixture's.
    viewer_language: Option<String>,
    /// Titles beside the fixtures, by id, which a test made up: see [`FakeAniList::add_titles`].
    extra: HashMap<u64, Value>,
    /// How long each operation's answers are held back: see [`FakeAniList::answer_slowly`].
    slow: HashMap<Operation, std::time::Duration>,
    /// Where image URLs point once the fake serves art: its own `/art/`.
    art: Option<String>,
    /// How the art is served when it is served badly: see [`FakeAniList::trouble_art`].
    art_trouble: ArtTrouble,
    /// How often each image has been asked for, and in which order they first were.
    art_asked: HashMap<String, (usize, u32)>,
}

/// Art served badly, as a busy CDN serves it.
#[derive(Debug, Clone, Copy, Default)]
struct ArtTrouble {
    /// How many times every second image is answered 503 before it is served.
    fail_first: u32,
    /// How long every image's answer is held back.
    delay: std::time::Duration,
}

impl FakeAniList {
    /// Bind a free port on loopback and start answering.
    pub async fn start() -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("the fake AniList binds a loopback port");
        let addr = listener.local_addr().expect("a bound listener has an address");
        let state = Arc::new(Mutex::new(State {
            requests: Vec::new(),
            next: VecDeque::new(),
            always: None,
            now: FIXTURE_NOW,
            ahead: 0,
            titles: HashMap::new(),
            viewer_language: None,
            extra: HashMap::new(),
            slow: HashMap::new(),
            art: None,
            art_trouble: ArtTrouble::default(),
            art_asked: HashMap::new(),
        }));
        let accept = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                while let Ok((socket, _)) = listener.accept().await {
                    tokio::spawn(serve(socket, Arc::clone(&state)));
                }
            }
        });
        Self { addr, state, accept }
    }

    /// The GraphQL endpoint, for [`AniList::with_endpoint`].
    pub fn endpoint(&self) -> String {
        format!("http://{}/graphql", self.addr)
    }

    /// The empty Tenrai's base, for [`Tenrai::with_endpoint`].
    pub fn tenrai_endpoint(&self) -> String {
        format!("http://{}/tenrai/v1", self.addr)
    }

    /// An AniList client that talks to this server, with [`crate::TEST_RATE_LIMIT`].
    pub fn anilist(&self, http: &HttpClient) -> AniList {
        AniList::new(http.clone(), crate::TEST_RATE_LIMIT).with_endpoint(self.endpoint())
    }

    /// A Tenrai client that talks to the empty one here.
    pub fn tenrai(&self, http: &HttpClient, store: &Store) -> Tenrai {
        Tenrai::new(http.clone(), store.clone()).with_endpoint(self.tenrai_endpoint())
    }

    /// A [`Meta`] over this server and `store`, with the fallback off.
    pub fn meta(&self, store: &Store) -> Meta {
        Meta::new(self.anilist(&crate::http()), None, store.clone())
    }

    /// A [`Meta`] over this server and `store`, with the fallback on and empty.
    pub fn meta_with_tenrai(&self, store: &Store) -> Meta {
        let http = crate::http();
        Meta::new(self.anilist(&http), Some(self.tenrai(&http, store)), store.clone())
    }

    /// Every request so far, in arrival order.
    pub fn requests(&self) -> Vec<Request> {
        lock(&self.state).requests.clone()
    }

    /// The requests for one operation, in arrival order.
    pub fn requests_for(&self, operation: Operation) -> Vec<Request> {
        lock(&self.state)
            .requests
            .iter()
            .filter(|r| r.operation == operation)
            .cloned()
            .collect()
    }

    pub fn clear_requests(&self) {
        lock(&self.state).requests.clear();
    }

    /// Answer the next GraphQL request with `failure`. Calls queue, one request each.
    pub fn fail_next(&self, failure: Failure) {
        lock(&self.state).next.push_back(failure);
    }

    /// Answer every GraphQL request with `failure` until [`Self::heal`]. Queued
    /// [`Self::fail_next`] failures still come first.
    pub fn fail_always(&self, failure: Failure) {
        lock(&self.state).always = Some(failure);
    }

    /// Drop every queued and standing failure.
    pub fn heal(&self) {
        let mut state = lock(&self.state);
        state.next.clear();
        state.always = None;
    }

    /// Serve the fixture titles' art ([`crate::art`]) under `/art/`, and point every image URL
    /// in an answer there instead of at [`crate::fixtures::ART_HOST`], which nothing resolves:
    /// for a front end that shows covers, banners and stills. Off until asked, so the image
    /// URLs a test sees are the fixtures' own.
    pub fn serve_art(&self) {
        lock(&self.state).art = Some(self.art_base());
    }

    /// Serve the art badly from now on, as a busy CDN would: every second image asked for (the
    /// first, the third, and so on, in the order they are first asked for) is answered
    /// `503 Service Unavailable` its first `fail_first` times, and every answer is held back by
    /// `delay`. For a front end's test that art on screen still arrives.
    pub fn trouble_art(&self, fail_first: u32, delay: std::time::Duration) {
        let mut state = lock(&self.state);
        state.art_trouble = ArtTrouble { fail_first, delay };
        state.art_asked.clear();
    }

    /// Where [`Self::serve_art`] serves the art from, standing in for
    /// [`crate::fixtures::ART_HOST`]: for a source's episode stills to point at too.
    pub fn art_base(&self) -> String {
        format!("http://{}/art/", self.addr)
    }

    /// Answer as if it were `now`, rather than [`FIXTURE_NOW`].
    ///
    /// Every broadcast moves by the same amount, so the schedule keeps its shape around the new
    /// clock: the next episode is still 2d 3h 15m away. For a test that runs against the real
    /// clock, which the calendar's window and the countdowns are taken from.
    pub fn set_now(&self, now: i64) {
        let mut state = lock(&self.state);
        state.now = now;
        state.ahead = 0;
    }

    /// Let `secs` pass: the clock moves on and the broadcasts stay where they are, so an
    /// episode due in an hour has aired once an hour has passed. For a test of what airs while
    /// the app is away, with the engine's own clock moved on as far.
    pub fn advance(&self, secs: i64) {
        lock(&self.state).ahead += secs;
    }

    /// Answer `id`'s title in these forms from now on, wherever the fake names it — a list, its
    /// page, a relation's card — for a test of a title that is missing one.
    pub fn set_title(
        &self,
        id: AnilistId,
        romaji: Option<&str>,
        english: Option<&str>,
        native: Option<&str>,
    ) {
        let title = json!({ "romaji": romaji, "english": english, "native": native });
        lock(&self.state).titles.insert(u64::from(id.get()), title);
    }

    /// The signed-in viewer reads AniList in `language` from now on: `ROMAJI`, `ENGLISH`,
    /// `NATIVE` or a stylised form, as AniList names them.
    pub fn set_viewer_title_language(&self, language: &str) {
        lock(&self.state).viewer_language = Some(language.to_owned());
    }

    /// Hold every answer to `operation` back by `wait` from now on, as a slow AniList would: for
    /// a test of what the app shows while it waits.
    pub fn answer_slowly(&self, operation: Operation, wait: std::time::Duration) {
        lock(&self.state).slow.insert(operation, wait);
    }

    /// Answer these titles too, each in the shape of a fixture under `fixtures/anilist/media/`,
    /// wherever a title page or the season walk asks after one: titles a test made up beside
    /// the fixtures, such as [`fixtures::franchise`]. No list, search or schedule has them.
    pub fn add_titles(&self, titles: impl IntoIterator<Item = Value>) {
        let mut state = lock(&self.state);
        for title in titles {
            let id = title["id"].as_u64().expect("a title has an id");
            state.extra.insert(id, title);
        }
    }
}

impl Drop for FakeAniList {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

/// A poisoned lock means a test already panicked; the state is still worth reading.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

/// What one connection asked for.
struct Head {
    method: String,
    path: String,
    length: usize,
    authorization: Option<String>,
}

enum Reply {
    Answer {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Value,
    },
    /// An image, when the fake serves art.
    Image(&'static [u8]),
    HangUp,
}

impl Reply {
    fn ok(data: Value) -> Self {
        Self::status(200, json!({ "data": data }))
    }

    fn status(status: u16, body: Value) -> Self {
        Self::Answer { status, headers: Vec::new(), body }
    }

    /// GraphQL's error shape: an `errors` array, with `data` as the query left it.
    fn errors(status: u16, message: &str, data: Value) -> Self {
        Self::status(
            status,
            json!({ "errors": [{ "message": message, "status": status }], "data": data }),
        )
    }
}

async fn serve(socket: TcpStream, state: Arc<Mutex<State>>) {
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    let Some(head) = read_head(&mut reader).await else { return };
    let mut body = vec![0; head.length.min(MAX_BODY)];
    if reader.read_exact(&mut body).await.is_err() {
        return;
    }
    let reply = answer(&state, &head, &body);
    // Held back as a slow AniList would be, once the request is on the record.
    let payload: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let operation = Operation::of(payload["query"].as_str().unwrap_or_default());
    let slow = if head.path.starts_with("/art/") {
        Some(lock(&state).art_trouble.delay).filter(|wait| !wait.is_zero())
    } else {
        lock(&state).slow.get(&operation).copied()
    };
    if let Some(wait) = slow {
        tokio::time::sleep(wait).await;
    }
    let (status, headers, content_type, body) = match reply {
        Reply::Answer { status, headers, body } => {
            let mut body = body.to_string();
            let art = lock(&state).art.clone();
            if let Some(art) = art {
                body = body.replace(crate::fixtures::ART_HOST, &art);
            }
            (status, headers, "application/json", body.into_bytes())
        }
        Reply::Image(bytes) => (200, Vec::new(), "image/png", bytes.to_vec()),
        // A hang-up is the socket dropped here, unanswered.
        Reply::HangUp => return,
    };

    let mut out = format!(
        "HTTP/1.1 {status} {}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\
         connection: close\r\n",
        reason(status),
        body.len()
    );
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    let mut out = out.into_bytes();
    out.extend_from_slice(&body);
    let _ = write.write_all(&out).await;
    let _ = write.shutdown().await;
}

async fn read_head(reader: &mut BufReader<OwnedReadHalf>) -> Option<Head> {
    let mut line = String::new();
    reader.read_line(&mut line).await.ok().filter(|n| *n > 0)?;
    let mut parts = line.split_whitespace();
    let mut head = Head {
        method: parts.next()?.to_owned(),
        path: parts.next()?.to_owned(),
        length: 0,
        authorization: None,
    };
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            return Some(head);
        }
        let Some((name, value)) = header.split_once(':') else { continue };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            head.length = value.parse().unwrap_or(0);
        } else if name.eq_ignore_ascii_case("authorization") {
            head.authorization = Some(value.to_owned());
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Record the request, then answer it: with a failure if one is due, else from the fixtures.
/// Art, when the fake serves it, is answered without being recorded or failed, as it is not
/// AniList's, unless [`FakeAniList::trouble_art`] asked for it to be served badly.
fn answer(state: &Mutex<State>, head: &Head, body: &[u8]) -> Reply {
    if let Some(path) = head.path.strip_prefix("/art/") {
        let mut guard = lock(state);
        let state = &mut *guard;
        let serving = state.art.is_some();
        let first_asked = state.art_asked.len();
        let (order, times) = state.art_asked.entry(path.to_owned()).or_insert((first_asked, 0));
        *times += 1;
        if serving && *order % 2 == 0 && *times <= state.art_trouble.fail_first {
            return Reply::status(503, json!({ "message": "busy" }));
        }
        drop(guard);
        return match crate::art::png(path).filter(|_| serving && head.method == "GET") {
            Some(bytes) => Reply::Image(bytes),
            None => Reply::status(404, json!({ "message": "no such image" })),
        };
    }
    if let Some(rest) = head.path.strip_prefix("/tenrai/v1") {
        lock(state).requests.push(Request {
            operation: Operation::Tenrai,
            path: head.path.clone(),
            query: String::new(),
            variables: Value::Null,
            authorization: head.authorization.clone(),
        });
        return tenrai(rest);
    }

    let payload: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let query = payload["query"].as_str().unwrap_or_default().to_owned();
    let variables = payload.get("variables").cloned().unwrap_or(Value::Null);
    let operation = if head.method == "POST" && head.path == "/graphql" {
        Operation::of(&query)
    } else {
        Operation::Unknown
    };

    let (failure, (now, ahead), titles, viewer_language, extra) = {
        let mut state = lock(state);
        state.requests.push(Request {
            operation,
            path: head.path.clone(),
            query,
            variables: variables.clone(),
            authorization: head.authorization.clone(),
        });
        let failure = state.next.pop_front().or_else(|| state.always.clone());
        let clock = (state.now, state.ahead);
        let extra = state.extra.clone();
        (failure, clock, state.titles.clone(), state.viewer_language.clone(), extra)
    };
    match failure {
        Some(failure) => fail(failure),
        None => {
            let clock = Clock { shift: now - FIXTURE_NOW, now: now + ahead };
            let catalogue = Catalogue { extra: &extra };
            let mut reply =
                respond(operation, &variables, head.authorization.is_some(), clock, catalogue);
            if let Reply::Answer { body, .. } = &mut reply {
                retitle(body, &titles);
                if let Some(language) = viewer_language
                    && let Some(viewer) =
                        body.pointer_mut("/data/Viewer").filter(|v| v.is_object())
                {
                    viewer["options"] = json!({ "titleLanguage": language });
                }
            }
            reply
        }
    }
}

/// Give every title the test set another way its forms, wherever it is in `value`.
fn retitle(value: &mut Value, titles: &HashMap<u64, Value>) {
    if titles.is_empty() {
        return;
    }
    match value {
        Value::Object(fields) => {
            let id = fields.get("id").and_then(Value::as_u64);
            if let Some(title) = id.and_then(|id| titles.get(&id))
                && fields.contains_key("title")
            {
                fields.insert("title".into(), title.clone());
            }
            fields.values_mut().for_each(|field| retitle(field, titles));
        }
        Value::Array(items) => items.iter_mut().for_each(|item| retitle(item, titles)),
        _ => {}
    }
}

fn fail(failure: Failure) -> Reply {
    match failure {
        Failure::RateLimited { retry_after_secs } => Reply::Answer {
            status: 429,
            headers: vec![
                ("retry-after", retry_after_secs.to_string()),
                ("x-ratelimit-limit", "30".into()),
                ("x-ratelimit-remaining", "0".into()),
            ],
            body: json!({
                "errors": [{ "message": "Too Many Requests.", "status": 429 }],
                "data": null,
            }),
        },
        Failure::Status(status) => {
            Reply::errors(status, &format!("the fake AniList answered {status}"), Value::Null)
        }
        Failure::GraphQl(message) => Reply::errors(200, &message, Value::Null),
        Failure::Disconnect => Reply::HangUp,
    }
}

/// The empty Tenrai: no title is found, and every list is empty.
fn tenrai(path: &str) -> Reply {
    let path = path.split('?').next().unwrap_or_default();
    if path.starts_with("/anime/") && path.ends_with("/full") {
        return Reply::status(
            404,
            json!({ "status": 404, "message": "Resource does not exist" }),
        );
    }
    Reply::status(200, json!({ "data": [], "pagination": { "has_next_page": false } }))
}

/// Every title the fake can name by id: the fixtures, and those a test added.
#[derive(Clone, Copy)]
struct Catalogue<'a> {
    extra: &'a HashMap<u64, Value>,
}

impl Catalogue<'_> {
    /// `id` as the single-title query answers it.
    fn get(self, id: u64) -> Option<Value> {
        let fixture = || fixtures::wire(AnilistId::new(id as u32)).cloned();
        self.extra.get(&id).cloned().or_else(fixture)
    }

    /// `id`'s own relations, as a related title carries them a level out: the kind, and the
    /// title by id and format. None for a title the fake does not have, as AniList would have
    /// some.
    fn neighbours(self, id: u64) -> Value {
        let edges: Vec<Value> = self
            .get(id)
            .and_then(|title| title["relations"]["edges"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .map(|edge| {
                json!({ "relationType": edge["relationType"],
                        "node": { "id": edge["node"]["id"], "format": edge["node"]["format"] } })
            })
            .collect();
        json!({ "edges": edges })
    }

    /// `media`'s relations with each related title's own, as the title page and the season walk
    /// ask for them.
    fn with_neighbours(self, mut media: Value) -> Value {
        if let Some(edges) = media.pointer_mut("/relations/edges").and_then(Value::as_array_mut)
        {
            for edge in edges {
                let id = edge["node"]["id"].as_u64().unwrap_or_default();
                edge["node"]["relations"] = self.neighbours(id);
            }
        }
        media
    }

    /// One title as the season walk is answered: its card, and its relations two levels deep.
    fn relations(self, id: u64) -> Option<Value> {
        let title = self.get(id)?;
        let mut card = json!({ "relations": title["relations"].clone() });
        for field in ["id", "title", "format", "season", "seasonYear"] {
            card[field] = title[field].clone();
        }
        card["coverImage"] = json!({ "large": title["coverImage"]["large"], "color": title["coverImage"]["color"] });
        Some(self.with_neighbours(card))
    }
}

fn respond(
    operation: Operation,
    vars: &Value,
    authorized: bool,
    clock: Clock,
    catalogue: Catalogue<'_>,
) -> Reply {
    let needs_token = matches!(
        operation,
        Operation::Library | Operation::Viewer | Operation::SaveEntry | Operation::DeleteEntry
    );
    // The client never sends these without a token; a request that does is a bug worth seeing.
    if needs_token && !authorized {
        return Reply::errors(401, "Invalid token", Value::Null);
    }
    match operation {
        Operation::Search => Reply::ok(media_page(search(vars), vars, clock)),
        Operation::Seasonal => {
            let found = fixtures::SEASONAL.iter().filter(|id| filtered(**id, vars, false));
            Reply::ok(media_page(found.copied().collect(), vars, clock))
        }
        Operation::Detail => {
            match vars["id"].as_u64().and_then(|id| catalogue.get(id)) {
                Some(media) => Reply::ok(detail(catalogue.with_neighbours(media), clock)),
                // What AniList answers for an id it has no title for: a 404 inside GraphQL's
                // error shape, which the client reads as an API failure rather than NotFound.
                None => Reply::errors(404, "Not Found.", json!({ "Media": null })),
            }
        }
        Operation::Many => {
            let asked = ids(&vars["ids"]);
            // In the catalogue's order, not the order asked: the client puts them back, and
            // answering in the asked order would let a client that forgot pass anyway.
            let media: Vec<Value> = fixtures::ALL
                .iter()
                .filter(|id| asked.contains(id))
                .filter_map(|id| fixtures::listed_wire(*id))
                .map(|m| clock.media(m))
                .collect();
            Reply::ok(json!({ "Page": { "media": media } }))
        }
        Operation::Relations => {
            let media: Vec<Value> = ids(&vars["ids"])
                .into_iter()
                .filter_map(|id| catalogue.relations(u64::from(id.get())))
                .collect();
            Reply::ok(json!({ "Page": { "media": media } }))
        }
        Operation::Schedule => Reply::ok(schedule(vars, clock)),
        Operation::LastAired => Reply::ok(last_aired(vars, clock)),
        Operation::AiredSince => Reply::ok(aired_since(vars, clock)),
        Operation::Viewer => Reply::ok(json!({ "Viewer": fixtures::library_wire()["viewer"] })),
        Operation::Library => Reply::ok(library(clock)),
        Operation::SaveEntry => Reply::ok(save_entry(vars)),
        Operation::DeleteEntry => {
            let known = entries().any(|e| e["id"] == vars["id"]);
            Reply::ok(json!({ "DeleteMediaListEntry": { "deleted": known } }))
        }
        Operation::Tenrai | Operation::Unknown => {
            Reply::errors(400, "the fake AniList was never taught this query", Value::Null)
        }
    }
}

/// The fake's clock: how far the fixtures move, and the moment countdowns are counted from,
/// which is that far on and as far again as [`FakeAniList::advance`] has moved it.
#[derive(Clone, Copy)]
struct Clock {
    shift: i64,
    now: i64,
}

impl Clock {
    fn at(self, fixture_time: &Value) -> i64 {
        fixture_time.as_i64().unwrap_or_default() + self.shift
    }

    /// A title with its next broadcast moved, and its countdown counted from now — AniList
    /// computes `timeUntilAiring` per request, so a stored one would be wrong a second later.
    /// Its schedule moves with it, so the two never disagree.
    fn media(self, mut media: Value) -> Value {
        if let Some(next) = media.get_mut("nextAiringEpisode").and_then(Value::as_object_mut) {
            let at = self.at(&next["airingAt"]);
            next.insert("airingAt".into(), at.into());
            next.insert("timeUntilAiring".into(), (at - self.now).into());
        }
        if let Some(nodes) = media.pointer_mut("/upcoming/nodes").and_then(Value::as_array_mut)
        {
            self.broadcasts(nodes);
        }
        if let Some(aired) = media.get_mut("aired").and_then(Value::as_array_mut) {
            self.broadcasts(aired);
        }
        media
    }

    /// Each broadcast's `airingAt`, moved.
    fn broadcasts(self, rows: &mut [Value]) {
        for row in rows {
            let at = self.at(&row["airingAt"]);
            row["airingAt"] = at.into();
        }
    }
}

/// The single-title answer, as AniList shapes it: the title, and beside it the broadcasts
/// already out, newest first, under the alias `query::by_id` gives them. The fixture keeps them
/// on the title, as the app's `Media` does once it has read them.
fn detail(media: Value, clock: Clock) -> Value {
    let mut media = clock.media(media);
    let mut aired = media
        .as_object_mut()
        .and_then(|fields| fields.remove("aired"))
        .and_then(|aired| aired.as_array().cloned())
        .unwrap_or_default();
    aired.sort_by_key(|row| std::cmp::Reverse(row["episode"].as_u64()));
    json!({ "Media": media, "aired": { "airingSchedules": aired } })
}

/// The titles a search finds: every one whose title or synonyms contain the term, any case,
/// that the filters allow. No term is a browse of the whole catalogue.
fn search(vars: &Value) -> Vec<AnilistId> {
    let term = vars["search"].as_str().unwrap_or_default().trim().to_lowercase();
    fixtures::ALL
        .iter()
        .copied()
        .filter(|id| {
            let media = fixtures::wire(*id).expect("ALL lists fixture titles");
            let names = ["romaji", "english", "native"]
                .iter()
                .filter_map(|form| media["title"][form].as_str())
                .chain(
                    media["synonyms"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str),
                );
            term.is_empty() || names.map(str::to_lowercase).any(|name| name.contains(&term))
        })
        .filter(|id| filtered(*id, vars, true))
        .collect()
}

/// Whether a title passes the server-side filters the app sends: genres (every one), format
/// and status, and for a search the year. The seasonal browse's season and year are not
/// filters here — see [`fixtures::SEASONAL`].
fn filtered(id: AnilistId, vars: &Value, with_year: bool) -> bool {
    let Some(media) = fixtures::wire(id) else { return false };
    let genres = media["genres"].as_array().cloned().unwrap_or_default();
    let mut wanted = vars["genres"].as_array().into_iter().flatten();
    wanted.all(|g| genres.contains(g))
        && ["format", "status"].iter().all(|f| vars[f].is_null() || vars[f] == media[f])
        && (!with_year
            || vars["seasonYear"].is_null()
            || vars["seasonYear"] == media["seasonYear"])
}

/// `page` and `perPage` applied to a list, as AniList pages: one-based, at most fifty.
fn paged<T>(items: Vec<T>, vars: &Value) -> (Vec<T>, bool) {
    let page = vars["page"].as_u64().unwrap_or(1).max(1) as usize;
    let per_page = (vars["perPage"].as_u64().unwrap_or(MAX_PER_PAGE as u64) as usize)
        .clamp(1, MAX_PER_PAGE);
    let more = items.len() > page * per_page;
    (items.into_iter().skip((page - 1) * per_page).take(per_page).collect(), more)
}

fn media_page(found: Vec<AnilistId>, vars: &Value, clock: Clock) -> Value {
    let (found, more) = paged(found, vars);
    let media: Vec<Value> =
        found.into_iter().filter_map(fixtures::listed_wire).map(|m| clock.media(m)).collect();
    json!({ "Page": { "pageInfo": { "hasNextPage": more }, "media": media } })
}

fn ids(value: &Value) -> Vec<AnilistId> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|id| AnilistId::new(id as u32))
        .collect()
}

/// Broadcasts strictly inside `from`..`to`, as `airingAt_greater` and `airingAt_lesser` have
/// it, in the order `sort` asks for.
fn schedule(vars: &Value, clock: Clock) -> Value {
    let from = vars["from"].as_i64().unwrap_or(i64::MIN);
    let to = vars["to"].as_i64().unwrap_or(i64::MAX);
    let newest_first = sort_is(vars, "TIME_DESC");
    let mut rows: Vec<(i64, &Value)> = fixtures::schedule_wire()
        .iter()
        .map(|row| (clock.at(&row["airingAt"]), row))
        .filter(|(at, _)| *at > from && *at < to)
        .collect();
    rows.sort_by_key(|(at, _)| if newest_first { -*at } else { *at });
    let (rows, more) = paged(rows, vars);
    let rows: Vec<Value> = rows
        .into_iter()
        .filter_map(|(at, row)| {
            let id = AnilistId::new(row["mediaId"].as_u64()? as u32);
            Some(json!({
                "episode": row["episode"],
                "airingAt": at,
                "media": clock.media(fixtures::listed_wire(id)?),
            }))
        })
        .collect();
    json!({ "Page": { "pageInfo": { "hasNextPage": more }, "airingSchedules": rows } })
}

/// The newest broadcasts already aired for these titles, newest first, `perPage` of them.
fn last_aired(vars: &Value, clock: Clock) -> Value {
    let asked = ids(&vars["ids"]);
    let mut rows: Vec<(i64, &Value)> = fixtures::schedule_wire()
        .iter()
        .filter(|row| {
            row["mediaId"].as_u64().is_some_and(|id| asked.contains(&AnilistId::new(id as u32)))
        })
        .map(|row| (clock.at(&row["airingAt"]), row))
        .filter(|(at, _)| *at <= clock.now)
        .collect();
    rows.sort_by_key(|(at, _)| -*at);
    let (rows, _) = paged(rows, vars);
    let rows: Vec<Value> = rows
        .into_iter()
        .map(|(at, row)| {
            json!({ "mediaId": row["mediaId"], "episode": row["episode"], "airingAt": at })
        })
        .collect();
    json!({ "Page": { "airingSchedules": rows } })
}

/// What aired for these titles strictly inside `from`..`to`, oldest first, a page of them: each
/// with its title's forms and cover.
fn aired_since(vars: &Value, clock: Clock) -> Value {
    let asked = ids(&vars["ids"]);
    let from = vars["from"].as_i64().unwrap_or(i64::MIN);
    let to = vars["to"].as_i64().unwrap_or(i64::MAX);
    let mut rows: Vec<(i64, &Value)> = fixtures::schedule_wire()
        .iter()
        .filter(|row| {
            row["mediaId"].as_u64().is_some_and(|id| asked.contains(&AnilistId::new(id as u32)))
        })
        .map(|row| (clock.at(&row["airingAt"]), row))
        .filter(|(at, _)| *at > from && *at < to)
        .collect();
    rows.sort_by_key(|(at, _)| *at);
    let (rows, more) = paged(rows, vars);
    let rows: Vec<Value> = rows
        .into_iter()
        .filter_map(|(at, row)| {
            let id = AnilistId::new(row["mediaId"].as_u64()? as u32);
            let media = fixtures::wire(id)?;
            Some(json!({
                "episode": row["episode"],
                "airingAt": at,
                "media": {
                    "id": media["id"],
                    "title": media["title"],
                    "coverImage": media["coverImage"],
                    "bannerImage": media["bannerImage"],
                },
            }))
        })
        .collect();
    json!({ "Page": { "pageInfo": { "hasNextPage": more }, "airingSchedules": rows } })
}

fn sort_is(vars: &Value, wanted: &str) -> bool {
    match &vars["sort"] {
        Value::Array(sorts) => sorts.first().and_then(Value::as_str) == Some(wanted),
        sort => sort.as_str() == Some(wanted),
    }
}

fn entries() -> impl Iterator<Item = &'static Value> {
    fixtures::library_wire()["entries"].as_array().into_iter().flatten()
}

/// The viewer's list, one list per status as AniList groups it, whatever `userId` asked for.
fn library(clock: Clock) -> Value {
    let mut lists: Vec<(String, Vec<Value>)> = Vec::new();
    for entry in entries() {
        let id = AnilistId::new(entry["mediaId"].as_u64().unwrap_or_default() as u32);
        let Some(media) = fixtures::listed_wire(id) else { continue };
        let status = entry["status"].as_str().unwrap_or("CURRENT").to_owned();
        let row = json!({
            "id": entry["id"],
            "status": status,
            "progress": entry["progress"],
            "score": entry["score"],
            "updatedAt": entry["updatedAt"],
            "media": clock.media(media),
        });
        match lists.iter_mut().find(|(s, _)| *s == status) {
            Some((_, rows)) => rows.push(row),
            None => lists.push((status, vec![row])),
        }
    }
    let lists: Vec<Value> =
        lists.into_iter().map(|(_, rows)| json!({ "entries": rows })).collect();
    json!({ "MediaListCollection": { "lists": lists } })
}

/// The upsert's answer: the library entry's id, or for a title not on the list a stable
/// made-up one (its media id plus a million), with the fields as written.
///
/// Nothing is stored. The library answers the same afterwards, and a test asserts on what
/// was sent through [`FakeAniList::requests_for`].
fn save_entry(vars: &Value) -> Value {
    let media_id = vars["mediaId"].as_u64().unwrap_or_default();
    let existing = entries().find(|e| e["mediaId"].as_u64() == Some(media_id));
    let id = existing.map_or(json!(media_id + 1_000_000), |e| e["id"].clone());
    let field = |name: &str, otherwise: Value| {
        if vars[name].is_null() {
            existing.map_or(otherwise, |e| e[name].clone())
        } else {
            vars[name].clone()
        }
    };
    json!({ "SaveMediaListEntry": {
        "id": id,
        "progress": field("progress", json!(0)),
        "status": field("status", json!("CURRENT")),
    } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_meta::anilist::query;

    #[test]
    fn every_document_the_app_sends_is_one_the_fake_knows() {
        // The documents carry no operation names, so the fake tells them apart by content. A
        // change to a query that broke that would otherwise surface as a baffling test failure
        // somewhere far from here.
        for (document, operation) in [
            (query::search(), Operation::Search),
            (query::search_filtered(), Operation::Search),
            (query::by_id(), Operation::Detail),
            (query::by_ids(), Operation::Many),
            (query::relations(), Operation::Relations),
            (query::seasonal(), Operation::Seasonal),
            (query::airing_schedule(), Operation::Schedule),
            (query::LAST_AIRED.to_owned(), Operation::LastAired),
            (query::AIRED_SINCE.to_owned(), Operation::AiredSince),
            (query::user_library(), Operation::Library),
            (query::VIEWER.to_owned(), Operation::Viewer),
            (query::SAVE_PROGRESS.to_owned(), Operation::SaveEntry),
            (query::DELETE_ENTRY.to_owned(), Operation::DeleteEntry),
        ] {
            assert_eq!(Operation::of(&document), operation, "{document}");
        }
        assert_eq!(Operation::of("query { Studio(id: 1) { name } }"), Operation::Unknown);
    }

    #[test]
    fn a_shifted_clock_moves_broadcasts_and_keeps_countdowns_honest() {
        let clock = Clock { shift: 1_000, now: FIXTURE_NOW + 1_000 };
        let moved = clock.media(fixtures::wire(fixtures::AIRING).unwrap().clone());
        let next = &moved["nextAiringEpisode"];
        assert_eq!(next["airingAt"], 1_792_163_730 + 1_000);
        assert_eq!(next["timeUntilAiring"], 184_500, "the same distance away");
    }
}
