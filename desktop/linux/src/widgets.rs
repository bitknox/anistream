//! Pieces the screens share: covers and banners with their flat placeholders, labels in the
//! app's type, and the poster grid.

use std::rc::Rc;

use anistream_model::catalog::Entry;
use adw::prelude::*;
use gtk::{gdk, gio, glib};

use crate::{fit::Fit, hub::Hub, words};

/// A colour as one pixel, stretched over a placeholder: the cover's own colour while its art
/// is on the way, and the raised colour when the title has none.
pub fn swatch(hex: Option<&str>) -> gdk::Texture {
    let rgb = hex.and_then(parse_hex).unwrap_or([0x22, 0x26, 0x38]);
    gdk::MemoryTexture::new(
        1,
        1,
        gdk::MemoryFormat::R8g8b8,
        &glib::Bytes::from_owned(rgb.to_vec()),
        3,
    )
    .upcast()
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Art in a rounded box: `url`'s once it is here, over a placeholder in `color`.
pub fn art(hub: &Hub, url: Option<&str>, color: Option<&str>, class: &str) -> (gtk::Box, gtk::Picture) {
    let picture = gtk::Picture::for_paintable(&swatch(color));
    picture.set_content_fit(gtk::ContentFit::Cover);
    picture.set_can_shrink(true);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    hub.art(&picture, url);
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.set_overflow(gtk::Overflow::Hidden);
    frame.add_css_class(class);
    frame.append(&picture);
    (frame, picture)
}

/// A title's cover at a fixed size, named for screen readers by its show.
pub fn cover(hub: &Hub, entry: &Entry, width: i32, height: i32) -> Fit {
    let (frame, _) = art(hub, entry.cover_url.as_deref(), entry.cover_color.as_deref(), "cover");
    if width <= 40 {
        frame.add_css_class("small");
    }
    let fit = Fit::new(&frame, f64::from(width) / f64::from(height.max(1)), width, true, 0);
    fit.set_halign(gtk::Align::Center);
    fit.set_valign(gtk::Align::Center);
    fit.update_property(&[gtk::accessible::Property::Label(&entry.title)]);
    fit
}

/// A cover that takes the width it is given, at 2:3, asking `width`.
pub fn cover_fill(hub: &Hub, entry: &Entry, width: i32) -> Fit {
    let (frame, _) = art(hub, entry.cover_url.as_deref(), entry.cover_color.as_deref(), "cover");
    let fit = Fit::new(&frame, 2.0 / 3.0, width, false, 0);
    fit.set_valign(gtk::Align::Start);
    fit.update_property(&[gtk::accessible::Property::Label(&entry.title)]);
    fit
}

/// A banner at 16:9 of the width it is given, cropped to at most `max_height` high.
pub fn banner(hub: &Hub, entry: &Entry, max_height: i32, class: &str) -> Fit {
    let url = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
    let (frame, _) = art(hub, url, entry.cover_color.as_deref(), "banner");
    frame.add_css_class(class);
    Fit::new(&frame, 16.0 / 9.0, max_height * 16 / 9, false, max_height)
}

/// An episode's still at a fixed size, 16:9.
pub fn still(hub: &Hub, url: Option<&str>, color: Option<&str>, width: i32) -> (Fit, gtk::Box) {
    let (frame, _) = art(hub, url, color, "still");
    let fit = Fit::new(&frame, 16.0 / 9.0, width, true, 0);
    fit.set_valign(gtk::Align::Center);
    (fit, frame)
}

/// A show's title in Zen Kaku Gothic New.
pub fn show_title(text: &str, size: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("show-title");
    label.add_css_class(size);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label
}

/// A line of secondary text.
pub fn haze(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("haze");
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}

/// A heading over a section of a page: "New this week".
pub fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("title-4");
    label.set_xalign(0.0);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    label
}

/// The button at the start of a content header bar that shows the sidebar while it is
/// collapsed into an overlay.
pub fn sidebar_button(split: &adw::OverlaySplitView) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_icon_name("sidebar-show-symbolic");
    button.set_tooltip_text(Some("Show Sidebar"));
    split.bind_property("collapsed", &button, "visible").sync_create().build();
    split.bind_property("show-sidebar", &button, "active").sync_create().bidirectional().build();
    button
}

/// A page of a destination: its header bar over `content`.
pub fn page(title: &str, header: &adw::HeaderBar, content: &impl IsA<gtk::Widget>) -> adw::NavigationPage {
    let view = adw::ToolbarView::new();
    view.add_top_bar(header);
    view.set_content(Some(content));
    adw::NavigationPage::builder().title(title).child(&view).build()
}

