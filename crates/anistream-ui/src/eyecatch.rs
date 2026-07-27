//! The eyecatch (アイキャッチ) — the wipe that covers stream resolution.
//!
//! Named for the bumper before an ad break. It is the app's only orchestrated motion, and it
//! sits here deliberately: resolving a stream is slow and failure-prone, so the one moment of
//! delight covers the weakest seam rather than decorating a fast path.
//!
//! Pure state, advanced one frame at a time by the event loop, so it can be tested without a
//! terminal and cannot depend on wall-clock timing.

/// Frames the band takes to sweep across the stage, at the animation tick rate.
///
/// [`FRAME_MS`] × [`SWEEP_FRAMES`] ≈ 150 ms per half, so a full cover-and-reveal is the ~300 ms
/// the design calls for.
pub const SWEEP_FRAMES: u16 = 9;

/// The animation tick, in milliseconds. Only used while an eyecatch is running — the idle loop
/// ticks far slower, because nothing else here animates.
pub const FRAME_MS: u64 = 16;

/// Frames the band will hold before revealing whether or not anything released it.
///
/// About eight seconds. The wipe exists to *cover* a slow operation, and it must never become
/// the app's terminal state — reported from real use, with mpv spawning but never reporting a
/// position, the band held over the whole stage indefinitely and the only way out was to kill
/// the process. Nothing that resolves in a reasonable time reaches this, so the deadline costs
/// the happy path nothing; when it does fire, it means something is genuinely wrong and the
/// screen underneath is more useful than an amber rectangle.
pub const MAX_HOLD_FRAMES: u16 = 500;

/// Frames the band holds silently before it admits the wait is a wait.
///
/// [`FRAME_MS`] × this ≈ 2 s *after* the cover completes. Anything answering inside that window
/// shows nothing extra at all, which is the point: a signal that flashes up on every fast play
/// reads as the app being slow rather than as reassurance. Past it, silence is the wrong answer
/// — a torrent that has to find peers can take many seconds, and a static band gives no way to
/// tell "working" from "wedged".
pub const QUIET_HOLD_FRAMES: u16 = 125;

/// Where the eyecatch is in its sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The band is growing from the left edge.
    Covering,
    /// Fully covered, waiting on resolution.
    Held,
    /// The band's leading edge is running off the right, revealing what is behind it.
    Revealing,
    /// Nothing left to draw.
    Done,
}

/// A running eyecatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eyecatch {
    /// What is being resolved, shown inside the band.
    pub label: String,
    frame: u16,
    /// The frame resolution completed on, which is when the reveal may begin.
    released_at: Option<u16>,
}

