//! The game controller: its buttons become the model's [`PadAction`]s, and this module carries the
//! browsing ones out as GTK focus moves, activations and menus. The player's go to the player.
//!
//! Input comes from libmanette ([`manette`]), which sees a controller as evdev codes in the
//! standard layout. Nothing polls: libmanette wakes the main loop on an event, and the only
//! timers run while a direction is held (focus repeating) or the right stick is deflected
//! (scrolling). With no controller connected the app does nothing at all for this.
//!
//! The first controller input puts the window in controller mode: the focus ring shows, the
//! pointer hides and hover previews stay shut. Moving the pointer leaves it, and a key press
//! leaves it too but keeps the ring.

mod focus;
mod help;
pub mod manette;

pub use help::show as show_help;

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_i18n::t;
use anistream_model::{
    input::{Direction, PadAction, PadButton, PadContext, StickDirections, pad_action},
    settings::SettingKey,
};
use gtk::{gdk, glib};

use self::manette::{Input, Manette, Signal};
use crate::{
    catalog,
    pages::Dest,
    window::{Ctx, Shell},
};

/// A held direction repeats after this long, then at the shorter interval.
const REPEAT_AFTER: Duration = Duration::from_millis(400);
const REPEAT_EVERY: Duration = Duration::from_millis(80);
/// A key arriving this soon after a controller's event is the same press, sent twice by Steam's
/// desktop mode alongside the controller, and is ignored.
const SAME_PRESS: Duration = Duration::from_millis(50);
/// View held this long shows the controller's help instead of toggling the sidebar.
const HELP_AFTER: Duration = Duration::from_secs(1);

// evdev button and axis codes, as libmanette reports the standard layout.
const BTN_SOUTH: u16 = 0x130;
const BTN_EAST: u16 = 0x131;
// Linux calls the Xbox X button north and Y west, whatever their place on the pad.
const BTN_X: u16 = 0x133;
const BTN_Y: u16 = 0x134;
const BTN_TL: u16 = 0x136;
const BTN_TR: u16 = 0x137;
const BTN_TL2: u16 = 0x138;
const BTN_TR2: u16 = 0x139;
const BTN_SELECT: u16 = 0x13a;
const BTN_START: u16 = 0x13b;
const BTN_THUMBL: u16 = 0x13d;
const BTN_THUMBR: u16 = 0x13e;
const BTN_DPAD_UP: u16 = 0x220;
const BTN_DPAD_DOWN: u16 = 0x221;
const BTN_DPAD_LEFT: u16 = 0x222;
const BTN_DPAD_RIGHT: u16 = 0x223;
const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;
const ABS_Z: u16 = 2;
const ABS_RX: u16 = 3;
const ABS_RY: u16 = 4;
const ABS_RZ: u16 = 5;

/// A button by its evdev code.
fn button_of(code: u16) -> Option<PadButton> {
    Some(match code {
        BTN_SOUTH => PadButton::South,
        BTN_EAST => PadButton::East,
        BTN_X => PadButton::West,
        BTN_Y => PadButton::North,
        BTN_TL => PadButton::LeftShoulder,
        BTN_TR => PadButton::RightShoulder,
        BTN_TL2 => PadButton::LeftTrigger,
        BTN_TR2 => PadButton::RightTrigger,
        BTN_SELECT => PadButton::Select,
        BTN_START => PadButton::Start,
        BTN_THUMBL => PadButton::LeftStick,
        BTN_THUMBR => PadButton::RightStick,
        BTN_DPAD_UP => PadButton::DpadUp,
        BTN_DPAD_DOWN => PadButton::DpadDown,
        BTN_DPAD_LEFT => PadButton::DpadLeft,
        BTN_DPAD_RIGHT => PadButton::DpadRight,
        _ => return None,
    })
}

fn button_for(direction: Direction) -> PadButton {
    match direction {
        Direction::Up => PadButton::DpadUp,
        Direction::Down => PadButton::DpadDown,
        Direction::Left => PadButton::DpadLeft,
        Direction::Right => PadButton::DpadRight,
    }
}

/// A direction held, and what repeats it.
struct Repeat {
    direction: Direction,
    /// The sources that press it: a button's and a stick's can overlap.
    holders: u8,
    source: Option<glib::SourceId>,
}

