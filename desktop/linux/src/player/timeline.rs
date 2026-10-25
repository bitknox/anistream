//! The timeline's extras: the opening and ending marked as segments on the track, read from
//! the chapters mpv has (the stream's own, or the ones the engine writes), and the seek preview,
//! a second mpv core on the same stream that shows the frame under the pointer.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_mpv_host::Mpv;
use gtk::gdk;
use serde_json::{Value, json};

use crate::{fit::Fit, video, words};

/// A stretch of the timeline the chapters name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Opening,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub kind: Kind,
    pub start: f64,
    pub end: f64,
}

/// What a chapter's name says it is, as the Mac reads it: "Opening", "OP", "NCOP1", "Intro".
fn kind(title: &str) -> Option<Kind> {
    let name = title.trim().to_lowercase();
    if ["op", "opening", "intro"].contains(&name.as_str()) || name.starts_with("ncop") {
        Some(Kind::Opening)
    } else if ["ed", "ending", "outro"].contains(&name.as_str()) || name.starts_with("nced") {
        Some(Kind::Ending)
    } else {
        None
    }
}

/// mpv's `chapter-list`, as names and times.
pub fn chapters(list: &Value) -> Vec<(String, f64)> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|c| {
                    let time = c.get("time")?.as_f64()?;
                    let title = c.get("title").and_then(Value::as_str).unwrap_or_default();
                    Some((title.to_owned(), time))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The engine's openings and endings as segments.
pub fn from_marks(marks: &[anistream_model::playback::TimelineMark]) -> Vec<Segment> {
    use anistream_model::playback::SkipKind;
    marks
        .iter()
        .map(|m| Segment {
            kind: match m.kind {
                SkipKind::Opening => Kind::Opening,
                SkipKind::Ending => Kind::Ending,
            },
            start: m.start,
            end: m.end,
        })
        .collect()
}

/// The openings and endings, each running to the next chapter or the end.
pub fn segments(chapters: &[(String, f64)], duration: f64) -> Vec<Segment> {
    if duration <= 0.0 {
        return Vec::new();
    }
    let mut sorted: Vec<_> = chapters.iter().filter(|(_, t)| *t < duration).collect();
    sorted.sort_by(|a, b| a.1.total_cmp(&b.1));
    sorted
        .iter()
        .enumerate()
        .filter_map(|(at, (title, start))| {
            let kind = kind(title)?;
            let end = sorted.get(at + 1).map_or(duration, |next| next.1);
            Some(Segment { kind, start: *start, end })
        })
        .collect()
}

/// The scale's trough, where the track is drawn: its bounds in `within`'s coordinates.
fn trough(scale: &gtk::Scale, within: &impl IsA<gtk::Widget>) -> Option<gtk::graphene::Rect> {
    let mut child = scale.first_child();
    while let Some(widget) = child {
        if widget.css_name() == "trough" {
            return widget.compute_bounds(within);
        }
        child = widget.next_sibling();
    }
    None
}

/// The timeline with its segments drawn over the track.
pub struct Marked {
    pub widget: gtk::Overlay,
    marks: gtk::DrawingArea,
    segments: Rc<RefCell<(Vec<Segment>, f64)>>,
    /// What the engine says is buffered, in seconds of the file.
    buffered: Rc<RefCell<Vec<(f64, f64)>>>,
}

impl Marked {
    pub fn new(scale: &gtk::Scale) -> Self {
        let marks = gtk::DrawingArea::new();
        marks.set_can_target(false);
        let segments: Rc<RefCell<(Vec<Segment>, f64)>> = Rc::default();
        let widget = gtk::Overlay::new();
        widget.set_hexpand(true);
        widget.set_child(Some(scale));
        widget.add_overlay(&marks);
        let buffered: Rc<RefCell<Vec<(f64, f64)>>> = Rc::default();
        marks.set_draw_func({
            let segments = Rc::clone(&segments);
            let buffered = Rc::clone(&buffered);
            let scale = scale.downgrade();
            move |area, cr, _, _| {
                let Some(scale) = scale.upgrade() else { return };
                let (segments, duration) = &*segments.borrow();
                let adjustment = scale.adjustment();
                let length = adjustment.upper() - adjustment.lower();
                let Some(track) = trough(&scale, area) else { return };
                let (x, y, w, h) = (
                    f64::from(track.x()),
                    f64::from(track.y()),
                    f64::from(track.width()),
                    f64::from(track.height()),
                );
                // Buffered stretches in silk at 45%, ahead of the played part only, which keeps
                // its amber.
                if length > 0.0 {
                    let played = scale.value();
                    cr.set_source_rgba(228.0 / 255.0, 224.0 / 255.0, 214.0 / 255.0, 0.45);
                    for (start, end) in buffered.borrow().iter() {
                        let (start, end) = (start.max(played), end.min(adjustment.upper()));
                        if end <= start {
                            continue;
                        }
                        let from = x + w * ((start - adjustment.lower()) / length).clamp(0.0, 1.0);
                        let to = x + w * ((end - adjustment.lower()) / length).clamp(0.0, 1.0);
                        cr.rectangle(from, y, (to - from).max(1.0), h);
                        let _ = cr.fill();
                    }
                }
                if segments.is_empty() || *duration <= 0.0 {
                    return;
                }
                // Water, the brief's colour for what the app knows of an episode's shape.
                cr.set_source_rgba(127.0 / 255.0, 199.0 / 255.0, 217.0 / 255.0, 0.7);
                for segment in segments {
                    let from = x + w * (segment.start / duration).clamp(0.0, 1.0);
                    let to = x + w * (segment.end / duration).clamp(0.0, 1.0);
                    // A pixel's gap at each end, so two segments in a row read as two.
                    let (from, to) = (from + 1.0, (to - 1.0).max(from + 2.0));
                    cr.rectangle(from, y, to - from, h);
                    let _ = cr.fill();
                }
            }
        });
        Self { widget, marks, segments, buffered }
    }

    /// The played part moved: the buffered ones start where it ends.
    pub fn moved(&self) {
        if !self.buffered.borrow().is_empty() {
            self.marks.queue_draw();
        }
    }

    /// Draw `ranges` as what the player holds, whole, replacing what was drawn.
    pub fn set_buffered(&self, ranges: Vec<(f64, f64)>) {
        if *self.buffered.borrow() != ranges {
            self.buffered.replace(ranges);
            self.marks.queue_draw();
        }
    }

    /// Mark `segments` on a timeline `duration` long.
    pub fn set(&self, segments: Vec<Segment>, duration: f64) {
        let changed = {
            let now = self.segments.borrow();
            now.0 != segments || (now.1 - duration).abs() > 0.5
        };
        if changed {
            self.segments.replace((segments, duration));
            self.marks.queue_draw();
        }
    }

    /// Which segment `seconds` falls in, for the preview's line.
    pub fn at(&self, seconds: f64) -> Option<Kind> {
        let segments = self.segments.borrow();
        segments.0.iter().find(|s| seconds >= s.start && seconds < s.end).map(|s| s.kind)
    }
}

/// A stream's address and the per-file options it was opened with.
type Stream = (String, Vec<(String, String)>);

/// The seek preview: a popover over the timeline with the frame under the pointer and its time.
pub struct Preview {
    popover: gtk::Popover,
    time: gtk::Label,
    mpv: Option<Mpv>,
    /// The stream it has open, and the options it was opened with.
    loaded: RefCell<Option<String>>,
    source: RefCell<Option<Stream>>,
    /// Seeks are sent at most this often while the pointer moves.
    last_seek: Cell<Option<Instant>>,
}

impl Preview {
    pub fn new(scale: &gtk::Scale, hwdec: bool) -> Rc<Self> {
        let options = [
            ("vo", "libmpv"),
            ("pause", "yes"),
            ("aid", "no"),
            ("sid", "no"),
            ("ao", "null"),
            ("audio", "no"),
            ("hr-seek", "no"),
            ("keep-open", "yes"),
            ("idle", "yes"),
            ("osc", "no"),
            ("ytdl", "no"),
            ("input-default-bindings", "no"),
            ("hwdec", if hwdec { "auto-safe" } else { "no" }),
            ("vd-lavc-fast", "yes"),
            ("vd-lavc-skiploopfilter", "all"),
            ("demuxer-readahead-secs", "2"),
            ("cache-pause", "no"),
        ];
        let mpv = match Mpv::new(options, "warn") {
            Ok(mpv) => Some(mpv),
            Err(e) => {
                tracing::warn!(error = %e, "no second mpv for the seek preview");
                None
            }
        };
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 6);
        frame.add_css_class("seek-preview");
        if let Some(mpv) = &mpv {
            let area = video::video_area(mpv.clone(), || {});
            frame.append(&Fit::new(&area.widget, 16.0 / 9.0, 160, true, 0));
        }
        let time = gtk::Label::new(Some("0:00"));
        time.add_css_class("numeric");
        time.add_css_class("caption");
        frame.append(&time);
        let popover = gtk::Popover::builder()
            .child(&frame)
            .position(gtk::PositionType::Top)
            .autohide(false)
            .has_arrow(false)
            .can_target(false)
            .build();
        popover.add_css_class("osd");
        popover.set_parent(scale);
        Rc::new(Self {
            popover,
            time,
            mpv,
            loaded: RefCell::default(),
            source: RefCell::default(),
            last_seek: Cell::new(None),
        })
    }

    /// The stream playing, and the options it was opened with: what the preview opens.
    pub fn stream(&self, url: &str, options: Vec<(String, String)>) {
        self.source.replace(Some((url.to_owned(), options)));
    }

    /// Frames from `url` instead, for a torrent: what has downloaded so far.
    pub fn thumbnails(&self, url: Option<&str>) {
        if let Some(url) = url {
            let options =
                self.source.borrow().as_ref().map(|s| s.1.clone()).unwrap_or_default();
            self.source.replace(Some((url.to_owned(), options)));
        }
    }

    pub fn forget(&self) {
        self.popover.popdown();
        self.source.replace(None);
        if self.loaded.take().is_some()
            && let Some(mpv) = &self.mpv
        {
            let _ = mpv.command(&["stop"]);
        }
    }

    /// The frame and time at `seconds`, over the timeline at `x`, labelled `segment` if it is
    /// in one.
    pub fn show(&self, x: f64, seconds: f64, segment: Option<Kind>) {
        let clock = words::clock(seconds);
        self.time.set_label(&match segment {
            Some(Kind::Opening) => format!("{clock} · Opening"),
            Some(Kind::Ending) => format!("{clock} · Ending"),
            None => clock,
        });
        self.popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, 0, 1, 1)));
        if !self.popover.is_visible() {
            self.popover.popup();
        }
        let Some(mpv) = &self.mpv else { return };
        let Some((url, options)) = self.source.borrow().clone() else { return };
        if self.loaded.borrow().as_deref() != Some(url.as_str()) {
            let mut map = serde_json::Map::new();
            for (name, value) in options {
                // The resume point is the main player's; the preview seeks for itself.
                if name != "start" {
                    map.insert(name, Value::String(value));
                }
            }
            map.insert("start".into(), Value::String(format!("{seconds:.1}")));
            let load =
                json!({ "name": "loadfile", "url": url, "flags": "replace", "options": map });
            if let Err(e) = mpv.command_node(&load) {
                tracing::debug!(error = %e, "the seek preview did not open the stream");
            }
            self.loaded.replace(Some(url));
            return;
        }
        let now = Instant::now();
        if self.last_seek.get().is_some_and(|at| now - at < Duration::from_millis(80)) {
            return;
        }
        self.last_seek.set(Some(now));
        let _ = mpv.command(&["seek", &format!("{seconds:.2}"), "absolute+keyframes"]);
    }

    /// The popover, while it shows, and where its top left is over `window`: centred on the
    /// point it points at, just above the timeline.
    pub fn placed(&self, window: &impl IsA<gtk::Widget>) -> Option<(gtk::Widget, f32, f32)> {
        if !self.popover.is_visible() {
            return None;
        }
        let scale = self.popover.parent()?;
        let at = self.popover.pointing_to().1;
        let origin =
            scale.compute_point(window, &gtk::graphene::Point::new(at.x() as f32, 0.0))?;
        let (width, height) = (self.popover.width() as f32, self.popover.height() as f32);
        Some((self.popover.clone().upcast(), origin.x() - width / 2.0, origin.y() - height))
    }

    pub fn hide(&self) {
        self.popover.popdown();
    }
}

