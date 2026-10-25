//! Search's filters up front, as the Mac has them: genres (wanted, or left out), year, season,
//! format and status, each a menu button, and the sort at the end. Below them, what is set, each
//! taken off with a click, and Clear All. Shown before typing too, so the titles can be browsed
//! without a word typed. The query is the engine's [`SearchQuery`]; the chips' words are its own.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use anistream_model::{
    catalog::Season,
    search::{
        ActiveFilter, FilterKey, MediaFormat, Pick, SEARCH_STATUSES, SearchQuery, SearchSort,
        season_label, status_label,
    },
};
use gtk::{gio, glib};

/// AniList's genres, until the engine says which it offers.
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

const SEASONS: [Season; 4] = [Season::Winter, Season::Spring, Season::Summer, Season::Fall];

/// The oldest year the menu offers.
const FIRST_YEAR: u32 = 1970;

/// What the filters call when one changes.
type Changed = Box<dyn Fn()>;

pub struct Filters {
    /// The menu buttons, in a row that scrolls sideways when the window is narrow.
    pub row: gtk::Box,
    /// What is set, as chips.
    pub chips: gtk::FlowBox,
    genres: gtk::MenuButton,
    year: gtk::MenuButton,
    season: gtk::MenuButton,
    format: gtk::MenuButton,
    status: gtk::MenuButton,
    sort: gtk::MenuButton,
    sort_label: gtk::Label,
    actions: gio::SimpleActionGroup,
    /// Everything but the text: the filters and the sort.
    query: RefCell<SearchQuery>,
    /// Whether text is typed, for the sorts on offer: relevance only with text.
    with_text: std::cell::Cell<bool>,
    names: RefCell<Vec<String>>,
    changed: RefCell<Option<Changed>>,
}

/// A menu button of the row: flat until it holds a choice.
fn menu_button(label: &str) -> gtk::MenuButton {
    let button = gtk::MenuButton::builder().label(label).always_show_arrow(true).build();
    button.add_css_class("flat");
    button.add_css_class("filter");
    button
}

impl Filters {
    pub fn new() -> Rc<Self> {
        let genres = menu_button("Genres");
        let year = menu_button("Year");
        let season = menu_button("Season");
        let format = menu_button("Format");
        let status = menu_button("Status");
        let sort_label = gtk::Label::new(None);
        let sort_line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        sort_line.append(&gtk::Image::from_icon_name("view-sort-descending-symbolic"));
        sort_line.append(&sort_label);
        let sort = gtk::MenuButton::builder().child(&sort_line).always_show_arrow(true).build();
        sort.add_css_class("flat");
        sort.add_css_class("filter");

        let capsules = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for button in [&genres, &year, &season, &format, &status] {
            capsules.append(button);
        }
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .hexpand(true)
            .child(&capsules)
            .build();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.append(&scroll);
        row.append(&sort);

        let chips = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .max_children_per_line(12)
            .column_spacing(6)
            .row_spacing(6)
            .homogeneous(false)
            .halign(gtk::Align::Start)
            .build();
        chips.set_margin_start(18);
        chips.set_margin_end(18);
        chips.set_visible(false);

        let actions = gio::SimpleActionGroup::new();
        row.insert_action_group("filter", Some(&actions));
        chips.insert_action_group("filter", Some(&actions));

        let filters = Rc::new(Self {
            row,
            chips,
            genres,
            year,
            season,
            format,
            status,
            sort,
            sort_label,
            actions,
            query: RefCell::new(SearchQuery::default()),
            with_text: std::cell::Cell::new(false),
            names: RefCell::new(GENRES.iter().map(|g| (*g).to_owned()).collect()),
            changed: RefCell::default(),
        });
        filters.wire();
        filters.refresh();
        filters
    }

    /// Run `f` whenever the viewer changes a filter or the sort.
    pub fn on_change(&self, f: impl Fn() + 'static) {
        self.changed.replace(Some(Box::new(f)));
    }

    /// The query with `text`, and every filter and the sort as chosen.
    pub fn query(&self, text: &str) -> SearchQuery {
        SearchQuery { text: text.trim().to_owned(), ..self.query.borrow().clone() }
    }

    /// Text typed or cleared: the sort on offer follows, relevance only with text.
    pub fn typed(self: &Rc<Self>, with_text: bool) {
        if self.with_text.replace(with_text) != with_text {
            self.refresh();
        }
    }

