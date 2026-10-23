//! A title's page: the banner, then the cover beside the show title, the metadata line, the one
//! amber action and the status and More menus; under them the season picker and the episodes,
//! and the details and Related in a side column. One column below the wide size, and on narrow
//! windows no cover beside the title, the action at full width and smaller stills.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_model::{
    catalog::{Entry, LibrarySegment, LinkedTitle, RelationKind},
    command::Command,
    episodes::EpisodeRow,
    event::Event,
    settings::SettingKey,
};
use gtk::{gio, glib};

use crate::{
    catalog, widgets,
    window::{Ctx, Size},
    words::{self, QueueItem},
};

struct Page {
    ctx: Rc<Ctx>,
    id: AnilistId,
    navigation: glib::WeakRef<adw::NavigationPage>,
    banner: gtk::Box,
    banner_filled: std::cell::Cell<bool>,
    head: gtk::Box,
    side: gtk::Box,
    seasons: gtk::Box,
    episodes: gtk::ListBox,
    episodes_state: gtk::Stack,
    columns: gtk::Box,
    rows: RefCell<Vec<EpisodeRow>>,
    listening: RefCell<Option<u64>>,
    sized: RefCell<Size>,
}

pub fn page(ctx: &Rc<Ctx>, id: AnilistId) -> adw::NavigationPage {
    let entry = ctx.hub.entry(id);
    let title = entry.as_ref().map_or("Title", |e| e.title.as_str()).to_owned();

    let head = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let seasons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let episodes = gtk::ListBox::new();
    episodes.add_css_class("boxed-list");
    episodes.set_selection_mode(gtk::SelectionMode::None);
    let loading = adw::Spinner::new();
    loading.set_size_request(32, 32);
    loading.set_halign(gtk::Align::Center);
    loading.set_margin_top(24);
    let episodes_state = gtk::Stack::new();
    episodes_state.add_named(&loading, Some("loading"));
    episodes_state.add_named(&episodes, Some("rows"));
    episodes_state.add_named(
        &widgets::haze("No episodes are listed for this title yet.", ""),
        Some("empty"),
    );
    episodes_state.set_vhomogeneous(false);

    let main = gtk::Box::new(gtk::Orientation::Vertical, 12);
    main.set_hexpand(true);
    main.append(&head);
    main.append(&seasons);
    let heading = widgets::heading("Episodes");
    heading.set_margin_top(12);
    main.append(&heading);
    main.append(&episodes_state);

    let side = gtk::Box::new(gtk::Orientation::Vertical, 18);
    side.set_size_request(260, -1);
    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 36);
    columns.append(&main);
    columns.append(&side);
    columns.set_margin_start(36);
    columns.set_margin_end(36);
    columns.set_margin_bottom(36);

    let banner_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&banner_slot);
    content.append(
        &adw::Clamp::builder()
            .maximum_size(1200)
            .tightening_threshold(1200)
            .child(&columns)
            .build(),
    );
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();

    let header = adw::HeaderBar::new();
    header.add_css_class("flat");
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_extend_content_to_top_edge(true);
    view.set_content(Some(&scroll));
    let navigation = adw::NavigationPage::builder().title(&title).child(&view).build();

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        id,
        navigation: navigation.downgrade(),
        banner: banner_slot,
        banner_filled: std::cell::Cell::new(false),
        head,
        side,
        seasons,
        episodes,
        episodes_state,
        columns,
        rows: RefCell::default(),
        listening: RefCell::default(),
        sized: RefCell::new(ctx.size()),
    });

    let weak = Rc::downgrade(&page);
    let listening = ctx.hub.listen(move |event| {
        if let Some(page) = weak.upgrade() {
            page.heard(event);
        }
    });
    page.listening.replace(Some(listening));
    ctx.on_size({
        let weak = Rc::downgrade(&page);
        move |size| {
            if let Some(page) = weak.upgrade() {
                page.resized(size);
            }
        }
    });
    page.actions();
    if let Some(entry) = &entry {
        page.fill(entry);
    }
    ctx.hub.send(Command::LoadDetail(id));
    ctx.hub.send(Command::LoadEpisodes(id));
    // Kept as long as the page it draws, and deaf once that is gone.
    navigation.connect_destroy(move |_| {
        if let Some(id) = page.listening.borrow_mut().take() {
            page.ctx.hub.forget(id);
        }
    });
    navigation
}

