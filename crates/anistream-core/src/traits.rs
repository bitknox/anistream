//! The three pluggable seams: sources, players and trackers.
//!
//! Every volatile part of anistream sits behind one of these. That is the whole strategy:
//! streaming sites die, gain challenges, or change their markup, and when they do the fix
//! should be a config edit or a dropped-in `.wasm` file rather than a new release.
//!
//! [`Provider`] is deliberately the same shape as the `provider` interface in
//! `wit/anistream-provider.wit`, so a WASM plugin written in Go or TypeScript is
//! indistinguishable from a native Rust source at this boundary.

use async_trait::async_trait;

use crate::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::Stream,
};

/// What kind of source this is. Drives UI labelling and ranking, and decides whether a
/// resolved stream is playable locally or has to be handed off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// Native in-process web source.
    Native,
    /// WASM component plugin.
    Plugin,
    /// A self-hosted HTTP API (Consumet-shaped). The escape hatch when a source goes away.
    Remote,
    /// A configured indexer + embedded torrent session. Fails for entirely different reasons than
    /// web sources do, which is why it exists.
    Torrent,
    /// A licensed service we can browse and track but not decrypt — resolves to an
    /// external deep link. Crunchyroll is the case in point.
    Licensed,
}

impl ProviderKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Plugin => "plugin",
            Self::Remote => "remote",
            Self::Torrent => "torrent",
            Self::Licensed => "licensed",
        }
    }
}

/// Static description of a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderManifest {
    /// Stable machine id, used in config and as the health/cache key.
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub kind: ProviderKind,
    /// Hosts this provider is permitted to contact.
    ///
    /// For WASM plugins the host *enforces* this allowlist on every `fetch`, which is
    /// the main containment for what is otherwise a supply-chain surface: a plugin
    /// registry for sources means running other people's code.
    pub allowed_hosts: Vec<String>,
    pub translations: Vec<Translation>,
}

/// One selectable release a provider can offer for an episode.
///
/// Deliberately descriptive rather than playable: listing must be cheap — no torrent
/// session, no stream negotiation — so a candidate carries an opaque `id` the same
/// provider can turn into streams later via [`Provider::resolve_source`].
#[derive(Debug, Clone, PartialEq)]
pub struct SourceCandidate {
    /// Opaque identifier, meaningful only to the provider that produced it.
    pub id: String,
    /// The provider that produced this candidate, so a pick can be routed straight
    /// back to it rather than re-entering failover.
    pub provider_id: String,
    /// The release title as listed at the source.
    pub title: String,
    /// Vertical resolution, when the listing states one.
    pub quality: Option<u32>,
    /// Seeders, for swarm-backed sources. `None` where the concept does not apply.
    pub seeders: Option<u32>,
    /// Payload size as listed at the source, already human-readable (`"1.4 GiB"`).
    pub size: Option<String>,
    pub dual_audio: bool,
    pub dubbed: bool,
    /// Whether automatic resolution would pick this one, so the list can say which
    /// release the user is currently getting.
    pub auto_pick: bool,
}

/// A source of episodes and streams.
#[async_trait]
pub trait Provider: Send + Sync {
    fn manifest(&self) -> &ProviderManifest;

