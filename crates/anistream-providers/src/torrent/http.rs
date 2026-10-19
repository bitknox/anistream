//! A minimal loopback HTTP server that streams one torrent file to a player.
//!
//! mpv needs a URL; librqbit gives an `AsyncRead + AsyncSeek`. This bridges them.
//!
//! librqbit ships an `http-api` feature that would do this, but it exposes the whole
//! torrent *management* surface — add, delete, pause — on the same port. Even on loopback
//! that lets any local process manipulate the session, which sits badly beside the
//! fail-closed VPN guard. So this serves exactly one thing: `GET` on a single unguessable
//! path, read-only, bound to `127.0.0.1` on an ephemeral port.
//!
//! The risky part of writing this by hand is range handling, so that is a pure function
//! with its own tests. Getting it wrong means mpv cannot seek.
//!
//! A server can also carry a second path, on its own token, for seekbar thumbnails. It serves
//! the same file but only the bytes already verified, and never waits for the rest; see
//! [`ReadySource`] for why that cannot simply be another reader on the first path, and
//! [`ReadyReply`] for what it answers when the bytes are not there.

use std::{
    io::SeekFrom,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use tokio::{
    io::{
        AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufReader,
    },
    net::{TcpListener, TcpStream},
};

/// A byte range resolved against a known content length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// First byte, inclusive.
    pub start: u64,
    /// Last byte, inclusive.
    pub end: u64,
}

impl Range {
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start) + 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What a `Range` header asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeRequest {
    /// No `Range` header: the whole file.
    Full,
    /// A satisfiable range.
    Partial(Range),
    /// Syntactically valid but outside the file. Must produce a `416`, not a clamp —
    /// silently returning different bytes than asked for corrupts a seek.
    Unsatisfiable,
}

/// Parse a `Range` header value against a content length.
///
/// Handles the three forms players actually send: `bytes=0-`, `bytes=500-999`, and the
/// suffix form `bytes=-500` meaning the last 500 bytes.
pub fn parse_range(header: Option<&str>, content_length: u64) -> RangeRequest {
    let Some(raw) = header else {
        return RangeRequest::Full;
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        // A unit we do not understand. Serving the whole file is the safe reading.
        return RangeRequest::Full;
    };
    // Multi-range requests are legal but rare from players; serving the first is a
    // reasonable simplification, and serving the whole file would break seeking.
    let spec = spec.split(',').next().unwrap_or("").trim();

    let Some((from, to)) = spec.split_once('-') else {
        return RangeRequest::Full;
    };

    if content_length == 0 {
        return RangeRequest::Unsatisfiable;
    }
    let last = content_length - 1;

    match (from.trim(), to.trim()) {
        // `bytes=-500`: the final 500 bytes.
        ("", suffix) => match suffix.parse::<u64>() {
            Ok(0) => RangeRequest::Unsatisfiable,
            Ok(n) => RangeRequest::Partial(Range {
                start: content_length.saturating_sub(n),
                end: last,
            }),
            Err(_) => RangeRequest::Full,
        },
        // `bytes=500-`: from 500 to the end.
        (start, "") => match start.parse::<u64>() {
            Ok(s) if s <= last => RangeRequest::Partial(Range { start: s, end: last }),
            Ok(_) => RangeRequest::Unsatisfiable,
            Err(_) => RangeRequest::Full,
        },
        // `bytes=500-999`.
        (start, end) => match (start.parse::<u64>(), end.parse::<u64>()) {
            (Ok(s), Ok(e)) if s <= e && s <= last => {
                // A player may ask past the end; clamping the *end* is correct here, unlike
                // clamping the start.
                RangeRequest::Partial(Range { start: s, end: e.min(last) })
            }
            (Ok(_), Ok(_)) => RangeRequest::Unsatisfiable,
            _ => RangeRequest::Full,
        },
    }
}

/// Build the response head for a request.
pub fn response_head(request: RangeRequest, content_length: u64, content_type: &str) -> String {
    match request {
        RangeRequest::Full => format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: {content_type}\r\n\
             Content-Length: {content_length}\r\n\
             Accept-Ranges: bytes\r\n\
             Connection: close\r\n\r\n"
        ),
        RangeRequest::Partial(range) => format!(
            "HTTP/1.1 206 Partial Content\r\n\
             Content-Type: {content_type}\r\n\
             Content-Length: {}\r\n\
             Content-Range: bytes {}-{}/{content_length}\r\n\
             Accept-Ranges: bytes\r\n\
             Connection: close\r\n\r\n",
            range.len(),
            range.start,
            range.end
        ),
        RangeRequest::Unsatisfiable => format!(
            "HTTP/1.1 416 Range Not Satisfiable\r\n\
             Content-Range: bytes */{content_length}\r\n\
             Content-Length: 0\r\n\
             Connection: close\r\n\r\n"
        ),
    }
}

/// Guess a content type from a filename. mpv does not need it, but a browser does.
pub fn content_type_for(name: &str) -> &'static str {
    let lowered = name.to_ascii_lowercase();
    if lowered.ends_with(".mp4") || lowered.ends_with(".m4v") {
        "video/mp4"
    } else if lowered.ends_with(".webm") {
        "video/webm"
    } else if lowered.ends_with(".avi") {
        "video/x-msvideo"
    } else {
        // Matroska is what anime releases overwhelmingly use.
        "video/x-matroska"
    }
}

/// Extract the request target from an HTTP request line.
pub fn request_target(line: &str) -> Option<&str> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    // Read-only by construction: nothing else is even parsed.
    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return None;
    }
    parts.next()
}

/// A reader that can also seek.
///
/// Exists so [`StreamSource`] can hand back a boxed reader instead of naming a concrete
/// type. That matters practically: librqbit's `FileStream` lives in a private module and
/// cannot be named from outside the crate, even though the method returning it is public.
/// Boxing also keeps this module free of any torrent dependency, so it is testable against
/// an in-memory cursor.
pub trait SeekableRead: AsyncRead + AsyncSeek + Send + Unpin {}
impl<T: AsyncRead + AsyncSeek + Send + Unpin> SeekableRead for T {}

/// Opens a fresh reader over the same file for each connection.
pub trait StreamSource: Send + Sync + 'static {
    fn open(&self) -> std::io::Result<Box<dyn SeekableRead>>;
    fn len(&self) -> u64;
    fn name(&self) -> String;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The same file as a [`StreamSource`], limited to bytes that are already verified.
