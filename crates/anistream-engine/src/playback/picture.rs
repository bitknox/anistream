//! What the picture is and how hard it can be pushed: HDR detection, tone-mapping choices,
//! and the frame-drop governor that eases upscaling when the GPU cannot keep up.
//!
//! Everything here is pure. The host feeds it mpv properties and timer samples; nothing in
//! this module talks to mpv or reads a clock, so every decision is reproducible in a test.
//!
//! ## HDR in mpv 0.41, and what anistream must leave alone
//!
//! mpv already does the right thing by default, and the way to keep it doing so is to not
//! touch it. Defaults on 0.41 (`DOCS/man/options.rst` at `v0.41.0`, checked on a Mac with an
//! XDR panel):
//!
//! - `vo=gpu-next` is the default VO; `tone-mapping=auto` means `spline` there (`bt.2390`
//!   on the old `gpu` VO), with `hdr-compute-peak=auto` measuring each scene's real peak and
//!   `gamut-mapping-mode=auto` bringing BT.2020 colour into the display's gamut.
//! - `target-colorspace-hint=auto` tags the swapchain with the display's colourspace, which
//!   is HDR passthrough on an HDR display, only where mpv can read the display's
//!   capabilities: Wayland, D3D11 and winvk. There an HDR display gets PQ and mpv
//!   tone-maps only what exceeds the display's own peak.
//! - On macOS (`macvk`) `auto` sets nothing, so HDR is tone-mapped to SDR even on an XDR
//!   panel: `video-target-params` reads `gamma2.2` / `bt.709` / 203 nits for a PQ file.
//!   `target-colorspace-hint=yes` switches the target to PQ / BT.2020; that is a user
//!   choice anistream could offer later, not a default to flip silently.
//!
//! So the host must not set `vo`, `gpu-api`/`gpu-context`, `target-colorspace-hint`,
//! `target-trc`/`target-prim`/`target-peak`, `hdr-compute-peak`, `gamut-mapping-mode`
//! or the ICC options, and sets `tone-mapping` only when the user picks something from the
//! menu. Each of those either breaks passthrough on the platforms that have it or replaces
//! a measured value with a guess.

use std::collections::VecDeque;

use serde_json::Value;

/// The HDR formats anistream names on the badge.
///
/// Only what mpv 0.41 reports reliably. The `video-params` sub-properties come from
/// `player/command.c` (`property_imgparams`) and were confirmed against synthetic PQ and HLG
/// encodes; Dolby Vision and HDR10+ come from the same source with no sample at hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdrKind {
    /// PQ transfer, static metadata or none. The name viewers know for "PQ HDR".
    Hdr10,
    /// PQ with per-scene HDR10+ (ST 2094-40) metadata on the current frame.
    Hdr10Plus,
    /// Hybrid log-gamma.
    Hlg,
    /// mpv is applying a Dolby Vision RPU, or the stream is profile 5 (no usable base layer).
    DolbyVision,
}

impl HdrKind {
    /// The badge text, exactly as the protocol's `media.hdr` spells it.
    pub fn badge(self) -> &'static str {
        match self {
            HdrKind::Hdr10 => "HDR10",
            HdrKind::Hdr10Plus => "HDR10+",
            HdrKind::Hlg => "HLG",
            HdrKind::DolbyVision => "Dolby Vision",
        }
    }
}

