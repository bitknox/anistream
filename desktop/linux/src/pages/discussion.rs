//! An episode's discussion: AniList's forum thread in an `AdwDialog`, which is a sheet on a
//! narrow window. The engine says the whole discussion on every change, so the sheet draws it
//! again each time, keeping the scroll place and what the viewer opened (spoilers and folded
//! replies). Opening it from the player pauses playback there; closing it does not resume.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{
    command::Command,
    discussion::{
        CommentBlock, CommentSpan, CommentView, DiscussionState, DiscussionView, MediaKind,
        PostState,
    },
    event::Event,
};
use gtk::{gio, glib};

use crate::{motion, say, states, widgets, window::Ctx};

/// Nothing shows for this long, then the placeholders.
const QUIET: Duration = Duration::from_millis(600);
/// After this long the placeholders say they are still waiting.
const SLOW: Duration = Duration::from_secs(4);
/// A comment longer than this many lines is cut off behind Show more.
const FOLD_LINES: i32 = 10;

/// What the field at the foot is for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    New,
    Reply { parent: u32, name: String },
    Edit { id: u32 },
}

struct Sheet {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    media: AnilistId,
    episode: u32,
    stack: gtk::Stack,
    scroller: gtk::ScrolledWindow,
    column: gtk::Box,
    status: gtk::Box,
    waiting_note: gtk::Label,
    count: gtk::Label,
    open: gtk::Button,
    note: gtk::Label,
    sign_in: gtk::Button,
    offline: gtk::Label,
    mode_bar: gtk::Box,
    mode_label: gtk::Label,
    entry: gtk::Entry,
    post: gtk::Button,
    field: gtk::Box,
    view: RefCell<Option<DiscussionView>>,
    mode: RefCell<Mode>,
    /// Spoilers the viewer opened, by comment and place in it.
    revealed: RefCell<HashSet<(u32, usize)>>,
    /// Replies the viewer unfolded, by the reply they answer.
    unfolded: RefCell<HashSet<u32>>,
    /// Long comments the viewer opened in full.
    expanded: RefCell<HashSet<u32>>,
    /// Each drawn comment's row, to put Raised behind the one being answered.
    rows: RefCell<HashMap<u32, gtk::Box>>,
    /// Like counts as last drawn, so a change rolls instead of jumping.
    likes: RefCell<HashMap<u32, u32>>,
    /// Comments already drawn once: a new one fades in.
    seen: RefCell<HashSet<u32>>,
    /// Whether this drawing is a later one, so new comments fade in.
    animate: Cell<bool>,
    /// The text of the comments that were on their way at the last drawing, and at this one:
    /// when one comes back posted it goes from half to full opacity instead of fading in.
    was_sending: RefCell<Vec<String>>,
    now_sending: RefCell<Vec<String>>,
    started: Instant,
    /// How many comments were there when the next page was asked for.
    asked: Cell<usize>,
    /// Scroll to the end once the next drawing lands: the viewer just posted.
    to_end: Cell<bool>,
}

/// Open the discussion of `episode` of `media`.
pub fn open(ctx: &Rc<Ctx>, media: AnilistId, episode: u32) {
    let sheet = build(ctx, media, episode);
    ctx.hub.send(Command::OpenDiscussion { media, episode });
    sheet.dialog.present(Some(&ctx.window));
}

