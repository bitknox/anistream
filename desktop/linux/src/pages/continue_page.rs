//! Continue: the queue down the start edge, grouped In Progress and New This Week, and beside it
//! the selected row's preview with its one amber action. Below the wide size the inner split
//! collapses: the queue fills the content and a row opens its title page. With nothing watched
//! yet, Nagare and Browse Seasonal across the whole page.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry},
    command::Command,
    episodes::EpisodeRow,
    event::Event,
    up_next::{UpNextAction, UpNextItem, UpNextReason},
};
use gtk::{gdk, gio, glib};

use crate::{
    motion, nagare,
    pages::calendar,
    say,
    say::{Queue, QueueRow},
    states::{Shape, States},
    widgets,
    window::{Ctx, Size},
};

struct Page {
    ctx: Rc<Ctx>,
    split: adw::NavigationSplitView,
    list: gtk::ListBox,
    body: Rc<States>,
    preview: gtk::Stack,
    preview_page: adw::NavigationPage,
    queue: RefCell<Queue>,
    ids: RefCell<Vec<u32>>,
    episodes: RefCell<HashMap<u32, Vec<EpisodeRow>>>,
    selected: Cell<Option<u32>>,
    settle: RefCell<Option<glib::SourceId>>,
    shown: Cell<u32>,
    /// "Up next for you": the engine's suggestions, less those just dismissed.
    up: RefCell<Vec<UpNextItem>>,
    /// Whether the group is under the queue now, so it fades in only as it arrives.
    up_on_screen: Cell<bool>,
    /// The suggestions as last drawn, and whether Continue has answered at all.
    drawn_up: RefCell<Vec<UpNextItem>>,
    continue_in: Cell<bool>,
    /// The group is Airing today, not suggestions, so its heading says so.
    fallback: Rc<Cell<bool>>,
    /// The welcome's two sources: the calendar's rows with the instant they count from, and
    /// the season's titles. A source that has not answered holds the empty state back.
    calendar: RefCell<Vec<Entry>>,
    anchor: Cell<i64>,
    /// The wall clock when the anchor arrived, so the engine's now can be told later.
    anchor_at: Cell<i64>,
    calendar_in: Cell<bool>,
    season: RefCell<Vec<Entry>>,
    season_in: Cell<bool>,
    /// Where the empty state goes: the welcome, or Nagare when there is nothing to welcome with.
    holder: gtk::Box,
    nagare: adw::StatusPage,
    welcome: RefCell<Option<gtk::Widget>>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let list = gtk::ListBox::new();
    list.add_css_class("queue");
    list.set_selection_mode(gtk::SelectionMode::Browse);
    list.set_activate_on_single_click(false);
    let fallback = Rc::new(Cell::new(false));
    let heading_fallback = Rc::clone(&fallback);
    list.set_header_func(move |row, before| {
        let group = row.widget_name();
        let changed = before.is_none_or(|b| b.widget_name() != group);
        if changed {
            let text = match group.as_str() {
                "new" => t::group_new_this_week(),
                "up" if heading_fallback.get() => t::continue_airing_today(),
                "up" => t::up_next_heading(),
                _ => t::group_in_progress(),
            };
            let label = widgets::heading(&text);
            label.add_css_class("queue-heading");
            row.set_header(Some(&label));
        } else {
            row.set_header(gtk::Widget::NONE);
        }
    });
    list.set_margin_start(8);
    list.set_margin_end(8);
    list.set_margin_bottom(12);
    // Across a medium window, the queue keeps to a readable width.
    let clamp =
        adw::Clamp::builder().maximum_size(720).tightening_threshold(720).child(&list).build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&clamp)
        .build();

    let browse = gtk::Button::with_label(&t::continue_empty_action());
    browse.add_css_class("pill");
    browse.add_css_class("suggested-action");
    browse.set_halign(gtk::Align::Center);
    browse.set_action_name(Some("win.go"));
    browse.set_action_target_value(Some(&"seasonal".to_variant()));
    let empty = nagare::status_page(
        &t::continue_empty_title(),
        &t::continue_empty_detail(),
        Some(&browse),
    );
    let rows = Shape::Rows { count: 5, width: 36, height: 54 };
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.append(&empty);
    let body = States::new(&ctx.hub, &scroll, rows, &holder, &t::continue_load_failed());
    body.loading(Some(t::activity_continue()));
    body.on_retry({
        let hub = Rc::downgrade(&ctx.hub);
        move || {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::LoadContinue);
            }
        }
    });

    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));
    let sidebar_page = widgets::content_page(&t::nav_continue(), &header, &body.widget);

    let preview = gtk::Stack::new();
    preview.set_transition_type(gtk::StackTransitionType::Crossfade);
    preview.set_transition_duration(200);
    let preview_header = adw::HeaderBar::new();
    preview_header.set_show_title(false);
    let preview_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&preview)
        .build();
    let preview_page = widgets::page(&t::continue_preview(), &preview_header, &preview_scroll);

    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&preview_page)
        .min_sidebar_width(360.0)
        .max_sidebar_width(360.0)
        .show_content(true)
        .build();
    split.add_css_class("continue");

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        split: split.clone(),
        list: list.clone(),
        body,
        preview,
        preview_page,
        queue: RefCell::default(),
        ids: RefCell::default(),
        episodes: RefCell::default(),
        selected: Cell::new(None),
        settle: RefCell::default(),
        shown: Cell::new(0),
        up: RefCell::default(),
        up_on_screen: Cell::new(false),
        drawn_up: RefCell::default(),
        continue_in: Cell::new(false),
        fallback,
        calendar: RefCell::default(),
        anchor: Cell::new(0),
        anchor_at: Cell::new(0),
        calendar_in: Cell::new(false),
        season: RefCell::default(),
        season_in: Cell::new(false),
        holder,
        nagare: empty,
        welcome: RefCell::default(),
    });
    page.wire();
    ctx.hub.send(Command::LoadContinue);
    ctx.hub.send(Command::LoadUpNext);
    ctx.hub.send(Command::LoadCalendar);
    ctx.hub.send(Command::LoadSeasonal);

    let root = adw::NavigationPage::builder().title(t::nav_continue()).child(&split).build();
    widgets::keep(&root, page);
    root
}

