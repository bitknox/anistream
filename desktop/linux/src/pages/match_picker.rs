//! The match picker for one title: which of a source's titles a show is, when the source could
//! not tell or got it wrong. The titles the source offered, a search of the source by hand and
//! Match automatically to forget a choice. A dialog, which a narrow window shows as a bottom
//! sheet. It opens from the title page's Choose the match, from Fix this match in the More menu
//! and from the source picker's last row, and closes itself when a pick settles.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use anistream_core::ids::{AnilistId, ProviderKey};
use anistream_i18n::t;
use anistream_model::{command::Command, event::Event, sources::MatchCandidate};
use gtk::glib;

use crate::{
    matching::{self, Search},
    widgets,
    window::Ctx,
};

/// A search or pick waits this long before it says so with a spinner, so a quick answer does not
/// flash one.
const SILENCE: Duration = Duration::from_millis(600);

/// What the list area holds.
enum Shown {
    Candidates { provider_id: String, source: String, candidates: Vec<MatchCandidate> },
    Searching { source: String, query: String },
    Empty { source: String, query: String },
    Failed { source: String, reason: String },
    Nothing,
}

struct Picker {
    ctx: Rc<Ctx>,
    id: AnilistId,
    dialog: adw::Dialog,
    field: gtk::SearchEntry,
    body: gtk::Box,
    automatic: gtk::Button,
    /// The serial of the newest pick settled when the dialog opened: a newer one closes it.
    settled_before: u32,
    /// The row Return picks: the best one at first.
    selected: RefCell<Option<ProviderKey>>,
    /// The search has run 600 ms: the searching row shows.
    search_slow: Cell<bool>,
    /// The pick has run 600 ms: its row says Matching.
    pick_slow: Cell<bool>,
    /// Timers for the two, so a later search or pick starts its own count.
    search_timer: RefCell<Option<glib::SourceId>>,
    pick_timer: RefCell<Option<glib::SourceId>>,
    rows: RefCell<Vec<(ProviderKey, gtk::ListBoxRow)>>,
}