fn build(ctx: &Rc<Ctx>, media: AnilistId, episode: u32) -> Rc<Sheet> {
    let title = t::discussion_title_episode(i64::from(episode));

    // The header is final from the first frame: its place is kept, and the count and the link
    // fade in when the thread is found, so nothing moves under the placeholders.
    let count = widgets::haze("", "discussion-count");
    count.set_hexpand(true);
    count.set_xalign(0.0);
    let open = gtk::Button::with_label(&t::title_open_anilist());
    open.add_css_class("flat");
    open.add_css_class("discussion-open");
    open.set_halign(gtk::Align::End);
    open.set_sensitive(false);
    open.set_opacity(0.0);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.set_size_request(-1, 32);
    head.set_margin_start(20);
    head.set_margin_end(12);
    head.append(&count);
    head.append(&open);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_margin_start(12);
    column.set_margin_end(12);
    column.set_margin_top(0);
    column.set_margin_bottom(12);
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&column)
        .build();

    let waiting = gtk::Box::new(gtk::Orientation::Vertical, 14);
    waiting.set_margin_start(16);
    waiting.set_margin_end(16);
    waiting.set_margin_top(8);
    for _ in 0..4 {
        waiting.append(&skeleton());
    }
    let waiting_note = widgets::haze("", "caption");
    waiting_note.set_xalign(0.0);
    waiting.append(&waiting_note);

    let status = gtk::Box::new(gtk::Orientation::Vertical, 0);
    status.set_vexpand(true);

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("blank"));
    stack.add_named(&waiting, Some("wait"));
    stack.add_named(&status, Some("status"));
    stack.add_named(&scroller, Some("list"));

    // The foot: what is being answered, the field and Post, or why there is no field.
    let mode_label = widgets::haze("", "caption");
    mode_label.set_xalign(0.0);
    mode_label.set_hexpand(true);
    mode_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let cancel = gtk::Button::from_icon_name("window-close-symbolic");
    cancel.add_css_class("flat");
    cancel.add_css_class("circular");
    cancel.set_tooltip_text(Some(&t::discussion_cancel_reply()));
    let mode_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    mode_bar.append(&mode_label);
    mode_bar.append(&cancel);
    mode_bar.set_visible(false);

    let entry = gtk::Entry::builder()
        .placeholder_text(t::discussion_add_comment())
        .hexpand(true)
        .build();
    // Spoiler in the field's edit menu wraps the selection in AniList's spoiler marks.
    let edit_menu = gio::Menu::new();
    edit_menu.append(Some(&t::discussion_spoiler()), Some("comment.spoiler"));
    entry.set_extra_menu(Some(&edit_menu));
    let post = gtk::Button::with_label(&t::discussion_post());
    post.set_sensitive(false);
    let field = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    field.append(&entry);
    field.append(&post);

    let sign_in = gtk::Button::with_label(&t::discussion_sign_in());
    sign_in.set_visible(false);
    let offline = widgets::haze(&t::discussion_offline_post(), "caption");
    offline.set_visible(false);
    let note = widgets::haze("", "caption");
    note.set_xalign(0.0);
    note.set_wrap(true);
    note.set_visible(false);

    let foot = gtk::Box::new(gtk::Orientation::Vertical, 6);
    foot.set_margin_start(16);
    foot.set_margin_end(16);
    foot.set_margin_top(8);
    foot.set_margin_bottom(12);
    foot.append(&mode_bar);
    foot.append(&field);
    foot.append(&sign_in);
    foot.append(&offline);

    let view = adw::ToolbarView::new();
    let bar = adw::HeaderBar::new();
    bar.set_title_widget(Some(&heading(
        &title,
        ctx.hub.entry(media).map(|e| e.title).as_deref(),
    )));
    view.add_top_bar(&bar);
    let top = gtk::Box::new(gtk::Orientation::Vertical, 4);
    top.append(&head);
    top.append(&note);
    view.add_top_bar(&top);
    view.set_content(Some(&stack));
    view.add_bottom_bar(&foot);

    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(540)
        .content_height(720)
        .child(&view)
        .build();

    let sheet = Rc::new(Sheet {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        media,
        episode,
        stack,
        scroller: scroller.clone(),
        column,
        status,
        waiting_note,
        count,
        open: open.clone(),
        note,
        sign_in: sign_in.clone(),
        offline,
        mode_bar,
        mode_label,
        entry: entry.clone(),
        post: post.clone(),
        field,
        view: RefCell::new(None),
        mode: RefCell::new(Mode::New),
        revealed: RefCell::new(HashSet::new()),
        unfolded: RefCell::new(HashSet::new()),
        expanded: RefCell::new(HashSet::new()),
        rows: RefCell::new(HashMap::new()),
        likes: RefCell::new(HashMap::new()),
        seen: RefCell::new(HashSet::new()),
        animate: Cell::new(false),
        was_sending: RefCell::new(Vec::new()),
        now_sending: RefCell::new(Vec::new()),
        started: Instant::now(),
        asked: Cell::new(0),
        to_end: Cell::new(false),
    });
    sheet.actions(&view);
    sheet.show_wait();

    let weak = Rc::downgrade(&sheet);
    open.connect_clicked({
        let weak = weak.clone();
        move |_| {
            if let Some(sheet) = weak.upgrade()
                && let Some(view) = sheet.view.borrow().as_ref()
            {
                sheet.ctx.hub.send(Command::OpenExternal { url: view.url.clone() });
            }
        }
    });
    cancel.connect_clicked({
        let weak = weak.clone();
        move |_| {
            if let Some(sheet) = weak.upgrade() {
                sheet.set_mode(Mode::New);
            }
        }
    });
    entry.connect_changed({
        let weak = weak.clone();
        move |entry| {
            if let Some(sheet) = weak.upgrade() {
                // Amber waits for Post, and shows once there is something to post.
                let text = !entry.text().trim().is_empty();
                sheet.post.set_sensitive(text);
                if text {
                    sheet.post.add_css_class("suggested-action");
                } else {
                    sheet.post.remove_css_class("suggested-action");
                }
            }
        }
    });
    let send = {
        let weak = weak.clone();
        move || {
            if let Some(sheet) = weak.upgrade() {
                sheet.send();
            }
        }
    };
    post.connect_clicked({
        let send = send.clone();
        move |_| send()
    });
    entry.connect_activate(move |_| send());
    sign_in.connect_clicked({
        let weak = weak.clone();
        move |_| {
            if let Some(sheet) = weak.upgrade() {
                sheet.dialog.close();
                let _ = gtk::prelude::WidgetExt::activate_action(
                    &sheet.ctx.window,
                    "win.settings-pane",
                    Some(&"accounts".to_variant()),
                );
            }
        }
    });
    scroller.connect_edge_reached({
        let weak = weak.clone();
        move |_, edge| {
            if edge == gtk::PositionType::Bottom
                && let Some(sheet) = weak.upgrade()
            {
                sheet.more();
            }
        }
    });

    let listening = ctx.hub.listen({
        let weak = weak.clone();
        move |event| {
            if let Event::Discussion(view) = event
                && let Some(sheet) = weak.upgrade()
                && view.media == sheet.media
                && view.episode == sheet.episode
            {
                sheet.view.replace(Some(view.clone()));
                sheet.render();
            }
        }
    });
    // The placeholders come after the quiet time, and the slow line after four seconds.
    let ticking = glib::timeout_add_local(Duration::from_millis(250), {
        let weak = weak.clone();
        move || match weak.upgrade() {
            Some(sheet) => {
                sheet.waiting();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        }
    });
    let ticking = RefCell::new(Some(ticking));
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(tick) = ticking.borrow_mut().take() {
            tick.remove();
        }
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
            hub.send(Command::CloseDiscussion);
        }
    });
    widgets::keep(&dialog, Rc::clone(&sheet));
    sheet
}

/// The dialog's title: ours, with the show's under it in Zen Kaku.
fn heading(title: &str, show: Option<&str>) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_valign(gtk::Align::Center);
    let top = gtk::Label::new(Some(title));
    top.add_css_class("heading");
    top.set_ellipsize(gtk::pango::EllipsizeMode::End);
    column.append(&top);
    if let Some(show) = show {
        let sub = gtk::Label::new(Some(show));
        sub.add_css_class("show-title");
        sub.add_css_class("haze");
        sub.add_css_class("discussion-show");
        sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
        column.append(&sub);
    }
    column
}

