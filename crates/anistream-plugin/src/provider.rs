//! A plugin, as an ordinary [`Provider`].
//!
//! The point of this module is that it is boring. Once a `.wasm` file is wrapped here, the
//! registry cannot tell it from a native Rust source: it ranks by the same config order, records
//! health against the same tracker, and fails over on the same error classes. That is the whole
//! payoff of having the WIT `provider` interface mirror the Rust trait one-for-one — the
//! translation below is mechanical, with no policy in it.
//!
//! Two conversions are worth reading, because getting either wrong would be silent:
//!
//! - **`not-found` stays `not-found`.** It is the one error that must *not* trigger failover, and
//!   flattening it into `Other` would make every missing episode try every remaining source.
//! - **A guest that traps becomes `Parse`.** A broken plugin is broken, not empty, so it counts
//!   against health exactly like a native source whose site changed shape.

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::{Stream, StreamKind, Subtitle},
    traits::{Provider, ProviderKind, ProviderManifest},
};

use crate::engine::{self, GuestError, LoadedPlugin, PluginError};

/// Wraps a loaded component as a [`Provider`].
pub struct WasmProvider {
    plugin: LoadedPlugin,
    manifest: ProviderManifest,
}

impl WasmProvider {
    pub fn new(plugin: LoadedPlugin) -> Self {
        let guest = plugin.manifest();
        let manifest = ProviderManifest {
            id: guest.id.clone(),
            display_name: guest.display_name.clone(),
            version: guest.version.clone(),
            kind: ProviderKind::Plugin,
            allowed_hosts: guest.allowed_hosts.clone(),
            translations: guest
                .translation_types
                .iter()
                .filter_map(|t| translation_from(t))
                .collect(),
        };
        Self { plugin, manifest }
    }

    pub fn plugin(&self) -> &LoadedPlugin {
        &self.plugin
    }
}

/// Parse a translation tag from a manifest.
///
/// Unknown values are dropped rather than defaulting to `Sub`: a plugin claiming to support
/// something we cannot name should not have that silently turned into a claim it does.
fn translation_from(tag: &str) -> Option<Translation> {
    match tag.trim().to_ascii_lowercase().as_str() {
        "sub" | "subbed" => Some(Translation::Sub),
        "dub" | "dubbed" => Some(Translation::Dub),
        _ => None,
    }
}

/// The tag a guest expects, matching the WIT documentation.
const fn translation_tag(translation: Translation) -> &'static str {
    match translation {
        Translation::Sub => "sub",
        Translation::Dub => "dub",
    }
}

/// Guest error → core error.
fn from_guest(error: GuestError) -> ProviderError {
    match error {
        // Load-bearing: `NotFound` deliberately does not trigger failover, so flattening it here
        // would make every missing episode walk the whole provider chain.
        GuestError::NotFound => ProviderError::NotFound,
        GuestError::Blocked(reason) => ProviderError::Blocked(reason),
        GuestError::Parse(reason) => ProviderError::Parse(reason),
        GuestError::Other(reason) => ProviderError::Other(reason),
    }
}

/// Flatten the two failure layers into one.
///
/// A call can fail because the guest returned an error, or because the guest misbehaved and the
/// host stopped it. Callers should not have to care which, so both collapse to `ProviderError` —
/// with a trap or deadline mapped to `Parse`, since a broken plugin should fail over.
fn flatten<T>(result: Result<Result<T, GuestError>, PluginError>) -> Result<T, ProviderError> {
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(guest)) => Err(from_guest(guest)),
        Err(host) => Err(host.as_provider_error()),
    }
}

/// Guest stream kind → core stream kind.
///
/// An unrecognised kind becomes `Mp4`, the conservative reading: hand the URL to the player and
/// let it decide. Refusing outright would break a plugin that knew about a container we do not —
/// and mpv plays a great deal more than this enum names.
fn stream_kind_from(kind: &str) -> StreamKind {
    match kind.trim().to_ascii_lowercase().as_str() {
        "hls" | "m3u8" => StreamKind::Hls,
        "torrent-http" => StreamKind::TorrentHttp,
        "external-deeplink" | "external" => StreamKind::ExternalDeepLink,
        _ => StreamKind::Mp4,
    }
}

#[async_trait::async_trait]
impl Provider for WasmProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        let hits = flatten(self.plugin.search(query, translation_tag(translation)).await)?;
        Ok(hits
            .into_iter()
            .map(|hit| SearchHit {
                key: ProviderKey::new(hit.id),
                title: hit.title,
                // The WIT record has no synonyms field: a plugin that wanted to widen the match
                // surface would return several hits, which the scorer already handles.
                synonyms: Vec::new(),
                episode_count: hit.episode_count,
                year: hit.year.map(|y| y as u16),
                format: None,
            })
            .collect())
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        let episodes = flatten(
            self.plugin.list_episodes(key.as_str(), translation_tag(translation)).await,
        )?;
        Ok(episodes
            .into_iter()
            .map(|episode| Episode {
                number: anistream_core::media::EpisodeNumber::new(episode.number),
                title: episode.title,
                // Seconds across the ABI, a `Duration` in the core: WIT has no duration type, and
                // an integer is the one representation every guest language agrees on.
                duration: episode
                    .duration_secs
                    .map(|secs| std::time::Duration::from_secs(u64::from(secs))),
                // The ABI has no still-frame field, so a plugin cannot supply one. The
                // metadata layer fills it in later where it can.
                thumbnail: None,
            })
            .collect())
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        let streams = flatten(
            self.plugin.resolve(key.as_str(), episode, translation_tag(translation)).await,
        )?;

        Ok(streams
            .into_iter()
            .map(|stream| Stream {
                url: stream.url,
                kind: stream_kind_from(&stream.kind),
                // Plugins do not offer one. A durable download reference is a per-source concept
                // (a magnet, for the only downloadable source there is), and inventing an ABI field
                // for it before a plugin can produce one would be guessing at its shape.
                download_source: None,
                // Same reasoning: a pick explanation is the torrent ranker's concept.
                pick_note: None,
                quality: stream.quality,
                headers: stream.headers,
                subtitles: stream
                    .subtitles
                    .into_iter()
                    .map(|sub| Subtitle {
                        language: sub.language,
                        url: sub.url,
                        hard: sub.hard,
                    })
                    .collect(),
                // Stamped by the host, not the guest: attribution has to be trustworthy for the
                // Providers screen and for health accounting, so a plugin cannot claim to be
                // another source.
                provider_id: self.manifest.id.clone(),
            })
            .collect())
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        // `describe` rather than a throwaway search: it needs no network, so a health check
        // costs a provider site nothing and still proves the component instantiates and runs.
        self.plugin.describe().await.map(|_| ()).map_err(|e: PluginError| e.as_provider_error())
    }
}

