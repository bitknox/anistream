//! The preview a cover opens when the pointer rests on it: the banner, the title, a facts line,
//! two lines of synopsis and the title's main action, in a popover. Where the title has a
//! trailer and trailers may play, the trailer plays muted under the banner as it does on the
//! title page.
//!
//! The pointer must rest: less than 4 px of movement for 800 ms. The preview closes 150 ms after
//! the pointer leaves both the cover and the preview, and at once on a scroll, a key press, a
//! click elsewhere or the window losing focus. Keyboard focus never opens one.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};

use adw::prelude::*;
use anistream_i18n::t;
use anistream_model::{catalog::Entry, command::Command};
use gtk::glib;

use super::{Stage, may_autoplay};
use crate::{fit::Fit, hub::Hub, say, widgets};

const DWELL: Duration = Duration::from_millis(800);
const LEAVE: Duration = Duration::from_millis(150);
/// The pointer may drift this far and still be resting.
const REST: f64 = 4.0;
/// The video's width: 384 by 216, over YouTube's 356 by 200 floor.
const WIDTH: i32 = 384;

thread_local! {
    /// The preview on screen, if any: there is one at a time.
    static OPEN: RefCell<Option<Rc<Preview>>> = const { RefCell::new(None) };
    /// Whether a title page has its own trailer playing, which gets no previews over it.
    static PAGE_PLAYING: Cell<bool> = const { Cell::new(false) };
}

/// A title page's trailer is on screen or is not: no preview opens while it is.
pub fn page_playing(playing: bool) {
    PAGE_PLAYING.with(|c| c.set(playing));
    if playing {
        close();
    }
}

/// The preview on screen and where it stands in the window, for a shot, which draws the window
/// without its popovers and puts this on top.
pub fn shot_popover(window: &gtk::Window) -> Option<(gtk::Widget, f32, f32)> {
    OPEN.with(|open| {
        let open = open.borrow();
        let preview = open.as_ref()?;
        let target = preview.popover.parent()?;
        let bounds = target.compute_bounds(window)?;
        Some((preview.popover.clone().upcast(), bounds.x() + bounds.width() + 6.0, bounds.y()))
    })
}

/// Close the preview on screen, if there is one.
pub fn close() {
    if let Some(preview) = OPEN.with(|open| open.borrow_mut().take()) {
        preview.dismiss();
    }
}

/// Let `target` open a preview of title `id` when the pointer rests on it.
pub fn attach(hub: &Rc<Hub>, target: &impl IsA<gtk::Widget>, id: u32) {
    let target = target.as_ref();
    let rest = Rc::new(Rest { hub: Rc::downgrade(hub), id, ..Rest::default() });
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let rest = Rc::clone(&rest);
        let target = target.downgrade();
        move |_, x, y| {
            rest.anchor.set((x, y));
            if let Some(target) = target.upgrade() {
                rest.rest(&target);
            }
        }
    });
    motion.connect_motion({
        let rest = Rc::clone(&rest);
        let target = target.downgrade();
        move |_, x, y| {
            let (ax, ay) = rest.anchor.get();
            if (x - ax).abs() >= REST || (y - ay).abs() >= REST {
                rest.anchor.set((x, y));
                if let Some(target) = target.upgrade() {
                    rest.rest(&target);
                }
            }
        }
    });
    motion.connect_leave({
        let rest = Rc::clone(&rest);
        move |_| {
            rest.cancel();
            OPEN.with(|open| {
                if let Some(preview) = open.borrow().as_ref()
                    && preview.id == rest.id
                {
                    preview.leave();
                }
            });
        }
    });
    target.add_controller(motion);
}

/// What one cover keeps while the pointer is on it.
#[derive(Default)]
struct Rest {
    hub: Weak<Hub>,
    id: u32,
    anchor: Cell<(f64, f64)>,
    dwell: RefCell<Option<glib::SourceId>>,
}

impl Rest {
    fn cancel(&self) {
        if let Some(source) = self.dwell.take() {
            source.remove();
        }
    }

