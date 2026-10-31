//! The source picker for one title, from the title page's More menu and the show settings: which
//! source its episodes come from, or one release of a source. Automatic first, then every source
//! in the order they are tried, each with how much of the title it has or why it has nothing, and
//! under a source with several releases a group of them. A pick is sent at once and closes the
//! dialog; the episode table follows from the engine's events. A dialog, which a narrow window
//! shows as a bottom sheet.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{command::Command, event::Event, sources::SourceOption};
use gtk::glib;

use crate::{
    choices::{self, Row},
    hub::Hub,
    window::Ctx,
};

struct Picker {
    ctx: Rc<Ctx>,
    id: AnilistId,
    dialog: adw::Dialog,
    page: adw::PreferencesPage,
    /// The Automatic row's check; the row itself never goes.
    automatic: gtk::Image,
    automatic_row: adw::ActionRow,
    /// The groups under Automatic, replaced whenever the engine lists the sources again.
    groups: RefCell<Vec<adw::PreferencesGroup>>,
    /// The row holding the check, when it is not Automatic's.
    chosen: RefCell<Option<adw::ActionRow>>,
    /// Whether the listing has come, so a spinner shows only while it is awaited.
    listed: Cell<bool>,
}

/// Open the picker for title `id`, over `parent`'s window.
pub fn open(ctx: &Rc<Ctx>, parent: &impl IsA<gtk::Widget>, id: AnilistId) -> adw::Dialog {
    let title = ctx.hub.entry(id).map(|e| e.title).unwrap_or_default();

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    let automatic_row = adw::ActionRow::builder().use_markup(false).activatable(true).build();
    let automatic = check();
    automatic_row.add_suffix(&automatic);
    group.add(&automatic_row);
    page.add(&group);

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_title_widget(Some(&adw::WindowTitle::new(&t::source_picker_title(), &title)));
    let done = gtk::Button::with_label(&t::action_done());
    header.pack_end(&done);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .title(t::source_picker_title())
        .content_width(480)
        .content_height(620)
        .child(&view)
        .build();
    done.connect_clicked({
        let dialog = dialog.downgrade();
        move |_| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.close();
            }
        }
    });

    let picker = Rc::new(Picker {
        ctx: Rc::clone(ctx),
        id,
        dialog: dialog.clone(),
        page,
        automatic,
        automatic_row,
        groups: RefCell::default(),
        chosen: RefCell::default(),
        listed: Cell::new(false),
    });
    picker.automatic_row.connect_activated({
        let weak = Rc::downgrade(&picker);
        move |_| {
            if let Some(picker) = weak.upgrade() {
                picker.pick(&Row::Automatic { chosen: picker.automatic.is_visible() });
            }
        }
    });

    // What was listed last time shows at once; the engine's fresh listing replaces it.
    let known = ctx.hub.known.borrow().source_options.get(&id.get()).cloned();
    picker.show(known.as_deref());
    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&picker);
        move |event| {
            if let (Some(picker), Event::SourceOptions { id, options }) =
                (weak.upgrade(), event)
                && *id == picker.id
            {
                picker.show(Some(options));
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    // The spinner waits a moment, so a quick answer does not flash it.
    glib::timeout_add_local_once(std::time::Duration::from_millis(300), {
        let weak = Rc::downgrade(&picker);
        move || {
            if let Some(picker) = weak.upgrade()
                && !picker.listed.get()
            {
                picker.loading();
            }
        }
    });
    crate::widgets::keep(&dialog, Rc::clone(&picker));
    ctx.hub.send(Command::LoadSourceOptions(id));
    // A pick applies at once, so a click outside loses nothing.
    crate::widgets::present_light_dismiss(&dialog, parent);
    picker.focus_chosen();
    dialog
}

/// The check on the chosen row.
fn check() -> gtk::Image {
    let image = gtk::Image::from_icon_name("object-select-symbolic");
    image.set_valign(gtk::Align::Center);
    image
}

impl Picker {
    /// The rows as `options` stand, in place of any before. `None` while the listing is awaited.
    fn show(self: &Rc<Self>, options: Option<&[SourceOption]>) {
        self.listed.set(options.is_some());
        for group in self.groups.take() {
            self.page.remove(&group);
        }
        let rows = choices::rows(options);
        let automatic = rows.first().cloned().unwrap_or(Row::Automatic { chosen: true });
        self.dress(&self.automatic_row, &self.automatic, &automatic);
        self.chosen.replace(None);

        let sources = adw::PreferencesGroup::builder().title(t::sources_title()).build();
        let mut sources_in = false;
        let mut releases: Option<adw::PreferencesGroup> = None;
        let mut later = Vec::new();
        for row in rows.iter().skip(1) {
            match row {
                Row::Source { .. } => {
                    sources.add(&self.row(row));
                    sources_in = true;
                }
                Row::Releases { provider_id } => {
                    let name = options
                        .and_then(|o| o.iter().find(|o| &o.provider_id == provider_id))
                        .map(|o| o.name.clone())
                        .unwrap_or_default();
                    let group = adw::PreferencesGroup::builder()
                        .title(t::source_picker_releases())
                        .description(glib::markup_escape_text(&name))
                        .build();
                    later.push(group.clone());
                    releases = Some(group);
                }
                Row::Release { .. } => {
                    if let Some(group) = &releases {
                        group.add(&self.row(row));
                    }
                }
                Row::Automatic { .. } => {}
            }
        }
        let mut shown = Vec::new();
        if sources_in {
            self.page.add(&sources);
            shown.push(sources);
        }
        for group in later {
            self.page.add(&group);
            shown.push(group);
        }
        self.groups.replace(shown);
        self.focus_chosen();
    }

    /// Focus the chosen row, so the keyboard starts where the choice is and a long list scrolls
    /// to it. Left alone once the viewer has moved to a row of their own.
    fn focus_chosen(&self) {
        let automatic: &gtk::Widget = self.automatic_row.upcast_ref();
        let moved =
            self.dialog.focus().is_some_and(|w| w != *automatic && w.is_ancestor(&self.page));
        if moved {
            return;
        }
        let chosen = self.chosen.borrow().clone().unwrap_or_else(|| self.automatic_row.clone());
        // Once the rows have a place, so the list can scroll to the one focused.
        glib::idle_add_local_once(move || {
            chosen.grab_focus();
        });
    }

    /// The spinner, under Automatic, while the sources are being listed.
    fn loading(&self) {
        let spinner = adw::Spinner::new();
        spinner.set_size_request(24, 24);
        spinner.set_halign(gtk::Align::Center);
        spinner.set_margin_top(12);
        spinner.set_margin_bottom(12);
        let group = adw::PreferencesGroup::new();
        group.add(&spinner);
        self.page.add(&group);
        self.groups.borrow_mut().push(group);
    }

    /// One source or release as a row: its words, its check when chosen, dimmed when the source
    /// has a note in place of a count.
    fn row(self: &Rc<Self>, row: &Row) -> adw::ActionRow {
        let widget = adw::ActionRow::builder().use_markup(false).build();
        let mark = check();
        widget.add_suffix(&mark);
        self.dress(&widget, &mark, row);
        if row.chosen() {
            self.chosen.replace(Some(widget.clone()));
        }
        if row.selectable() {
            widget.connect_activated({
                let weak = Rc::downgrade(self);
                let row = row.clone();
                move |_| {
                    if let Some(picker) = weak.upgrade() {
                        picker.pick(&row);
                    }
                }
            });
        }
        widget
    }

    /// Set a row's text, check, dimming and what a screen reader says.
    fn dress(&self, widget: &adw::ActionRow, mark: &gtk::Image, row: &Row) {
        let (title, detail) = choices::words(row);
        widget.set_title(&title);
        widget.set_subtitle(detail.as_deref().unwrap_or_default());
        widget.set_activatable(row.selectable());
        mark.set_visible(row.chosen());
        if row.dimmed() {
            widget.add_css_class("noted");
        } else {
            widget.remove_css_class("noted");
        }
        let accessible = widget.upcast_ref::<gtk::Widget>();
        accessible.update_property(&[gtk::accessible::Property::Label(&choices::spoken(row))]);
        accessible.update_state(&[gtk::accessible::State::Selected(Some(row.chosen()))]);
    }

    /// A pick: sent at once, shown at once, and the dialog closes. The row already chosen only
    /// closes it.
    fn pick(&self, row: &Row) {
        if !row.chosen() {
            send(&self.ctx.hub, self.id, row);
        }
        self.dialog.close();
    }
}

/// The command a pick sends, and the listing it leaves behind.
fn send(hub: &Hub, id: AnilistId, row: &Row) {
    match row {
        Row::Automatic { .. } => {
            hub.send(Command::SetProviderPreference { id, provider: None });
            hub.known.borrow_mut().choose(id, None, None);
        }
        Row::Source { option, .. } => {
            hub.send(Command::SetProviderPreference {
                id,
                provider: Some(option.provider_id.clone()),
            });
            hub.known.borrow_mut().choose(id, Some(&option.provider_id), None);
        }
        Row::Release { provider_id, candidate, .. } => {
            hub.send(Command::PinRelease {
                id,
                provider_id: provider_id.clone(),
                release_id: candidate.id.clone(),
                title: candidate.title.clone(),
            });
            hub.known.borrow_mut().choose(id, Some(provider_id), Some(&candidate.id));
        }
        Row::Releases { .. } => {}
    }
}
