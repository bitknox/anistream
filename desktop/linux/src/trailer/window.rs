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
        // The header bar and a 16:9 content area: 960 by 540 under it.
        .content_height(540 + 47)
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
            Signal::Ending | Signal::Message(_) => heard.set(true),
        })
    };
    let Some(web) = web else {
        browser(entry);
        return false;
    };
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    let frame = gtk::AspectFrame::new(0.5, 0.5, 16.0 / 9.0, false);
    frame.set_child(Some(&web.widget));
    view.set_content(Some(&frame));
    dialog.set_child(Some(&view));
    // Ctrl-W closes the trailer window, not the app window behind it. Captured before the web
    // view sees the key.
    let close = gtk::ShortcutController::new();
    close.set_propagation_phase(gtk::PropagationPhase::Capture);
    close.add_shortcut(gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Control>w"),
        Some(gtk::CallbackAction::new({
            let dialog = dialog.downgrade();
            move |_, _| {
                if let Some(dialog) = dialog.upgrade() {
                    dialog.close();
                }
                glib::Propagation::Stop
            }
        })),
    ));
    dialog.add_controller(close);
    // Held until the dialog closes, then let go of: the web process and its decoder go with it.
    let held = std::cell::RefCell::new(Some(web));
    dialog.connect_closed({
        let ctx = Rc::downgrade(ctx);
        let entry = entry.clone();
        let refused = Rc::clone(&refused);
        move |dialog| {
            if let Some(child) = dialog.child().and_downcast::<adw::ToolbarView>()
                && let Some(frame) = child.content().and_downcast::<gtk::AspectFrame>()
            {
                frame.set_child(gtk::Widget::NONE);
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
