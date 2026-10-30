//! What each TV played and refused, so the next cast to it starts with a plan it plays.
//!
//! In `cast-tvs.json` in the engine's data directory, beside `remote-tvs.json`, written whole to
//! a temporary file and renamed over the old one as the paired TVs are. A refusal is dropped
//! after 30 days, or as soon as the TV's firmware changes: an update may have added the codec.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use serde::{Deserialize, Serialize};

use crate::{client::VolumeControl, formats::CastPlan};

/// How long a refusal is believed, in seconds.
pub const REFUSAL_KEPT: i64 = 30 * 24 * 60 * 60;

/// What happened when a TV was handed one format part, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatAnswer {
    pub played: bool,
    /// Unix seconds.
    pub at: i64,
}

/// What the engine has learned about one TV.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CastCaps {
    /// The TXT `id`.
    pub id: String,
    pub name: String,
    pub model: String,
    /// `cast_build_revision` from 8008 `/setup/eureka_info`, when the TV answers it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    #[serde(default)]
    pub volume: VolumeControl,
    /// Each format part (`hevc-main10`, `opus`, `mkv`) with what happened and when.
    #[serde(default)]
    pub formats: BTreeMap<String, FormatAnswer>,
    /// The last cast to it went through this device: the picker's hint for the next.
    #[serde(default)]
    pub relayed_last: bool,
}

impl CastCaps {
    /// Whether the TV refused `part`, as far as is still believed.
    pub fn refused(&self, part: &str) -> bool {
        self.formats.get(part).is_some_and(|answer| !answer.played)
    }

    pub fn played(&self, part: &str) -> bool {
        self.formats.get(part).is_some_and(|answer| answer.played)
    }

    /// Note what happened to `part` at `at`.
    pub fn remember(&mut self, part: &str, played: bool, at: i64) {
        self.formats.insert(part.to_owned(), FormatAnswer { played, at });
    }

    /// Note what a plan did. Played: every part of it played. Refused: the blame goes to the
    /// first part not already known to play, video first, then audio, then the container, so a
    /// TV that plays HEVC isn't marked against it when only the Opus beside it was new. When
    /// every part is known to play, nothing is marked: it was the stream, not the format.
    pub fn record(&mut self, plan: &CastPlan, played: bool, at: i64) {
        let parts = plan.parts();
        if played {
            for part in parts {
                self.remember(&part, true, at);
            }
        } else if let Some(part) = parts.into_iter().find(|part| !self.played(part)) {
            self.remember(&part, false, at);
        }
        self.relayed_last = plan.relayed();
    }

    /// Note a plan the TV took and never played, with every byte it asked for served in good
    /// time. That says less than a refusal: the Nest Hub sits in `BUFFERING` on audio it can't
    /// decode, but a picture or a container it can't read is refused outright. So only a copied
    /// audio codec not known to play is blamed, and a plan without one teaches nothing. Returns
    /// the part blamed.
    pub fn record_stall(&mut self, plan: &CastPlan, at: i64) -> Option<String> {
        let part = plan.audio_part().filter(|part| !self.played(part))?;
        self.remember(&part, false, at);
        Some(part)
    }

    /// Drop the refusals older than [`REFUSAL_KEPT`].
    fn expire(&mut self, now: i64) {
        self.formats.retain(|_, answer| answer.played || now - answer.at < REFUSAL_KEPT);
    }
}

/// The clock the memory reads, so a test can move it.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

fn system_clock() -> Clock {
    Arc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    })
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

/// Every TV's [`CastCaps`].
pub struct CastMemory {
    path: Option<PathBuf>,
    tvs: Mutex<BTreeMap<String, CastCaps>>,
    clock: Clock,
}

impl std::fmt::Debug for CastMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CastMemory").field("path", &self.path).finish_non_exhaustive()
    }
}

impl CastMemory {
    /// Kept in memory only, for tests and for a build without a data directory.
    pub fn memory() -> Self {
        Self { path: None, tvs: Mutex::default(), clock: system_clock() }
    }

