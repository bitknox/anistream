//! Search's filters up front, as the Mac has them: genres (wanted, or left out), year, season,
//! format and status, each a menu button, and the sort at the end. Below them, what is set, each
//! taken off with a click, and Clear All. Shown before typing too, so the titles can be browsed
//! without a word typed. The query is the engine's [`SearchQuery`]; the chips' words are its own.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use anistream_i18n::t;
use anistream_model::{
    catalog::Season,
    search::{
        ActiveFilter, FilterKey, MediaFormat, Pick, SEARCH_STATUSES, SearchQuery, SearchSort,
    },
};
use gtk::{gio, glib};

use crate::say;

/// AniList's genres, until the engine says which it offers.
const GENRES: [&str; 16] = [
    "Action",        // i18n-skip
    "Adventure",     // i18n-skip
    "Comedy",        // i18n-skip
    "Drama",         // i18n-skip
    "Fantasy",       // i18n-skip
    "Horror",        // i18n-skip
    "Mecha",         // i18n-skip
    "Music",         // i18n-skip
    "Mystery",       // i18n-skip
    "Psychological", // i18n-skip
    "Romance",       // i18n-skip
    "Sci-Fi",        // i18n-skip
    "Slice of Life", // i18n-skip
    "Sports",        // i18n-skip
    "Supernatural",  // i18n-skip
    "Thriller",      // i18n-skip
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
    pub chips: adw::WrapBox,
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
        let genres = menu_button(&t::filter_genres());
        let year = menu_button(&t::filter_year());
        let season = menu_button(&t::filter_season());
        let format = menu_button(&t::filter_format());
        let status = menu_button(&t::filter_status());
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
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.append(&faded_scroll(&capsules));
        row.append(&sort);

        let chips = adw::WrapBox::builder()
            .child_spacing(6)
            .line_spacing(6)
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

    /// Every filter off, as Clear All does.
    pub fn clear(&self) {
        self.actions.activate_action("clear", None);
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
                // A chip removed from the keyboard hands focus on: to the chip that takes its
                // place, else the one before, else the filter row.
                let had_focus = filters.chips.focus_child().is_some();
                filters.edit(|q| q.without(&key));
                if had_focus {
                    filters.focus_chip(at as usize);
                }
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
            wanted.append(Some(&say::genre(name)), Some(&format!("filter.{want}")));
            left_out.append(Some(&say::genre(name)), Some(&format!("filter.{leave}")));
        }
        let menu = gio::Menu::new();
        menu.append_section(None, &wanted);
        let more = gio::Menu::new();
        more.append_submenu(Some(&t::filter_left_out()), &left_out);
        menu.append_section(None, &more);
        let set = query.genres.len() + query.genres_excluded.len();
        if set > 0 {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_genre()), Some("filter.any-genre"));
            menu.append_section(None, &any);
        }
        self.genres.set_menu_model(Some(&menu));
        let label = match set {
            0 => t::filter_genres(),
            1 => query
                .active()
                .into_iter()
                .next()
                .map(|f| chip_label(query, &f))
                .unwrap_or_default(),
            n => t::filter_linux_genres_count(n as i64),
        };
        set_label(&self.genres, &label, set > 0);
    }

    fn show_year(&self, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let any = gio::Menu::new();
        any.append(Some(&t::filter_any_year()), Some("filter.year::"));
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
        set_label(&self.year, &label.map(|f| f.label).unwrap_or_else(t::filter_year), set);
    }

    fn show_season(&self, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let any = gio::Menu::new();
        any.append(Some(&t::filter_any_season()), Some("filter.season::"));
        menu.append_section(None, &any);
        let seasons = gio::Menu::new();
        for season in SEASONS {
            seasons.append(
                Some(&say::season_name(season)),
                Some(&format!("filter.season::{}", season_name(season))),
            );
        }
        menu.append_section(None, &seasons);
        self.season.set_menu_model(Some(&menu));
        self.set_state("season", query.season.map(season_name).unwrap_or_default());
        // A season is a season of one year.
        let open = query.years.single().is_some() || query.season.is_some();
        self.season.set_sensitive(open);
        self.season.set_tooltip_text((!open).then(t::filter_season_pick_year).as_deref());
        let label = query.season.map(say::season_name).unwrap_or_else(t::filter_season);
        set_label(&self.season, &label, query.season.is_some());
    }

    fn show_formats(self: &Rc<Self>, query: &SearchQuery) {
        let menu = gio::Menu::new();
        let all = gio::Menu::new();
        for (at, format) in MediaFormat::ALL.into_iter().enumerate() {
            let name = format!("format-{at}");
            self.check(&name, query.formats.contains(&format), move |q| {
                q.toggling_format(format)
            });
            all.append(Some(&format_name(format)), Some(&format!("filter.{name}")));
        }
        menu.append_section(None, &all);
        let count = query.formats.len();
        if count > 0 {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_format()), Some("filter.any-format"));
            menu.append_section(None, &any);
        }
        self.format.set_menu_model(Some(&menu));
        let label = match count {
            0 => t::filter_format(),
            1 | 2 => formats_line(&query.formats),
            n => t::filter_linux_formats_count(n as i64),
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
            all.append(Some(&say::airing_status(status)), Some(&format!("filter.{name}")));
        }
        menu.append_section(None, &all);
        let count = query.statuses.len();
        if count > 0 {
            let any = gio::Menu::new();
            any.append(Some(&t::filter_any_status()), Some("filter.any-status"));
            menu.append_section(None, &any);
        }
        self.status.set_menu_model(Some(&menu));
        let label = match count {
            0 => t::filter_status(),
            1 => say::airing_status(query.statuses[0]),
            n => t::filter_linux_statuses_count(n as i64),
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
            sorts.append(
                Some(&sort_name(sort)),
                Some(&format!("filter.sort::{}", sort.label())),
            );
        }
        menu.append_section(Some(&t::filter_linux_sort_by()), &sorts);
        self.sort.set_menu_model(Some(&menu));
        self.set_state("sort", effect.label());
        self.sort_label.set_label(&sort_name(effect));
        self.sort.set_tooltip_text(Some(&t::sort_label(&sort_name(effect))));
        if query.sort.is_some() {
            self.sort.remove_css_class("flat");
        } else {
            self.sort.add_css_class("flat");
        }
    }

    fn focus_chip(&self, at: usize) {
        let mut chips = Vec::new();
        let mut child = self.chips.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            chips.push(c);
        }
        let target = chips.get(at).or_else(|| at.checked_sub(1).and_then(|i| chips.get(i)));
        match target {
            Some(chip) => {
                chip.grab_focus();
            }
            None => {
                self.genres.grab_focus();
            }
        }
    }

    fn show_chips(&self, active: &[ActiveFilter]) {
        let query = self.query.borrow().clone();
        while let Some(child) = self.chips.first_child() {
            self.chips.remove(&child);
        }
        for (at, filter) in active.iter().enumerate() {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let label = gtk::Label::new(Some(&chip_label(&query, filter)));
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(28);
            line.append(&label);
            line.append(&gtk::Image::from_icon_name("window-close-symbolic"));
            let chip = gtk::Button::builder().child(&line).build();
            chip.add_css_class("pill");
            chip.add_css_class("filter-chip");
            chip.set_halign(gtk::Align::Start);
            chip.set_tooltip_text(Some(&t::filter_remove(&chip_label(&query, filter))));
            chip.update_property(&[gtk::accessible::Property::Label(&t::filter_remove(
                &chip_label(&query, filter),
            ))]);
            chip.set_action_name(Some("filter.remove"));
            chip.set_action_target_value(Some(&(at as u32).to_variant()));
            self.chips.append(&chip);
        }
        if active.len() > 1 {
            let clear = gtk::Button::with_label(&t::filter_linux_clear_all());
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

/// A format as the catalogue says it.
fn format_name(format: MediaFormat) -> String {
    match format {
        MediaFormat::Tv => t::format_tv(),
        MediaFormat::TvShort => t::format_tv_short(),
        MediaFormat::Movie => t::format_movie(),
        MediaFormat::Special => t::format_special(),
        MediaFormat::Ova => t::format_ova(),
        MediaFormat::Ona => t::format_ona(),
        MediaFormat::Music => t::format_music(),
    }
}

fn formats_line(formats: &[MediaFormat]) -> String {
    formats.iter().map(|f| format_name(*f)).collect::<Vec<_>>().join(", ")
}

/// A sort as the catalogue says it.
fn sort_name(sort: SearchSort) -> String {
    match sort {
        SearchSort::Relevance => t::sort_relevance(),
        SearchSort::Trending => t::filter_linux_sort_trending(),
        SearchSort::Popularity => t::sort_popularity(),
        SearchSort::Score => t::sort_score(),
        SearchSort::Newest => t::filter_linux_sort_newest(),
        SearchSort::Oldest => t::filter_linux_sort_oldest(),
        SearchSort::Title => t::library_sort_title(),
        SearchSort::Favourites => t::filter_linux_sort_favourites(),
    }
}

/// What a chip says, in the language in use where the catalogue has the words; the engine's own
/// wording for the filters it words alone.
fn chip_label(query: &SearchQuery, filter: &ActiveFilter) -> String {
    match &filter.key {
        FilterKey::Genre { name } => say::genre(name),
        FilterKey::NotGenre { name } => t::filter_linux_not(&say::genre(name)),
        FilterKey::Formats => formats_line(&query.formats),
        FilterKey::Statuses => {
            query.statuses.iter().map(|s| say::airing_status(*s)).collect::<Vec<_>>().join(", ")
        }
        FilterKey::Premiered => query.season.map(say::season_name).unwrap_or_default(),
        _ => filter.label.clone(),
    }
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

/// The menu buttons in a row that scrolls sideways, as the Mac draws it: the edge faded where
/// more of it goes on, with an arrow there that scrolls to it, so a narrow window shows there is
/// more to reach.
fn faded_scroll(capsules: &gtk::Box) -> gtk::Overlay {
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::External)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .min_content_width(120)
        .hexpand(true)
        .child(capsules)
        .build();
    let adjustment = scroll.hadjustment();
    let edge = |side: &str, align: gtk::Align, icon: &str, tip: &str, by: f64| {
        let fade =
            gtk::Box::builder().halign(align).width_request(48).can_target(false).build();
        fade.add_css_class("edge-fade");
        fade.add_css_class(side);
        let arrow = gtk::Button::from_icon_name(icon);
        arrow.add_css_class("circular");
        arrow.add_css_class("edge-arrow");
        arrow.set_halign(align);
        arrow.set_valign(gtk::Align::Center);
        arrow.set_tooltip_text(Some(tip));
        // The row's buttons take the keyboard on their own: the arrow is for the pointer.
        arrow.set_focusable(false);
        let adjustment = adjustment.clone();
        arrow.connect_clicked(move |_| {
            let to = adjustment.value() + by * adjustment.page_size() * 0.7;
            adjustment.set_value(to.clamp(0.0, adjustment.upper() - adjustment.page_size()));
        });
        (fade, arrow)
    };
    let (start_fade, start_arrow) = edge(
        "start",
        gtk::Align::Start,
        "go-previous-symbolic",
        &t::filter_linux_earlier(),
        -1.0,
    );
    let (end_fade, end_arrow) =
        edge("end", gtk::Align::End, "go-next-symbolic", &t::filter_linux_more(), 1.0);
    let overlay = gtk::Overlay::builder().child(&scroll).hexpand(true).build();
    for widget in [&start_fade, &end_fade] {
        overlay.add_overlay(widget);
    }
    overlay.add_overlay(&start_arrow);
    overlay.add_overlay(&end_arrow);
    let follow = move |a: &gtk::Adjustment| {
        let (value, room) = (a.value(), a.upper() - a.page_size());
        let (before, after) = (value > 1.0, room - value > 1.0);
        start_fade.set_visible(before);
        start_arrow.set_visible(before);
        end_fade.set_visible(after);
        end_arrow.set_visible(after);
    };
    follow(&adjustment);
    adjustment.connect_value_changed(follow.clone());
    adjustment.connect_changed(follow);
    overlay
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
