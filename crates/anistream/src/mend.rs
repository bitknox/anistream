//! A loopback proxy that repairs disguised media on its way to the player.
//!
//! **The problem.** Some hosts serve video segments dressed as something else. The case that
//! prompted this: every segment of an episode was a valid 1×1 PNG with the real MPEG-TS
//! appended after `IEND`, served from an image CDN — an image host will carry an image, so the
//! video is smuggled behind one. The site's own player strips the prefix in JavaScript before
//! handing bytes to MediaSource. mpv has no such hook: libavformat probes the first bytes, sees
//! PNG, decodes a 1×1 image and finds no video. The stream is not encrypted and there is no DRM
//! here — the bytes are simply mislabelled, and this puts the label right.
//!
//! **Why it is generic.** Nothing here knows about PNG, or about any particular source. It
//! searches the head of each segment for the first offset where a *container signature* appears
//! — MPEG-TS's repeating sync byte, or an ISO-BMFF box — and serves from there. A segment that
//! is already valid matches at offset zero and passes through byte-for-byte, so routing a
//! healthy stream through this changes nothing. Anything unrecognised is also passed through
//! untouched: the player is the better judge, and refusing to serve bytes we merely do not
//! recognise would break more than it fixed.
//!
//! **What it is not.** Not a rewriter of media, and not a way around encryption: an
//! `#EXT-X-KEY` is proxied verbatim so encrypted HLS keeps working exactly as it did, and
//! nothing attempts to decrypt anything.
//!
//! **Two players read through it.** The seek bar's thumbnailer is a second mpv opening the
//! same stream, and left to itself it fetched whole segments at every hover — twice, since
//! thumbfast seeks to a keyframe and then exactly — until the host answered `429` and the
//! rate-limit hold below parked the thumbnailer, and with it the thumbnail, for ten seconds at
//! a time. So the thumbnailer gets its own entry (see [`MendServer::thumbnail_url`]) and is
//! served by different rules: from what playback already fetched when it can be, never held,
//! and never the reason playback is held. See [`Class`].

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anistream_net::HttpClient;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

/// How much of a segment's head to search for a container signature.
///
/// The wrapper is a decoy meant to be cheap for the host to prepend, so it is small — the
/// observed PNG is 70 bytes. 64 KiB is far beyond any plausible prefix while keeping the
/// search bounded.
const SNIFF_LIMIT: usize = 64 * 1024;

/// MPEG-TS packets are exactly this long, and each begins with `0x47`.
const TS_PACKET: usize = 188;

/// Consecutive TS packets required before an offset is believed.
///
/// One `0x47` is a coincidence — it is a perfectly ordinary byte. Four in a row at exactly
/// 188-byte spacing is a transport stream.
const TS_CONFIRMATIONS: usize = 4;

/// The largest response body the proxy will hold in memory.
///
/// Segments are seconds of video; a playlist is text. Anything far past that is not something
/// this proxy should be buffering, so it is refused rather than swallowing the process.
const MAX_BODY: usize = 96 * 1024 * 1024;

/// How long one request may be held while the upstream asks us to slow down.
///
/// mpv reads ahead as fast as the network allows, and a Cloudflare-fronted segment host was
/// measured answering `429` with `retry-after: 10` after about a hundred segments. ffmpeg's HLS
/// demuxer does not wait — it drops a failed segment and opens the next, which fails too — so
/// relaying the refusal skipped every remaining segment in a few seconds, mpv's cache closed over
/// the hole, and the playhead jumped to the end: the episode "finished" minutes in. Holding the
/// request instead turns that into a slow segment. Kept well inside `--network-timeout=60`, so
/// mpv is still waiting when the answer arrives.
const RATE_LIMIT_BUDGET: Duration = Duration::from_secs(45);

/// How many bytes of fetched resources the proxy keeps for a second read.
///
/// mpv's default cache holds 150 MiB ahead of the playhead and 50 MiB behind it, and what it
/// holds is what the seek bar shows as loaded. Keeping as much here means a thumbnail over any
/// loaded part of the bar is answered from memory, with nothing sent upstream.
const CACHE_BYTES: usize = 200 * 1024 * 1024;

/// The largest single resource the cache will keep. Segments are a few megabytes; anything
/// this size is not a segment and would only push a hundred of them out.
const CACHE_ENTRY_LIMIT: usize = 32 * 1024 * 1024;

/// The longest a thumbnail request waits for anything: a fetch already under way, or its turn
/// to go upstream. The thumbnailer reads one segment at a time, so while it waits here it
/// cannot get to the newer position the pointer has moved on to.
const THUMBNAIL_WAIT: Duration = Duration::from_secs(3);

/// The least time between two thumbnail fetches from upstream.
///
/// Sweeping across the bar asks for a segment at every position the thumbnailer reaches. The
/// thumbnailer coalesces positions while it is busy, but against a fast upstream it would still
/// fetch several segments a second, every one of them counted by a host that rate-limits.
const THUMBNAIL_SPACING: Duration = Duration::from_millis(250);

/// How soon after a refused thumbnail request the next segment counts as ffmpeg skipping on.
///
/// ffmpeg's HLS demuxer answers a segment it cannot open by opening the next one at once, so a
/// refusal is followed within milliseconds by a request for its successor. See
/// [`Upstream::refused_successor`].
const SKIP_WINDOW: Duration = Duration::from_secs(1);

/// The wait when a `429` does not say how long, or says it in a form not worth parsing.
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(2);

/// Where the real payload starts in `data`, if it does not start at the beginning.
///
/// Returns `0` for anything already valid or simply unrecognised, which makes the caller's
/// slice a no-op — "pass it through" is the safe default in both cases.
///
/// **Not what `infer` or `file-format` do.** Those identify a buffer by its leading magic
/// bytes, and here the leading bytes are a *genuine* PNG — they would answer "this is an
/// image", correctly and uselessly. The question is where the media begins, which needs a
/// search rather than a lookup, and structural confirmation rather than a signature: a lone
/// `0x47` is the letter G, four of them 188 bytes apart is a transport stream.
pub fn payload_start(data: &[u8]) -> usize {
    // Text formats are recognised up front so a subtitle track is never mistaken for a
    // container with a prefix on it.
    if data.starts_with(b"WEBVTT") || data.starts_with(b"#EXTM3U") {
        return 0;
    }

    let limit = data.len().min(SNIFF_LIMIT);
    // Two passes, earliest match within each. The first insists a transport stream open the way
    // a segment does, which is what stops a stray sync byte inside the wrapper from claiming an
    // offset 188 bytes early. The second accepts any structurally valid packet, so a source that
    // starts mid-stream is still mended rather than refused.
    for offset in 0..limit {
        if is_iso_bmff(data, offset) || is_transport_stream(data, offset, true) {
            return offset;
        }
    }
    for offset in 0..limit {
        if is_transport_stream(data, offset, false) {
            return offset;
        }
    }
    0
}

/// Whether a transport stream starts at `offset`: `0x47` every 188 bytes, several times over.
fn is_transport_stream(data: &[u8], offset: usize, strict: bool) -> bool {
    if data.get(offset) != Some(&0x47) {
        return false;
    }
    // The tail of a segment is not a place to start looking: a lone `0x47` near the end could
    // not be confirmed, and confirming is the whole point.
    let cadence = (1..=TS_CONFIRMATIONS).all(|packet| {
        let at = offset + packet * TS_PACKET;
        // Missing bytes count as confirmation only when the data genuinely ended, so a short
        // final segment still validates.
        at >= data.len() || data.get(at) == Some(&0x47)
    }) && offset + TS_PACKET < data.len();
    cadence && is_packet_header(data, offset, strict)
}

/// Whether the four bytes at `offset` are a plausible TS packet header.
///
/// **The cadence alone cannot be trusted, and the arithmetic says why.** A wrapper of length `w`
/// puts the real payload at `w`; a stray `0x47` at `w - 188` then has *genuine* packet
/// boundaries at every position the cadence check probes, so it passes on the strength of the
/// real stream that follows it. One byte in 256 — a coin flip across a full episode — and the
/// result is a segment served with 188 bytes of wrapper still attached, which a decoder
/// recovers from by dropping whatever it cannot parse. That is a cut and a slip in sync, not a
/// clean failure, so it has to be excluded structurally rather than noticed.
///
/// `strict` additionally demands the packet look like the *first* of a segment: HLS transport
/// streams open on a PAT (PID 0) or the SDT beside it, with the payload-unit-start flag set.
/// Wrapper bytes clear all four tests about once in 65,000 rather than once in 256.
fn is_packet_header(data: &[u8], offset: usize, strict: bool) -> bool {
    let Some(header) = data.get(offset..offset + 4) else {
        return false;
    };
    // A set transport-error bit means the sender itself marked the packet corrupt, and an
    // adaptation-field control of zero is reserved — neither belongs at a payload's start.
    if header[1] & 0x80 != 0 || (header[3] >> 4) & 0b11 == 0 {
        return false;
    }
    if !strict {
        return true;
    }
    let pid = (u16::from(header[1] & 0x1F) << 8) | u16::from(header[2]);
    header[1] & 0x40 != 0 && pid <= 0x1F
}

