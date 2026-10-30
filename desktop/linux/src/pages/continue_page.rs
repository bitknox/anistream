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
};
use gtk::{gdk, gio, glib};

use crate::{
    nagare, say,
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
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let list = gtk::ListBox::new();
    list.add_css_class("queue");
    list.set_selection_mode(gtk::SelectionMode::Browse);
    list.set_activate_on_single_click(false);
    list.set_header_func(|row, before| {
        let group = row.widget_name();
        let changed = before.is_none_or(|b| b.widget_name() != group);
        if changed {
            let text =
                if group == "new" { t::group_new_this_week() } else { t::group_in_progress() };
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
    let body = States::new(&ctx.hub, &scroll, rows, &empty, &t::continue_load_failed());
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
    });
    page.wire();
    ctx.hub.send(Command::LoadContinue);
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
                    *self.ids.borrow_mut() = entries.iter().map(|e| e.id.get()).collect();
                    self.rebuild();
                }
                ContentList::Empty => {
                    self.ids.borrow_mut().clear();
                    self.rebuild();
                }
                ContentList::Failed(why) => {
                    tracing::warn!(%why, "continue did not load");
                    self.body.failed(why);
                }
            },
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
            Event::Episodes { id, rows, .. } => {
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

    /// The queue from what is known of each title, in the engine's order.
    fn rebuild(self: &Rc<Self>) {
        let entries: Vec<Entry> = {
            let known = self.ctx.hub.known.borrow();
            self.ids.borrow().iter().filter_map(|id| known.entries.get(id).cloned()).collect()
        };
        let queue = say::queue(&entries, &[], &self.episodes.borrow());
        let same = *self.queue.borrow() == queue;
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
        if queue.is_empty() {
            self.body.empty();
        } else {
            self.body.content();
        }
        self.layout();
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