///
/// The seam that keeps seekbar thumbnails from starving playback. Every reader a
/// `StreamSource` opens is a librqbit stream, and librqbit fetches ahead of each open stream
/// in turn — so a thumbnail child seeking all over the file would pull the swarm away from
/// the playback head once per hover. A `ReadySource` has no way to ask for a piece: it says
/// what is on hand and reads that from where it already sits. The thumbnail path is wired
/// to this trait alone, so it cannot open a stream by mistake.
pub trait ReadySource: Send + Sync + 'static {
    /// The leading part of `range` that can be read right now, or `None` when its first byte
    /// cannot. Called once per request before anything is written, and must not wait.
    fn ready(&self, range: Range) -> Option<Range>;

    /// A reader over the whole file that never waits on the swarm. Only bytes
    /// [`Self::ready`] has just vouched for are read from it.
    fn open(&self) -> std::io::Result<Box<dyn SeekableRead>>;
}

/// How the thumbnail path answers one request.
///
/// The client is mpv's thumbnail child, so in practice ffmpeg's HTTP reader with the
/// `reconnect=1` mpv always sets. Each shape below comes from running a thumbfast-style child
/// against a half-verified file and watching what it did with the next seek:
///
/// - **A run that stops short is sent chunked.** A `Content-Length` body that ends before the
///   file does looks truncated to ffmpeg, which logs an error and reconnects for the rest —
///   and once a reconnect fails, it sleeps between tries. A chunked body closed by the
///   last-chunk marker is a clean end of file instead: the demuxer stops there, nothing is
///   retried, and the next seek opens a fresh request.
/// - **A missing first byte gets a 206 with nothing in it**, not an error. ffmpeg handles a
///   failed seek by falling back to the previous connection, but parses the error's headers
///   first and does not restore what they overwrote. A 503 with `Content-Length: 0` left the
///   file's size at zero, after which every seek in that child "succeeded" without requesting
///   anything. Without the header, the previous chunked body was read on as raw bytes, framing
///   and all, and the reconnect loop then slept eleven seconds against more 503s. An empty
///   206 is a seek that succeeds and meets the end of the file at once: the child draws
///   nothing new and the next seek is unaffected. Its `Content-Range` names the one byte that
///   was asked for, the only way a 206 can state a position; the body holding none of it is
///   the signal.
/// - **No `Range` header and an incomplete file is a 503.** ffmpeg always sends a range, so
///   only other clients see this, and they are better served by an error than by a 206
///   they never asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyReply {
    /// All of it is on hand: exactly the response the stream path would give.
    Whole(RangeRequest),
    /// A leading run is on hand: a chunked 206 for that run.
    Run(Range),
    /// The first byte asked for is not on hand: a chunked 206 with no data.
    Nothing { at: u64 },
    /// No range was asked for and the file is not complete.
    Unavailable,
    /// Outside the file, answered as on the stream path.
    Unsatisfiable,
}

/// Decide the thumbnail path's answer, given what `ready` says is on hand.
pub fn ready_reply(
    request: RangeRequest,
    length: u64,
    ready: impl FnOnce(Range) -> Option<Range>,
) -> ReadyReply {
    let asked = match request {
        RangeRequest::Unsatisfiable => return ReadyReply::Unsatisfiable,
        // Nothing to wait for in an empty file.
        RangeRequest::Full if length == 0 => return ReadyReply::Whole(request),
        RangeRequest::Full => Range { start: 0, end: length - 1 },
        RangeRequest::Partial(range) => range,
    };
    // Believed only as far as the request goes: a run starting elsewhere or ending past what
    // was asked would put bytes on the wire that the Content-Range does not describe.
    let run = ready(asked)
        .filter(|run| run.start == asked.start && run.end >= run.start)
        .map(|run| Range { start: asked.start, end: run.end.min(asked.end) });
    match (request, run) {
        (_, Some(run)) if run.end == asked.end => ReadyReply::Whole(request),
        (RangeRequest::Full, _) => ReadyReply::Unavailable,
        (_, Some(run)) => ReadyReply::Run(run),
        (_, None) => ReadyReply::Nothing { at: asked.start },
    }
}

/// Build the response head for a thumbnail-path reply.
pub fn ready_head(reply: ReadyReply, length: u64, content_type: &str) -> String {
    let chunked = |start: u64, end: u64| {
        format!(
            "HTTP/1.1 206 Partial Content\r\n\
             Content-Type: {content_type}\r\n\
             Content-Range: bytes {start}-{end}/{length}\r\n\
             Transfer-Encoding: chunked\r\n\
             Accept-Ranges: bytes\r\n\
             Connection: close\r\n\r\n"
        )
    };
    match reply {
        ReadyReply::Whole(request) => response_head(request, length, content_type),
        ReadyReply::Run(run) => chunked(run.start, run.end),
        ReadyReply::Nothing { at } => chunked(at, at),
        ReadyReply::Unavailable => "HTTP/1.1 503 Service Unavailable\r\n\
                                    Retry-After: 1\r\n\
                                    Content-Length: 0\r\n\
                                    Connection: close\r\n\r\n"
            .to_owned(),
        ReadyReply::Unsatisfiable => {
            response_head(RangeRequest::Unsatisfiable, length, content_type)
        }
    }
}

/// A running stream server.
pub struct StreamServer {
    url: String,
    /// The thumbnail path's URL, when the server carries one.
    thumbnail_url: Option<String>,
    handle: tokio::task::JoinHandle<()>,
    /// Held, never sent on. Every connection task waits for this to close, so dropping the
    /// server *is* the cancellation — there is no signal to remember to send.
    cancel: tokio::sync::watch::Sender<()>,
}

impl StreamServer {
    /// The URL to hand to a player.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The URL for a thumbnail reader: the same file, verified bytes only, never waiting.
    /// `None` when the server was started without a [`ReadySource`].
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnail_url.as_deref()
    }

    /// Stop serving. Playback in progress will end.
    pub fn shutdown(self) {
        self.stop();
    }

    /// Close the listener *and* every connection it spawned.
    ///
    /// Aborting the accept loop alone left the handed-off tasks running, so a server dropped
    /// because the VPN guard started leaking went on feeding an already-connected player from
    /// the torrent — the teardown half of `halt()` did not, in fact, tear anything down. Worse
    /// once the torrent was paused underneath them: the read stalls, no write is attempted, so
    /// a client that has gone away is never noticed and the task, its socket and its torrent
    /// stream stay for the life of the process.
    fn stop(&self) {
        self.handle.abort();
        self.cancel.send_replace(());
    }
}

impl Drop for StreamServer {
    fn drop(&mut self) {
        // The accept loop stops here; the connections stop because `cancel` is dropped with
        // the struct a moment later, which closes the channel they are waiting on.
        self.handle.abort();
    }
}

/// Serve `source` on loopback until dropped.
///
/// The path carries a random token. It is not a security boundary on its own — loopback
/// binding is — but it stops an unrelated local process from stumbling onto the stream by
/// guessing a predictable path.
pub async fn serve<S: StreamSource>(source: S, token: &str) -> std::io::Result<StreamServer> {
    listen(source, token, None).await
}

