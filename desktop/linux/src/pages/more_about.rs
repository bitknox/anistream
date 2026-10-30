//! "More about this show": what the title page has no room for, in a dialog. People you follow,
//! where it streams, rankings, cast, staff, more like this and tags. A section with nothing in it
//! is left out.

use std::rc::Rc;

use adw::prelude::*;
use anistream_i18n::t;
use anistream_model::catalog::{CastRole, Entry, TitleDetail};
use gtk::glib;

use crate::{
    portal, say, widgets,
    window::{Ctx, Size},
};

/// Opens the dialog for `entry` over the window.
pub fn open(ctx: &Rc<Ctx>, entry: &Entry) {
    let Some(detail) = entry.detail.as_ref() else { return };
    let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
    column.set_margin_top(6);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    if ctx.size() != Size::Narrow {
        column.set_size_request(460, -1);
    }
    let dialog = adw::Dialog::builder().follows_content_size(true).build();

    let sections = [
        following(detail),
        streaming(ctx, detail),
        rankings(detail),
        cast(detail),
        staff(detail),
        more_like_this(ctx, &dialog, detail),
        tags(detail),
    ];
    let mut any = false;
    for section in sections.into_iter().flatten() {
        any = true;
        column.append(&section);
    }
    if !any {
        column.append(&widgets::haze(&t::more_empty(), ""));
    }

    let header = adw::HeaderBar::new();
    let heading = adw::WindowTitle::new(&entry.title, &t::title_more_about());
    header.set_title_widget(Some(&heading));
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(560)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    dialog.set_child(Some(&view));
    dialog.present(Some(&ctx.window));
}

/// A heading over a boxed list, with an optional note under it.
fn section(title: &str, note: Option<&str>, list: &gtk::ListBox) -> gtk::Box {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
    group.append(&widgets::heading(title));
    if let Some(note) = note {
        group.append(&widgets::haze(note, "caption"));
    }
    group.append(list);
    group
}

fn row(title: &str, subtitle: Option<&str>) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(glib::markup_escape_text(title)).build();
    if let Some(subtitle) = subtitle {
        row.set_subtitle(&glib::markup_escape_text(subtitle));
    }
    row
}

fn suffix(text: &str) -> gtk::Label {
    let label = widgets::haze(text, "numeric");
    label.set_valign(gtk::Align::Center);
    label
}

fn following(detail: &TitleDetail) -> Option<gtk::Box> {
    if detail.following.is_empty() {
        return None;
    }
    let list = widgets::boxed_list();
    for person in &detail.following {
        let status = person.status.map(crate::pages::library::segment_title);
        let progress =
            (person.progress > 0).then(|| t::episode_number(i64::from(person.progress)));
        let line = match (status, progress) {
            (Some(s), Some(p)) => Some(t::more_followed_progress(&s, &p)),
            (Some(s), None) => Some(s),
            (None, Some(p)) => Some(p),
            (None, None) => None,
        };
        let row = row(&person.name, line.as_deref());
        if let Some(score) = person.score.filter(|s| *s > 0) {
            row.add_suffix(&suffix(&t::more_followed_score(i64::from(score))));
        }
        list.append(&row);
    }
    Some(section(&t::more_following(), Some(&t::more_following_note()), &list))
}