    /// The genres the engine says AniList offers.
    pub fn set_genres(self: &Rc<Self>, genres: &[String]) {
        if genres.is_empty() || *self.names.borrow() == genres {
            return;
        }
        self.names.replace(genres.to_vec());
        self.refresh();
    }

    /// The genres on offer, for Search's start.
    pub fn genres(&self) -> Vec<String> {
        self.names.borrow().clone()
    }

    /// Set the filters and the sort to `query`'s, as a genre tile or a screenshot does, and
    /// search.
    pub fn set(self: &Rc<Self>, query: &SearchQuery) {
        self.query.replace(SearchQuery { text: String::new(), ..query.clone() });
        self.refresh();
        self.tell();
    }

    fn edit(self: &Rc<Self>, f: impl FnOnce(&SearchQuery) -> SearchQuery) {
        let next = f(&self.query.borrow());
        self.query.replace(next);
        self.refresh();
        self.tell();
    }

    fn tell(&self) {
        if let Some(changed) = self.changed.borrow().as_ref() {
            changed();
        }
    }

    fn wire(self: &Rc<Self>) {
        let plain = |name: &str, f: fn(&SearchQuery) -> SearchQuery| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(filters) = weak.upgrade() {
                    filters.edit(f);
                }
            });
            self.actions.add_action(&action);
        };
        plain("any-genre", |q| SearchQuery {
            genres: vec![],
            genres_excluded: vec![],
            ..q.clone()
        });
        plain("any-format", |q| SearchQuery { formats: vec![], ..q.clone() });
        plain("any-status", |q| SearchQuery { statuses: vec![], ..q.clone() });
        plain("clear", SearchQuery::cleared);

        // The one-of-many choices: a string target each, "" for any.
        let choice = |name: &str, f: fn(&SearchQuery, &str) -> SearchQuery| {
            let action = gio::SimpleAction::new_stateful(
                name,
                Some(glib::VariantTy::STRING),
                &"".to_variant(),
            );
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, target| {
                let (Some(filters), Some(value)) =
                    (weak.upgrade(), target.and_then(String::from_variant))
                else {
                    return;
                };
                filters.edit(|q| f(q, &value));
            });
            self.actions.add_action(&action);
        };
        choice("year", |q, value| q.in_year(value.parse().ok()));
        choice("season", |q, value| SearchQuery {
            season: SEASONS.into_iter().find(|s| season_name(*s) == value),
            ..q.clone()
        });
        choice("sort", |q, value| SearchQuery {
            sort: SearchSort::ALL.into_iter().find(|s| s.label() == value),
            ..q.clone()
        });

        // Each chip takes its filter off: its index among what is set.
        let remove = gio::SimpleAction::new("remove", Some(glib::VariantTy::UINT32));
        let weak = Rc::downgrade(self);
        remove.connect_activate(move |_, target| {
            let (Some(filters), Some(at)) =
                (weak.upgrade(), target.and_then(u32::from_variant))
            else {
                return;
            };
            let key = filters.query.borrow().active().get(at as usize).map(|f| f.key.clone());
            if let Some(key) = key {
                filters.edit(|q| q.without(&key));
            }
        });
        self.actions.add_action(&remove);
    }

    /// A check item's action: on while `on` says so, and `flip` when chosen.
    fn check(
        self: &Rc<Self>,
        name: &str,
        on: bool,
        flip: impl Fn(&SearchQuery) -> SearchQuery + 'static,
    ) {
        if let Some(action) =
            self.actions.lookup_action(name).and_downcast::<gio::SimpleAction>()
        {
            action.set_state(&on.to_variant());
            return;
        }
        let action = gio::SimpleAction::new_stateful(name, None, &on.to_variant());
        let weak = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(filters) = weak.upgrade() {
                filters.edit(&flip);
            }
        });
        self.actions.add_action(&action);
    }

    fn set_state(&self, name: &str, value: &str) {
        if let Some(action) =
            self.actions.lookup_action(name).and_downcast::<gio::SimpleAction>()
        {
            action.set_state(&value.to_variant());
        }
    }

    /// The buttons' words, their menus and the chips, as the query stands.
    fn refresh(self: &Rc<Self>) {
        let query = self.query.borrow().clone();
        self.show_genres(&query);
        self.show_year(&query);
        self.show_season(&query);
        self.show_formats(&query);
        self.show_statuses(&query);
        self.show_sort(&query);
        self.show_chips(&query.active());
    }

    fn show_genres(self: &Rc<Self>, query: &SearchQuery) {
        let names = self.names.borrow().clone();
        let wanted = gio::Menu::new();
        let left_out = gio::Menu::new();
        for (at, name) in names.iter().enumerate() {
            let pick = query.genre_pick(name);
            let (want, leave) = (format!("genre-{at}"), format!("leave-{at}"));
            let named = name.clone();
            self.check(&want, pick == Pick::Include, move |q| {
                let on = q.genre_pick(&named) == Pick::Include;
                q.picking_genre(&named, if on { Pick::Off } else { Pick::Include })
            });
            let named = name.clone();
            self.check(&leave, pick == Pick::Exclude, move |q| {
                let on = q.genre_pick(&named) == Pick::Exclude;
                q.picking_genre(&named, if on { Pick::Off } else { Pick::Exclude })
            });
            wanted.append(Some(name), Some(&format!("filter.{want}")));
            left_out.append(Some(name), Some(&format!("filter.{leave}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(None, &wanted);
        let more = gio::Menu::new();
        more.append_submenu(Some("Leave Out"), &left_out);
        menu.append_section(None, &more);
        let set = query.genres.len() + query.genres_excluded.len();
        if set > 0 {
            let any = gio::Menu::new();
            any.append(Some("Any Genre"), Some("filter.any-genre"));
            menu.append_section(None, &any);
        }
        self.genres.set_menu_model(Some(&menu));
        let label = match set {
            0 => "Genres".to_owned(),
            1 => query.active().into_iter().next().map(|f| f.label).unwrap_or_default(),
            n => format!("{n} Genres"),
        };
        set_label(&self.genres, &label, set > 0);
    }

    fn show_year(&self, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let any = gio::Menu::new();
        any.append(Some("Any Year"), Some("filter.year::"));
        menu.append_section(None, &any);
        let years = gio::Menu::new();
        for year in (FIRST_YEAR..=this_year() + 1).rev() {
            years.append(Some(&year.to_string()), Some(&format!("filter.year::{year}")));
        }
        menu.append_section(None, &years);
        self.year.set_menu_model(Some(&menu));
        let single = query.years.single();
        self.set_state("year", &single.map(|y| y.to_string()).unwrap_or_default());
        let label = query.active().into_iter().find(|f| f.key == FilterKey::Years);
        let set = !query.years.is_open();
        set_label(&self.year, &label.map(|f| f.label).unwrap_or_else(|| "Year".into()), set);
    }

    fn show_season(&self, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let any = gio::Menu::new();
        any.append(Some("Any Season"), Some("filter.season::"));
        menu.append_section(None, &any);
        let seasons = gio::Menu::new();
        for season in SEASONS {
            seasons.append(
                Some(season_label(season)),
                Some(&format!("filter.season::{}", season_name(season))),
            );
        }
        menu.append_section(None, &seasons);
        self.season.set_menu_model(Some(&menu));
        self.set_state("season", query.season.map(season_name).unwrap_or_default());
        // A season is a season of one year.
        let open = query.years.single().is_some() || query.season.is_some();
        self.season.set_sensitive(open);
        self.season.set_tooltip_text((!open).then_some("Choose a year first"));
        let label = query.season.map(season_label).unwrap_or("Season");
        set_label(&self.season, label, query.season.is_some());
    }

    fn show_formats(self: &Rc<Self>, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let all = gio::Menu::new();
        for (at, format) in MediaFormat::ALL.into_iter().enumerate() {
            let name = format!("format-{at}");
            self.check(&name, query.formats.contains(&format), move |q| {
                q.toggling_format(format)
            });
            all.append(Some(format.label()), Some(&format!("filter.{name}")));
        }
        menu.append_section(None, &all);
        let count = query.formats.len();
        if count > 0 {
            let any = gio::Menu::new();
            any.append(Some("Any Format"), Some("filter.any-format"));
            menu.append_section(None, &any);
        }
        self.format.set_menu_model(Some(&menu));
        let label = match count {
            0 => "Format".to_owned(),
            1 | 2 => label_of(query, &FilterKey::Formats).unwrap_or_else(|| "Format".into()),
            n => format!("{n} Formats"),
        };
        set_label(&self.format, &label, count > 0);
    }

    fn show_statuses(self: &Rc<Self>, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let all = gio::Menu::new();
        for (at, status) in SEARCH_STATUSES.into_iter().enumerate() {
            let name = format!("status-{at}");
            self.check(&name, query.statuses.contains(&status), move |q: &SearchQuery| {
                q.toggling_status(status)
            });
            all.append(Some(status_label(status)), Some(&format!("filter.{name}")));
        }
        menu.append_section(None, &all);
        let count = query.statuses.len();
        if count > 0 {
            let any = gio::Menu::new();
            any.append(Some("Any Status"), Some("filter.any-status"));
            menu.append_section(None, &any);
        }
        self.status.set_menu_model(Some(&menu));
        let label = match count {
            0 => "Status".to_owned(),
            1 => label_of(query, &FilterKey::Statuses).unwrap_or_else(|| "Status".into()),
            n => format!("{n} Statuses"),
        };
        set_label(&self.status, &label, count > 0);
    }

    fn show_sort(&self, query: &SearchQuery) {
        let with_text = self.with_text.get();
        let effect = SearchQuery {
            text: if with_text { "x".into() } else { String::new() },
            ..query.clone()
        }
        .sort_in_effect();
        let menu = gio::Menu::new();
        let sorts = gio::Menu::new();
        for sort in SearchSort::ALL {
            if sort == SearchSort::Relevance && !with_text {
                continue;
            }
            sorts.append(Some(sort.label()), Some(&format!("filter.sort::{}", sort.label())));
        }
        menu.append_section(Some("Sort By"), &sorts);
        self.sort.set_menu_model(Some(&menu));
        self.set_state("sort", effect.label());
        self.sort_label.set_label(effect.label());
        self.sort.set_tooltip_text(Some(&format!("Sort by {}", effect.label())));
        if query.sort.is_some() {
            self.sort.remove_css_class("flat");
        } else {
            self.sort.add_css_class("flat");
        }
    }

    fn show_chips(&self, active: &[ActiveFilter]) {
        while let Some(child) = self.chips.first_child() {
            self.chips.remove(&child);
        }
        for (at, filter) in active.iter().enumerate() {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let label = gtk::Label::new(Some(&filter.label));
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(28);
            line.append(&label);
            line.append(&gtk::Image::from_icon_name("window-close-symbolic"));
            let chip = gtk::Button::builder().child(&line).build();
            chip.add_css_class("pill");
            chip.add_css_class("filter-chip");
            chip.set_halign(gtk::Align::Start);
            chip.set_tooltip_text(Some(&format!("Remove {}", filter.label)));
            chip.update_property(&[gtk::accessible::Property::Label(&format!(
                "Remove {}",
                filter.label
            ))]);
            chip.set_action_name(Some("filter.remove"));
            chip.set_action_target_value(Some(&(at as u32).to_variant()));
            self.chips.append(&chip);
        }
        if active.len() > 1 {
            let clear = gtk::Button::with_label("Clear All");
            clear.add_css_class("flat");
            clear.set_halign(gtk::Align::Start);
            clear.set_action_name(Some("filter.clear"));
            self.chips.append(&clear);
        }
        self.chips.set_visible(!active.is_empty());
    }
}

/// A button's words, and whether it holds a choice: flat until it does.
fn set_label(button: &gtk::MenuButton, label: &str, set: bool) {
    button.set_label(label);
    if set {
        button.remove_css_class("flat");
    } else {
        button.add_css_class("flat");
    }
}

/// What the chip for `key` says, when it is set.
fn label_of(query: &SearchQuery, key: &FilterKey) -> Option<String> {
    query.active().into_iter().find(|f| f.key == *key).map(|f| f.label)
}

/// A season's name as an action's target.
fn season_name(season: Season) -> &'static str {
    match season {
        Season::Winter => "winter",
        Season::Spring => "spring",
        Season::Summer => "summer",
        Season::Fall => "fall",
    }
}

fn this_year() -> u32 {
    glib::DateTime::now_local()
        .map(|now| now.year())
        .ok()
        .and_then(|y| u32::try_from(y).ok())
        .unwrap_or(2026)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_season_name_reads_back_as_its_season() {
        for season in SEASONS {
            assert_eq!(
                SEASONS.into_iter().find(|s| season_name(*s) == season_name(season)),
                Some(season)
            );
        }
    }

    #[test]
    fn the_genres_known_before_the_engine_says_are_unique() {
        let mut names = GENRES.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), GENRES.len());
    }
}