    /// The pointer is at a new resting place: wait out the dwell again.
    fn rest(self: &Rc<Self>, target: &gtk::Widget) {
        self.cancel();
        let Some(hub) = self.hub.upgrade() else { return };
        // A title the lists have not shown is fetched while the pointer rests.
        if hub.entry(anistream_core::ids::AnilistId::new(self.id)).is_none() {
            hub.send(Command::LoadDetail(anistream_core::ids::AnilistId::new(self.id)));
        }
        let this = Rc::clone(self);
        let target = target.downgrade();
        self.dwell.replace(Some(glib::timeout_add_local_once(DWELL, move || {
            this.dwell.take();
            let (Some(target), Some(hub)) = (target.upgrade(), this.hub.upgrade()) else {
                return;
            };
            open(&hub, &target, this.id);
        })));
    }
}

/// One open preview.
struct Preview {
    id: u32,
    popover: gtk::Popover,
    stage: Rc<Stage>,
    leaving: RefCell<Option<glib::SourceId>>,
    window: gtk::Window,
    guards: RefCell<Vec<gtk::EventController>>,
    active: RefCell<Option<glib::SignalHandlerId>>,
}

impl Preview {
    /// The pointer left the cover or the preview: close unless it is back within 150 ms.
    fn leave(self: &Rc<Self>) {
        if let Some(source) = self.leaving.take() {
            source.remove();
        }
        let this = Rc::downgrade(self);
        self.leaving.replace(Some(glib::timeout_add_local_once(LEAVE, move || {
            if let Some(this) = this.upgrade() {
                this.leaving.take();
                close();
            }
        })));
    }

    fn stay(&self) {
        if let Some(source) = self.leaving.take() {
            source.remove();
        }
    }

    /// Take it down and let go of its web view.
    fn dismiss(&self) {
        self.stay();
        self.stage.stop();
        for controller in self.guards.take() {
            self.window.remove_controller(&controller);
        }
        if let Some(handler) = self.active.take() {
            self.window.disconnect(handler);
        }
        self.popover.popdown();
        self.popover.unparent();
    }
}

/// Open the preview of title `id` on `target` now, without the pointer: for the shots.
pub fn show(hub: &Rc<Hub>, target: &gtk::Widget, id: u32) {
    open(hub, target, id);
}

fn open(hub: &Rc<Hub>, target: &gtk::Widget, id: u32) {
    if PAGE_PLAYING.with(Cell::get) || !target.is_mapped() {
        return;
    }
    let Some(entry) = hub.entry(anistream_core::ids::AnilistId::new(id)) else { return };
    let Some(window) = target.root().and_downcast::<gtk::Window>() else { return };
    close();

    let (content, stage) = card(hub, &entry);
    let popover = gtk::Popover::new();
    popover.set_autohide(false);
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Right);
    popover.set_child(Some(&content));
    popover.add_css_class("trailer-preview");
    popover.set_parent(target);
    popover.update_property(&[gtk::accessible::Property::Label(&entry.title)]);

    let preview = Rc::new(Preview {
        id,
        popover: popover.clone(),
        stage: Rc::clone(&stage),
        leaving: RefCell::default(),
        window: window.clone(),
        guards: RefCell::default(),
        active: RefCell::default(),
    });

    // The pointer on the preview keeps it.
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let preview = Rc::downgrade(&preview);
        move |_, _, _| {
            if let Some(preview) = preview.upgrade() {
                preview.stay();
            }
        }
    });
    motion.connect_leave({
        let preview = Rc::downgrade(&preview);
        move |_| {
            if let Some(preview) = preview.upgrade() {
                preview.leave();
            }
        }
    });
    popover.add_controller(motion);

    // A key, a scroll, a click elsewhere or the window losing focus closes it at once.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(|_, _, _, _| {
        close();
        glib::Propagation::Proceed
    });
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    scroll.connect_scroll(|_, _, _| {
        close();
        glib::Propagation::Proceed
    });
    let click = gtk::GestureClick::new();
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    click.connect_pressed(|gesture, _, x, y| {
        // A press on the preview itself (its action) is not a click elsewhere.
        let inside = gesture
            .widget()
            .and_then(|window| window.pick(x, y, gtk::PickFlags::DEFAULT))
            .is_some_and(|hit| hit.ancestor(gtk::Popover::static_type()).is_some());
        if !inside {
            close();
        }
    });
    for controller in [keys.upcast::<gtk::EventController>(), scroll.upcast(), click.upcast()] {
        window.add_controller(controller.clone());
        preview.guards.borrow_mut().push(controller);
    }
    let handler = window.connect_is_active_notify(|_| close());
    preview.active.replace(Some(handler));

    OPEN.with(|open| open.borrow_mut().replace(Rc::clone(&preview)));
    popover.popup();

    if let Some(trailer) = entry.trailer.clone()
        && may_autoplay(hub, &entry)
    {
        stage.start(&trailer, 0);
    }
}

