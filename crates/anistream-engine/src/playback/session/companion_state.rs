//! What the player's companion shows, kept current through the playback: the episode, the
//! menus and night mode, the Up Next card with the next episode's still, and the next
//! episode's streams asked for once this one's ending begins.

use anistream_core::{media::Translation, stream::Stream};
use anistream_model::{event::Event, playback::stream_label};
use anistream_net::HttpClient;
use anistream_playback::{SkipKind, companion};

use super::{EpisodeEntry, NextEpisode, Run};
use crate::playback::{
    PlaybackContext,
    link::{Overlay, PlayerLink},
    picture, shaders,
};

/// What the player's companion is told of the episode: mpv's own business, beside what any
/// player is told of a playback above.
impl PlaybackContext {
    pub(super) fn episode_info(&self, episodes: &[EpisodeEntry]) -> companion::EpisodeInfo {
        companion::EpisodeInfo {
            number: self.episode.clone(),
            label: self.episode_label(),
            title: episodes
                .iter()
                .find(|e| e.number == self.episode)
                .and_then(|e| e.title.clone()),
            show: self.title.clone(),
            translation: match self.translation {
                Translation::Dub => "dub",
                _ => "sub",
            },
        }
    }
}

/// The audio filter night mode adds, labelled so removing it leaves the viewer's own alone.
///
/// `dynaudnorm` lifts quiet passages and holds loud ones down over a short window, which is
/// what keeps dialogue audible at a low volume without the opening arriving at full blast.
const NIGHT_FILTER: &str = "@anistream-night:lavfi=[dynaudnorm=f=150:g=15:p=0.6:m=8]";

/// How long before the end the next episode is found, when the ending's start is not known.
const PREFETCH_LEAD: f64 = 90.0;

