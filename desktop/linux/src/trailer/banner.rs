//! A title page's trailer: it plays behind the banner once the page has settled, and the banner
//! clears over it when a frame is playing.
//!
//! The page opens with its banner as it always did. After the transition has ended and half a
//! second has passed, and when the engine allows autoplay for this title, the trailer loads under
//! the banner. Nothing is drawn over the video: the header bar's band stays clear of it, the
//! banner and its scrim are gone, and the title and its actions step down below it. Scrolling
//! the page more than 8 px pauses the trailer and brings the banner back; scrolling to the top
//! within 30 s plays it again, and after 30 away it is let go of for this visit. Leaving the
//! page, starting an episode, opening a dialog or the window losing focus releases it at once.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;

use super::{Stage, may_autoplay, preview};
use crate::{hub::Hub, window::Ctx};

const SETTLE: Duration = Duration::from_millis(500);
const AWAY: Duration = Duration::from_secs(30);
/// Scrolling further than this pauses it.
const SCROLLED: f64 = 8.0;
/// Where the head of the page sits over the banner, as its margin says.
const HEAD_OVER: i32 = 120;

pub struct PageTrailer {
    pub stage: Rc<Stage>,
    pub sound: gtk::ToggleButton,
    ctx: Rc<Ctx>,
    id: u32,
    head: gtk::Box,
    header: adw::HeaderBar,
    scrolled: Cell<f64>,
    settled: Cell<bool>,
    /// Let go of for this visit: scrolled away for good.
    abandoned: Cell<bool>,
    settle: RefCell<Option<glib::SourceId>>,
    away: RefCell<Option<glib::SourceId>>,
    step: RefCell<Option<adw::TimedAnimation>>,
    handlers: RefCell<Vec<glib::SignalHandlerId>>,
}

