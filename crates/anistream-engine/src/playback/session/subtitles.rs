//! A stream's subtitles, made ready before mpv opens it: each external track fetched with the
//! stream's own headers and written beside the IPC socket, a track that cannot be fetched left
//! out, and a WebVTT file mended where its conversion broke it.

use std::time::Duration;

use anistream_core::stream::{Stream, Subtitle};
use anistream_net::HttpClient;
use futures::StreamExt;
use tokio::sync::mpsc;

/// Download a stream's external subtitles and point it at the local copies.
///
/// **A stream's headers are its subtitles' headers.** A provider returns whatever a track needs
/// alongside the stream it belongs to, and getting that right is the provider's job, not this
/// one's. So nothing is guessed here: each track is requested with exactly what the stream
/// declared, through the emulated client, so a subtitle behind the same protection as its video
/// is fetched the same way the video would be.
///
/// **What this buys over handing the URL to mpv** is that a track which cannot actually be
/// retrieved is never offered. A provider may list a track that its chosen stream's headers do
/// not open — listings are often scoped to the episode rather than to the individual stream — and
/// mpv would accept that URL, fail quietly, and leave an empty track for the user to cycle onto.
/// Fetching first turns that into a track that is simply absent.
///
/// Files land beside the IPC socket, in a directory the player already owns.
///
/// **Playback waits for them no longer than `wait`.** A subtitle host that takes its time, or
/// never answers, used to hold the file back for as long as every round of fetches took to time
/// out, which with a dozen languages was a minute before the next stream opened after one that
/// failed. What has arrived by then is pointed at; what has not is fetched on, and comes back
/// as [`LateSubtitles`], for the player to be given each track as it is cached.
pub(super) async fn localise_subtitles(
    http: &HttpClient,
    scratch: &std::path::Path,
    stream: &mut Stream,
    wait: Duration,
) -> Option<LateSubtitles> {
    let soft = stream.subtitles.iter().filter(|s| !s.hard).count();
    if soft == 0 {
        return None;
    }

    let dir = scratch.join("subs");
    if tokio::fs::create_dir_all(&dir).await.is_err() {
        return None;
    }

    // Exactly what the stream declared, because that is what the subtitle host wants too.
    let headers = stream.headers.clone();
    let http = http.clone();

    // Several tracks at once: playback waits on this, and a stream listing a dozen languages
    // was a dozen round trips back to back before mpv could open. `buffered` keeps the tracks
    // in their listed order, which is the order mpv offers them in, and the bound keeps a long
    // list from opening a connection per language against the subtitle host.
    let tracks = std::mem::take(&mut stream.subtitles).into_iter().enumerate();
    let (found, mut arriving) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let (http, headers, dir) = (&http, &headers, &dir);
        let localised = futures::stream::iter(tracks)
            .map(|(index, subtitle)| localise_one(http, headers, dir, index, subtitle))
            .buffered(SUBTITLE_FETCHES)
            .filter_map(std::future::ready);
        futures::pin_mut!(localised);
        while let Some(subtitle) = localised.next().await {
            // Nobody waits for it any more: the attempt it was for is over.
            if found.send(subtitle).is_err() {
                return;
            }
        }
    });

    let deadline = tokio::time::Instant::now() + wait;
    loop {
        match tokio::time::timeout_at(deadline, arriving.recv()).await {
            Ok(Some(subtitle)) => stream.subtitles.push(subtitle),
            // Every track in, or out.
            Ok(None) => return None,
            Err(_) => {
                tracing::info!(
                    ready = stream.subtitles.len(),
                    "opening the file with the subtitles that have arrived; the rest follow"
                );
                return Some(LateSubtitles(arriving));
            }
        }
    }
}

/// How long the file waits for its subtitles before it opens without the ones still coming.
pub(super) const SUBTITLE_WAIT: Duration = Duration::from_secs(2);

/// A stream's subtitle tracks still being fetched once its file opened without them, each
/// handed over as it is cached.
pub(super) struct LateSubtitles(mpsc::UnboundedReceiver<Subtitle>);

impl LateSubtitles {
    /// The next track to arrive, or `None` once every one has, or been left out.
    pub(super) async fn next(&mut self) -> Option<Subtitle> {
        self.0.recv().await
    }
}