/// Serve `source` as [`serve`] does, plus a thumbnail path on its own token.
///
/// One server rather than two so the thumbnail path shares the stream's lifetime: dropping
/// the server — a new episode, or the VPN guard's `halt()` — ends both, with the same
/// cancellation, and there is no second listener to forget.
pub async fn serve_with_thumbnails<S: StreamSource, R: ReadySource>(
    source: S,
    token: &str,
    ready: R,
    thumbnail_token: &str,
) -> std::io::Result<StreamServer> {
    listen(source, token, Some((Box::new(ready), thumbnail_token))).await
}

/// What one server answers, by path.
struct Routes<S> {
    stream: S,
    stream_path: String,
    thumbnails: Option<(Box<dyn ReadySource>, String)>,
}

/// Which of a server's paths a request named, holding what serves it.
enum Route<'a, S> {
    Stream(&'a S),
    Thumbnails(&'a dyn ReadySource),
}

impl<S> Routes<S> {
    fn route(&self, target: Option<&str>) -> Option<Route<'_, S>> {
        let path = target.map(|t| t.split('?').next().unwrap_or(t))?;
        if path == self.stream_path {
            return Some(Route::Stream(&self.stream));
        }
        match &self.thumbnails {
            Some((ready, own)) if path == own => Some(Route::Thumbnails(ready.as_ref())),
            _ => None,
        }
    }
}

async fn listen<S: StreamSource>(
    source: S,
    token: &str,
    thumbnails: Option<(Box<dyn ReadySource>, &str)>,
) -> std::io::Result<StreamServer> {
    // Loopback only. Never bind a torrent stream to a routable interface.
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
    let port = listener.local_addr()?.port();
    let path = format!("/s/{token}");
    let url = format!("http://127.0.0.1:{port}{path}");
    let thumbnails = thumbnails.map(|(ready, token)| (ready, format!("/t/{token}")));
    let thumbnail_url =
        thumbnails.as_ref().map(|(_, path)| format!("http://127.0.0.1:{port}{path}"));

    let routes = Arc::new(Routes { stream: source, stream_path: path, thumbnails });
    let (cancel, cancel_rx) = tokio::sync::watch::channel(());

    let handle = tokio::spawn(async move {
        // Accept errors are not all fatal. `EMFILE` in particular is transient and is exactly
        // what a long binge produces — and breaking here retired the listener for good, so the
        // URL kept resolving to a port that refused every connection, with mpv failing on its
        // next range request and nothing logged.
        let mut consecutive_errors = 0_u32;
        loop {
            let socket = match listener.accept().await {
                Ok((socket, _)) => {
                    consecutive_errors = 0;
                    socket
                }
                Err(e) => {
                    consecutive_errors += 1;
                    tracing::warn!(error = %e, consecutive_errors, "stream server accept failed");
                    // A listener that cannot accept anything is broken rather than busy; give
                    // up rather than spin a core on it.
                    if consecutive_errors >= 16 {
                        tracing::error!(
                            "stream server giving up after repeated accept failures"
                        );
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    continue;
                }
            };
            let routes = Arc::clone(&routes);
            let mut cancelled = cancel_rx.clone();
            // One task per connection: mpv opens several, and a stalled read on one must
            // not block the others.
            //
            // Raced against the server going away, because these tasks are detached: aborting
            // the accept loop left them serving the torrent to an already-connected player,
            // which is why dropping the server did not stop the stream it was serving.
            tokio::spawn(async move {
                tokio::select! {
                    result = handle_connection(socket, routes) => {
                        if let Err(e) = result {
                            tracing::debug!(error = %e, "stream connection ended");
                        }
                    }
                    // Resolves when the server is dropped, closing the channel.
                    _ = cancelled.changed() => {
                        tracing::debug!("stream server went away; dropping the connection");
                    }
                }
            });
        }
    });

    tracing::info!(
        %url,
        thumbnails = thumbnail_url.is_some(),
        "torrent stream server listening on loopback"
    );
    Ok(StreamServer { url, thumbnail_url, handle, cancel })
}

async fn handle_connection<S: StreamSource>(
    socket: TcpStream,
    routes: Arc<Routes<S>>,
) -> std::io::Result<()> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    read_line(&mut reader, &mut request_line).await?;

    let Some(route) = routes.route(request_target(&request_line)) else {
        write_half.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await?;
        return Ok(());
    };
    let is_head = request_line.trim_start().to_ascii_uppercase().starts_with("HEAD");

    // Read headers, keeping only Range.
    let mut range_header: Option<String> = None;
    loop {
        let mut line = String::new();
        let n = read_line(&mut reader, &mut line).await?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("range")
        {
            range_header = Some(value.trim().to_owned());
        }
    }

    // Length and name are the stream's on both paths — it is the same file — and neither
    // touches the swarm. Only `open` does, and the thumbnail path never calls it.
    let length = routes.stream.len();
    let content_type = content_type_for(&routes.stream.name());
    let request = parse_range(range_header.as_deref(), length);

    let ready = match route {
        Route::Stream(source) => {
            let head = response_head(request, length, content_type);
            write_half.write_all(head.as_bytes()).await?;

            if matches!(request, RangeRequest::Unsatisfiable) {
                return Ok(());
            }
            if is_head {
                return Ok(());
            }

            let (start, remaining) = match request {
                RangeRequest::Partial(r) => (r.start, r.len()),
                _ => (0, length),
            };

            let mut file = source.open()?;
            if start > 0 {
                file.seek(SeekFrom::Start(start)).await?;
            }
            return copy_exact(&mut file, &mut write_half, remaining).await;
        }
        Route::Thumbnails(ready) => ready,
    };

    let reply = ready_reply(request, length, |range| ready.ready(range));
    let body = match reply {
        ReadyReply::Whole(RangeRequest::Partial(range)) | ReadyReply::Run(range) => Some(range),
        ReadyReply::Whole(RangeRequest::Full) if length > 0 => {
            Some(Range { start: 0, end: length - 1 })
        }
        _ => None,
    };
    // Opened before the head goes out, so a file that has vanished underneath — the torrent
    // forgotten with its data — is still answered with a reply the player handles cleanly.
    let (reply, file) = match body {
        Some(range) if !is_head => match open_at(ready, range.start).await {
            Ok(file) => (reply, Some((file, range))),
            Err(e) => {
                tracing::debug!(error = %e, "thumbnail read failed; answering as not on hand");
                let missing = match request {
                    RangeRequest::Full => ReadyReply::Unavailable,
                    _ => ReadyReply::Nothing { at: range.start },
                };
                (missing, None)
            }
        },
        _ => (reply, None),
    };
    tracing::trace!(?reply, "thumbnail request");

    write_half.write_all(ready_head(reply, length, content_type).as_bytes()).await?;
    if is_head {
        return Ok(());
    }
    match (reply, file) {
        (ReadyReply::Run(_), Some((mut file, range))) => {
            copy_chunked(&mut file, &mut write_half, range.len()).await
        }
        (ReadyReply::Whole(_), Some((mut file, range))) => {
            copy_exact(&mut file, &mut write_half, range.len()).await
        }
        (ReadyReply::Nothing { .. }, _) => {
            write_half.write_all(LAST_CHUNK).await?;
            write_half.flush().await
        }
        _ => Ok(()),
    }
}

