//! First launch: three short pages in a dialog (a bottom sheet on a narrow window), then the
//! app. Welcome, AniList, Sources. Every page can be skipped, closing the dialog skips the rest,
//! and each step is in Settings afterwards. It says plainly that anistream hosts nothing and
//! that sources belong to other people, and never mentions torrents.

use std::rc::Rc;

use adw::prelude::*;
use anistream_i18n::t;

use crate::{nagare, pages::join, widgets, window::Ctx};

/// Open first launch over the window.
pub fn open(ctx: &Rc<Ctx>) {
    let pages = adw::NavigationView::new();
    let dialog = adw::Dialog::builder()
        .title(t::onboarding_welcome_to())
        .content_width(560)
        .content_height(480)
        .child(&pages)
        .build();
    pages.add(&welcome(&pages));
    pages.add(&anilist(ctx, &pages));
    pages.add(&sources(ctx, &dialog));
    dialog.present(Some(&ctx.window));
}

/// A page of first launch: its header bar, and `content` centred under it.
fn step(tag: &str, title: &str, content: &gtk::Box) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    header.set_show_title(false);
    content.set_valign(gtk::Align::Center);
    content.set_vexpand(true);
    content.set_margin_top(12);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let clamp = adw::Clamp::builder().maximum_size(420).child(content).build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&clamp)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    adw::NavigationPage::builder().tag(tag).title(title).child(&view).build()
}

fn centred(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.set_max_width_chars(44);
    if !class.is_empty() {
        label.add_css_class(class);
    }
    label
}

fn pill(label: &str, primary: bool) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("pill");
    if primary {
        button.add_css_class("suggested-action");
    }
    button.set_halign(gtk::Align::Center);
    button
}

fn welcome(pages: &adw::NavigationView) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let nagare = nagare::picture(5);
    nagare.set_halign(gtk::Align::Center);
    nagare.set_margin_bottom(6);
    column.append(&nagare);
    column.append(&centred("anistream", "title-1")); // i18n-skip
    column.append(&centred(&t::onboarding_tagline(), "body"));
    let legal = centred(&t::legal_hosts_no_content(), "haze");
    column.append(&legal);
    let next = pill(&t::onboarding_continue(), true);
    next.set_margin_top(12);
    next.connect_clicked({
        let pages = pages.clone();
        move |_| pages.push_by_tag("anilist")
    });
    column.append(&next);
    step("welcome", &t::onboarding_welcome(), &column)
}

fn anilist(ctx: &Rc<Ctx>, pages: &adw::NavigationView) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let tracker = crate::say::tracker_name("anilist"); // i18n-skip
    column.append(&centred(tracker, "title-1"));
    column.append(&centred(&t::accounts_welcome_detail(tracker), "body"));
    let sign_in = pill(&t::library_signed_out_action(), true);
    sign_in.set_margin_top(12);
    sign_in.connect_clicked({
        let (pages, window) = (pages.clone(), ctx.window.clone());
        move |_| {
            let _ = WidgetExt::activate_action(
                &window,
                "win.connect",
                Some(&"anilist".to_variant()),
            );
            pages.push_by_tag("sources");
        }
    });
    column.append(&sign_in);
    let later = pill(&t::onboarding_not_now(), false);
    later.add_css_class("flat");
    later.connect_clicked({
        let pages = pages.clone();
        move |_| pages.push_by_tag("sources")
    });
    column.append(&later);
    let other = centred(&t::accounts_other_trackers_note(), "caption");
    other.add_css_class("haze");
    other.set_margin_top(6);
    column.append(&other);
    step("anilist", tracker, &column)
}

fn sources(ctx: &Rc<Ctx>, dialog: &adw::Dialog) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&centred(&t::onboarding_sources_title(), "title-1"));
    column.append(&centred(&t::legal_hosts_no_content(), "body"));
    column.append(&centred(&t::onboarding_where_hosts_linux(), "body"));
    // Their own media first: a folder or a server, as secondary buttons.
    let own = centred(&t::onboarding_own_heading(), "heading");
    own.set_margin_top(6);
    column.append(&own);
    let own_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    own_buttons.set_halign(gtk::Align::Center);
    let folder = pill(&t::sources_add_folder_button(), false);
    folder.connect_clicked({
        let (ctx, dialog) = (Rc::clone(ctx), dialog.clone());
        move |_| {
            dialog.close();
            crate::pages::folders::choose(&ctx);
        }
    });
    own_buttons.append(&folder);
    let server = pill(&t::sources_add_server_button(), false);
    server.connect_clicked({
        let (ctx, dialog) = (Rc::clone(ctx), dialog.clone());
        move |_| {
            dialog.close();
            crate::pages::servers::open_add(&ctx, None);
        }
    });
    own_buttons.append(&server);
    column.append(&own_buttons);
    let invite = centred(&t::onboarding_invite_heading(), "heading");
    invite.set_margin_top(6);
    column.append(&invite);
    let link = adw::EntryRow::builder().title(t::onboarding_invite_hint()).build();
    let join = gtk::Button::with_label(&t::onboarding_join());
    join.add_css_class("suggested-action");
    join.set_valign(gtk::Align::Center);
    join.set_sensitive(false);
    link.add_suffix(&join);
    link.connect_changed({
        let join = join.clone();
        move |link| join.set_sensitive(!link.text().trim().is_empty())
    });
    let list = widgets::boxed_list();
    list.set_margin_top(6);
    list.append(&link);
    column.append(&list);
    let risk = centred(&t::plugin_risk_warning(), "caption");
    risk.add_css_class("haze");
    column.append(&risk);
    let skip = pill(&t::onboarding_skip_for_now(), false);
    skip.add_css_class("flat");
    skip.set_margin_top(6);
    skip.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    column.append(&skip);
    let go = {
        let (ctx, dialog, link) = (Rc::clone(ctx), dialog.clone(), link.clone());
        move || {
            let text = link.text().trim().to_owned();
            if text.is_empty() {
                return;
            }
            dialog.close();
            join::open(&ctx, Some(&text));
        }
    };
    join.connect_clicked({
        let go = go.clone();
        move |_| go()
    });
    link.connect_entry_activated(move |_| go());
    step("sources", &t::onboarding_sources_title(), &column)
}