/// One track, fetched with the stream's `headers` and cached in `dir` as the `index`th: its
/// local copy, or `None` for one that cannot be fetched.
async fn localise_one(
    http: &HttpClient,
    headers: &[(String, String)],
    dir: &std::path::Path,
    index: usize,
    mut subtitle: Subtitle,
) -> Option<Subtitle> {
    // Already burned in, or already a local path — nothing to fetch either way.
    if subtitle.hard || !subtitle.url.starts_with("http") {
        return Some(subtitle);
    }

    let mut request = http.emulated().get(&subtitle.url);
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }

    let fetched = match request.send().await {
        Ok(response) if response.status().is_success() => response.bytes().await.ok(),
        Ok(response) => {
            tracing::debug!(url = %subtitle.url, status = %response.status(), "subtitle refused");
            None
        }
        Err(error) => {
            tracing::debug!(url = %subtitle.url, %error, "subtitle fetch failed");
            None
        }
    };

    // Dropped rather than passed along. mpv would request it with the same headers and get
    // the same refusal, so keeping it only buys the user an empty track to cycle onto.
    let Some(body) = fetched.filter(|b| !b.is_empty()) else {
        tracing::debug!(language = %subtitle.language, "subtitle dropped: could not be fetched");
        return None;
    };

    // The extension is what tells mpv which parser to use — `.vtt` and `.ass` are both
    // common and are not interchangeable. The source's own claim wins, because an
    // API-shaped URL (`/subtitles?id=…`) carries no extension to read; the URL is the
    // fallback, and `vtt` the guess of last resort.
    let plausible = |ext: &String| {
        !ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric())
    };
    let extension = subtitle
        .format
        .as_deref()
        .map(|format| format.trim_start_matches('.').to_ascii_lowercase())
        .filter(plausible)
        .or_else(|| {
            subtitle
                .url
                .rsplit('/')
                .next()
                .and_then(|name| name.rsplit_once('.'))
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .filter(plausible)
        })
        .unwrap_or_else(|| "vtt".into());
    // Indexed rather than hashed: two tracks in the same language would otherwise collide
    // and the second would silently replace the first.
    let path = dir.join(format!("{index}-{}.{extension}", sanitise(&subtitle.language)));

    let mended = (extension == "vtt")
        .then(|| std::str::from_utf8(&body).ok().and_then(mend_webvtt))
        .flatten();
    if mended.is_some() {
        tracing::debug!(language = %subtitle.language, "subtitle mended");
    }
    let contents = mended.as_deref().map_or(&body[..], str::as_bytes);

    match tokio::fs::write(&path, contents).await {
        Ok(()) => {
            tracing::debug!(language = %subtitle.language, path = %path.display(), "subtitle cached");
            subtitle.url = path.to_string_lossy().into_owned();
        }
        // The fetch worked and only the write failed, so the remote URL is still good.
        Err(error) => tracing::debug!(%error, "could not cache subtitle; using its url"),
    }
    Some(subtitle)
}

/// Subtitle tracks fetched at once.
const SUBTITLE_FETCHES: usize = 4;

