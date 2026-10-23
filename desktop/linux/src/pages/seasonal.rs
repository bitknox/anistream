//! Seasonal: a season's titles as a poster grid, the season picked in the header's centre and
//! the format in a toggle group above the grid.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, SearchFilters, Season},
    command::Command,
    event::Event,
};
use adw::prelude::*;
use gtk::glib;

use crate::{
    nagare,
    widgets::{self, PosterGrid},
    window::Ctx,
    words,
};

/// The formats the toggle group offers: what it says, and the wire formats it keeps.
const FORMATS: [(&str, &str, &[&str]); 4] = [
    ("all", "All", &[]),
    ("tv", "TV", &["TV", "TV_SHORT"]),
    ("movie", "Movie", &["MOVIE"]),
    ("ona", "ONA and OVA", &["ONA", "OVA", "SPECIAL"]),
];

/// The seasons the picker offers: the current, the next, and the two years before.
const BACK: i32 = 8;
const AHEAD: i32 = 1;

struct Page {
    ctx: Rc<Ctx>,
    grid: PosterGrid,
    body: gtk::Stack,
    entries: RefCell<Vec<Entry>>,
    format: Cell<usize>,
    /// The seasons in the picker, newest first, and which is the current one.
    seasons: Vec<(Season, i32)>,
    now: usize,
    picked: Cell<usize>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let (season, year) = words::season_at(now());
    let seasons: Vec<_> =
        (-AHEAD..=BACK).map(|back| words::season_step(season, year, -back)).collect();
    let now = AHEAD as usize;

    let names: Vec<String> = seasons.iter().map(|(s, y)| words::season(*s, y)).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let picker = gtk::DropDown::from_strings(&refs);
    picker.set_selected(now as u32);
    picker.set_tooltip_text(Some("Season"));
    picker.add_css_class("flat");

    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.set_title_widget(Some(&picker));

    let toggles = adw::ToggleGroup::new();
    for (name, label, _) in FORMATS {
        toggles.add(adw::Toggle::builder().name(name).label(label).build());
    }
    toggles.set_active_name(Some("all"));
    toggles.set_halign(gtk::Align::Start);
    toggles.add_css_class("round");

    let grid = PosterGrid::new(&ctx.hub);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&grid.widget)
        .build();
    let loading = adw::Spinner::new();
    loading.set_size_request(32, 32);
    loading.set_halign(gtk::Align::Center);
    loading.set_valign(gtk::Align::Center);
    let body = gtk::Stack::new();
    body.add_named(&loading, Some("loading"));
    body.add_named(&scroll, Some("grid"));
    body.add_named(
        &nagare::status_page("Nothing This Season", "No titles in this format this season.", None),
        Some("empty"),
    );

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    bar.set_margin_top(12);
    bar.set_margin_start(18);
    bar.set_margin_end(18);
    bar.append(&toggles);
    column.append(&bar);
    column.append(&body);

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        grid,
        body,
        entries: RefCell::default(),
        format: Cell::new(0),
        seasons,
        now,
        picked: Cell::new(now),
    });
    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        if let Some(page) = weak.upgrade() {
            page.heard(event);
        }
    });
    toggles.connect_active_name_notify({
        let page = Rc::clone(&page);
        move |toggles| {
            let name = toggles.active_name().unwrap_or_default();
            page.format.set(FORMATS.iter().position(|(n, _, _)| *n == name).unwrap_or(0));
            page.show();
        }
    });
    picker.connect_selected_notify({
        let page = Rc::clone(&page);
        move |picker| page.pick(picker.selected() as usize)
    });

    widgets::page("Seasonal", &header, &column)
}

impl Page {
    /// The current season is the engine's own Seasonal list; any other is a search by season.
    fn pick(&self, at: usize) {
        self.picked.set(at);
        self.body.set_visible_child_name("loading");
        if at == self.now {
            self.ctx.hub.send(Command::LoadSeasonal);
        } else {
            self.ctx.hub.send(Command::Search { query: String::new(), filter: self.filter(at) });
        }
    }

    fn filter(&self, at: usize) -> SearchFilters {
        let (season, year) = self.seasons[at];
        SearchFilters {
            season: Some(words::season_wire(season).into()),
            year: u32::try_from(year).ok(),
            sort: Some("POPULARITY_DESC".into()),
            ..SearchFilters::default()
        }
    }

    fn heard(&self, event: &Event) {
        let Event::Content { view, content } = event else { return };
        let ours = match view {
            ContentFor::Seasonal => self.picked.get() == self.now,
            ContentFor::Search { query, filter } => {
                query.is_empty() && self.picked.get() != self.now && *filter == self.filter(self.picked.get())
            }
            _ => false,
        };
        if !ours {
            return;
        }
        match content {
            ContentList::Loading => self.body.set_visible_child_name("loading"),
            ContentList::Failed(why) => {
                tracing::warn!(%why, "the season did not load");
                *self.entries.borrow_mut() = Vec::new();
                self.show();
            }
            _ => {
                *self.entries.borrow_mut() = content.entries().to_vec();
                self.show();
            }
        }
    }

    fn show(&self) {
        let (_, _, keep) = FORMATS[self.format.get()];
        let entries: Vec<Entry> = self
            .entries
            .borrow()
            .iter()
            .filter(|e| {
                keep.is_empty()
                    || e.format.as_deref().is_some_and(|f| keep.contains(&f.to_ascii_uppercase().as_str()))
            })
            .cloned()
            .collect();
        self.grid.set(&entries);
        self.body.set_visible_child_name(if entries.is_empty() { "empty" } else { "grid" });
    }
}

fn now() -> i64 {
    glib::DateTime::now_utc().map_or(0, |t| t.to_unix())
}