impl Page {
    fn wire(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.ctx.hub.listen(move |event| {
            if let Some(page) = weak.upgrade() {
                page.heard(event);
            }
        });

        let weak = Rc::downgrade(self);
        self.list.connect_row_selected(move |_, row| {
            let Some(page) = weak.upgrade() else { return };
            let Some(id) = row.and_then(row_id) else { return };
            page.select(id);
        });
        // Enter plays; double-click, or a single click when collapsed, opens the title.
        let weak = Rc::downgrade(self);
        self.list.connect_row_activated(move |_, row| {
            let Some(page) = weak.upgrade() else { return };
            if let Some(id) = row_id(row) {
                let _ = page.list.activate_action("win.open-title", Some(&id.to_variant()));
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(page) = weak.upgrade() else { return glib::Propagation::Proceed };
            if let Some(up) = page.selected_up() {
                return page.suggestion_key(&up, key);
            }
            let Some(item) = page.selected_item() else { return glib::Propagation::Proceed };
            let id = item.entry.id.get();
            let play = (id, item.episode.to_string()).to_variant();
            let ctrl_shift = gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK;
            match key {
                gdk::Key::Return | gdk::Key::KP_Enter if !page.split.is_collapsed() => {
                    if item.playable {
                        let _ = page.list.activate_action("win.play", Some(&play));
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::Delete => {
                    let _ =
                        page.list.activate_action("win.hide-continue", Some(&id.to_variant()));
                    glib::Propagation::Stop
                }
                gdk::Key::M | gdk::Key::m if modifiers.contains(ctrl_shift) => {
                    let _ = page.list.activate_action("win.mark-watched", Some(&play));
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.list.add_controller(keys);

        // Not interested, from a suggestion's menu or the Delete key.
        let actions = gio::SimpleActionGroup::new();
        let dismiss = gio::SimpleAction::new("dismiss", Some(glib::VariantTy::UINT32));
        let weak = Rc::downgrade(self);
        dismiss.connect_activate(move |_, id| {
            if let (Some(page), Some(id)) = (weak.upgrade(), id.and_then(u32::from_variant)) {
                page.dismiss(id);
            }
        });
        actions.add_action(&dismiss);
        self.list.insert_action_group("up", Some(&actions));

        let weak = Rc::downgrade(self);
        self.ctx.on_size(move |_| {
            if let Some(page) = weak.upgrade() {
                page.layout();
            }
        });
    }

    /// The queue beside its preview on a wide window with something in it; otherwise the
    /// queue, or what stands in for it, across the whole page.
    fn layout(self: &Rc<Self>) {
        let narrow = self.ctx.size() != Size::Wide;
        let collapsed = narrow || self.queue.borrow().is_empty();
        self.split.set_collapsed(collapsed);
        self.split.set_show_content(!collapsed);
        self.list.set_activate_on_single_click(narrow);
        self.list.set_selection_mode(if narrow {
            gtk::SelectionMode::None
        } else {
            gtk::SelectionMode::Browse
        });
        if !collapsed {
            self.reselect();
        }
    }

    fn heard(self: &Rc<Self>, event: &Event) {
        match event {
            Event::Content { view: ContentFor::Continue, content } => match content {
                ContentList::Loading => self.body.loading(Some(t::activity_continue())),
                ContentList::Entries(entries) => {
                    self.continue_in.set(true);
                    *self.ids.borrow_mut() = entries.iter().map(|e| e.id.get()).collect();
                    self.rebuild();
                }
                ContentList::Empty => {
                    self.continue_in.set(true);
                    self.ids.borrow_mut().clear();
                    self.rebuild();
                }
                ContentList::Failed(why) => {
                    tracing::warn!(%why, "continue did not load");
                    self.body.failed(why);
                }
            },
            Event::UpNext(items) => {
                *self.up.borrow_mut() = items.clone();
                self.rebuild();
            }
            Event::CalendarWindow { anchor, .. } => {
                self.anchor.set(*anchor);
                self.anchor_at.set(now());
            }
            Event::Content { view: ContentFor::Calendar, content } => {
                *self.calendar.borrow_mut() = content.entries().to_vec();
                self.calendar_in.set(!matches!(content, ContentList::Loading));
                self.welcome_changed();
            }
            Event::Content { view: ContentFor::Seasonal, content } => {
                *self.season.borrow_mut() = content.entries().to_vec();
                self.season_in.set(!matches!(content, ContentList::Loading));
                self.welcome_changed();
            }
            Event::Progress { id, .. } => {
                if self.ids.borrow().contains(&id.get()) {
                    self.rebuild();
                }
            }
            Event::Detail(entry) => {
                if self.ids.borrow().contains(&entry.id.get()) {
                    self.rebuild();
                }
            }
            Event::LastAired(_) => self.rebuild(),
            // Titles are named another way now: the queue and its preview again.
            Event::TitlesChanged(_) => self.ctx.hub.send(Command::LoadContinue),
            Event::Episodes { id, rows } => {
                self.episodes.borrow_mut().insert(id.get(), rows.clone());
                // A row plays the next episode a source lists, so a table can move it.
                if self.ids.borrow().contains(&id.get()) {
                    self.rebuild();
                }
                if self.selected.get() == Some(id.get()) {
                    self.show_preview(true);
                }
            }
            _ => {}
        }
    }

    /// The suggestions to draw: none with the welcome, and none for a title the queue holds or
    /// one just dismissed.
    fn suggestions(&self, queue: &Queue) -> Vec<UpNextItem> {
        if queue.is_empty() {
            return Vec::new();
        }
        let queued: Vec<u32> = queue.all().map(|r| r.entry.id.get()).collect();
        self.up
            .borrow()
            .iter()
            .filter(|i| !queued.contains(&i.entry.id.get()))
            .cloned()
            .collect()
    }

    /// The queue from what is known of each title, in the engine's order, and under it the
    /// suggestions.
    fn rebuild(self: &Rc<Self>) {
        let entries: Vec<Entry> = {
            let known = self.ctx.hub.known.borrow();
            self.ids.borrow().iter().filter_map(|id| known.entries.get(id).cloned()).collect()
        };
        let queue = say::queue(&entries, &[], &self.episodes.borrow());
        let up = self.suggestions(&queue);
        let same = *self.queue.borrow() == queue && *self.drawn_up.borrow() == up;
        *self.queue.borrow_mut() = queue;
        if same && self.list.first_child().is_some() && self.body.has_answer() {
            return;
        }
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let queue = self.queue.borrow().clone();
        for (group, items) in [("progress", &queue.in_progress), ("new", &queue.new_this_week)]
        {
            for item in items {
                self.list.append(&self.row(group, item));
            }
        }
        self.fallback.set(up.first().is_some_and(|i| i.fallback));
        let arriving = !up.is_empty() && !self.up_on_screen.get();
        for item in &up {
            let row = self.up_row(item);
            self.list.append(&row);
            if arriving {
                // The group comes in as one, and nothing above it moves.
                motion::fade_in(&row);
                if let Some(header) = row.header() {
                    motion::fade_in(&header);
                }
            }
        }
        self.up_on_screen.set(!up.is_empty());
        *self.drawn_up.borrow_mut() = up;
        if queue.is_empty() {
            self.empty_state();
        } else {
            self.body.content();
        }
        self.layout();
    }

    /// Nothing is under way: the welcome once its two sources have answered, with nothing to
    /// show in it Nagare.
    fn empty_state(self: &Rc<Self>) {
        if !(self.calendar_in.get() && self.season_in.get()) {
            self.body.loading(Some(t::activity_continue()));
            return;
        }
        if let Some(old) = self.welcome.borrow_mut().take() {
            self.holder.remove(&old);
        }
        let welcome = self.welcome_view();
        self.nagare.set_visible(welcome.is_none());
        if let Some(welcome) = welcome {
            self.holder.append(&welcome);
            *self.welcome.borrow_mut() = Some(welcome);
        }
        self.body.empty();
    }

    /// The calendar or the season answered: the welcome may now be drawn.
    fn welcome_changed(self: &Rc<Self>) {
        if self.continue_in.get() && self.queue.borrow().is_empty() {
            self.empty_state();
        }
    }

    /// The engine's now, in Unix seconds: its calendar's anchor and the time since, which is
    /// the wall clock wherever the engine has no clock of its own to set.
    fn engine_now(&self) -> i64 {
        match self.anchor.get() {
            0 => now(),
            anchor => anchor + (now() - self.anchor_at.get()),
        }
    }

    /// What airs today, in the order it airs, one card to a title.
    fn airing_today(&self) -> Vec<(Entry, u32, i64)> {
        let anchor = self.anchor.get();
        let today = glib::DateTime::from_unix_local(self.engine_now())
            .ok()
            .and_then(|d| d.format("%F").ok());
        let mut seen = Vec::new();
        let mut out: Vec<(Entry, u32, i64)> = self
            .calendar
            .borrow()
            .iter()
            .filter_map(|e| {
                let (airing_in, episode) = (e.airing_in?, e.airing_episode?);
                let airs = anchor + airing_in;
                let day = glib::DateTime::from_unix_local(airs).ok()?.format("%F").ok();
                (day == today).then(|| (e.clone(), episode, airs))
            })
            .collect();
        out.sort_by_key(|(_, _, airs)| *airs);
        out.retain(|(e, ..)| {
            let fresh = !seen.contains(&e.id.get());
            seen.push(e.id.get());
            fresh
        });
        out.truncate(4);
        out
    }

    /// The welcome: Airing today as wide cards, This season as posters. `None` when both are
    /// empty.
    fn welcome_view(self: &Rc<Self>) -> Option<gtk::Widget> {
        let today = self.airing_today();
        let season: Vec<Entry> = self.season.borrow().iter().take(6).cloned().collect();
        if today.is_empty() && season.is_empty() {
            return None;
        }
        let hub = &self.ctx.hub;
        let now = self.engine_now();
        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        column.set_margin_start(24);
        column.set_margin_end(24);
        column.set_margin_top(8);
        column.set_margin_bottom(24);

        if !today.is_empty() {
            column.append(&section(
                &t::continue_airing_today(),
                &t::nav_calendar(),
                "calendar",
            ));
            let cards = flow(4);
            for (entry, episode, airs) in &today {
                let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
                card.set_size_request(CARD_WIDTH, -1);
                let art = widgets::banner(hub, entry, 136, "welcome-card");
                art.set_halign(gtk::Align::Fill);
                card.append(&art);
                card.append(&caption_title(&entry.title, 8));
                let line = if *airs <= now {
                    t::up_next_broadcast(
                        i64::from(*episode),
                        &t::up_next_out(&say::ago(now - airs)),
                    )
                } else {
                    t::up_next_broadcast(i64::from(*episode), &calendar::time(*airs))
                };
                let dot = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                if *airs <= now && !entry_watched(entry, *episode) {
                    let water = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    water.add_css_class("state-dot");
                    water.set_valign(gtk::Align::Center);
                    dot.append(&water);
                }
                let label = widgets::haze(&line, "caption");
                label.add_css_class("numeric");
                label.set_wrap(false);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                dot.append(&label);
                card.append(&dot);
                cards.append(&opens(&card, entry, &format!("{}, {line}", entry.title)));
            }
            column.append(&cards);
        }
        if !season.is_empty() {
            let head = section(&t::continue_this_season(), &t::nav_seasonal(), "seasonal");
            head.set_margin_top(if today.is_empty() { 0 } else { 14 });
            column.append(&head);
            let posters = flow(6);
            for entry in &season {
                let poster = gtk::Box::new(gtk::Orientation::Vertical, 0);
                poster.append(&widgets::cover(hub, entry, 128, 192));
                poster.append(&caption_title(&entry.title, 8));
                posters.append(&opens(&poster, entry, &entry.title));
            }
            column.append(&posters);
        }
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&adw::Clamp::builder().maximum_size(1100).child(&column).build())
            .build();
        Some(scroll.upcast())
    }

    fn row(&self, group: &str, item: &QueueRow) -> gtk::ListBoxRow {
        let hub = &self.ctx.hub;
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.set_widget_name(&item.entry.id.get().to_string());
        line.append(&widgets::cover(hub, &item.entry, 36, 54));
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_valign(gtk::Align::Center);
        text.set_hexpand(true);
        let title = widgets::show_title(&item.entry.title, "row");
        title.set_wrap(false);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&title);
        let action = widgets::haze(&item.action, "caption");
        action.add_css_class("numeric");
        if item.aired_ago.is_some() {
            action.remove_css_class("haze");
            action.add_css_class("state");
        }
        text.append(&action);
        if let Some(fraction) = item.fraction {
            let bar = gtk::ProgressBar::new();
            bar.set_fraction(fraction);
            bar.set_margin_top(4);
            text.append(&bar);
        }
        line.append(&text);
        line.set_size_request(-1, 54);
        let row = gtk::ListBoxRow::builder().child(&line).build();
        row.set_widget_name(group);
        row.update_property(&[gtk::accessible::Property::Label(&format!(
            "{}, {}",
            item.entry.title, item.action
        ))]);

        let id = item.entry.id.get();
        let menu = gio::Menu::new();
        let play = format!("(uint32 {id}, '{}')", item.episode); // i18n-skip
        let n = item.episode;
        let first = gio::Menu::new();
        if let Some(label) = item.button.as_deref().filter(|_| item.playable) {
            first.append(Some(label), Some(&format!("win.play({play})")));
            if item.resume.is_some() {
                first.append(
                    Some(&t::episode_play_from_beginning()),
                    Some(&format!("win.play-from-start({play})")),
                );
            }
        }
        first.append(Some(&t::title_open()), Some(&format!("win.open-title(uint32 {id})"))); // i18n-skip
        menu.append_section(None, &first);
        let second = gio::Menu::new();
        second.append(
            Some(&t::episode_mark_n_watched(i64::from(n))),
            Some(&format!("win.mark-watched({play})")),
        );
        second.append(
            Some(&t::continue_hide()),
            Some(&format!("win.hide-continue(uint32 {id})")), // i18n-skip
        );
        second.append(Some(&t::continue_drop()), Some(&format!("win.drop(uint32 {id})"))); // i18n-skip
        menu.append_section(None, &second);
        widgets::context_menu(&row, &menu);
        row
    }

    /// A suggestion: the queue row's height, cover and type, with a second line that says why
    /// it is there. No bar and no water dot.
    fn up_row(&self, item: &UpNextItem) -> gtk::ListBoxRow {
        let id = item.entry.id.get();
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.set_widget_name(&id.to_string());
        line.append(&widgets::cover(&self.ctx.hub, &item.entry, 36, 54));
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_valign(gtk::Align::Center);
        text.set_hexpand(true);
        let title = widgets::show_title(&item.entry.title, "row");
        title.set_wrap(false);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&title);
        let why = say::up_next_line(item, self.engine_now(), &calendar::time);
        let reason = widgets::haze(&why, "caption");
        reason.add_css_class("numeric");
        reason.set_wrap(false);
        reason.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&reason);
        line.append(&text);
        line.set_size_request(-1, 54);
        let row = gtk::ListBoxRow::builder().child(&line).build();
        row.set_widget_name("up");
        row.update_property(&[gtk::accessible::Property::Label(&format!(
            "{}, {why}",
            item.entry.title
        ))]);

        let menu = gio::Menu::new();
        let first = gio::Menu::new();
        match item.action {
            UpNextAction::PlayFirstEpisode => first.append(
                Some(&t::title_button_play(1)),
                Some(&format!("win.play((uint32 {id}, '1'))")), // i18n-skip
            ),
            UpNextAction::AddToPlanning => first.append(
                Some(&t::up_next_add_planning()),
                Some(&format!("win.plan(uint32 {id})")), // i18n-skip
            ),
            UpNextAction::Nothing => {}
        }
        first.append(Some(&t::title_open()), Some(&format!("win.open-title(uint32 {id})"))); // i18n-skip
        menu.append_section(None, &first);
        let second = gio::Menu::new();
        second.append(
            Some(&t::up_next_not_interested()),
            Some(&format!("up.dismiss(uint32 {id})")), // i18n-skip
        );
        menu.append_section(None, &second);
        widgets::context_menu(&row, &menu);
        row
    }

    fn selected_up(&self) -> Option<UpNextItem> {
        let id = self.selected.get()?;
        self.drawn_up.borrow().iter().find(|i| i.entry.id.get() == id).cloned()
    }

    /// Return does a suggestion's one action; Delete is Not interested.
    fn suggestion_key(self: &Rc<Self>, item: &UpNextItem, key: gdk::Key) -> glib::Propagation {
        let id = item.entry.id.get();
        match key {
            gdk::Key::Return | gdk::Key::KP_Enter if !self.split.is_collapsed() => {
                let _ = match item.action {
                    UpNextAction::PlayFirstEpisode => self
                        .list
                        .activate_action("win.play", Some(&(id, "1".to_owned()).to_variant())),
                    UpNextAction::AddToPlanning => {
                        self.list.activate_action("win.plan", Some(&id.to_variant()))
                    }
                    UpNextAction::Nothing => {
                        self.list.activate_action("win.open-title", Some(&id.to_variant()))
                    }
                };
                glib::Propagation::Stop
            }
            gdk::Key::Delete => {
                self.dismiss(id);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    /// Not interested: the row goes at once, the selection moves to the next, and a notice
    /// offers Undo.
    fn dismiss(self: &Rc<Self>, id: u32) {
        let Some(at) = self.up.borrow().iter().position(|i| i.entry.id.get() == id) else {
            return;
        };
        let item = self.up.borrow_mut().remove(at);
        self.ctx.hub.send(Command::DismissUpNext { id: AnilistId::new(id) });
        let next = {
            let shown = self.drawn_up.borrow();
            let at = shown.iter().position(|i| i.entry.id.get() == id).unwrap_or(0);
            shown
                .get(at + 1)
                .or_else(|| at.checked_sub(1).and_then(|b| shown.get(b)))
                .map(|i| i.entry.id.get())
        };
        if self.selected.get() == Some(id) {
            self.selected.set(next);
        }
        let text = if item.starts.is_some() {
            t::up_next_hidden_until_airs(&item.entry.title)
        } else {
            t::up_next_hidden(&item.entry.title)
        };
        self.ctx.toast_to(&text, &t::action_undo(), "win.restore-up-next", &id.to_string());
        self.rebuild();
    }

    fn selected_item(&self) -> Option<QueueRow> {
        let id = self.selected.get()?;
        self.queue.borrow().all().find(|i| i.entry.id.get() == id).cloned()
    }

    /// Keep the selection on the same title, or the first.
    fn reselect(self: &Rc<Self>) {
        if self.split.is_collapsed() {
            return;
        }
        let want = self.selected.get();
        let mut first = None;
        let mut child = self.list.first_child();
        while let Some(widget) = child {
            if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>() {
                first.get_or_insert_with(|| row.clone());
                if row_id(row) == want {
                    self.list.select_row(Some(row));
                    self.show_preview(false);
                    return;
                }
            }
            child = widget.next_sibling();
        }
        if let Some(row) = first {
            self.list.select_row(Some(&row));
        }
    }

    /// The selection moved: the preview follows once it settles, so a selection running down
    /// the list does not crossfade every row it passes.
    fn select(self: &Rc<Self>, id: u32) {
        if self.selected.replace(Some(id)) == Some(id) && self.shown.get() == id {
            return;
        }
        if !self.episodes.borrow().contains_key(&id) {
            self.ctx.hub.send(Command::LoadEpisodes(AnilistId::new(id)));
        }
        if let Some(source) = self.settle.borrow_mut().take() {
            source.remove();
        }
        let first = self.shown.get() == 0;
        if first {
            self.show_preview(false);
            return;
        }
        let weak = Rc::downgrade(self);
        let source =
            glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                if let Some(page) = weak.upgrade() {
                    page.settle.borrow_mut().take();
                    page.show_preview(true);
                }
            });
        *self.settle.borrow_mut() = Some(source);
    }

    fn show_preview(&self, fade: bool) {
        if let Some(up) = self.selected_up() {
            return self.show_suggestion(&up, fade);
        }
        let Some(item) = self.selected_item() else { return };
        let id = item.entry.id.get();
        let name = format!(
            "{id}-{}-{}-{}",
            self.episodes.borrow().get(&id).map_or(0, Vec::len),
            item.episode,
            item.playable
        );
        if self.preview.child_by_name(&name).is_none() {
            let view = self.preview_of(&item);
            self.preview.add_named(&view, Some(&name));
        }
        self.preview.set_transition_type(if fade {
            gtk::StackTransitionType::Crossfade
        } else {
            gtk::StackTransitionType::None
        });
        let old = self.preview.visible_child();
        self.preview.set_visible_child_name(&name);
        self.preview_page.set_title(&item.entry.title);
        self.shown.set(id);
        // What was shown before goes once it has faded.
        if let Some(old) = old.filter(|o| self.preview.visible_child().as_ref() != Some(o)) {
            let stack = self.preview.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                if stack.visible_child().as_ref() != Some(&old) {
                    stack.remove(&old);
                }
            });
        }
    }

    /// The preview of a suggestion, crossfaded in like a queue row's.
    fn show_suggestion(&self, item: &UpNextItem, fade: bool) {
        let id = item.entry.id.get();
        let name = format!("u{id}-{:?}", item.action);
        if self.preview.child_by_name(&name).is_none() {
            self.preview.add_named(&self.suggestion_preview(item), Some(&name));
        }
        self.preview.set_transition_type(if fade {
            gtk::StackTransitionType::Crossfade
        } else {
            gtk::StackTransitionType::None
        });
        let old = self.preview.visible_child();
        self.preview.set_visible_child_name(&name);
        self.preview_page.set_title(&item.entry.title);
        self.shown.set(id);
        if let Some(old) = old.filter(|o| self.preview.visible_child().as_ref() != Some(o)) {
            let stack = self.preview.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                if stack.visible_child().as_ref() != Some(&old) {
                    stack.remove(&old);
                }
            });
        }
    }

