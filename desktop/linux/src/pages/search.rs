//! Search: a sidebar destination and the header bar's search button. Typing anywhere in the
//! window opens the search bar and searches. Before typing, recent searches, This season and the
//! genres; then the poster grid with its filters as a row of drop-downs, and the Settings rows
//! whose names match under "Settings".

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_model::{
    catalog::{AiringStatus, ContentFor, ContentList, Season},
    command::Command,
    event::Event,
    search::{MediaFormat, SearchQuery, SearchSort},
};

use crate::{
    catalog, nagare,
    widgets::{self, PosterGrid},
    window::Ctx,
};

/// AniList's genres, as its search takes them.
const GENRES: [&str; 16] = [
    "Action",
    "Adventure",
    "Comedy",
    "Drama",
    "Fantasy",
    "Horror",
    "Mecha",
    "Music",
    "Mystery",
    "Psychological",
    "Romance",
    "Sci-Fi",
    "Slice of Life",
    "Sports",
    "Supernatural",
    "Thriller",
];

const FORMATS: [(&str, Option<MediaFormat>); 6] = [
    ("Any format", None),
    ("TV", Some(MediaFormat::Tv)),
    ("Movie", Some(MediaFormat::Movie)),
    ("ONA", Some(MediaFormat::Ona)),
    ("OVA", Some(MediaFormat::Ova)),
    ("Special", Some(MediaFormat::Special)),
];
const SEASONS: [(&str, Option<Season>); 5] = [
    ("Any season", None),
    ("Winter", Some(Season::Winter)),
    ("Spring", Some(Season::Spring)),
    ("Summer", Some(Season::Summer)),
    ("Fall", Some(Season::Fall)),
];
const STATUSES: [(&str, Option<AiringStatus>); 4] = [
    ("Any status", None),
    ("Airing", Some(AiringStatus::Releasing)),
    ("Finished", Some(AiringStatus::Finished)),
    ("Not yet aired", Some(AiringStatus::NotYetReleased)),
];
const SORTS: [(&str, Option<SearchSort>); 4] = [
    ("Best match", None),
    ("Popular", Some(SearchSort::Popularity)),
    ("Top rated", Some(SearchSort::Score)),
    ("Newest", Some(SearchSort::Newest)),
];

/// A drop-down of these options' names.
fn drop_down<T>(options: &[(&str, T)]) -> gtk::DropDown {
    let names: Vec<&str> = options.iter().map(|(n, _)| *n).collect();
    let drop = gtk::DropDown::from_strings(&names);
    drop.add_css_class("flat");
    drop
}

/// How many recent searches the page keeps.
const RECENT: usize = 6;

/// The Search destination, and the window's hold on its search bar.
pub struct SearchPage {
    pub page: adw::NavigationPage,
    inner: Rc<Inner>,
}