/// Whether an ISO base media file box starts at `offset`.
///
/// fMP4 segments begin with one of a small set of box types. The size field is checked for
/// plausibility as well, because four ASCII letters alone appear inside compressed video often
/// enough to matter.
fn is_iso_bmff(data: &[u8], offset: usize) -> bool {
    let Some(header) = data.get(offset..offset + 8) else {
        return false;
    };
    let kind = &header[4..8];
    let known: [&[u8]; 6] = [b"ftyp", b"styp", b"moof", b"sidx", b"moov", b"emsg"];
    if !known.contains(&kind) {
        return false;
    }
    let size = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
    // `1` means the real size is a following 64-bit field; `0` means "to end of file". Both
    // are legal, and anything else must at least be a header's worth.
    size == 0 || size == 1 || size >= 8
}

/// A media playlist with every fetchable URI pointed back at this proxy.
///
/// Segment URIs, initialisation segments and key URIs are all rewritten: a key that still
/// pointed upstream would be fetched by mpv without the stream's referer and refused, and a
/// nested playlist has to come back through here or its own segments would go direct and
/// unmended.
///
/// **Rewriting rather than parsing** is deliberate. A full HLS parser (`m3u8-rs` is the good
/// one) would round-trip the playlist through its own model and re-serialise it, which drops
/// any tag the model does not know — including whatever the spec adds next. Every byte here
/// survives except the URIs, which is the only change a proxy has any business making.
pub fn rewrite_playlist(body: &str, base: &str, proxy: &dyn Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(body.len() + 256);

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        if trimmed.starts_with('#') {
            // Any tag carrying a `URI="…"` attribute, rather than an enumerated list of the
            // ones known today: `EXT-X-KEY`, `-MAP`, `-MEDIA`, `-I-FRAME-STREAM-INF`,
            // `-SESSION-KEY` and `-PART` all use the same attribute, and a tag added to the
            // spec next year would otherwise slip through pointing upstream. The value is
            // replaced in place and every other byte of the tag is kept.
            if let Some(rewritten) = rewrite_uri_attribute(trimmed, base, proxy) {
                out.push_str(&rewritten);
                out.push('\n');
                continue;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // A bare line in a playlist is a URI: a segment, or a nested playlist.
        out.push_str(&proxy(&resolve_url(base, trimmed)));
        out.push('\n');
    }

    out
}

/// Rewrite the `URI="…"` attribute of one tag, if it has one.
fn rewrite_uri_attribute(
    tag: &str,
    base: &str,
    proxy: &dyn Fn(&str) -> String,
) -> Option<String> {
    let start = tag.find("URI=\"")? + 5;
    let end = start + tag[start..].find('"')?;
    let absolute = resolve_url(base, &tag[start..end]);
    Some(format!("{}{}{}", &tag[..start], proxy(&absolute), &tag[end..]))
}

/// Resolve a possibly-relative URI against the URL it was found in.
///
/// Hand-rolled rather than pulled from a URL crate: playlists use three forms — absolute,
/// root-relative and path-relative — and this handles exactly those without adding a
/// dependency for the sake of it.
pub fn resolve_url(base: &str, reference: &str) -> String {
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return reference.to_owned();
    }

    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    let authority_end =
        base[scheme_end..].find('/').map(|i| scheme_end + i).unwrap_or(base.len());

    if let Some(rooted) = reference.strip_prefix('/') {
        return format!("{}/{}", &base[..authority_end], rooted);
    }

    // Path-relative: everything up to the last slash, then the reference. Query strings on the
    // base are not part of the directory.
    let path = base.split('?').next().unwrap_or(base);
    let directory = path.rfind('/').map(|i| &path[..=i]).unwrap_or(base);
    format!("{directory}{reference}")
}

/// A running mender.
pub struct MendServer {
    url: String,
    thumbnail_url: String,
    handle: tokio::task::JoinHandle<()>,
}

impl MendServer {
    /// The URL to hand to the player, in place of the stream's own.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The URL to hand to the seek bar's thumbnailer: the same stream, read under
    /// [`Class::Thumbnail`]'s rules.
    pub fn thumbnail_url(&self) -> &str {
        &self.thumbnail_url
    }
}

/// Which player a request comes from, told apart by the entry it came in through.
///
/// Both read the same stream and share one cache, but they want different things when the
/// upstream pushes back. Playback must not lose a segment — ffmpeg skips one it cannot open,
/// and skipping is what ran an episode to its end minutes in — so its requests are held through
/// a rate limit. A thumbnail is worth nothing a second later: the pointer has moved on, and a
/// thumbnailer parked on a held request shows the last picture it drew for as long as the hold
/// lasts. So thumbnail requests are answered from the cache when they can be, go upstream one
/// at a time, never wait out a rate limit, and a `429` they earn holds back only further
/// thumbnail requests, never playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Playback,
    Thumbnail,
}

impl Class {
    /// The path segment that names the class in a proxied URL.
    fn segment(self) -> &'static str {
        match self {
            Self::Playback => "m",
            Self::Thumbnail => "t",
        }
    }
}

/// A resource as the upstream sent it, before any mending or rewriting.
#[derive(Debug)]
struct Fetched {
    content_type: String,
    body: Vec<u8>,
}

/// One fetch, awaited by every request that wants the same resource while it runs.
type InFlight = Arc<tokio::sync::OnceCell<Result<Arc<Fetched>, String>>>;

/// Resources fetched whole, kept for whichever player asks next, least recently used first out.
///
/// Keyed by upstream URL. A segment's URL names one immutable piece of media, so a second read
/// can never be stale; a playlist that may still grow is never kept (see [`is_cacheable`]).
#[derive(Default)]
struct Cache {
    entries: HashMap<String, (Arc<Fetched>, u64)>,
    bytes: usize,
    clock: u64,
}

impl Cache {
    fn get(&mut self, url: &str) -> Option<Arc<Fetched>> {
        self.clock += 1;
        let clock = self.clock;
        self.entries.get_mut(url).map(|(fetched, used)| {
            *used = clock;
            Arc::clone(fetched)
        })
    }

    fn insert(&mut self, url: &str, fetched: Arc<Fetched>, limit: usize) {
        let size = fetched.body.len();
        if size > CACHE_ENTRY_LIMIT || size > limit || self.entries.contains_key(url) {
            return;
        }
        while self.bytes + size > limit {
            // A linear scan: a few hundred entries at most, against a network fetch.
            let Some(oldest) =
                self.entries.iter().min_by_key(|(_, (_, used))| *used).map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some((evicted, _)) = self.entries.remove(&oldest) {
                self.bytes -= evicted.body.len();
            }
        }
        self.clock += 1;
        self.bytes += size;
        self.entries.insert(url.to_owned(), (fetched, self.clock));
    }
}

/// Whether a fetched resource can be served again later.
///
/// Everything but a media playlist that may still grow: a live or event playlist is re-read
/// for its new segments, and a kept copy would freeze it. A playlist that has ended, or a
/// master playlist with no segments of its own, is as fixed as a segment.
fn is_cacheable(fetched: &Fetched) -> bool {
    if !looks_like_playlist(&fetched.content_type, &fetched.body) {
        return true;
    }
    let text = String::from_utf8_lossy(&fetched.body);
    text.contains("#EXT-X-ENDLIST") || !text.contains("#EXTINF")
}

/// The segments a media playlist lists, in order, as absolute URLs.
fn segment_urls(body: &str, base: &str) -> Vec<String> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| resolve_url(base, line))
        .collect()
}

impl Drop for MendServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Everything one proxied stream needs: the client to fetch with, and the headers to fetch with.
struct Upstream {
    http: HttpClient,
    headers: Vec<(String, String)>,
    token: String,
    port: u16,
    /// No upstream request before this instant. Shared, because a rate limit is per client
    /// rather than per segment: one request learning to wait means every other one should too,
    /// not discover it again by spending another request.
    cooldown: Mutex<Option<Instant>>,
    /// No thumbnail request upstream before this instant: set by a `429` a thumbnail request
    /// earned, and read by thumbnail requests alone. Playback's own `cooldown` holds back
    /// thumbnails too, but this never holds back playback.
    thumbnail_cooldown: Mutex<Option<Instant>>,
    /// Whole resources already fetched, for whichever player asks next.
    cache: Mutex<Cache>,
    cache_limit: usize,
    /// Fetches under way, so a second request for the same resource waits for the first
    /// rather than asking the upstream again — thumbfast's exact seek right behind its
    /// keyframe seek, or a thumbnail of the segment playback is fetching this moment.
    in_flight: Mutex<HashMap<String, InFlight>>,
    /// Held by the one thumbnail request allowed upstream at a time, with when it went.
    thumbnail_turn: tokio::sync::Mutex<Option<Instant>>,
    /// The ticket of the newest thumbnail request waiting to go upstream.
    thumbnail_newest: AtomicU64,
    /// Each segment's successor in the playlists served so far, and the last thumbnail
    /// request refused, with when. See [`Upstream::refused_successor`].
    successors: Mutex<HashMap<String, String>>,
    last_refused: Mutex<Option<(String, Instant)>>,
}