/// A WebVTT file as the source meant it, or `None` when it needs nothing.
///
/// Some sources convert their subtitles to WebVTT from a format that allows more than WebVTT
/// does, and two things come out broken:
///
/// - **A cue with a blank line in it**, an on-screen panel of text laid out as paragraphs. A
///   blank line ends a WebVTT cue, so the paragraphs after it are left as blocks with no timing.
///   The format says to skip such a block. ffmpeg, which mpv reads WebVTT with, stops at it
///   instead, and every cue after it is lost: an episode's subtitles end a few minutes in. Each
///   such block goes back onto the cue before it, after a line holding only a no-break space,
///   which is not blank to a parser and shows as the gap the source had.
/// - **A shape drawn by command**, `m 370 219 l 370 339 595 339`, the box behind that panel. WebVTT
///   has no drawings, so it shows the commands as text. They are dropped, and a cue that was
///   nothing else goes with them.
fn mend_webvtt(text: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n");
    let mut blocks: Vec<String> = Vec::new();
    let mut mended = false;
    for block in text.split("\n\n").map(|b| b.trim_start_matches('\n')) {
        if block.trim().is_empty() {
            continue;
        }
        let mut lines = block.lines();
        let timed = lines.next().is_some_and(|l| l.contains("-->"))
            || lines.next().is_some_and(|l| l.contains("-->"));
        let own_block = ["NOTE", "STYLE", "REGION"].iter().any(|word| block.starts_with(word));
        let kept: Vec<&str> = block.lines().filter(|line| !is_drawing(line)).collect();
        mended |= kept.len() != block.lines().count();
        let block = kept.join("\n");
        match blocks.last_mut() {
            Some(previous) if !timed && !own_block => {
                if !block.trim().is_empty() {
                    previous.push_str("\n\u{a0}\n");
                    previous.push_str(&block);
                }
                mended = true;
            }
            _ => blocks.push(block),
        }
    }
    // A cue whose only text was a drawing is its timing alone now.
    blocks.retain(|block| !block.lines().last().is_some_and(|l| l.contains("-->")));
    mended.then(|| blocks.join("\n\n") + "\n")
}

/// Whether a cue line is a vector drawing — `m`, then drawing commands and coordinates only —
/// once its tags are set aside.
fn is_drawing(line: &str) -> bool {
    let mut text = String::new();
    let mut in_tag = false;
    for c in line.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let mut words = text.split_whitespace();
    if words.next() != Some("m") {
        return false;
    }
    let rest: Vec<&str> = words.collect();
    let number = |w: &str| w.parse::<f64>().is_ok();
    rest.iter().filter(|w| number(w)).count() >= 2
        && rest
            .iter()
            .all(|w| number(w) || matches!(*w, "m" | "n" | "l" | "b" | "s" | "p" | "c"))
}