impl Eyecatch {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), frame: 0, released_at: None }
    }

    /// Resolution finished — begin revealing.
    ///
    /// Never before the cover completes: a provider that answers in 20 ms would otherwise make
    /// the wipe a flicker, which reads as a glitch rather than a transition.
    pub fn release(&mut self) {
        if self.released_at.is_none() {
            self.released_at = Some(self.frame.max(SWEEP_FRAMES));
        }
    }

    /// Advance one frame. Returns `false` once the eyecatch has nothing left to draw.
    pub fn advance(&mut self) -> bool {
        self.frame = self.frame.saturating_add(1);
        self.stage() != Stage::Done
    }

    /// Whether the band gave up waiting rather than being released.
    ///
    /// Distinguished from a normal reveal so the caller can say what happened: a wipe that simply
    /// ends looks like success, and this is not success.
    pub fn timed_out(&self) -> bool {
        self.released_at.is_none() && self.frame >= MAX_HOLD_FRAMES
    }

    pub fn stage(&self) -> Stage {
        match self.released_at {
            None if self.frame < SWEEP_FRAMES => Stage::Covering,
            // The deadline. Reveal rather than hold forever.
            None if self.frame >= MAX_HOLD_FRAMES + SWEEP_FRAMES => Stage::Done,
            None if self.frame >= MAX_HOLD_FRAMES => Stage::Revealing,
            None => Stage::Held,
            Some(released) if self.frame < released => Stage::Covering,
            Some(released) if self.frame < released + SWEEP_FRAMES => Stage::Revealing,
            Some(_) => Stage::Done,
        }
    }

    /// The band's horizontal extent as fractions of the stage width, left to right.
    ///
    /// Covering grows the right edge; revealing advances the left edge. Together that reads as
    /// one band travelling across rather than two separate animations.
    pub fn band(&self) -> (f64, f64) {
        let progress = |numerator: u16| f64::from(numerator) / f64::from(SWEEP_FRAMES);
        match self.stage() {
            Stage::Covering => (0.0, progress(self.frame.min(SWEEP_FRAMES))),
            Stage::Held => (0.0, 1.0),
            Stage::Revealing => {
                let released = self.released_at.unwrap_or(MAX_HOLD_FRAMES);
                (progress(self.frame.saturating_sub(released)), 1.0)
            }
            Stage::Done => (1.0, 1.0),
        }
    }

    /// How long the band has been fully covering, in frames.
    fn held_frames(&self) -> u16 {
        if self.stage() != Stage::Held {
            return 0;
        }
        self.frame.saturating_sub(SWEEP_FRAMES)
    }

    /// Whole seconds spent holding, once the wait is long enough to be worth admitting.
    ///
    /// A count rather than an animation, deliberately. The app's one moving indicator is the
    /// three-cell pulse, and that belongs to the skeleton block: it is built from shade glyphs
    /// that read by *density* against the dark ground, so on a solid amber band its bright cell
    /// is the band itself. It also means "content is arriving", which is not what is happening
    /// here. Elapsed time is legible on the fill, says "still trying" without implying progress
    /// nothing can measure, and tells you *how* stuck it is — which a wave cannot.
    ///
    /// Returning a number rather than a string keeps this module free of the theme, so it stays
    /// pure state that can be tested without a terminal.
    pub fn waited_secs(&self) -> Option<u64> {
        let held = self.held_frames();
        (held >= QUIET_HOLD_FRAMES).then(|| u64::from(held) * FRAME_MS / 1000)
    }

    /// Whether the band covers enough of the stage to carry its label.
    ///
    /// Text appearing in a two-cell sliver would be truncated garbage, so it waits.
    pub fn shows_label(&self) -> bool {
        matches!(self.stage(), Stage::Held) || self.band().1 - self.band().0 > 0.6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wipe_covers_then_holds_until_released() {
        let mut e = Eyecatch::new("Frieren ep 12");
        assert_eq!(e.stage(), Stage::Covering);
        for _ in 0..SWEEP_FRAMES {
            assert!(e.advance());
        }
        // Resolution is still in flight, so the band must stay — this is the whole point.
        assert_eq!(e.stage(), Stage::Held);
        assert_eq!(e.band(), (0.0, 1.0));
        for _ in 0..100 {
            assert!(e.advance(), "a held eyecatch must never expire on its own");
        }
    }

    #[test]
    fn releasing_early_still_plays_the_full_cover() {
        // A provider answering in one frame would otherwise turn the transition into a
        // flicker, which looks like a rendering bug rather than a wipe.
        let mut e = Eyecatch::new("x");
        e.advance();
        e.release();
        assert_eq!(e.stage(), Stage::Covering);

        let mut frames = 1;
        while e.stage() == Stage::Covering {
            e.advance();
            frames += 1;
        }
        assert_eq!(frames, SWEEP_FRAMES, "the cover must run to completion");
        assert_eq!(e.stage(), Stage::Revealing);
    }

    #[test]
    fn a_release_finishes_and_reports_done() {
        let mut e = Eyecatch::new("x");
        for _ in 0..SWEEP_FRAMES {
            e.advance();
        }
        e.release();
        let mut running = true;
        for _ in 0..SWEEP_FRAMES {
            running = e.advance();
        }
        assert!(!running, "the caller needs a signal to drop the eyecatch");
        assert_eq!(e.stage(), Stage::Done);
    }

    #[test]
    fn a_quick_resolve_never_admits_to_waiting() {
        // The whole point: a signal that flashes up on every fast play would make the app feel
        // slow rather than reassure anyone.
        let mut e = Eyecatch::new("x");
        for _ in 0..SWEEP_FRAMES {
            e.advance();
            assert_eq!(e.waited_secs(), None, "covering is not a wait");
        }
        for _ in 0..QUIET_HOLD_FRAMES - 1 {
            assert_eq!(e.waited_secs(), None, "still inside the quiet window");
            e.advance();
        }
        assert_eq!(e.waited_secs(), None);
    }

    #[test]
    fn a_slow_resolve_reports_seconds_that_keep_climbing() {
        let mut e = Eyecatch::new("Frieren ep 12");
        for _ in 0..SWEEP_FRAMES + QUIET_HOLD_FRAMES {
            e.advance();
        }
        let first = e.waited_secs().expect("a long hold has to say something");
        assert!(first >= 1, "the count reflects real elapsed time, not frames");

        // A frozen number would be no better than silence.
        let one_more_second = 1000 / FRAME_MS as u16 + 1;
        for _ in 0..one_more_second {
            e.advance();
        }
        assert!(e.waited_secs().unwrap() > first, "the count has to move");
    }

    #[test]
    fn the_count_stops_the_moment_the_band_is_released() {
        let mut e = Eyecatch::new("x");
        for _ in 0..SWEEP_FRAMES + QUIET_HOLD_FRAMES {
            e.advance();
        }
        assert!(e.waited_secs().is_some());

        e.release();
        e.advance();
        assert_eq!(e.stage(), Stage::Revealing);
        assert_eq!(e.waited_secs(), None, "revealing is no longer waiting");
    }

    #[test]
    fn releasing_twice_does_not_restart_the_reveal() {
        // The release comes from whichever update lands first — playback starting, a status
        // line, or a failure toast — so it has to be idempotent.
        let mut e = Eyecatch::new("x");
        for _ in 0..SWEEP_FRAMES + 4 {
            e.advance();
        }
        e.release();
        let after_first = e.band();
        e.release();
        assert_eq!(e.band(), after_first);
    }

    #[test]
    fn the_band_only_ever_moves_rightward() {
        // A band that jumped backwards between stages would read as two animations.
        let mut e = Eyecatch::new("x");
        let mut last = (0.0, 0.0);
        e.release();
        loop {
            let band = e.band();
            assert!(band.0 >= last.0 - f64::EPSILON, "left edge went backwards: {band:?}");
            assert!(band.1 >= last.1 - f64::EPSILON, "right edge went backwards: {band:?}");
            assert!(band.0 <= band.1 + f64::EPSILON, "band inverted: {band:?}");
            last = band;
            if !e.advance() {
                break;
            }
        }
    }

    #[test]
    fn an_unreleased_wipe_gives_up_instead_of_holding_forever() {
        // Reported from real use: mpv spawned, never reported a position, and the band covered
        // the entire stage until the process was killed. The wipe covers a slow operation; it
        // must never *be* the outcome.
        let mut e = Eyecatch::new("Frieren");
        for _ in 0..MAX_HOLD_FRAMES {
            assert!(e.advance(), "should still be running before the deadline");
        }
        assert!(e.timed_out(), "the deadline should have fired");
        assert_eq!(e.stage(), Stage::Revealing);

        let mut frames = 0;
        while e.advance() {
            frames += 1;
            assert!(frames < SWEEP_FRAMES * 4, "the reveal must terminate");
        }
        assert_eq!(e.stage(), Stage::Done, "the stage must end up uncovered");
    }

    #[test]
    fn a_normal_release_is_not_reported_as_a_timeout() {
        // The two paths look identical on screen, so the caller needs to tell them apart — one
        // deserves an explanation and the other must stay silent.
        let mut e = Eyecatch::new("Frieren");
        for _ in 0..3 {
            e.advance();
        }
        e.release();
        while e.advance() {}
        assert!(!e.timed_out(), "a released wipe is a success, not a failure");
    }

    #[test]
    fn the_label_waits_for_room() {
        let mut e = Eyecatch::new("Frieren ep 12");
        assert!(!e.shows_label(), "a two-cell sliver cannot carry text");
        for _ in 0..SWEEP_FRAMES {
            e.advance();
        }
        assert!(e.shows_label());
    }
}