impl Page {
    fn heard(&self, event: &Event) {
        match event {
            Event::Detail(entry) if entry.id == self.id => {
                if let Some(navigation) = self.navigation.upgrade() {
                    navigation.set_title(&entry.title);
                }
                self.fill(entry);
            }
            Event::Episodes { id, rows } if *id == self.id => {
                self.rows.replace(rows.clone());
                self.draw_episodes();
            }
            Event::Progress { id, episodes, .. } if *id == self.id => {
                {
                    let mut rows = self.rows.borrow_mut();
                    for row in rows.iter_mut() {
                        if let Some(p) = episodes.iter().find(|p| p.number == row.number) {
                            row.watched = p.watched;
                            row.completed = p.completed;
                        }
                    }
                }
                if let Some(entry) = self.ctx.hub.entry(self.id) {
                    self.fill(&entry);
                }
                self.draw_episodes();
            }
            _ => {}
        }
    }

    fn resized(&self, size: Size) {
        self.sized.replace(size);
        let wide = size == Size::Wide;
        self.columns.set_orientation(if wide {
            gtk::Orientation::Horizontal
        } else {
            gtk::Orientation::Vertical
        });
        self.side.set_size_request(if wide { 260 } else { -1 }, -1);
        let margin = if size == Size::Narrow { 12 } else { 36 };
        self.columns.set_margin_start(margin);
        self.columns.set_margin_end(margin);
        if let Some(entry) = self.ctx.hub.entry(self.id) {
            self.fill(&entry);
        }
        self.draw_episodes();
    }

    /// The page's own actions: the status menu and More.
    fn actions(&self) {
        let group = gio::SimpleActionGroup::new();
        let status = gio::SimpleAction::new("status", Some(glib::VariantTy::STRING));
        let hub = Rc::downgrade(&self.ctx.hub);
        let id = self.id;
        status.connect_activate({
            let hub = hub.clone();
            move |_, arg| {
                let Some(hub) = hub.upgrade() else { return };
                let Some(name) = arg.and_then(String::from_variant) else { return };
                if let Some(status) =
                    LibrarySegment::ALL.into_iter().find(|s| s.label() == name)
                {
                    hub.send(Command::SetStatus { id, status });
                    hub.send(Command::LoadDetail(id));
                }
            }
        });
        group.add_action(&status);
        let previous = gio::SimpleAction::new("mark-previous", None);
        previous.connect_activate({
            let hub = hub.clone();
            move |_, _| {
                let Some(hub) = hub.upgrade() else { return };
                let Some(entry) = hub.entry(id) else { return };
                let next = entry.progress.map_or(1, |p| p.next);
                let episodes: Vec<String> = (1..next).map(|n| n.to_string()).collect();
                if !episodes.is_empty() {
                    hub.send(Command::SetWatched { id, episodes, watched: true });
                }
            }
        });
        group.add_action(&previous);
        let show_settings = gio::SimpleAction::new("show-settings", None);
        show_settings.set_enabled(false);
        group.add_action(&show_settings);
        let watched = gio::SimpleAction::new(
            "watched",
            Some(&glib::VariantType::new("(sb)").expect("a type")),
        );
        watched.connect_activate(move |_, arg| {
            let Some(hub) = hub.upgrade() else { return };
            if let Some((episode, on)) = arg.and_then(<(String, bool)>::from_variant) {
                hub.send(Command::SetWatched { id, episodes: vec![episode], watched: on });
            }
        });
        group.add_action(&watched);
        if let Some(navigation) = self.navigation.upgrade() {
            navigation.insert_action_group("title", Some(&group));
        }
    }

