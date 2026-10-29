//! Play trailer: the trailer in a window of its own, with YouTube's controls and sound on. When
//! the embed refuses the video (embedding off, region-locked) or WebKitGTK is not there, the
//! window closes and the watch URL opens in the browser, as it always did.

use std::{cell::Cell, rc::Rc, time::Duration};

use adw::prelude::*;
use anistream_model::{
    catalog::Entry,
    command::Command,
    trailer::{AppIdentity, EmbedStyle, TrailerMessage, embed_page},
};
use gtk::glib;

use super::webkit::{Signal, Web};
use crate::window::Ctx;

/// Play `entry`'s trailer in a window. False when there is nothing to show it in, and the
/// browser was asked instead.
pub fn play(ctx: &Rc<Ctx>, entry: &Entry) -> bool {
    let browser = |entry: &Entry| {
        if let Some(url) = entry.trailer_url.clone() {
            ctx.hub.send(Command::PlayTrailer { url, title: entry.title.clone() });
        }
    };
    let Some(trailer) = entry.trailer.clone() else {
        browser(entry);
        return false;
    };
    let app = AppIdentity::new(AppIdentity::LINUX);
    let Some(page) = full_page(&trailer, &app) else {
        browser(entry);
        return false;
    };

    let dialog = adw::Dialog::builder()
        .title(&entry.title)
        .content_width(960)
        .content_height(600)
        .build();
    let refused = Rc::new(Cell::new(false));
    let heard = Rc::new(Cell::new(false));
    let give_up = {
        let dialog = dialog.downgrade();
        let refused = Rc::clone(&refused);
        Rc::new(move || {
            if !refused.replace(true) {
                // Closing from inside the view's own signal: after it has returned.
                let dialog = dialog.clone();
                glib::idle_add_local_once(move || {
                    if let Some(dialog) = dialog.upgrade() {
                        dialog.force_close();
                    }
                });
            }
        })
    };
    let web = {
        let give_up = Rc::clone(&give_up);
        let heard = Rc::clone(&heard);
        Web::new(&page, &app.base_url(), true, true, move |signal| match signal {
            Signal::Failed | Signal::Message(TrailerMessage::Error { .. }) => give_up(),
            Signal::Message(_) => heard.set(true),
        })
    };
    let Some(web) = web else {
        browser(entry);
        return false;
    };
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&web.widget));
    dialog.set_child(Some(&view));
    // Held until the dialog closes, then let go of: the web process and its decoder go with it.
    let held = std::cell::RefCell::new(Some(web));
    dialog.connect_closed({
        let ctx = Rc::downgrade(ctx);
        let entry = entry.clone();
        let refused = Rc::clone(&refused);
        move |dialog| {
            if let Some(child) = dialog.child().and_downcast::<adw::ToolbarView>() {
                child.set_content(gtk::Widget::NONE);
            }
            held.take();
            if refused.get()
                && let Some(ctx) = ctx.upgrade()
                && let Some(url) = entry.trailer_url.clone()
            {
                ctx.hub.send(Command::PlayTrailer { url, title: entry.title.clone() });
            }
        }
    });
    // A trailer that never reports in is let go of, and the browser has it instead.
    glib::timeout_add_local_once(Duration::from_secs(15), {
        let heard = Rc::clone(&heard);
        move || {
            if !heard.get() {
                give_up();
            }
        }
    });
    dialog.present(Some(&ctx.window));
    true
}

fn full_page(trailer: &anistream_model::catalog::Trailer, app: &AppIdentity) -> Option<String> {
    #[cfg(feature = "fixtures")]
    if crate::fixtures::active() {
        return Some(super::stage::fixture_page());
    }
    embed_page(trailer, EmbedStyle::Full, app)
}