struct Inner {
    ctx: Rc<Ctx>,
    bar: gtk::SearchBar,
    entry: gtk::SearchEntry,
    body: gtk::Stack,
    grid: PosterGrid,
    season: PosterGrid,
    settings: gtk::ListBox,
    settings_group: gtk::Box,
    recent: gtk::FlowBox,
    recent_group: gtk::Box,
    empty: adw::StatusPage,
    filters: [gtk::DropDown; 4],
    genre: RefCell<Option<String>>,
    asked: RefCell<Option<SearchQuery>>,
    searches: RefCell<Vec<String>>,
    capture: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> SearchPage {
    let entry = gtk::SearchEntry::builder()
        .placeholder_text("Search titles and settings")
        .hexpand(true)
        .build();
    let clamp = adw::Clamp::builder().maximum_size(480).child(&entry).build();
    let bar = gtk::SearchBar::builder().child(&clamp).show_close_button(false).build();
    bar.connect_entry(&entry);

    let toggle = gtk::ToggleButton::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text("Search")
        .build();
    toggle
        .bind_property("active", &bar, "search-mode-enabled")
        .sync_create()
        .bidirectional()
        .build();
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_end(&toggle);

    let filters =
        [drop_down(&FORMATS), drop_down(&SEASONS), drop_down(&STATUSES), drop_down(&SORTS)];
    let filter_row = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(5)
        .column_spacing(6)
        .row_spacing(6)
        .homogeneous(false)
        .build();
    for filter in &filters {
        filter_row.append(filter);
    }
    filter_row.set_margin_start(12);
    filter_row.set_margin_end(12);

    // Before typing.
    let recent = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(6)
        .column_spacing(6)
        .row_spacing(6)
        .build();
    let recent_group = section("Recent Searches", &recent);
    recent_group.set_visible(false);
    let clear = gtk::Button::with_label("Clear");
    clear.add_css_class("flat");
    clear.set_halign(gtk::Align::End);
    clear.set_tooltip_text(Some("Clear Recent Searches"));
    if let Some(heading) = recent_group.first_child() {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        recent_group.remove(&heading);
        heading.set_hexpand(true);
        line.append(&heading);
        line.append(&clear);
        recent_group.prepend(&line);
    }
    let season = PosterGrid::new(&ctx.hub);
    season.widget.set_vexpand(false);
    let genres = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(8)
        .column_spacing(8)
        .row_spacing(8)
        .homogeneous(true)
        .build();
    for genre in GENRES {
        let tile = gtk::Button::with_label(genre);
        tile.add_css_class("genre-tile");
        tile.set_widget_name(genre);
        genres.append(&tile);
    }
    let before = gtk::Box::new(gtk::Orientation::Vertical, 18);
    before.set_margin_top(12);
    before.set_margin_bottom(24);
    before.append(&recent_group);
    before.append(&section("This Season", &season.widget));
    before.append(&section("Genres", &genres));
    let before = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&before)
        .build();

    // Results.
    let grid = PosterGrid::new(&ctx.hub);
    grid.widget.set_vexpand(false);
    let settings = gtk::ListBox::new();
    settings.add_css_class("boxed-list");
    settings.set_selection_mode(gtk::SelectionMode::None);
    let settings_group = section("Settings", &settings);
    let results = gtk::Box::new(gtk::Orientation::Vertical, 12);
    results.set_margin_bottom(24);
    results.append(&grid.widget);
    results.append(&settings_group);
    let results = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&results)
        .build();

    let empty = nagare::status_page("Nothing Found", "", None);
    let loading = adw::Spinner::new();
    loading.set_size_request(32, 32);
    loading.set_halign(gtk::Align::Center);
    loading.set_valign(gtk::Align::Center);
    let body = gtk::Stack::new();
    body.set_vexpand(true);
    body.add_named(&before, Some("before"));
    body.add_named(&loading, Some("loading"));
    body.add_named(&results, Some("results"));
    body.add_named(&empty, Some("empty"));

    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.append(&bar);
    column.append(&filter_row);
    column.append(&body);
    body.connect_visible_child_name_notify({
        let filter_row = filter_row.clone();
        move |body| {
            filter_row.set_visible(body.visible_child_name().as_deref() != Some("before"));
        }
    });
    filter_row.set_visible(false);

    let inner = Rc::new(Inner {
        ctx: Rc::clone(ctx),
        bar: bar.clone(),
        entry: entry.clone(),
        body,
        grid,
        season,
        settings,
        settings_group,
        recent,
        recent_group,
        empty,
        filters,
        genre: RefCell::default(),
        asked: RefCell::default(),
        searches: RefCell::new(ctx.prefs.recent_searches()),
        capture: Cell::new(false),
    });

    inner.show_recent();
    clear.connect_clicked({
        let inner = Rc::downgrade(&inner);
        move |_| {
            if let Some(inner) = inner.upgrade() {
                inner.forget_searches();
            }
        }
    });
    entry.connect_search_changed({
        let inner = Rc::downgrade(&inner);
        move |_| {
            if let Some(inner) = inner.upgrade() {
                inner.search();
            }
        }
    });
    entry.connect_activate({
        let inner = Rc::downgrade(&inner);
        move |entry| {
            if let Some(inner) = inner.upgrade() {
                inner.remember(entry.text().trim());
                inner.search();
            }
        }
    });
    for filter in &inner.filters {
        filter.connect_selected_notify({
            let inner = Rc::downgrade(&inner);
            move |_| {
                if let Some(inner) = inner.upgrade() {
                    inner.search();
                }
            }
        });
    }
    let mut tile = genres.first_child();
    while let Some(child) = tile {
        if let Some(button) = child.first_child().and_downcast::<gtk::Button>() {
            button.connect_clicked({
                let inner = Rc::downgrade(&inner);
                move |button| {
                    if let Some(inner) = inner.upgrade() {
                        inner.genre.replace(Some(button.widget_name().to_string()));
                        inner.search();
                    }
                }
            });
        }
        tile = child.next_sibling();
    }
    inner.settings.connect_row_activated(|list, row| {
        let pane = row.widget_name();
        let _ = list.activate_action("win.settings-pane", Some(&pane.to_variant()));
    });
    ctx.hub.listen({
        let inner = Rc::downgrade(&inner);
        move |event| {
            if let Some(inner) = inner.upgrade() {
                inner.heard(event);
            }
        }
    });
    // Typing elsewhere opens Search before it searches.
    bar.connect_search_mode_enabled_notify({
        let inner = Rc::downgrade(&inner);
        move |bar| {
            let Some(inner) = inner.upgrade() else { return };
            if bar.is_search_mode() && inner.capture.get() && !inner.on_screen() {
                let _ = WidgetExt::activate_action(
                    &inner.ctx.window,
                    "win.go",
                    Some(&"search".to_variant()),
                );
            }
        }
    });
    inner.capture_keys(true);

    let page = widgets::page("Search", &header, &column);
    SearchPage { page, inner }
}