pub struct Pad {
    me: Weak<Self>,
    shell: Weak<Shell>,
    ctx: Rc<Ctx>,
    monitor: RefCell<Option<Manette>>,
    /// The controllers connected: libmanette's id and the name each reports.
    devices: RefCell<Vec<(usize, String)>>,
    /// Whether this session has already said a controller connected.
    announced: Cell<bool>,
    left: RefCell<StickDirections>,
    stick: Cell<(f64, f64)>,
    scroll: Cell<f64>,
    scrolling: RefCell<Option<gtk::TickCallbackId>>,
    triggers: Cell<[bool; 2]>,
    repeat: RefCell<Option<Repeat>>,
    /// View pressed and not yet let go: it is the sidebar's on release, the help's at 1 s.
    view: RefCell<Option<(PadAction, glib::SourceId)>>,
    last_input: Cell<Option<Instant>>,
    /// When controller mode began, so the pointer's first move after it is not its end.
    entered: Cell<Option<Instant>>,
    pointer: Cell<Option<(f64, f64)>>,
    /// Speed before the right trigger doubled it.
    held_speed: Cell<Option<f64>>,
}

impl Pad {
    /// The controller for `shell`'s window, listening for controllers if libmanette is there.
    pub fn attach(shell: &Rc<Shell>) -> Rc<Self> {
        let pad = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            shell: Rc::downgrade(shell),
            ctx: Rc::clone(&shell.ctx),
            monitor: RefCell::default(),
            devices: RefCell::default(),
            announced: Cell::new(false),
            left: RefCell::default(),
            stick: Cell::new((0.0, 0.0)),
            scroll: Cell::new(0.0),
            scrolling: RefCell::default(),
            triggers: Cell::new([false; 2]),
            repeat: RefCell::default(),
            view: RefCell::default(),
            last_input: Cell::new(None),
            entered: Cell::new(None),
            pointer: Cell::new(None),
            held_speed: Cell::new(None),
        });
        pad.watch_window();
        let weak = Rc::downgrade(&pad);
        let monitor = Manette::start(move |signal| {
            if let Some(pad) = weak.upgrade() {
                pad.heard(signal);
            }
        });
        pad.monitor.replace(monitor);
        pad
    }

    fn enabled(&self) -> bool {
        self.ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::ControllerInput)
            .is_none_or(|row| catalog::is_on(&row))
    }

    fn swap_face(&self) -> bool {
        self.ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::SwapFaceButtons)
            .is_some_and(|row| catalog::is_on(&row))
    }

    // What the window does around the controller: the pointer and keys leave controller mode,
    // and a key that is the controller's own press again is ignored.
    fn watch_window(self: &Rc<Self>) {
        let window = &self.ctx.window;
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(pad) = weak.upgrade() else { return glib::Propagation::Proceed };
            if pad.same_press() && pad.duplicated(key) {
                return glib::Propagation::Stop;
            }
            pad.leave(false);
            glib::Propagation::Proceed
        });
        window.add_controller(keys);

        let motion = gtk::EventControllerMotion::new();
        motion.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        motion.connect_motion(move |_, x, y| {
            let Some(pad) = weak.upgrade() else { return };
            // The first motion only says where the pointer is.
            let Some((px, py)) = pad.pointer.replace(Some((x, y))) else { return };
            let moved = (x - px).abs() + (y - py).abs() > 2.0;
            let settled =
                pad.entered.get().is_none_or(|at| at.elapsed() > Duration::from_millis(250));
            if moved && settled {
                pad.leave(true);
            }
        });
        window.add_controller(motion);
    }

    fn same_press(&self) -> bool {
        self.last_input.get().is_some_and(|at| at.elapsed() < SAME_PRESS)
    }

    /// Whether `key` is one Steam's desktop mode sends beside the controller's own event.
    fn duplicated(&self, key: gdk::Key) -> bool {
        use gdk::Key;
        matches!(
            key,
            Key::Up
                | Key::Down
                | Key::Left
                | Key::Right
                | Key::Return
                | Key::KP_Enter
                | Key::Escape
                | Key::space
                | Key::Page_Up
                | Key::Page_Down
        )
    }

    /// Whether the window is in controller mode.
    pub fn in_controller_mode(&self) -> bool {
        self.ctx.controller_mode.get()
    }

    fn enter(&self) {
        if self.ctx.controller_mode.replace(true) {
            return;
        }
        self.entered.set(Some(Instant::now()));
        let window = &self.ctx.window;
        window.set_focus_visible(true);
        window.set_cursor_from_name(Some("none"));
        // The first press shows the ring where focus belongs: on the page's first control, not on
        // the sidebar row the window starts with.
        let in_page = GtkWindowExt::focus(window).is_some_and(|w| {
            !w.is::<gtk::Window>()
                && self.ctx.split.sidebar().is_none_or(|s| w != s && !w.is_ancestor(&s))
        });
        if !in_page {
            focus::first(&self.ctx);
        }
    }

    /// Leave controller mode. A key keeps the ring; the pointer takes it away.
    fn leave(&self, pointer: bool) {
        if !self.ctx.controller_mode.replace(false) {
            return;
        }
        self.ctx.window.set_cursor(None);
        if pointer {
            self.ctx.window.set_focus_visible(false);
        }
    }

    fn heard(self: &Rc<Self>, signal: Signal) {
        match signal {
            Signal::Connected { id, name } => {
                if self.devices.borrow().iter().any(|(known, _)| *known == id) {
                    return;
                }
                self.devices.borrow_mut().push((id, name.clone()));
                self.ctx
                    .controller_device
                    .replace(self.devices.borrow().first().map(|d| d.1.clone()));
                self.ctx.controller_seen();
                if !self.announced.replace(true) && self.enabled() {
                    self.ctx.toast(&t::controller_connected(&name));
                }
            }
            Signal::Disconnected { id } => {
                let was_in_use =
                    self.devices.borrow().first().is_some_and(|(first, _)| *first == id);
                self.devices.borrow_mut().retain(|(known, _)| *known != id);
                self.ctx
                    .controller_device
                    .replace(self.devices.borrow().first().map(|d| d.1.clone()));
                self.release_all();
                if was_in_use && self.enabled() && self.in_controller_mode() {
                    self.ctx.toast(&t::controller_disconnected());
                }
            }
            Signal::Input(input) => self.input(input),
        }
    }

    /// Everything held is let go: a controller that went away mid-press sends no release.
    fn release_all(self: &Rc<Self>) {
        self.stop_repeat();
        self.stick.set((0.0, 0.0));
        *self.left.borrow_mut() = StickDirections::default();
        self.set_scroll(0.0);
        if let Some((_, source)) = self.view.borrow_mut().take() {
            source.remove();
        }
        self.end_hold_speed();
    }

    /// One input from a controller, in the standard layout.
    pub fn input(self: &Rc<Self>, input: Input) {
        if !self.enabled() || !self.ctx.window.is_active() {
            return;
        }
        match input {
            Input::Button { code, pressed } => {
                if let Some(button) = button_of(code) {
                    self.button(button, pressed);
                }
            }
            Input::Axis { code, value } => self.axis(code, value),
            Input::Hat { axis, value } => self.hat(axis, value),
        }
    }

    fn hat(self: &Rc<Self>, axis: u8, value: i8) {
        // Hat 0 is the D-pad: x is the even axis, y the odd.
        let (negative, positive) = if axis.is_multiple_of(2) {
            (PadButton::DpadLeft, PadButton::DpadRight)
        } else {
            (PadButton::DpadUp, PadButton::DpadDown)
        };
        self.button(negative, value < 0);
        self.button(positive, value > 0);
    }

    fn axis(self: &Rc<Self>, code: u16, value: f64) {
        match code {
            ABS_X | ABS_Y => {
                let (mut x, mut y) = self.stick.get();
                if code == ABS_X {
                    x = value;
                } else {
                    y = value;
                }
                self.stick.set((x, y));
                let changes = self.left.borrow_mut().update(x, y);
                for (direction, pressed) in changes {
                    self.button(button_for(direction), pressed);
                }
            }
            ABS_RY => self.set_scroll(if value.abs() > 0.2 { value } else { 0.0 }),
            ABS_RX => {}
            ABS_Z | ABS_RZ => {
                let which = usize::from(code == ABS_RZ);
                let mut held = self.triggers.get();
                let now = if held[which] { value > 0.3 } else { value > 0.5 };
                if now != held[which] {
                    held[which] = now;
                    self.triggers.set(held);
                    let button = if which == 0 {
                        PadButton::LeftTrigger
                    } else {
                        PadButton::RightTrigger
                    };
                    self.button(button, now);
                }
            }
            _ => {}
        }
    }

    /// The model's reading of the window now.
    fn context(&self) -> PadContext {
        self.shell.upgrade().map_or(PadContext::Browse, |shell| shell.pad_context())
    }

    /// A direction pressed or released, with focus to move.
    fn direction(self: &Rc<Self>, direction: Direction, pressed: bool) {
        if pressed {
            let was = self.in_controller_mode();
            self.enter();
            if !was {
                // The press that woke the controller only shows where focus is.
                return;
            }
            self.start_repeat(direction);
        } else {
            self.end_repeat(direction);
        }
    }

    /// A button pressed or released.
    pub fn button(self: &Rc<Self>, button: PadButton, pressed: bool) {
        self.last_input.set(Some(Instant::now()));
        let direction = match button {
            PadButton::DpadUp => Some(Direction::Up),
            PadButton::DpadDown => Some(Direction::Down),
            PadButton::DpadLeft => Some(Direction::Left),
            PadButton::DpadRight => Some(Direction::Right),
            _ => None,
        };
        if let Some(direction) = direction
            && matches!(
                self.context(),
                PadContext::Browse | PadContext::Player { controls_shown: true, .. }
            )
        {
            self.direction(direction, pressed);
            return;
        }
        if pressed {
            self.enter();
        }
        if button == PadButton::Select {
            self.view_button(pressed);
            return;
        }
        let context = self.context();
        let action = pad_action(button, pressed, &context, self.swap_face());
        if let Some(action) = action {
            self.carry_out(action);
        }
    }

    /// View: the sidebar on a short press, the controller's help held for a second.
    fn view_button(self: &Rc<Self>, pressed: bool) {
        let context = self.context();
        if pressed {
            let Some(action) = pad_action(PadButton::Select, true, &context, false) else {
                return;
            };
            let weak = self.me.clone();
            let source = glib::timeout_add_local_once(HELP_AFTER, move || {
                if let Some(pad) = weak.upgrade() {
                    pad.view.borrow_mut().take();
                    pad.show_help();
                }
            });
            if let Some((_, old)) = self.view.borrow_mut().replace((action, source)) {
                old.remove();
            }
        } else if let Some((action, source)) = self.view.borrow_mut().take() {
            source.remove();
            self.carry_out(action);
        }
    }

    fn show_help(&self) {
        help::show(&self.ctx, &self.context());
    }

    // Held directions.

    fn start_repeat(self: &Rc<Self>, direction: Direction) {
        {
            let mut repeat = self.repeat.borrow_mut();
            match repeat.as_mut() {
                Some(held) if held.direction == direction => {
                    held.holders += 1;
                    return;
                }
                _ => {}
            }
            if let Some(old) = repeat.take()
                && let Some(source) = old.source
            {
                source.remove();
            }
        }
        self.step(direction);
        let weak = self.me.clone();
        let source = glib::timeout_add_local_once(REPEAT_AFTER, move || {
            let Some(pad) = weak.upgrade() else { return };
            pad.begin_repeating(direction);
        });
        self.repeat.replace(Some(Repeat { direction, holders: 1, source: Some(source) }));
    }

    fn begin_repeating(self: &Rc<Self>, direction: Direction) {
        let weak = self.me.clone();
        let source = glib::timeout_add_local(REPEAT_EVERY, move || {
            let Some(pad) = weak.upgrade() else { return glib::ControlFlow::Break };
            if pad.repeat.borrow().as_ref().is_none_or(|r| r.direction != direction) {
                return glib::ControlFlow::Break;
            }
            pad.step(direction);
            glib::ControlFlow::Continue
        });
        if let Some(held) = self.repeat.borrow_mut().as_mut() {
            held.source = Some(source);
        } else {
            source.remove();
        }
    }

    fn end_repeat(&self, direction: Direction) {
        let mut repeat = self.repeat.borrow_mut();
        let Some(held) = repeat.as_mut() else { return };
        if held.direction != direction {
            return;
        }
        held.holders = held.holders.saturating_sub(1);
        if held.holders == 0
            && let Some(old) = repeat.take()
            && let Some(source) = old.source
        {
            source.remove();
        }
    }

    fn stop_repeat(&self) {
        if let Some(old) = self.repeat.borrow_mut().take()
            && let Some(source) = old.source
        {
            source.remove();
        }
    }

    /// One press of a direction: the model says what it does where focus is.
    fn step(self: &Rc<Self>, direction: Direction) {
        let context = self.context();
        if let Some(action) = pad_action(button_for(direction), true, &context, false) {
            self.carry_out(action);
        }
    }

    // Right stick: scroll the page without moving focus.

    fn set_scroll(self: &Rc<Self>, value: f64) {
        self.scroll.set(value);
        let running = self.scrolling.borrow().is_some();
        if value == 0.0 {
            if let Some(id) = self.scrolling.borrow_mut().take() {
                id.remove();
            }
        } else if !running {
            self.enter();
            let weak = self.me.clone();
            let last = Cell::new(None::<i64>);
            let id = self.ctx.window.add_tick_callback(move |_, clock| {
                let Some(pad) = weak.upgrade() else { return glib::ControlFlow::Break };
                let now = clock.frame_time();
                let before = last.replace(Some(now));
                if let Some(before) = before {
                    let seconds = (now - before) as f64 / 1_000_000.0;
                    focus::scroll_by(&pad.ctx.window, pad.scroll.get() * 900.0 * seconds);
                }
                glib::ControlFlow::Continue
            });
            self.scrolling.replace(Some(id));
        }
    }

    // Carrying out.

    fn carry_out(self: &Rc<Self>, action: PadAction) {
        let Some(shell) = self.shell.upgrade() else { return };
        let window = &self.ctx.window;
        match action {
            PadAction::Focus(direction) => {
                if shell.is_player() {
                    shell.player.pad_touch();
                }
                focus::step(&self.ctx, direction);
            }
            PadAction::Activate => {
                if shell.is_player() {
                    shell.player.pad_touch();
                }
                self.activate();
            }
            PadAction::Back => {
                if focus::close_overlay(window) {
                    return;
                }
                if shell.is_player() {
                    shell.player.pad_back();
                } else {
                    shell.back();
                }
            }
            PadAction::ItemMenu => {
                if let Some(widget) = GtkWindowExt::focus(window) {
                    let _ = widget.activate_action("context.popup", None);
                }
            }
            PadAction::Search => {
                let _ = WidgetExt::activate_action(window, "win.search", None);
            }
            PadAction::MainMenu => self.ctx.open_main_menu(),
            PadAction::ToggleSidebar => {
                let split = &self.ctx.split;
                split.set_show_sidebar(!split.shows_sidebar());
            }
            PadAction::Section(by) => self.section(&shell, by),
            PadAction::Page(by) => focus::page(&self.ctx, by),
            PadAction::Player(command) => shell.player.pad_command(command),
            PadAction::Panel(panel) => shell.player.pad_panel(panel),
            PadAction::Reveal(target) => shell.player.pad_reveal(target),
            PadAction::PlayPause => shell.player.pad_play_pause(),
            PadAction::HoldSpeed(on) => {
                if on {
                    self.held_speed.set(Some(shell.player.pad_hold_speed()));
                } else {
                    self.end_hold_speed();
                }
            }
        }
    }

    fn end_hold_speed(&self) {
        if let Some(speed) = self.held_speed.take()
            && let Some(shell) = self.shell.upgrade()
        {
            shell.player.pad_restore_speed(speed);
        }
    }

    /// A on the focused widget: press it, or open the keyboard for a field.
    fn activate(&self) {
        let Some(widget) = GtkWindowExt::focus(&self.ctx.window) else { return };
        if focus::is_text_field(&widget) {
            focus::open_keyboard();
            return;
        }
        focus::activate(&widget);
    }

    /// The previous or next section in the sidebar's order. Nothing wraps.
    fn section(&self, shell: &Rc<Shell>, by: i32) {
        let Some(now) = self.ctx.current_dest() else { return };
        let at = Dest::ALL.iter().position(|d| *d == now).unwrap_or(0) as i32;
        let Some(next) = usize::try_from(at + by).ok().and_then(|i| Dest::ALL.get(i)) else {
            return;
        };
        shell.go(*next);
        self.first_focus();
    }

    /// Focus goes to a page's first control: its main action, first row or first cover.
    pub fn first_focus(&self) {
        let ctx = Rc::clone(&self.ctx);
        glib::idle_add_local_once(move || focus::first(&ctx));
    }

    /// A page came on screen. In controller mode focus goes to its first control, unless it
    /// already sits somewhere in the page (a page going back keeps its place).
    pub fn page_shown(&self) {
        if !self.in_controller_mode() {
            return;
        }
        self.first_focus();
    }
}

/// For the shots: a controller that presses what it is told, for the screens it drives.
#[allow(dead_code)]
impl Pad {
    pub fn shot_press(self: &Rc<Self>, button: PadButton) {
        self.button(button, true);
        self.button(button, false);
    }

    pub fn shot_enter(&self) {
        self.enter();
    }

    pub fn shot_leave(&self) {
        self.stop_repeat();
        self.leave(true);
    }

    pub fn shot_help(&self) {
        self.show_help();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evdev_codes_map_to_the_buttons_by_their_place_on_the_pad() {
        assert_eq!(button_of(BTN_SOUTH), Some(PadButton::South));
        assert_eq!(button_of(BTN_EAST), Some(PadButton::East));
        // Linux names the Xbox X button north, but it sits on the west.
        assert_eq!(button_of(BTN_X), Some(PadButton::West));
        assert_eq!(button_of(BTN_Y), Some(PadButton::North));
        assert_eq!(button_of(BTN_START), Some(PadButton::Start));
        assert_eq!(button_of(BTN_SELECT), Some(PadButton::Select));
        assert_eq!(button_of(BTN_DPAD_LEFT), Some(PadButton::DpadLeft));
        assert_eq!(button_of(0x13c), None, "the guide button is the system's");
    }
}
