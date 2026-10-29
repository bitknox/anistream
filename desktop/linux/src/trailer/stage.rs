//! A banner that plays its title's trailer behind it, for the title page and the hover preview.
//!
//! The banner shows at once and stays on top. The web view sits underneath, transparent, and the
//! banner fades away only when the page says a frame is playing, so the wait for the video is
//! never seen. Nothing is ever drawn over the video: the banner is clear by then, and the stage
//! reserves the top `inset` for what the window puts over its edge.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use anistream_model::{
    catalog::Trailer,
    trailer::{AppIdentity, EmbedStyle, TrailerControl, TrailerMessage, embed_page},
};
use gtk::glib;

use super::webkit::{Signal, Web};
use crate::motion;

/// YouTube's smallest player: 200 by 200 px.
const MIN_SIDE: i32 = 200;
/// A trailer that has not played in this long is let go, with nothing said.
const GIVE_UP: Duration = Duration::from_secs(8);

/// What hears the banner clear and return.
type Showing = Rc<dyn Fn(bool)>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// No web view.
    Idle,
    /// A view is loading the page; the banner is still up.
    Loading,
    /// A frame is on screen and the banner is clear.
    Playing,
    /// The view is held, paused, with the banner back over it.
    Paused,
    /// It ended or was refused: not again on this visit.
    Done,
}

pub struct Stage {
    pub widget: gtk::Overlay,
    video: gtk::Box,
    banner: gtk::Widget,
    state: Cell<State>,
    web: RefCell<Option<Rc<Web>>>,
    timeout: RefCell<Option<glib::SourceId>>,
    changed: RefCell<Option<Showing>>,
    fade: RefCell<Option<adw::TimedAnimation>>,
}

impl Stage {
    /// A stage whose size is `banner`'s: the banner is the top layer, the video the one under.
    pub fn new(banner: &impl IsA<gtk::Widget>) -> Rc<Self> {
        let widget = gtk::Overlay::new();
        let video = gtk::Box::new(gtk::Orientation::Vertical, 0);
        video.set_can_target(false);
        video.set_overflow(gtk::Overflow::Hidden);
        widget.set_child(Some(&video));
        widget.add_overlay(banner);
        widget.set_measure_overlay(banner, true);
        widget.set_clip_overlay(banner, true);
        Rc::new(Self {
            widget,
            video,
            banner: banner.clone().upcast(),
            state: Cell::new(State::Idle),
            web: RefCell::default(),
            timeout: RefCell::default(),
            changed: RefCell::default(),
            fade: RefCell::default(),
        })
    }

    /// Run `f` with `true` when the banner clears for a playing trailer and `false` when it
    /// returns.
    pub fn on_showing(&self, f: impl Fn(bool) + 'static) {
        self.changed.replace(Some(Rc::new(f)));
    }

    /// Whether a trailer is on screen.
    pub fn showing(&self) -> bool {
        self.state.get() == State::Playing
    }

    /// Whether a view is held at all: loading, playing or paused.
    pub fn active(&self) -> bool {
        matches!(self.state.get(), State::Loading | State::Playing | State::Paused)
    }

    /// Whether this stage may start `trailer`: it has not ended or failed already, and its video
    /// area would be at least 200 by 200 once `inset` is kept clear at the top.
    pub fn fits(&self, inset: i32) -> bool {
        self.state.get() != State::Done
            && self.widget.width() >= MIN_SIDE
            && self.widget.height() - inset >= MIN_SIDE
    }

    /// Load `trailer` under the banner, keeping `inset` pixels at the top clear of the video.
    /// False when it cannot: no WebKitGTK, a trailer the player does not play, or one already
    /// held.
    pub fn start(self: &Rc<Self>, trailer: &Trailer, inset: i32) -> bool {
        if self.web.borrow().is_some() || self.state.get() == State::Done {
            return false;
        }
        let app = AppIdentity::new(AppIdentity::LINUX);
        let Some(page) = page(trailer, &app) else { return false };
        let weak = Rc::downgrade(self);
        let web = Web::new(&page, &app.base_url(), false, false, move |signal| {
            if let Some(stage) = weak.upgrade() {
                stage.heard(signal);
            }
        });
        let Some(web) = web else { return false };
        self.video.set_margin_top(inset);
        self.video.append(&web.widget);
        self.web.replace(Some(web));
        self.state.set(State::Loading);
        let weak = Rc::downgrade(self);
        self.timeout.replace(Some(glib::timeout_add_local_once(GIVE_UP, move || {
            if let Some(stage) = weak.upgrade() {
                // The source is the one running: it is gone once this returns.
                stage.timeout.take();
                if stage.state.get() == State::Loading {
                    stage.release(State::Idle);
                }
            }
        })));
        true
    }