impl SearchPage {
    /// The search bar open with the caret in it, as Search in the sidebar and Ctrl+F leave it.
    pub fn focus(&self) {
        self.inner.bar.set_search_mode(true);
        self.inner.entry.grab_focus();
    }

    /// Typing anywhere in the window opens the search: on while the library shows, off while the
    /// player, which has keys of its own, does.
    pub fn capture_keys(&self, on: bool) {
        self.inner.capture_keys(on);
    }

    /// Search for `query`, as a screenshot or a deep link asks.
    pub fn search_for(&self, query: &str) {
        self.focus();
        self.inner.entry.set_text(query);
        self.inner.search();
    }
}

impl Inner {
    fn capture_keys(&self, on: bool) {
        self.capture.set(on);
        let window: gtk::Widget = self.ctx.window.clone().upcast();
        self.bar.set_key_capture_widget(if on { Some(&window) } else { None });
    }

    fn on_screen(&self) -> bool {
        self.bar.is_mapped()
    }

    fn picked<T: Copy>(drop: &gtk::DropDown, options: &[(&str, Option<T>)]) -> Option<T> {
        options.get(drop.selected() as usize)?.1
    }

    /// What is typed and chosen, the text trimmed as the engine tags its answer.
    fn query(&self) -> SearchQuery {
        let [format, season, status, sort] = &self.filters;
        SearchQuery {
            text: self.entry.text().trim().to_owned(),
            genres: self.genre.borrow().iter().cloned().collect(),
            season: Self::picked(season, &SEASONS),
            formats: Self::picked(format, &FORMATS).into_iter().collect(),
            statuses: Self::picked(status, &STATUSES).into_iter().collect(),
            sort: Self::picked(sort, &SORTS),
            ..SearchQuery::default()
        }
    }