/// Where on the timeline `x` is, as a time: by the trough, as the scale itself reads it.
pub fn time_at(scale: &gtk::Scale, x: f64) -> Option<f64> {
    let track = trough(scale, scale)?;
    let adjustment = scale.adjustment();
    let fraction =
        ((x - f64::from(track.x())) / f64::from(track.width()).max(1.0)).clamp(0.0, 1.0);
    Some(adjustment.lower() + fraction * (adjustment.upper() - adjustment.lower()))
}

/// Where on the timeline `fraction` of the way along is, in the scale's coordinates.
#[cfg(feature = "fixtures")]
pub fn x_at(scale: &gtk::Scale, fraction: f64) -> Option<f64> {
    let track = trough(scale, scale)?;
    Some(f64::from(track.x()) + f64::from(track.width()) * fraction.clamp(0.0, 1.0))
}

/// Raise the subtitles by `bar` of `height`, or put them back with zero: mpv's `sub-pos`, at
/// once, with no animation.
pub fn lift_subtitles(mpv: &Mpv, bar: i32, height: i32) {
    let lift = if height > 0 { f64::from(bar.max(0)) / f64::from(height) } else { 0.0 };
    let position = (100.0 - lift.clamp(0.0, 0.5) * 100.0).round();
    let _ = mpv.set("sub-pos", &json!(position as i64));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engines_marks_become_segments() {
        use anistream_model::playback::{SkipKind, TimelineMark};
        let marks = [
            TimelineMark { kind: SkipKind::Opening, start: 5.0, end: 90.0 },
            TimelineMark { kind: SkipKind::Ending, start: 1300.0, end: 1390.0 },
        ];
        assert_eq!(
            from_marks(&marks),
            [
                Segment { kind: Kind::Opening, start: 5.0, end: 90.0 },
                Segment { kind: Kind::Ending, start: 1300.0, end: 1390.0 },
            ]
        );
        assert!(from_marks(&[]).is_empty());
    }

    #[test]
    fn openings_and_endings_run_to_the_next_chapter() {
        let list = json!([
            { "title": "Episode", "time": 0.0 },
            { "title": "Opening", "time": 90.0 },
            { "title": "Episode", "time": 180.0 },
            { "title": "ED", "time": 1300.0 },
        ]);
        let chapters = chapters(&list);
        assert_eq!(
            segments(&chapters, 1420.0),
            [
                Segment { kind: Kind::Opening, start: 90.0, end: 180.0 },
                Segment { kind: Kind::Ending, start: 1300.0, end: 1420.0 },
            ]
        );
        assert!(segments(&chapters, 0.0).is_empty());
        assert_eq!(kind("NCOP1"), Some(Kind::Opening));
        assert_eq!(kind("Part B"), None);
    }
}
