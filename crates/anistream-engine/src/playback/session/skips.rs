//! Skipping the opening and the ending: the intervals aniskip knows for an episode, how a skip
//! is named to the interface and to the companion, and where one lands when its segment runs
//! to the end of the file.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, OnceLock},
};

use anistream_net::HttpClient;
use anistream_playback::{SkipInterval, SkipKind, skip};

use super::PlaySettings;

/// How many episodes' answers are kept at most. A binge is a few dozen; past this the oldest
/// go, and an episode played again after that is asked about again.
const KEPT: usize = 512;

/// The opening and ending times aniskip gave, per MyAnimeList id and episode, kept for as long
/// as the process runs: watching an episode again, or going back to one, does not ask again.
///
/// An episode nobody has submitted times for is kept too, as no times. A failed ask is not,
/// so the next play of that episode asks again.
pub struct SkipTimes {
    origin: String,
    known: Mutex<Known>,
}

#[derive(Default)]
struct Known {
    answers: HashMap<(u32, u32), Vec<SkipInterval>>,
    /// The keys in the order they were first answered, oldest first, for [`KEPT`].
    order: VecDeque<(u32, u32)>,
}

impl SkipTimes {
    /// The process's own, which every playback shares.
    pub fn shared() -> &'static Self {
        static SHARED: OnceLock<SkipTimes> = OnceLock::new();
        SHARED.get_or_init(|| Self::on(skip::API))
    }

    /// Times asked of `origin` rather than aniskip, kept apart from the process's own: a
    /// test's server.
    pub fn on(origin: &str) -> Self {
        Self { origin: origin.to_owned(), known: Mutex::default() }
    }

    /// The times a playback under `settings` needs: none at all when nothing it shows uses them,
    /// which asks nothing of aniskip.
    ///
    /// The times are used to skip on their own, to mark the seek bar and to show the skip
    /// button. With all three off, the episode's MyAnimeList id and number stay on the device.
    pub async fn for_playback(
        &self,
        http: &HttpClient,
        settings: &PlaySettings,
        mal_id: Option<u32>,
        episode: &str,
        runtime_secs: Option<u32>,
    ) -> Vec<SkipInterval> {
        if !settings.uses_skip_times() {
            tracing::debug!("nothing shows skip times; aniskip not asked");
            return Vec::new();
        }
        self.get(http, mal_id, episode, runtime_secs).await
    }

    /// An episode's skip intervals, from what this process was told already or else from
    /// aniskip.
    ///
    /// Keyed on MAL id, so a title the mapping layer could not resolve simply has no skip data:
    /// the prompt is absent rather than broken.
    pub async fn get(
        &self,
        http: &HttpClient,
        mal_id: Option<u32>,
        episode: &str,
        runtime_secs: Option<u32>,
    ) -> Vec<SkipInterval> {
        let Some(mal_id) = mal_id else {
            tracing::debug!("no mal id; skip data unavailable for this title");
            return Vec::new();
        };
        let Ok(number) = episode.trim().parse::<u32>() else {
            return Vec::new();
        };
        let key = (mal_id, number);
        if let Some(known) = self.lock().answers.get(&key) {
            tracing::debug!(count = known.len(), "skip intervals already known");
            return known.clone();
        }
        let Some(intervals) = self.ask(http, mal_id, number, runtime_secs).await else {
            return Vec::new();
        };
        let mut known = self.lock();
        if known.answers.insert(key, intervals.clone()).is_none() {
            known.order.push_back(key);
            while known.order.len() > KEPT {
                if let Some(oldest) = known.order.pop_front() {
                    known.answers.remove(&oldest);
                }
            }
        }
        intervals
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Known> {
        self.known.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Ask aniskip. `None` when there is no answer worth keeping: the network failed, or the
    /// server did on both asks.
    async fn ask(
        &self,
        http: &HttpClient,
        mal_id: u32,
        number: u32,
        runtime_secs: Option<u32>,
    ) -> Option<Vec<SkipInterval>> {
        // Length 0 first, because it returns every submission regardless of which encode it
        // was timed against. It has one measured failure: aniskip answers 500 when a stored
        // interval runs to the exact end of the episode — an ending that fades on the last
        // frame, common for currently-airing shows. The retry asks again with the advertised
        // runtime, which that server handles; its tight length-tolerance is the price, and it
        // costs nothing here since the alternative was no data at all.
        let attempts = [Some(0), runtime_secs];
        for length in attempts.into_iter().flatten() {
            let url = skip::query_url_on(&self.origin, mal_id, number, length);
            match http.plain().get(url).send().await {
                Ok(response) if response.status().is_success() => {
                    let body = response.text().await.ok()?;
                    let intervals = skip::parse(&body);
                    tracing::info!(count = intervals.len(), length, "skip intervals loaded");
                    return Some(intervals);
                }
                // aniskip returns 404 for titles nobody has submitted times for, which is
                // the norm — only a server error is worth the second ask.
                Ok(response) if !response.status().is_server_error() => return Some(Vec::new()),
                _ => {}
            }
        }
        None
    }
}

impl PlaySettings {
    /// Whether anything this playback shows uses the opening and ending times: skipping them on
    /// its own, marking them on the seek bar, or the skip button.
    pub const fn uses_skip_times(&self) -> bool {
        self.auto_skip || self.mark_chapters || self.skip_button
    }
}

/// A skip as the interface is told it.
///
/// Public for the binary's test that holds the player's words for a skip to the interface's,
/// since only the binary sees both.
pub const fn offered(kind: SkipKind) -> anistream_model::playback::SkipKind {
    match kind {
        SkipKind::Opening => anistream_model::playback::SkipKind::Opening,
        SkipKind::Ending => anistream_model::playback::SkipKind::Ending,
    }
}

/// How far short of the end a skip lands when its segment runs to the end of the file.
///
/// An ending cut as the release's last chapter ends exactly at the runtime, and aniskip's times,
/// taken against another rip, can end past it. Seeking there left mpv seeking at end-of-file for
/// up to most of a minute, until Up Next fired on its own; landing just short lets the last
/// frames play and the file end the ordinary way, which is what rolls on to the next episode.
const SKIP_SHORT_OF_END: f64 = 1.0;

/// Where a skip to `to` lands in a file `duration` long: `to` itself, unless that is at or past
/// the end. An unknown length is no reason to second-guess the segment.
pub(super) fn skip_target(to: f64, duration: Option<f64>) -> f64 {
    match duration {
        Some(d) if d > SKIP_SHORT_OF_END => to.min(d - SKIP_SHORT_OF_END),
        _ => to,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn http() -> HttpClient {
        HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap()
    }

    /// A stand-in for aniskip that counts what it is asked, answering every ask with an
    /// opening, or with a server error for an episode numbered 500.
    async fn aniskip() -> (SkipTimes, Arc<AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let host =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let origin = format!("http://{}", host.local_addr().unwrap());
        let asked = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&asked);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = host.accept().await {
                let counted = Arc::clone(&counted);
                tokio::spawn(async move {
                    let mut request = [0u8; 2048];
                    let n = socket.read(&mut request).await.unwrap_or(0);
                    let head = String::from_utf8_lossy(&request[..n]).into_owned();
                    let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    counted.fetch_add(1, Ordering::SeqCst);
                    let response = if path.contains("/500?") {
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\
                         Connection: close\r\n\r\n"
                            .to_owned()
                    } else {
                        let body = r#"{"found":true,"results":[{"interval":{"startTime":3.0,"endTime":93.0},"skipType":"op"}]}"#;
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    };
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        (SkipTimes::on(&origin), asked)
    }

    fn settings(auto_skip: bool, mark_chapters: bool, skip_button: bool) -> PlaySettings {
        PlaySettings {
            auto_skip,
            mark_chapters,
            skip_button,
            ..super::super::scripted::settings()
        }
    }

    #[tokio::test]
    async fn a_title_without_a_mal_id_simply_has_no_skip_data() {
        // The mapping layer cannot resolve every title, and skip data is decoration.
        let (times, asked) = aniskip().await;
        assert!(times.get(&http(), None, "1", None).await.is_empty());
        assert_eq!(asked.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_non_numeric_episode_is_not_looked_up() {
        // "OVA" and "S1" are real episode labels; asking aniskip about them is meaningless.
        let (times, asked) = aniskip().await;
        assert!(times.get(&http(), Some(52_991), "OVA", None).await.is_empty());
        assert_eq!(asked.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn with_nothing_to_show_the_times_aniskip_is_not_asked() {
        // Skipping on its own, the seek bar's marks and the skip button, all off: the
        // episode's MyAnimeList id and number never leave the device.
        let (times, asked) = aniskip().await;
        let off = settings(false, false, false);
        assert!(!off.uses_skip_times());
        assert!(
            times.for_playback(&http(), &off, Some(52_991), "1", Some(1440)).await.is_empty()
        );
        assert_eq!(asked.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn anything_that_shows_the_times_asks_once() {
        for on in [
            settings(true, false, false),
            settings(false, true, false),
            settings(false, false, true),
        ] {
            let (times, asked) = aniskip().await;
            assert!(on.uses_skip_times());
            let skips = times.for_playback(&http(), &on, Some(52_991), "1", Some(1440)).await;
            assert_eq!(skips.len(), 1, "the opening the server knows");
            assert_eq!(asked.load(Ordering::SeqCst), 1, "one ask, the first one answered");
        }
    }

    #[tokio::test]
    async fn an_episode_played_again_is_answered_from_what_was_kept() {
        let (times, asked) = aniskip().await;
        let on = settings(false, false, true);
        let first = times.for_playback(&http(), &on, Some(52_991), "1", Some(1440)).await;
        let again = times.for_playback(&http(), &on, Some(52_991), " 1", Some(1440)).await;
        assert_eq!(first, again);
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the replay asked nothing");
        // Another episode is another question.
        times.for_playback(&http(), &on, Some(52_991), "2", Some(1440)).await;
        assert_eq!(asked.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_failed_ask_is_not_kept() {
        // Both asks fail on the server, so the next play of the episode asks again.
        let (times, asked) = aniskip().await;
        assert!(times.get(&http(), Some(52_991), "500", Some(1440)).await.is_empty());
        assert_eq!(asked.load(Ordering::SeqCst), 2, "length 0, then the runtime");
        times.get(&http(), Some(52_991), "500", Some(1440)).await;
        assert_eq!(asked.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn a_skip_that_reaches_the_end_lands_just_short_of_it() {
        // An ending that closes the file, and aniskip's ending for a longer rip.
        assert_eq!(skip_target(1420.0, Some(1420.0)), 1420.0 - SKIP_SHORT_OF_END);
        assert_eq!(skip_target(1507.0, Some(1420.0)), 1420.0 - SKIP_SHORT_OF_END);
        // Anything short of the end, or a length not known yet, is the segment's own end.
        assert_eq!(skip_target(93.2, Some(1420.0)), 93.2);
        assert_eq!(skip_target(1507.0, None), 1507.0);
    }
}