    /// Find candidate titles. For site-backed sources this is the *primary* identification
    /// path, not a fallback: site catalogues key on their own opaque ids, which
    /// appear in no mapping dataset, so there is nothing to look up.
    async fn search(
        &self,
        query: &str,
        translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError>;

    async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError>;

    /// Resolve playable streams, best-first where the provider has an opinion.
    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError>;

    /// The selectable releases for an episode, best-first, without starting any of them.
    ///
    /// Optional, and not part of the WIT plugin interface: an empty list means "nothing
    /// to choose between here", which is the honest answer for sources that resolve to
    /// exactly one stream anyway.
    async fn sources(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        Ok(Vec::new())
    }

    /// Resolve one specific candidate from [`Self::sources`] by its id.
    ///
    /// The default refuses rather than falling back to [`Self::resolve`]: silently
    /// substituting the automatic pick for the one the user chose would be worse than
    /// failing.
    async fn resolve_source(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
        _source_id: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        Err(ProviderError::NotFound)
    }

    /// Cheap liveness probe for the Providers screen. Default implementation runs a
    /// throwaway search, which is a fair proxy for "is this source answering".
    async fn health_check(&self) -> Result<(), ProviderError> {
        self.search("a", Translation::Sub).await.map(|_| ())
    }

    /// Whether this provider is currently willing to serve.
    ///
    /// Distinct from health: the torrent provider reports `false` here whenever the VPN
    /// guard is failing, which is a local policy decision rather than a sign the source
    /// is broken. Returning `false` keeps it out of resolution entirely.
    fn is_available(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

/// Something that can play a [`Stream`].
///
/// mpv over JSON IPC is the reference implementation, but the trait is what lets the
/// licensed path work at all: Crunchyroll episodes resolve to an external deep link and
/// are opened by a handoff player, needing no new machinery anywhere else.
#[async_trait]
pub trait Player: Send + Sync {
    fn id(&self) -> &str;

    /// Whether this player can handle a given stream. The registry uses it to route an
    /// `ExternalDeepLink` to the handoff player and everything else to mpv.
    fn supports(&self, stream: &Stream) -> bool;

    /// Begin playback. Returns once playback has *started*, not once it finishes.
    async fn play(&self, stream: &Stream, session: PlaybackRequest)
    -> Result<(), crate::Error>;
}

/// Context handed to a player alongside the stream.
#[derive(Debug, Clone, Default)]
pub struct PlaybackRequest {
    /// Window/OSD title.
    pub title: String,
    /// Where to resume from, in seconds.
    pub start_at: Option<f64>,
    /// Preferred subtitle language tag.
    pub subtitle_language: Option<String>,
    /// Carried across episodes within a series so a chosen speed sticks.
    pub speed: Option<f64>,
    /// Carried across sessions so a chosen volume sticks, in mpv's 0–100 scale.
    pub volume: Option<f64>,
    /// Whether the viewer wants dubbed audio. Drives track selection: audio in their
    /// own language with signs-only subtitles, instead of original audio with full
    /// subtitles. Track order in the file never decides.
    pub dub: bool,
}

/// An external service that holds watch progress.
///
/// Local SQLite history is always the source of truth; a tracker is a *projection* of
/// it. That ordering is what makes the app work with no account and no network, and it
/// is why `push` must be idempotent — the outbox will retry.
#[async_trait]
pub trait Tracker: Send + Sync {
    fn id(&self) -> &str;

    /// Whether credentials are present. `false` means degrade to local-only rather than
    /// erroring.
    fn is_authenticated(&self) -> bool;

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, crate::Error>;

    /// Apply a batch of operations. Must be safe to call twice with the same input.
    async fn push(&self, ops: &[TrackOp]) -> Result<(), crate::Error>;

    /// Drop any in-memory credential.
    ///
    /// Signing out clears the stored token, but a tracker that cached one at construction would
    /// keep using it — reporting itself as connected and pushing with a credential the user has
    /// revoked. Default no-op for trackers that read the store on every call.
    async fn forget_credentials(&self) {}

    /// Take a credential obtained while the app was already running.
    ///
    /// The mirror of [`Self::forget_credentials`], and it exists for a reason worth recording:
    /// signing in used to end with "restart to start syncing", because every tracker captured its
    /// token at construction and nothing could hand it a new one. The token reached the keychain and
    /// the running session never saw it.
    async fn accept_credentials(
        &self,
        _access: &str,
        _refresh: Option<&str>,
        _expires_at: Option<i64>,
    ) {
    }
}

/// One title's state on a tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedEntry {
    pub anilist_id: crate::ids::AnilistId,
    pub progress: u32,
    pub status: WatchStatus,
    pub score: Option<f32>,
}

/// A pending change to push to a tracker.
///
/// Serialisable because these are persisted in the durable outbox — progress recorded
/// while offline has to survive a process kill, so the queue lives in SQLite rather than
/// in memory.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TrackOp {
    /// Progress is monotonic, so conflicts resolve to `max(local, remote)` without
    /// needing a timestamp or user intervention.
    SetProgress {
        anilist_id: crate::ids::AnilistId,
        episode: u32,
    },
    /// Status is *not* monotonic, so this carries a timestamp for last-write-wins and
    /// genuine divergence is surfaced rather than silently resolved.
    SetStatus {
        anilist_id: crate::ids::AnilistId,
        status: WatchStatus,
        at: i64,
    },
    SetScore {
        anilist_id: crate::ids::AnilistId,
        score: f32,
        at: i64,
    },
}

impl TrackOp {
    pub const fn anilist_id(&self) -> crate::ids::AnilistId {
        match self {
            Self::SetProgress { anilist_id, .. }
            | Self::SetStatus { anilist_id, .. }
            | Self::SetScore { anilist_id, .. } => *anilist_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchStatus {
    Current,
    Planning,
    Completed,
    Paused,
    Dropped,
    Repeating,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_ops_expose_their_subject_uniformly() {
        let id = crate::ids::AnilistId::new(154_587);
        let ops = [
            TrackOp::SetProgress { anilist_id: id, episode: 12 },
            TrackOp::SetStatus { anilist_id: id, status: WatchStatus::Current, at: 0 },
            TrackOp::SetScore { anilist_id: id, score: 9.0, at: 0 },
        ];
        for op in &ops {
            assert_eq!(op.anilist_id(), id);
        }
    }

    #[test]
    fn provider_kind_labels_are_stable() {
        // These strings appear in config files and on the Providers screen, so they are
        // part of the user-facing contract.
        assert_eq!(ProviderKind::Torrent.as_str(), "torrent");
        assert_eq!(ProviderKind::Licensed.as_str(), "licensed");
    }
}
