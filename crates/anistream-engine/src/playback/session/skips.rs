//! Skipping the opening and the ending: the intervals aniskip knows for an episode, how a skip
//! is named to the interface and to the companion, and where one lands when its segment runs
//! to the end of the file.

use anistream_net::HttpClient;
use anistream_player::{SkipInterval, SkipKind, skip};

/// Fetch skip intervals for an episode.
///
/// Keyed on MAL id, so a title the mapping layer could not resolve simply has no skip data —
/// the prompt is absent rather than broken.
pub async fn fetch_skips(
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

    // Length 0 first, because it returns every submission regardless of which encode it
    // was timed against. It has one measured failure: aniskip answers 500 when a stored
    // interval runs to the exact end of the episode — an ending that fades on the last
    // frame, common for currently-airing shows. The retry asks again with the advertised
    // runtime, which that server handles; its tight length-tolerance is the price, and it
    // costs nothing here since the alternative was no data at all.
    let attempts = [Some(0), runtime_secs];
    for length in attempts.into_iter().flatten() {
        match http.plain().get(skip::query_url(mal_id, number, length)).send().await {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();
                let intervals = skip::parse(&body);
                tracing::info!(count = intervals.len(), length, "skip intervals loaded");
                return intervals;
            }
            // aniskip returns 404 for titles nobody has submitted times for, which is
            // the norm — only a server error is worth the second ask.
            Ok(response) if !response.status().is_server_error() => return Vec::new(),
            _ => {}
        }
    }
    Vec::new()
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

    #[tokio::test]
    async fn a_title_without_a_mal_id_simply_has_no_skip_data() {
        // The mapping layer cannot resolve every title, and skip data is decoration.
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        assert!(fetch_skips(&http, None, "1", None).await.is_empty());
    }

    #[tokio::test]
    async fn a_non_numeric_episode_is_not_looked_up() {
        // "OVA" and "S1" are real episode labels; asking aniskip about them is meaningless.
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        assert!(fetch_skips(&http, Some(52_991), "OVA", None).await.is_empty());
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