    /// A suggestion's preview: the title, why it is suggested, the one amber action it has, and
    /// the synopsis. An announced title has no banner to speak of, so its cover stands beside
    /// the title instead. Signed out, an announced title has no button at all.
    fn suggestion_preview(&self, item: &UpNextItem) -> gtk::Widget {
        let hub = &self.ctx.hub;
        let entry = &item.entry;
        let id = entry.id.get();
        let announced = item.action != UpNextAction::PlayFirstEpisode;
        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        column.set_margin_start(36);
        column.set_margin_end(36);
        column.set_margin_bottom(36);

        let head = gtk::Box::new(gtk::Orientation::Vertical, 2);
        if let Some(english) = entry.title_english.as_deref().filter(|e| *e != entry.title) {
            head.append(&widgets::haze(english, ""));
        }
        let meta = say::meta_line(entry);
        if !meta.is_empty() {
            let meta = widgets::haze(&meta, "caption");
            meta.add_css_class("numeric");
            head.append(&meta);
        }
        let why = say::up_next_line(item, self.engine_now(), &calendar::time);
        let why = gtk::Label::builder().label(&why).xalign(0.0).wrap(true).build();
        why.set_margin_top(6);
        let detail = match (item.reason, item.because.as_deref()) {
            (UpNextReason::Recommended, Some(title)) => {
                Some(widgets::haze(&t::up_next_because_detail(title), ""))
            }
            _ => None,
        };

        let title = widgets::show_title(&entry.title, "title-2");
        if announced {
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 20);
            top.set_margin_top(12);
            top.append(&widgets::cover(hub, entry, 120, 180));
            let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
            text.set_valign(gtk::Align::End);
            text.append(&title);
            if head.first_child().is_some() {
                text.append(&head);
            }
            text.append(&why);
            top.append(&text);
            column.append(&top);
        } else {
            let banner = widgets::banner(hub, entry, 360, "preview");
            banner.set_halign(gtk::Align::Fill);
            banner.set_cursor_from_name(Some("pointer"));
            let open = gtk::GestureClick::new();
            open.connect_released({
                let banner = banner.clone();
                move |_, _, _, _| {
                    let _ = banner.activate_action("win.open-title", Some(&id.to_variant()));
                }
            });
            banner.add_controller(open);
            column.append(&banner);
            title.set_margin_top(12);
            column.append(&title);
            if head.first_child().is_some() {
                column.append(&head);
            }
            column.append(&why);
        }
        if let Some(detail) = detail {
            column.append(&detail);
        }

