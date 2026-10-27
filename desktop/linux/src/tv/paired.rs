//! The TVs paired for Play on TV, in Settings' Integrations, as the Mac lists them: each with
//! when it was paired and when it was last reached, Forget, and a way to pair another.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use anistream_engine::remote::protocol::known::KnownTv;
use anistream_i18n::t;
use gtk::glib;

use super::{Tv, paired_note};
use crate::window::Ctx;

/// The paired TVs as drawn, and the one connected.
type Shown = (Vec<KnownTv>, Option<String>);

/// The group, following the paired TVs while it is shown. `None` when Play on TV is not there.
pub fn group(ctx: &Rc<Ctx>) -> Option<adw::PreferencesGroup> {
    let tv = &ctx.tv;
    if !tv.available() {
        return None;
    }
    let group = adw::PreferencesGroup::builder()
        .title(t::tv_play_on())
        .description(t::tv_fingerprint_note(&tv.short_fingerprint()))
        .build();
    let rows: Rc<std::cell::RefCell<Vec<gtk::Widget>>> = Rc::default();
    let fill = {
        let group = group.downgrade();
        let ctx = Rc::downgrade(ctx);
        let rows = Rc::clone(&rows);
        let shown: Rc<std::cell::RefCell<Option<Shown>>> = Rc::default();
        move || -> bool {
            let (Some(group), Some(ctx)) = (group.upgrade(), ctx.upgrade()) else {
                return false;
            };
            let paired = ctx.tv.paired();
            let connected = ctx.tv.connected().map(|f| f.to_hex());
            let now = (paired.clone(), connected.clone());
            if shown.borrow().as_ref() == Some(&now) {
                return true;
            }
            shown.replace(Some(now));
            for row in rows.borrow_mut().drain(..) {
                group.remove(&row);
            }
            let seen_now = glib::DateTime::now_utc().map_or(0, |t| t.to_unix());
            for tv in &paired {
                let on = connected.as_deref() == Some(tv.fingerprint.to_hex().as_str());
                let row = adw::ActionRow::builder()
                    .title(&tv.name)
                    .subtitle(paired_note(tv, on, seen_now))
                    .use_markup(false)
                    .build();
                let forget = gtk::Button::with_label(&t::action_forget());
                forget.set_valign(gtk::Align::Center);
                forget.connect_clicked({
                    let ctx = Rc::downgrade(&ctx);
                    let tv = tv.clone();
                    move |_| {
                        if let Some(ctx) = ctx.upgrade() {
                            confirm_forget(&ctx, &tv);
                        }
                    }
                });
                row.add_suffix(&forget);
                group.add(&row);
                rows.borrow_mut().push(row.upcast());
            }
            let another = adw::ActionRow::builder()
                .title(if paired.is_empty() { t::tv_none_paired() } else { t::tv_another() })
                .subtitle(t::tv_none_paired_detail())
                .build();
            let pair = gtk::Button::with_label(&t::tv_pair_menu());
            pair.set_valign(gtk::Align::Center);
            pair.connect_clicked({
                let tv = Rc::downgrade(&ctx.tv);
                move |_| {
                    if let Some(tv) = tv.upgrade() {
                        tv.open_picker();
                    }
                }
            });
            another.add_suffix(&pair);
            group.add(&another);
            rows.borrow_mut().push(another.upcast());
            true
        }
    };
    fill();
    let listening = Rc::new(Cell::new(0));
    let id = tv.listen({
        let listening = Rc::clone(&listening);
        let tv = Rc::downgrade(tv);
        move || {
            if !fill()
                && let Some(tv) = tv.upgrade()
            {
                tv.forget(listening.get());
            }
        }
    });
    listening.set(id);
    group.connect_destroy({
        let tv = Rc::downgrade(tv);
        move |_| {
            if let Some(tv) = tv.upgrade() {
                tv.forget(id);
            }
        }
    });
    Some(group)
}

fn confirm_forget(ctx: &Rc<Ctx>, paired: &KnownTv) {
    let dialog = adw::AlertDialog::new(
        Some(&t::tv_forget_title(&paired.name)),
        Some(&t::tv_forget_detail()),
    );
    dialog.add_response("cancel", &t::action_cancel());
    dialog.add_response("forget", &t::action_forget());
    dialog.set_response_appearance("forget", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let tv: std::rc::Weak<Tv> = Rc::downgrade(&ctx.tv);
    let fingerprint = paired.fingerprint;
    dialog.connect_response(None, move |_, response| {
        if let (Some(tv), "forget") = (tv.upgrade(), response) {
            tv.forget_tv(fingerprint);
        }
    });
    dialog.present(Some(&ctx.window));
}
