//! Fetching from the stream's host: with its headers, held through a rate limit for
//! playback, read whole within a ceiling, and kept for a second read, so one fetch answers
//! every request that wants the same resource.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use super::{Fetched, InFlight, MAX_BODY, Upstream, lock, playlist::looks_like_playlist};

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

/// The largest single resource the cache will keep. Segments are a few megabytes; anything
/// this size is not a segment and would only push a hundred of them out.
const CACHE_ENTRY_LIMIT: usize = 32 * 1024 * 1024;

/// The wait when a `429` does not say how long, or says it in a form not worth parsing.
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(2);

/// Resources fetched whole, kept for whichever player asks next, least recently used first out.
///
/// Keyed by upstream URL. A segment's URL names one immutable piece of media, so a second read
/// can never be stale; a playlist that may still grow is never kept (see [`is_cacheable`]).
#[derive(Default)]
pub(super) struct Cache {
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

impl Upstream {
    pub(super) fn cached(&self, url: &str) -> Option<Arc<Fetched>> {
        lock(&self.cache).get(url)
    }

    /// Run `fetch` as the one fetch of `url`, publishing it for others to wait on while it
    /// runs and keeping the result if it can be served again.
    pub(super) async fn lead(
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
    pub(super) fn begin(&self, url: &str) -> InFlight {
        let cell = InFlight::default();
        lock(&self.in_flight).entry(url.to_owned()).or_insert_with(|| Arc::clone(&cell));
        cell
    }

    /// Fetch for the playing stream: from the cache, else upstream, held through rate limits.
    pub(super) async fn for_playback(&self, url: &str) -> Result<Arc<Fetched>, String> {
        if let Some(hit) = self.cached(url) {
            return Ok(hit);
        }
        // Playback always fetches for itself rather than waiting on another fetch: one led by
        // a thumbnail request gives up on a `429`, and playback must not.
        let cell = self.begin(url);
        self.lead(url, cell, fetch(self, url)).await
    }
}

/// Send one upstream request with the stream's headers, waiting out a rate limit.
///
/// Only `429` is waited on: it is the one status that says "this will work if you ask again
/// later". Once the budget is spent the refusal is returned for the caller to report.
pub(super) async fn send(
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

/// Fetch one upstream URL with the stream's headers, for playback.
async fn fetch(upstream: &Upstream, url: &str) -> Result<Fetched, String> {
    let response = send(upstream, url, None).await?;
    upstream.answered(response.status().as_u16());
    read_body(response).await
}

/// Fetch one upstream URL for the thumbnailer: a single request, never held. A `429` starts
/// the thumbnail cooldown, and only that one.
pub(super) async fn fetch_once(upstream: &Upstream, url: &str) -> Result<Fetched, String> {
    read_body(send_once(upstream, url, None).await?).await
}

/// One upstream request, for the thumbnailer. See [`fetch_once`].
pub(super) async fn send_once(
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use tokio::net::TcpListener;

    use super::*;
    use crate::playback::mend::{tests::transport_stream, *};

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
}
