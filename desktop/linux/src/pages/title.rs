//! A title's page: the banner, then the cover beside the show title, the metadata line, the one
//! amber action and the status and More menus; under them the season picker and the episodes,
//! and the details and Related in a side column. One column below the wide size, and on narrow
//! windows no cover beside the title, the action at full width and smaller stills.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{
    catalog::{Entry, LibrarySegment, LinkedTitle, RelationKind},
    command::Command,
    episodes::EpisodeRow,
    event::Event,
    settings::SettingKey,
};
use gtk::{gio, glib};

use crate::{
    catalog, choices, motion,
    states::{Shape, States},
    widgets,
    window::{Ctx, Size},
    words,
};

/// How far a watched episode's still dims.
const WATCHED: f64 = 0.55;

/// An episode row as drawn, kept so a mark changes it in place: focus stays where it was, and
/// the still dims on the quick token.
struct EpisodeLine {
    number: String,
    check: gtk::CheckButton,
    toggled: glib::SignalHandlerId,
    name: gtk::Label,
    still: gtk::Widget,
    frame: gtk::Box,
    /// The row's content, dimmed when no source lists the episode.
    root: gtk::Box,
    sub: gtk::Label,
    bar: gtk::ProgressBar,
    menu: gio::Menu,
    shown: RefCell<EpisodeRow>,
}

struct Page {
    ctx: Rc<Ctx>,
    id: AnilistId,
    navigation: glib::WeakRef<adw::NavigationPage>,
    banner: gtk::Box,
    banner_filled: Cell<bool>,
    head: gtk::Box,
    side: gtk::Box,
    seasons: gtk::Box,
    synopsis: gtk::Box,
    episodes: gtk::ListBox,
    episodes_state: Rc<States>,
    lines: RefCell<Vec<EpisodeLine>>,
    /// Whether the engine has answered for the episodes, so no table means none.
    listed: Cell<bool>,
    columns: gtk::Box,
    rows: Rc<RefCell<Vec<EpisodeRow>>>,
    /// The page's status action, its state the list the title is on.
    status: gio::SimpleAction,
    /// How far the dub goes, from the source a play would use: "Dub to episode 6".
    dubbed_to: RefCell<Option<String>>,
    listening: RefCell<Option<u64>>,
    sized: RefCell<Size>,
    /// The page's own heading, and the header bar that shows the title once it scrolls away.
    heading: RefCell<Option<gtk::Label>>,
    header: adw::HeaderBar,
    /// Play on TV, at the header's end: the episode the primary action plays.
    tv_button: gtk::Button,
    content: gtk::Box,
}

