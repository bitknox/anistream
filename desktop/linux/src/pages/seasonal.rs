//! Seasonal: a season's titles as a poster grid, the season picked in the header's centre and
//! the format in a toggle group above the grid.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, Season},
    command::Command,
    event::Event,
    search::SearchQuery,
};
use gtk::glib;

use crate::{
    states::{self, Shape, States},
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
    body: Rc<States>,
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
    header.pack_start(&crate::window::tv_session_button(ctx));
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
    let empty = states::plain("Nothing This Season", "No titles in this format this season.");
    let body = States::new(&ctx.hub, &scroll, Shape::Grid, &empty, "Couldn't Load This Season");

    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    bar.set_margin_top(12);
    bar.set_margin_start(18);
    bar.set_margin_end(18);
    bar.append(&toggles);
    column.append(&bar);
    column.append(&body.widget);

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
    // Continue asks for this season as the app opens.
    page.body.loading(Command::LoadSeasonal.activity());
    page.body.on_retry({
        let weak = Rc::downgrade(&page);
        move || {
            if let Some(page) = weak.upgrade() {
                page.pick(page.picked.get());
            }
        }
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
            if page.body.has_answer() {
                page.show();
            }
        }
    });
    picker.connect_selected_notify({
        let page = Rc::clone(&page);
        move |picker| page.pick(picker.selected() as usize)
    });

    widgets::content_page("Seasonal", &header, &column)
}

impl Page {
    /// The current season is the engine's own Seasonal list; any other is a search by season.
    fn pick(&self, at: usize) {
        self.picked.set(at);
        let command = if at == self.now {
            Command::LoadSeasonal
        } else {
            Command::Search { query: self.query(at) }
        };
        self.body.loading(Some("loading this season".into()));
        self.ctx.hub.send(command);
    }

    fn query(&self, at: usize) -> SearchQuery {
        let (season, year) = self.seasons[at];
        SearchQuery::season(season, u16::try_from(year).unwrap_or_default())
    }

    fn heard(&self, event: &Event) {
        let Event::Content { view, content } = event else { return };
        let ours = match view {
            ContentFor::Seasonal => self.picked.get() == self.now,
            ContentFor::Search { query } => {
                self.picked.get() != self.now && *query == self.query(self.picked.get())
            }
            _ => false,
        };
        if !ours {
            return;
        }
        match content {
            ContentList::Loading => self.body.loading(Some("loading this season".into())),
            ContentList::Failed(why) => {
                tracing::warn!(%why, "the season did not load");
                *self.entries.borrow_mut() = Vec::new();
                self.body.failed(why);
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
                    || e.format
                        .as_deref()
                        .is_some_and(|f| keep.contains(&f.to_ascii_uppercase().as_str()))
            })
            .cloned()
            .collect();
        self.grid.set(&entries);
        if entries.is_empty() {
            self.body.empty();
        } else {
            self.body.content();
        }
    }
}

fn now() -> i64 {
    glib::DateTime::now_utc().map_or(0, |t| t.to_unix())
}
