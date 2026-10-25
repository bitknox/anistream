//! A party's episode that none of this device's sources found: the viewer is in the room with
//! nothing playing. The alert says why and offers what fits: nothing to choose before the
//! episode airs, Providers when there are no sources, and Choose Source otherwise, which lists
//! what each source has for the episode, the one picked playing with the room. Not Now stays in
//! the room, and Leave Party leaves it.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use anistream_core::{ids::AnilistId, traits::SourceCandidate};
use anistream_model::{command::Command, event::Event, notice::NoticeKind, party::NoSourceWhy};

use crate::{
    widgets,
    window::{Ctx, Size},
};

/// The alert for an episode none of the sources has.
pub fn open(ctx: &Rc<Ctx>, id: AnilistId, episode: &str, why: NoSourceWhy, reason: &str) {
    let (heading, body) = words(ctx, id, episode, why, reason);
    let dialog = adw::AlertDialog::new(Some(&heading), Some(&body));
    dialog.add_responses(&[("later", "Not Now"), ("leave", "Leave Party")]);
    dialog.set_close_response("later");
    match why {
        NoSourceWhy::NotAired => dialog.set_default_response(Some("later")),
        NoSourceWhy::NoSources => {
            dialog.add_response("providers", "Open Providers");
            dialog.set_response_appearance("providers", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("providers"));
        }
        NoSourceWhy::NotFound => {
            dialog.add_response("choose", "Choose Source");
            dialog.set_response_appearance("choose", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("choose"));
        }
    }
    let episode = episode.to_owned();
    dialog.connect_response(None, {
        let ctx = Rc::downgrade(ctx);
        move |_, response| {
            let Some(ctx) = ctx.upgrade() else { return };
            match response {
                "leave" => ctx.hub.send(Command::PartyLeave),
                "providers" => {
                    let _ = WidgetExt::activate_action(
                        &ctx.window,
                        "win.go",
                        Some(&"providers".to_variant()),
                    );
                }
                "choose" => choose(&ctx, id, &episode),
                _ => {}
            }
        }
    });
    close_on_leaving(ctx, dialog.upcast_ref());
    dialog.present(Some(&ctx.window));
}

/// The alert's heading and body, by why the episode has no source.
fn words(
    ctx: &Ctx,
    id: AnilistId,
    episode: &str,
    why: NoSourceWhy,
    reason: &str,
) -> (String, String) {
    let place = match ctx.hub.known.borrow().party.as_ref() {
        Some(party) => {
            format!("You're in {} on {}, with nothing playing.", party.room, party.server)
        }
        None => "You're in the room, with nothing playing.".into(),
    };
    let what = super::episode_of(ctx, id, episode);
    match why {
        NoSourceWhy::NotAired => {
            // The schedule's own words lead: "Episode 6 airs Tuesday at 13:05", or the episode
            // not out yet with when the next one is.
            let said = at_time(reason);
            match said.split_once("; ") {
                Some((first, then)) => (
                    crate::catalog::shown(first),
                    format!(
                        "{place} {}. Your sources won't have episode {episode} before it's out.",
                        crate::catalog::shown(then)
                    ),
                ),
                None => (
                    crate::catalog::shown(&said),
                    format!(
                        "{place} Your sources won't have it before then. It plays with the room \
                         once one of them does."
                    ),
                ),
            }
        }
        NoSourceWhy::NoSources => (
            "You have no sources yet".into(),
            format!(
                "{place} Sources come from plugins, which Providers adds. {what} plays with the \
                 room once one of them has it."
            ),
        ),
        NoSourceWhy::NotFound => (
            "None of your sources has this episode".into(),
            format!(
                "{place} {}. {what} plays with the room once you choose a source that has it.",
                crate::catalog::shown(reason.trim_end_matches('.'))
            ),
        ),
    }
}

/// A schedule's time read as one: "airs Tuesday 13:05" as "airs Tuesday at 13:05".
fn at_time(text: &str) -> String {
    let is_time = |word: &str| {
        let Some((hours, minutes)) = word.split_once(':') else { return false };
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        (1..=2).contains(&hours.len()) && minutes.len() == 2 && digits(hours) && digits(minutes)
    };
    let mut out: Vec<&str> = Vec::new();
    for word in text.split(' ') {
        let bare = word.trim_end_matches([';', '.', ',']);
        if is_time(bare) && out.last() != Some(&"at") {
            out.push("at");
        }
        out.push(word);
    }
    out.join(" ")
}

