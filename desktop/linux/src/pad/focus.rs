//! Moving focus the way the keyboard does, for a controller: native GTK focus moves, never
//! synthetic key events, so every widget answers as it does to its own keys.

use std::time::Duration;

use adw::prelude::*;
use anistream_model::input::Direction;
use gtk::{gio, glib};

use crate::window::Ctx;

/// The first descendant of `root`, depth first, for which `pick` is true. Hidden subtrees are
/// skipped.
fn find(root: &gtk::Widget, pick: &dyn Fn(&gtk::Widget) -> bool) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.is_visible() {
            if pick(&widget) {
                return Some(widget);
            }
            if let Some(found) = find(&widget, pick) {
                return Some(found);
            }
        }
        child = widget.next_sibling();
    }
    None
}

fn within(widget: &gtk::Widget, ancestor: &impl IsA<gtk::Widget>) -> bool {
    widget == ancestor.as_ref() || widget.is_ancestor(ancestor.as_ref())
}

/// Whether `widget` sits in the sidebar.
fn in_sidebar(ctx: &Ctx, widget: &gtk::Widget) -> bool {
    ctx.split.sidebar().is_some_and(|sidebar| within(widget, &sidebar))
}

/// Widgets that hold focusable children and are no stop of their own.
fn is_container(widget: &gtk::Widget) -> bool {
    widget.is::<gtk::ListBox>()
        || widget.is::<gtk::GridView>()
        || widget.is::<gtk::ListView>()
        || widget.is::<gtk::FlowBox>()
        || widget.is::<gtk::ScrolledWindow>()
        || widget.is::<gtk::Viewport>()
        || widget.is::<gtk::Window>()
        || widget.is::<gtk::Popover>()
}

fn stops(root: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.is_visible() && widget.is_child_visible() {
            if widget.is_focusable()
                && widget.is_sensitive()
                && widget.is_mapped()
                && widget.can_target()
                && !is_container(&widget)
            {
                out.push(widget.clone());
            }
            stops(&widget, out);
        }
        child = widget.next_sibling();
    }
}

/// The stop nearest `from` in `direction`: strictly beyond it along that axis, and among those
/// the one with the least gap, with a pull towards the one most in line. Rows and the controls
/// inside them are both stops; the gap prefers the row beside, not the control within.
fn nearest(
    root: &gtk::Widget,
    from: &gtk::Widget,
    direction: Direction,
) -> Option<gtk::Widget> {
    let origin = from.compute_bounds(root)?;
    let mut all = Vec::new();
    stops(root, &mut all);
    let (cx, cy) = (origin.x() + origin.width() / 2.0, origin.y() + origin.height() / 2.0);
    all.into_iter()
        .filter(|w| w != from && !from.is_ancestor(w) && !w.is_ancestor(from))
        .filter_map(|w| {
            let b = w.compute_bounds(root)?;
            let (gap, off) = match direction {
                Direction::Down => (
                    b.y() - (origin.y() + origin.height()),
                    (b.x() + b.width() / 2.0 - cx).abs(),
                ),
                Direction::Up => {
                    (origin.y() - (b.y() + b.height()), (b.x() + b.width() / 2.0 - cx).abs())
                }
                Direction::Right => (
                    b.x() - (origin.x() + origin.width()),
                    (b.y() + b.height() / 2.0 - cy).abs(),
                ),
                Direction::Left => {
                    (origin.x() - (b.x() + b.width()), (b.y() + b.height() / 2.0 - cy).abs())
                }
            };
            (gap >= -2.0).then(|| (gap.max(0.0) + 2.0 * off, w))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, w)| w)
}