/// A reader from `ready`, positioned at `start`.
async fn open_at(
    ready: &dyn ReadySource,
    start: u64,
) -> std::io::Result<Box<dyn SeekableRead>> {
    let mut file = ready.open()?;
    if start > 0 {
        file.seek(SeekFrom::Start(start)).await?;
    }
    Ok(file)
}

/// The marker that ends a chunked body, and with it the response.
const LAST_CHUNK: &[u8] = b"0\r\n\r\n";

/// Read one CRLF-terminated line, with a cap so a hostile client cannot exhaust memory.
async fn read_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut String,
) -> std::io::Result<usize> {
    const MAX: usize = 8 * 1024;
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

/// Copy exactly `amount` bytes, streaming as they become available.
async fn copy_exact<R, W>(reader: &mut R, writer: &mut W, amount: u64) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    // 64 KiB balances syscall overhead against how long a stalled piece blocks the write.
    let mut buffer = vec![0u8; 64 * 1024];
    let mut left = amount;

    while left > 0 {
        let want = buffer.len().min(left as usize);
        let read = reader.read(&mut buffer[..want]).await?;
        if read == 0 {
            // Short read: the torrent ended or the stream was dropped. Closing the
            // connection is the honest signal.
            break;
        }
        writer.write_all(&buffer[..read]).await?;
        left -= read as u64;
    }
    writer.flush().await
}