/// Close `dialog` when the party ends.
fn close_on_leaving(ctx: &Rc<Ctx>, dialog: &adw::Dialog) {
    let weak = dialog.downgrade();
    let listening = ctx.hub.listen(move |event| {
        if matches!(event, Event::PartyLeft { .. })
            && let Some(dialog) = weak.upgrade()
        {
            dialog.force_close();
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
}

/// What each source lists for the episode, loading in place; the one picked plays with the
/// room.
struct Choose {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    id: AnilistId,
    episode: String,
    spinner: adw::Spinner,
    mark: gtk::Image,
    line: gtk::Label,
    list: gtk::ListBox,
    again: gtk::Button,
    providers: gtk::Button,
    /// Waiting on the list: an alert in the meantime is why it found nothing.
    looking: Cell<bool>,
}

/// The second step: the sources' list for the episode.
pub fn choose(ctx: &Rc<Ctx>, id: AnilistId, episode: &str) {
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let cancel = gtk::Button::with_label("Cancel");
    header.pack_start(&cancel);
    let again = gtk::Button::with_label("Try Again");
    again.add_css_class("suggested-action");
    again.set_visible(false);
    header.pack_end(&again);
    let providers = gtk::Button::with_label("Open Providers");
    providers.add_css_class("suggested-action");
    providers.set_visible(false);
    header.pack_end(&providers);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(12);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let what = gtk::Label::new(Some(&super::episode_of(ctx, id, episode)));
    what.add_css_class("heading");
    what.set_xalign(0.0);
    what.set_wrap(true);
    column.append(&what);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(16, 16);
    let mark = gtk::Image::from_icon_name("dialog-error-symbolic");
    mark.add_css_class("error");
    mark.set_visible(false);
    let line = widgets::haze("Looking through your sources…", "");
    line.set_hexpand(true);
    let waiting = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    waiting.append(&spinner);
    waiting.append(&mark);
    waiting.append(&line);
    column.append(&waiting);
    let list = widgets::boxed_list();
    list.set_visible(false);
    list.update_property(&[gtk::accessible::Property::Label("Sources for this episode")]);
    column.append(&list);
    if ctx.size() != Size::Narrow {
        column.set_size_request(460, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(520)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .title("Choose a Source")
        .follows_content_size(true)
        .child(&view)
        .build();

    let state = Rc::new(Choose {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        id,
        episode: episode.to_owned(),
        spinner,
        mark,
        line,
        list,
        again: again.clone(),
        providers: providers.clone(),
        looking: Cell::new(false),
    });
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    again.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade() {
                state.look();
            }
        }
    });
    providers.connect_clicked({
        let weak = Rc::downgrade(&state);
        move |_| {
            let Some(state) = weak.upgrade() else { return };
            state.dialog.close();
            let _ = WidgetExt::activate_action(
                &state.ctx.window,
                "win.go",
                Some(&"providers".to_variant()),
            );
        }
    });
    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&state);
        move |event| {
            let Some(state) = weak.upgrade() else { return };
            match event {
                Event::Sources { id, episode, candidates }
                    if state.looking.get() && *id == state.id && *episode == state.episode =>
                {
                    state.listed(candidates);
                }
                // The engine says why it found nothing as an alert, not as an empty list.
                Event::Toast(notice)
                    if state.looking.get() && notice.kind == NoticeKind::Alert =>
                {
                    state.failed(&notice.text);
                }
                _ => {}
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    close_on_leaving(ctx, &dialog);
    widgets::keep(&dialog, Rc::clone(&state));
    state.look();
    dialog.present(Some(&ctx.window));
}

#[cfg(feature = "fixtures")]
pub use choose as rehearse_choose;

impl Choose {
    fn look(&self) {
        self.looking.set(true);
        self.again.set_visible(false);
        self.providers.set_visible(false);
        self.spinner.set_visible(true);
        self.mark.set_visible(false);
        self.line.add_css_class("haze");
        self.line.set_label("Looking through your sources…");
        self.list.set_visible(false);
        self.ctx.hub.send(Command::LoadSources { id: self.id, episode: self.episode.clone() });
    }

    fn failed(&self, why: &str) {
        self.looking.set(false);
        self.spinner.set_visible(false);
        self.mark.set_visible(true);
        // The reason in silk beside the mark.
        self.line.remove_css_class("haze");
        self.line.set_label(&crate::catalog::shown(why));
        self.again.set_visible(true);
    }

    fn listed(self: &Rc<Self>, found: &[SourceCandidate]) {
        self.looking.set(false);
        self.spinner.set_visible(false);
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        if found.is_empty() {
            self.line.set_label(&format!(
                "No source lists episode {}. Sources come from plugins, which Providers adds.",
                self.episode
            ));
            self.providers.set_visible(true);
            return;
        }
        self.line.set_label(&if found.len() == 1 {
            "1 source has it. The one you choose plays with the party.".to_owned()
        } else {
            format!("{} sources have it. The one you choose plays with the party.", found.len())
        });
        for candidate in found {
            let row = adw::ActionRow::builder()
                .title(gtk::glib::markup_escape_text(&candidate.title))
                .subtitle(gtk::glib::markup_escape_text(&detail(candidate)))
                .activatable(true)
                .build();
            row.set_title_lines(1);
            row.set_subtitle_lines(1);
            row.add_suffix(&widgets::chevron());
            let pick = candidate.clone();
            let weak = Rc::downgrade(self);
            row.connect_activated(move |_| {
                let Some(state) = weak.upgrade() else { return };
                // In the room already: this play plays with it.
                state.ctx.hub.send(Command::PlaySource {
                    id: state.id,
                    episode: state.episode.clone(),
                    provider_id: pick.provider_id.clone(),
                    source_id: pick.id.clone(),
                });
                state.dialog.close();
            });
            self.list.append(&row);
        }
        self.list.set_visible(true);
        crate::motion::fade_in(&self.list);
    }
}

/// "sample-source · 1080p · 1.4 GiB · 12 seeders"
fn detail(candidate: &SourceCandidate) -> String {
    let mut parts = vec![candidate.provider_id.clone()];
    if let Some(quality) = candidate.quality {
        parts.push(format!("{quality}p"));
    }
    if let Some(size) = candidate.size.as_deref().filter(|s| !s.is_empty()) {
        parts.push(size.to_owned());
    }
    if let Some(seeders) = candidate.seeders {
        parts.push(anistream_model::wording::plural(seeders as usize, "seeder"));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_in_the_schedule_reads_with_at() {
        assert_eq!(at_time("episode 4 airs today 18:30"), "episode 4 airs today at 18:30");
        assert_eq!(
            at_time("episode 6 hasn't aired yet; episode 5 airs Monday 2 November 9:05"),
            "episode 6 hasn't aired yet; episode 5 airs Monday 2 November at 9:05"
        );
        assert_eq!(at_time("airs at 18:30"), "airs at 18:30");
        assert_eq!(at_time("could not match 1:2"), "could not match 1:2");
    }
}