/// Open the match picker for title `id` over `parent`. `from_menu` is Fix this match, which
/// fills the field with the title and searches at once; the page's question shows what the
/// source offered instead. `query` fills the field with other words, for a shot.
pub fn open(
    ctx: &Rc<Ctx>,
    parent: &impl IsA<gtk::Widget>,
    id: AnilistId,
    from_menu: bool,
    query: Option<&str>,
) -> adw::Dialog {
    let entry = ctx.hub.entry(id);
    let title = entry.as_ref().map(|e| e.title.clone()).unwrap_or_default();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_title_widget(Some(&adw::WindowTitle::new(&t::match_choose(), &title)));
    let cancel = gtk::Button::with_label(&t::action_cancel());
    header.pack_start(&cancel);

    let source_name = source(ctx, id).map(|s| s.1).unwrap_or_default();
    let field = gtk::SearchEntry::builder()
        .placeholder_text(t::match_search_field(&source_name))
        .hexpand(true)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    field.update_property(&[gtk::accessible::Property::Label(&t::match_search_field(
        &source_name,
    ))]);

    let top = gtk::Box::new(gtk::Orientation::Vertical, 0);
    top.append(&field);
    if let Some(entry) = &entry {
        let facts = matching::anilist(entry);
        if !facts.is_empty() {
            let line = widgets::haze(&facts, "numeric");
            line.set_margin_start(14);
            line.set_margin_end(14);
            line.set_margin_bottom(6);
            top.append(&line);
        }
    }

    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_start(12);
    body.set_margin_end(12);
    body.set_margin_top(6);
    body.set_margin_bottom(12);
    let clamp = adw::Clamp::builder().maximum_size(560).child(&body).build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build();

    let automatic = gtk::Button::with_label(&t::match_automatic());
    automatic.add_css_class("pill");
    automatic.set_tooltip_text(Some(&t::match_automatic_detail()));
    automatic.set_visible(false);
    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bottom.set_halign(gtk::Align::Center);
    bottom.set_margin_top(8);
    bottom.set_margin_bottom(8);
    bottom.append(&automatic);

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.add_top_bar(&top);
    view.add_bottom_bar(&bottom);
    view.set_bottom_bar_style(adw::ToolbarStyle::Raised);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .title(t::match_choose())
        .content_width(480)
        .content_height(620)
        .child(&view)
        .build();
    dialog.set_focus(Some(&field));

    let picker = Rc::new(Picker {
        ctx: Rc::clone(ctx),
        id,
        dialog: dialog.clone(),
        field: field.clone(),
        body,
        automatic: automatic.clone(),
        settled_before: ctx
            .hub
            .known
            .borrow()
            .matches
            .settled()
            .filter(|s| s.id == id)
            .map_or(0, |s| s.serial),
        selected: RefCell::default(),
        search_slow: Cell::new(false),
        pick_slow: Cell::new(false),
        search_timer: RefCell::default(),
        pick_timer: RefCell::default(),
        rows: RefCell::default(),
    });

    cancel.connect_clicked({
        let dialog = dialog.downgrade();
        move |_| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    });
    automatic.connect_clicked({
        let weak = Rc::downgrade(&picker);
        move |_| {
            if let Some(picker) = weak.upgrade() {
                picker.match_automatically();
            }
        }
    });
    // Return searches new words, or picks the selected row when the words are the search's.
    field.connect_activate({
        let weak = Rc::downgrade(&picker);
        move |_| {
            if let Some(picker) = weak.upgrade() {
                picker.submitted();
            }
        }
    });
    field.connect_search_changed({
        let weak = Rc::downgrade(&picker);
        move |field| {
            // An emptied field has nothing to search for: the question or the list stands.
            if let Some(picker) = weak.upgrade()
                && field.text().is_empty()
            {
                picker.refresh();
            }
        }
    });
    // Up and Down move through the rows while the focus stays in the field.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let weak = Rc::downgrade(&picker);
        move |_, key, _, _| {
            let Some(picker) = weak.upgrade() else { return glib::Propagation::Proceed };
            match key {
                gtk::gdk::Key::Down => picker.step(1),
                gtk::gdk::Key::Up => picker.step(-1),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        }
    });
    field.add_controller(keys);

    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&picker);
        move |event| {
            if let Some(picker) = weak.upgrade() {
                picker.heard(event);
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed({
        let weak = Rc::downgrade(&picker);
        move |_| {
            if let Some(hub) = hub.upgrade() {
                hub.forget(listening);
                hub.known.borrow_mut().matches.closed(id);
            }
            if let Some(picker) = weak.upgrade() {
                if let Some(timer) = picker.search_timer.take() {
                    timer.remove();
                }
                if let Some(timer) = picker.pick_timer.take() {
                    timer.remove();
                }
            }
        }
    });
    widgets::keep(&dialog, Rc::clone(&picker));

    if let Some(query) = query {
        field.set_text(query);
    } else if from_menu && !title.is_empty() {
        // From the menu: the AniList title, searched at once.
        field.set_text(&title);
    }
    picker.first_pick();
    if query.is_some() || from_menu {
        picker.search();
    }
    picker.refresh();
    dialog.present(Some(parent));
    dialog
}

/// The source the picker searches, as `(provider id, name)`: the one that asked or is matched,
/// else the first one in use.
fn source(ctx: &Ctx, id: AnilistId) -> Option<(String, String)> {
    let known = ctx.hub.known.borrow();
    known.matches.source_for(id).or_else(|| {
        known.ordered_providers().into_iter().find(|p| p.usable).map(|p| (p.id, p.name))
    })
}

impl Picker {
    fn heard(self: &Rc<Self>, event: &Event) {
        let mine = match event {
            Event::MatchChoices { id, .. }
            | Event::MatchSearchFailed { id, .. }
            | Event::MatchFixFailed { id, .. }
            | Event::TitleMatch { id, .. } => *id == self.id,
            _ => false,
        };
        if !mine {
            return;
        }
        let settled = self
            .ctx
            .hub
            .known
            .borrow()
            .matches
            .settled()
            .is_some_and(|s| s.id == self.id && s.serial > self.settled_before);
        if settled {
            self.dialog.close();
            return;
        }
        self.first_pick();
        self.refresh();
    }

    fn shown(&self) -> Shown {
        let known = self.ctx.hub.known.borrow();
        let book = &known.matches;
        let name = source(&self.ctx, self.id).map(|s| s.1).unwrap_or_default();
        match book.search(self.id) {
            Some(Search::Searching { query }) => {
                Shown::Searching { source: name, query: query.clone() }
            }
            Some(Search::Found { provider_id, source, candidates, .. }) => Shown::Candidates {
                provider_id: provider_id.clone(),
                source: source.clone(),
                candidates: candidates.clone(),
            },
            Some(Search::Empty { query, source }) => {
                Shown::Empty { source: source.clone(), query: query.clone() }
            }
            Some(Search::Failed { reason, .. }) => {
                Shown::Failed { source: name, reason: reason.clone() }
            }
            None => match book.asked(self.id) {
                Some(asked) => Shown::Candidates {
                    provider_id: asked.provider_id.clone(),
                    source: asked.source.clone(),
                    candidates: asked.candidates.clone(),
                },
                None => Shown::Nothing,
            },
        }
    }

    /// The best row is selected until the person moves: the first one not ruled out.
    fn first_pick(&self) {
        let valid = |key: &ProviderKey, candidates: &[MatchCandidate]| {
            candidates.iter().any(|c| &c.key == key)
        };
        if let Shown::Candidates { candidates, .. } = self.shown() {
            let keep = self.selected.borrow().as_ref().is_some_and(|k| valid(k, &candidates));
            if !keep {
                let best =
                    candidates.iter().find(|c| c.rejected.is_none()).or(candidates.first());
                self.selected.replace(best.map(|c| c.key.clone()));
            }
        } else {
            self.selected.replace(None);
        }
    }

    fn search(self: &Rc<Self>) {
        let query = self.field.text().trim().to_owned();
        if query.is_empty() {
            return;
        }
        let Some((provider_id, _)) = source(&self.ctx, self.id) else { return };
        self.ctx.hub.known.borrow_mut().matches.searching(self.id, &query);
        self.selected.replace(None);
        self.search_slow.set(false);
        if let Some(old) = self.search_timer.take() {
            old.remove();
        }
        let weak = Rc::downgrade(self);
        self.search_timer.replace(Some(glib::timeout_add_local_once(SILENCE, move || {
            if let Some(picker) = weak.upgrade() {
                picker.search_timer.replace(None);
                picker.search_slow.set(true);
                picker.refresh();
            }
        })));
        self.ctx.hub.send(Command::ManualSearch {
            id: self.id,
            query,
            provider_id: Some(provider_id),
        });
        self.refresh();
    }

    /// Return in the field: new words search, the same ones pick the selected row.
    fn submitted(self: &Rc<Self>) {
        let typed = self.field.text().trim().to_owned();
        let searched =
            self.ctx.hub.known.borrow().matches.search(self.id).map(|s| s.query().to_owned());
        if !typed.is_empty() && searched.as_deref() != Some(typed.as_str()) {
            self.search();
        } else if let Shown::Candidates { provider_id, source, candidates } = self.shown() {
            let selected = self.selected.borrow().clone();
            if let Some(candidate) =
                candidates.iter().find(|c| Some(&c.key) == selected.as_ref())
            {
                self.pick(candidate, &provider_id, &source);
            }
        }
    }

    fn pick(self: &Rc<Self>, candidate: &MatchCandidate, provider_id: &str, source: &str) {
        if self.ctx.hub.known.borrow().matches.picking(self.id).is_some() {
            return;
        }
        self.selected.replace(Some(candidate.key.clone()));
        self.ctx.hub.known.borrow_mut().matches.pick(self.id, candidate, source);
        self.pick_slow.set(false);
        let weak = Rc::downgrade(self);
        if let Some(old) = self.pick_timer.take() {
            old.remove();
        }
        self.pick_timer.replace(Some(glib::timeout_add_local_once(SILENCE, move || {
            if let Some(picker) = weak.upgrade() {
                picker.pick_timer.replace(None);
                picker.pick_slow.set(true);
                picker.refresh();
            }
        })));
        self.ctx.hub.send(Command::FixMatch {
            id: self.id,
            provider_id: provider_id.to_owned(),
            key: candidate.key.clone(),
            title: Some(candidate.title.clone()),
        });
        self.refresh();
    }

    fn match_automatically(&self) {
        self.ctx.hub.known.borrow_mut().matches.cleared(self.id);
        self.ctx.toast(&t::match_cleared());
        self.ctx.hub.send(Command::ClearMatch { id: self.id });
        self.dialog.close();
    }

    /// Move the selected row by `step`, keeping the field focused.
    fn step(&self, step: isize) {
        let rows = self.rows.borrow();
        if rows.is_empty() {
            return;
        }
        let selected = self.selected.borrow().clone();
        let at = rows.iter().position(|(key, _)| Some(key) == selected.as_ref());
        let next = match at {
            Some(at) => (at as isize + step).clamp(0, rows.len() as isize - 1) as usize,
            None if step > 0 => 0,
            None => rows.len() - 1,
        };
        let (key, row) = &rows[next];
        self.selected.replace(Some(key.clone()));
        if let Some(list) = row.parent().and_downcast::<gtk::ListBox>() {
            list.select_row(Some(row));
        }
    }

    /// The list area from what the book holds now.
    fn refresh(self: &Rc<Self>) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        self.rows.borrow_mut().clear();
        let (now, picking, failed, asks) = {
            let known = self.ctx.hub.known.borrow();
            let book = &known.matches;
            (
                book.current(self.id).cloned(),
                book.picking(self.id).cloned(),
                book.pick_failed(self.id).map(str::to_owned),
                book.asks(self.id),
            )
        };
        self.automatic.set_visible(now.as_ref().is_some_and(|m| m.chosen_by_you));

        // The match there is now, when the sheet was not opened by a question.
        if let (Some(now), false) = (&now, asks) {
            self.body.append(&heading(&t::match_now()));
            let row = adw::ActionRow::builder()
                .use_markup(false)
                .title(glib::markup_escape_text(now.title.as_deref().unwrap_or(&now.source)))
                .build();
            let how = if now.chosen_by_you {
                t::match_chosen_by_you()
            } else {
                t::match_chosen_automatically()
            };
            row.add_suffix(&widgets::haze(&how, ""));
            let list = boxed();
            list.append(&row);
            list.set_margin_bottom(10);
            self.body.append(&list);
        }

        match self.shown() {
            Shown::Candidates { provider_id, source, candidates } => {
                self.body.append(&heading(&t::match_section_source(&source)));
                let list = boxed();
                list.set_selection_mode(gtk::SelectionMode::Single);
                for candidate in &candidates {
                    let row = self.row(candidate, picking.as_ref());
                    let weak = Rc::downgrade(self);
                    let (candidate_for, provider_id, source) =
                        (candidate.clone(), provider_id.clone(), source.clone());
                    row.connect_activated(move |_| {
                        if let Some(picker) = weak.upgrade() {
                            picker.pick(&candidate_for, &provider_id, &source);
                        }
                    });
                    list.append(&row);
                    self.rows.borrow_mut().push((candidate.key.clone(), row.upcast()));
                }
                let selected = self.selected.borrow().clone();
                if let Some((_, row)) =
                    self.rows.borrow().iter().find(|(key, _)| Some(key) == selected.as_ref())
                {
                    list.select_row(Some(row));
                }
                self.body.append(&list);
                if let Some(reason) = failed {
                    self.body.append(&said(true, &t::match_fix_failed(), &reason, None));
                }
            }
            Shown::Searching { source, query } => {
                self.body.append(&heading(&t::match_section_source(&source)));
                let wait = gtk::Box::new(gtk::Orientation::Vertical, 10);
                wait.set_valign(gtk::Align::Center);
                wait.set_height_request(200);
                // Nothing shows for the first 600 ms, so a quick answer does not flash.
                wait.set_opacity(if self.search_slow.get() { 1.0 } else { 0.0 });
                let spinner = adw::Spinner::new();
                spinner.set_size_request(24, 24);
                spinner.set_halign(gtk::Align::Center);
                wait.append(&spinner);
                let line = widgets::haze(&t::match_searching(&source, &query), "");
                line.set_justify(gtk::Justification::Center);
                line.set_xalign(0.5);
                wait.append(&line);
                self.body.append(&wait);
            }
            Shown::Empty { source, query } => {
                self.body.append(&said(
                    false,
                    &t::match_search_empty(&source, &query),
                    &t::match_search_empty_hint(),
                    None,
                ));
            }
            Shown::Failed { source, reason } => {
                let retry = Rc::downgrade(self);
                self.body.append(&said(
                    true,
                    &t::match_search_failed(&source),
                    &reason,
                    Some(Box::new(move || {
                        if let Some(picker) = retry.upgrade() {
                            picker.search();
                        }
                    })),
                ));
            }
            Shown::Nothing => {}
        }
    }

    /// One candidate as a row: its name and facts, and how close it is or why it was ruled out.
    /// While a pick runs the others dim, and the one picked says Matching after 600 ms.
    fn row(&self, candidate: &MatchCandidate, picking: Option<&ProviderKey>) -> adw::ActionRow {
        let facts = matching::facts(candidate);
        let closeness = matching::closeness(candidate);
        let picked = picking == Some(&candidate.key);
        let row = adw::ActionRow::builder()
            .use_markup(false)
            .title(glib::markup_escape_text(&candidate.title))
            .subtitle(glib::markup_escape_text(&facts))
            .activatable(picking.is_none())
            .build();
        let end = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        end.set_valign(gtk::Align::Center);
        if picked && self.pick_slow.get() {
            let spinner = adw::Spinner::new();
            spinner.set_size_request(16, 16);
            end.append(&spinner);
        }
        let words =
            if picked && self.pick_slow.get() { t::match_picking() } else { closeness.clone() };
        let label = widgets::haze(&words, "numeric");
        label.set_wrap(false);
        end.append(&label);
        row.add_suffix(&end);
        if picking.is_some() && !picked {
            row.set_sensitive(false);
        }
        if candidate.rejected.is_some() {
            row.add_css_class("noted");
        }
        row.upcast_ref::<gtk::Widget>().update_property(&[gtk::accessible::Property::Label(
            &t::match_row_label(&candidate.title, &facts, &closeness),
        )]);
        row
    }
}