impl Run<'_> {
    /// Tell the companion what it is showing: the episode, what follows, the menus.
    pub(super) async fn publish(
        &mut self,
        link: &dyn PlayerLink,
        streams: &[Stream],
        current: usize,
    ) {
        let context = self.context;
        let episode = context.episode_info(&self.episodes);
        let starting = Overlay::Starting {
            card: companion::Status::loading(
                format!("Opening {}", context.episode_label()),
                None,
            ),
            id: context.anilist_id,
            title: context.title.clone(),
            episode: context.episode.clone(),
            episode_title: episode.title.clone(),
            cover_url: context.cover_url.clone(),
            banner_url: context.banner_url.clone(),
        };
        let overlays = [
            Overlay::Episode(episode),
            Overlay::Next {
                card: self.next_up(),
                still: self.next_still(),
                countdown: self.settings.up_next_countdown,
            },
            Overlay::Menus(self.menus(streams, current)),
            Overlay::Media(companion::Media::default()),
            starting,
        ];
        for overlay in overlays {
            let _ = link.overlay(overlay).await;
        }
        self.apply_night(link).await;
    }

    /// Ask for the next episode's streams once this one's ending begins.
    ///
    /// Only when the episode will roll on by itself: fetching ahead for a viewer who stops at
    /// the end would be a search, and for a torrent a download, nobody asked for.
    pub(super) fn maybe_prefetch(&mut self, position: f64, duration: Option<f64>) {
        if self.prefetch_sent || !self.settings.auto_next || self.cancel_next {
            return;
        }
        let Some(next) = &self.next else { return };
        let ending = self.skips.iter().find(|s| s.kind == SkipKind::Ending).map(|s| s.start);
        let at = ending.or_else(|| {
            duration.filter(|d| *d > 2.0 * PREFETCH_LEAD).map(|d| d - PREFETCH_LEAD)
        });
        if at.is_some_and(|at| position >= at) {
            self.prefetch_sent = true;
            tracing::info!(episode = %next.number, "finding the next episode ahead");
            let _ = self.tx.send(Event::PrefetchNext {
                id: self.context.anilist_id,
                episode: next.number.clone(),
            });
        }
    }

    /// Take an episode list that arrived after playback started.
    ///
    /// Only fills what the context left empty: a next episode the episode table already named
    /// is the one the viewer saw, and stays. Returns the next episode's still to fetch, if a
    /// next episode was learned here and has one.
    pub(super) fn learn_episodes(
        &mut self,
        rows: Vec<anistream_model::episodes::EpisodeRow>,
        next: Option<String>,
    ) -> Option<String> {
        if self.episodes.is_empty() {
            self.episodes = rows
                .iter()
                .map(|row| EpisodeEntry {
                    number: row.number.clone(),
                    title: row.title.clone(),
                    watched: row.completed,
                    filler: row.skippable,
                })
                .collect();
        }
        if self.next.is_some() {
            return None;
        }
        let row = rows.into_iter().find(|row| next.as_deref() == Some(row.number.as_str()))?;
        tracing::info!(episode = %row.number, "learned the next episode after starting");
        self.next = Some(NextEpisode {
            number: row.number,
            title: row.title,
            thumbnail: row.thumbnail.clone(),
        });
        row.thumbnail
    }

    /// The Up Next card as it stands now, with the next episode's still by URL, when an episode
    /// follows.
    pub(super) fn up_next(&self) -> Option<Overlay> {
        let card = self.next_up()?;
        Some(Overlay::Next {
            card: Some(card),
            still: self.next_still(),
            countdown: self.settings.up_next_countdown,
        })
    }

    /// The next episode's still, by URL, for a player that fetches its own.
    fn next_still(&self) -> Option<String> {
        self.next.as_ref().and_then(|next| next.thumbnail.clone())
    }

    /// What the Up Next card shows, as it stands now.
    pub(super) fn next_up(&self) -> Option<companion::NextUp> {
        let ending = self.skips.iter().find(|s| s.kind == SkipKind::Ending).map(|s| s.start);
        self.next.as_ref().map(|next| companion::NextUp {
            number: next.number.clone(),
            label: next.label(),
            title: next.title.clone(),
            at: ending,
            auto: self.settings.auto_next && !self.cancel_next,
            thumbnail: self.still.clone(),
        })
    }

    pub(super) fn menus(&self, streams: &[Stream], current: usize) -> companion::Menus {
        let context = self.context;
        companion::Menus {
            episodes: self
                .episodes
                .iter()
                .map(|e| companion::EpisodeItem {
                    number: e.number.clone(),
                    label: format!("Episode {}", e.number),
                    title: e.title.clone(),
                    watched: e.watched,
                    filler: e.filler,
                    current: e.number == context.episode,
                })
                .collect(),
            sources: streams
                .iter()
                .zip(source_details(streams))
                .enumerate()
                .map(|(i, (s, detail))| companion::SourceItem {
                    id: i.to_string(),
                    label: s.provider_id.clone(),
                    detail: Some(detail),
                    current: i == current,
                    health: self.servers.note(s),
                })
                .collect(),
            translation: companion::TranslationMenu {
                current: match context.translation {
                    Translation::Dub => "dub",
                    _ => "sub",
                },
                options: vec!["sub", "dub"],
            },
            upscaling: shaders::menu(self.look),
            tone_mapping: picture::tone_mapping_menu(self.hdr, &self.tone_mapping),
            night_mode: self.night,
        }
    }

    pub(super) async fn apply_night(&mut self, link: &dyn PlayerLink) {
        if self.night == self.night_applied {
            return;
        }
        let filter = if self.night { NIGHT_FILTER } else { "@anistream-night" };
        let _ = link.audio_filter(self.night, filter).await;
        self.night_applied = self.night;
    }
}

/// The size the Up Next still is written at: 16:9, large enough to stay sharp when the card is
/// drawn at HiDPI scale, small enough to write in a few milliseconds.
const STILL: (u32, u32) = (640, 360);

