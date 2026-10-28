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

/// Move focus one step. Within a popover or dialog the move stays there; between the sidebar
/// and the page it crosses only at an edge, to the sidebar's current row and back to where the
/// page last had focus. Nothing wraps.
pub fn step(ctx: &Ctx, direction: Direction) {
    let window = &ctx.window;
    let before = GtkWindowExt::focus(window);
    let kind = match direction {
        Direction::Up => gtk::DirectionType::Up,
        Direction::Down => gtk::DirectionType::Down,
        Direction::Left => gtk::DirectionType::Left,
        Direction::Right => gtk::DirectionType::Right,
    };
    let overlay =
        before.as_ref().and_then(|w| w.ancestor(gtk::Popover::static_type()));
    let from_sidebar = before.as_ref().is_some_and(|w| in_sidebar(ctx, w));
    if from_sidebar && direction == Direction::Right {
        if !restore_content_focus(ctx) {
            first(ctx);
        }
        return;
    }
    let root: gtk::Widget = overlay.clone().unwrap_or_else(|| window.clone().upcast());
    let moved = root.child_focus(kind);
    tracing::warn!(moved, root = ?root.type_().name(), "PADDEBUG step");
    let after = GtkWindowExt::focus(window);
    if overlay.is_none() && !from_sidebar && direction == Direction::Left {
        let landed_in_sidebar = moved && after.as_ref().is_some_and(|w| in_sidebar(ctx, w));
        // A page whose leftmost item has nothing beside it opens the sidebar that a narrow
        // window keeps away.
        if landed_in_sidebar || !moved {
            focus_current_row(ctx);
        }
    }
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
    let row = find(&sidebar, &|w| {
        w.is::<gtk::ListBoxRow>() && w.widget_name() == dest.name()
    });
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
    if let Some(popover) = focus.ancestor(gtk::Popover::static_type()) {
        if let Ok(popover) = popover.downcast::<gtk::Popover>() {
            popover.popdown();
            return true;
        }
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
                && !w.is::<gtk::Window>()
                && w.ancestor(adw::HeaderBar::static_type()).is_none()
                && w.ancestor(gtk::HeaderBar::static_type()).is_none()
        })
    });
    if let Some(widget) = widget {
        widget.grab_focus();
    }
}