impl Upstream {
    fn new(http: HttpClient, headers: Vec<(String, String)>, token: &str, port: u16) -> Self {
        Self {
            http,
            headers,
            token: token.to_owned(),
            port,
            cooldown: Mutex::new(None),
            thumbnail_cooldown: Mutex::new(None),
            cache: Mutex::new(Cache::default()),
            cache_limit: CACHE_BYTES,
            in_flight: Mutex::new(HashMap::new()),
            thumbnail_turn: tokio::sync::Mutex::new(None),
            thumbnail_newest: AtomicU64::new(0),
            successors: Mutex::new(HashMap::new()),
            last_refused: Mutex::new(None),
        }
    }

    /// The loopback URL that will fetch and mend `url` for a player of `class`.
    fn proxied(&self, url: &str, class: Class) -> String {
        format!(
            "http://127.0.0.1:{}/{}/{}/{}",
            self.port,
            class.segment(),
            self.token,
            encode_path(url)
        )
    }

    fn cached(&self, url: &str) -> Option<Arc<Fetched>> {
        lock(&self.cache).get(url)
    }

    /// Run `fetch` as the one fetch of `url`, publishing it for others to wait on while it
    /// runs and keeping the result if it can be served again.
    async fn lead(
        &self,
        url: &str,
        cell: InFlight,
        fetch: impl Future<Output = Result<Fetched, String>>,
    ) -> Result<Arc<Fetched>, String> {
        let result = cell.get_or_init(|| async { fetch.await.map(Arc::new) }).await.clone();
        {
            let mut in_flight = lock(&self.in_flight);
            if in_flight.get(url).is_some_and(|c| Arc::ptr_eq(c, &cell)) {
                in_flight.remove(url);
            }
        }
        if let Ok(fetched) = &result
            && is_cacheable(fetched)
        {
            lock(&self.cache).insert(url, Arc::clone(fetched), self.cache_limit);
        }
        result
    }

    /// A new in-flight entry for `url`, published unless another fetch already holds the slot.
    fn begin(&self, url: &str) -> InFlight {
        let cell = InFlight::default();
        lock(&self.in_flight).entry(url.to_owned()).or_insert_with(|| Arc::clone(&cell));
        cell
    }

    /// Fetch for the playing stream: from the cache, else upstream, held through rate limits.
    async fn for_playback(&self, url: &str) -> Result<Arc<Fetched>, String> {
        if let Some(hit) = self.cached(url) {
            return Ok(hit);
        }
        // Playback always fetches for itself rather than waiting on another fetch: one led by
        // a thumbnail request gives up on a `429`, and playback must not.
        let cell = self.begin(url);
        self.lead(url, cell, fetch(self, url)).await
    }

    /// Fetch for the thumbnailer, by [`Class::Thumbnail`]'s rules. `Err` is a refusal, to be
    /// answered at once.
    async fn for_thumbnail(&self, url: &str) -> Result<Arc<Fetched>, String> {
        if self.refused_successor(url) {
            return Err(self.refuse(url, "the segment after a refused one"));
        }
        if let Some(hit) = self.cached(url) {
            return Ok(hit);
        }
        if self.cooling_down() {
            return Err(self.refuse(url, "upstream is rate-limiting"));
        }
        let joined = lock(&self.in_flight).get(url).cloned();
        if let Some(cell) = joined {
            return self.await_other(url, cell).await;
        }

        // One thumbnail fetch upstream at a time, spaced, and only the newest request that
        // wants one: a request overtaken while it waited gives way to the one that overtook
        // it, so a sweep sends its last position upstream rather than every one it crossed.
        let ticket = self.thumbnail_newest.fetch_add(1, Ordering::SeqCst) + 1;
        let Ok(mut turn) =
            tokio::time::timeout(THUMBNAIL_WAIT, self.thumbnail_turn.lock()).await
        else {
            return Err(self.refuse(url, "another thumbnail fetch is still running"));
        };
        if let Some(last) = *turn {
            tokio::time::sleep_until((last + THUMBNAIL_SPACING).into()).await;
        }
        if self.thumbnail_newest.load(Ordering::SeqCst) != ticket {
            // Not `refuse`: whoever asked has moved on to the newer request, so nothing will
            // skip on from this one.
            return Err("a newer thumbnail request overtook it".to_owned());
        }
        // The wait may have been long enough for someone else to fetch it.
        if let Some(hit) = self.cached(url) {
            return Ok(hit);
        }
        if self.cooling_down() {
            return Err(self.refuse(url, "upstream is rate-limiting"));
        }
        let joined = lock(&self.in_flight).get(url).cloned();
        if let Some(cell) = joined {
            drop(turn);
            return self.await_other(url, cell).await;
        }
        *turn = Some(Instant::now());
        let cell = self.begin(url);
        self.lead(url, cell, fetch_once(self, url)).await.map_err(|e| self.refuse(url, &e))
    }

    /// Wait, briefly, on a fetch another request started.
    async fn await_other(&self, url: &str, cell: InFlight) -> Result<Arc<Fetched>, String> {
        let waited = tokio::time::timeout(
            THUMBNAIL_WAIT,
            // Only reached if that fetch was abandoned midway: nothing to wait for any more.
            cell.get_or_init(|| async {
                Err("the fetch it was waiting on stopped".to_owned())
            }),
        )
        .await;
        match waited {
            Ok(Ok(fetched)) => Ok(Arc::clone(fetched)),
            Ok(Err(e)) => Err(self.refuse(url, e)),
            Err(_) => Err(self.refuse(url, "the fetch under way is taking too long")),
        }
    }

    /// Whether either class is waiting out a rate limit.
    fn cooling_down(&self) -> bool {
        let now = Instant::now();
        [&self.cooldown, &self.thumbnail_cooldown]
            .iter()
            .any(|cooldown| lock(cooldown).is_some_and(|until| until > now))
    }

    /// Note a refused thumbnail request, and say why.
    fn refuse(&self, url: &str, why: &str) -> String {
        *lock(&self.last_refused) = Some((url.to_owned(), Instant::now()));
        why.to_owned()
    }

    /// Whether `url` is the segment after one just refused to the thumbnailer.
    ///
    /// A refusal makes ffmpeg's HLS demuxer open the next segment at once, and the next, until
    /// one opens. Left alone that walks forward to the first segment the cache holds — often
    /// the part around the playhead, minutes from where the pointer is — and the thumbnail
    /// shows that, under the wrong time. Refusing the walk as well runs it to the end of the
    /// playlist instead, which draws nothing: no thumbnail rather than a wrong one.
    fn refused_successor(&self, url: &str) -> bool {
        let Some((refused, at)) = lock(&self.last_refused).clone() else { return false };
        at.elapsed() < SKIP_WINDOW
            && lock(&self.successors).get(&refused).is_some_and(|next| next == url)
    }

    /// Remember the order of a media playlist's segments.
    fn learn_order(&self, body: &str, base: &str) {
        // A master playlist lists variants, not a sequence.
        if !body.contains("#EXTINF") {
            return;
        }
        let urls = segment_urls(body, base);
        let mut successors = lock(&self.successors);
        for pair in urls.windows(2) {
            successors.insert(pair[0].clone(), pair[1].clone());
        }
    }
}

/// Lock a mutex, recovering it if a panic poisoned it: every value guarded here is valid at
/// any instant, so carrying on beats taking the proxy down with the panic.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Percent-encode a URL so it can ride inside a path segment.
fn encode_path(url: &str) -> String {
    let mut out = String::with_capacity(url.len() + 16);
    for byte in url.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Reverse [`encode_path`].
fn decode_path(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&encoded[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Serve `url` through a mending proxy on loopback, until the returned server is dropped.
///
/// `headers` are the stream's own — the referer a locked CDN insists on travels with every
/// upstream request, which is a second thing this fixes: mpv would otherwise have to be told
/// them, and it cannot be told different headers per host.
pub async fn serve(
    http: HttpClient,
    url: &str,
    headers: Vec<(String, String)>,
    token: &str,
) -> std::io::Result<MendServer> {
    // Loopback only, like every other server this app runs.
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
    let port = listener.local_addr()?.port();

    let upstream = Arc::new(Upstream::new(http, headers, token, port));
    let entry = upstream.proxied(url, Class::Playback);
    let thumbnail_entry = upstream.proxied(url, Class::Thumbnail);

    let handle = {
        let upstream = Arc::clone(&upstream);
        tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    break;
                };
                let upstream = Arc::clone(&upstream);
                // One task per connection: mpv opens several at once for a playlist and its
                // segments, and a slow segment must not block the rest.
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(socket, upstream).await {
                        tracing::debug!(error = %e, "mender connection ended");
                    }
                });
            }
        })
    };

    tracing::info!(%entry, "mending proxy listening on loopback");
    Ok(MendServer { url: entry, thumbnail_url: thumbnail_entry, handle })
}

