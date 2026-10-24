//! First launch: three short pages in a dialog (a bottom sheet on a narrow window), then the
//! app. Welcome, AniList, Sources. Every page can be skipped, closing the dialog skips the rest,
//! and each step is in Settings afterwards. It says plainly that anistream hosts nothing and
//! that sources belong to other people, and never mentions torrents.

use std::rc::Rc;

use adw::prelude::*;

use crate::{nagare, pages::join, widgets, window::Ctx};

/// Open first launch over the window.
pub fn open(ctx: &Rc<Ctx>) {
    let pages = adw::NavigationView::new();
    let dialog = adw::Dialog::builder()
        .title("Welcome to anistream")
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
    column.append(&centred("anistream", "title-1"));
    column.append(&centred("Keep track of anime and play it from sources you choose.", "body"));
    let legal = centred("anistream hosts no content.", "haze");
    column.append(&legal);
    let next = pill("Continue", true);
    next.set_margin_top(12);
    next.connect_clicked({
        let pages = pages.clone();
        move |_| pages.push_by_tag("anilist")
    });
    column.append(&next);
    step("welcome", "Welcome", &column)
}

fn anilist(ctx: &Rc<Ctx>, pages: &adw::NavigationView) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&centred("AniList", "title-1"));
    column.append(&centred(
        "Sign in to AniList to sync your list and where you stopped.",
        "body",
    ));
    let sign_in = pill("Sign In to AniList", true);
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
    let later = pill("Not Now", false);
    later.add_css_class("flat");
    later.connect_clicked({
        let pages = pages.clone();
        move |_| pages.push_by_tag("sources")
    });
    column.append(&later);
    let other = centred("Other trackers are in Settings, under Accounts.", "caption");
    other.add_css_class("haze");
    other.set_margin_top(6);
    column.append(&other);
    step("anilist", "AniList", &column)
}

fn sources(ctx: &Rc<Ctx>, dialog: &adw::Dialog) -> adw::NavigationPage {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&centred("Sources", "title-1"));
    column.append(&centred(
        "Sources are plugins, shared through plugin stores by invite.",
        "body",
    ));
    let link = adw::EntryRow::builder().title("Invite link").build();
    let join = gtk::Button::with_label("Join");
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
    let risk = centred(
        "Each plugin is someone else's code connecting to someone else's service. Use them at \
         your own risk.",
        "caption",
    );
    risk.add_css_class("haze");
    column.append(&risk);
    let skip = pill("Skip for Now", false);
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
    step("sources", "Sources", &column)
}