/// Classify the current video from `video-params` and `current-tracks/video`.
///
/// Fields read, as mpv 0.41 names them:
///
/// - `video-params/colormatrix == "dolbyvision"`: libplacebo mapped a single-layer RPU and
///   is reshaping with it. This is the one reliable sign that what is on screen is Dolby
///   Vision; a profile 7 stream with a full enhancement layer is played from its HDR10 base
///   and reports `bt.2020-ncl`, which is then the honest badge.
/// - `current-tracks/video/dolby-vision-profile`: from the container's DOVI record.
///   Profile 5 has no backward-compatible base layer, so it is Dolby Vision whatever the
///   decoder did; other profiles defer to what `video-params` says is being shown.
/// - `video-params/gamma`: `pq` or `hlg`. Primaries are not consulted — PQ in a P3
///   container is still HDR, and BT.2020 alone (an SDR wide-gamut file) is not.
/// - `video-params/scene-max-r|g|b` and `scene-avg`: present only while the current frame
///   carries HDR10+ dynamic metadata (`PL_HDR_METADATA_HDR10PLUS`), so the badge can move
///   from HDR10 to HDR10+ once the first such frame decodes.
///
/// Not used: `sig-peak` (deprecated) and `max-luma`/`max-cll`, which libplacebo fills with
/// nominal values (1000 nits for HLG, 10000 for bare PQ) when a file carries none, so they
/// do not prove static metadata exists.
pub fn classify(video_params: &Value, video_track: Option<&Value>) -> Option<HdrKind> {
    let text = |key: &str| video_params.get(key).and_then(Value::as_str);
    if text("colormatrix") == Some("dolbyvision") {
        return Some(HdrKind::DolbyVision);
    }
    let profile =
        video_track.and_then(|t| t.get("dolby-vision-profile")).and_then(Value::as_i64);
    if profile == Some(5) {
        return Some(HdrKind::DolbyVision);
    }
    match text("gamma")? {
        "hlg" => Some(HdrKind::Hlg),
        "pq" => {
            let dynamic = ["scene-max-r", "scene-max-g", "scene-max-b", "scene-avg"]
                .iter()
                .any(|k| video_params.get(*k).and_then(Value::as_f64).is_some_and(|v| v > 0.0));
            Some(if dynamic { HdrKind::Hdr10Plus } else { HdrKind::Hdr10 })
        }
        _ => None,
    }
}

/// One tone-mapping choice for the in-player menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToneMapping {
    /// The value for mpv's `tone-mapping` property.
    pub id: &'static str,
    pub label: &'static str,
}

/// The default, and what the host should leave mpv on unless the user picks otherwise.
pub const DEFAULT_TONE_MAPPING: &str = "auto";

/// Tone-mapping curves worth offering, best-first, all valid on mpv 0.39–0.41 with
/// `gpu-next` (`mpv --list-options` on 0.41 lists these plus `linear`, `gamma` and
/// `st2094-10`).
///
/// Left out: `linear` and `gamma` stretch or fit the whole range and are calibration
/// tools, not looks; `st2094-10` is, in mpv's own words, generally worse than
/// `st2094-40`. Shown only while an HDR source plays — on SDR the menu is `null`.
pub const TONE_MAPPINGS: &[ToneMapping] = &[
    ToneMapping { id: "auto", label: "Automatic" },
    ToneMapping { id: "spline", label: "Spline" },
    ToneMapping { id: "bt.2390", label: "BT.2390" },
    ToneMapping { id: "bt.2446a", label: "BT.2446a" },
    ToneMapping { id: "st2094-40", label: "ST 2094-40 (HDR10+)" },
    ToneMapping { id: "hable", label: "Hable" },
    ToneMapping { id: "mobius", label: "Möbius" },
    ToneMapping { id: "reinhard", label: "Reinhard" },
    ToneMapping { id: "clip", label: "Clip" },
];

/// The `menus.tone_mapping` value for the companion script: `null` for SDR, otherwise
/// `{"current", "options": [{"id", "label"}]}`. An unknown `current` (a value set by hand
/// in `mpv.conf`) falls back to `auto` rather than leaving no row marked.
pub fn tone_mapping_menu(hdr: Option<HdrKind>, current: &str) -> Value {
    if hdr.is_none() {
        return Value::Null;
    }
    let current =
        TONE_MAPPINGS.iter().find(|t| t.id == current).map_or(DEFAULT_TONE_MAPPING, |t| t.id);
    serde_json::json!({
        "current": current,
        "options": TONE_MAPPINGS
            .iter()
            .map(|t| serde_json::json!({ "id": t.id, "label": t.label }))
            .collect::<Vec<_>>(),
    })
}