/// The card: banner (with the trailer under it), title, facts, synopsis and the main action.
fn card(hub: &Rc<Hub>, entry: &Entry) -> (gtk::Box, Rc<Stage>) {
    let url = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
    let (frame, _) = widgets::art(hub, url, entry.cover_color.as_deref(), "banner");
    let banner = Fit::new(&frame, 16.0 / 9.0, WIDTH, true, 0);
    let stage = Stage::new(&banner);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_size_request(WIDTH, -1);
    column.append(&stage.widget);

    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let title = widgets::show_title(&entry.title, "row");
    title.set_hexpand(true);
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    heading.append(&title);
    let sound = sound_button();
    heading.append(&sound);
    column.append(&heading);
    {
        let sound = sound.clone();
        let hear = Rc::downgrade(&stage);
        sound.connect_toggled(move |button| {
            if let Some(stage) = hear.upgrade() {
                stage.set_sound(button.is_active());
            }
            button.set_icon_name(sound_icon(button.is_active()));
        });
        let sound = sound.clone();
        stage.on_showing(move |showing| sound.set_visible(showing));
    }

    column.append(&widgets::haze(&say::meta_line(entry), "numeric"));
    let synopsis = spoiler_free(entry);
    if !synopsis.is_empty() {
        let line = gtk::Label::new(Some(&synopsis));
        line.set_wrap(true);
        line.set_lines(2);
        line.set_ellipsize(gtk::pango::EllipsizeMode::End);
        line.set_xalign(0.0);
        line.set_max_width_chars(48);
        column.append(&line);
    }

    let item = say::queue_item(entry, None);
    let episode = item.episode;
    if let Some(label) = item.button {
        let text = gtk::Label::new(Some(&label));
        text.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let button = gtk::Button::builder().child(&text).halign(gtk::Align::Start).build();
        button.add_css_class("pill");
        button.add_css_class("suggested-action");
        button.set_action_name(Some("win.play"));
        button
            .set_action_target_value(Some(&(entry.id.get(), episode.to_string()).to_variant()));
        column.append(&button);
    }
    column.set_margin_top(10);
    column.set_margin_bottom(12);
    column.set_margin_start(10);
    column.set_margin_end(10);
    (column, stage)
}

/// The synopsis with the parts a spoiler tag marks left out.
fn spoiler_free(entry: &Entry) -> String {
    if entry.synopsis_spans.is_empty() {
        return entry.synopsis.clone();
    }
    entry
        .synopsis_spans
        .iter()
        .filter(|span| !span.spoiler)
        .map(|span| span.text.as_str())
        .collect::<String>()
        .trim()
        .to_owned()
}

/// The sound button, below the video: off at every start, and only while a trailer shows.
pub fn sound_button() -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_icon_name(sound_icon(false));
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    button.set_tooltip_text(Some(&t::title_trailer_sound()));
    button.update_property(&[gtk::accessible::Property::Label(&t::title_trailer_sound())]);
    button.set_visible(false);
    button
}

pub fn sound_icon(on: bool) -> &'static str {
    if on { "audio-volume-high-symbolic" } else { "audio-volume-muted-symbolic" } // i18n-skip
}

#[cfg(test)]
mod tests {
    use anistream_model::catalog::SynopsisSpan;

    use super::*;

    fn span(text: &str, spoiler: bool) -> SynopsisSpan {
        SynopsisSpan { text: text.into(), spoiler }
    }

    #[test]
    fn the_preview_leaves_out_what_a_spoiler_tag_marks() {
        let mut entry = Entry::new(anistream_core::ids::AnilistId::new(1), "A");
        entry.synopsis = "plain whole text".into();
        assert_eq!(spoiler_free(&entry), "plain whole text", "no spans, the text as it is");
        entry.synopsis_spans = vec![span("A lamp is lit. ", false), span("It goes out.", true)];
        assert_eq!(spoiler_free(&entry), "A lamp is lit.");
    }
}