    fn heard(self: &Rc<Self>, signal: Signal) {
        match signal {
            Signal::Message(TrailerMessage::Playing) => {
                if matches!(self.state.get(), State::Loading | State::Paused) {
                    self.cancel_timeout();
                    self.state.set(State::Playing);
                    self.fade_banner(false);
                    self.say(true);
                }
            }
            Signal::Message(TrailerMessage::Ended) => self.release(State::Done),
            Signal::Message(TrailerMessage::Error { .. }) | Signal::Failed => {
                self.release(State::Done);
            }
            Signal::Message(_) => {}
        }
    }

    /// Hold the trailer: it pauses and the banner comes back over it.
    pub fn pause(&self) {
        if self.state.get() != State::Playing {
            return;
        }
        self.state.set(State::Paused);
        self.control(TrailerControl::Pause);
        self.fade_banner(true);
        self.say(false);
    }

    /// Play it again from where it paused; the banner clears when the page says it is playing.
    pub fn resume(self: &Rc<Self>) {
        if self.state.get() != State::Paused {
            return;
        }
        self.control(TrailerControl::Play);
        self.timeout.replace(None);
    }

    /// Sound on or off, for this trailer only.
    pub fn set_sound(&self, on: bool) {
        self.control(if on { TrailerControl::Unmute } else { TrailerControl::Mute });
    }

    /// Let go of the view, and bring the banner back if it was clear.
    pub fn stop(&self) {
        if self.active() {
            self.release(State::Idle);
        }
    }

    fn control(&self, control: TrailerControl) {
        if let Some(web) = self.web.borrow().as_ref() {
            web.control(control);
        }
    }

    fn release(&self, then: State) {
        self.cancel_timeout();
        let was = self.state.replace(then);
        if let Some(web) = self.web.take() {
            self.video.remove(&web.widget);
        }
        if was == State::Playing || was == State::Paused {
            if self.banner.opacity() < 1.0 || was == State::Playing {
                self.fade_banner(true);
            }
            if was == State::Playing {
                self.say(false);
            }
        }
    }

    fn cancel_timeout(&self) {
        if let Some(source) = self.timeout.take() {
            source.remove();
        }
    }

    /// Fade the banner back in (`back`) or away: 300 ms in, 200 ms out, the motion tokens.
    fn fade_banner(&self, back: bool) {
        if let Some(old) = self.fade.take() {
            old.pause();
        }
        let (from, to, ms, easing) = if back {
            (self.banner.opacity(), 1.0, 300, adw::Easing::EaseInCubic)
        } else {
            (self.banner.opacity(), 0.0, 200, adw::Easing::EaseOutCubic)
        };
        let animation =
            adw::TimedAnimation::new(&self.banner, from, to, ms, motion::opacity(&self.banner));
        animation.set_easing(easing);
        animation.play();
        self.fade.replace(Some(animation));
    }

    fn say(&self, showing: bool) {
        let changed = self.changed.borrow().clone();
        if let Some(changed) = changed {
            changed(showing);
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(source) = self.timeout.take() {
            source.remove();
        }
    }
}

/// The page to load: the engine's, and in the test kit's scene a local stand-in that plays a
/// made-up clip with no network.
fn page(trailer: &Trailer, app: &AppIdentity) -> Option<String> {
    #[cfg(feature = "fixtures")]
    if crate::fixtures::active() {
        return Some(fixture_page());
    }
    embed_page(trailer, EmbedStyle::Banner, app)
}

/// A page that says what the real one would, for the shots and for trying the app offline: a
/// slow gradient in place of the video, `ready` at once and `playing` a moment later.
#[cfg(feature = "fixtures")]
const FIXTURE_PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}
#clip{width:100%;height:100%;background:linear-gradient(120deg,#1d3557,#e76f51,#f4a261,#264653);
background-size:300% 300%;animation:drift 6s linear infinite}
@keyframes drift{0%{background-position:0% 50%}100%{background-position:100% 50%}}
</style></head><body><div id="clip"></div><script>
function say(m){try{window.webkit.messageHandlers.anistream.postMessage(m)}catch(e){}}
window.trailer={play:function(){say("playing")},pause:function(){say("paused")},mute:function(){},unmute:function(){}};
say("ready");setTimeout(function(){say("playing")},700);
</script></body></html>"#; // i18n-skip

#[cfg(feature = "fixtures")]
pub fn fixture_page() -> String {
    FIXTURE_PAGE.to_owned()
}