pub fn page(ctx: &Rc<Ctx>, id: AnilistId) -> adw::NavigationPage {
    let entry = ctx.hub.entry(id);
    // Nothing in the header until the title is known, rather than a word standing in for it.
    let title = entry.as_ref().map_or("", |e| e.title.as_str()).to_owned();

    let head = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let seasons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let synopsis = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let episodes = gtk::ListBox::new();
    episodes.add_css_class("boxed-list");
    episodes.set_selection_mode(gtk::SelectionMode::None);
    let none = widgets::haze("No episodes are listed for this title yet.", "");
    none.set_valign(gtk::Align::Start);
    let shape = Shape::Rows { count: 4, width: 128, height: 72 };
    let episodes_state =
        States::new(&ctx.hub, &episodes, shape, &none, "Couldn't List Episodes");
    episodes_state.widget.set_vhomogeneous(false);
    episodes_state.widget.set_vexpand(false);
    episodes_state.loading(Command::LoadEpisodes(id).activity());
    episodes_state.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        let state = Rc::downgrade(&episodes_state);
        move || {
            if let (Some(hub), Some(state)) = (hub.upgrade(), state.upgrade()) {
                state.loading(Command::LoadEpisodes(id).activity());
                hub.send(Command::LoadEpisodes(id));
            }
        }
    });

    let main = gtk::Box::new(gtk::Orientation::Vertical, 12);
    main.set_hexpand(true);
    main.append(&synopsis);
    main.append(&seasons);
    let heading = widgets::heading("Episodes");
    heading.set_margin_top(12);
    main.append(&heading);
    main.append(&episodes_state.widget);

    let side = gtk::Box::new(gtk::Orientation::Vertical, 18);
    side.set_size_request(260, -1);
    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 36);
    columns.append(&main);
    columns.append(&side);
    columns.set_margin_start(36);
    columns.set_margin_end(36);
    columns.set_margin_top(18);
    columns.set_margin_bottom(36);

    // The banner, up to 360 high, with the cover and the title over its lower part, on the
    // ground's fade: both in one cell of a grid, so the head may run past the banner.
    let banner_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    banner_slot.set_valign(gtk::Align::Start);
    head.set_valign(gtk::Align::End);
    head.set_margin_top(120);
    head.set_margin_start(36);
    head.set_margin_end(36);
    let head_clamp = adw::Clamp::builder()
        .maximum_size(1200)
        .tightening_threshold(1200)
        .child(&head)
        .build();
    let top = gtk::Grid::new();
    top.attach(&banner_slot, 0, 0, 1, 1);
    top.attach(&head_clamp, 0, 0, 1, 1);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&top);
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
    // The heading says the title; the header bar says it only once the heading has gone.
    header.set_show_title(false);
    let tv_button = gtk::Button::from_icon_name("video-display-symbolic");
    tv_button.set_tooltip_text(Some("Play on TV"));
    tv_button.update_property(&[gtk::accessible::Property::Description(
        "Play this episode on a TV with anistream",
    )]);
    tv_button.set_action_name(Some("win.play-on-tv"));
    tv_button.set_visible(false);
    header.pack_end(&tv_button);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.add_top_bar(&widgets::offline_banner());
    view.set_extend_content_to_top_edge(true);
    view.set_content(Some(&scroll));
    let navigation = adw::NavigationPage::builder().title(&title).child(&view).build();

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        id,
        navigation: navigation.downgrade(),
        banner: banner_slot,
        banner_filled: Cell::new(false),
        head,
        side,
        seasons,
        synopsis,
        episodes,
        episodes_state,
        lines: RefCell::default(),
        listed: Cell::new(false),
        columns,
        rows: Rc::default(),
        listening: RefCell::default(),
        sized: RefCell::new(ctx.size()),
        status: gio::SimpleAction::new_stateful(
            "status",
            Some(glib::VariantTy::STRING),
            &String::new().to_variant(),
        ),
        dubbed_to: RefCell::default(),
        heading: RefCell::default(),
        header,
        tv_button,
        content: content.clone(),
    });
    scroll.vadjustment().connect_value_changed({
        let weak = Rc::downgrade(&page);
        move |adjustment| {
            if let Some(page) = weak.upgrade() {
                page.title_shown(adjustment.value());
            }
        }
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
    ctx.hub.known.borrow_mut().visit(id);
    if let Some(entry) = &entry {
        page.fill(entry);
    }
    ctx.hub.send(Command::LoadDetail(id));
    ctx.hub.send(Command::LoadEpisodes(id));
    ctx.hub.send(Command::LoadDub(id));
    // Kept as long as the page it draws, and deaf once that is gone.
    navigation.connect_destroy(move |_| {
        if let Some(id) = page.listening.borrow_mut().take() {
            page.ctx.hub.forget(id);
        }
    });
    navigation
}

impl Page {
    /// The header bar's title, shown once the page's heading has scrolled under it.
    fn title_shown(&self, scrolled: f64) {
        let gone = self.heading.borrow().as_ref().and_then(|heading| {
            let bottom = gtk::graphene::Point::new(0.0, heading.height() as f32);
            heading.compute_point(&self.content, &bottom)
        });
        // The header bar stands over the banner: about 47 pixels.
        let shown = gone.is_some_and(|point| scrolled > f64::from(point.y()) - 47.0);
        if self.header.shows_title() != shown {
            self.header.set_show_title(shown);
        }
    }

    fn heard(&self, event: &Event) {
        match event {
            // Titles are named another way now: this one and its related titles again.
            Event::TitlesChanged(_) => self.ctx.hub.send(Command::LoadDetail(self.id)),
            Event::Detail(entry) if entry.id == self.id => {
                if let Some(navigation) = self.navigation.upgrade() {
                    navigation.set_title(&entry.title);
                }
                self.fill(entry);
            }
            Event::Episodes { id, rows } if *id == self.id => {
                self.listed.set(true);
                self.rows.replace(rows.clone());
                self.draw_episodes();
                // The table decides what More offers.
                if let Some(entry) = self.ctx.hub.entry(self.id) {
                    self.fill(&entry);
                }
            }
            // No table until a match is picked.
            Event::MatchChoices { id, .. } if *id == self.id => {
                self.listed.set(true);
                self.episodes_state.empty();
            }
            // The source pinned for this title is off, gone or resting, so its episodes come
            // from the others. Said once, until a source is chosen again.
            Event::PinUnavailable { id, provider_id, name } if *id == self.id => {
                let new = self.ctx.hub.known.borrow_mut().say_pin(*id, provider_id);
                if new {
                    self.ctx.toast(&t::source_pin_unavailable(name));
                }
            }
            Event::TitleDub { id, dub } if *id == self.id => {
                self.dubbed_to.replace(anistream_model::present::dub_line(dub.as_ref()));
                if let Some(entry) = self.ctx.hub.entry(self.id) {
                    self.fill(&entry);
                }
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
                self.update_episodes();
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
        self.head.set_margin_start(margin);
        self.head.set_margin_end(margin);
        if let Some(entry) = self.ctx.hub.entry(self.id) {
            self.fill(&entry);
        }
        self.draw_episodes();
    }

    /// The page's own actions: the status menu and More.
    fn actions(&self) {
        let group = gio::SimpleActionGroup::new();
        let status = &self.status;
        let hub = Rc::downgrade(&self.ctx.hub);
        let id = self.id;
        status.connect_activate({
            let hub = hub.clone();
            move |action, arg| {
                let Some(hub) = hub.upgrade() else { return };
                let Some(name) = arg.and_then(String::from_variant) else { return };
                if let Some(status) =
                    LibrarySegment::ALL.into_iter().find(|s| s.label() == name)
                {
                    action.set_state(&name.to_variant());
                    hub.send(Command::SetStatus { id, status });
                    hub.send(Command::LoadDetail(id));
                }
            }
        });
        group.add_action(status);
        let trailer = gio::SimpleAction::new("trailer", None);
        trailer.connect_activate({
            let hub = hub.clone();
            move |_, _| {
                let Some(hub) = hub.upgrade() else { return };
                let Some(entry) = hub.entry(id) else { return };
                if let Some(url) = entry.trailer_url.clone() {
                    hub.send(Command::PlayTrailer { url, title: entry.title });
                }
            }
        });
        group.add_action(&trailer);
        let download = gio::SimpleAction::new("download-unwatched", None);
        download.connect_activate({
            let hub = hub.clone();
            let rows = Rc::clone(&self.rows);
            move |_, _| {
                let Some(hub) = hub.upgrade() else { return };
                let episodes = unwatched(&rows.borrow());
                if !episodes.is_empty() {
                    hub.send(Command::DownloadMany { id, episodes });
                }
            }
        });
        group.add_action(&download);
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
        show_settings.connect_activate({
            let ctx = Rc::downgrade(&self.ctx);
            move |_, _| {
                let Some(ctx) = ctx.upgrade() else { return };
                let title = ctx.hub.entry(id).map(|e| e.title).unwrap_or_default();
                crate::pages::show_settings::open(&ctx, id, &title);
            }
        });
        group.add_action(&show_settings);
        let source = gio::SimpleAction::new("source-picker", None);
        source.connect_activate({
            let ctx = Rc::downgrade(&self.ctx);
            move |_, _| {
                let Some(ctx) = ctx.upgrade() else { return };
                let _ = crate::pages::source_picker::open(&ctx, &ctx.window, id);
            }
        });
        group.add_action(&source);
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
            self.banner.append(&widgets::faded_banner(hub, entry, 360));
        }
        let id = entry.id.get();

        let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
        text.set_valign(gtk::Align::End);
        text.set_hexpand(true);
        let title = widgets::show_title(&entry.title, "page");
        text.append(&title);
        self.heading.replace(Some(title.clone()));
        if let Some(other) = entry.secondary.as_deref().or(entry.title_native.as_deref())
            && other != entry.title
        {
            text.append(&widgets::haze(other, ""));
        }
        text.append(&widgets::haze(&words::meta_line(entry), "numeric"));
        if let Some(line) = self.dubbed_to.borrow().as_deref() {
            text.append(&widgets::haze(line, "numeric"));
        }

        let item = {
            let rows = self.rows.borrow();
            words::queue_item(entry, (!rows.is_empty()).then_some(rows.as_slice()))
        };
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_margin_top(10);
        match item.button.clone() {
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
                if let Some(waiting) = item.waiting.clone() {
                    actions.append(&widgets::haze(&waiting, ""));
                }
            }
        }
        // A choice each, checked where the title is.
        self.status.set_state(
            &entry.list_status.map_or_else(String::new, |s| s.label().to_owned()).to_variant(),
        );
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
        let first = gio::Menu::new();
        let (playable, unsourced) = {
            let rows = self.rows.borrow();
            (
                !rows.is_empty() && !rows.iter().any(|r| r.metadata_only),
                choices::next(entry, Some(rows.as_slice())).unsourced,
            )
        };
        let on_tv = playable && item.button.is_some() && self.ctx.tv.available();
        self.tv_button.set_visible(on_tv);
        if on_tv {
            let from = item.resume.as_ref().map_or(0.0, |r| r.position);
            self.tv_button.set_action_target_value(Some(
                &(id, item.episode.to_string(), from).to_variant(),
            ));
        }
        // A party plays the episode the primary action would.
        if item.button.is_some() {
            first.append(
                Some("Watch Together…"),
                Some(&format!("win.watch-together((uint32 {id}, '{}'))", item.episode)),
            );
        }
        if playable {
            if !unsourced {
                first.append(
                    Some(&format!("Download Episode {}", item.episode)),
                    Some(&format!("win.download((uint32 {id}, '{}'))", item.episode)),
                );
            }
            if unwatched(&self.rows.borrow()).len() > 1 {
                first.append(
                    Some("Download Unwatched Episodes"),
                    Some("title.download-unwatched"),
                );
            }
        }
        first.append(Some("Mark All Previous Watched"), Some("title.mark-previous"));
        more_menu.append_section(None, &first);
        let second = gio::Menu::new();
        if entry.trailer_url.is_some() {
            second.append(Some("Play Trailer"), Some("title.trailer"));
        }
        second.append(Some("Open on AniList"), Some(&format!("win.open-anilist(uint32 {id})")));
        more_menu.append_section(None, &second);
        let third = gio::Menu::new();
        third.append(Some(&t::source_picker_title()), Some("title.source-picker"));
        third.append(Some("Show Settings…"), Some("title.show-settings"));
        more_menu.append_section(None, &third);
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

        clear(&self.synopsis);
        if !entry.synopsis.is_empty() {
            let synopsis = gtk::Label::new(Some(&entry.synopsis));
            synopsis.set_wrap(true);
            synopsis.set_xalign(0.0);
            synopsis.set_margin_top(6);
            synopsis.set_max_width_chars(90);
            self.synopsis.append(&synopsis);
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
            let id = linked.id.get();
            let button = gtk::Button::builder().child(&tile).build();
            button.add_css_class("flat");
            button.add_css_class("related");
            button.set_tooltip_text(Some(&linked.title));
            button.set_action_name(Some("win.open-title"));
            button.set_action_target_value(Some(&id.to_variant()));
            let relation = linked.relation.map(|r| spaced(&format!("{r:?}")));
            button.update_property(&[gtk::accessible::Property::Label(&match relation {
                Some(relation) => format!("{}, {relation}", linked.title),
                None => linked.title.clone(),
            })]);
            widgets::title_menu(&button, id, linked.list_status.is_none());
            box_.append(&button);
        }
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.append(&widgets::heading("Related"));
        column.append(&box_);
        self.side.append(&column);
    }

    /// The episode table, drawn whole: when it arrives and when the window changes size.
    fn draw_episodes(&self) {
        clear_list(&self.episodes);
        self.lines.borrow_mut().clear();
        let rows = self.rows.borrow();
        if rows.is_empty() {
            if self.listed.get() {
                self.episodes_state.empty();
            }
            return;
        }
        let narrow = *self.sized.borrow() == Size::Narrow;
        let w = if narrow { 96 } else { 128 };
        let entry = self.ctx.hub.entry(self.id);
        let color = entry.as_ref().and_then(|e| e.cover_color.clone());
        let id = self.id.get();
        let mut lines = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.add_css_class("episode-row");
            line.set_size_request(-1, if narrow { 54 } else { 72 });

            let (still, frame) =
                widgets::still(&self.ctx.hub, row.thumbnail.as_deref(), color.as_deref(), w);
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
            text.append(&label);
            let sub = widgets::haze("", "caption");
            sub.add_css_class("numeric");
            text.append(&sub);
            let bar = gtk::ProgressBar::new();
            bar.set_margin_top(4);
            bar.set_size_request(120, -1);
            bar.set_halign(gtk::Align::Start);
            text.append(&bar);
            line.append(&text);

            let check = gtk::CheckButton::new();
            check.set_valign(gtk::Align::Center);
            check.update_property(&[gtk::accessible::Property::Label(&format!(
                "Episode {} watched",
                row.number
            ))]);
            let episode = row.number.clone();
            let toggled = check.connect_toggled(move |check| {
                let _ = check.activate_action(
                    "title.watched",
                    Some(&(episode.clone(), check.is_active()).to_variant()),
                );
            });
            line.append(&check);

            let menu = gio::Menu::new();
            let more = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .tooltip_text("Episode Menu")
                .menu_model(&menu)
                .valign(gtk::Align::Center)
                .build();
            more.add_css_class("flat");
            line.append(&more);

            let list_row = gtk::ListBoxRow::builder()
                .child(&line)
                .activatable(choices::playable(row))
                .build();
            list_row.set_action_name(Some("win.play"));
            list_row.set_action_target_value(Some(&(id, row.number.clone()).to_variant()));
            widgets::context_menu(&list_row, &menu);
            self.episodes.append(&list_row);

            let drawn = EpisodeLine {
                number: row.number.clone(),
                check,
                toggled,
                name: label,
                still: still.upcast(),
                frame,
                root: line,
                sub,
                bar,
                menu,
                shown: RefCell::new(row.clone()),
            };
            self.dress(&drawn, row, false);
            lines.push(drawn);
        }
        drop(rows);
        self.lines.replace(lines);
        self.episodes_state.content();
    }

    /// Marks and progress moved: each row changed in place.
    fn update_episodes(&self) {
        if self.lines.borrow().len() != self.rows.borrow().len() {
            self.draw_episodes();
            return;
        }
        let rows = self.rows.borrow();
        for line in self.lines.borrow().iter() {
            if let Some(row) = rows.iter().find(|r| r.number == line.number)
                && *line.shown.borrow() != *row
            {
                self.dress(line, row, true);
                line.shown.replace(row.clone());
            }
        }
    }

    /// One row as `row` stands: its check, its line, its still and its menu.
    fn dress(&self, line: &EpisodeLine, row: &EpisodeRow, animate: bool) {
        let entry = self.ctx.hub.entry(self.id);
        let next = entry.as_ref().and_then(|e| e.progress).map_or(1, |p| p.next);
        let number: u32 = row.number.parse().unwrap_or(0);
        let spoilers = self
            .ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::SpoilerProtection)
            .is_some_and(|r| catalog::is_on(&r));
        if spoilers && !row.completed && number > next {
            line.frame.add_css_class("spoiler");
        } else {
            line.frame.remove_css_class("spoiler");
        }

        line.check.block_signal(&line.toggled);
        line.check.set_active(row.completed);
        line.check.unblock_signal(&line.toggled);
        line.check.set_tooltip_text(Some(if row.completed {
            "Mark Unwatched"
        } else {
            "Mark Watched"
        }));
        if row.completed {
            line.name.add_css_class("haze");
        } else {
            line.name.remove_css_class("haze");
        }
        let to = if row.completed { WATCHED } else { 1.0 };
        let from = line.still.opacity();
        if animate && (from - to).abs() > f64::EPSILON {
            motion::quick(&line.still, from, to, motion::opacity(&line.still)).play();
        } else {
            line.still.set_opacity(to);
        }

        // An episode no source lists is dimmed and says so; it cannot be played or downloaded.
        if row.unsourced {
            line.root.add_css_class("dim-label");
        } else {
            line.root.remove_css_class("dim-label");
        }
        let part_way = row.watched > 0.0 && !row.completed;
        let sub = match (part_way, row.duration_secs) {
            _ if row.unsourced => t::episode_unsourced(),
            (true, Some(d)) => {
                let left = (d as f64 * (1.0 - row.watched)).max(60.0);
                format!("{} min left", (left / 60.0).round() as u64)
            }
            (false, Some(d)) => words::minutes(d),
            _ if row.metadata_only => "Not out yet".to_owned(),
            _ => String::new(),
        };
        line.sub.set_label(&sub);
        line.sub.set_visible(!sub.is_empty());
        line.bar.set_fraction(row.watched);
        line.bar.set_visible(part_way);

        let id = self.id.get();
        let n = &row.number;
        let target = format!("(uint32 {id}, '{n}')");
        line.menu.remove_all();
        let first = gio::Menu::new();
        if choices::playable(row) {
            let verb = if part_way { "Resume" } else { "Play" };
            first.append(
                Some(&format!("{verb} Episode {n}")),
                Some(&format!("win.play({target})")),
            );
            if part_way {
                first.append(
                    Some("Play from the Beginning"),
                    Some(&format!("win.play-from-start({target})")),
                );
            }
            if self.ctx.tv.available() {
                // Resumed on the TV where it was left here.
                let from = match (part_way, row.duration_secs) {
                    (true, Some(d)) => d as f64 * row.watched,
                    _ => 0.0,
                };
                first.append(
                    Some("Play on TV…"),
                    Some(&format!("win.play-on-tv((uint32 {id}, '{n}', {from:.1}))")),
                );
            }
        }
        line.menu.append_section(None, &first);
        let second = gio::Menu::new();
        if row.completed {
            second.append(
                Some(&format!("Mark Episode {n} as Unwatched")),
                Some(&format!("win.mark-unwatched({target})")),
            );
        } else {
            second.append(
                Some(&format!("Mark Episode {n} as Watched")),
                Some(&format!("win.mark-watched({target})")),
            );
        }
        line.menu.append_section(None, &second);
        if choices::playable(row) {
            let third = gio::Menu::new();
            third.append(
                Some(&format!("Download Episode {n}")),
                Some(&format!("win.download({target})")),
            );
            line.menu.append_section(None, &third);
        }
    }
}

/// The episodes Download Unwatched Episodes queues: each one a source lists, out, and not
/// watched.
fn unwatched(rows: &[EpisodeRow]) -> Vec<String> {
    let now = glib::DateTime::now_utc().map_or(0, |t| t.to_unix());
    rows.iter()
        .filter(|r| {
            choices::playable(r) && !r.completed && r.airs_at.is_none_or(|at| at <= now)
        })
        .map(|r| r.number.clone())
        .collect()
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
    fn a_range_download_skips_the_episodes_no_source_lists() {
        let row = |number: &str, unsourced: bool, metadata_only: bool| EpisodeRow {
            number: number.into(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at: None,
            delayed_from: None,
            metadata_only,
            unsourced,
            source: None,
        };
        let rows = [
            row("1", false, false),
            row("2", true, false),
            row("3", false, false),
            row("4", false, true),
        ];
        assert_eq!(unwatched(&rows), ["1", "3"]);
    }

    #[test]
    fn enum_names_read_as_words() {
        assert_eq!(spaced("LightNovel"), "Light novel");
        assert_eq!(spaced("Manga"), "Manga");
        assert_eq!(spaced("NotYetReleased"), "Not yet released");
    }
}