/// A grey plate where a comment will land.
fn skeleton() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let face = gtk::Box::new(gtk::Orientation::Vertical, 0);
    face.add_css_class("plate");
    face.add_css_class("discussion-avatar");
    face.set_size_request(32, 32);
    face.set_valign(gtk::Align::Start);
    let lines = gtk::Box::new(gtk::Orientation::Vertical, 6);
    for width in [120, 280, 200] {
        let line = gtk::Box::new(gtk::Orientation::Vertical, 0);
        line.add_css_class("plate");
        line.add_css_class("bar");
        line.set_size_request(width, 12);
        line.set_halign(gtk::Align::Start);
        lines.append(&line);
    }
    row.append(&face);
    row.append(&lines);
    row
}

impl Sheet {
    /// The comment menu's actions and the links' target.
    fn actions(self: &Rc<Self>, root: &impl IsA<gtk::Widget>) {
        let group = gio::SimpleActionGroup::new();
        let string = Some(glib::VariantTy::STRING);
        let edit = gio::SimpleAction::new("edit", string);
        edit.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, target| {
                let Some(sheet) = weak.upgrade() else { return };
                let Some(id) = target.and_then(|t| t.str()).and_then(|s| s.parse().ok()) else {
                    return;
                };
                let text = sheet
                    .view
                    .borrow()
                    .as_ref()
                    .and_then(|v| find(&v.comments, id).map(|c| c.markdown.clone()));
                if let Some(text) = text {
                    sheet.set_mode(Mode::Edit { id });
                    sheet.entry.set_text(&text);
                    sheet.entry.set_position(-1);
                }
            }
        });
        let delete = gio::SimpleAction::new("delete", string);
        delete.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, target| {
                let Some(sheet) = weak.upgrade() else { return };
                let Some(id) = target.and_then(|t| t.str()).and_then(|s| s.parse().ok()) else {
                    return;
                };
                sheet.confirm_delete(id);
            }
        });
        let id_of = |target: Option<&glib::Variant>| -> Option<u32> {
            target.and_then(|t| t.str()).and_then(|s| s.parse().ok())
        };
        let reply = gio::SimpleAction::new("reply", string);
        reply.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, target| {
                let Some(sheet) = weak.upgrade() else { return };
                let Some(id) = id_of(target) else { return };
                let name = sheet
                    .view
                    .borrow()
                    .as_ref()
                    .and_then(|v| find(&v.comments, id).map(|c| c.author.clone()));
                if let Some(name) = name {
                    sheet.set_mode(Mode::Reply { parent: id, name });
                }
            }
        });
        let copy = gio::SimpleAction::new("copy", string);
        copy.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, target| {
                let Some(sheet) = weak.upgrade() else { return };
                let Some(id) = id_of(target) else { return };
                let text = sheet
                    .view
                    .borrow()
                    .as_ref()
                    .and_then(|v| find(&v.comments, id).map(plain_text));
                if let Some(text) = text {
                    sheet.dialog.clipboard().set_text(&text);
                }
            }
        });
        let open = gio::SimpleAction::new("open", string);
        open.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, target| {
                let Some(sheet) = weak.upgrade() else { return };
                let Some(id) = id_of(target) else { return };
                if let Some(view) = sheet.view.borrow().as_ref()
                    && !view.url.is_empty()
                {
                    let url = format!("{}/comment/{id}", view.url.trim_end_matches('/'));
                    sheet.ctx.hub.send(Command::OpenExternal { url });
                }
            }
        });
        let spoiler = gio::SimpleAction::new("spoiler", None);
        spoiler.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, _| {
                if let Some(sheet) = weak.upgrade() {
                    mark_spoiler(&sheet.entry);
                }
            }
        });
        group.add_action(&edit);
        group.add_action(&delete);
        group.add_action(&reply);
        group.add_action(&copy);
        group.add_action(&open);
        group.add_action(&spoiler);
        root.insert_action_group("comment", Some(&group));
    }

    fn confirm_delete(self: &Rc<Self>, id: u32) {
        let alert = adw::AlertDialog::new(
            Some(&t::discussion_delete_title()),
            Some(&t::discussion_delete_body()),
        );
        alert
            .add_responses(&[("cancel", &t::action_cancel()), ("delete", &t::action_delete())]);
        alert.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        let hub = Rc::clone(&self.ctx.hub);
        alert.connect_response(None, move |_, response| {
            if response == "delete" {
                hub.send(Command::DeleteComment { id });
            }
        });
        alert.present(Some(&self.dialog));
    }

    fn set_mode(&self, mode: Mode) {
        let label = match &mode {
            Mode::New => String::new(),
            Mode::Reply { name, .. } => t::discussion_replying_to(name),
            Mode::Edit { .. } => t::discussion_edit_comment(),
        };
        self.mode_bar.set_visible(mode != Mode::New);
        self.mode_label.set_label(&label);
        self.entry.set_placeholder_text(Some(&if matches!(mode, Mode::Edit { .. }) {
            t::discussion_edit_comment()
        } else {
            t::discussion_add_comment()
        }));
        if mode == Mode::New {
            self.entry.set_text("");
        } else {
            self.entry.grab_focus();
        }
        self.mode.replace(mode);
        self.mark_answering();
    }

    /// Raised behind the comment being answered or edited: a selection, not a card.
    fn mark_answering(&self) {
        let answering = match &*self.mode.borrow() {
            Mode::New => None,
            Mode::Reply { parent, .. } => Some(*parent),
            Mode::Edit { id } => Some(*id),
        };
        for (id, row) in self.rows.borrow().iter() {
            if Some(*id) == answering {
                row.add_css_class("answering");
            } else {
                row.remove_css_class("answering");
            }
        }
    }

    fn send(&self) {
        let text = self.entry.text().trim().to_owned();
        if text.is_empty() {
            return;
        }
        let mode = self.mode.borrow().clone();
        let command = match mode {
            Mode::New => Command::PostComment { parent: None, text },
            Mode::Reply { parent, .. } => Command::PostComment { parent: Some(parent), text },
            Mode::Edit { id } => Command::EditComment { id, text },
        };
        if matches!(command, Command::PostComment { parent: None, .. }) {
            self.to_end.set(true);
        }
        self.ctx.hub.send(command);
        self.set_mode(Mode::New);
    }

    /// The next page, once, when the viewer reaches the end.
    fn more(&self) {
        let view = self.view.borrow();
        let Some(view) = view.as_ref() else { return };
        if matches!(view.state, DiscussionState::Ready { more: true })
            && self.asked.get() != view.comments.len()
        {
            self.asked.set(view.comments.len());
            self.ctx.hub.send(Command::MoreComments);
        }
    }

    /// The placeholders and the lines that come with waiting, by how long it has been.
    fn waiting(&self) {
        let view = self.view.borrow();
        let Some(state) = view.as_ref().map(|v| &v.state) else {
            self.show_wait();
            return;
        };
        if matches!(state, DiscussionState::Loading | DiscussionState::RateLimited { .. }) {
            drop(view);
            self.show_wait();
        }
    }

    fn show_wait(&self) {
        let elapsed = self.started.elapsed();
        let seconds = match self.view.borrow().as_ref().map(|v| v.state.clone()) {
            Some(DiscussionState::RateLimited { seconds }) => Some(seconds),
            _ => None,
        };
        if let Some(seconds) = seconds {
            let left = (i64::from(seconds) - elapsed.as_secs() as i64).max(0);
            self.waiting_note.set_label(&t::discussion_rate_limited(&say::countdown(left)));
            self.waiting_note.set_visible(true);
            self.stack.set_visible_child_name("wait");
        } else if elapsed < QUIET {
            self.stack.set_visible_child_name("blank");
        } else {
            self.waiting_note.set_label(&t::discussion_still_loading());
            self.waiting_note.set_visible(elapsed >= SLOW);
            self.stack.set_visible_child_name("wait");
        }
    }

    /// The whole discussion as the engine last said it.
    fn render(self: &Rc<Self>) {
        let Some(view) = self.view.borrow().clone() else { return };
        if view.comment_count > 0 {
            let fresh = self.count.label().is_empty();
            self.count.set_label(&t::discussion_count(i64::from(view.comment_count)));
            if fresh {
                let _ = motion::fade_in(&self.count);
            }
        }
        // With no thread, the status page has the same button.
        let linked = !view.url.is_empty() && !matches!(view.state, DiscussionState::NoThread);
        self.open.set_visible(!matches!(view.state, DiscussionState::NoThread));
        if linked && !self.open.is_sensitive() {
            self.open.set_sensitive(true);
            let _ = motion::fade_in(&self.open);
        }
        self.foot(&view);

        let offline_age = match view.state {
            DiscussionState::Offline { age_minutes } => Some(i64::from(age_minutes)),
            _ => None,
        };
        match &view.state {
            DiscussionState::Loading | DiscussionState::RateLimited { .. } => {
                self.note.set_visible(false);
                self.show_wait();
                return;
            }
            DiscussionState::NoThread => {
                let page = states::plain(
                    &t::discussion_no_thread(),
                    &t::discussion_no_thread_detail(),
                );
                let button = gtk::Button::with_label(&t::title_open_anilist());
                button.add_css_class("pill");
                button.set_halign(gtk::Align::Center);
                let hub = Rc::clone(&self.ctx.hub);
                let url = view.url.clone();
                button.connect_clicked(move |_| {
                    hub.send(Command::OpenExternal { url: url.clone() });
                });
                page.set_child(Some(&button));
                self.status_page(&page);
                return;
            }
            DiscussionState::Failed => {
                self.status_page(
                    &self.retry_page(&t::discussion_failed(), &t::title_no_answer()),
                );
                return;
            }
            DiscussionState::Offline { .. } if view.comments.is_empty() => {
                self.status_page(&self.retry_page(&t::state_offline_title(), ""));
                return;
            }
            DiscussionState::Ready { .. } | DiscussionState::Offline { .. } => {}
        }
        if view.comments.is_empty() {
            self.status_page(&states::plain(&t::discussion_no_comments(), ""));
            // On the desktop the field is ready for the first comment.
            if self.field.is_visible() {
                self.entry.grab_focus();
            }
            return;
        }
        match offline_age {
            Some(minutes) => {
                self.note.set_label(&t::discussion_offline_cached(&say::ago(minutes * 60)));
                self.note.set_margin_start(16);
                self.note.set_visible(true);
            }
            None => self.note.set_visible(false),
        }

        let place = self.scroller.vadjustment().value();
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        self.rows.borrow_mut().clear();
        self.animate.set(!self.seen.borrow().is_empty());
        self.was_sending.replace(self.now_sending.take());
        for (n, comment) in view.comments.iter().enumerate() {
            if n > 0 {
                let rule = gtk::Separator::new(gtk::Orientation::Horizontal);
                rule.add_css_class("discussion-rule");
                self.column.append(&rule);
            }
            self.append(&self.column, comment, None, 0, &view);
        }
        if matches!(view.state, DiscussionState::Ready { more: true }) {
            self.column.append(&skeleton());
        }
        self.mark_answering();
        if self.stack.visible_child_name().as_deref() != Some("list") {
            self.stack.set_visible_child_name("list");
            let _ = motion::crossfade_in(&self.scroller);
        }
        let to_end = self.to_end.replace(false);
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(sheet) = weak.upgrade() else { return };
            let adjustment = sheet.scroller.vadjustment();
            if to_end {
                adjustment.set_value(adjustment.upper() - adjustment.page_size());
            } else {
                adjustment.set_value(place);
            }
            // A page that fits without scrolling would never reach its edge.
            if adjustment.upper() <= adjustment.page_size() + 1.0 {
                sheet.more();
            }
        });
    }

    fn retry_page(&self, title: &str, line: &str) -> adw::StatusPage {
        let page = states::plain(title, line);
        let button = gtk::Button::with_label(&t::action_try_again());
        button.add_css_class("pill");
        button.set_halign(gtk::Align::Center);
        let hub = Rc::clone(&self.ctx.hub);
        let (media, episode) = (self.media, self.episode);
        button.connect_clicked(move |_| hub.send(Command::OpenDiscussion { media, episode }));
        page.set_child(Some(&button));
        page
    }

    fn status_page(&self, page: &adw::StatusPage) {
        while let Some(child) = self.status.first_child() {
            self.status.remove(&child);
        }
        self.status.append(page);
        self.stack.set_visible_child_name("status");
    }

    /// The field, or the line that takes its place.
    fn foot(&self, view: &DiscussionView) {
        let no_thread = matches!(view.state, DiscussionState::NoThread);
        let field = view.can_post && !no_thread;
        self.field.set_visible(field);
        if !field {
            self.mode_bar.set_visible(false);
        }
        self.sign_in.set_visible(!view.signed_in && !no_thread);
        let settled = matches!(
            view.state,
            DiscussionState::Ready { .. } | DiscussionState::Offline { .. }
        );
        self.offline.set_visible(view.signed_in && !view.can_post && settled);
    }

    /// One comment, and under a top-level one its replies. A reply's own replies fold into
    /// "N more replies" until the viewer opens them, and then follow it at the same step.
    fn append(
        self: &Rc<Self>,
        into: &gtk::Box,
        comment: &CommentView,
        parent: Option<u32>,
        depth: u8,
        view: &DiscussionView,
    ) {
        let hub = &self.ctx.hub;
        let id = comment.id;
        let size = if depth == 0 { 32 } else { 24 };
        let row = gtk::Box::new(gtk::Orientation::Horizontal, if depth == 0 { 12 } else { 10 });
        row.add_css_class("discussion-row");
        if depth == 0 {
            row.add_css_class("top");
        } else {
            row.add_css_class("reply");
            row.set_margin_top(10);
            // Its padding, so the reply's text sits in the parent's text column.
            row.set_margin_start(-8);
            row.set_margin_end(-8);
        }
        let (frame, _) =
            widgets::art(hub, comment.avatar.as_deref(), None, "discussion-avatar");
        if depth != 0 {
            frame.add_css_class("small");
        }
        let face = crate::fit::Fit::new(&frame, 1.0, size, true, 0);
        face.set_valign(gtk::Align::Start);
        face.set_halign(gtk::Align::Start);
        face.set_margin_top(2);
        row.append(&face);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_hexpand(true);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let author = gtk::Label::new(Some(&comment.author));
        author.add_css_class("discussion-author");
        author.set_xalign(0.0);
        author.set_ellipsize(gtk::pango::EllipsizeMode::End);
        line.append(&author);
        let age = if comment.pending == PostState::Sending {
            t::discussion_sending()
        } else {
            say::msg(&comment.age)
        };
        let age = widgets::haze(&age, "discussion-age");
        age.set_wrap(false);
        line.append(&age);

        let more = self.comment_menu(comment, view);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        line.append(&spacer);
        line.append(&more);
        content.append(&line);

        let (body, labels, pictures) = self.body(comment);
        body.set_margin_top(4);
        content.append(&body);
        let show_more = gtk::Button::with_label(&t::discussion_show_more());
        show_more.add_css_class("flat");
        show_more.add_css_class("discussion-reply");
        show_more.set_halign(gtk::Align::Start);
        show_more.set_visible(false);
        content.append(&show_more);
        self.fold(id, &body, labels, pictures, &show_more);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_size_request(-1, 34);
        // The like's own padding, so the heart starts under the text.
        actions.set_margin_start(-8);
        actions.append(&self.like(comment, view.can_post));
        if view.can_post && comment.pending == PostState::Posted {
            let reply = gtk::Button::with_label(&t::discussion_reply());
            reply.add_css_class("flat");
            reply.add_css_class("discussion-reply");
            let name = comment.author.clone();
            reply.connect_clicked({
                let weak = Rc::downgrade(self);
                move |_| {
                    if let Some(sheet) = weak.upgrade() {
                        sheet.set_mode(Mode::Reply { parent: id, name: name.clone() });
                    }
                }
            });
            actions.append(&reply);
        }
        content.append(&actions);

        if comment.pending == PostState::Failed {
            let again = gtk::Button::with_label(&t::discussion_post_failed());
            again.add_css_class("flat");
            again.add_css_class("error");
            again.set_halign(gtk::Align::Start);
            let text = comment.markdown.clone();
            again.connect_clicked({
                let hub = Rc::clone(hub);
                move |_| hub.send(Command::PostComment { parent, text: text.clone() })
            });
            content.append(&again);
        }
        if comment.pending != PostState::Posted {
            row.set_opacity(0.5);
        }
        row.append(&content);
        // Secondary click opens the comment's menu where the pointer is.
        let secondary = gtk::GestureClick::builder().button(3).build();
        secondary.connect_pressed({
            let more = more.clone();
            move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                more.popup();
            }
        });
        row.add_controller(secondary);
        into.append(&row);
        self.rows.borrow_mut().insert(id, row.clone());
        if comment.pending == PostState::Sending {
            self.now_sending.borrow_mut().push(comment.markdown.clone());
        }
        // The comment you sent, answered: from half to full opacity on the quick spring.
        let confirmed = comment.own
            && comment.pending == PostState::Posted
            && self.was_sending.borrow().contains(&comment.markdown);
        if confirmed {
            self.was_sending.borrow_mut().retain(|m| *m != comment.markdown);
            self.seen.borrow_mut().insert(id);
            row.set_opacity(0.5);
            motion::quick(&row, 0.5, 1.0, motion::opacity(&row)).play();
        } else if self.seen.borrow_mut().insert(id) && self.animate.get() {
            let to = if comment.pending == PostState::Posted { 1.0 } else { 0.5 };
            row.set_opacity(0.0);
            motion::standard(&row, 0.0, to, motion::opacity(&row)).play();
        }

        if depth == 0 {
            for reply in &comment.replies {
                self.append(&content, reply, Some(comment.id), 1, view);
            }
        } else if depth == 1 && comment.hidden_replies > 0 {
            if self.unfolded.borrow().contains(&comment.id) {
                for later in &comment.replies {
                    self.append(into, later, Some(comment.id), 2, view);
                }
            } else {
                let fold = gtk::Button::with_label(&t::discussion_more_replies(i64::from(
                    comment.hidden_replies,
                )));
                fold.add_css_class("flat");
                fold.add_css_class("discussion-reply");
                fold.set_halign(gtk::Align::Start);
                fold.connect_clicked({
                    let weak = Rc::downgrade(self);
                    move |_| {
                        if let Some(sheet) = weak.upgrade() {
                            sheet.unfolded.borrow_mut().insert(id);
                            sheet.render();
                        }
                    }
                });
                content.append(&fold);
            }
        }
    }

    /// The hover menu: Reply, Copy text and Open on AniList for every comment, and Edit and
    /// Delete under the viewer's own.
    fn comment_menu(&self, comment: &CommentView, view: &DiscussionView) -> gtk::MenuButton {
        let target = comment.id.to_string();
        let posted = comment.pending == PostState::Posted;
        let menu = gio::Menu::new();
        let main = gio::Menu::new();
        if view.can_post && posted {
            main.append_item(&widgets::menu_item(
                &t::discussion_reply(),
                "comment.reply",
                &target,
            ));
        }
        main.append_item(&widgets::menu_item(
            &t::discussion_copy_text(),
            "comment.copy",
            &target,
        ));
        if posted && !view.url.is_empty() {
            main.append_item(&widgets::menu_item(
                &t::title_open_anilist(),
                "comment.open",
                &target,
            ));
        }
        menu.append_section(None, &main);
        if comment.own && posted {
            let own = gio::Menu::new();
            own.append_item(&widgets::menu_item(&t::action_edit(), "comment.edit", &target));
            own.append_item(&widgets::menu_item(
                &t::action_delete(),
                "comment.delete",
                &target,
            ));
            menu.append_section(None, &own);
        }
        let more = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .menu_model(&menu)
            .tooltip_text(t::episode_options())
            .valign(gtk::Align::Start)
            .build();
        more.add_css_class("flat");
        more.add_css_class("circular");
        more.add_css_class("discussion-more");
        more
    }

    /// The heart and its count: a button when the viewer can like, the bare count when not. A
    /// liked comment fills the heart in the text colour; a changed count rolls.
    fn like(self: &Rc<Self>, comment: &CommentView, can_like: bool) -> gtk::Widget {
        let liked = comment.liked;
        let heart = gtk::DrawingArea::new();
        heart.set_content_width(14);
        heart.set_content_height(14);
        heart.set_valign(gtk::Align::Center);
        heart.set_draw_func(move |area, cr, w, h| draw_heart(area, cr, w, h, liked));

        let words = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        words.append(&heart);
        let count = comment.like_count;
        let before = self.likes.borrow_mut().insert(comment.id, count);
        let digits = gtk::Stack::new();
        digits.set_transition_duration(300);
        let label = |n: u32| {
            let label =
                gtk::Label::new(Some(&if n > 0 { n.to_string() } else { String::new() }));
            label.add_css_class("numeric");
            label
        };
        match before {
            Some(before) if before != count => {
                digits.add_named(&label(before), Some("before"));
                digits.add_named(&label(count), Some("after"));
                digits.set_visible_child_name("before");
                digits.set_transition_type(if count > before {
                    gtk::StackTransitionType::SlideUp
                } else {
                    gtk::StackTransitionType::SlideDown
                });
                glib::idle_add_local_once({
                    let digits = digits.clone();
                    move || digits.set_visible_child_name("after")
                });
            }
            _ => {
                digits.add_named(&label(count), Some("after"));
            }
        }
        words.append(&digits);

        if !can_like {
            words.add_css_class("discussion-like");
            if liked {
                words.add_css_class("liked");
            }
            return words.upcast();
        }
        let like = gtk::Button::new();
        like.add_css_class("flat");
        like.add_css_class("discussion-like");
        if liked {
            like.add_css_class("liked");
        }
        like.set_child(Some(&words));
        let id = comment.id;
        like.connect_clicked({
            let hub = Rc::clone(&self.ctx.hub);
            move |_| hub.send(Command::LikeComment { id })
        });
        like.upcast()
    }

    /// Cut a long comment off after ten lines, once its width is known, behind Show more.
    fn fold(
        self: &Rc<Self>,
        id: u32,
        body: &gtk::Box,
        labels: Vec<gtk::Label>,
        pictures: usize,
        button: &gtk::Button,
    ) {
        if self.expanded.borrow().contains(&id) {
            return;
        }
        let done = Cell::new(false);
        body.add_tick_callback({
            let button = button.clone();
            let weak = Rc::downgrade(self);
            move |body, _| {
                if done.get() || weak.upgrade().is_none() {
                    return glib::ControlFlow::Break;
                }
                if body.width() == 0 {
                    return glib::ControlFlow::Continue;
                }
                done.set(true);
                let counts: Vec<i32> = labels.iter().map(|l| l.layout().line_count()).collect();
                let total = counts.iter().sum::<i32>() + 9 * pictures as i32;
                if total <= FOLD_LINES {
                    return glib::ControlFlow::Break;
                }
                let mut left = FOLD_LINES - 9 * pictures as i32;
                for (label, lines) in labels.iter().zip(&counts) {
                    if left <= 0 {
                        label.set_visible(false);
                    } else if *lines > left {
                        label.set_lines(left);
                        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                        left = 0;
                    } else {
                        left -= lines;
                    }
                }
                button.set_visible(true);
                button.connect_clicked({
                    let labels = labels.clone();
                    let weak = weak.clone();
                    move |button| {
                        for label in &labels {
                            label.set_visible(true);
                            label.set_lines(-1);
                            label.set_ellipsize(gtk::pango::EllipsizeMode::None);
                        }
                        button.set_visible(false);
                        if let Some(sheet) = weak.upgrade() {
                            sheet.expanded.borrow_mut().insert(id);
                        }
                    }
                });
                glib::ControlFlow::Break
            }
        });
    }

    /// The comment's paragraphs, the labels in them and how many pictures. Spoilers are runs
    /// inside the sentence; links are the label's links; images load only when Settings says so.
    fn body(self: &Rc<Self>, comment: &CommentView) -> (gtk::Box, Vec<gtk::Label>, usize) {
        let load_images = self.view.borrow().as_ref().is_some_and(|v| v.load_images);
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let mut labels = Vec::new();
        let mut pictures = 0;
        let mut spoiler = 0usize;
        for block in &comment.body {
            let paragraph = gtk::Box::new(gtk::Orientation::Vertical, 6);
            if block.quote {
                paragraph.add_css_class("discussion-quote");
            }
            for part in parts(block, load_images, &mut spoiler) {
                match part {
                    Part::Inline(segs) => {
                        let label = self.label(segs, comment.id);
                        labels.push(label.clone());
                        paragraph.append(&label);
                    }
                    Part::Picture(url) => {
                        let picture = gtk::Picture::new();
                        picture.set_content_fit(gtk::ContentFit::Contain);
                        picture.set_can_shrink(true);
                        picture.set_size_request(-1, 180);
                        picture.set_halign(gtk::Align::Start);
                        picture.set_property("alternative-text", host(&url));
                        self.ctx.hub.art(&picture, Some(&url));
                        paragraph.append(&picture);
                        pictures += 1;
                    }
                }
            }
            column.append(&paragraph);
        }
        (column, labels, pictures)
    }

    /// A run of text with its spoilers: each a bar as long as the words it hides, opened in
    /// place with a quick crossfade, so nothing reflows.
    fn label(self: &Rc<Self>, segs: Vec<Seg>, comment: u32) -> gtk::Label {
        let label = gtk::Label::new(None);
        label.set_use_markup(true);
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let segs = Rc::new(segs);
        let shown: Rc<RefCell<HashMap<usize, f64>>> = {
            let revealed = self.revealed.borrow();
            Rc::new(RefCell::new(
                segs.iter()
                    .filter_map(|s| match s {
                        Seg::Spoiler { at, .. } => Some((
                            *at,
                            if revealed.contains(&(comment, *at)) { 1.0 } else { 0.0 },
                        )),
                        Seg::Text(_) => None,
                    })
                    .collect(),
            ))
        };
        label.set_markup(&inline_markup(&segs, &shown.borrow()));
        label.connect_activate_link({
            let weak = Rc::downgrade(self);
            let segs = Rc::clone(&segs);
            let shown = Rc::clone(&shown);
            move |label, uri| {
                let Some(sheet) = weak.upgrade() else {
                    return glib::Propagation::Stop;
                };
                if let Some(at) = uri.strip_prefix("spoiler:").and_then(|n| n.parse().ok()) {
                    let key = (comment, at);
                    let open = {
                        let mut revealed = sheet.revealed.borrow_mut();
                        if revealed.remove(&key) {
                            false
                        } else {
                            revealed.insert(key);
                            true
                        }
                    };
                    let from = shown.borrow().get(&at).copied().unwrap_or(0.0);
                    let target = {
                        let (label, segs, shown) =
                            (label.clone(), Rc::clone(&segs), Rc::clone(&shown));
                        adw::CallbackAnimationTarget::new(move |value| {
                            shown.borrow_mut().insert(at, value);
                            label.set_markup(&inline_markup(&segs, &shown.borrow()));
                        })
                    };
                    let animation = adw::TimedAnimation::new(
                        label,
                        from,
                        if open { 1.0 } else { 0.0 },
                        150,
                        target,
                    );
                    animation.set_easing(adw::Easing::EaseOutCubic);
                    animation.play();
                } else {
                    sheet.ctx.hub.send(Command::OpenExternal { url: uri.to_owned() });
                }
                glib::Propagation::Stop
            }
        });
        label
    }
}