    /// The memory in `dir`. A file that can't be read is set aside and started afresh: what a
    /// TV plays is learnt again in a cast or two, and a cast should never fail over it.
    pub fn open(dir: &Path) -> Result<Self, MemoryError> {
        let path = dir.join("cast-tvs.json");
        let mut tvs = BTreeMap::new();
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<CastCaps>>(&bytes) {
                Ok(list) => tvs.extend(list.into_iter().map(|tv| (tv.id.clone(), tv))),
                Err(e) => {
                    tracing::warn!(error = %e, "cast-tvs.json unreadable; starting afresh");
                    let _ = std::fs::rename(&path, path.with_extension("json.unreadable"));
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(Self { path: Some(path), tvs: Mutex::new(tvs), clock: system_clock() })
    }

    /// The same memory, read by `clock` instead of the system's.
    #[must_use]
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, CastCaps>> {
        self.tvs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What is known of the TV `id`, without the refusals that have expired; a blank record for
    /// a TV never cast to.
    pub fn caps(&self, id: &str) -> CastCaps {
        let now = (self.clock)();
        let mut tvs = self.lock();
        match tvs.get_mut(id) {
            Some(caps) => {
                caps.expire(now);
                caps.clone()
            }
            None => CastCaps { id: id.to_owned(), ..CastCaps::default() },
        }
    }

    /// The TV `id` was reached, calling itself `name`, a `model`, with `firmware` when it said,
    /// and its volume works as `volume`. A changed firmware drops every refusal.
    pub fn seen(
        &self,
        id: &str,
        name: &str,
        model: &str,
        firmware: Option<&str>,
        volume: VolumeControl,
    ) -> Result<(), MemoryError> {
        {
            let mut tvs = self.lock();
            let caps = tvs
                .entry(id.to_owned())
                .or_insert_with(|| CastCaps { id: id.to_owned(), ..CastCaps::default() });
            if let Some(firmware) = firmware {
                if caps.firmware.as_deref().is_some_and(|known| known != firmware) {
                    tracing::info!(tv = %caps.name, "new firmware; forgetting what it refused");
                    caps.formats.retain(|_, answer| answer.played);
                }
                caps.firmware = Some(firmware.to_owned());
            }
            name.clone_into(&mut caps.name);
            model.clone_into(&mut caps.model);
            caps.volume = volume;
        }
        self.save()
    }

    /// Note what `plan` did on the TV `id`.
    pub fn record(&self, id: &str, plan: &CastPlan, played: bool) -> Result<(), MemoryError> {
        let now = (self.clock)();
        self.lock()
            .entry(id.to_owned())
            .or_insert_with(|| CastCaps { id: id.to_owned(), ..CastCaps::default() })
            .record(plan, played, now);
        self.save()
    }

    /// Note that `plan` stalled on the TV `id`, as [`CastCaps::record_stall`] says.
    pub fn record_stall(
        &self,
        id: &str,
        plan: &CastPlan,
    ) -> Result<Option<String>, MemoryError> {
        let now = (self.clock)();
        let blamed = self
            .lock()
            .entry(id.to_owned())
            .or_insert_with(|| CastCaps { id: id.to_owned(), ..CastCaps::default() })
            .record_stall(plan, now);
        if blamed.is_some() {
            self.save()?;
        }
        Ok(blamed)
    }

    /// Note what TV `id` did with one part alone: a capability clip's answer.
    pub fn note(&self, id: &str, part: &str, played: bool) -> Result<(), MemoryError> {
        let now = (self.clock)();
        self.lock()
            .entry(id.to_owned())
            .or_insert_with(|| CastCaps { id: id.to_owned(), ..CastCaps::default() })
            .remember(part, played, now);
        self.save()
    }

    fn save(&self) -> Result<(), MemoryError> {
        let Some(path) = &self.path else { return Ok(()) };
        let list: Vec<CastCaps> = self.lock().values().cloned().collect();
        let body = serde_json::to_vec_pretty(&list)?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, body)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;
    use crate::formats::{AudioCodec, AudioPlan, CastRoute, Delivery, VideoFormat};

    fn plan(video: VideoFormat, audio: AudioCodec, delivery: Delivery) -> CastPlan {
        CastPlan {
            route: CastRoute::Relayed { remux: true },
            video,
            height: None,
            audio: AudioPlan::Copy(audio),
            delivery,
        }
    }

    #[test]
    fn a_refusal_blames_the_first_part_not_known_to_play() {
        let mut caps = CastCaps::default();
        caps.record(&plan(VideoFormat::Hevc, AudioCodec::Aac, Delivery::HlsFmp4), true, 1);
        caps.record(&plan(VideoFormat::Hevc, AudioCodec::Opus, Delivery::HlsFmp4), false, 2);
        assert!(caps.played("hevc/1080/hls"));
        assert!(caps.refused("opus/hls"));
        assert!(!caps.refused("hls-fmp4"));
        assert!(caps.relayed_last);

        // Every part known to play: the stream's fault, not the format's.
        let before = caps.formats.clone();
        caps.record(&plan(VideoFormat::Hevc, AudioCodec::Aac, Delivery::HlsFmp4), false, 3);
        assert_eq!(caps.formats, before);
    }

    #[test]
    fn a_stall_blames_copied_audio_and_never_the_picture_or_the_container() {
        let mut caps = CastCaps::default();
        let mut tall = plan(VideoFormat::H264, AudioCodec::Aac, Delivery::HlsFmp4);
        tall.height = Some(1080);
        assert_eq!(caps.record_stall(&tall, 1), None);
        let mut mkv = plan(VideoFormat::Hevc, AudioCodec::Aac, Delivery::Mkv);
        mkv.route = CastRoute::Relayed { remux: false };
        assert_eq!(caps.record_stall(&mkv, 1), None);
        assert!(caps.formats.is_empty(), "{:?}", caps.formats);

        let flac = plan(VideoFormat::H264, AudioCodec::Flac, Delivery::HlsFmp4);
        assert_eq!(caps.record_stall(&flac, 2).as_deref(), Some("flac/hls"));
        assert!(caps.refused("flac/hls"));
        assert_eq!(caps.formats.len(), 1);
    }

    #[test]
    fn refusals_expire_and_a_new_firmware_forgets_them() {
        let dir = tempfile::tempdir().unwrap();
        let now = Arc::new(AtomicI64::new(1_000));
        let clock: Clock = {
            let now = Arc::clone(&now);
            Arc::new(move || now.load(Ordering::Relaxed))
        };
        let memory = CastMemory::open(dir.path()).unwrap().with_clock(Arc::clone(&clock));
        memory
            .seen("tv", "Living room", "Chromecast", Some("1.0"), VolumeControl::Level)
            .unwrap();
        memory
            .record(
                "tv",
                &plan(VideoFormat::HevcMain10, AudioCodec::Aac, Delivery::HlsFmp4),
                false,
            )
            .unwrap();
        assert!(memory.caps("tv").refused("hevc-main10/1080/hls"));

        // Read back from the file.
        let again = CastMemory::open(dir.path()).unwrap().with_clock(Arc::clone(&clock));
        let caps = again.caps("tv");
        assert!(caps.refused("hevc-main10/1080/hls"));
        assert_eq!(caps.name, "Living room");

        now.store(1_000 + REFUSAL_KEPT, Ordering::Relaxed);
        assert!(!again.caps("tv").refused("hevc-main10/1080/hls"), "30 days on");

        now.store(1_000, Ordering::Relaxed);
        let memory = CastMemory::open(dir.path()).unwrap().with_clock(clock);
        memory
            .record("tv", &plan(VideoFormat::H264, AudioCodec::Opus, Delivery::Mkv), false)
            .unwrap();
        memory
            .record("tv", &plan(VideoFormat::H264, AudioCodec::Aac, Delivery::HlsTs), true)
            .unwrap();
        memory
            .seen("tv", "Living room", "Chromecast", Some("1.1"), VolumeControl::MuteOnly)
            .unwrap();
        let caps = memory.caps("tv");
        assert!(!caps.refused("opus/progressive"), "the update may have added it");
        assert!(caps.played("hls-ts"), "what played stays");
        assert_eq!(caps.volume, VolumeControl::MuteOnly);
    }

    #[test]
    fn an_unreadable_file_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cast-tvs.json"), b"{not json").unwrap();
        let memory = CastMemory::open(dir.path()).unwrap();
        assert!(memory.caps("tv").formats.is_empty());
        assert!(dir.path().join("cast-tvs.json.unreadable").exists());
    }
}