/// Reduce a language label to something safe to put in a filename.
pub(super) fn sanitise(language: &str) -> String {
    let cleaned: String = language
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();
    if cleaned.is_empty() { "sub".into() } else { cleaned }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use anistream_core::stream::StreamKind;

    use super::*;

    #[test]
    fn a_cue_with_a_blank_line_keeps_its_paragraphs_and_the_cues_after_it() {
        let vtt = "WEBVTT\n\n00:01.000 --> 00:02.000\nHello.\n\n\
                   q1\n00:03.000 --> 00:06.000\n<b>Quest</b>\n\nRescue the prisoner,\nquickly.\n\n\
                   00:03.000 --> 00:06.000\n<b>m 0 0 l 200 0 200 71 0 71</b>\n\n\
                   00:04.000 --> 00:05.000\nmore to do\nm 370 219 l 370 339 595 339\n\n\
                   NOTE kept as it is\n\n00:07.000 --> 00:08.000\nGoodbye.\n";
        let mended = mend_webvtt(vtt).expect("the file is mended");
        assert_eq!(
            mended,
            "WEBVTT\n\n00:01.000 --> 00:02.000\nHello.\n\n\
             q1\n00:03.000 --> 00:06.000\n<b>Quest</b>\n\u{a0}\nRescue the prisoner,\nquickly.\n\n\
             00:04.000 --> 00:05.000\nmore to do\n\n\
             NOTE kept as it is\n\n00:07.000 --> 00:08.000\nGoodbye.\n"
        );

        let clean = "WEBVTT\r\n\r\n00:01.000 --> 00:02.000\r\nHello.\r\nmy 2 cents\r\n";
        assert_eq!(mend_webvtt(clean), None, "a sound file is left as it came");
    }

    /// A subtitle host on loopback that answers each request once `release` lets it, with a
    /// WebVTT file named for its path, or `404` for a path saying `missing`; and how many
    /// requests it has held at once, at most.
    async fn host(
        release: Arc<tokio::sync::Semaphore>,
    ) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let host =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let origin = format!("http://{}", host.local_addr().unwrap());
        let (held, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let seen = Arc::clone(&most);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = host.accept().await {
                let (release, held, most) =
                    (Arc::clone(&release), Arc::clone(&held), Arc::clone(&most));
                tokio::spawn(async move {
                    let mut request = [0u8; 2048];
                    let n = socket.read(&mut request).await.unwrap_or(0);
                    let head = String::from_utf8_lossy(&request[..n]).into_owned();
                    let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    let now = held.fetch_add(1, Ordering::SeqCst) + 1;
                    most.fetch_max(now, Ordering::SeqCst);
                    let _answer = release.acquire().await;
                    held.fetch_sub(1, Ordering::SeqCst);
                    let response = if path.contains("missing") {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .to_owned()
                    } else {
                        let body = format!("WEBVTT\n\nNOTE {path}\n");
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    };
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        (origin, seen)
    }

    fn track(language: &str, url: String, hard: bool) -> Subtitle {
        Subtitle { language: language.into(), url, hard, format: None }
    }

    fn stream_with(subtitles: Vec<Subtitle>) -> Stream {
        Stream {
            url: "http://unused.invalid/master.m3u8".into(),
            kind: StreamKind::Hls,
            quality: None,
            headers: Vec::new(),
            subtitles,
            provider_id: "test".into(),
            download_source: None,
            pick_note: None,
            thumbnail_url: None,
        }
    }

    fn client() -> HttpClient {
        HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap()
    }

    #[tokio::test]
    async fn subtitles_are_fetched_together_and_kept_in_their_listed_order() {
        // The host holds every answer until all three requests are in: fetched in turn, the
        // first would never be answered, and the test would wait out its own limit instead.
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let (origin, most) = host(Arc::clone(&release)).await;
        let mut stream = stream_with(vec![
            track("en", format!("{origin}/en.vtt"), false),
            track("de", format!("{origin}/missing.vtt"), false),
            track("fr", format!("{origin}/fr.vtt"), false),
            track("ja", "burned-in".into(), true),
        ]);
        let scratch = tempfile::tempdir().unwrap();
        let opened = tokio::spawn(async move {
            // A wait far past the test's: this is about order and togetherness, not time.
            let wait = Duration::from_secs(3600);
            let late = localise_subtitles(&client(), scratch.path(), &mut stream, wait).await;
            (stream, late, scratch)
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while most.load(std::sync::atomic::Ordering::SeqCst) < 3 {
            assert!(
                std::time::Instant::now() < deadline,
                "the tracks were not asked for at once"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        release.add_permits(3);
        let (stream, late, _scratch) = opened.await.unwrap();
        assert!(late.is_none(), "every track in");

        let languages: Vec<_> = stream.subtitles.iter().map(|s| s.language.as_str()).collect();
        assert_eq!(
            languages,
            ["en", "fr", "ja"],
            "listed order, with the refused track dropped"
        );
        for fetched in &stream.subtitles[..2] {
            assert!(
                !fetched.url.starts_with("http"),
                "{} must point at its local copy",
                fetched.language
            );
            assert!(std::fs::read_to_string(&fetched.url).unwrap().starts_with("WEBVTT"));
        }
    }

    #[tokio::test]
    async fn a_host_slower_than_the_wait_opens_the_file_without_its_track_which_follows() {
        // Held until the localiser has returned: it can only have returned by giving up the
        // wait, never by the track arriving, whatever the machine's load.
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let (origin, _) = host(Arc::clone(&release)).await;
        let mut stream = stream_with(vec![
            track("ja", "burned-in".into(), true),
            track("en", format!("{origin}/en.vtt"), false),
        ]);
        let scratch = tempfile::tempdir().unwrap();
        let late =
            localise_subtitles(&client(), scratch.path(), &mut stream, Duration::ZERO).await;
        let mut late = late.expect("the slow track follows");
        let mut all: Vec<Subtitle> = stream.subtitles.clone();
        assert!(all.iter().all(|s| s.language != "en"), "not before its host answered");

        release.add_permits(1);
        while let Some(arrived) = late.next().await {
            all.push(arrived);
        }
        let languages: std::collections::BTreeSet<_> =
            all.iter().map(|s| s.language.as_str()).collect();
        assert_eq!(languages, ["en", "ja"].into(), "every track, in time or after");
        let en = all.iter().find(|s| s.language == "en").unwrap();
        assert!(!en.url.starts_with("http"), "at its local copy");
    }
}