/// The comment as plain paragraphs, for Copy text.
fn plain_text(comment: &CommentView) -> String {
    comment
        .body
        .iter()
        .map(|block| block.spans.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Wrap the field's selection in AniList's spoiler marks, or put an empty pair at the cursor.
fn mark_spoiler(entry: &gtk::Entry) {
    let (start, end) =
        entry.selection_bounds().unwrap_or_else(|| (entry.position(), entry.position()));
    let words = entry.chars(start, end);
    entry.delete_text(start, end);
    let mut at = start;
    entry.insert_text(&format!("~!{words}!~"), &mut at);
    entry.set_position(if words.is_empty() { start + 2 } else { at });
    entry.grab_focus();
}

/// The heart, outline at rest and filled when liked, in the widget's own text colour.
fn draw_heart(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, w: i32, h: i32, liked: bool) {
    let color = area.color();
    cr.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha()),
    );
    let k = f64::from(w.min(h)) / 16.0;
    cr.scale(k, k);
    cr.move_to(8.0, 13.5);
    cr.curve_to(3.5, 10.5, 1.5, 8.0, 1.5, 5.6);
    cr.curve_to(1.5, 3.6, 3.0, 2.3, 4.8, 2.3);
    cr.curve_to(6.1, 2.3, 7.3, 3.0, 8.0, 4.1);
    cr.curve_to(8.7, 3.0, 9.9, 2.3, 11.2, 2.3);
    cr.curve_to(13.0, 2.3, 14.5, 3.6, 14.5, 5.6);
    cr.curve_to(14.5, 8.0, 12.5, 10.5, 8.0, 13.5);
    cr.close_path();
    if liked {
        let _ = cr.fill();
    } else {
        cr.set_line_width(1.5);
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        let _ = cr.stroke();
    }
}