/// Load every plugin in a directory as a provider.
///
/// Failures are logged and skipped rather than propagated: one unreadable file in a
/// user-writable directory must not take the whole source list down.
pub async fn load_providers(
    host: &engine::PluginHost,
    dir: impl AsRef<std::path::Path>,
) -> Vec<WasmProvider> {
    host.load_dir(dir)
        .await
        .into_iter()
        .filter_map(|result| match result {
            Ok(plugin) => Some(WasmProvider::new(plugin)),
            Err(e) => {
                tracing::warn!(error = %e, "skipping unloadable plugin");
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_survives_the_conversion() {
        // The single most important mapping here: `NotFound` must not trigger failover, so
        // flattening it into `Other` would make every missing episode try every source.
        let mapped = from_guest(GuestError::NotFound);
        assert_eq!(mapped, ProviderError::NotFound);
        assert!(!mapped.should_failover(), "a missing episode must not fail over");
    }

    #[test]
    fn blocked_and_parse_both_fail_over() {
        // The two signature source failures: bot protection, and the site changing shape.
        for error in [
            GuestError::Blocked("cloudflare".into()),
            GuestError::Parse("no player element".into()),
        ] {
            assert!(from_guest(error.clone()).should_failover(), "{error:?} should fail over");
        }
    }

    #[test]
    fn a_host_level_failure_flattens_into_a_provider_error() {
        // Callers should not have to know whether the guest returned an error or misbehaved.
        let deadline: Result<Result<(), GuestError>, PluginError> =
            Err(PluginError::Deadline {
                plugin: "p".into(),
                limit: std::time::Duration::from_secs(1),
            });
        let flattened = flatten(deadline).unwrap_err();
        assert!(matches!(flattened, ProviderError::Parse(_)));
        assert!(flattened.should_failover());
    }

    #[test]
    fn a_guest_error_flattens_without_being_reclassified() {
        let not_found: Result<Result<(), GuestError>, PluginError> =
            Ok(Err(GuestError::NotFound));
        assert_eq!(flatten(not_found).unwrap_err(), ProviderError::NotFound);
    }

    #[test]
    fn stream_kinds_map_from_the_documented_tags() {
        // The WIT documents exactly these, because the enum decides whether the player can render
        // a stream or has to hand it off.
        assert_eq!(stream_kind_from("hls"), StreamKind::Hls);
        assert_eq!(stream_kind_from("HLS"), StreamKind::Hls);
        assert_eq!(stream_kind_from("m3u8"), StreamKind::Hls);
        assert_eq!(stream_kind_from("mp4"), StreamKind::Mp4);
        assert_eq!(stream_kind_from("torrent-http"), StreamKind::TorrentHttp);
        assert_eq!(stream_kind_from("external-deeplink"), StreamKind::ExternalDeepLink);
    }

    #[test]
    fn an_unknown_stream_kind_is_handed_to_the_player_rather_than_refused() {
        // A plugin that knows about a container this host does not should still work — mpv plays
        // a great deal more than this enum names.
        assert_eq!(stream_kind_from("dash"), StreamKind::Mp4);
        assert_eq!(stream_kind_from("webm-but-newer"), StreamKind::Mp4);
        assert_eq!(stream_kind_from(""), StreamKind::Mp4);
    }

    #[test]
    fn the_handoff_kind_is_never_reached_by_accident() {
        // `ExternalDeepLink` means "we cannot play this" and routes to a browser. A typo landing
        // there would open a tab instead of playing, so only the exact tags produce it.
        for kind in ["externaldeeplink", "deeplink", "link", "hls "] {
            assert_ne!(
                stream_kind_from(kind),
                StreamKind::ExternalDeepLink,
                "{kind:?} must not become a handoff"
            );
        }
        assert_eq!(stream_kind_from("  external-deeplink  "), StreamKind::ExternalDeepLink);
    }

    #[test]
    fn translation_tags_round_trip() {
        // These strings cross the ABI, so a mismatch would silently ask for the wrong audio.
        for translation in [Translation::Sub, Translation::Dub] {
            assert_eq!(translation_from(translation_tag(translation)), Some(translation));
        }
        assert_eq!(translation_from("SUBBED"), Some(Translation::Sub));
        assert_eq!(translation_from("dubbed"), Some(Translation::Dub));
    }

    #[test]
    fn an_unknown_translation_is_dropped_rather_than_assumed() {
        // A plugin claiming something we cannot name must not have it turned into a claim it
        // does support subs.
        assert_eq!(translation_from("raw"), None);
        assert_eq!(translation_from(""), None);
    }
}