    /// The header block and the side column, from what is known of the title.
    fn fill(&self, entry: &Entry) {
        let size = *self.sized.borrow();
        let narrow = size == Size::Narrow;
        clear(&self.head);
        let hub = &self.ctx.hub;
        // The banner, once: its art does not change with the detail.
        if !self.banner_filled.replace(true) {
            self.banner.append(&widgets::banner(hub, entry, 280, "flush"));
        }
        let id = entry.id.get();

        let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
        text.set_valign(gtk::Align::End);
        text.set_hexpand(true);
        let title = widgets::show_title(&entry.title, "page");
        text.append(&title);
        if let Some(other) = entry.secondary.as_deref().or(entry.title_native.as_deref())
            && other != entry.title
        {
            text.append(&widgets::haze(other, ""));
        }
        text.append(&widgets::haze(&words::meta_line(entry), "numeric"));

        let item = QueueItem::of(entry);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(10);
        match item.button() {
            Some(label) => {
                let button = gtk::Button::with_label(&label);
                button.add_css_class("pill");
                button.add_css_class("suggested-action");
                button.set_action_name(Some("win.play"));
                button.set_action_target_value(Some(
                    &(id, item.episode.to_string()).to_variant(),
                ));
                button.set_hexpand(narrow);
                actions.append(&button);
            }
            None => {
                if let Some(waiting) = item.waiting() {
                    actions.append(&widgets::haze(&waiting, ""));
                }
            }
        }
        let status_menu = gio::Menu::new();
        for segment in LibrarySegment::ALL {
            status_menu.append(
                Some(&catalog::shown(segment.label())),
                Some(&format!("title.status::{}", segment.label())),
            );
        }
        let status = gtk::MenuButton::builder()
            .label(
                entry
                    .list_status
                    .map_or("Add to List".to_owned(), |s| catalog::shown(s.label())),
            )
            .menu_model(&status_menu)
            .valign(gtk::Align::Center)
            .build();
        status.add_css_class("pill");
        let more_menu = gio::Menu::new();
        more_menu.append(Some("Show Settings…"), Some("title.show-settings"));
        more_menu.append(Some("Mark All Previous Watched"), Some("title.mark-previous"));
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text("More")
            .menu_model(&more_menu)
            .valign(gtk::Align::Center)
            .build();
        more.add_css_class("circular");
        if !narrow {
            actions.append(&status);
            actions.append(&more);
        }
        text.append(&actions);
        if narrow {
            let menus = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            menus.append(&status);
            menus.append(&more);
            text.append(&menus);
        }

        let line = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        line.set_margin_top(18);
        if !narrow {
            line.append(&widgets::cover(hub, entry, 120, 180));
        }
        line.append(&text);
        self.head.append(&line);

        if !entry.synopsis.is_empty() {
            let synopsis = gtk::Label::new(Some(&entry.synopsis));
            synopsis.set_wrap(true);
            synopsis.set_xalign(0.0);
            synopsis.set_margin_top(18);
            synopsis.set_max_width_chars(90);
            self.head.append(&synopsis);
        }

        self.fill_seasons(entry);
        self.fill_side(entry, size);
    }