fn streaming(ctx: &Rc<Ctx>, detail: &TitleDetail) -> Option<gtk::Box> {
    if detail.streaming_links.is_empty() {
        return None;
    }
    let list = widgets::boxed_list();
    for link in &detail.streaming_links {
        let language = link
            .language
            .as_deref()
            .map(|code| t::media_link_language(&anistream_model::present::language_name(code)));
        let row = row(&link.site, language.as_deref());
        row.set_activatable(true);
        row.add_suffix(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
        let window = ctx.window.clone();
        let url = link.url.clone();
        row.connect_activated(move |_| portal::open_uri(&window, &url, || {}));
        list.append(&row);
    }
    Some(section(&t::media_streaming_on(), Some(&t::media_streaming_region_note()), &list))
}

fn rankings(detail: &TitleDetail) -> Option<gtk::Box> {
    if detail.rankings.is_empty() {
        return None;
    }
    let list = widgets::boxed_list();
    for ranking in &detail.rankings {
        list.append(&row(&say::ranking_line(ranking), None));
    }
    Some(section(&t::more_rankings(), None, &list))
}

fn cast(detail: &TitleDetail) -> Option<gtk::Box> {
    if detail.characters.is_empty() {
        return None;
    }
    let list = widgets::boxed_list();
    for member in &detail.characters {
        let voice = member.voice_actors.first().map(|p| p.name.as_str());
        let row = row(&member.name, voice);
        row.add_suffix(&suffix(&match member.role {
            CastRole::Main => t::more_role_main(),
            CastRole::Supporting => t::more_role_supporting(),
            CastRole::Background => t::more_role_background(),
        }));
        list.append(&row);
    }
    Some(section(&t::more_cast(), Some(&t::more_cast_voices()), &list))
}

fn staff(detail: &TitleDetail) -> Option<gtk::Box> {
    if detail.staff.is_empty() {
        return None;
    }
    let list = widgets::boxed_list();
    for credit in &detail.staff {
        list.append(&row(&credit.name, Some(&credit.role)));
    }
    Some(section(&t::more_staff(), None, &list))
}

fn more_like_this(
    ctx: &Rc<Ctx>,
    dialog: &adw::Dialog,
    detail: &TitleDetail,
) -> Option<gtk::Box> {
    if detail.recommendations.is_empty() {
        return None;
    }
    let list = widgets::boxed_list();
    for linked in &detail.recommendations {
        let row = row(&linked.title, None);
        row.set_activatable(true);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let id = linked.id.get();
        let window = ctx.window.clone();
        let dialog = dialog.clone();
        row.connect_activated(move |_| {
            dialog.force_close();
            let _ =
                WidgetExt::activate_action(&window, "win.open-title", Some(&id.to_variant()));
        });
        list.append(&row);
    }
    Some(section(&t::more_like_this(), None, &list))
}

/// The tags in one line, the ones that give away the story behind a button with their count.
fn tags(detail: &TitleDetail) -> Option<gtk::Box> {
    let (spoilers, plain): (Vec<_>, Vec<_>) =
        detail.tags.iter().partition(|tag| tag.is_spoiler());
    if plain.is_empty() && spoilers.is_empty() {
        return None;
    }
    let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
    group.append(&widgets::heading(&t::title_tags()));
    if !plain.is_empty() {
        let names: Vec<&str> = plain.iter().map(|tag| tag.name.as_str()).collect();
        let line = gtk::Label::new(Some(&names.join(" · ")));
        line.set_xalign(0.0);
        line.set_wrap(true);
        group.append(&line);
    }
    if !spoilers.is_empty() {
        let names: Vec<&str> = spoilers.iter().map(|tag| tag.name.as_str()).collect();
        let hidden = gtk::Label::new(Some(&format!(
            "{}: {}", // i18n-skip
            t::more_spoiler(),
            names.join(" · ")
        )));
        hidden.set_xalign(0.0);
        hidden.set_wrap(true);
        hidden.add_css_class("haze");
        let revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideDown)
            .transition_duration(360)
            .child(&hidden)
            .build();
        let count = i64::try_from(spoilers.len()).unwrap_or(i64::MAX);
        let button = gtk::Button::with_label(&t::more_show_spoiler_tags(count));
        button.add_css_class("flat");
        button.set_halign(gtk::Align::Start);
        let shown = std::cell::Cell::new(false);
        button.connect_clicked({
            let revealer = revealer.clone();
            move |button| {
                let now = !shown.get();
                shown.set(now);
                revealer.set_reveal_child(now);
                button.set_label(&if now {
                    t::more_hide_spoiler_tags()
                } else {
                    t::more_show_spoiler_tags(count)
                });
            }
        });
        group.append(&button);
        group.append(&revealer);
    }
    Some(group)
}