/// The governor's verdict: re-choose the shader chain with `Budget::eased = to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ease {
    pub to: u8,
}

/// Decides when frame drops are the GPU losing to the shader chain, and eases one step.
///
/// Fed `(time, frame-drop-count, paused)` samples:
///
/// - `time` is the host's monotonic clock in seconds — not the playback position, which
///   jumps on every seek.
/// - `drops` is mpv's cumulative `frame-drop-count` (frames the VO dropped for lateness,
///   counted with the default `video-sync=audio`).
/// - `paused` should be `pause || paused-for-cache || seeking`: anything that stops frames
///   from being due is not playback the GPU can be judged on.
///
/// Feed at least once a second while a file plays (a 1 s tick, plus property changes if
/// convenient — the result does not depend on the rate).
///
/// **What counts as sustained.** Over the last [`WINDOW`](Self::WINDOW) seconds of active
/// playback, at least [`MIN_DROPS`](Self::MIN_DROPS) drops spread over at least
/// [`MIN_BAD_SECONDS`](Self::MIN_BAD_SECONDS) distinct seconds. A chain that is over
/// budget drops in nearly every second (the M4 Pro measurements in the report: 6–10 a
/// second); a seek, a resume or a stutter in another app drops a burst in one or two.
///
/// **Grace.** Drops in the first [`GRACE`](Self::GRACE) seconds after a file starts,
/// playback resumes, the counter resets or a sample gap of more than
/// [`MAX_GAP`](Self::MAX_GAP) seconds (sleep, a stalled host) are not counted: that is
/// when mpv compiles shaders and refills its queues.
///
/// **Hysteresis.** An ease clears the window and restarts the grace, because the new chain
/// compiles with a hitch of its own; the next step needs a full fresh window of evidence,
/// so steps are at least `GRACE + WINDOW` (13 s) apart. Steps only go down: nothing in an
/// episode makes the governor step back up — a GPU that kept up for a minute at the lighter
/// chain says nothing about the heavier one, and flapping between them is worse than either.
/// It stops at `cap` steps.
#[derive(Debug, Clone)]
pub struct Governor {
    cap: u8,
    eased: u8,
    last: Option<(f64, u64)>,
    paused: bool,
    grace_left: f64,
    active: f64,
    /// `(active seconds, drops)` for each sample that saw drops, within the window.
    events: VecDeque<(f64, u64)>,
}

impl Default for Governor {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAP)
    }
}

impl Governor {
    /// Seconds of active playback the evidence must span.
    pub const WINDOW: f64 = 10.0;
    /// Drops within the window that make it a problem: one a second, about 4% of a 24 fps
    /// stream — the point where judder is visible on a pan.
    pub const MIN_DROPS: u64 = 10;
    /// Distinct seconds within the window that must have dropped: half of them. A burst is
    /// one or two.
    pub const MIN_BAD_SECONDS: usize = 5;
    /// Active seconds ignored after a start, a resume, a reset or an ease.
    pub const GRACE: f64 = 3.0;
    /// A sample gap longer than this is a discontinuity, not playback.
    pub const MAX_GAP: f64 = 5.0;
    /// Two steps: full → light → chroma only (see [`crate::shaders::choose`]). Below that
    /// the shaders are no longer what is dropping frames.
    pub const DEFAULT_CAP: u8 = 2;

    pub fn new(cap: u8) -> Self {
        Self {
            cap,
            eased: 0,
            last: None,
            paused: false,
            grace_left: Self::GRACE,
            active: 0.0,
            events: VecDeque::new(),
        }
    }

    /// Steps taken so far; pass as `Budget::eased`.
    pub fn eased(&self) -> u8 {
        self.eased
    }

    /// A new file in the same session: keep the level, forget the evidence.
    ///
    /// Episodes of one series from one source are encoded alike, so a GPU that could not
    /// hold the full chain in episode 1 will not in episode 2 either — starting over would
    /// replay thirteen seconds of judder and the same toast every episode.
    pub fn new_file(&mut self) {
        self.restart(None);
    }

