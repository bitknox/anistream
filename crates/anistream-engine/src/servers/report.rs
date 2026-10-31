//! Telling the server what is watched.
//!
//! A playback of a server's stream opens a [`Report`]: it says the playback began, where it is
//! about every ten seconds and when it is paused or resumed, and that it stopped. Each call is a
//! task of its own, so a slow server never holds the player. Off when the server's switch is.

use std::time::{Duration, Instant};

use anistream_core::stream::Stream;
use anistream_jellyfin::Jellyfin;

use super::Servers;

/// How often progress is said while playing.
const EVERY: Duration = Duration::from_secs(10);

/// What a playback asks for a report from. Does nothing by default.
#[derive(Clone, Default)]
pub struct Reports(Option<Servers>);

impl std::fmt::Debug for Reports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reports")
    }
}

impl Reports {
    pub(super) fn new(servers: Servers) -> Self {
        Self(Some(servers))
    }

    /// A report for this stream, when it is a server's and the server wants reports.
    pub fn open(&self, stream: &Stream) -> Option<Report> {
        let servers = self.0.as_ref()?;
        let (item, source) = parse(&stream.url)?;
        let record = servers
            .records()
            .into_iter()
            .find(|r| anistream_jellyfin::source_id(&r.id) == stream.provider_id)?;
        if !record.report {
            return None;
        }
        let client = servers.signed_in(&record)?;
        Some(Report::begin(client, item, source))
    }
}

/// The item and media source a direct stream address names.
pub(crate) fn parse(url: &str) -> Option<(String, String)> {
    let (path, query) = url.split_once('?')?;
    let item = path.rsplit_once("/Videos/")?.1.strip_suffix("/stream")?.to_owned();
    let source = query.split('&').find_map(|p| p.strip_prefix("mediaSourceId="))?.to_owned();
    Some((item, source))
}

/// One playback's reports.
pub struct Report {
    client: Jellyfin,
    item: String,
    source: String,
    last: Instant,
    paused: bool,
}

impl Report {
    fn begin(client: Jellyfin, item: String, source: String) -> Self {
        let report = Self { client, item, source, last: Instant::now(), paused: false };
        let (c, i, s) = (report.client.clone(), report.item.clone(), report.source.clone());
        tokio::spawn(async move {
            let _ = c.report_started(&i, &s, 0.0).await;
        });
        report
    }

    /// Where the player is. Said at most every ten seconds, and at once on a pause or resume.
    pub fn observe(&mut self, position: f64, paused: bool) {
        if paused == self.paused && self.last.elapsed() < EVERY {
            return;
        }
        self.paused = paused;
        self.last = Instant::now();
        let (c, i, s) = (self.client.clone(), self.item.clone(), self.source.clone());
        tokio::spawn(async move {
            let _ = c.report_progress(&i, &s, position, paused).await;
        });
    }

    /// The playback ended at `position`. The server marks the episode played near its end.
    pub fn finish(self, position: f64) {
        tokio::spawn(async move {
            let _ = self.client.report_stopped(&self.item, &self.source, position).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_direct_stream_address_names_its_item_and_source() {
        let url =
            "http://nas.example:8096/Videos/abc123/stream?static=true&mediaSourceId=src-9";
        assert_eq!(parse(url), Some(("abc123".into(), "src-9".into())));
        assert_eq!(parse("https://other.example/video.mp4"), None);
    }
}