        let button = match item.action {
            UpNextAction::PlayFirstEpisode => {
                Some((t::title_button_play(1), "win.play", (id, "1".to_owned()).to_variant()))
            }
            UpNextAction::AddToPlanning => {
                Some((t::up_next_add_planning(), "win.plan", id.to_variant()))
            }
            UpNextAction::Nothing => None,
        };
        if let Some((label, action, target)) = button {
            let button = gtk::Button::with_label(&label);
            button.add_css_class("pill");
            button.add_css_class("suggested-action");
            button.set_halign(gtk::Align::Start);
            button.set_margin_top(6);
            button.set_action_name(Some(action));
            button.set_action_target_value(Some(&target));
            column.append(&button);
        }
        if !entry.synopsis.is_empty() {
            let synopsis = gtk::Label::new(Some(&entry.synopsis));
            synopsis.set_wrap(true);
            synopsis.set_xalign(0.0);
            synopsis.set_margin_top(12);
            synopsis.set_max_width_chars(80);
            column.append(&synopsis);
        }
        adw::Clamp::builder().maximum_size(720).child(&column).build().upcast()
    }

    fn preview_of(&self, item: &QueueRow) -> gtk::Widget {
        let hub = &self.ctx.hub;
        let entry = &item.entry;
        let id = entry.id.get();
        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        column.set_margin_start(36);
        column.set_margin_end(36);
        column.set_margin_bottom(36);

        let banner = widgets::banner(hub, entry, 360, "preview");
        banner.set_halign(gtk::Align::Fill);
        banner.set_cursor_from_name(Some("pointer"));
        banner.upcast_ref::<gtk::Widget>().update_property(&[
            gtk::accessible::Property::Label(&t::title_open_named(&entry.title)),
        ]);
        let open = gtk::GestureClick::new();
        open.connect_released({
            let banner = banner.clone();
            move |_, _, _, _| {
                let _ = banner.activate_action("win.open-title", Some(&id.to_variant()));
            }
        });
        banner.add_controller(open);
        column.append(&banner);

        let title = widgets::show_title(&entry.title, "title-2");
        title.set_margin_top(12);
        column.append(&title);
        let heading = gtk::Box::new(gtk::Orientation::Vertical, 2);
        if let Some(english) = entry.title_english.as_deref().filter(|e| *e != entry.title) {
            heading.append(&widgets::haze(english, ""));
        }
        let meta = say::meta_line(entry);
        if !meta.is_empty() {
            let meta = widgets::haze(&meta, "caption");
            meta.add_css_class("numeric");
            heading.append(&meta);
        }
        if heading.first_child().is_some() {
            column.append(&heading);
        }

        let episode_title = self
            .episodes
            .borrow()
            .get(&id)
            .and_then(|rows| {
                rows.iter().find(|r| r.number == item.episode.to_string()).cloned()
            })
            .and_then(|r| r.title);
        let dubbed = item.mark.is_some();
        let line = match episode_title {
            Some(name) if dubbed => {
                t::episode_with_title_dubbed(i64::from(item.episode), &name)
            }
            Some(name) => t::episode_with_title(i64::from(item.episode), &name),
            None => say::marked(t::episode_number(i64::from(item.episode)), dubbed),
        };
        let episode = gtk::Label::new(Some(&line));
        episode.add_css_class("title-4");
        episode.set_xalign(0.0);
        episode.set_wrap(true);
        column.append(&episode);

        if let Some(fraction) = item.fraction {
            let meter = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let bar = gtk::ProgressBar::new();
            bar.set_fraction(fraction);
            bar.set_hexpand(true);
            bar.set_valign(gtk::Align::Center);
            meter.append(&bar);
            if let Some(left) = item.left.clone() {
                let left = widgets::haze(&left, "caption");
                left.add_css_class("numeric");
                left.set_wrap(false);
                meter.append(&left);
            }
            meter.set_size_request(-1, 20);
            column.append(&meter);
        }

        match (item.button.clone(), item.waiting.clone()) {
            (Some(label), _) => {
                let button = gtk::Button::with_label(&label);
                button.add_css_class("pill");
                button.add_css_class("suggested-action");
                button.set_halign(gtk::Align::Start);
                button.set_margin_top(6);
                button.set_action_name(Some("win.play"));
                button.set_action_target_value(Some(
                    &(id, item.episode.to_string()).to_variant(),
                ));
                column.append(&button);
            }
            (None, Some(waiting)) => column.append(&widgets::haze(&waiting, "")),
            (None, None) => {}
        }

        if !entry.synopsis.is_empty() {
            let synopsis = gtk::Label::new(Some(&entry.synopsis));
            synopsis.set_wrap(true);
            synopsis.set_xalign(0.0);
            synopsis.set_margin_top(12);
            synopsis.set_max_width_chars(80);
            column.append(&synopsis);
        }
        let clamp = adw::Clamp::builder().maximum_size(720).child(&column).build();
        clamp.upcast()
    }
}

