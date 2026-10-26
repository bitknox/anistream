//! Search: a sidebar destination and the header bar's search button. Typing anywhere in the
//! window opens the search bar and searches. The filters stand under the bar from the start, so
//! the titles can be browsed by genre, year or format without a word typed. Before anything is
//! typed or chosen, recent searches, This season and the genres; then the poster grid, and the
//! Settings rows whose names match under "Settings".

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry},
    command::Command,
    event::Event,
    search::SearchQuery,
};

use super::search_filters::Filters;

use crate::{
    catalog,
    states::{Shape, States},
    widgets::{self, PosterGrid},
    window::Ctx,
};

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
    /// The results' loading, failed and offline states, under the filters.
    states: Rc<States>,
    grid: PosterGrid,
    /// The results on screen, every page so far.
    entries: RefCell<Vec<Entry>>,
    /// The page to ask for as the end comes near, while there is one, and whether it is asked.
    next_page: Cell<Option<u32>>,
    asking_more: Cell<bool>,
    /// Under the results while the next page is on its way.
    more_spinner: adw::Spinner,
    /// Whether searching as you type waits for Return, to stay within AniList's limits.
    paused: Cell<bool>,
    season: PosterGrid,
    settings: gtk::ListBox,
    settings_group: gtk::Box,
    recent: gtk::FlowBox,
    recent_group: gtk::Box,
    empty: adw::StatusPage,
    filters: Rc<Filters>,
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
    header.pack_start(&crate::window::tv_session_button(ctx));
    header.pack_end(&toggle);

    let filters = Filters::new();

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
    for genre in filters.genres() {
        let tile = gtk::Button::with_label(&genre);
        tile.add_css_class("genre-tile");
        tile.set_widget_name(&genre);
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
    // At the margins This Season's covers have, so a cover is one width in both.
    grid.widget.set_margin_start(18);
    grid.widget.set_margin_end(18);
    let settings = gtk::ListBox::new();
    settings.add_css_class("boxed-list");
    settings.set_selection_mode(gtk::SelectionMode::None);
    let settings_group = section("Settings", &settings);
    let results = gtk::Box::new(gtk::Orientation::Vertical, 12);
    results.set_margin_bottom(24);
    results.append(&grid.widget);
    let more_spinner = adw::Spinner::new();
    more_spinner.set_size_request(24, 24);
    more_spinner.set_halign(gtk::Align::Center);
    more_spinner.set_visible(false);
    more_spinner
        .upcast_ref::<gtk::Widget>()
        .update_property(&[gtk::accessible::Property::Label("Loading more titles")]);
    results.append(&more_spinner);
    results.append(&settings_group);
    let results = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&results)
        .build();

    let empty = crate::states::plain("Nothing Found", "");
    // With filters set, the way out, as the Mac offers it.
    let clear_filters = gtk::Button::with_label("Clear Filters");
    clear_filters.add_css_class("pill");
    clear_filters.set_halign(gtk::Align::Center);
    clear_filters.set_visible(false);
    clear_filters.connect_clicked({
        let filters = Rc::downgrade(&filters);
        move |_| {
            if let Some(filters) = filters.upgrade() {
                filters.clear();
            }
        }
    });
    empty.set_child(Some(&clear_filters));
    let states = States::new(&ctx.hub, &results, Shape::Grid, &empty, "Couldn't Search");
    states.offline_words("Search Needs a Connection", "Downloaded episodes still play.");
    let body = gtk::Stack::new();
    body.set_vexpand(true);
    body.add_named(&before, Some("before"));
    body.add_named(&states.widget, Some("results"));

    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.append(&bar);
    column.append(&filters.row);
    column.append(&filters.chips);
    column.append(&body);

    let inner = Rc::new(Inner {
        ctx: Rc::clone(ctx),
        bar: bar.clone(),
        entry: entry.clone(),
        body,
        states,
        grid,
        entries: RefCell::default(),
        next_page: Cell::new(None),
        asking_more: Cell::new(false),
        more_spinner,
        paused: Cell::new(false),
        season,
        settings,
        settings_group,
        recent,
        recent_group,
        empty,
        filters: Rc::clone(&filters),
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
                inner.search(true);
            }
        }
    });
    entry.connect_activate({
        let inner = Rc::downgrade(&inner);
        move |entry| {
            if let Some(inner) = inner.upgrade() {
                inner.remember(entry.text().trim());
                inner.search(false);
            }
        }
    });
    inner.filters.on_change({
        let inner = Rc::downgrade(&inner);
        move || {
            if let Some(inner) = inner.upgrade() {
                inner.search(true);
            }
        }
    });
    inner.states.on_retry({
        let inner = Rc::downgrade(&inner);
        move || {
            if let Some(inner) = inner.upgrade() {
                inner.asked.replace(None);
                inner.search(false);
            }
        }
    });
    // The next page as the end comes within a screen.
    results.vadjustment().connect_value_changed({
        let inner = Rc::downgrade(&inner);
        move |adjustment| {
            if let Some(inner) = inner.upgrade()
                && adjustment.upper() - adjustment.value() - adjustment.page_size()
                    < adjustment.page_size()
            {
                inner.more();
            }
        }
    });
    let mut tile = genres.first_child();
    while let Some(child) = tile {
        if let Some(button) = child.first_child().and_downcast::<gtk::Button>() {
            button.connect_clicked({
                let inner = Rc::downgrade(&inner);
                move |button| {
                    if let Some(inner) = inner.upgrade() {
                        // A genre's titles, as its filter alone.
                        let genre = button.widget_name().to_string();
                        inner.filters.set(&SearchQuery {
                            genres: vec![genre],
                            ..SearchQuery::default()
                        });
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
    ctx.hub.send(Command::LoadSearchFacets);

    let page = widgets::content_page("Search", &header, &column);
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

    /// Browse by `query`'s filters with nothing typed, as a screenshot asks.
    #[cfg(feature = "fixtures")]
    pub fn browse(&self, query: &SearchQuery) {
        self.focus();
        self.inner.entry.set_text("");
        self.inner.filters.set(query);
    }

    /// The query on screen, and the next page asked for, as a screenshot of paging asks.
    #[cfg(feature = "fixtures")]
    pub fn asked(&self) -> Option<SearchQuery> {
        self.inner.asked.borrow().clone()
    }

    /// Ask for the next page, as reaching the end does.
    #[cfg(feature = "fixtures")]
    pub fn more(&self) {
        self.inner.more();
    }

    /// Search for `query`, as a screenshot or a deep link asks.
    pub fn search_for(&self, query: &str) {
        self.focus();
        self.inner.entry.set_text(query);
        self.inner.search(false);
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

    /// What is typed and chosen, the text trimmed as the engine tags its answer.
    fn query(&self) -> SearchQuery {
        self.filters.query(&self.entry.text())
    }

    /// Search for what is typed and chosen: `live` as it changes, which the engine waits on
    /// until the changes stop, or at once on Return.
    fn search(&self, live: bool) {
        let query = self.query();
        self.filters.typed(!query.trimmed_text().is_empty());
        if query.is_blank() {
            self.asked.replace(None);
            self.body.set_visible_child_name("before");
            return;
        }
        self.match_settings(&query.text);
        if live && query.waits_for_words() {
            return;
        }
        if self.asked.borrow().as_ref() == Some(&query) && (live || !self.paused.get()) {
            return;
        }
        self.asked.replace(Some(query.clone()));
        self.next_page.set(None);
        self.more_waits(false);
        self.body.set_visible_child_name("results");
        // The results on screen stay for 600 ms, then placeholders stand in their place.
        self.states.loading(Command::Search { query: query.clone() }.activity());
        self.ctx.hub.send(if live && !self.paused.get() {
            Command::SearchLive { query }
        } else {
            Command::Search { query }
        });
    }

    /// The next page of the results, when there is one and it is not already asked for.
    fn more(&self) {
        let Some(page) = self.next_page.get() else { return };
        let Some(query) = self.asked.borrow().clone() else { return };
        if self.asking_more.get() || !self.states.has_answer() {
            return;
        }
        self.more_waits(true);
        self.ctx.hub.send(Command::SearchMore { query, page });
    }

    fn more_waits(&self, on: bool) {
        self.asking_more.set(on);
        self.more_spinner.set_visible(on);
    }

    fn heard(&self, event: &Event) {
        match event {
            // Titles are named another way now: what was found, searched for again.
            Event::TitlesChanged(_) => {
                if let Some(query) = self.asked.borrow().clone() {
                    self.ctx.hub.send(Command::Search { query });
                }
            }
            Event::Facets(facets) => self.filters.set_genres(&facets.genres),
            Event::Content { view: ContentFor::Seasonal, content } => {
                let entries = content.entries();
                self.season.set(&entries[..entries.len().min(12)]);
            }
            Event::LiveSearch { paused } => {
                self.paused.set(*paused);
                // A change of text that was not searched: Return searches it.
                if *paused && !self.states.has_answer() && self.asked.borrow().is_some() {
                    self.asked.replace(None);
                    self.empty.set_title("Press Enter to Search");
                    if let Some(clear) = self.empty.child() {
                        clear.set_visible(false);
                    }
                    self.empty.set_description(Some(
                        "Searching as you type is paused for a moment, to stay within \
                         AniList's limits.",
                    ));
                    self.states.empty();
                }
            }
            Event::SearchPaging { query, has_next } => {
                if self.asked.borrow().as_ref() == Some(query) {
                    self.next_page.set(has_next.then_some(2));
                }
            }
            Event::SearchMore { query, page, entries, has_next } => {
                if self.asked.borrow().as_ref() != Some(query) {
                    return;
                }
                self.more_waits(false);
                if entries.is_empty() {
                    // That page failed: asked for again on the next scroll.
                    return;
                }
                let mut all = self.entries.borrow_mut();
                let new: Vec<Entry> = entries
                    .iter()
                    .filter(|e| !all.iter().any(|a| a.id == e.id))
                    .cloned()
                    .collect();
                all.extend(new);
                self.grid.set(&all);
                self.next_page.set(has_next.then_some(page + 1));
            }
            Event::Content { view: ContentFor::Search { query }, content } => {
                if self.asked.borrow().as_ref() != Some(query) {
                    return;
                }
                match content {
                    ContentList::Loading => {
                        self.states
                            .loading(Command::Search { query: query.clone() }.activity());
                    }
                    ContentList::Entries(entries) if !entries.is_empty() => {
                        self.entries.replace(entries.clone());
                        self.grid.set(entries);
                        self.grid.widget.set_visible(true);
                        self.states.content();
                    }
                    ContentList::Entries(_) | ContentList::Empty | ContentList::Failed(_) => {
                        self.entries.borrow_mut().clear();
                        self.grid.set(&[]);
                        self.grid.widget.set_visible(false);
                        self.next_page.set(None);
                        if self.settings_group.is_visible() {
                            self.states.content();
                        } else if let ContentList::Failed(why) = content {
                            self.states.failed(why);
                        } else {
                            self.empty.set_title(&query.nothing_found());
                            if let Some(clear) = self.empty.child() {
                                clear.set_visible(query.has_filters());
                            }
                            self.empty.set_description(Some(if query.is_browse() {
                                "Try fewer filters."
                            } else {
                                "Try another spelling, or fewer filters."
                            }));
                            self.states.empty();
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