    /// Back to the full chain: the user picked a look, or the source tier changed.
    pub fn reset(&mut self) {
        self.eased = 0;
        self.restart(None);
    }

    /// One sample. Returns an [`Ease`] when the chain should step down now.
    pub fn sample(&mut self, time: f64, drops: u64, paused: bool) -> Option<Ease> {
        let Some((last_time, last_drops)) = self.last else {
            self.restart(Some((time, drops)));
            return None;
        };
        let dt = time - last_time;
        if !(0.0..=Self::MAX_GAP).contains(&dt) || drops < last_drops {
            // Sleep, a host stall, a clock oddity or mpv's counter starting over with a new
            // file: whatever happened in between is not evidence.
            self.restart(Some((time, drops)));
            return None;
        }
        self.last = Some((time, drops));

        if paused {
            self.paused = true;
            return None;
        }
        if self.paused {
            // Resuming. The interval just ended was paused time, and the next few seconds
            // are a refill: start over rather than judge either.
            self.restart(Some((time, drops)));
            return None;
        }

        if self.grace_left > 0.0 {
            self.grace_left -= dt;
            return None;
        }
        if self.eased >= self.cap {
            return None;
        }

        self.active += dt;
        let delta = drops - last_drops;
        if delta > 0 {
            self.events.push_back((self.active, delta));
        }
        while self.events.front().is_some_and(|(at, _)| *at <= self.active - Self::WINDOW) {
            self.events.pop_front();
        }
        if self.active < Self::WINDOW {
            return None;
        }

        let total: u64 = self.events.iter().map(|(_, n)| n).sum();
        let mut seconds: Vec<i64> =
            self.events.iter().map(|(at, _)| at.ceil() as i64).collect();
        seconds.dedup();
        if total < Self::MIN_DROPS || seconds.len() < Self::MIN_BAD_SECONDS {
            return None;
        }

        self.eased += 1;
        self.restart(Some((time, drops)));
        Some(Ease { to: self.eased })
    }