    /// The franchise's seasons as a picker, when there is more than one.
    fn fill_seasons(&self, entry: &Entry) {
        clear(&self.seasons);
        let Some(seasons) = entry.detail.as_ref().and_then(|d| d.seasons.clone()) else {
            return;
        };
        if seasons.len() < 2 {
            return;
        }
        let names: Vec<String> = (1..=seasons.len()).map(|n| format!("Season {n}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let picker = gtk::DropDown::from_strings(&refs);
        picker.set_tooltip_text(Some("Season"));
        if let Some(at) = seasons.iter().position(|s| s.id == entry.id) {
            picker.set_selected(at as u32);
        }
        picker.connect_selected_notify(move |picker| {
            if let Some(season) = seasons.get(picker.selected() as usize)
                && season.id != AnilistId::new(0)
            {
                let _ = picker
                    .activate_action("win.open-title", Some(&season.id.get().to_variant()));
            }
        });
        self.seasons.append(&picker);
    }

    /// Details, and Related as small covers.
    fn fill_side(&self, entry: &Entry, size: Size) {
        clear(&self.side);
        let details = gtk::Box::new(gtk::Orientation::Vertical, 6);
        details.append(&widgets::heading("Details"));
        let mut facts: Vec<(&str, String)> = Vec::new();
        if let Some(studio) = &entry.studio {
            facts.push(("Studio", studio.clone()));
        }
        if !entry.genres.is_empty() {
            facts.push(("Genres", entry.genres.join(", ")));
        }
        if let Some(source) = entry.source {
            facts.push(("Source", spaced(&format!("{source:?}"))));
        }
        if let Some(status) = entry.status {
            facts.push(("Status", spaced(&format!("{status:?}"))));
        }
        for (name, value) in facts {
            let line = gtk::Box::new(gtk::Orientation::Vertical, 0);
            line.append(&widgets::haze(name, "caption"));
            let value = gtk::Label::new(Some(&value));
            value.set_xalign(0.0);
            value.set_wrap(true);
            line.append(&value);
            details.append(&line);
        }
        self.side.append(&details);

        let related: Vec<LinkedTitle> = entry
            .detail
            .as_ref()
            .map(|d| {
                d.relations
                    .iter()
                    .filter(|r| {
                        !matches!(
                            r.relation,
                            Some(
                                RelationKind::Adaptation
                                    | RelationKind::Source
                                    | RelationKind::Character
                            )
                        )
                    })
                    .take(8)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if related.is_empty() {
            return;
        }
        let box_ = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(if size == Size::Wide { 3 } else { 4 })
            .max_children_per_line(if size == Size::Wide { 3 } else { 8 })
            .column_spacing(8)
            .row_spacing(8)
            .halign(gtk::Align::Start)
            .build();
        for linked in &related {
            let mut shown = Entry::new(linked.id, linked.title.clone());
            shown.cover_url.clone_from(&linked.cover_url);
            shown.cover_color.clone_from(&linked.cover_color);
            let tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
            tile.append(&widgets::cover(&self.ctx.hub, &shown, 76, 114));
            let label = widgets::haze(
                &linked
                    .relation
                    .map_or_else(|| linked.title.clone(), |r| spaced(&format!("{r:?}"))),
                "caption",
            );
            label.set_wrap(false);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(10);
            tile.append(&label);
            tile.set_tooltip_text(Some(&linked.title));
            let open = gtk::GestureClick::new();
            let id = linked.id.get();
            open.connect_released({
                let tile = tile.clone();
                move |_, _, _, _| {
                    let _ = tile.activate_action("win.open-title", Some(&id.to_variant()));
                }
            });
            tile.add_controller(open);
            tile.set_cursor_from_name(Some("pointer"));
            widgets::title_menu(&tile, id, linked.list_status.is_none());
            box_.append(&tile);
        }
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.append(&widgets::heading("Related"));
        column.append(&box_);
        self.side.append(&column);
    }

    fn draw_episodes(&self) {
        clear_list(&self.episodes);
        let rows = self.rows.borrow();
        if rows.is_empty() {
            self.episodes_state.set_visible_child_name(
                if self.ctx.hub.entry(self.id).is_some_and(|e| e.detail.is_some()) {
                    "empty"
                } else {
                    "loading"
                },
            );
            return;
        }
        self.episodes_state.set_visible_child_name("rows");
        let narrow = *self.sized.borrow() == Size::Narrow;
        let w = if narrow { 96 } else { 128 };
        let entry = self.ctx.hub.entry(self.id);
        let next = entry.as_ref().and_then(|e| e.progress).map_or(1, |p| p.next);
        let spoilers = self
            .ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::SpoilerProtection)
            .is_some_and(|r| catalog::is_on(&r));
        let id = self.id.get();
        for row in rows.iter() {
            let number: u32 = row.number.parse().unwrap_or(0);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.add_css_class("episode-row");
            line.set_size_request(-1, if narrow { 54 } else { 72 });

            let (still, frame) = widgets::still(
                &self.ctx.hub,
                row.thumbnail.as_deref(),
                entry.as_ref().and_then(|e| e.cover_color.as_deref()),
                w,
            );
            if spoilers && !row.completed && number > next {
                frame.add_css_class("spoiler");
            }
            line.append(&still);

            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_valign(gtk::Align::Center);
            text.set_hexpand(true);
            let name = match &row.title {
                Some(t) if !t.is_empty() => format!("{} · {t}", row.number),
                _ => format!("Episode {}", row.number),
            };
            let label = gtk::Label::new(Some(&name));
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            if row.completed {
                label.add_css_class("haze");
            }
            text.append(&label);
            let part_way = row.watched > 0.0 && !row.completed;
            let sub = match (part_way, row.duration_secs) {
                (true, Some(d)) => {
                    let left = (d as f64 * (1.0 - row.watched)).max(60.0);
                    format!("{} min left", (left / 60.0).round() as u64)
                }
                (false, Some(d)) => words::minutes(d),
                _ if row.metadata_only => "Not out yet".to_owned(),
                _ => String::new(),
            };
            if !sub.is_empty() {
                let sub = widgets::haze(&sub, "caption");
                sub.add_css_class("numeric");
                text.append(&sub);
            }
            if part_way {
                let bar = gtk::ProgressBar::new();
                bar.set_fraction(row.watched);
                bar.set_margin_top(4);
                bar.set_size_request(120, -1);
                bar.set_halign(gtk::Align::Start);
                text.append(&bar);
            }
            line.append(&text);

            let check = gtk::CheckButton::new();
            check.set_active(row.completed);
            check.set_valign(gtk::Align::Center);
            check.set_tooltip_text(Some(if row.completed {
                "Mark Unwatched"
            } else {
                "Mark Watched"
            }));
            check.update_property(&[gtk::accessible::Property::Label(&format!(
                "Episode {} watched",
                row.number
            ))]);
            let episode = row.number.clone();
            check.connect_toggled(move |check| {
                let _ = check.activate_action(
                    "title.watched",
                    Some(&(episode.clone(), check.is_active()).to_variant()),
                );
            });
            line.append(&check);

            let menu = gio::Menu::new();
            let target = format!("(uint32 {id}, '{}')", row.number);
            menu.append(Some("Play"), Some(&format!("win.play({target})")));
            menu.append(Some("Mark Watched"), Some(&format!("win.mark-watched({target})")));
            let more = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .tooltip_text("Episode Menu")
                .menu_model(&menu)
                .valign(gtk::Align::Center)
                .build();
            more.add_css_class("flat");
            line.append(&more);

            let list_row =
                gtk::ListBoxRow::builder().child(&line).activatable(!row.metadata_only).build();
            list_row.set_action_name(Some("win.play"));
            list_row.set_action_target_value(Some(&(id, row.number.clone()).to_variant()));
            widgets::context_menu(&list_row, &menu);
            self.episodes.append(&list_row);
        }
    }
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn clear_list(list: &gtk::ListBox) {
    list.remove_all();
}

/// An enum's name as words: "LightNovel" is "Light novel".
fn spaced(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_names_read_as_words() {
        assert_eq!(spaced("LightNovel"), "Light novel");
        assert_eq!(spaced("Manga"), "Manga");
        assert_eq!(spaced("NotYetReleased"), "Not yet released");
    }
}