/// Move focus one step. Within a popover or dialog the move stays there; between the sidebar
/// and the page it crosses only at an edge, to the sidebar's current row and back to where the
/// page last had focus. Nothing wraps.
pub fn step(ctx: &Ctx, direction: Direction) {
    let window = &ctx.window;
    let Some(before) = GtkWindowExt::focus(window) else {
        first(ctx);
        return;
    };
    let overlay = before.ancestor(gtk::Popover::static_type());
    let from_sidebar = in_sidebar(ctx, &before);
    if from_sidebar && direction == Direction::Right {
        if !restore_content_focus(ctx) {
            first(ctx);
        }
        return;
    }
    let root: gtk::Widget = overlay.clone().unwrap_or_else(|| match window.visible_dialog() {
        Some(dialog) => dialog.upcast(),
        None => window.clone().upcast(),
    });
    // Within the page, the sidebar is another place, reached only by its own edge.
    let mut moved = nearest(&root, &before, direction);
    if let Some(target) = &moved
        && overlay.is_none()
        && !from_sidebar
        && in_sidebar(ctx, target)
    {
        moved = None;
    }
    match moved {
        Some(target) => {
            target.grab_focus();
        }
        None => {
            if overlay.is_none() && !from_sidebar && direction == Direction::Left {
                // A page whose leftmost item has nothing beside it goes to the sidebar, which a
                // narrow window keeps away until now.
                focus_current_row(ctx);
            } else if matches!(direction, Direction::Up | Direction::Down) {
                scroll_more(ctx, &before, direction);
            }
        }
    }
}

/// Nothing further in view: a long grid or list scrolls a little and looks again, as its
/// unseen items are not built yet.
fn scroll_more(ctx: &Ctx, from: &gtk::Widget, direction: Direction) {
    let Some(scrolled) = from.ancestor(gtk::ScrolledWindow::static_type()) else { return };
    let Ok(scrolled) = scrolled.downcast::<gtk::ScrolledWindow>() else { return };
    let adjustment = scrolled.vadjustment();
    let bottom = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
    let by =
        adjustment.page_size() * 0.5 * if direction == Direction::Down { 1.0 } else { -1.0 };
    let value = (adjustment.value() + by).clamp(adjustment.lower(), bottom);
    if (value - adjustment.value()).abs() < 1.0 {
        return;
    }
    adjustment.set_value(value);
    let window = ctx.window.clone();
    let from = from.clone();
    glib::timeout_add_local_once(Duration::from_millis(80), move || {
        if GtkWindowExt::focus(&window).as_ref() != Some(&from) {
            return;
        }
        if let Some(target) = nearest(window.upcast_ref(), &from, direction) {
            target.grab_focus();
        }
    });
}

fn restore_content_focus(ctx: &Ctx) -> bool {
    let last = ctx.content_focus.borrow().as_ref().and_then(|weak| weak.upgrade());
    match last {
        Some(widget) if widget.is_mapped() && widget.is_sensitive() => widget.grab_focus(),
        _ => false,
    }
}

/// Focus the sidebar's row for the section on screen.
fn focus_current_row(ctx: &Ctx) {
    let Some(dest) = ctx.current_dest() else { return };
    if ctx.split.is_collapsed() && !ctx.split.shows_sidebar() {
        ctx.split.set_show_sidebar(true);
    }
    let Some(sidebar) = ctx.split.sidebar() else { return };
    let row = find(&sidebar, &|w| w.is::<gtk::ListBoxRow>() && w.widget_name() == dest.name());
    if let Some(row) = row {
        row.grab_focus();
    } else {
        let _ = sidebar.child_focus(gtk::DirectionType::TabForward);
    }
}

/// Press whatever has focus, or the nearest thing around it that can be pressed.
pub fn activate(widget: &gtk::Widget) {
    let mut at = Some(widget.clone());
    while let Some(w) = at {
        if w.is::<gtk::Window>() {
            return;
        }
        if w.is_sensitive() && w.activate() {
            return;
        }
        at = w.parent();
    }
}

pub fn is_text_field(widget: &gtk::Widget) -> bool {
    widget.is::<gtk::Editable>() || widget.is::<gtk::TextView>()
}

/// In Steam's game mode, its on-screen keyboard. Elsewhere the field stays focused for a real
/// keyboard, or the desktop's own.
pub fn open_keyboard() {
    if std::env::var_os("SteamGamepadUI").is_some() {
        let _ = gio::AppInfo::launch_default_for_uri(
            "steam://open/keyboard",
            gio::AppLaunchContext::NONE,
        );
    }
}