fn find(comments: &[CommentView], id: u32) -> Option<&CommentView> {
    comments.iter().find_map(|c| if c.id == id { Some(c) } else { find(&c.replies, id) })
}

/// A piece of a paragraph's text: plain markup, or a spoiler by its place in the comment.
enum Seg {
    Text(String),
    Spoiler { at: usize, spans: Vec<CommentSpan> },
}

/// A paragraph cut into what draws as one label (text with its spoilers in it) or a picture.
enum Part {
    Inline(Vec<Seg>),
    Picture(String),
}

/// `block` as its parts. `spoiler` counts the spoilers so far in the comment, so each has a
/// place that stays the same when the comment is drawn again.
fn parts(block: &CommentBlock, load_images: bool, spoiler: &mut usize) -> Vec<Part> {
    let mut out = Vec::new();
    let mut segs: Vec<Seg> = Vec::new();
    let mut text = String::new();
    let mut hidden: Option<(usize, Vec<CommentSpan>)> = None;
    let flush = |segs: &mut Vec<Seg>, text: &mut String| {
        if !text.is_empty() {
            segs.push(Seg::Text(std::mem::take(text)));
        }
    };
    let close = |out: &mut Vec<Part>, segs: &mut Vec<Seg>| {
        if !segs.is_empty() {
            out.push(Part::Inline(std::mem::take(segs)));
        }
    };
    for span in &block.spans {
        if span.spoiler {
            let group = hidden.get_or_insert_with(|| {
                let at = *spoiler;
                *spoiler += 1;
                (at, Vec::new())
            });
            group.1.push(span.clone());
            continue;
        }
        if let Some((at, spans)) = hidden.take() {
            flush(&mut segs, &mut text);
            segs.push(Seg::Spoiler { at, spans });
        }
        match (span.media, &span.link) {
            (Some(MediaKind::Image), Some(url)) if load_images => {
                flush(&mut segs, &mut text);
                close(&mut out, &mut segs);
                out.push(Part::Picture(url.clone()));
            }
            (Some(kind), link) => {
                let words = match kind {
                    MediaKind::Image => t::discussion_image(),
                    MediaKind::Video => t::discussion_video(),
                };
                // The line names the site the tap goes to.
                let words = match link {
                    Some(url) => t::discussion_media_line(&words, &host(url)),
                    None => words,
                };
                text.push_str(&match link {
                    Some(url) => format!("<a href=\"{}\">{}</a>", escape(url), escape(&words)), // i18n-skip
                    None => escape(&words),
                });
            }
            (None, _) => text.push_str(&markup(span)),
        }
    }
    if let Some((at, spans)) = hidden.take() {
        flush(&mut segs, &mut text);
        segs.push(Seg::Spoiler { at, spans });
    }
    flush(&mut segs, &mut text);
    close(&mut out, &mut segs);
    out
}