async fn handle_connection(socket: TcpStream, upstream: Arc<Upstream>) -> std::io::Result<()> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    read_line(&mut reader, &mut request_line).await?;

    // The `Range` header is the one thing carried through. A mended body has different offsets
    // from the upstream one, so a range cannot be answered *and* mended — but ignoring it and
    // replying `200` with the whole resource is worse than either: a playlist using
    // `#EXT-X-BYTERANGE` addresses many segments into one large file, so every segment request
    // returned the entire file, blew the body ceiling, and killed a stream that plays fine
    // without the proxy in the way. Such a resource is passed through untouched instead.
    let mut range: Option<String> = None;
    loop {
        let mut line = String::new();
        let n = read_line(&mut reader, &mut line).await?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("range")
        {
            range = Some(value.trim().to_owned());
        }
    }

    let Some(target) = request_target(&request_line) else {
        write_half
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n")
            .await?;
        return Ok(());
    };

    let routed = [Class::Playback, Class::Thumbnail].into_iter().find_map(|class| {
        let prefix = format!("/{}/{}/", class.segment(), upstream.token);
        target.strip_prefix(&prefix).map(|encoded| (class, decode_path(encoded)))
    });
    let Some((class, url)) = routed else {
        write_half.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await?;
        return Ok(());
    };

    // A ranged request asks for a slice of a resource, which is never the start of a segment,
    // so there is nothing to mend and the upstream's own answer is the right one to relay.
    //
    // Except the one mpv opens every resource with. `bytes=0-` asks for the whole thing and is
    // sent on the very first request, playlists included — relaying that meant the master
    // playlist came back verbatim, its segment URIs never rewritten, and mpv resolved them
    // against the proxy's own address and got nothing. An open range from zero is not a slice.
    if let Some(range) = range.filter(|r| !is_whole_resource(r)) {
        return relay_range(&upstream, class, &url, &range, &mut write_half).await;
    }

    let fetched = match class {
        Class::Playback => upstream.for_playback(&url).await,
        Class::Thumbnail => upstream.for_thumbnail(&url).await,
    };
    match fetched {
        Ok(fetched) => {
            let content_type = &fetched.content_type;
            let body = if looks_like_playlist(content_type, &fetched.body) {
                let text = String::from_utf8_lossy(&fetched.body);
                upstream.learn_order(&text, &url);
                let proxy = |u: &str| upstream.proxied(u, class);
                rewrite_playlist(&text, &url, &proxy).into_bytes()
            } else {
                let start = payload_start(&fetched.body);
                if start > 0 {
                    tracing::debug!(url = %url, stripped = start, "mended a disguised segment");
                }
                fetched.body[start..].to_vec()
            };
            // The thumbnailer's refusals are logged and its answers were not, so a seek bar
            // showing no thumbnails could not be told from one that never asked.
            if class == Class::Thumbnail {
                tracing::debug!(url = %url, bytes = body.len(), "thumbnail request answered");
            }

            let head = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {content_type}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            );
            write_half.write_all(head.as_bytes()).await?;
            write_half.write_all(&body).await?;
            write_half.flush().await
        }
        Err(e) if class == Class::Thumbnail => {
            tracing::debug!(url = %url, reason = %e, "thumbnail request refused");
            // At once, and as a refusal: the thumbnailer moves on to the newest position,
            // where waiting would have frozen it on this one.
            write_half
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n")
                .await?;
            Ok(())
        }
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "mender upstream failed");
            // The upstream's failure, reported as one. mpv treats 502 as a dead segment and
            // says so, which is more honest than an empty 200.
            write_half
                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                .await?;
            Ok(())
        }
    }
}

/// Whether a `Range` header asks for the entire resource.
///
/// `bytes=0-` is what a player sends to open a file it intends to read through, so it carries no
/// intent to slice and must not turn off mending.
fn is_whole_resource(range: &str) -> bool {
    range
        .trim()
        .strip_prefix("bytes=")
        .map(str::trim)
        .is_some_and(|spec| spec == "0-" || spec.is_empty())
}

/// Relay a byte range from upstream, unmended.
///
/// Mending is a whole-resource operation — it decides where a payload *starts* — so a request
/// for bytes 800..1599 of a file has nothing to mend and must not be answered with the file.
/// The upstream's status, content range and body are passed through as they came.
async fn relay_range(
    upstream: &Upstream,
    class: Class,
    url: &str,
    range: &str,
    write_half: &mut tokio::net::tcp::OwnedWriteHalf,
) -> std::io::Result<()> {
    let relayed = async {
        let response = match class {
            Class::Playback => send(upstream, url, Some(range)).await?,
            // Never held, as for a whole thumbnail segment; see `Class`.
            Class::Thumbnail => {
                if upstream.cooling_down() {
                    return Err("upstream is rate-limiting".to_owned());
                }
                send_once(upstream, url, Some(range)).await?
            }
        };
        let status = response.status().as_u16();
        let content_range = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().await.map_err(|e| e.to_string())?;
        if body.len() > MAX_BODY {
            return Err(format!("a ranged body of {} bytes is too large", body.len()));
        }
        Ok::<_, String>((status, content_range, body))
    }
    .await;

    match relayed {
        Ok((status, content_range, body)) => {
            let reason = if status == 206 { "Partial Content" } else { "OK" };
            let mut head = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n",
                body.len()
            );
            if let Some(content_range) = content_range {
                head.push_str(&format!("Content-Range: {content_range}\r\n"));
            }
            head.push_str("Connection: close\r\n\r\n");
            write_half.write_all(head.as_bytes()).await?;
            write_half.write_all(&body).await?;
            write_half.flush().await
        }
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "mender range relay failed");
            write_half.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n").await
        }
    }
}

/// Send one upstream request with the stream's headers, waiting out a rate limit.
///
/// Only `429` is waited on: it is the one status that says "this will work if you ask again
/// later". Once the budget is spent the refusal is returned for the caller to report.
async fn send(
    upstream: &Upstream,
    url: &str,
    range: Option<&str>,
) -> Result<wreq::Response, String> {
    let deadline = Instant::now() + RATE_LIMIT_BUDGET;
    loop {
        let not_before = *upstream.cooldown.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(at) = not_before {
            tokio::time::sleep_until(at.into()).await;
        }

        let response = request(upstream, url, range).await?;
        if response.status().as_u16() != 429 {
            return Ok(response);
        }

        let wait = retry_after(&response).unwrap_or(DEFAULT_RETRY_AFTER);
        let resume = Instant::now() + wait;
        if resume > deadline {
            return Ok(response);
        }
        tracing::info!(url = %url, wait_secs = wait.as_secs(), "upstream rate-limited; holding");
        let mut cooldown = upstream.cooldown.lock().unwrap_or_else(|e| e.into_inner());
        if cooldown.is_none_or(|at| at < resume) {
            *cooldown = Some(resume);
        }
    }
}

/// A `Retry-After` given in seconds. The HTTP-date form is rare for rate limits and falls back
/// to the default.
fn retry_after(response: &wreq::Response) -> Option<Duration> {
    let value = response.headers().get("retry-after")?.to_str().ok()?;
    parse_retry_after(value)
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    // Capped: a host asking for minutes is not going to be waited out inside one request.
    value.trim().parse::<u64>().ok().map(|secs| Duration::from_secs(secs.clamp(1, 30)))
}

/// Fetch one upstream URL with the stream's headers.
async fn fetch(upstream: &Upstream, url: &str) -> Result<Fetched, String> {
    read_body(send(upstream, url, None).await?).await
}

/// Fetch one upstream URL for the thumbnailer: a single request, never held. A `429` starts
/// the thumbnail cooldown, and only that one.
async fn fetch_once(upstream: &Upstream, url: &str) -> Result<Fetched, String> {
    read_body(send_once(upstream, url, None).await?).await
}

/// One upstream request, for the thumbnailer. See [`fetch_once`].
async fn send_once(
    upstream: &Upstream,
    url: &str,
    range: Option<&str>,
) -> Result<wreq::Response, String> {
    let response = request(upstream, url, range).await?;
    if response.status().as_u16() == 429 {
        let wait = retry_after(&response).unwrap_or(DEFAULT_RETRY_AFTER);
        tracing::info!(url = %url, wait_secs = wait.as_secs(), "upstream rate-limited a thumbnail");
        *lock(&upstream.thumbnail_cooldown) = Some(Instant::now() + wait);
        return Err("upstream answered 429".to_owned());
    }
    Ok(response)
}

/// One request upstream, with the stream's headers.
async fn request(
    upstream: &Upstream,
    url: &str,
    range: Option<&str>,
) -> Result<wreq::Response, String> {
    let mut request = upstream.http.emulated().get(url);
    if let Some(range) = range {
        request = request.header("range", range);
    }
    for (name, value) in &upstream.headers {
        request = request.header(name.as_str(), value.as_str());
    }
    request.send().await.map_err(|e| e.to_string())
}