/// B on a popover, menu or dialog closes it. Whether there was one.
pub fn close_overlay(window: &adw::ApplicationWindow) -> bool {
    let Some(focus) = GtkWindowExt::focus(window) else { return false };
    if let Some(popover) = focus.ancestor(gtk::Popover::static_type())
        && let Ok(popover) = popover.downcast::<gtk::Popover>()
    {
        popover.popdown();
        return true;
    }
    if let Some(dialog) = focus.ancestor(adw::Dialog::static_type())
        && let Ok(dialog) = dialog.downcast::<adw::Dialog>()
    {
        dialog.close();
        return true;
    }
    false
}

/// The scrolled window around the focus, else the first one showing on the page.
fn scroller(ctx_window: &adw::ApplicationWindow) -> Option<gtk::ScrolledWindow> {
    let focus = GtkWindowExt::focus(ctx_window)?;
    if let Some(scrolled) = focus.ancestor(gtk::ScrolledWindow::static_type()) {
        return scrolled.downcast().ok();
    }
    None
}

/// Scroll the page under the focus by `pixels` without moving focus.
pub fn scroll_by(window: &adw::ApplicationWindow, pixels: f64) {
    let Some(scrolled) = scroller(window) else { return };
    let adjustment = scrolled.vadjustment();
    let top = adjustment.lower();
    let bottom = (adjustment.upper() - adjustment.page_size()).max(top);
    adjustment.set_value((adjustment.value() + pixels).clamp(top, bottom));
}

/// Page up (`-1`) or page down (`1`) in the list or grid around the focus: scroll a page and
/// put focus on what is now at the same place in the view.
pub fn page(ctx: &Ctx, by: i32) {
    let window = &ctx.window;
    let Some(scrolled) = scroller(window) else { return };
    let adjustment = scrolled.vadjustment();
    let before = GtkWindowExt::focus(window);
    let target_x = before
        .as_ref()
        .and_then(|w| w.compute_point(&scrolled, &gtk::graphene::Point::new(0.0, 0.0)))
        .map_or(40.0, |p| f64::from(p.x()) + 8.0);
    let step = adjustment.page_size() * 0.9 * f64::from(by);
    let top = adjustment.lower();
    let bottom = (adjustment.upper() - adjustment.page_size()).max(top);
    adjustment.set_value((adjustment.value() + step).clamp(top, bottom));
    let height = f64::from(scrolled.height());
    let target_y = if by > 0 { height * 0.15 } else { height * 0.85 };
    let scrolled = scrolled.clone();
    glib::timeout_add_local_once(Duration::from_millis(60), move || {
        let hit = scrolled.pick(target_x, target_y, gtk::PickFlags::DEFAULT);
        let mut at = hit;
        while let Some(w) = at {
            if w.is_focusable() && w.is_sensitive() && w.grab_focus() {
                return;
            }
            if w == scrolled.clone().upcast::<gtk::Widget>() {
                return;
            }
            at = w.parent();
        }
    });
}

/// Focus the first control of the page on screen: the one it names with `first-focus`, else the
/// first control in the page's content that is not in its header.
pub fn first(ctx: &Ctx) {
    let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) else { return };
    let page: gtk::Widget = page.upcast();
    if GtkWindowExt::focus(&ctx.window).is_some_and(|f| f.is_ancestor(&page)) {
        return;
    }
    let named = find(&page, &|w| w.has_css_class("first-focus") && w.is_focusable());
    let widget = named.or_else(|| {
        find(&page, &|w| {
            w.is_focusable()
                && w.is_sensitive()
                && !is_container(w)
                && w.ancestor(adw::HeaderBar::static_type()).is_none()
                && w.ancestor(gtk::HeaderBar::static_type()).is_none()
        })
    });
    if let Some(widget) = widget {
        widget.grab_focus();
    }
}