/// Fetch an episode still and write it as raw BGRA for `overlay-add`.
///
/// Cropped to fill 16:9 rather than letterboxed: stills arrive in every shape and the card has
/// one. `None` on any failure — the card simply shows no picture.
pub(super) async fn fetch_still(
    http: HttpClient,
    url: String,
    dir: std::path::PathBuf,
) -> Option<companion::Still> {
    let response = http.plain().get(&url).send().await.ok()?.error_for_status().ok()?;
    let bytes = response.bytes().await.ok()?;
    tokio::task::spawn_blocking(move || {
        let (width, height) = STILL;
        let image = image::load_from_memory(&bytes).ok()?;
        let mut pixels = image
            .resize_to_fill(width, height, image::imageops::FilterType::Lanczos3)
            .to_rgba8()
            .into_raw();
        // mpv's overlays are BGRA; swap red and blue in place.
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let name = format!("upnext-{}.bgra", anistream_core::hash::stable_hash(&url));
        let path = dir.join(name);
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::write(&path, &pixels).ok()?;
        Some(companion::Still { path: path.to_string_lossy().into_owned(), width, height })
    })
    .await
    .ok()
    .flatten()
}

/// Each stream's second line in the Sources menu, told apart where two would read the same.
///
/// A provider often offers an episode from several servers with nothing else between them, and
/// two rows reading "HLS" under the same provider gave no reason to pick either, nor a way to
/// see which one was playing. The server's name tells them apart, and a number where even that
/// is shared.
fn source_details(streams: &[Stream]) -> Vec<String> {
    let detail = |i: usize, server: bool| {
        // A stream's second line before telling twins apart: the label a front end's own
        // player is handed too.
        let base = stream_label(&streams[i]);
        match server.then(|| server_name(&streams[i].url)).flatten() {
            Some(name) => format!("{base} · {name}"),
            None => base,
        }
    };
    let alike = |i: usize, j: usize, server: bool| {
        streams[i].provider_id == streams[j].provider_id
            && detail(i, server) == detail(j, server)
    };
    let shared =
        |i: usize, server: bool| (0..streams.len()).any(|j| j != i && alike(i, j, server));
    (0..streams.len())
        .map(|i| {
            if !shared(i, false) {
                return detail(i, false);
            }
            if !shared(i, true) {
                return detail(i, true);
            }
            // In the order they are listed, which is the order they are ranked.
            let nth = (0..i).filter(|&j| alike(i, j, true)).count() + 1;
            format!("{} · {nth}", detail(i, true))
        })
        .collect()
}

/// The part of a stream's host that names its server: `edge.alpha.example` is `alpha`. An
/// address is kept whole; a loopback one names no server, and torrents share it.
fn server_name(url: &str) -> Option<String> {
    let authority = url.split_once("://")?.1.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
    if host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()) {
        return None;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Some(host.to_owned());
    }
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    let name = match labels.as_slice() {
        [] => return None,
        [only] => *only,
        [.., name, _suffix] => *name,
    };
    Some(name.to_owned())
}

#[cfg(test)]
mod tests {
    use anistream_core::stream::StreamKind;

    use super::*;

    fn hls(provider: &str, url: &str) -> Stream {
        Stream { provider_id: provider.into(), ..Stream::new(url, StreamKind::Hls) }
    }

    #[test]
    fn sources_that_would_read_the_same_are_told_apart() {
        let streams = [
            hls("plugin", "https://edge-1.alpha.example/a/master.m3u8"),
            hls("plugin", "https://cdn.beta.example/b/master.m3u8?token=x"),
            hls("plugin", "https://cdn.beta.example/c/master.m3u8"),
            hls("other", "https://cdn.gamma.example/master.m3u8").with_quality(720),
        ];
        assert_eq!(
            source_details(&streams),
            ["HLS · alpha", "HLS · beta · 1", "HLS · beta · 2", "720p · HLS"],
            "a row already unique is left as it was"
        );
        // A loopback address names no server; torrents behind it are numbered.
        let local = [
            hls("torrent", "http://127.0.0.1:4040/s/1"),
            hls("torrent", "http://127.0.0.1:4040/s/2"),
        ];
        assert_eq!(source_details(&local), ["HLS · 1", "HLS · 2"]);
    }
}