/// Read a whole response body, refusing anything that is not a success or is too large.
async fn read_body(response: wreq::Response) -> Result<Fetched, String> {
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();

    // Refused on the declared length before anything is read, then enforced again while
    // reading, because a length is a claim rather than a fact. Checking only the finished
    // body — as this did — meant the ceiling rejected the response *after* the host had
    // already allocated it, so a segment advertised at four gigabytes cost four gigabytes to
    // decline. `fetch-many` makes that concurrent.
    if response.content_length().is_some_and(|len| len > MAX_BODY as u64) {
        return Err(format!(
            "a body of {} bytes is beyond what this proxies",
            response.content_length().unwrap_or_default()
        ));
    }

    let mut response = response;
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(format!("a body beyond {MAX_BODY} bytes is more than this proxies"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Fetched { content_type, body })
}

/// Whether a response is an HLS playlist rather than media.
///
/// The body is trusted over the content type: image CDNs serving video segments are exactly
/// the kind of host that also mislabels a playlist.
fn looks_like_playlist(content_type: &str, body: &[u8]) -> bool {
    if body.starts_with(b"#EXTM3U") {
        return true;
    }
    let lowered = content_type.to_ascii_lowercase();
    (lowered.contains("mpegurl") || lowered.contains("m3u"))
        && body.len() < 4 * 1024 * 1024
        && std::str::from_utf8(body).is_ok()
}

/// The request target, for read methods only.
fn request_target(line: &str) -> Option<&str> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return None;
    }
    parts.next().map(|t| t.split('?').next().unwrap_or(t))
}

/// Read one CRLF-terminated line, capped so a hostile client cannot exhaust memory.
async fn read_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut String,
) -> std::io::Result<usize> {
    const MAX: usize = 16 * 1024;
    let mut total = 0;
    let mut byte = [0u8; 1];
    loop {
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            break;
        }
        total += n;
        if byte[0] == b'\n' {
            break;
        }
        if byte[0] != b'\r' {
            out.push(byte[0] as char);
        }
        if total > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request line too long",
            ));
        }
    }
    Ok(total)
}

/// Registry of every mender kept alive for a playback, keyed by the URL it fronts.
///
/// A playback holds one; dropping it stops every proxy it started.
#[derive(Default)]
pub struct Menders(HashMap<String, MendServer>);

impl Menders {
    /// Route `url` through a mender, returning the URL to play instead.
    ///
    /// Failure to start one is not fatal — the original URL is returned and playback proceeds
    /// exactly as it would have. A proxy that cannot start must not cost you the episode.
    pub async fn route(
        &mut self,
        http: &HttpClient,
        url: &str,
        headers: &[(String, String)],
        token: &str,
    ) -> String {
        match serve(http.clone(), url, headers.to_vec(), token).await {
            Ok(server) => {
                let mended = server.url().to_owned();
                self.0.insert(url.to_owned(), server);
                mended
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not start the mender; playing direct");
                url.to_owned()
            }
        }
    }

