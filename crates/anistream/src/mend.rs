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
//! **What it is not.** Not a cache, not a rewriter of media, and not a way around encryption:
//! an `#EXT-X-KEY` is proxied verbatim so encrypted HLS keeps working exactly as it did, and
//! nothing attempts to decrypt anything.

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
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
    for offset in 0..limit {
        if is_transport_stream(data, offset) || is_iso_bmff(data, offset) {
            return offset;
        }
    }
    0
}

/// Whether a transport stream starts at `offset`: `0x47` every 188 bytes, several times over.
fn is_transport_stream(data: &[u8], offset: usize) -> bool {
    if data.get(offset) != Some(&0x47) {
        return false;
    }
    // The tail of a segment is not a place to start looking: a lone `0x47` near the end could
    // not be confirmed, and confirming is the whole point.
    (1..=TS_CONFIRMATIONS).all(|packet| {
        let at = offset + packet * TS_PACKET;
        // Missing bytes count as confirmation only when the data genuinely ended, so a short
        // final segment still validates.
        at >= data.len() || data.get(at) == Some(&0x47)
    }) && offset + TS_PACKET < data.len()
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
    handle: tokio::task::JoinHandle<()>,
}

impl MendServer {
    /// The URL to hand to the player, in place of the stream's own.
    pub fn url(&self) -> &str {
        &self.url
    }
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
}

impl Upstream {
    /// The loopback URL that will fetch and mend `url`.
    fn proxied(&self, url: &str) -> String {
        format!("http://127.0.0.1:{}/m/{}/{}", self.port, self.token, encode_path(url))
    }
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

    let upstream = Arc::new(Upstream { http, headers, token: token.to_owned(), port });
    let entry = upstream.proxied(url);

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
    Ok(MendServer { url: entry, handle })
}

async fn handle_connection(socket: TcpStream, upstream: Arc<Upstream>) -> std::io::Result<()> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    read_line(&mut reader, &mut request_line).await?;

    // Drain the rest of the request. Nothing in it is honoured: a mended body has different
    // offsets from the upstream one, so a range would name bytes that no longer mean the same
    // thing, and every HLS segment is fetched whole anyway.
    loop {
        let mut line = String::new();
        let n = read_line(&mut reader, &mut line).await?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
    }

    let Some(target) = request_target(&request_line) else {
        write_half
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n")
            .await?;
        return Ok(());
    };

    let prefix = format!("/m/{}/", upstream.token);
    let Some(encoded) = target.strip_prefix(&prefix) else {
        write_half.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await?;
        return Ok(());
    };
    let url = decode_path(encoded);

    match fetch(&upstream, &url).await {
        Ok((content_type, body)) => {
            let body = if looks_like_playlist(&content_type, &body) {
                let text = String::from_utf8_lossy(&body);
                let proxy = |u: &str| upstream.proxied(u);
                rewrite_playlist(&text, &url, &proxy).into_bytes()
            } else {
                let start = payload_start(&body);
                if start > 0 {
                    tracing::debug!(url = %url, stripped = start, "mended a disguised segment");
                }
                body[start..].to_vec()
            };

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
        Err(e) => {
            tracing::debug!(url = %url, error = %e, "mender upstream failed");
            // The upstream's failure, reported as one. mpv treats 502 as a dead segment and
            // says so, which is more honest than an empty 200.
            write_half
                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                .await?;
            Ok(())
        }
    }
}

/// Fetch one upstream URL with the stream's headers.
async fn fetch(upstream: &Upstream, url: &str) -> Result<(String, Vec<u8>), String> {
    let mut request = upstream.http.emulated().get(url);
    for (name, value) in &upstream.headers {
        request = request.header(name.as_str(), value.as_str());
    }

    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();

    let body = response.bytes().await.map_err(|e| e.to_string())?;
    if body.len() > MAX_BODY {
        return Err(format!("body of {} bytes is beyond what this proxies", body.len()));
    }
    Ok((content_type, body.to_vec()))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four TS packets, the shape a real segment has.
    fn transport_stream() -> Vec<u8> {
        let mut data = Vec::new();
        for _ in 0..6 {
            data.push(0x47);
            data.extend(std::iter::repeat_n(0xAB, TS_PACKET - 1));
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

    #[tokio::test]
    async fn the_proxy_binds_loopback_only() {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let server =
            serve(http, "https://example.test/x.m3u8", Vec::new(), "tok").await.unwrap();
        assert!(server.url().starts_with("http://127.0.0.1:"), "got {}", server.url());
    }
}
