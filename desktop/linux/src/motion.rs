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