    fn restart(&mut self, at: Option<(f64, u64)>) {
        self.last = at;
        self.paused = false;
        self.grace_left = Self::GRACE;
        self.active = 0.0;
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- HDR ---------------------------------------------------------------------------

    /// What mpv 0.41 reported for an x265 PQ encode with mastering metadata (trimmed).
    fn pq_params() -> Value {
        json!({
            "colormatrix": "bt.2020-ncl", "colorlevels": "limited", "primaries": "bt.2020",
            "gamma": "pq", "light": "display", "max-cll": 1000.0, "max-fall": 400.0,
            "max-luma": 1000.0, "min-luma": 0.0001, "sig-peak": 4.926108,
            "pixelformat": "yuv420p10", "w": 1920, "h": 1080
        })
    }

    #[test]
    fn sdr_is_not_hdr() {
        let params = json!({
            "colormatrix": "bt.709", "primaries": "bt.709", "gamma": "bt.1886",
            "light": "display", "sig-peak": 0.0
        });
        assert_eq!(classify(&params, None), None);
    }

    #[test]
    fn no_params_yet_is_not_hdr() {
        assert_eq!(classify(&Value::Null, None), None);
        assert_eq!(classify(&json!({}), None), None);
    }

    #[test]
    fn pq_with_static_metadata_is_hdr10() {
        assert_eq!(classify(&pq_params(), None), Some(HdrKind::Hdr10));
    }

    #[test]
    fn bare_pq_is_still_hdr10() {
        // mpv's report for a PQ encode with no metadata: libplacebo fills a nominal peak.
        let params = json!({
            "colormatrix": "bt.2020-ncl", "primaries": "bt.2020", "gamma": "pq",
            "max-luma": 10000.0, "max-cll": 0.0, "sig-peak": 49.261086
        });
        assert_eq!(classify(&params, None), Some(HdrKind::Hdr10));
    }

    #[test]
    fn hlg_is_hlg() {
        let params = json!({
            "colormatrix": "bt.2020-ncl", "primaries": "bt.2020", "gamma": "hlg",
            "light": "hlg", "max-luma": 1000.0
        });
        assert_eq!(classify(&params, None), Some(HdrKind::Hlg));
    }

    #[test]
    fn bt2020_sdr_is_not_hdr() {
        let params =
            json!({ "colormatrix": "bt.2020-ncl", "primaries": "bt.2020", "gamma": "bt.1886" });
        assert_eq!(classify(&params, None), None);
    }

    #[test]
    fn scene_metadata_on_the_frame_makes_it_hdr10_plus() {
        let mut params = pq_params();
        params["scene-max-r"] = json!(812.0);
        params["scene-max-g"] = json!(790.5);
        params["scene-max-b"] = json!(640.0);
        params["scene-avg"] = json!(120.0);
        assert_eq!(classify(&params, None), Some(HdrKind::Hdr10Plus));
    }

    #[test]
    fn an_applied_rpu_is_dolby_vision() {
        let mut params = pq_params();
        params["colormatrix"] = json!("dolbyvision");
        let track =
            json!({ "codec": "hevc", "dolby-vision-profile": 8, "dolby-vision-level": 6 });
        assert_eq!(classify(&params, Some(&track)), Some(HdrKind::DolbyVision));
    }

    #[test]
    fn profile_8_played_from_its_base_layer_is_badged_as_what_is_shown() {
        let track =
            json!({ "codec": "hevc", "dolby-vision-profile": 8, "dolby-vision-level": 6 });
        assert_eq!(classify(&pq_params(), Some(&track)), Some(HdrKind::Hdr10));
    }

    #[test]
    fn profile_5_is_dolby_vision_even_before_the_first_frame() {
        let track = json!({ "codec": "hevc", "dolby-vision-profile": 5 });
        assert_eq!(classify(&Value::Null, Some(&track)), Some(HdrKind::DolbyVision));
    }

    #[test]
    fn badges_match_the_protocol() {
        let badges: Vec<_> =
            [HdrKind::Hdr10, HdrKind::Hdr10Plus, HdrKind::Hlg, HdrKind::DolbyVision]
                .map(HdrKind::badge)
                .to_vec();
        assert_eq!(badges, ["HDR10", "HDR10+", "HLG", "Dolby Vision"]);
    }

    #[test]
    fn tone_mapping_menu_is_null_for_sdr_and_marks_auto_by_default() {
        assert_eq!(tone_mapping_menu(None, "auto"), Value::Null);
        let menu = tone_mapping_menu(Some(HdrKind::Hdr10), "auto");
        assert_eq!(menu["current"], "auto");
        assert_eq!(menu["options"][0], json!({ "id": "auto", "label": "Automatic" }));
        let unknown = tone_mapping_menu(Some(HdrKind::Hlg), "linear");
        assert_eq!(unknown["current"], "auto");
    }

    #[test]
    fn tone_mapping_ids_are_values_mpv_041_accepts() {
        // From `mpv --list-options` on 0.41.0.
        let accepted = [
            "auto",
            "clip",
            "mobius",
            "reinhard",
            "hable",
            "gamma",
            "linear",
            "spline",
            "bt.2390",
            "bt.2446a",
            "st2094-40",
            "st2094-10",
        ];
        for t in TONE_MAPPINGS {
            assert!(accepted.contains(&t.id), "{} is not an mpv tone-mapping value", t.id);
        }
        assert_eq!(TONE_MAPPINGS[0].id, DEFAULT_TONE_MAPPING);
    }

    // --- Governor ----------------------------------------------------------------------

    /// Plays `seconds` at `hz` samples a second from `start`, with `drops_at(t)` giving the
    /// cumulative counter, and returns every ease with the time it happened.
    fn play(
        g: &mut Governor,
        start: f64,
        seconds: f64,
        hz: f64,
        paused: impl Fn(f64) -> bool,
        drops_at: impl Fn(f64) -> u64,
    ) -> Vec<(f64, u8)> {
        let mut eases = Vec::new();
        let steps = (seconds * hz).round() as usize;
        for i in 0..=steps {
            let t = start + i as f64 / hz;
            if let Some(Ease { to }) = g.sample(t, drops_at(t), paused(t)) {
                eases.push((t, to));
            }
        }
        eases
    }

    fn never(_: f64) -> bool {
        false
    }

    #[test]
    fn smooth_playback_never_eases() {
        let mut g = Governor::default();
        assert!(play(&mut g, 0.0, 600.0, 1.0, never, |_| 0).is_empty());
        assert_eq!(g.eased(), 0);
    }

    #[test]
    fn an_overloaded_chain_eases_after_grace_and_a_full_window_then_again_then_stops() {
        // ~8 drops a second: what Ani4Kv2 at 1080p did on an M4 Pro.
        let mut g = Governor::default();
        let eases = play(&mut g, 0.0, 120.0, 1.0, never, |t| (t * 8.0) as u64);
        assert_eq!(eases.len(), 2, "{eases:?}");
        assert_eq!(eases[0].1, 1);
        assert_eq!(eases[1].1, 2);
        // Grace (3 s) plus a full window (10 s), each time.
        assert!((12.0..=14.0).contains(&eases[0].0), "{eases:?}");
        assert!((eases[1].0 - eases[0].0) >= Governor::GRACE + Governor::WINDOW - 1.0);
        assert_eq!(g.eased(), Governor::DEFAULT_CAP);
    }

    #[test]
    fn a_shader_compile_hitch_at_start_is_forgiven() {
        // 30 frames lost in the first two seconds, then clean.
        let mut g = Governor::default();
        let eases =
            play(
                &mut g,
                0.0,
                120.0,
                1.0,
                never,
                |t| if t < 2.0 { (t * 15.0) as u64 } else { 30 },
            );
        assert!(eases.is_empty(), "{eases:?}");
    }

    #[test]
    fn a_seek_burst_mid_episode_is_not_sustained() {
        // A seek that the host did not flag: 18 drops inside one second at t = 300.
        let mut g = Governor::default();
        let eases = play(&mut g, 0.0, 600.0, 1.0, never, |t| if t < 300.0 { 0 } else { 18 });
        assert!(eases.is_empty(), "{eases:?}");
    }

    #[test]
    fn bursts_after_each_resume_are_forgiven() {
        // Paused for the last 20 s of every minute; the start and each resume cost 12
        // frames over two seconds.
        let paused = |t: f64| (t % 60.0) >= 40.0;
        let drops =
            |t: f64| 12 * (t / 60.0).floor() as u64 + ((t % 60.0) * 6.0).min(12.0) as u64;
        let mut g = Governor::default();
        let eases = play(&mut g, 0.0, 600.0, 1.0, paused, drops);
        assert!(eases.is_empty(), "{eases:?}");
    }

    #[test]
    fn occasional_drops_do_not_ease() {
        // One drop every three seconds, forever: under the bar on both counts.
        let mut g = Governor::default();
        let eases = play(&mut g, 0.0, 600.0, 1.0, never, |t| (t / 3.0) as u64);
        assert!(eases.is_empty(), "{eases:?}");
    }

    #[test]
    fn a_dense_burst_of_many_drops_in_few_seconds_does_not_ease() {
        // 40 drops in three seconds at t = 100 (a background app spiking the GPU).
        let mut g = Governor::default();
        let eases = play(&mut g, 0.0, 300.0, 1.0, never, |t| match t {
            t if t < 100.0 => 0,
            t if t < 103.0 => ((t - 100.0) * 13.4) as u64,
            _ => 40,
        });
        assert!(eases.is_empty(), "{eases:?}");
    }

    #[test]
    fn one_drop_every_second_is_the_threshold() {
        let mut g = Governor::default();
        let eases = play(&mut g, 0.0, 30.0, 1.0, never, |t| t as u64);
        assert_eq!(eases.first().map(|e| e.1), Some(1), "{eases:?}");
    }

    #[test]
    fn the_sample_rate_does_not_change_the_decision() {
        let overloaded = |t: f64| (t * 7.0) as u64;
        let at_1hz = play(&mut Governor::default(), 0.0, 60.0, 1.0, never, overloaded);
        let at_4hz = play(&mut Governor::default(), 0.0, 60.0, 4.0, never, overloaded);
        assert_eq!(at_1hz.len(), at_4hz.len());
        for (a, b) in at_1hz.iter().zip(&at_4hz) {
            assert_eq!(a.1, b.1);
            assert!((a.0 - b.0).abs() <= 1.0, "{at_1hz:?} vs {at_4hz:?}");
        }
        let smooth_4hz =
            play(&mut Governor::default(), 0.0, 60.0, 4.0, never, |t| (t / 3.0) as u64);
        assert!(smooth_4hz.is_empty());
    }

    #[test]
    fn a_counter_reset_is_not_negative_drops() {
        let mut g = Governor::default();
        assert!(play(&mut g, 0.0, 30.0, 1.0, never, |_| 500).is_empty());
        // mpv's counter starts over with the next file.
        assert!(play(&mut g, 31.0, 60.0, 1.0, never, |_| 0).is_empty());
        assert_eq!(g.eased(), 0);
    }

    #[test]
    fn a_long_gap_is_a_discontinuity_not_drops() {
        let mut g = Governor::default();
        assert!(play(&mut g, 0.0, 20.0, 1.0, never, |_| 0).is_empty());
        // The laptop slept for an hour; mpv dropped a pile of frames catching up.
        assert!(
            play(&mut g, 3620.0, 60.0, 1.0, never, |t| if t < 3622.0 { 0 } else { 45 })
                .is_empty()
        );
        assert_eq!(g.eased(), 0);
    }

    #[test]
    fn steps_never_go_back_up_within_an_episode() {
        let mut g = Governor::default();
        let eases = play(&mut g, 0.0, 20.0, 1.0, never, |t| (t * 8.0) as u64);
        assert_eq!(eases.len(), 1);
        let drops_then = (20.0 * 8.0) as u64;
        assert!(play(&mut g, 21.0, 600.0, 1.0, never, |_| drops_then).is_empty());
        assert_eq!(g.eased(), 1);
    }

    #[test]
    fn new_file_keeps_the_level_and_reset_drops_it() {
        let mut g = Governor::default();
        play(&mut g, 0.0, 20.0, 1.0, never, |t| (t * 8.0) as u64);
        assert_eq!(g.eased(), 1);
        g.new_file();
        assert_eq!(g.eased(), 1);
        // The new file's own start-up hitch is forgiven like any other.
        assert!(
            play(&mut g, 100.0, 2.0, 1.0, never, |t| ((t - 100.0) * 20.0) as u64).is_empty()
        );
        g.reset();
        assert_eq!(g.eased(), 0);
    }

    #[test]
    fn a_cap_of_zero_never_eases() {
        let mut g = Governor::new(0);
        assert!(play(&mut g, 0.0, 120.0, 1.0, never, |t| (t * 8.0) as u64).is_empty());
    }

    #[test]
    fn paused_time_counts_for_nothing() {
        // Drops reported while paused (a redraw on resize, say) are not playback drops.
        let mut g = Governor::default();
        let eases = play(
            &mut g,
            0.0,
            120.0,
            1.0,
            |t| (10.0..100.0).contains(&t),
            |t| {
                if (10.0..100.0).contains(&t) {
                    ((t - 10.0) * 5.0) as u64
                } else if t < 10.0 {
                    0
                } else {
                    450
                }
            },
        );
        assert!(eases.is_empty(), "{eases:?}");
    }
}