/// Copy exactly `amount` bytes as a chunked body, then end it.
///
/// Unlike [`copy_exact`], a short read is an error and the body is left unterminated. The
/// last-chunk marker tells the reader it has everything the head promised, which after a
/// short read would be false — and on this path a clean end is exactly what the player
/// believes.
async fn copy_chunked<R, W>(reader: &mut R, writer: &mut W, amount: u64) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = vec![0u8; 64 * 1024];
    let mut left = amount;

    while left > 0 {
        let want = buffer.len().min(left as usize);
        let read = reader.read(&mut buffer[..want]).await?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "verified bytes ended early on disk",
            ));
        }
        writer.write_all(format!("{read:x}\r\n").as_bytes()).await?;
        writer.write_all(&buffer[..read]).await?;
        writer.write_all(b"\r\n").await?;
        left -= read as u64;
    }
    writer.write_all(LAST_CHUNK).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const LEN: u64 = 1000;

    #[test]
    fn no_range_header_means_the_whole_file() {
        assert_eq!(parse_range(None, LEN), RangeRequest::Full);
    }

    #[test]
    fn an_open_ended_range_runs_to_the_last_byte() {
        // What mpv sends first, every time.
        assert_eq!(
            parse_range(Some("bytes=0-"), LEN),
            RangeRequest::Partial(Range { start: 0, end: 999 })
        );
        assert_eq!(
            parse_range(Some("bytes=500-"), LEN),
            RangeRequest::Partial(Range { start: 500, end: 999 })
        );
    }

    #[test]
    fn a_closed_range_is_inclusive_at_both_ends() {
        let r = parse_range(Some("bytes=500-999"), LEN);
        assert_eq!(r, RangeRequest::Partial(Range { start: 500, end: 999 }));
        if let RangeRequest::Partial(range) = r {
            assert_eq!(range.len(), 500, "inclusive: 500..=999 is 500 bytes");
        }
    }

    #[test]
    fn a_suffix_range_returns_the_tail() {
        // mpv uses this to read the moov atom / trailing index.
        assert_eq!(
            parse_range(Some("bytes=-100"), LEN),
            RangeRequest::Partial(Range { start: 900, end: 999 })
        );
        // A suffix longer than the file is the whole file, not an error.
        assert_eq!(
            parse_range(Some("bytes=-5000"), LEN),
            RangeRequest::Partial(Range { start: 0, end: 999 })
        );
    }

    #[test]
    fn a_start_past_the_end_is_unsatisfiable_rather_than_clamped() {
        // Clamping would silently return different bytes than the player asked for, which
        // corrupts a seek instead of failing it.
        assert_eq!(parse_range(Some("bytes=1000-"), LEN), RangeRequest::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=5000-6000"), LEN), RangeRequest::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=-0"), LEN), RangeRequest::Unsatisfiable);
    }

    #[test]
    fn an_end_past_the_last_byte_is_clamped() {
        // Clamping the *end* is correct and expected; clamping the start is not.
        assert_eq!(
            parse_range(Some("bytes=900-99999"), LEN),
            RangeRequest::Partial(Range { start: 900, end: 999 })
        );
    }

    #[test]
    fn a_backwards_range_is_unsatisfiable() {
        assert_eq!(parse_range(Some("bytes=900-100"), LEN), RangeRequest::Unsatisfiable);
    }

    #[test]
    fn a_malformed_or_unknown_range_falls_back_to_the_whole_file() {
        for header in ["bytes=abc-def", "bytes=", "items=0-10", "nonsense", "bytes=x-"] {
            assert_eq!(
                parse_range(Some(header), LEN),
                RangeRequest::Full,
                "unexpected handling of {header:?}"
            );
        }
    }

    #[test]
    fn only_the_first_of_a_multi_range_request_is_served() {
        assert_eq!(
            parse_range(Some("bytes=0-99,200-299"), LEN),
            RangeRequest::Partial(Range { start: 0, end: 99 })
        );
    }

    #[test]
    fn a_zero_length_file_cannot_satisfy_a_range() {
        assert_eq!(parse_range(Some("bytes=0-"), 0), RangeRequest::Unsatisfiable);
    }

    #[test]
    fn a_full_response_advertises_range_support() {
        // Without Accept-Ranges mpv will not attempt to seek at all.
        let head = response_head(RangeRequest::Full, LEN, "video/x-matroska");
        assert!(head.starts_with("HTTP/1.1 200 OK"));
        assert!(head.contains("Accept-Ranges: bytes"));
        assert!(head.contains("Content-Length: 1000"));
    }

    #[test]
    fn a_partial_response_reports_the_exact_range_and_length() {
        let head = response_head(
            RangeRequest::Partial(Range { start: 500, end: 999 }),
            LEN,
            "video/x-matroska",
        );
        assert!(head.starts_with("HTTP/1.1 206 Partial Content"));
        assert!(head.contains("Content-Range: bytes 500-999/1000"));
        assert!(head.contains("Content-Length: 500"), "must be the range length, not the file");
    }

    #[test]
    fn an_unsatisfiable_response_is_a_416_with_the_real_length() {
        let head = response_head(RangeRequest::Unsatisfiable, LEN, "video/x-matroska");
        assert!(head.starts_with("HTTP/1.1 416"));
        assert!(head.contains("Content-Range: bytes */1000"));
    }

    #[test]
    fn content_types_cover_what_releases_actually_use() {
        assert_eq!(content_type_for("ep01.mkv"), "video/x-matroska");
        assert_eq!(content_type_for("EP01.MP4"), "video/mp4");
        assert_eq!(content_type_for("x.webm"), "video/webm");
        // Matroska is the sane default for anime.
        assert_eq!(content_type_for("no-extension"), "video/x-matroska");
    }

    #[test]
    fn only_read_methods_are_accepted() {
        // Read-only by construction: nothing else is even parsed.
        assert_eq!(request_target("GET /s/abc HTTP/1.1"), Some("/s/abc"));
        assert_eq!(request_target("HEAD /s/abc HTTP/1.1"), Some("/s/abc"));
        assert_eq!(request_target("POST /s/abc HTTP/1.1"), None);
        assert_eq!(request_target("DELETE /s/abc HTTP/1.1"), None);
        assert_eq!(request_target("garbage"), None);
    }

    /// An in-memory source, so the server can be exercised without a torrent session.
    struct MemorySource(Vec<u8>);

    impl StreamSource for MemorySource {
        fn open(&self) -> std::io::Result<Box<dyn SeekableRead>> {
            Ok(Box::new(Cursor::new(self.0.clone())))
        }
        fn len(&self) -> u64 {
            self.0.len() as u64
        }
        fn name(&self) -> String {
            "test.mkv".into()
        }
    }

    async fn request(url: &str, range: Option<&str>) -> (String, Vec<u8>) {
        send("GET", url, range).await
    }

    async fn send(method: &str, url: &str, range: Option<&str>) -> (String, Vec<u8>) {
        use tokio::io::AsyncWriteExt;
        let without_scheme = url.trim_start_matches("http://");
        let (authority, path) = without_scheme.split_once('/').unwrap();
        let mut socket = TcpStream::connect(authority).await.unwrap();

        let mut req = format!("{method} /{path} HTTP/1.1\r\nHost: {authority}\r\n");
        if let Some(r) = range {
            req.push_str(&format!("Range: {r}\r\n"));
        }
        req.push_str("\r\n");
        socket.write_all(req.as_bytes()).await.unwrap();

        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(raw.len());
        (
            String::from_utf8_lossy(&raw[..split]).to_string(),
            raw.get(split + 4..).unwrap_or_default().to_vec(),
        )
    }

    #[tokio::test]
    async fn the_server_returns_the_whole_file_by_default() {
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let server = serve(MemorySource(data.clone()), "tok").await.unwrap();

        let (head, body) = request(server.url(), None).await;
        assert!(head.starts_with("HTTP/1.1 200 OK"), "got {head}");
        assert_eq!(body.len(), 4096);
        assert_eq!(body, data);
    }

    #[tokio::test]
    async fn the_server_honours_a_range_so_seeking_works() {
        // This is the property mpv depends on for seeking to work at all.
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let server = serve(MemorySource(data.clone()), "tok").await.unwrap();

        let (head, body) = request(server.url(), Some("bytes=1000-1099")).await;
        assert!(head.starts_with("HTTP/1.1 206"), "got {head}");
        assert!(head.contains("Content-Range: bytes 1000-1099/4096"));
        assert_eq!(body.len(), 100);
        assert_eq!(body, &data[1000..1100], "wrong bytes returned for the range");
    }

    #[tokio::test]
    async fn the_server_rejects_an_unsatisfiable_range() {
        let server = serve(MemorySource(vec![0; 100]), "tok").await.unwrap();
        let (head, body) = request(server.url(), Some("bytes=500-")).await;
        assert!(head.starts_with("HTTP/1.1 416"));
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn the_server_serves_only_its_own_path() {
        // The token is not a security boundary — loopback is — but it stops an unrelated
        // local process from stumbling onto the stream.
        let server = serve(MemorySource(vec![1, 2, 3]), "secret").await.unwrap();
        let wrong = server.url().replace("secret", "guessed");
        let (head, _) = request(&wrong, None).await;
        assert!(head.starts_with("HTTP/1.1 404"), "got {head}");
    }

    #[tokio::test]
    async fn the_server_binds_loopback_only() {
        // Never expose a torrent stream on a routable interface.
        let server = serve(MemorySource(vec![0; 10]), "tok").await.unwrap();
        assert!(server.url().starts_with("http://127.0.0.1:"), "got {}", server.url());
    }

    #[tokio::test]
    async fn dropping_the_server_stops_a_connection_already_serving() {
        // Dropping it aborted the accept loop and nothing else, so a player already connected
        // kept being fed from the torrent — the teardown half of the VPN guard's `halt()` did
        // not tear anything down. Worse once the torrent was paused underneath: the read
        // stalls, nothing is ever written, and the dead connection is never noticed.
        let server = serve(MemorySource(vec![7; 8 * 1024 * 1024]), "tok").await.unwrap();
        let url = server.url().to_owned();
        let authority = url.trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let path = url[url.find("/s/").unwrap()..].to_owned();

        let mut socket = TcpStream::connect(&authority).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        // Wait until it is genuinely mid-response before pulling the server away.
        let mut head = [0u8; 16];
        socket.read_exact(&mut head).await.unwrap();

        drop(server);

        let mut rest = Vec::new();
        let finished = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            socket.read_to_end(&mut rest),
        )
        .await;
        assert!(finished.is_ok(), "the connection outlived the server that spawned it");
        // Truncation is the point, not merely termination: without the cancellation the task
        // carries on and delivers the whole eight megabytes, and a client that keeps reading —
        // as this one does — would never notice the server was supposed to be gone.
        assert!(
            rest.len() < 8 * 1024 * 1024,
            "the whole body arrived after the server was dropped ({} bytes)",
            rest.len()
        );
    }

    #[tokio::test]
    async fn dropping_the_server_stops_it_listening() {
        let server = serve(MemorySource(vec![0; 10]), "tok").await.unwrap();
        let url = server.url().to_owned();
        drop(server);
        // Give the abort a moment to take effect.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let authority = url.trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let mut socket = match TcpStream::connect(&authority).await {
            Ok(s) => s,
            // Refused outright is the ideal outcome.
            Err(_) => return,
        };
        // Otherwise the accept loop is gone, so no response ever arrives.
        socket.write_all(b"GET / HTTP/1.1\r\n\r\n").await.ok();
        let mut buf = Vec::new();
        let read = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            socket.read_to_end(&mut buf),
        )
        .await;
        assert!(read.is_err() || buf.is_empty(), "server still responding after drop");
    }

    // The thumbnail path.

    /// The leading run of `true` bytes from `range.start`: what a correct source reports.
    fn leading_run(have: &[bool], range: Range) -> Option<Range> {
        if !have.get(range.start as usize).copied().unwrap_or(false) {
            return None;
        }
        let mut end = range.start;
        while end < range.end && have.get(end as usize + 1).copied().unwrap_or(false) {
            end += 1;
        }
        Some(Range { start: range.start, end })
    }

    #[test]
    fn the_reply_follows_the_policy_for_every_small_file() {
        // Every length, availability pattern and request in a small space, against the policy
        // stated plainly: all of it → as the stream path; a leading run → that run; nothing at
        // the start → an empty 206; no range and incomplete → 503.
        for length in 1..=7_u64 {
            for pattern in 0..(1_u32 << length) {
                let have: Vec<bool> = (0..length).map(|b| pattern & (1 << b) != 0).collect();
                let mut requests = vec![RangeRequest::Full, RangeRequest::Unsatisfiable];
                for start in 0..length {
                    for end in start..length {
                        requests.push(RangeRequest::Partial(Range { start, end }));
                    }
                }
                for request in requests {
                    let got = ready_reply(request, length, |r| leading_run(&have, r));
                    let expected = match request {
                        RangeRequest::Unsatisfiable => ReadyReply::Unsatisfiable,
                        RangeRequest::Full if have.iter().all(|b| *b) => {
                            ReadyReply::Whole(request)
                        }
                        RangeRequest::Full => ReadyReply::Unavailable,
                        RangeRequest::Partial(range) => match leading_run(&have, range) {
                            None => ReadyReply::Nothing { at: range.start },
                            Some(run) if run == range => ReadyReply::Whole(request),
                            Some(run) => ReadyReply::Run(run),
                        },
                    };
                    assert_eq!(got, expected, "length {length}, have {have:?}, {request:?}");
                }
            }
        }
    }

    #[test]
    fn a_source_cannot_widen_what_is_sent() {
        // The Content-Range is built from the reply, so a run reported from elsewhere or past
        // the request must not reach the wire as such.
        let asked = RangeRequest::Partial(Range { start: 10, end: 19 });
        let elsewhere = ready_reply(asked, 100, |_| Some(Range { start: 0, end: 50 }));
        assert_eq!(elsewhere, ReadyReply::Nothing { at: 10 });
        let past = ready_reply(asked, 100, |_| Some(Range { start: 10, end: 99 }));
        assert_eq!(past, ReadyReply::Whole(asked));
        let backwards = ready_reply(asked, 100, |_| Some(Range { start: 10, end: 5 }));
        assert_eq!(backwards, ReadyReply::Nothing { at: 10 });
    }

    #[test]
    fn an_empty_file_is_whole_without_asking_the_source() {
        let reply = ready_reply(RangeRequest::Full, 0, |_| panic!("nothing to ask about"));
        assert_eq!(reply, ReadyReply::Whole(RangeRequest::Full));
    }

    #[test]
    fn a_short_run_is_a_chunked_206_for_exactly_that_run() {
        let head =
            ready_head(ReadyReply::Run(Range { start: 100, end: 199 }), LEN, "video/mp4");
        assert!(head.starts_with("HTTP/1.1 206 Partial Content\r\n"));
        assert!(head.contains("Content-Range: bytes 100-199/1000\r\n"));
        assert!(head.contains("Transfer-Encoding: chunked\r\n"));
        // A length alongside chunking is a protocol error, and a length is what made ffmpeg
        // read the early end as a truncation.
        assert!(!head.contains("Content-Length"));
    }

    #[test]
    fn nothing_on_hand_is_an_empty_206_at_the_requested_byte_not_an_error() {
        let head = ready_head(ReadyReply::Nothing { at: 500 }, LEN, "video/mp4");
        assert!(head.starts_with("HTTP/1.1 206"), "got {head}");
        assert!(head.contains("Content-Range: bytes 500-500/1000\r\n"));
        assert!(head.contains("Transfer-Encoding: chunked\r\n"));
        // The file's real size is in it, so ffmpeg keeps knowing how long the file is — the
        // 503 with `Content-Length: 0` this replaces left it believing zero.
        assert!(!head.contains("Content-Length"));
    }

    #[test]
    fn whole_replies_are_byte_for_byte_the_stream_paths() {
        for request in [RangeRequest::Full, RangeRequest::Partial(Range { start: 3, end: 9 })] {
            assert_eq!(
                ready_head(ReadyReply::Whole(request), LEN, "video/mp4"),
                response_head(request, LEN, "video/mp4")
            );
        }
        assert_eq!(
            ready_head(ReadyReply::Unsatisfiable, LEN, "video/mp4"),
            response_head(RangeRequest::Unsatisfiable, LEN, "video/mp4")
        );
        assert!(
            ready_head(ReadyReply::Unavailable, LEN, "video/mp4").starts_with("HTTP/1.1 503")
        );
    }

    /// An in-memory file with byte-level availability, for the thumbnail path.
    struct MemoryReady {
        data: Vec<u8>,
        have: Vec<bool>,
        /// Bytes missing from the end of what `open` returns, to fake a file cut short on disk.
        short_by: usize,
    }

    impl ReadySource for MemoryReady {
        fn ready(&self, range: Range) -> Option<Range> {
            leading_run(&self.have, range)
        }
        fn open(&self) -> std::io::Result<Box<dyn SeekableRead>> {
            let end = self.data.len() - self.short_by;
            Ok(Box::new(Cursor::new(self.data[..end].to_vec())))
        }
    }

    /// A reader that never yields a byte: a torrent stream waiting on a piece that never comes.
    struct Starved;

    impl AsyncRead for Starved {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    impl AsyncSeek for Starved {
        fn start_seek(self: std::pin::Pin<&mut Self>, _: SeekFrom) -> std::io::Result<()> {
            Ok(())
        }
        fn poll_complete(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<u64>> {
            std::task::Poll::Ready(Ok(0))
        }
    }

    /// The prioritising side, instrumented: counts every stream opened and starves each one.
    struct WatchedStream {
        len: u64,
        opens: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl StreamSource for WatchedStream {
        fn open(&self) -> std::io::Result<Box<dyn SeekableRead>> {
            self.opens.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Box::new(Starved))
        }
        fn len(&self) -> u64 {
            self.len
        }
        fn name(&self) -> String {
            "episode.mkv".into()
        }
    }

    const FILE: usize = 64 * 1024;

    /// A server over a 64 KiB file whose bytes are on hand except in `missing`.
    async fn thumbnail_server(
        missing: std::ops::Range<usize>,
        short_by: usize,
    ) -> (StreamServer, Vec<u8>, Arc<std::sync::atomic::AtomicUsize>) {
        let data: Vec<u8> = (0..FILE).map(|i| (i % 251) as u8).collect();
        let have = (0..FILE).map(|i| !missing.contains(&i)).collect();
        let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let stream = WatchedStream { len: FILE as u64, opens: Arc::clone(&opens) };
        let ready = MemoryReady { data: data.clone(), have, short_by };
        let server = serve_with_thumbnails(stream, "stream", ready, "thumbs").await.unwrap();
        (server, data, opens)
    }

    /// Decode a chunked body: the data, and whether the last-chunk marker closed it.
    fn dechunk(mut raw: &[u8]) -> (Vec<u8>, bool) {
        let mut data = Vec::new();
        loop {
            let Some(eol) = raw.windows(2).position(|w| w == b"\r\n") else {
                return (data, false);
            };
            let size = usize::from_str_radix(std::str::from_utf8(&raw[..eol]).unwrap(), 16)
                .expect("chunk size");
            raw = &raw[eol + 2..];
            if size == 0 {
                return (data, raw == b"\r\n");
            }
            if raw.len() < size + 2 {
                data.extend_from_slice(&raw[..raw.len().min(size)]);
                return (data, false);
            }
            data.extend_from_slice(&raw[..size]);
            raw = &raw[size + 2..];
        }
    }

    /// A request that must be answered promptly: the thumbnail path never waits.
    async fn prompt(method: &str, url: &str, range: Option<&str>) -> (String, Vec<u8>) {
        tokio::time::timeout(std::time::Duration::from_secs(5), send(method, url, range))
            .await
            .expect("the thumbnail path waited instead of answering")
    }

    #[tokio::test]
    async fn a_range_wholly_on_hand_is_served_like_the_stream_path() {
        let (server, data, _) = thumbnail_server(40_000..50_000, 0).await;
        let url = server.thumbnail_url().unwrap();
        let (head, body) = prompt("GET", url, Some("bytes=1000-1999")).await;
        assert!(head.starts_with("HTTP/1.1 206"), "got {head}");
        assert!(head.contains("Content-Range: bytes 1000-1999/65536"));
        assert!(head.contains("Content-Length: 1000"));
        assert_eq!(body, &data[1000..2000]);
    }

    #[tokio::test]
    async fn a_range_running_into_a_gap_serves_the_run_and_ends_cleanly() {
        let (server, data, _) = thumbnail_server(40_000..50_000, 0).await;
        let (head, body) =
            prompt("GET", server.thumbnail_url().unwrap(), Some("bytes=30000-")).await;
        assert!(head.contains("Content-Range: bytes 30000-39999/65536"), "got {head}");
        let (served, terminated) = dechunk(&body);
        assert_eq!(served, &data[30_000..40_000]);
        assert!(terminated, "the run must end with the last-chunk marker, read as a clean EOF");
    }

    #[tokio::test]
    async fn a_range_starting_in_a_gap_is_answered_at_once_with_nothing() {
        let (server, _, _) = thumbnail_server(40_000..50_000, 0).await;
        let (head, body) =
            prompt("GET", server.thumbnail_url().unwrap(), Some("bytes=45000-")).await;
        assert!(head.starts_with("HTTP/1.1 206"), "got {head}");
        assert!(head.contains("Content-Range: bytes 45000-45000/65536"));
        assert_eq!(dechunk(&body), (Vec::new(), true));
    }

    #[tokio::test]
    async fn the_thumbnail_path_never_opens_the_prioritising_stream() {
        // The property everything else exists for. Every reader the stream source opens is a
        // librqbit stream that pulls the swarm toward it; this one counts them and starves
        // each, so a thumbnail request that leaked onto it would both show up in the count
        // and hang past `prompt`'s deadline.
        let (server, _, opens) = thumbnail_server(40_000..50_000, 0).await;
        let url = server.thumbnail_url().unwrap().to_owned();
        for range in [
            None,
            Some("bytes=0-"),
            Some("bytes=0-99"),
            Some("bytes=39000-"),
            Some("bytes=45000-46000"),
            Some("bytes=-100"),
            Some("bytes=99999-"),
            Some("nonsense"),
        ] {
            prompt("GET", &url, range).await;
            prompt("HEAD", &url, range).await;
        }
        assert_eq!(opens.load(std::sync::atomic::Ordering::SeqCst), 0);

        // The control: the stream path does open one, so the counter is wired up.
        let stream_url = server.url().to_owned();
        let pending = tokio::spawn(async move { request(&stream_url, Some("bytes=0-")).await });
        for _ in 0..100 {
            if opens.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(opens.load(std::sync::atomic::Ordering::SeqCst), 1);
        pending.abort();
    }

    #[tokio::test]
    async fn a_head_request_gets_the_reply_head_and_no_body() {
        let (server, _, _) = thumbnail_server(40_000..50_000, 0).await;
        let url = server.thumbnail_url().unwrap();
        let (head, body) = prompt("HEAD", url, Some("bytes=30000-")).await;
        assert!(head.contains("Content-Range: bytes 30000-39999/65536"), "got {head}");
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn an_incomplete_file_asked_for_whole_is_a_503() {
        let (server, _, _) = thumbnail_server(40_000..50_000, 0).await;
        let (head, body) = prompt("GET", server.thumbnail_url().unwrap(), None).await;
        assert!(head.starts_with("HTTP/1.1 503"), "got {head}");
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn a_run_cut_short_on_disk_is_never_closed_as_complete() {
        // The last-chunk marker tells the player it has everything the head promised. If the
        // file on disk holds less, the body must break off instead.
        let (server, _, _) = thumbnail_server(60_000..65_536, 10_000).await;
        let url = server.thumbnail_url().unwrap();
        let (head, body) = prompt("GET", url, Some("bytes=1000-")).await;
        assert!(head.contains("Content-Range: bytes 1000-59999/65536"), "got {head}");
        let (served, terminated) = dechunk(&body);
        assert!(!terminated, "a short read was dressed up as a clean end");
        assert_eq!(
            served.len(),
            FILE - 10_000 - 1000,
            "everything the disk held, then a break"
        );
    }

    #[tokio::test]
    async fn each_path_answers_only_to_its_own_token() {
        let (server, _, _) = thumbnail_server(0..0, 0).await;
        let thumbs = server.thumbnail_url().unwrap().to_owned();
        assert!(thumbs.starts_with("http://127.0.0.1:"), "loopback only, got {thumbs}");
        assert_ne!(thumbs, server.url());

        for wrong in [
            thumbs.replace("thumbs", "guessed"),
            thumbs.replace("/t/", "/s/"),
            server.url().replace("/s/", "/t/"),
        ] {
            let (head, _) = prompt("GET", &wrong, None).await;
            assert!(head.starts_with("HTTP/1.1 404"), "{wrong} answered {head}");
        }
    }

    /// mpv's JSON IPC, just enough to drive a thumbnail child.
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
            for _ in 0..200 {
                if let Ok(socket) = tokio::net::UnixStream::connect(path).await {
                    let (read, writer) = socket.into_split();
                    return Self { lines: BufReader::new(read).lines(), writer, next_id: 0 };
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            panic!("the thumbnail child never opened its IPC socket");
        }

        /// Run a command, panicking if the child does not answer within two seconds — an
        /// unanswered command is the hang this whole path exists to rule out.
        async fn command(&mut self, args: serde_json::Value) -> serde_json::Value {
            self.next_id += 1;
            let id = self.next_id;
            let line = serde_json::json!({ "command": args, "request_id": id }).to_string();
            self.writer.write_all(format!("{line}\n").as_bytes()).await.unwrap();
            let reply = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while let Some(line) = self.lines.next_line().await.unwrap() {
                    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
                    if value["request_id"] == id {
                        return value;
                    }
                }
                panic!("the thumbnail child closed its IPC socket");
            })
            .await;
            reply.unwrap_or_else(|_| panic!("the thumbnail child stopped answering: {args}"))
        }
    }

    /// Wait for the child to write a whole thumbnail, as thumbfast does: by the file appearing.
    #[cfg(unix)]
    async fn drawn_within(
        path: &std::path::Path,
        size: u64,
        wait: std::time::Duration,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + wait;
        while tokio::time::Instant::now() < deadline {
            if std::fs::metadata(path).is_ok_and(|m| m.len() == size) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        false
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn a_thumbnail_child_keeps_working_through_seeks_into_missing_pieces() {
        // A thumbfast-style mpv child against the thumbnail path of a file with a hole in the
        // middle. The reply policy was chosen by watching ffmpeg's HTTP reader do exactly
        // this; here it is held to it. An error reply for the hole left the child either
        // believing the file was empty — every later seek a silent no-op — or asleep for
        // eleven seconds in its reconnect loop, and either fails this test.
        let dir = std::env::temp_dir().join(format!("as-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fixture = dir.join("f.mkv");
        // One keyframe a second, so a keyframe seek lands within a second of its target.
        let encoded = tokio::process::Command::new("mpv")
            .args(["--no-config", "--really-quiet", "--no-audio"])
            .arg("av://lavfi:testsrc2=size=320x180:rate=24:duration=60")
            .args(["--of=matroska", "--ovc=mpeg4", "--ovcopts=g=24,qscale=4"])
            .arg(format!("--o={}", fixture.display()))
            .status()
            .await;
        match encoded {
            Err(_) => {
                // CI's Linux job installs mpv and sets this, so a missing mpv fails there
                // instead of passing as a skip.
                assert!(
                    std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                    "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
                );
                eprintln!("skipping: mpv is not on PATH");
                return;
            }
            Ok(status) if !status.success() => {
                eprintln!("skipping: this mpv cannot encode a fixture");
                return;
            }
            Ok(_) => {}
        }

        // The middle fifth is missing; the head and the tail (Matroska's index) are on hand.
        // testsrc2 encodes at a near-constant rate, so bytes stand in for seconds.
        let data = std::fs::read(&fixture).unwrap();
        let len = data.len();
        let have = (0..len).map(|i| !(len * 4 / 10..len * 6 / 10).contains(&i)).collect();
        let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let stream = WatchedStream { len: len as u64, opens: Arc::clone(&opens) };
        let ready = MemoryReady { data, have, short_by: 0 };
        let server = serve_with_thumbnails(stream, "s", ready, "t").await.unwrap();

        let ipc_path = dir.join("ipc");
        let thumb = dir.join("thumb.bgra");
        let frame = 320 * 180 * 4;
        // thumbfast's own arguments, less the ones that only matter inside a real player.
        let mut child = tokio::process::Command::new("mpv")
            .args(["--no-config", "--msg-level=all=no", "--idle", "--pause"])
            .args(["--keep-open=always", "--really-quiet", "--no-terminal"])
            .args(["--load-scripts=no", "--osc=no", "--ytdl=no", "--no-sub", "--no-audio"])
            .args(["--start=6", "--hr-seek=no", "--demuxer-readahead-secs=0"])
            .args(["--demuxer-max-bytes=128KiB", "--vd-lavc-fast", "--hwdec=no"])
            .args(["--vf=scale=320:-2,format=bgra", "--ovc=rawvideo", "--of=image2"])
            .arg("--ofopts=update=1")
            .arg(format!("--o={}", thumb.display()))
            .arg(format!("--input-ipc-server={}", ipc_path.display()))
            .args(["--", server.thumbnail_url().unwrap()])
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut ipc = Ipc::connect(&ipc_path).await;
        assert!(
            drawn_within(&thumb, frame, std::time::Duration::from_secs(5)).await,
            "no first thumbnail"
        );

        // Seconds, against the hole at 24–36 s: in, out, in, out, and back to the start.
        for (target, on_hand) in [
            (48.0, true),
            (30.0, false),
            (12.0, true),
            (33.0, false),
            (54.0, true),
            (2.0, true),
        ] {
            std::fs::remove_file(&thumb).ok();
            ipc.command(serde_json::json!(["seek", target, "absolute+keyframes"])).await;
            if on_hand {
                assert!(
                    drawn_within(&thumb, frame, std::time::Duration::from_secs(4)).await,
                    "the seek to {target}s drew nothing: an earlier seek poisoned the child"
                );
                let position =
                    ipc.command(serde_json::json!(["get_property", "time-pos"])).await;
                let position = position["data"].as_f64().unwrap_or(f64::NAN);
                assert!(
                    (target - 1.5..=target + 0.5).contains(&position),
                    "the seek to {target}s landed at {position}s"
                );
            } else {
                // Whatever the child shows for a hole is mpv's business; that it is still
                // listening afterwards is this server's.
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                ipc.command(serde_json::json!(["get_property", "time-pos"])).await;
            }
        }
        assert_eq!(opens.load(std::sync::atomic::Ordering::SeqCst), 0);

        ipc.command(serde_json::json!(["quit"])).await;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await;
        drop(server);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_server_without_thumbnails_has_no_thumbnail_path() {
        let server = serve(MemorySource(vec![1, 2, 3]), "tok").await.unwrap();
        assert!(server.thumbnail_url().is_none());
        let probe = server.url().replace("/s/", "/t/");
        let (head, _) = request(&probe, None).await;
        assert!(head.starts_with("HTTP/1.1 404"), "got {head}");
    }
}
