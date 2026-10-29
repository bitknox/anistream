//! An episode's discussion: AniList's forum thread in an `AdwDialog`, which is a sheet on a
//! narrow window. The engine says the whole discussion on every change, so the sheet draws it
//! again each time, keeping the scroll place and what the viewer opened (spoilers and folded
//! replies). Opening it from the player pauses playback there; closing it does not resume.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
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

use crate::{say, states, widgets, window::Ctx};

/// Nothing shows for this long, then the placeholders.
const QUIET: Duration = Duration::from_millis(600);
/// After this long the placeholders say they are still waiting.
const SLOW: Duration = Duration::from_secs(4);

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

    let count = widgets::haze("", "caption");
    count.set_hexpand(true);
    count.set_xalign(0.0);
    count.set_visible(false);
    let open = gtk::Button::with_label(&t::title_open_anilist());
    open.add_css_class("flat");
    open.set_halign(gtk::Align::End);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.set_margin_start(16);
    head.set_margin_end(8);
    head.append(&count);
    head.append(&open);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 14);
    column.set_margin_start(16);
    column.set_margin_end(16);
    column.set_margin_top(8);
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
    let post = gtk::Button::with_label(&t::discussion_post());
    post.add_css_class("suggested-action");
    post.set_sensitive(false);
    let field = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    field.append(&entry);
    field.append(&post);

    let sign_in = gtk::Button::with_label(&t::discussion_sign_in());
    sign_in.add_css_class("flat");
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
    view.add_top_bar(&adw::HeaderBar::new());
    let top = gtk::Box::new(gtk::Orientation::Vertical, 4);
    top.append(&head);
    top.append(&note);
    view.add_top_bar(&top);
    view.set_content(Some(&stack));
    view.add_bottom_bar(&foot);

    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(480)
        .content_height(680)
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
                sheet.post.set_sensitive(!entry.text().trim().is_empty());
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
        group.add_action(&edit);
        group.add_action(&delete);
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
        if !view.title.is_empty() {
            self.dialog.set_title(&view.title);
        }
        self.count.set_visible(view.comment_count > 0);
        self.count.set_label(&t::discussion_count(i64::from(view.comment_count)));
        // With no thread, the status page has the same button.
        self.open.set_visible(
            !view.url.is_empty() && !matches!(view.state, DiscussionState::NoThread),
        );
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
        for comment in &view.comments {
            self.append(&self.column, comment, None, 0, &view);
        }
        if matches!(view.state, DiscussionState::Ready { more: true }) {
            self.column.append(&skeleton());
        }
        self.stack.set_visible_child_name("list");
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
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let (frame, _) =
            widgets::art(hub, comment.avatar.as_deref(), None, "discussion-avatar");
        let face = crate::fit::Fit::new(&frame, 1.0, 32, true, 0);
        face.set_valign(gtk::Align::Start);
        face.set_halign(gtk::Align::Start);
        row.append(&face);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content.set_hexpand(true);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let author = gtk::Label::new(Some(&comment.author));
        author.add_css_class("heading");
        author.set_xalign(0.0);
        author.set_ellipsize(gtk::pango::EllipsizeMode::End);
        line.append(&author);
        line.append(&widgets::haze(&say::msg(&comment.age), "caption"));
        content.append(&line);
        content.append(&self.body(comment));

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        // The buttons' own padding, so Like starts under the text.
        actions.set_margin_start(-10);
        let like = gtk::Button::new();
        like.add_css_class("flat");
        like.add_css_class("discussion-like");
        if comment.liked {
            like.add_css_class("liked");
        }
        let words = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        words.append(&gtk::Label::new(Some(&t::discussion_like())));
        if comment.like_count > 0 {
            let number = gtk::Label::new(Some(&comment.like_count.to_string()));
            number.add_css_class("numeric");
            words.append(&number);
        }
        like.set_child(Some(&words));
        let id = comment.id;
        like.connect_clicked({
            let hub = Rc::clone(hub);
            move |_| hub.send(Command::LikeComment { id })
        });
        actions.append(&like);
        if view.can_post && comment.pending == PostState::Posted {
            let reply = gtk::Button::with_label(&t::discussion_reply());
            reply.add_css_class("flat");
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
        if comment.own && comment.pending == PostState::Posted {
            let menu = gio::Menu::new();
            let target = comment.id.to_string();
            menu.append_item(&widgets::menu_item(&t::action_edit(), "comment.edit", &target));
            menu.append_item(&widgets::menu_item(
                &t::action_delete(),
                "comment.delete",
                &target,
            ));
            let more = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .menu_model(&menu)
                .tooltip_text(t::episode_options())
                .build();
            more.add_css_class("flat");
            actions.append(&more);
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
        into.append(&row);

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

    /// The comment's paragraphs. Spoilers and links are labels' links; images load only when
    /// Settings says so.
    fn body(self: &Rc<Self>, comment: &CommentView) -> gtk::Box {
        let load_images = self.view.borrow().as_ref().is_some_and(|v| v.load_images);
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let mut spoiler = 0usize;
        for block in &comment.body {
            let paragraph = gtk::Box::new(gtk::Orientation::Vertical, 6);
            if block.quote {
                paragraph.add_css_class("discussion-quote");
            }
            let revealed = self.revealed.borrow().clone();
            for part in parts(block, load_images, &mut spoiler) {
                match part {
                    Part::Text(markup) => paragraph.append(&self.label(&markup, comment.id)),
                    Part::Spoiler { at, spans } => {
                        let open = revealed.contains(&(comment.id, at));
                        let markup = spoiler_markup(at, &spans, open);
                        paragraph.append(&self.label(&markup, comment.id));
                    }
                    Part::Picture(url) => {
                        let picture = gtk::Picture::new();
                        picture.set_content_fit(gtk::ContentFit::Contain);
                        picture.set_can_shrink(true);
                        picture.set_size_request(-1, 180);
                        picture.set_halign(gtk::Align::Start);
                        self.ctx.hub.art(&picture, Some(&url));
                        paragraph.append(&picture);
                    }
                }
            }
            column.append(&paragraph);
        }
        column
    }

    fn label(self: &Rc<Self>, markup: &str, comment: u32) -> gtk::Label {
        let label = gtk::Label::new(None);
        label.set_use_markup(true);
        label.set_markup(markup);
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.connect_activate_link({
            let weak = Rc::downgrade(self);
            move |_, uri| {
                let Some(sheet) = weak.upgrade() else {
                    return glib::Propagation::Stop;
                };
                if let Some(at) = uri.strip_prefix("spoiler:").and_then(|n| n.parse().ok()) {
                    let key = (comment, at);
                    {
                        let mut revealed = sheet.revealed.borrow_mut();
                        if !revealed.remove(&key) {
                            revealed.insert(key);
                        }
                    }
                    sheet.render();
                } else {
                    sheet.ctx.hub.send(Command::OpenExternal { url: uri.to_owned() });
                }
                glib::Propagation::Stop
            }
        });
        label
    }
}

fn find(comments: &[CommentView], id: u32) -> Option<&CommentView> {
    comments.iter().find_map(|c| if c.id == id { Some(c) } else { find(&c.replies, id) })
}

/// A paragraph cut into what draws as one label, a spoiler, or a picture.
enum Part {
    Text(String),
    Spoiler { at: usize, spans: Vec<CommentSpan> },
    Picture(String),
}

/// `block` as its parts. `spoiler` counts the spoilers so far in the comment, so each has a
/// place that stays the same when the comment is drawn again.
fn parts(block: &CommentBlock, load_images: bool, spoiler: &mut usize) -> Vec<Part> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut hidden: Option<(usize, Vec<CommentSpan>)> = None;
    let flush = |out: &mut Vec<Part>, text: &mut String| {
        if !text.is_empty() {
            out.push(Part::Text(std::mem::take(text)));
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
            flush(&mut out, &mut text);
            out.push(Part::Spoiler { at, spans });
        }
        match (span.media, &span.link) {
            (Some(MediaKind::Image), Some(url)) if load_images => {
                flush(&mut out, &mut text);
                out.push(Part::Picture(url.clone()));
            }
            (Some(kind), link) => {
                let words = match kind {
                    MediaKind::Image => t::discussion_image(),
                    MediaKind::Video => t::discussion_video(),
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
        flush(&mut out, &mut text);
        out.push(Part::Spoiler { at, spans });
    }
    flush(&mut out, &mut text);
    out
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

/// A spoiler: a muted bar reading "Spoiler" until opened, then its text on the same bar.
/// Either way it is one link, so a click toggles it.
fn spoiler_markup(at: usize, spans: &[CommentSpan], open: bool) -> String {
    let inner = if open {
        spans
            .iter()
            .map(|s| markup(&CommentSpan { link: None, ..s.clone() }))
            .collect::<String>()
    } else {
        // Non-breaking spaces keep the bar a little wider than its word.
        format!("\u{a0}\u{a0}{}\u{a0}\u{a0}", escape(&t::discussion_spoiler()))
    };
    let alpha = if open { "15%" } else { "40%" };
    format!(
        "<a href=\"spoiler:{at}\"><span bgcolor=\"#8a8a8a\" bgalpha=\"{alpha}\">{inner}</span></a>" // i18n-skip
    )
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
            .filter_map(|p| if let Part::Spoiler { at, .. } = p { Some(*at) } else { None })
            .collect();
        assert_eq!(places, [0, 1]);
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
        assert!(matches!(parts(&block, false, &mut n)[0], Part::Text(_)));
    }
}
