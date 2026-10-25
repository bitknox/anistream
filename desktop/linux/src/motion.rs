//! The motion tokens, as libadwaita animations: quick and standard are critically damped
//! springs (no bounce), fade-in and fade-out are timed. libadwaita reads `gtk-enable-animations`
//! itself, so with GNOME's Reduce Animation every one of these jumps to its end.

use adw::prelude::*;

/// A spring of zero bounce with `stiffness`: 1000 for quick, 440 for standard, the stiffness of
/// zero-bounce springs of 0.2 s and 0.3 s.
fn spring(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    stiffness: f64,
    target: impl IsA<adw::AnimationTarget>,
) -> adw::SpringAnimation {
    let params = adw::SpringParams::new(1.0, 1.0, stiffness);
    let animation = adw::SpringAnimation::new(widget, from, to, params, target);
    animation.set_clamp(true);
    animation
}

/// Selection tint, toggles, a row dimming after it is marked watched.
pub fn quick(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    target: impl IsA<adw::AnimationTarget>,
) -> adw::SpringAnimation {
    spring(widget, from, to, 1000.0, target)
}

/// Content changing inside a screen: a crossfade from placeholders, cards appearing.
pub fn standard(
    widget: &impl IsA<gtk::Widget>,
    from: f64,
    to: f64,
    target: impl IsA<adw::AnimationTarget>,
) -> adw::SpringAnimation {
    spring(widget, from, to, 440.0, target)
}

/// `widget`'s opacity, as an animation target.
pub fn opacity(widget: &impl IsA<gtk::Widget>) -> adw::PropertyAnimationTarget {
    adw::PropertyAnimationTarget::new(widget.as_ref(), "opacity")
}

/// Fade `widget` in from clear: ease-out, 200 ms. Art arriving, a line appearing.
pub fn fade_in(widget: &impl IsA<gtk::Widget>) -> adw::TimedAnimation {
    widget.set_opacity(0.0);
    let animation = adw::TimedAnimation::new(widget, 0.0, 1.0, 200, opacity(widget));
    animation.set_easing(adw::Easing::EaseOutCubic);
    animation.play();
    animation
}

/// Fade `widget` out: ease-in, 300 ms, then hide it.
pub fn fade_out(widget: &impl IsA<gtk::Widget>) -> adw::TimedAnimation {
    let from = widget.opacity();
    let animation = adw::TimedAnimation::new(widget, from, 0.0, 300, opacity(widget));
    animation.set_easing(adw::Easing::EaseInCubic);
    let hidden = widget.as_ref().clone();
    animation.connect_done(move |_| {
        hidden.set_visible(false);
        hidden.set_opacity(1.0);
    });
    animation.play();
    animation
}

/// Bring `widget` in with the standard spring on its opacity.
pub fn crossfade_in(widget: &impl IsA<gtk::Widget>) -> adw::SpringAnimation {
    widget.set_opacity(0.0);
    let animation = standard(widget, 0.0, 1.0, opacity(widget));
    animation.play();
    animation
}

/// One widget shown and hidden with the fade tokens: in over 200 ms ease-out, out over 300 ms
/// ease-in and then hidden. A change of mind takes over from wherever the last fade had got to,
/// so a card asked back while it fades out never blinks.
#[derive(Default)]
pub struct Fader {
    running: std::cell::RefCell<Option<adw::TimedAnimation>>,
    shown: std::cell::Cell<bool>,
}

impl Fader {
    /// Whether `widget` is shown or on its way in.
    pub fn shown(&self) -> bool {
        self.shown.get()
    }

    /// Fade `widget` in or out, unless it is already showing or going that way.
    pub fn set(&self, widget: &impl IsA<gtk::Widget>, show: bool) {
        let widget = widget.as_ref();
        if self.shown.get() == show
            && (show == widget.is_visible() || self.running.borrow().is_some())
        {
            return;
        }
        self.shown.set(show);
        if let Some(old) = self.running.take() {
            old.pause();
        }
        let from = if widget.is_visible() { widget.opacity() } else { 0.0 };
        widget.set_opacity(from);
        widget.set_visible(true);
        let (to, ms, easing) = if show {
            (1.0, 200, adw::Easing::EaseOutCubic)
        } else {
            (0.0, 300, adw::Easing::EaseInCubic)
        };
        let animation = adw::TimedAnimation::new(widget, from, to, ms, opacity(widget));
        animation.set_easing(easing);
        let done = widget.clone();
        animation.connect_done(move |_| {
            if !show {
                done.set_visible(false);
                done.set_opacity(1.0);
            }
        });
        animation.play();
        self.running.replace(Some(animation));
    }

    /// Hide `widget` at once, with nothing left running.
    pub fn hide_now(&self, widget: &impl IsA<gtk::Widget>) {
        if let Some(old) = self.running.take() {
            old.pause();
        }
        self.shown.set(false);
        widget.as_ref().set_visible(false);
        widget.as_ref().set_opacity(1.0);
    }
}