/// The title a queue row is for.
fn row_id(row: &gtk::ListBoxRow) -> Option<u32> {
    row.child()?.widget_name().parse().ok()
}

/// A card under Airing today is a quarter of the content's width at its widest.
const CARD_WIDTH: i32 = 232;

/// The wall clock, in Unix seconds.
fn now() -> i64 {
    glib::DateTime::now_utc().map_or(0, |t| t.to_unix())
}

/// A section's heading with the pill that opens the page it is a part of.
fn section(title: &str, button: &str, dest: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let heading = widgets::heading(title);
    heading.set_hexpand(true);
    row.append(&heading);
    let open = gtk::Button::with_label(button);
    open.add_css_class("pill");
    open.set_action_name(Some("win.go"));
    open.set_action_target_value(Some(&dest.to_variant()));
    row.append(&open);
    row
}

/// A grid of at most `columns` across that gives each cell the same room.
fn flow(columns: u32) -> gtk::FlowBox {
    let flow = gtk::FlowBox::new();
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(false);
    flow.set_max_children_per_line(columns);
    flow.set_column_spacing(16);
    flow.set_row_spacing(16);
    flow.set_valign(gtk::Align::Start);
    // Takes the room its cells ask for, so they sit together at the start.
    flow.set_halign(gtk::Align::Start);
    flow
}

/// A title under its art, cut at one line.
fn caption_title(text: &str, top: i32) -> gtk::Label {
    let title = widgets::show_title(text, "row");
    title.set_wrap(false);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    // Asks for no width of its own: the art above sets it, and the title is cut to fit.
    title.set_max_width_chars(1);
    title.set_margin_top(top);
    title
}

/// `card` as a flat button that opens the title, read as `label`.
fn opens(card: &gtk::Box, entry: &Entry, label: &str) -> gtk::Widget {
    let id = entry.id.get();
    let button = gtk::Button::builder().child(card).build();
    button.add_css_class("flat");
    button.set_halign(gtk::Align::Start);
    button.set_action_name(Some("win.open-title"));
    button.set_action_target_value(Some(&id.to_variant()));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    widgets::title_menu(&button, id, true);
    button.upcast()
}

/// Whether the viewer has watched `episode` of the title.
fn entry_watched(entry: &Entry, episode: u32) -> bool {
    entry.progress.as_ref().is_some_and(|p| p.done >= episode)
}
