//! What the pages of Your sources share: a section's heading over its boxed list, a page pushed
//! onto the destination that draws itself again when the engine says something new, and the
//! words for a source's failure.

use std::rc::Rc;

use adw::prelude::*;
use anistream_model::event::Event;

use crate::{widgets, window::Ctx};

/// A section of the Sources page: its heading over its boxed list.
pub fn section(title: &str, list: &gtk::ListBox) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = widgets::heading(title);
    section.append(&heading);
    section.append(list);
    section
}

/// A line of text under a field or button that went wrong: a vermilion mark and a sentence, the
/// field keeping its text. Hidden until it has something to say.
pub struct Problem {
    pub widget: gtk::Box,
    label: gtk::Label,
}

impl Problem {
    pub fn new() -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.set_valign(gtk::Align::Start);
        widget.set_visible(false);
        let mark = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        mark.add_css_class("mark");
        mark.add_css_class("vermilion");
        mark.set_valign(gtk::Align::Start);
        mark.set_margin_top(6);
        widget.append(&mark);
        let label = gtk::Label::new(None);
        label.add_css_class("problem");
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_hexpand(true);
        label.set_selectable(false);
        widget.append(&label);
        Self { widget, label }
    }

    /// Say `words`, fading in; `None` clears it.
    pub fn say(&self, words: Option<&str>) {
        match words {
            Some(words) => {
                self.label.set_label(words);
                if !self.widget.is_visible() {
                    self.widget.set_visible(true);
                    crate::motion::fade_in(&self.widget);
                }
                self.widget.update_property(&[gtk::accessible::Property::Description(words)]);
            }
            None => self.widget.set_visible(false),
        }
    }
}

/// Push a page of Your sources onto the destination on screen. `fill` draws it into `column`
/// and again whenever `redraw` says the engine's answer changed. `title` names the page in the
/// header and the navigation.
pub fn push(
    ctx: &Rc<Ctx>,
    tag: &str,
    header: &adw::HeaderBar,
    redraw: fn(&Event) -> bool,
    fill: impl Fn(&Rc<Ctx>, &adw::NavigationPage, &gtk::Box) + 'static,
) {
    let Some(nav) = ctx.nav() else { return };
    // One page at a time for a tag: asking again leaves the one that is up.
    if nav.visible_page().and_then(|p| p.tag()).is_some_and(|t| t == tag) {
        return;
    }
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroll = widgets::column_page(&column, 640);
    let page = widgets::page("", header, &scroll);
    page.set_tag(Some(tag));
    let refresh = {
        let ctx = Rc::downgrade(ctx);
        let page = page.downgrade();
        let column = column.downgrade();
        move || {
            let (Some(ctx), Some(page), Some(column)) =
                (ctx.upgrade(), page.upgrade(), column.upgrade())
            else {
                return;
            };
            while let Some(child) = column.first_child() {
                column.remove(&child);
            }
            fill(&ctx, &page, &column);
        }
    };
    refresh();
    let listening = ctx.hub.listen(move |event| {
        if redraw(event) {
            refresh();
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    page.connect_destroy(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    nav.push(&page);
}

/// A page's big title and its quiet line.
pub fn title_block(column: &gtk::Box, title: &str, line: &str) {
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("title-2");
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    column.append(&heading);
    if !line.is_empty() {
        let line = widgets::haze(line, "");
        line.set_margin_top(2);
        column.append(&line);
    }
}
