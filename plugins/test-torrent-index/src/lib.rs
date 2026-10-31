//! A made-up torrent index, for tests.
//!
//! Test infrastructure, not an example, and it points at nothing real: its "index" is a table
//! below, its host is `index.example`, and its info hashes are counting patterns. It shows the
//! shape the plugin ABI gives a source that finds torrents:
//!
//! - it declares the `torrents` capability, without which the host drops its torrent streams;
//! - `resolve` returns a `torrent` stream, a magnet link or a bare info hash, with the file to play;
//! - `sources` returns its candidates in its own order, each with a `label` it writes itself.
//!
//! All ranking is in here, in `ranked`. The host plays what it is handed and ranks nothing.

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../wit/anistream-provider.wit",
        world: "plugin",
    });
}

use bindings::exports::anistream::provider::provider::{
    Episode, Guest, Manifest, MediaStream, ProviderError, SearchHit, SourceCandidate,
};

/// One torrent the made-up index lists.
struct Listing {
    id: &'static str,
    title: &'static str,
    /// A bare info hash or a whole magnet link: the ABI takes either.
    torrent: &'static str,
    /// Episodes 1 to 3 are files 0 to 2 of a season torrent; a single-episode one has none.
    season: bool,
    episode: u32,
    quality: u32,
    peers: u32,
    size: &'static str,
}

const SHOW: &str = "hoshikuzu";

const INDEX: &[Listing] = &[
    Listing {
        id: "season",
        title: "Hoshikuzu no Tabibito, season one",
        torrent: "1111111111111111111111111111111111111111",
        season: true,
        episode: 0,
        quality: 1080,
        peers: 12,
        size: "4.2 GiB",
    },
    Listing {
        id: "single-2",
        title: "Hoshikuzu no Tabibito - 02",
        torrent: "magnet:?xt=urn:btih:2222222222222222222222222222222222222222&dn=Hoshikuzu%2002",
        season: false,
        episode: 2,
        quality: 720,
        peers: 40,
        size: "350 MiB",
    },
];

/// This plugin's own order for an episode: more peers first. Nothing outside it ranks anything.
fn ranked(episode: u32) -> Vec<&'static Listing> {
    let mut found: Vec<&Listing> = INDEX
        .iter()
        .filter(|l| (l.season && (1..=3).contains(&episode)) || l.episode == episode)
        .collect();
    found.sort_by(|a, b| b.peers.cmp(&a.peers));
    found
}

fn stream(listing: &Listing, episode: u32) -> MediaStream {
    MediaStream {
        url: listing.torrent.into(),
        kind: "torrent".into(),
        quality: Some(listing.quality),
        headers: vec![],
        subtitles: vec![],
        download_source: None,
        pick_note: Some(format!("{} from index.example", listing.title)),
        file_index: listing.season.then(|| episode - 1),
    }
}

fn episode_number(episode: &str) -> Result<u32, ProviderError> {
    episode.trim().parse().map_err(|_| ProviderError::NotFound)
}

struct Component;

impl Guest for Component {
    fn describe() -> Manifest {
        Manifest {
            id: "test-torrent-index".into(),
            display_name: "Torrent index (test only)".into(),
            version: "0.1.0".into(),
            allowed_hosts: vec!["index.example".into()],
            translation_types: vec!["sub".into()],
            capabilities: vec!["torrents".into()],
        }
    }

    fn search(query: String, _translation: String) -> Result<Vec<SearchHit>, ProviderError> {
        if !query.to_lowercase().contains(SHOW) {
            return Ok(vec![]);
        }
        Ok(vec![SearchHit {
            id: SHOW.into(),
            title: "Hoshikuzu no Tabibito".into(),
            synonyms: vec![],
            episode_count: Some(3),
            year: None,
            format: Some("tv".into()),
        }])
    }

    fn list_episodes(id: String, _translation: String) -> Result<Vec<Episode>, ProviderError> {
        if id != SHOW {
            return Err(ProviderError::NotFound);
        }
        Ok((1..=3)
            .map(|n| Episode {
                number: n.to_string(),
                title: None,
                duration_secs: None,
                description: None,
                thumbnail: None,
                air_date: None,
                filler: None,
            })
            .collect())
    }

    fn resolve(
        id: String,
        episode: String,
        _translation: String,
    ) -> Result<Vec<MediaStream>, ProviderError> {
        let episode = episode_number(&episode)?;
        if id != SHOW {
            return Err(ProviderError::NotFound);
        }
        let best = ranked(episode).into_iter().next().ok_or(ProviderError::NotFound)?;
        Ok(vec![stream(best, episode)])
    }

    fn sources(
        id: String,
        episode: String,
        _translation: String,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        let episode = episode_number(&episode)?;
        if id != SHOW {
            return Err(ProviderError::NotFound);
        }
        Ok(ranked(episode)
            .into_iter()
            .map(|listing| SourceCandidate {
                id: listing.id.into(),
                title: listing.title.into(),
                quality: Some(listing.quality),
                label: Some(format!("{} peers", listing.peers)),
                size: Some(listing.size.into()),
                dual_audio: false,
                dubbed: false,
            })
            .collect())
    }

    fn resolve_source(
        id: String,
        episode: String,
        _translation: String,
        source_id: String,
    ) -> Result<Vec<MediaStream>, ProviderError> {
        let episode = episode_number(&episode)?;
        if id != SHOW {
            return Err(ProviderError::NotFound);
        }
        let listing =
            INDEX.iter().find(|l| l.id == source_id).ok_or(ProviderError::NotFound)?;
        Ok(vec![stream(listing, episode)])
    }
}

bindings::export!(Component with_types_in bindings);