    /// The thumbnailer's entry to the mender fronting `url`, when one is running.
    pub fn thumbnail_url(&self, url: &str) -> Option<String> {
        self.0.get(url).map(|server| server.thumbnail_url().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Several TS packets, the shape a real segment has: a PAT first, payload-unit-start set.
    fn transport_stream() -> Vec<u8> {
        let mut data = Vec::new();
        for packet in 0..6 {
            // First packet opens the segment (PUSI, PID 0); the rest carry an ordinary PID.
            let (flags, pid_low) = if packet == 0 { (0x40, 0x00) } else { (0x00, 0x64) };
            data.extend([0x47, flags, pid_low, 0x10]);
            data.extend(std::iter::repeat_n(0xAB, TS_PACKET - 4));
        }
        data
    }

    /// The observed disguise: a complete 1×1 PNG, then the real video.
    fn png_wrapped(payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend(b"\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89");
        data.extend(b"\0\0\0\rIDATx\xdacd\xf8\xcfP\x0f\0\x03\x86\x01\x80Z4}k");
        data.extend(b"\0\0\0\0IEND\xaeB`\x82");
        data.extend(payload);
        data
    }

    #[test]
    fn a_healthy_transport_stream_is_left_alone() {
        // The property that makes routing every stream through this safe: an untouched
        // segment passes through byte-for-byte.
        assert_eq!(payload_start(&transport_stream()), 0);
    }

    #[test]
    fn video_hidden_behind_an_image_header_is_found() {
        // The case this exists for. The prefix is a genuine, complete PNG — so "is it a PNG"
        // is the wrong question, and only looking for the video answers it.
        let wrapped = png_wrapped(&transport_stream());
        let start = payload_start(&wrapped);
        assert_ne!(start, 0, "the disguise was not seen through");
        assert_eq!(&wrapped[start..], &transport_stream()[..]);
    }

    #[test]
    fn an_fmp4_segment_is_recognised_wrapped_or_not() {
        let mut fmp4 = Vec::new();
        fmp4.extend(&[0, 0, 0, 0x18]);
        fmp4.extend(b"ftypiso5");
        fmp4.extend(std::iter::repeat_n(0u8, 16));
        assert_eq!(payload_start(&fmp4), 0);

        let wrapped = png_wrapped(&fmp4);
        assert_eq!(&wrapped[payload_start(&wrapped)..], &fmp4[..]);
    }

    #[test]
    fn a_sync_byte_one_packet_before_the_payload_does_not_steal_the_offset() {
        // The failure this exists for, and it is not hypothetical: a wrapper of length `w` puts
        // a stray `0x47` at `w - 188` exactly 188 bytes from the real start, so every boundary
        // the cadence check probes is a *genuine* packet. One byte in 256 — near certain across
        // an episode's worth of segments — and the reward for believing it is 188 bytes of
        // wrapper left on the front: a cut and a slip in sync rather than an honest failure.
        // 252 bytes is the wrapper length observed in the wild, and the reason the decoy lands
        // at 64: 252 - 188. The decoy is deliberately a *plausible* packet header — error bit
        // clear, adaptation field valid — so only "does this open a segment" separates it from
        // the real thing.
        const WRAPPER: usize = 252;
        let payload = transport_stream();
        let mut wrapped = vec![0u8; WRAPPER];
        wrapped[..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        wrapped[WRAPPER - TS_PACKET..][..4].copy_from_slice(&[0x47, 0x01, 0x64, 0x10]);
        wrapped.extend_from_slice(&payload);

        assert_eq!(
            payload_start(&wrapped),
            WRAPPER,
            "a decoy sync byte took the offset one packet early"
        );
        assert_eq!(&wrapped[payload_start(&wrapped)..], &payload[..]);
    }

    #[test]
    fn a_stream_that_starts_mid_flow_is_still_mended() {
        // The strict pass wants a segment opening — PAT, payload-unit-start. A source whose
        // segments begin mid-stream has neither, and must still be served rather than refused.
        let mut payload = Vec::new();
        for _ in 0..6 {
            payload.extend([0x47, 0x01, 0x64, 0x10]);
            payload.extend(std::iter::repeat_n(0xCD, TS_PACKET - 4));
        }
        let wrapped = png_wrapped(&payload);
        assert_eq!(&wrapped[payload_start(&wrapped)..], &payload[..]);
    }

    #[test]
    fn a_lone_sync_byte_is_not_mistaken_for_a_stream() {
        // 0x47 is the letter G and an ordinary byte in compressed video; one occurrence must
        // never truncate a segment. Only the 188-byte cadence is believed.
        let mut data = vec![0u8; 2000];
        data[5] = 0x47;
        data[100] = 0x47;
        assert_eq!(payload_start(&data), 0, "a stray 0x47 must not be treated as a payload");
    }

    #[test]
    fn unrecognised_bytes_pass_through_untouched() {
        // Refusing to serve what we merely do not recognise would break more than it fixes;
        // the player is the better judge.
        let noise: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        assert_eq!(payload_start(&noise), 0);
    }

    #[test]
    fn subtitles_and_playlists_are_never_sliced() {
        assert_eq!(payload_start(b"WEBVTT\n\n00:00.000 --> 00:02.000\nhello\n"), 0);
        assert_eq!(payload_start(b"#EXTM3U\n#EXT-X-VERSION:3\n"), 0);
    }

    #[test]
    fn a_tag_this_code_never_heard_of_still_has_its_uri_rewritten() {
        // Any tag with a URI attribute, rather than a list of the ones known today — a tag
        // added to the spec later would otherwise quietly point upstream, unmended.
        let proxy = |u: &str| format!("proxied:{u}");
        let out = rewrite_playlist(
            "#EXTM3U\n#EXT-X-SOMETHING-NEW:URI=\"future.ts\",FLAG=1\n",
            "https://cdn.test/a/index.m3u8",
            &proxy,
        );
        assert!(out.contains("proxied:https://cdn.test/a/future.ts"), "{out}");
        assert!(out.contains("FLAG=1"), "the rest of the tag must survive: {out}");
    }

    #[test]
    fn every_uri_in_a_playlist_comes_back_through_the_proxy() {
        let playlist = "#EXTM3U\n\
             #EXT-X-VERSION:3\n\
             #EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x00\n\
             #EXT-X-MAP:URI=\"init.mp4\"\n\
             #EXTINF:10.0,\n\
             seg1.ts\n\
             #EXTINF:10.0,\n\
             https://other.test/seg2.ts\n";
        let proxy = |u: &str| format!("http://127.0.0.1:1/m/t/{}", encode_path(u));
        let out = rewrite_playlist(playlist, "https://cdn.test/hls/index.m3u8", &proxy);

        // Segments, the key and the init segment all route back — a key fetched directly
        // would go without the stream's referer and be refused.
        assert!(out.contains(&proxy("https://cdn.test/hls/seg1.ts")), "{out}");
        assert!(out.contains(&proxy("https://other.test/seg2.ts")), "{out}");
        assert!(out.contains(&proxy("https://cdn.test/hls/key.bin")), "{out}");
        assert!(out.contains(&proxy("https://cdn.test/hls/init.mp4")), "{out}");
        // Tags keep their other attributes exactly.
        assert!(out.contains("METHOD=AES-128"), "{out}");
        assert!(out.contains("IV=0x00"), "{out}");
        assert!(out.contains("#EXTINF:10.0,"), "{out}");
    }

    #[test]
    fn relative_and_absolute_references_both_resolve() {
        let base = "https://cdn.test/a/b/index.m3u8?token=1";
        assert_eq!(resolve_url(base, "seg.ts"), "https://cdn.test/a/b/seg.ts");
        assert_eq!(resolve_url(base, "/root/seg.ts"), "https://cdn.test/root/seg.ts");
        assert_eq!(resolve_url(base, "https://x.test/s.ts"), "https://x.test/s.ts");
    }

    #[test]
    fn the_range_a_player_opens_with_does_not_turn_mending_off() {
        // mpv sends `Range: bytes=0-` on its first request for every resource, playlists
        // included. Treating that as a slice relayed the master playlist verbatim, so its
        // segment URIs were never rewritten and mpv resolved them against the proxy's own
        // address — the stream died on a change meant to fix a different one.
        assert!(is_whole_resource("bytes=0-"));
        assert!(is_whole_resource(" bytes=0- "));

        // A genuine slice, which has nothing to mend and must be relayed.
        assert!(!is_whole_resource("bytes=800-1599"));
        assert!(!is_whole_resource("bytes=-500"));
        assert!(!is_whole_resource("bytes=1000-"));
    }

    #[test]
    fn a_url_survives_the_round_trip_through_a_path() {
        for url in [
            "https://cdn.test/a/b.ts?x=1&y=2",
            // The opaque, extensionless form a segment takes on a CDN that is not serving
            // it as video — no path to parse, so the round trip has to be exact.
            "https://images.test/obj/site-i18n/202604045d0d1d30d738c249",
            "https://cdn.test/spaced%20name.ts",
        ] {
            assert_eq!(decode_path(&encode_path(url)), url);
        }
    }

    #[tokio::test]
    async fn the_proxy_serves_only_its_own_token() {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let server =
            serve(http, "https://example.test/x.m3u8", Vec::new(), "secret").await.unwrap();
        let wrong = server.url().replace("/m/secret/", "/m/guessed/");

        let authority =
            wrong.trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let path = &wrong[wrong.find("/m/").unwrap()..];
        let mut socket = TcpStream::connect(&authority).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        assert!(String::from_utf8_lossy(&raw).starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn a_retry_after_in_seconds_is_honoured_within_bounds() {
        assert_eq!(parse_retry_after("10"), Some(Duration::from_secs(10)));
        assert_eq!(parse_retry_after(" 0 "), Some(Duration::from_secs(1)));
        assert_eq!(parse_retry_after("3600"), Some(Duration::from_secs(30)));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"), None);
    }

    #[tokio::test]
    async fn a_rate_limited_segment_is_waited_for_rather_than_failed() {
        // ffmpeg skips a segment it cannot open, and skipping every segment after a 429 is
        // what ran episodes to their end minutes in. The proxy has to hold on and retry.
        let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let origin = format!("http://{}/seg.ts", upstream.local_addr().unwrap());
        let payload = transport_stream();
        let served = payload.clone();
        tokio::spawn(async move {
            for attempt in 0.. {
                let Ok((mut socket, _)) = upstream.accept().await else { break };
                let mut request = [0u8; 4096];
                let _ = socket.read(&mut request).await;
                let head = if attempt == 0 {
                    "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\n\
                     Connection: close\r\n\r\n"
                        .to_owned()
                } else {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\nContent-Length: {}\r\n\
                         Connection: close\r\n\r\n",
                        served.len()
                    )
                };
                let _ = socket.write_all(head.as_bytes()).await;
                if attempt > 0 {
                    let _ = socket.write_all(&served).await;
                }
            }
        });

        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let server = serve(http, &origin, Vec::new(), "tok").await.unwrap();
        let authority =
            server.url().trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let path = &server.url()[server.url().find("/m/").unwrap()..];

        let started = Instant::now();
        let mut socket = TcpStream::connect(&authority).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();

        assert!(
            String::from_utf8_lossy(&raw).starts_with("HTTP/1.1 200"),
            "relayed the refusal"
        );
        assert!(raw.ends_with(&payload), "the retried body did not come through");
        assert!(started.elapsed() >= Duration::from_secs(1), "did not wait for Retry-After");
    }

    #[tokio::test]
    async fn the_proxy_binds_loopback_only() {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let server =
            serve(http, "https://example.test/x.m3u8", Vec::new(), "tok").await.unwrap();
        assert!(server.url().starts_with("http://127.0.0.1:"), "got {}", server.url());
    }

    // Two players: playback and the seek bar's thumbnailer.

    /// A stand-in segment host on loopback: serves `files` by name after `delay`, logs every
    /// request, and answers `429` with `Retry-After: 10` whenever `limited(name, recent)`
    /// says so, `recent` being the segment requests in the last two seconds, this one
    /// included.
    struct Origin {
        base: String,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Origin {
        fn asked(&self, name: &str) -> usize {
            lock(&self.log).iter().filter(|n| *n == name).count()
        }

        fn segments_asked(&self) -> usize {
            lock(&self.log).iter().filter(|n| n.ends_with(".ts")).count()
        }
    }

    async fn origin(
        files: Vec<(String, Vec<u8>)>,
        delay: Duration,
        limited: impl Fn(&str, usize) -> bool + Send + Sync + 'static,
    ) -> Origin {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let files: Arc<HashMap<String, Vec<u8>>> = Arc::new(files.into_iter().collect());
        let recent = Arc::new(Mutex::new(Vec::<Instant>::new()));
        let limited = Arc::new(limited);
        {
            let log = Arc::clone(&log);
            tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    let (log, files, recent, limited) = (
                        Arc::clone(&log),
                        Arc::clone(&files),
                        Arc::clone(&recent),
                        Arc::clone(&limited),
                    );
                    tokio::spawn(async move {
                        let (read, mut write) = socket.into_split();
                        let mut reader = BufReader::new(read);
                        let mut line = String::new();
                        if read_line(&mut reader, &mut line).await.is_err() {
                            return;
                        }
                        loop {
                            let mut header = String::new();
                            match read_line(&mut reader, &mut header).await {
                                Ok(n) if n > 0 && !header.trim().is_empty() => {}
                                _ => break,
                            }
                        }
                        let path = line.split_whitespace().nth(1).unwrap_or("/");
                        let name = path.rsplit('/').next().unwrap_or("").to_owned();
                        lock(&log).push(name.clone());
                        let is_limited = name.ends_with(".ts") && {
                            let mut recent = lock(&recent);
                            recent.retain(|at| at.elapsed() < Duration::from_secs(2));
                            recent.push(Instant::now());
                            limited(&name, recent.len())
                        };
                        if is_limited {
                            let _ = write
                                .write_all(
                                    b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 10\r\n\
                                      Content-Length: 0\r\nConnection: close\r\n\r\n",
                                )
                                .await;
                            return;
                        }
                        tokio::time::sleep(delay).await;
                        let Some(body) = files.get(&name) else {
                            let _ = write
                                .write_all(
                                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
                                )
                                .await;
                            return;
                        };
                        let kind = if name.ends_with(".m3u8") {
                            "application/vnd.apple.mpegurl"
                        } else {
                            "video/mp2t"
                        };
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                             Connection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = write.write_all(head.as_bytes()).await;
                        let _ = write.write_all(body).await;
                    });
                }
            });
        }
        Origin { base, log }
    }

    /// A VOD playlist of `count` segments named `seg<n>.ts`, each a distinct transport stream.
    fn vod(count: usize) -> Vec<(String, Vec<u8>)> {
        let mut playlist =
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:4\n#EXT-X-PLAYLIST-TYPE:VOD\n"
                .to_owned();
        let mut files = Vec::new();
        for n in 0..count {
            playlist.push_str(&format!("#EXTINF:4.0,\nseg{n}.ts\n"));
            let mut segment = transport_stream();
            segment.push(n as u8);
            files.push((format!("seg{n}.ts"), segment));
        }
        playlist.push_str("#EXT-X-ENDLIST\n");
        files.push(("index.m3u8".to_owned(), playlist.into_bytes()));
        files
    }

    async fn mender_for(origin: &Origin) -> MendServer {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        serve(http, &format!("{}/index.m3u8", origin.base), Vec::new(), "tok").await.unwrap()
    }

    /// GET through the mender: status and body.
    async fn get(url: impl AsRef<str>) -> (u16, Vec<u8>) {
        let rest = url.as_ref().trim_start_matches("http://");
        let (authority, path) = rest.split_once('/').unwrap();
        let mut socket = TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(raw.len());
        let status = String::from_utf8_lossy(&raw[..split])
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        (status, raw.get(split + 4..).unwrap_or_default().to_vec())
    }

    /// The proxied URL of segment `n`, as a player of `class` would find it in its playlist.
    fn segment(server: &MendServer, origin: &Origin, class: Class, n: usize) -> String {
        let entry = match class {
            Class::Playback => server.url(),
            Class::Thumbnail => server.thumbnail_url(),
        };
        let port_and_token = &entry[..entry.rfind('/').unwrap()];
        format!("{port_and_token}/{}", encode_path(&format!("{}/seg{n}.ts", origin.base)))
    }

    /// GET a thumbnail request, insisting it is answered within a second: the property under
    /// test everywhere below. A held request would take the host's ten.
    async fn get_promptly(url: &str) -> (u16, Vec<u8>) {
        tokio::time::timeout(Duration::from_secs(1), get(url))
            .await
            .expect("a thumbnail request was held instead of answered")
    }

    #[tokio::test]
    async fn the_thumbnail_entry_rewrites_its_playlist_onto_itself() {
        let origin = origin(vod(2), Duration::ZERO, |_, _| false).await;
        let server = mender_for(&origin).await;

        let (_, playback) = get(server.url()).await;
        let (_, thumbnails) = get(server.thumbnail_url()).await;
        let (playback, thumbnails) =
            (String::from_utf8_lossy(&playback), String::from_utf8_lossy(&thumbnails));
        // Segments found through one entry come back through the same one, so the
        // thumbnailer's requests stay recognisable all the way down.
        assert!(playback.contains("/m/tok/") && !playback.contains("/t/tok/"), "{playback}");
        assert!(
            thumbnails.contains("/t/tok/") && !thumbnails.contains("/m/tok/"),
            "{thumbnails}"
        );
        // An ended playlist is as fixed as a segment: the second player costs no request.
        assert_eq!(origin.asked("index.m3u8"), 1);
    }

    #[tokio::test]
    async fn thumbnails_are_answered_from_what_playback_already_fetched() {
        let origin = origin(vod(3), Duration::ZERO, |_, _| false).await;
        let server = mender_for(&origin).await;

        let (status, played) = get(&segment(&server, &origin, Class::Playback, 1)).await;
        assert_eq!(status, 200);
        let (status, thumbnail) =
            get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await;
        assert_eq!(status, 200);
        assert_eq!(thumbnail, played);
        assert_eq!(origin.asked("seg1.ts"), 1, "a loaded part of the bar went upstream again");
    }

    #[tokio::test]
    async fn a_rate_limited_thumbnail_is_refused_at_once_and_never_holds_playback() {
        // The reported failure: a 429 parked the thumbnailer for ten seconds, and the hold it
        // set was shared, so playback's own requests waited behind it too.
        let origin = origin(vod(4), Duration::ZERO, |name, _| name == "seg1.ts").await;
        let server = mender_for(&origin).await;

        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await;
        assert_eq!(status, 503);

        let started = Instant::now();
        let (status, _) = get(&segment(&server, &origin, Class::Playback, 2)).await;
        assert_eq!(status, 200);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "playback waited out a rate limit only a thumbnail had earned"
        );

        // While the thumbnail hold lasts, what playback fetched is still a thumbnail away,
        // and nothing else is asked for.
        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 2)).await;
        assert_eq!(status, 200);
        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 3)).await;
        assert_eq!(status, 503);
        assert_eq!(origin.asked("seg3.ts"), 0);
    }

    #[tokio::test]
    async fn while_playback_is_held_thumbnails_are_refused_without_asking_upstream() {
        let origin = origin(vod(3), Duration::ZERO, |name, _| name == "seg0.ts").await;
        let server = mender_for(&origin).await;

        // Playback meets the 429 and holds, as it must.
        let held = tokio::spawn(get(segment(&server, &origin, Class::Playback, 0)));
        let started = Instant::now();
        while origin.asked("seg0.ts") == 0 {
            assert!(started.elapsed() < Duration::from_secs(5), "playback never asked");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await;
        assert_eq!(status, 503);
        assert_eq!(origin.asked("seg1.ts"), 0, "a thumbnail spent a request during the hold");
        held.abort();
    }

    #[tokio::test]
    async fn one_segment_asked_for_at_once_is_fetched_once() {
        // thumbfast's exact seek lands right behind its keyframe seek, on the same segment.
        let origin = origin(vod(1), Duration::from_millis(300), |_, _| false).await;
        let server = mender_for(&origin).await;
        let url = segment(&server, &origin, Class::Thumbnail, 0);

        let asks: Vec<_> = (0..4).map(|_| tokio::spawn(get(url.clone()))).collect();
        for ask in asks {
            assert_eq!(ask.await.unwrap().0, 200);
        }
        assert_eq!(origin.asked("seg0.ts"), 1);
    }

    #[tokio::test]
    async fn only_the_newest_waiting_thumbnail_goes_upstream() {
        // A sweep crosses positions faster than they can be fetched; the ones it crossed are
        // not worth a request each.
        let origin = origin(vod(3), Duration::from_millis(500), |_, _| false).await;
        let server = mender_for(&origin).await;

        let first = tokio::spawn(get(segment(&server, &origin, Class::Thumbnail, 0)));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let crossed = tokio::spawn(get(segment(&server, &origin, Class::Thumbnail, 1)));
        tokio::time::sleep(Duration::from_millis(50)).await;
        let newest = tokio::spawn(get(segment(&server, &origin, Class::Thumbnail, 2)));

        assert_eq!(first.await.unwrap().0, 200);
        assert_eq!(crossed.await.unwrap().0, 503);
        assert_eq!(newest.await.unwrap().0, 200);
        assert_eq!((origin.asked("seg1.ts"), origin.asked("seg2.ts")), (0, 1));
    }

    #[tokio::test]
    async fn the_segments_after_a_refused_one_are_refused_too() {
        // ffmpeg opens the next segment when one is refused, and the next, until one opens:
        // left alone it would land on the first cached one and draw that, minutes off.
        let origin = origin(vod(4), Duration::ZERO, |name, _| name == "seg1.ts").await;
        let server = mender_for(&origin).await;
        get(server.thumbnail_url()).await;
        for n in [0, 2, 3] {
            get(&segment(&server, &origin, Class::Playback, n)).await;
        }

        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await.0, 503);
        // Cached, and refused all the same: these are ffmpeg walking on from the refusal.
        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 2)).await.0, 503);
        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 3)).await.0, 503);
        // A seek elsewhere is not part of the walk.
        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 0)).await.0, 200);
    }

    #[tokio::test]
    async fn a_sweep_through_a_host_that_starts_refusing_never_stalls_a_thumbnail() {
        // The regression as reported, without mpv: a host that answers 429 to a burst of
        // segment requests, playback having loaded the start, and a thumbnailer sweeping the
        // whole bar one segment at a time. Every thumbnail request is answered within a
        // second, no thumbnail request goes upstream once the host has said to stop, and the
        // loaded part of the bar keeps its thumbnails throughout.
        let tripped = Arc::new(Mutex::new(None::<Instant>));
        let origin = origin(vod(12), Duration::from_millis(20), {
            let tripped = Arc::clone(&tripped);
            move |_, recent| {
                let mut tripped = lock(&tripped);
                if tripped.is_some_and(|at| at.elapsed() < Duration::from_secs(10)) {
                    return true;
                }
                if recent > 6 {
                    *tripped = Some(Instant::now());
                    return true;
                }
                false
            }
        })
        .await;
        let server = mender_for(&origin).await;
        get(server.url()).await;
        for n in 0..3 {
            assert_eq!(get(&segment(&server, &origin, Class::Playback, n)).await.0, 200);
        }
        get(server.thumbnail_url()).await;

        let mut answered = Vec::new();
        for n in (3..12).chain((0..12).rev()) {
            answered
                .push(get_promptly(&segment(&server, &origin, Class::Thumbnail, n)).await.0);
        }
        assert!(answered.contains(&503), "the host never pushed back: {answered:?}");
        let at_trip = origin.segments_asked();
        assert!(lock(&tripped).is_some());

        // Resting on the loaded start: answered from memory while the host still refuses.
        for n in [1, 0, 2] {
            assert_eq!(
                get_promptly(&segment(&server, &origin, Class::Thumbnail, n)).await.0,
                200
            );
        }
        assert_eq!(
            origin.segments_asked(),
            at_trip,
            "a thumbnail went upstream while the host was refusing"
        );
    }

    #[test]
    fn the_cache_keeps_the_most_recently_used_within_its_budget() {
        let entry = |size: usize| {
            Arc::new(Fetched { content_type: "video/mp2t".into(), body: vec![0; size] })
        };
        let mut cache = Cache::default();
        cache.insert("a", entry(4), 10);
        cache.insert("b", entry(4), 10);
        assert!(cache.get("a").is_some());
        cache.insert("c", entry(4), 10);
        assert!(cache.get("b").is_none(), "the least recently used should have gone");
        assert!(cache.get("a").is_some() && cache.get("c").is_some());
        assert_eq!(cache.bytes, 8);

        // Something bigger than the whole budget is not kept, and evicts nothing.
        cache.insert("huge", entry(11), 10);
        assert!(cache.get("huge").is_none());
        assert_eq!(cache.bytes, 8);
    }

    #[test]
    fn a_playlist_that_can_still_grow_is_never_kept() {
        let fetched = |body: &str| Fetched {
            content_type: "application/vnd.apple.mpegurl".into(),
            body: body.as_bytes().to_vec(),
        };
        assert!(!is_cacheable(&fetched("#EXTM3U\n#EXTINF:4.0,\na.ts\n")));
        assert!(is_cacheable(&fetched("#EXTM3U\n#EXTINF:4.0,\na.ts\n#EXT-X-ENDLIST\n")));
        assert!(is_cacheable(&fetched("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n")));
        assert!(is_cacheable(&Fetched {
            content_type: "video/mp2t".into(),
            body: transport_stream()
        }));
    }

    /// Diagnosis only: the child's log, printed when the test fails.
    #[cfg(unix)]
    struct DiagDump(std::path::PathBuf, Instant);

    #[cfg(unix)]
    impl Drop for DiagDump {
        fn drop(&mut self) {
            if std::thread::panicking() || std::env::var_os("DIAG_ALWAYS").is_some() {
                let log = std::fs::read_to_string(&self.0).unwrap_or_default();
                let lines: Vec<&str> = log.lines().collect();
                let from = lines.len().saturating_sub(700);
                eprintln!(
                    "diag: failed after {:?}; child log, {} lines, from {from}:\n{}",
                    self.1.elapsed(),
                    lines.len(),
                    lines[from..].join("\n")
                );
            }
        }
    }

    /// mpv's JSON IPC, enough to drive a thumbnailer.
    #[cfg(unix)]
    struct Ipc {
        lines: tokio::io::Lines<BufReader<tokio::net::unix::OwnedReadHalf>>,
        writer: tokio::net::unix::OwnedWriteHalf,
        next_id: u64,
    }

    #[cfg(unix)]
    impl Ipc {
        async fn connect(path: &std::path::Path) -> Self {
            use tokio::io::AsyncBufReadExt;
            for _ in 0..400 {
                if let Ok(socket) = tokio::net::UnixStream::connect(path).await {
                    let (read, writer) = socket.into_split();
                    return Self { lines: BufReader::new(read).lines(), writer, next_id: 0 };
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            panic!("the thumbnailer never opened its IPC socket");
        }

        /// Run a command; a reply slower than two seconds is the freeze under test.
        async fn command(&mut self, args: serde_json::Value) -> serde_json::Value {
            self.next_id += 1;
            let id = self.next_id;
            let line = serde_json::json!({ "command": args, "request_id": id }).to_string();
            self.writer.write_all(format!("{line}\n").as_bytes()).await.unwrap();
            let reply = tokio::time::timeout(Duration::from_secs(2), async {
                while let Some(line) = self.lines.next_line().await.unwrap() {
                    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
                    if value["request_id"] == id {
                        return value;
                    }
                }
                panic!("the thumbnailer closed its IPC socket");
            })
            .await;
            reply.unwrap_or_else(|_| panic!("the thumbnailer stopped answering: {args}"))
        }
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn a_flooded_thumbnailer_draws_where_the_pointer_rests() {
        // thumbfast's own child, with thumbfast's arguments, on the thumbnail entry of an HLS
        // stream whose host starts answering 429 to the burst a fast sweep makes. Playback has
        // loaded the first part. After a flood of seeks across the whole bar, a seek into the
        // loaded part must draw within a bound — before, a held segment froze the child on
        // its last picture for ten seconds at a time.
        let dir = tempfile::tempdir().unwrap();
        let encoded = tokio::process::Command::new("mpv")
            .args(["--no-config", "--really-quiet", "--no-audio"])
            .arg("av://lavfi:testsrc2=size=320x180:rate=24:duration=60")
            .args(["--of=hls", "--ofopts=hls_time=4,hls_playlist_type=vod"])
            // MPEG-2 repeats its sequence header at every keyframe, so each segment decodes
            // on its own; MPEG-4 part 2 puts it in the first segment only.
            .args(["--ovc=mpeg2video", "--ovcopts=g=48,qscale=4"])
            .arg(format!("--o={}", dir.path().join("index.m3u8").display()))
            .status()
            .await;
        match encoded {
            Err(_) => {
                // CI's mpv job installs mpv and sets this, so a missing mpv fails there
                // instead of passing as a skip.
                assert!(
                    std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                    "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
                );
                eprintln!("skipping: mpv is not on PATH");
                return;
            }
            Ok(status) if !status.success() => {
                eprintln!("skipping: this mpv cannot write an HLS fixture");
                return;
            }
            Ok(_) => {}
        }
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            files.push((name, std::fs::read(&path).unwrap()));
        }
        let segments = files.iter().filter(|(name, _)| name.ends_with(".ts")).count();
        let origin = origin(files, Duration::from_millis(30), |_, recent| recent > 6).await;
        let server = mender_for(&origin).await;

        // Playback loads the first 16 seconds.
        get(server.url()).await;
        for n in 0..4 {
            let url = format!("{}/index{n}.ts", origin.base);
            let entry = server.url();
            let proxied =
                format!("{}/{}", &entry[..entry.rfind('/').unwrap()], encode_path(&url));
            assert_eq!(get(&proxied).await.0, 200);
        }

        let ipc_path = dir.path().join("ipc");
        let thumb = dir.path().join("thumb.bgra");
        let frame = 320 * 180 * 4;
        let log = dir.path().join("child.log");
        let _dump = DiagDump(log.clone(), Instant::now());
        let mut child = tokio::process::Command::new("mpv")
            .args(["--no-config", "--msg-level=all=no", "--idle", "--pause"])
            .args(["--keep-open=always", "--really-quiet", "--no-terminal"])
            .args(["--load-scripts=no", "--osc=no", "--ytdl=no", "--no-sub", "--no-audio"])
            .args(["--start=8", "--hr-seek=no", "--demuxer-readahead-secs=0"])
            .args(["--demuxer-max-bytes=128KiB", "--vd-lavc-fast", "--hwdec=no"])
            .args(["--vf=scale=320:-2,format=bgra", "--ovc=rawvideo", "--of=image2"])
            .args(["--ofopts=update=1", "--demuxer-lavf-o-append=http_multiple=0"])
            .arg(format!("--log-file={}", log.display()))
            .arg("--msg-level=all=v,ffmpeg=debug")
            .arg(format!("--o={}", thumb.display()))
            .arg(format!("--input-ipc-server={}", ipc_path.display()))
            .args(["--", server.thumbnail_url()])
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut ipc = Ipc::connect(&ipc_path).await;
        let drawn = |wait: Duration| {
            let thumb = thumb.clone();
            async move {
                let deadline = Instant::now() + wait;
                while Instant::now() < deadline {
                    if std::fs::metadata(&thumb).is_ok_and(|m| m.len() == frame) {
                        return true;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                false
            }
        };
        assert!(drawn(Duration::from_secs(5)).await, "no first thumbnail");

        // The flood: a new position every 15 ms for a second and a half, back and forth
        // across the bar, as thumbfast sends them while the pointer races.
        let started = Instant::now();
        let mut step = 0_u32;
        while started.elapsed() < Duration::from_millis(1500) {
            let sweep = f64::from(step % 40) / 40.0;
            let target = if (step / 40).is_multiple_of(2) { sweep } else { 1.0 - sweep } * 58.0;
            ipc.command(serde_json::json!(["seek", target, "absolute+keyframes"])).await;
            step += 1;
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        assert!(origin.segments_asked() > 4, "the flood never reached the host");
        eprintln!(
            "diag: flood done after {step} seeks, host asked {}",
            origin.segments_asked()
        );

        // Rest on a loaded position, the way thumbfast does: keyframe, then exact.
        std::fs::remove_file(&thumb).ok();
        let rested = Instant::now();
        ipc.command(serde_json::json!(["seek", 10.0, "absolute+keyframes"])).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        ipc.command(serde_json::json!(["seek", 10.0, "absolute+exact"])).await;
        eprintln!("diag: rest seeks sent at {:?}", rested.elapsed());
        assert!(
            drawn(Duration::from_secs(3)).await,
            "the thumbnailer drew nothing where the pointer rested ({segments} segments, host \
             asked {} times)",
            origin.segments_asked()
        );
        // The first picture can be the keyframe seek's, from the keyframe at 8s; the exact
        // seek lands a moment later. Reading the position once raced it on a loaded runner, so
        // it is read until it lands, within the same bound.
        let mut position = f64::NAN;
        while rested.elapsed() < Duration::from_secs(3) {
            let reply = ipc.command(serde_json::json!(["get_property", "time-pos"])).await;
            position = reply["data"].as_f64().unwrap_or(f64::NAN);
            if (8.5..=10.5).contains(&position) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!((8.5..=10.5).contains(&position), "drew {position}s for a rest at 10s");

        ipc.command(serde_json::json!(["quit"])).await;
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    }
}