/// Keep `value` as long as `widget` lives: a screen's state, which its handlers hold weakly.
pub fn keep<T: 'static>(widget: &impl IsA<gtk::Widget>, value: T) {
    let value = std::cell::RefCell::new(Some(value));
    widget.as_ref().connect_destroy(move |_| drop(value.borrow_mut().take()));
}

/// A title in a grid, kept in the grid's model.
pub type Item = glib::BoxedAnyObject;

/// A poster grid: covers 140 to 180 wide, as many columns as fit, the show title in Zen Kaku and
/// one caption line under each. A cover opens its title; right-click offers the title's menu.
pub struct PosterGrid {
    pub widget: gtk::GridView,
    store: gio::ListStore,
}

impl PosterGrid {
    pub fn new(hub: &Rc<Hub>) -> Self {
        let store = gio::ListStore::new::<Item>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_bind({
            let hub = Rc::clone(hub);
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let Some(object) = item.item().and_downcast::<Item>() else { return };
                let entry = object.borrow::<Entry>();
                item.set_child(Some(&poster(&hub, &entry)));
            }
        });
        factory.connect_unbind(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(gtk::Widget::NONE);
            }
        });
        let selection = gtk::NoSelection::new(Some(store.clone()));
        let widget = gtk::GridView::new(Some(selection), Some(factory));
        widget.set_max_columns(12);
        widget.set_min_columns(3);
        widget.set_single_click_activate(true);
        widget.add_css_class("posters");
        widget.set_vexpand(true);
        widget.connect_activate({
            let store = store.clone();
            move |grid, position| {
                if let Some(object) = store.item(position).and_downcast::<Item>() {
                    let id = object.borrow::<Entry>().id.get();
                    let _ = grid.activate_action("win.open-title", Some(&id.to_variant()));
                }
            }
        });
        Self { widget, store }
    }

    pub fn set(&self, entries: &[Entry]) {
        let items: Vec<Item> = entries.iter().cloned().map(Item::new).collect();
        self.store.splice(0, self.store.n_items(), &items);
    }
}

fn poster(hub: &Hub, entry: &Entry) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.set_size_request(140, -1);
    column.append(&cover_fill(hub, entry, 148));
    let title = show_title(&entry.title, "row");
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_tooltip_text(Some(&entry.title));
    title.set_valign(gtk::Align::Start);
    column.append(&title);
    let caption = haze(&words::cover_caption(entry), "caption");
    caption.set_wrap(false);
    caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
    column.append(&caption);
    title_menu(&column, entry.id.get(), true);
    column
}

/// The menu a title's cover or row opens on right-click, Shift+F10 and the Menu key.
pub fn title_menu(widget: &impl IsA<gtk::Widget>, id: u32, planning: bool) {
    let menu = gio::Menu::new();
    if planning {
        menu.append(Some("Add to Planning"), Some(&format!("win.plan(uint32 {id})")));
    }
    menu.append(Some("Open Title"), Some(&format!("win.open-title(uint32 {id})")));
    context_menu(widget, &menu);
}

/// `menu` as `widget`'s context menu.
pub fn context_menu(widget: &impl IsA<gtk::Widget>, menu: &gio::Menu) {
    let widget = widget.as_ref();
    let popover = gtk::PopoverMenu::from_model(Some(menu));
    popover.set_parent(widget);
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_SECONDARY);
    click.connect_pressed({
        let popover = popover.clone();
        move |_, _, x, y| {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    });
    widget.add_controller(click);
    let press = gtk::GestureLongPress::new();
    press.set_touch_only(true);
    press.connect_pressed({
        let popover = popover.clone();
        move |_, x, y| {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    });
    widget.add_controller(press);
    let keys = gtk::ShortcutController::new();
    keys.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Shift>F10|Menu"),
        Some(gtk::CallbackAction::new({
            let popover = popover.clone();
            move |_, _| {
                popover.set_pointing_to(None);
                popover.popup();
                glib::Propagation::Stop
            }
        })),
    ));
    widget.add_controller(keys);
    // The popover goes with its widget.
    widget.connect_destroy(move |_| popover.unparent());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_colours_read_from_anilists_hex() {
        assert_eq!(parse_hex("#e4a15d"), Some([0xE4, 0xA1, 0x5D]));
        assert_eq!(parse_hex("e4a15d"), Some([0xE4, 0xA1, 0x5D]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
    }
}