    fn search(&self) {
        let query = self.query();
        if query.is_blank() {
            self.asked.replace(None);
            self.body.set_visible_child_name("before");
            return;
        }
        self.match_settings(&query.text);
        if self.asked.borrow().as_ref() == Some(&query) {
            return;
        }
        self.asked.replace(Some(query.clone()));
        self.body.set_visible_child_name("loading");
        self.ctx.hub.send(Command::Search { query });
    }

    fn heard(&self, event: &Event) {
        match event {
            Event::Content { view: ContentFor::Seasonal, content } => {
                let entries = content.entries();
                self.season.set(&entries[..entries.len().min(12)]);
            }
            Event::Content { view: ContentFor::Search { query }, content } => {
                if self.asked.borrow().as_ref() != Some(query) {
                    return;
                }
                match content {
                    ContentList::Loading => self.body.set_visible_child_name("loading"),
                    ContentList::Entries(entries) => {
                        self.grid.set(entries);
                        self.grid.widget.set_visible(true);
                        self.body.set_visible_child_name("results");
                    }
                    ContentList::Empty | ContentList::Failed(_) => {
                        self.grid.set(&[]);
                        self.grid.widget.set_visible(false);
                        if self.settings_group.is_visible() {
                            self.body.set_visible_child_name("results");
                        } else {
                            let line = if query.is_browse() {
                                "Nothing found with these filters".to_owned()
                            } else {
                                format!("Nothing found for \u{201c}{}\u{201d}", query.text)
                            };
                            self.empty.set_title(&line);
                            self.empty.set_description(Some(
                                "Try another spelling, or fewer filters.",
                            ));
                            self.body.set_visible_child_name("empty");
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The Settings rows whose names match, under "Settings" at the end.
    fn match_settings(&self, query: &str) {
        while let Some(row) = self.settings.first_child() {
            self.settings.remove(&row);
        }
        let query = query.to_lowercase();
        let mut found = 0;
        if query.chars().count() >= 3 {
            let known = self.ctx.hub.known.borrow();
            let rows = known.settings.iter().flat_map(|v| v.rows.iter().chain(&v.native_rows));
            for row in rows {
                let Some(pane) = super::settings::pane_of(row.key) else { continue };
                if !row.key.label().contains(&query) {
                    continue;
                }
                let line = adw::ActionRow::builder()
                    .title(catalog::shown(row.key.label()))
                    .subtitle(format!(
                        "{} · {}",
                        super::settings::pane_title(pane),
                        catalog::shown(&row.value)
                    ))
                    .activatable(true)
                    .build();
                line.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                line.set_widget_name(pane);
                self.settings.append(&line);
                found += 1;
            }
        }
        self.settings_group.set_visible(found > 0);
    }

    fn remember(&self, query: &str) {
        if query.is_empty() {
            return;
        }
        {
            let mut searches = self.searches.borrow_mut();
            searches.retain(|s| s != query);
            searches.insert(0, query.to_owned());
            searches.truncate(RECENT);
            self.ctx.prefs.set_recent_searches(&searches);
        }
        self.show_recent();
    }

    fn forget_searches(&self) {
        self.searches.borrow_mut().clear();
        self.ctx.prefs.set_recent_searches(&[]);
        self.show_recent();
    }

    fn show_recent(&self) {
        while let Some(child) = self.recent.first_child() {
            self.recent.remove(&child);
        }
        let searches = self.searches.borrow();
        for search in searches.iter() {
            let chip = gtk::Button::with_label(search);
            chip.add_css_class("pill");
            chip.add_css_class("small");
            let entry = self.entry.clone();
            chip.connect_clicked(move |chip| entry.set_text(&chip.label().unwrap_or_default()));
            self.recent.append(&chip);
        }
        self.recent_group.set_visible(!searches.is_empty());
    }
}

/// A heading over a block of the page, at the page's margins.
fn section(title: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_start(18);
    column.set_margin_end(18);
    column.append(&widgets::heading(title));
    column.append(child);
    column
}