/// The site a link goes to: "pics.example".
fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest).to_owned()
}

fn escape(text: &str) -> String {
    glib::markup_escape_text(text).to_string()
}

/// One span as Pango markup.
fn markup(span: &CommentSpan) -> String {
    let mut out = escape(&span.text);
    if span.bold {
        out = format!("<b>{out}</b>");
    }
    if span.italic {
        out = format!("<i>{out}</i>");
    }
    if span.strike {
        out = format!("<s>{out}</s>");
    }
    if let Some(url) = &span.link {
        out = format!("<a href=\"{}\">{out}</a>", escape(url)); // i18n-skip
    }
    out
}

/// A paragraph's markup. A spoiler is its own words on a Raised bar, clear while hidden, so
/// the bar is as long as the words and opening it moves nothing. `shown` is how far each is
/// open, from 0 to 1, while it fades.
fn inline_markup(segs: &[Seg], shown: &HashMap<usize, f64>) -> String {
    segs.iter()
        .map(|seg| match seg {
            Seg::Text(markup) => markup.clone(),
            Seg::Spoiler { at, spans } => {
                let open = shown.get(at).copied().unwrap_or(0.0);
                let inner: String = spans
                    .iter()
                    .map(|s| markup(&CommentSpan { link: None, ..s.clone() }))
                    .collect();
                let text = (open * 100.0).round().max(1.0);
                let bar = (100.0 - open * 65.0).round();
                format!(
                    "<a href=\"spoiler:{at}\"><span bgcolor=\"#343A57\" bgalpha=\"{bar}%\" fgalpha=\"{text}%\">{inner}</span></a>" // i18n-skip
                )
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> CommentSpan {
        CommentSpan { text: text.into(), ..Default::default() }
    }

    #[test]
    fn neighbouring_spoiler_spans_are_one_spoiler_with_one_place() {
        let hidden = |s: &str| CommentSpan { spoiler: true, ..text(s) };
        let block = CommentBlock {
            spans: vec![text("a "), hidden("b"), hidden("c"), text(" d"), hidden("e")],
            quote: false,
        };
        let mut count = 0;
        let out = parts(&block, false, &mut count);
        assert_eq!(count, 2);
        let places: Vec<usize> = out
            .iter()
            .filter_map(|p| if let Part::Inline(segs) = p { Some(segs) } else { None })
            .flatten()
            .filter_map(|s| if let Seg::Spoiler { at, .. } = s { Some(*at) } else { None })
            .collect();
        assert_eq!(places, [0, 1]);
    }

    #[test]
    fn a_spoiler_stays_in_its_sentence() {
        let hidden = CommentSpan { spoiler: true, ..text("the ferryman") };
        let block = CommentBlock {
            spans: vec![text("Read on: "), hidden, text(" waits.")],
            quote: false,
        };
        let mut n = 0;
        let out = parts(&block, false, &mut n);
        assert_eq!(out.len(), 1, "one label, so the sentence does not break");
    }

    #[test]
    fn a_link_names_its_site() {
        assert_eq!(host("https://pics.example/a/b.png?x=1"), "pics.example");
        assert_eq!(host("pics.example"), "pics.example");
    }

    #[test]
    fn markup_escapes_what_a_comment_wrote() {
        let span = CommentSpan { bold: true, ..text("a <b> & c") };
        assert_eq!(markup(&span), "<b>a &lt;b&gt; &amp; c</b>");
    }

    #[test]
    fn an_image_is_a_picture_only_when_images_load() {
        let image = CommentSpan {
            media: Some(MediaKind::Image),
            link: Some("https://img.example/a.png".into()),
            ..Default::default()
        };
        let block = CommentBlock { spans: vec![image], quote: false };
        let mut n = 0;
        assert!(matches!(parts(&block, true, &mut n)[0], Part::Picture(_)));
        assert!(matches!(parts(&block, false, &mut n)[0], Part::Inline(_)));
    }
}