fn heading(text: &str) -> gtk::Label {
    let label = widgets::heading(text);
    label.remove_css_class("title-4");
    label.add_css_class("heading");
    label.add_css_class("haze");
    label.set_margin_start(6);
    label
}

fn boxed() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
}

/// A state said in the list's place: no results, or a failure with its reason and a way to try
/// again.
fn said(
    dot: bool,
    text: &str,
    detail: &str,
    retry: Option<Box<dyn Fn() + 'static>>,
) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.set_valign(gtk::Align::Center);
    column.set_height_request(200);
    let head = if dot {
        widgets::standing(text, "vermilion")
    } else {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let label = gtk::Label::new(Some(text));
        label.set_wrap(true);
        label.set_justify(gtk::Justification::Center);
        line.append(&label);
        line
    };
    head.set_halign(gtk::Align::Center);
    head.add_css_class("heading");
    column.append(&head);
    let detail = widgets::haze(detail, "");
    detail.set_justify(gtk::Justification::Center);
    detail.set_xalign(0.5);
    column.append(&detail);
    if let Some(retry) = retry {
        let button = gtk::Button::with_label(&t::action_try_again());
        button.add_css_class("pill");
        button.set_halign(gtk::Align::Center);
        button.set_margin_top(6);
        button.connect_clicked(move |_| retry());
        column.append(&button);
    }
    column
}