impl PageTrailer {
    /// The trailer for the page of title `id`, whose banner is `banner`. `head` is the block
    /// that stands over the banner's lower part, `header` the header bar over its top.
    pub fn new(
        ctx: &Rc<Ctx>,
        id: u32,
        banner: &impl IsA<gtk::Widget>,
        head: &gtk::Box,
        header: &adw::HeaderBar,
        navigation: &adw::NavigationPage,
    ) -> Rc<Self> {
        let stage = Stage::new(banner);
        let sound = preview::sound_button();
        let this = Rc::new(Self {
            stage: Rc::clone(&stage),
            sound: sound.clone(),
            ctx: Rc::clone(ctx),
            id,
            head: head.clone(),
            header: header.clone(),
            scrolled: Cell::new(0.0),
            settled: Cell::new(false),
            abandoned: Cell::new(false),
            settle: RefCell::default(),
            away: RefCell::default(),
            step: RefCell::default(),
            handlers: RefCell::default(),
        });

        sound.connect_toggled({
            let stage = Rc::downgrade(&stage);
            move |button| {
                if let Some(stage) = stage.upgrade() {
                    stage.set_sound(button.is_active());
                }
                button.set_icon_name(preview::sound_icon(button.is_active()));
            }
        });
        stage.on_showing({
            let this = Rc::downgrade(&this);
            move |showing| {
                if let Some(this) = this.upgrade() {
                    this.showing(showing);
                }
            }
        });

        // The page settles once its transition has ended; coming back to it from a sheet or
        // the player settles it again.
        navigation.connect_shown({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.settle_soon();
                }
            }
        });
        stage.widget.connect_map({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade()
                    && this.settled.get()
                {
                    this.settle_soon();
                }
            }
        });
        stage.widget.connect_unmap({
            let this = Rc::downgrade(&this);
            move |_| {
                if let Some(this) = this.upgrade() {
                    this.stage.stop();
                    this.cancel_settle();
                }
            }
        });

        // The window losing focus or a dialog over the page lets the trailer go.
        let window = ctx.window.clone();
        let stop = {
            let this = Rc::downgrade(&this);
            move || {
                if let Some(this) = this.upgrade() {
                    this.stage.stop();
                }
            }
        };
        let handlers = vec![
            window.connect_is_active_notify({
                let stop = stop.clone();
                move |window| {
                    if !window.is_active() {
                        stop();
                    }
                }
            }),
            window.connect_visible_dialog_notify(move |window| {
                if window.visible_dialog().is_some() {
                    stop();
                }
            }),
        ];
        this.handlers.replace(handlers);
        this
    }

    /// Let go of everything the page holds: the trailer, its timers and its hold on the window.
    pub fn release(&self) {
        self.stage.stop();
        preview::page_playing(false);
        self.cancel_settle();
        if let Some(source) = self.away.take() {
            source.remove();
        }
        for handler in self.handlers.take() {
            self.ctx.window.disconnect(handler);
        }
    }

    fn cancel_settle(&self) {
        if let Some(source) = self.settle.take() {
            source.remove();
        }
    }

    fn settle_soon(self: &Rc<Self>) {
        self.cancel_settle();
        let this = Rc::downgrade(self);
        self.settle.replace(Some(glib::timeout_add_local_once(SETTLE, move || {
            if let Some(this) = this.upgrade() {
                this.settle.take();
                this.settled.set(true);
                this.maybe_start();
            }
        })));
    }

    /// Start the trailer if everything allows it now. Called as the page's facts arrive, as the
    /// engine's word on autoplay changes and when the page settles.
    pub fn maybe_start(self: &Rc<Self>) {
        if !self.settled.get()
            || self.abandoned.get()
            || self.stage.active()
            || self.scrolled.get() > SCROLLED
            || !self.stage.widget.is_mapped()
            || !self.ctx.window.is_active()
            || self.ctx.window.visible_dialog().is_some()
        {
            return;
        }
        let Some(entry) = self.hub().entry(anistream_core::ids::AnilistId::new(self.id)) else {
            return;
        };
        let Some(trailer) = entry.trailer.clone() else { return };
        let inset = self.header.height().max(0);
        if may_autoplay(self.hub(), &entry) && self.stage.fits(inset) {
            self.stage.start(&trailer, inset);
        }
    }

    fn hub(&self) -> &Hub {
        &self.ctx.hub
    }

    /// The page scrolled to `value`.
    pub fn scrolled(self: &Rc<Self>, value: f64) {
        self.scrolled.set(value);
        if value > SCROLLED {
            if self.stage.showing() {
                self.stage.pause();
                let this = Rc::downgrade(self);
                self.away.replace(Some(glib::timeout_add_local_once(AWAY, move || {
                    if let Some(this) = this.upgrade() {
                        this.away.take();
                        // Not again on this visit.
                        this.abandoned.set(true);
                        this.stage.stop();
                    }
                })));
            }
        } else if let Some(source) = self.away.take() {
            source.remove();
            self.stage.resume();
        }
    }

    /// The banner cleared for a trailer, or came back.
    fn showing(&self, showing: bool) {
        self.sound.set_visible(showing);
        preview::page_playing(showing);
        if showing {
            self.sound.set_active(false);
            self.step_to(self.stage.widget.height() + 12);
        } else if !self.stage.active() {
            // Gone for good: the title goes back over the banner.
            self.step_to(HEAD_OVER);
        }
    }

    /// Move the title block's top margin to `margin`: down below the video, or back over the
    /// banner.
    fn step_to(&self, margin: i32) {
        if let Some(old) = self.step.take() {
            old.pause();
        }
        let head = self.head.clone();
        let target = adw::CallbackAnimationTarget::new({
            let head = head.clone();
            move |value| head.set_margin_top(value.round() as i32)
        });
        let animation = adw::TimedAnimation::new(
            &head,
            f64::from(head.margin_top()),
            f64::from(margin),
            300,
            target,
        );
        animation.set_easing(adw::Easing::EaseOutCubic);
        animation.play();
        self.step.replace(Some(animation));
    }
}
