//! The player: inside the main window, over the page it came from. The video is the bottom
//! layer, mpv drawing into a `GtkGLArea` through its render API; the controls are native
//! widgets over it on a 35% dim of the ground. They fade out after two seconds idle and are then
//! unmapped, so the compositor can scan the video out with nothing over it.
//!
//! The engine drives mpv through `anistream-mpv-host`: its ops are applied as they come, but
//! only once the GL renderer exists, since a file loaded before that would find no video output.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_engine::engine::Engine;
use anistream_model::{
    command::{Command, PlayerCommand},
    event::Event,
    hosted::{
        MenuChoices, MenuEpisode, MenuSource, PlayerChoice, PlayerOp, PlayerUi, TrackKind,
    },
    playback::{PlaybackId, SkipKind},
};
use anistream_mpv_host::{HostEvent, Player, PlayerOptions, PlayerState};
use gtk::{gdk, gio, glib};

use crate::{
    video, widgets,
    window::{Ctx, Shell, Size},
    words,
};

mod timeline;
mod tracks;

use timeline::{Marked, Preview};

/// The mini player's size.
const MINI: (i32, i32) = (480, 270);

/// The window as it was before the mini player: its size, and whether it filled the screen.
#[derive(Clone, Copy)]
struct Restore {
    width: i32,
    height: i32,
    maximized: bool,
    fullscreen: bool,
}

/// How long the controls stay up with nothing happening.
const IDLE: Duration = Duration::from_secs(2);
/// The fade, in and out.
const FADE_MS: u32 = 200;

/// The player page and what drives it.
pub struct PlayerView {
    pub widget: gtk::Widget,
    me: Weak<Self>,
    ctx: Rc<Ctx>,
    shell: RefCell<Weak<Shell>>,
    player: Option<Rc<Player>>,
    /// Whether mpv can draw: until then the engine's ops wait here.
    ready: Rc<Cell<bool>>,
    waiting: Rc<RefCell<Vec<(PlaybackId, PlayerOp)>>>,
    gl: Rc<Cell<Option<(i32, i32, bool)>>>,
    ui: Ui,
    shown: Cell<bool>,
    idle: RefCell<Option<glib::SourceId>>,
    fade: RefCell<Option<adw::TimedAnimation>>,
    /// The playhead's own updates are held off while a drag on the timeline seeks.
    held_until: Cell<Instant>,
    state: RefCell<PlayerState>,
    over_bar: Cell<bool>,
    inhibit: Cell<Option<u32>>,
    countdown: RefCell<Option<glib::SourceId>>,
    menus: RefCell<Option<Menus>>,
    /// The chapters changed or a file loaded: read them again once the file has a duration.
    chapters_due: Cell<bool>,
    preview: Rc<Preview>,
    /// The window as it was, while the mini player stands in for it.
    mini: Cell<Option<Restore>>,
    /// Since when the starting screen has waited, the line's tick, and the engine's note on
    /// the start's job.
    starting_since: Cell<Option<Instant>>,
    starting_tick: RefCell<Option<glib::SourceId>>,
    starting_note: RefCell<Option<String>>,
    /// Where the playback resumed, while "Resumed at" shows: it goes 6 s of playback later.
    resumed_from: Cell<Option<f64>>,
    /// The last automatic skip, while Undo is offered for it, and its notice.
    undo: Cell<Option<Undo>>,
    undo_toast: RefCell<Option<adw::Toast>>,
    /// The Subtitles and Audio menu's radio actions, set to the file's tracks as it opens.
    subtitle_action: gio::SimpleAction,
    audio_action: gio::SimpleAction,
}

/// An automatic skip that can still be undone: where it started, when, and where it landed.
#[derive(Clone, Copy)]
struct Undo {
    to: f64,
    since: Instant,
    landed: Option<f64>,
}

impl Undo {
    /// Undo is offered for 6 s of playback after the skip, and at most 15 s while paused.
    fn open(&self, position: Option<f64>) -> bool {
        let played = match (self.landed, position) {
            (Some(landed), Some(now)) => now - landed,
            _ => 0.0,
        };
        played < UNDO_PLAYED && self.since.elapsed() < UNDO_MOST
    }
}

/// How long Undo stays after an automatic skip, in seconds of playback, and at most.
const UNDO_PLAYED: f64 = 6.0;
const UNDO_MOST: Duration = Duration::from_secs(15);

/// How long the starting screen says only "Starting episode 7".
const STARTING_QUIET: Duration = Duration::from_secs(3);
/// How long "Resumed at" stays, in seconds of playback.
const RESUMED_FOR: f64 = 6.0;

/// The widgets the player updates.
/// The episodes, sources and choices the menus offer, and whether they are open.
type Menus = (Vec<MenuEpisode>, Vec<MenuSource>, MenuChoices, bool);

/// What one of the page's actions does.
type Handler = Box<dyn Fn(&PlayerView, Option<&glib::Variant>)>;

struct Ui {
    page: adw::ToolbarView,
    /// The bottom bar's controls that narrow windows leave out: the keys still do them.
    roomy: Vec<gtk::Widget>,
    top: adw::HeaderBar,
    bottom: gtk::Box,
    dim: gtk::Box,
    stage: gtk::Overlay,
    show: gtk::Label,
    episode: gtk::Label,
    play: gtk::Button,
    timeline: gtk::Scale,
    marked: Marked,
    position: gtk::Label,
    remaining: gtk::Label,
    volume: gtk::Scale,
    starting: gtk::Box,
    starting_art: gtk::Picture,
    starting_line: gtk::Label,
    cards: gtk::Box,
    skip: gtk::Box,
    skip_button: gtk::Button,
    up_next: gtk::Box,
    up_next_title: gtk::Label,
    up_next_line: gtk::Label,
    up_next_bar: gtk::ProgressBar,
    stall: gtk::Box,
    stall_line: gtk::Label,
    resumed: gtk::Box,
    resumed_line: gtk::Label,
    episodes: gtk::MenuButton,
    tracks: gtk::MenuButton,
    settings: gtk::MenuButton,
    menu_buttons: Vec<gtk::MenuButton>,
    /// The mini player's controls, over the video in place of the bars.
    mini: gtk::Box,
    mini_play: gtk::Button,
}

impl PlayerView {
    pub fn new(ctx: &Rc<Ctx>, options: &PlayerOptions) -> Rc<Self> {
        let (host_tx, host_rx) = async_channel::unbounded::<HostEvent>();
        let engine: std::sync::Weak<Engine> = ctx.hub.engine_link();
        // Reports go to the engine from mpv's thread; the rest to the main loop.
        let sink = Arc::new(move |event: HostEvent| match event {
            HostEvent::Report { playback, report } => {
                if let Some(engine) = engine.upgrade() {
                    engine.send(Command::PlayerReport { playback, report });
                }
            }
            other => {
                let _ = host_tx.send_blocking(other);
            }
        });
        let player = match Player::new(options, sink) {
            Ok(player) => Some(Rc::new(player)),
            Err(e) => {
                tracing::error!(error = %e, "mpv did not start");
                None
            }
        };

        let ready = Rc::new(Cell::new(false));
        let waiting: Rc<RefCell<Vec<(PlaybackId, PlayerOp)>>> = Rc::default();
        let (video_widget, gl) = match &player {
            Some(p) => {
                let area = video::video_area(p.mpv().clone(), {
                    let (ready, waiting, player) =
                        (Rc::clone(&ready), Rc::clone(&waiting), Rc::clone(p));
                    move || {
                        ready.set(true);
                        let ops: Vec<_> = waiting.borrow_mut().drain(..).collect();
                        tracing::info!(waiting = ops.len(), "mpv can draw");
                        for (playback, op) in &ops {
                            player.apply(*playback, op);
                        }
                    }
                });
                if std::env::var_os("ANISTREAM_STATS").is_some() {
                    log_pacing(Rc::clone(&area.pacing), Rc::clone(p));
                }
                (area.widget, area.gl)
            }
            None => (gtk::Box::new(gtk::Orientation::Vertical, 0).upcast(), Rc::default()),
        };

        let ui = build_ui(&video_widget);
        let preview = Preview::new(&ui.timeline, false);
        ctx.on_size({
            let roomy = ui.roomy.clone();
            move |size| {
                for widget in &roomy {
                    widget.set_visible(size != Size::Narrow);
                }
            }
        });
        let view = Rc::new_cyclic(|me| Self {
            widget: ui.page.clone().upcast(),
            me: me.clone(),
            ctx: Rc::clone(ctx),
            shell: RefCell::default(),
            player,
            ready,
            waiting,
            gl,
            ui,
            shown: Cell::new(true),
            idle: RefCell::default(),
            fade: RefCell::default(),
            held_until: Cell::new(Instant::now()),
            state: RefCell::default(),
            over_bar: Cell::new(false),
            inhibit: Cell::new(None),
            countdown: RefCell::default(),
            menus: RefCell::default(),
            chapters_due: Cell::new(false),
            preview,
            mini: Cell::new(None),
            starting_since: Cell::new(None),
            starting_tick: RefCell::default(),
            starting_note: RefCell::default(),
            resumed_from: Cell::new(None),
            undo: Cell::new(None),
            undo_toast: RefCell::default(),
            subtitle_action: gio::SimpleAction::new_stateful(
                "subtitle",
                Some(glib::VariantTy::STRING),
                &"off".to_variant(),
            ),
            audio_action: gio::SimpleAction::new_stateful(
                "audio",
                Some(glib::VariantTy::STRING),
                &"off".to_variant(),
            ),
        });
        view.wire();
        glib::spawn_future_local({
            let view = Rc::downgrade(&view);
            async move {
                while let Ok(event) = host_rx.recv().await {
                    let Some(view) = view.upgrade() else { return };
                    view.host_event(event);
                }
            }
        });
        view
    }

    /// The window the player shows in, once it is built.
    pub fn attach(&self, shell: Weak<Shell>) {
        self.shell.replace(shell);
    }

    fn shell(&self) -> Option<Rc<Shell>> {
        self.shell.borrow().upgrade()
    }

    fn send(&self, command: PlayerCommand) {
        self.ctx.hub.send(Command::Player(command));
    }

    pub fn focus(&self) {
        self.ui.play.grab_focus();
        self.wake();
    }

    /// The viewer closed the player: Esc, the back button, Alt+Left.
    pub fn close(&self) {
        if let Some(player) = &self.player {
            player.viewer_closed();
        }
        self.closed();
    }

    /// The window is closing: whatever plays ends as if the viewer closed it.
    pub fn window_closed(&self) {
        if let Some(player) = &self.player {
            player.viewer_closed();
        }
    }

    fn closed(&self) {
        self.set_mini(false);
        self.preview.forget();
        self.ui.marked.set(Vec::new(), 0.0);
        if let Some(shell) = self.shell() {
            shell.show_player(false);
        }
        if self.ctx.window.is_fullscreen() {
            self.ctx.window.unfullscreen();
        }
        if let (Some(cookie), Some(app)) = (self.inhibit.take(), self.ctx.window.application())
        {
            app.uninhibit(cookie);
        }
        self.clear_cards();
        self.forget_undo();
        self.ui.starting.set_visible(true);
        self.ui.starting.set_opacity(1.0);
    }

    fn open(&self) {
        if let Some(shell) = self.shell()
            && !shell.is_player()
        {
            shell.show_player(true);
        }
    }

    /// What the engine said, as the player hears it.
    pub fn engine_event(&self, event: &Event) {
        match event {
            Event::PlayerOps { playback, ops } => {
                if ops.iter().any(|op| matches!(op, PlayerOp::Open { .. })) {
                    self.open();
                }
                let Some(player) = &self.player else {
                    self.ctx.toast("The player could not start");
                    return;
                };
                for op in ops {
                    match op {
                        PlayerOp::Load { url, options } => {
                            let options = options
                                .iter()
                                .map(|o| (o.name.clone(), o.value.clone()))
                                .collect();
                            self.preview.stream(url, options);
                            self.ui.marked.set(Vec::new(), 0.0);
                            self.chapters_due.set(true);
                        }
                        PlayerOp::Set { property, .. } if property == "chapter-list" => {
                            self.chapters_due.set(true);
                        }
                        _ => {}
                    }
                    if self.ready.get() {
                        player.apply(*playback, op);
                    } else {
                        self.waiting.borrow_mut().push((*playback, op.clone()));
                    }
                }
            }
            Event::PlayerUi { ui, .. } => self.player_ui(ui),
            Event::JobNote { job, note, .. } => {
                let starting = self
                    .ctx
                    .hub
                    .known
                    .borrow()
                    .jobs
                    .get(job)
                    .is_some_and(|activity| activity.starts_with("starting"));
                if starting {
                    self.starting_note.replace(Some(crate::catalog::shown(note)));
                }
            }
            _ => {}
        }
    }

    /// The starting screen's line: "Starting episode 7", and from 3 s the wait admitted, with
    /// the engine's note when it has one: "Finding a source · 4 s".
    fn tick_starting(&self, episode: &str) {
        if let Some(tick) = self.starting_tick.borrow_mut().take() {
            tick.remove();
        }
        self.starting_since.set(Some(Instant::now()));
        self.starting_note.replace(None);
        self.ui.starting_line.set_label(&format!("Starting episode {episode}"));
        let me = self.me.clone();
        let tick = glib::timeout_add_local(Duration::from_secs(1), move || {
            let Some(view) = me.upgrade() else { return glib::ControlFlow::Break };
            let since = view.starting_since.get();
            let (Some(since), true) = (since, view.ui.starting.is_visible()) else {
                view.starting_tick.borrow_mut().take();
                return glib::ControlFlow::Break;
            };
            let waited = since.elapsed();
            if waited >= STARTING_QUIET {
                let note =
                    view.starting_note.borrow().clone().unwrap_or("Finding a source".into());
                view.ui.starting_line.set_label(&format!("{note} · {} s", waited.as_secs()));
            }
            glib::ControlFlow::Continue
        });
        self.starting_tick.replace(Some(tick));
    }

    /// "Resumed at 12:04" with Start Over, in the top leading corner.
    fn resumed(&self, position: f64) {
        self.ui.resumed_line.set_label(&format!("Resumed at {}", words::clock(position)));
        self.resumed_from.set(Some(position));
        self.ui.resumed.set_visible(true);
        crate::motion::fade_in(&self.ui.resumed);
    }

    fn resumed_done(&self) {
        if self.resumed_from.take().is_some() && self.ui.resumed.is_visible() {
            crate::motion::fade_out(&self.ui.resumed);
        }
    }

    fn player_ui(&self, ui: &PlayerUi) {
        match ui {
            PlayerUi::Starting {
                title, episode, episode_title, banner_url, cover_url, ..
            } => {
                self.ui.show.set_label(title);
                self.ui.episode.set_label(&episode_line(episode, episode_title.as_deref()));
                self.tick_starting(episode);
                self.resumed_done();
                self.ui.starting.set_visible(true);
                self.ui.starting.set_opacity(1.0);
                self.ui.starting_art.set_paintable(Some(&widgets::swatch(None)));
                self.ctx
                    .hub
                    .art(&self.ui.starting_art, banner_url.as_deref().or(cover_url.as_deref()));
                self.clear_cards();
                self.open();
            }
            PlayerUi::Episode { number, title, show, .. } => {
                self.ui.show.set_label(show);
                self.ui.episode.set_label(&episode_line(number, title.as_deref()));
            }
            PlayerUi::Menus { episodes, sources, upscaling, night_mode, .. } => {
                self.menus.replace(Some((
                    episodes.clone(),
                    sources.clone(),
                    upscaling.clone(),
                    *night_mode,
                )));
                self.rebuild_menus();
            }
            PlayerUi::Skip { kind, .. } => {
                self.ui.skip_button.set_label(match kind {
                    SkipKind::Opening => "Skip Opening",
                    SkipKind::Ending => "Skip Ending",
                });
                self.ui.skip.set_visible(true);
            }
            PlayerUi::SkipCleared => self.ui.skip.set_visible(false),
            PlayerUi::AutoSkipped { kind, undo_to } => self.auto_skipped(*kind, *undo_to),
            PlayerUi::UpNext { next, countdown, auto, .. } => {
                self.ui.up_next_title.set_label(&next.label());
                self.ui.up_next.set_visible(true);
                self.count_down(if *auto { *countdown } else { 0 });
            }
            PlayerUi::UpNextCleared => {
                self.ui.up_next.set_visible(false);
                if let Some(source) = self.countdown.borrow_mut().take() {
                    source.remove();
                }
            }
            PlayerUi::Stall { seconds } => {
                self.ui.stall_line.set_label(&format!("Nothing has played for {seconds} s."));
                self.ui.stall.set_visible(true);
                self.wake();
            }
            PlayerUi::StallCleared => self.ui.stall.set_visible(false),
            PlayerUi::Alert { text, .. } => self.ctx.toast(&crate::catalog::shown(text)),
            PlayerUi::Thumbnails { url } => self.preview.thumbnails(url.as_deref()),
            PlayerUi::Resumed { position } => self.resumed(*position),
            _ => {}
        }
        self.ui.cards.set_visible(
            self.ui.skip.is_visible()
                || self.ui.up_next.is_visible()
                || self.ui.stall.is_visible(),
        );
    }

    fn clear_cards(&self) {
        for card in [&self.ui.skip, &self.ui.up_next, &self.ui.stall] {
            card.set_visible(false);
        }
        self.ui.cards.set_visible(false);
        if let Some(source) = self.countdown.borrow_mut().take() {
            source.remove();
        }
    }

    /// "Plays in 8 s" and the bar under it, down to the engine's own play.
    fn count_down(&self, seconds: u32) {
        if let Some(source) = self.countdown.borrow_mut().take() {
            source.remove();
        }
        if seconds == 0 {
            self.ui.up_next_line.set_label("Up next");
            self.ui.up_next_bar.set_visible(false);
            return;
        }
        self.ui.up_next_bar.set_visible(true);
        let left = Rc::new(Cell::new(seconds));
        let (line, bar) = (self.ui.up_next_line.clone(), self.ui.up_next_bar.clone());
        let say = move |left: u32| {
            line.set_label(&format!("Plays in {left} s"));
            bar.set_fraction(f64::from(left) / f64::from(seconds));
        };
        say(seconds);
        let weak = self.self_weak();
        let source = glib::timeout_add_seconds_local(1, move || {
            let now = left.get().saturating_sub(1);
            left.set(now);
            say(now);
            if now > 0 {
                return glib::ControlFlow::Continue;
            }
            // Done: forgotten, so clearing the cards later does not remove it a second time.
            if let Some(view) = weak.upgrade() {
                view.countdown.borrow_mut().take();
            }
            glib::ControlFlow::Break
        });
        self.countdown.replace(Some(source));
    }

    /// "Skipped the opening" with Undo, for 6 s of playback: the button, U, or Ctrl+Z.
    fn auto_skipped(&self, kind: SkipKind, undo_to: f64) {
        self.forget_undo();
        self.undo.set(Some(Undo { to: undo_to, since: Instant::now(), landed: None }));
        let text = match kind {
            SkipKind::Opening => "Skipped the opening",
            SkipKind::Ending => "Skipped the ending",
        };
        let toast = self.ctx.toast_with(text, "Undo", "win.undo-skip");
        toast.set_timeout(UNDO_PLAYED as u32);
        let me = self.me.clone();
        toast.connect_dismissed(move |toast| {
            if let Some(view) = me.upgrade()
                && view.undo_toast.borrow().as_ref() == Some(toast)
            {
                view.undo_toast.replace(None);
            }
        });
        self.undo_toast.replace(Some(toast));
    }

    /// Back to where the last automatic skip started, while Undo is offered. True when it was.
    fn undo_skip(&self) -> bool {
        let position = self.state.borrow().position;
        let Some(undo) = self.undo.get().filter(|u| u.open(position)) else {
            self.forget_undo();
            return false;
        };
        self.send(PlayerCommand::UndoSkip { to: undo.to });
        self.forget_undo();
        true
    }

    fn forget_undo(&self) {
        self.undo.set(None);
        if let Some(toast) = self.undo_toast.take() {
            toast.dismiss();
        }
    }

    /// The Subtitles and Audio menu, from the file's tracks as they are now.
    fn fill_tracks(&self) {
        let list = self.player.as_ref().and_then(|p| p.mpv().get("track-list"));
        let tracks = list.as_ref().map(tracks::tracks).unwrap_or_default();
        let playing = !tracks.is_empty() && !self.ui.starting.is_visible();
        self.subtitle_action
            .set_state(&tracks::chosen(&tracks, TrackKind::Subtitle).to_variant());
        self.audio_action.set_state(&tracks::chosen(&tracks, TrackKind::Audio).to_variant());
        self.ui.tracks.set_menu_model(Some(&tracks::menu(&tracks, playing)));
    }

    /// A track picked from the menu: the engine selects it and keeps it for the series.
    fn choose_track(&self, kind: TrackKind, target: &str) {
        let id = if target == "off" { None } else { target.parse().ok() };
        if id.is_none() && (target != "off" || kind == TrackKind::Audio) {
            return;
        }
        let action = match kind {
            TrackKind::Subtitle => &self.subtitle_action,
            TrackKind::Audio => &self.audio_action,
        };
        action.set_state(&target.to_variant());
        self.send(PlayerCommand::ChooseTrack { kind, id });
    }

    /// Load a subtitle or audio track from a file, through the file chooser portal, and
    /// select it. mpv's alone: the next episode does not carry it.
    fn load_track(&self, kind: TrackKind) {
        let title = match kind {
            TrackKind::Subtitle => "Load Subtitle File",
            TrackKind::Audio => "Load Audio File",
        };
        let dialog = gtk::FileDialog::builder()
            .title(title)
            .modal(true)
            .filters(&tracks::filter(kind))
            .build();
        let me = self.me.clone();
        dialog.open(Some(&self.ctx.window), gio::Cancellable::NONE, move |picked| {
            let Some(view) = me.upgrade() else { return };
            let file = match picked {
                Ok(file) => file,
                // Dismissed: nothing to say.
                Err(e) if e.matches(gtk::DialogError::Dismissed) => return,
                Err(e) => {
                    tracing::warn!(error = %e, "the file chooser failed");
                    view.ctx.toast("Couldn't open the file chooser");
                    return;
                }
            };
            let Some(path) = file.path() else {
                view.ctx.toast("Couldn't read that file");
                return;
            };
            let path = path.to_string_lossy().into_owned();
            let added = view.player.as_ref().map(|player| {
                player.mpv().command(&[tracks::add_command(kind), &path, "select"])
            });
            match added {
                Some(Ok(())) => view.fill_tracks(),
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "mpv did not load the track");
                    view.ctx.toast(match kind {
                        TrackKind::Subtitle => "Couldn't load that subtitle file",
                        TrackKind::Audio => "Couldn't load that audio file",
                    });
                }
                None => view.ctx.toast("The player could not start"),
            }
            view.wake();
        });
    }

    fn host_event(&self, event: HostEvent) {
        match event {
            HostEvent::Opened { fullscreen } => {
                self.open();
                self.report_display();
                let start_fullscreen = self
                    .ctx
                    .hub
                    .known
                    .borrow()
                    .setting(anistream_model::settings::SettingKey::Fullscreen)
                    .is_some_and(|r| crate::catalog::is_on(&r));
                if fullscreen || start_fullscreen {
                    self.ctx.window.fullscreen();
                }
                if self.inhibit.get().is_none()
                    && let Some(app) = self.ctx.window.application()
                {
                    let cookie = app.inhibit(
                        Some(&self.ctx.window),
                        gtk::ApplicationInhibitFlags::IDLE
                            | gtk::ApplicationInhibitFlags::SUSPEND,
                        Some("Playing an episode"),
                    );
                    // Zero when there is no session to ask, as in a headless run.
                    self.inhibit.set((cookie > 0).then_some(cookie));
                }
                self.wake();
                self.lift(self.shown.get());
            }
            HostEvent::Closed => self.closed(),
            HostEvent::Fullscreen(on) => self.fullscreen(on),
            HostEvent::State(state) => self.show_state(state),
            HostEvent::Report { .. } => {}
        }
    }

    fn fullscreen(&self, on: Option<bool>) {
        let window = &self.ctx.window;
        if on.unwrap_or(!window.is_fullscreen()) {
            window.fullscreen();
        } else {
            window.unfullscreen();
        }
    }

    fn show_state(&self, state: PlayerState) {
        let ui = &self.ui;
        let (icon, tip) = if state.paused {
            ("media-playback-start-symbolic", "Play")
        } else {
            ("media-playback-pause-symbolic", "Pause")
        };
        for button in [&ui.play, &ui.mini_play] {
            button.set_icon_name(icon);
            button.set_tooltip_text(Some(tip));
        }
        if let Some(duration) = state.duration {
            ui.timeline.set_range(0.0, duration.max(0.1));
        }
        if let Some(position) = state.position {
            ui.position.set_label(&words::clock(position));
            if let Some(duration) = state.duration {
                ui.remaining.set_label(&format!("-{}", words::clock(duration - position)));
            }
            if Instant::now() >= self.held_until.get() {
                ui.timeline.set_value(position);
            }
        }
        if state.has_picture && ui.starting.is_visible() {
            fade(&ui.starting, false);
            self.starting_since.set(None);
        }
        // Six seconds of playback on, or a seek back past it, and "Resumed at" goes.
        if let (Some(from), Some(position)) = (self.resumed_from.get(), state.position)
            && (position > from + RESUMED_FOR || position + 1.0 < from)
        {
            self.resumed_done();
        }
        if let (Some(player), Some(duration)) = (&self.player, state.duration) {
            if state.has_picture && self.chapters_due.replace(false) {
                let chapters = player.mpv().get("chapter-list").map(|v| timeline::chapters(&v));
                ui.marked
                    .set(timeline::segments(&chapters.unwrap_or_default(), duration), duration);
            }
            // The buffered part of the track: as far as the cache reaches.
            if let Some(cached) =
                player.mpv().get("demuxer-cache-time").and_then(|v| v.as_f64())
            {
                ui.timeline.set_fill_level(cached.min(duration));
            }
        }
        // Where an automatic skip landed, which Undo's 6 s of playback count from.
        if let (Some(mut undo), Some(position)) = (self.undo.get(), state.position)
            && undo.landed.is_none()
        {
            undo.landed = Some(position);
            self.undo.set(Some(undo));
        }
        let paused_now = state.paused && !self.state.borrow().paused;
        self.state.replace(state);
        // Controls stay up while paused.
        if paused_now {
            self.wake();
        }
    }

    /// The display the window is on, for the engine's upscaling.
    fn report_display(&self) {
        let Some(player) = &self.player else { return };
        let window = &self.ctx.window;
        let Some(surface) = window.surface() else { return };
        let Some(monitor) = WidgetExt::display(window).monitor_at_surface(&surface) else {
            return;
        };
        let geometry = monitor.geometry();
        let scale = monitor.scale();
        let refresh = monitor.refresh_rate();
        let compute =
            self.gl.get().is_some_and(|(major, minor, gles)| !gles && (major, minor) >= (4, 3));
        player.display(
            (f64::from(geometry.width()) * scale) as u32,
            (f64::from(geometry.height()) * scale) as u32,
            scale,
            (refresh > 0).then(|| f64::from(refresh) / 1000.0),
            compute,
        );
    }

    /// Show the controls, and hide them again after the idle time unless something holds them.
    fn wake(&self) {
        if !self.shown.replace(true) {
            for widget in self.chrome() {
                widget.set_visible(true);
            }
            self.ui.page.set_cursor(None);
            self.lift(true);
            self.animate(true);
        }
        if let Some(source) = self.idle.borrow_mut().take() {
            source.remove();
        }
        let weak = self.self_weak();
        let source = glib::timeout_add_local_once(IDLE, move || {
            if let Some(view) = weak.upgrade() {
                view.idle.borrow_mut().take();
                view.rest();
            }
        });
        self.idle.replace(Some(source));
    }

    /// Hide the controls, unless paused, scrubbing, a menu is open or the pointer is on a bar.
    fn rest(&self) {
        let menu_open = self.ui.menu_buttons.iter().any(|b| b.is_active());
        let card = self.ui.stall.is_visible();
        if self.state.borrow().paused
            || self.over_bar.get()
            || menu_open
            || card
            || !self.shown.get()
        {
            if self.shown.get() {
                self.wake();
            }
            return;
        }
        self.shown.set(false);
        self.ui.page.set_cursor_from_name(Some("none"));
        self.preview.hide();
        self.lift(false);
        self.animate(false);
    }

    /// Subtitles above the bottom bar while it shows, at once: never under it.
    fn lift(&self, up: bool) {
        let Some(player) = &self.player else { return };
        let bar = if up && self.mini.get().is_none() {
            let height = self.ui.bottom.height();
            if height > 0 {
                height
            } else {
                self.ui.bottom.measure(gtk::Orientation::Vertical, -1).1
            }
        } else {
            0
        };
        timeline::lift_subtitles(player.mpv(), bar, self.ui.stage.height());
    }

    /// The mini player: the window itself, small, undecorated and with three controls, or back
    /// to what it was.
    pub fn set_mini(&self, on: bool) {
        let window = &self.ctx.window;
        match (on, self.mini.get()) {
            (true, None) => {
                let (width, height) = window.default_size();
                self.mini.set(Some(Restore {
                    width: if width > 0 { width } else { window.width() },
                    height: if height > 0 { height } else { window.height() },
                    maximized: window.is_maximized(),
                    fullscreen: window.is_fullscreen(),
                }));
                // The bars go; the mini controls stand in for them.
                for widget in self.chrome() {
                    widget.set_visible(false);
                }
                self.ui.top.set_visible(false);
                self.ui.bottom.set_visible(false);
                window.unfullscreen();
                window.unmaximize();
                window.set_decorated(false);
                window.set_size_request(MINI.0, MINI.1);
                window.set_default_size(MINI.0, MINI.1);
                self.shown.set(false);
                self.preview.hide();
                self.lift(false);
                self.wake();
            }
            (false, Some(restore)) => {
                self.mini.set(None);
                self.ui.mini.set_visible(false);
                window.set_decorated(true);
                window.set_size_request(360, 360);
                window.set_default_size(restore.width, restore.height);
                if restore.maximized {
                    window.maximize();
                }
                if restore.fullscreen {
                    window.fullscreen();
                }
                self.shown.set(false);
                self.wake();
            }
            _ => {}
        }
    }

    fn chrome(&self) -> Vec<gtk::Widget> {
        if self.mini.get().is_some() {
            return vec![self.ui.mini.clone().upcast(), self.ui.dim.clone().upcast()];
        }
        vec![
            self.ui.top.clone().upcast(),
            self.ui.bottom.clone().upcast(),
            self.ui.dim.clone().upcast(),
        ]
    }

    /// Fade the bars and the dim in or out; faded out, they are unmapped.
    fn animate(&self, show: bool) {
        if let Some(old) = self.fade.borrow_mut().take() {
            old.skip();
        }
        let chrome = self.chrome();
        let reduced = !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations());
        let target = adw::CallbackAnimationTarget::new({
            let chrome = chrome.clone();
            move |value| {
                for widget in &chrome {
                    widget.set_opacity(value);
                }
            }
        });
        let (from, to) = if show { (0.0, 1.0) } else { (1.0, 0.0) };
        let animation = adw::TimedAnimation::new(
            &self.ui.page,
            from,
            to,
            if reduced { 0 } else { FADE_MS },
            target,
        );
        if !show {
            let weak = self.self_weak();
            animation.connect_done(move |_| {
                if let Some(view) = weak.upgrade()
                    && !view.shown.get()
                {
                    for widget in view.chrome() {
                        widget.set_visible(false);
                    }
                }
            });
        }
        animation.play();
        self.fade.replace(Some(animation));
    }

    fn self_weak(&self) -> Weak<Self> {
        self.me.clone()
    }

    /// The episodes menu and the settings menu, from what the engine offers.
    fn rebuild_menus(&self) {
        let Some((episodes, sources, upscaling, night)) = self.menus.borrow().clone() else {
            return;
        };
        let list = gio::Menu::new();
        for episode in episodes.iter().take(200) {
            let label = match &episode.title {
                Some(title) => format!("{} · {title}", episode.label),
                None => episode.label.clone(),
            };
            let label = if episode.current { format!("{label} (playing)") } else { label };
            list.append(Some(&label), Some(&format!("player.episode::{}", episode.number)));
        }
        self.ui.episodes.set_menu_model(Some(&list));

        let menu = gio::Menu::new();
        let source_menu = gio::Menu::new();
        for source in &sources {
            let label = if source.current {
                format!("{} (playing)", source.label)
            } else {
                source.label.clone()
            };
            source_menu.append(Some(&label), Some(&format!("player.source::{}", source.id)));
        }
        if !sources.is_empty() {
            menu.append_section(Some("Source"), &source_menu);
        }
        let picture = gio::Menu::new();
        for choice in &upscaling.options {
            let label = if choice.id == upscaling.current {
                format!("{} (on)", choice.label)
            } else {
                choice.label.clone()
            };
            picture.append(Some(&label), Some(&format!("player.upscaling::{}", choice.id)));
        }
        picture.append(
            Some(if night { "Night Mode (on)" } else { "Night Mode" }),
            Some("player.night"),
        );
        menu.append_section(Some("Upscaling"), &picture);
        self.ui.settings.set_menu_model(Some(&menu));
    }

    fn wire(self: &Rc<Self>) {
        let ui = &self.ui;
        let weak = Rc::downgrade(self);
        let act = move |f: fn(&PlayerView)| {
            let weak = weak.clone();
            move || {
                if let Some(view) = weak.upgrade() {
                    f(&view);
                }
            }
        };

        // The page's actions, for its buttons and menus.
        let group = gio::SimpleActionGroup::new();
        let add = |name: &str, kind: Option<&glib::VariantTy>, f: Handler| {
            let action = gio::SimpleAction::new(name, kind);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, arg| {
                if let Some(view) = weak.upgrade() {
                    f(&view, arg);
                    view.wake();
                }
            });
            group.add_action(&action);
        };
        add("back", None, Box::new(|v, _| v.close()));
        add("toggle", None, Box::new(|v, _| v.send(PlayerCommand::TogglePause)));
        add("back-10", None, Box::new(|v, _| v.send(PlayerCommand::Seek(-10.0))));
        add("forward-10", None, Box::new(|v, _| v.send(PlayerCommand::Seek(10.0))));
        add("fullscreen", None, Box::new(|v, _| v.fullscreen(None)));
        add("mini", None, Box::new(|v, _| v.set_mini(v.mini.get().is_none())));
        add("skip", None, Box::new(|v, _| v.send(PlayerCommand::TakeSkip)));
        add(
            "undo-skip",
            None,
            Box::new(|v, _| {
                v.undo_skip();
            }),
        );
        add("load-subtitle", None, Box::new(|v, _| v.load_track(TrackKind::Subtitle)));
        add("load-audio", None, Box::new(|v, _| v.load_track(TrackKind::Audio)));
        for (action, kind) in [
            (&self.subtitle_action, TrackKind::Subtitle),
            (&self.audio_action, TrackKind::Audio),
        ] {
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, arg| {
                if let (Some(view), Some(target)) =
                    (weak.upgrade(), arg.and_then(String::from_variant))
                {
                    view.choose_track(kind, &target);
                    view.wake();
                }
            });
            group.add_action(action);
        }
        add("play-next", None, Box::new(|v, _| v.send(PlayerCommand::PlayNext)));
        add("cancel-next", None, Box::new(|v, _| v.send(PlayerCommand::CancelNext)));
        add("stall-switch", None, Box::new(|v, _| v.send(PlayerCommand::StallSwitch)));
        add("stall-wait", None, Box::new(|v, _| v.send(PlayerCommand::StallWait)));
        add(
            "start-over",
            None,
            Box::new(|v, _| {
                v.send(PlayerCommand::SeekTo(0.0));
                v.resumed_done();
            }),
        );
        add(
            "night",
            None,
            Box::new(|v, _| {
                let on = v.menus.borrow().as_ref().is_some_and(|m| m.3);
                v.send(PlayerCommand::NightMode(!on));
            }),
        );
        add(
            "speed",
            Some(glib::VariantTy::DOUBLE),
            Box::new(|v, a| {
                if let Some(speed) = a.and_then(f64::from_variant) {
                    v.send(PlayerCommand::SpeedTo(speed));
                }
            }),
        );
        add(
            "episode",
            Some(glib::VariantTy::STRING),
            Box::new(|v, a| {
                if let Some(n) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Choose(PlayerChoice::Episode(n)));
                }
            }),
        );
        add(
            "source",
            Some(glib::VariantTy::STRING),
            Box::new(|v, a| {
                if let Some(id) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Choose(PlayerChoice::Source(id)));
                }
            }),
        );
        add(
            "upscaling",
            Some(glib::VariantTy::STRING),
            Box::new(|v, a| {
                if let Some(id) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Upscaling(id));
                }
            }),
        );
        ui.page.insert_action_group("player", Some(&group));
        // The skip notice's Undo, which a toast over the whole window runs.
        let undo = gio::SimpleAction::new("undo-skip", None);
        undo.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, _| {
                if let Some(view) = weak.upgrade() {
                    view.undo_skip();
                    view.wake();
                }
            }
        });
        self.ctx.window.add_action(&undo);
        // The track menu is read from the file each time it opens.
        ui.tracks.set_create_popup_func({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.fill_tracks();
                }
            }
        });

        // A drag on the timeline seeks.
        ui.timeline.connect_change_value({
            let weak = Rc::downgrade(self);
            move |_, _, value| {
                if let Some(view) = weak.upgrade() {
                    view.held_until.set(Instant::now() + Duration::from_millis(400));
                    view.send(PlayerCommand::SeekTo(value));
                    view.wake();
                }
                glib::Propagation::Proceed
            }
        });
        // Hovering the timeline shows the frame there.
        let hover = gtk::EventControllerMotion::new();
        hover.connect_motion({
            let weak = Rc::downgrade(self);
            move |_, x, _| {
                let Some(view) = weak.upgrade() else { return };
                let has_duration = view.state.borrow().duration.is_some_and(|d| d > 0.0);
                if let Some(seconds) = timeline::time_at(&view.ui.timeline, x)
                    && has_duration
                {
                    view.preview.show(x, seconds, view.ui.marked.at(seconds));
                }
            }
        });
        hover.connect_leave({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.preview.hide();
                }
            }
        });
        ui.timeline.add_controller(hover);
        ui.volume.connect_change_value({
            let weak = Rc::downgrade(self);
            move |_, _, value| {
                if let Some(view) = weak.upgrade() {
                    let now = view
                        .player
                        .as_ref()
                        .and_then(|p| p.mpv().get("volume"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(100.0);
                    view.send(PlayerCommand::Volume(value - now));
                }
                glib::Propagation::Proceed
            }
        });

        // The pointer shows the controls; over a bar it holds them.
        let motion = gtk::EventControllerMotion::new();
        let last = Cell::new((0.0, 0.0));
        let wake = act(|v| v.wake());
        motion.connect_motion(move |_, x, y| {
            // GTK reports a motion when the cursor changes; only a real move wakes.
            if last.replace((x, y)) != (x, y) {
                wake();
            }
        });
        ui.page.add_controller(motion);
        for bar in [ui.top.clone().upcast::<gtk::Widget>(), ui.bottom.clone().upcast()] {
            let hover = gtk::EventControllerMotion::new();
            let weak = Rc::downgrade(self);
            hover.connect_enter(move |_, _, _| {
                if let Some(view) = weak.upgrade() {
                    view.over_bar.set(true);
                }
            });
            let weak = Rc::downgrade(self);
            hover.connect_leave(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.over_bar.set(false);
                    view.wake();
                }
            });
            bar.add_controller(hover);
        }

        // Click plays or pauses; double-click toggles fullscreen.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, presses, x, y| {
            let Some(view) = weak.upgrade() else { return };
            // Only on the video, not on a card.
            let picked = view.ui.stage.pick(x, y, gtk::PickFlags::DEFAULT);
            if picked.is_some_and(|w| w.is_ancestor(&view.ui.cards)) {
                return;
            }
            if gesture.current_event().is_some_and(|e| {
                e.device().is_some_and(|d| d.source() == gdk::InputSource::Touchscreen)
            }) {
                // A tap toggles the controls instead.
                if view.shown.get() {
                    view.rest_now()
                } else {
                    view.wake()
                }
                return;
            }
            if view.mini.get().is_some() {
                // The mini player is dragged by its picture; a double-click is the full window.
                if presses == 2 {
                    view.set_mini(false);
                } else {
                    view.wake();
                }
                return;
            }
            match presses {
                1 => view.send(PlayerCommand::TogglePause),
                2 => {
                    view.send(PlayerCommand::TogglePause);
                    view.fullscreen(None);
                }
                _ => {}
            }
        });
        ui.stage.add_controller(click);
        let drag = gtk::GestureDrag::new();
        let weak = Rc::downgrade(self);
        drag.connect_drag_begin(move |gesture, x, y| {
            let Some(view) = weak.upgrade() else { return };
            if view.mini.get().is_none() {
                return;
            }
            let window = &view.ctx.window;
            let (Some(surface), Some(device)) = (window.surface(), gesture.device()) else {
                return;
            };
            let Ok(toplevel) = surface.downcast::<gdk::Toplevel>() else { return };
            let point = view
                .ui
                .stage
                .compute_point(window, &gtk::graphene::Point::new(x as f32, y as f32))
                .unwrap_or_else(|| gtk::graphene::Point::new(x as f32, y as f32));
            let (dx, dy) = window.surface_transform();
            toplevel.begin_move(
                &device,
                gesture.current_button() as i32,
                f64::from(point.x()) + dx,
                f64::from(point.y()) + dy,
                gesture.current_event_time(),
            );
        });
        ui.stage.add_controller(drag);

        // The wheel steps the volume; horizontal scrolling seeks.
        let scroll =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        let weak = Rc::downgrade(self);
        scroll.connect_scroll(move |_, dx, dy| {
            let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
            if dy.abs() > dx.abs() {
                view.send(PlayerCommand::Volume(-dy.signum() * 5.0));
            } else if dx != 0.0 {
                view.send(PlayerCommand::Seek(dx.signum() * 5.0));
            }
            view.wake();
            glib::Propagation::Stop
        });
        ui.page.add_controller(scroll);

        // The keys.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
            if view.key(key, modifiers) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        ui.page.add_controller(keys);
    }

    /// For a shot: "Resumed at" as a resumed playback shows it, `at` said and the six seconds
    /// counted from where the test pattern is.
    #[cfg(feature = "fixtures")]
    pub fn shot_resumed(&self, at: f64) {
        self.resumed(at);
        self.resumed_from.set(self.state.borrow().position);
    }

    /// For a shot: an opening and an ending on the timeline, and `subtitle` on screen.
    #[cfg(feature = "fixtures")]
    pub fn shot_marks(&self, subtitle: &str) {
        let Some(player) = &self.player else { return };
        let list = serde_json::json!([
            { "title": "Episode", "time": 0.0 },
            { "title": "Opening", "time": 4.0 },
            { "title": "Episode", "time": 16.0 },
            { "title": "Ending", "time": 50.0 },
        ]);
        let _ = player.mpv().set("chapter-list", &list);
        let _ = player.mpv().command(&["sub-add", subtitle, "select"]);
        self.chapters_due.set(true);
    }

    /// For a shot: the Subtitles and Audio menu open, with `subtitle` added as a track.
    #[cfg(feature = "fixtures")]
    pub fn shot_tracks(&self, subtitle: Option<&str>) {
        if let (Some(player), Some(subtitle)) = (&self.player, subtitle) {
            let _ =
                player.mpv().command(&["sub-add", subtitle, "select", "Lantern Road", "eng"]);
            return;
        }
        self.ui.tracks.popup();
        self.wake();
    }

    /// For a shot: the notice an automatic skip of the opening shows, with its Undo.
    #[cfg(feature = "fixtures")]
    pub fn shot_auto_skipped(&self) {
        self.auto_skipped(SkipKind::Opening, 2.0);
        self.wake();
    }

    /// For a shot: the Subtitles and Audio menu's popover, under its button, over the window.
    pub fn shot_tracks_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        let popover = self.ui.tracks.popover()?;
        if !popover.is_visible() {
            return None;
        }
        let window = &self.ctx.window;
        let button = self.ui.tracks.compute_bounds(window)?;
        let width = popover.width() as f32;
        let x = (button.x() + button.width() / 2.0 - width / 2.0)
            .clamp(0.0, (window.width() as f32 - width).max(0.0));
        Some((popover.upcast(), x, button.y() + button.height()))
    }

    /// For a shot: the seek preview's popover and where it is over the window.
    pub fn shot_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        self.preview.placed(&self.ctx.window)
    }

    /// For a shot: the seek preview `fraction` of the way along, as hovering there shows it.
    #[cfg(feature = "fixtures")]
    pub fn shot_preview(&self, fraction: f64) {
        let Some(duration) = self.state.borrow().duration else { return };
        let Some(x) = timeline::x_at(&self.ui.timeline, fraction) else { return };
        let seconds = duration * fraction;
        self.preview.show(x, seconds, self.ui.marked.at(seconds));
        self.wake();
    }

    /// Close whichever of the player's menus is open.
    pub fn close_menus(&self) {
        for button in &self.ui.menu_buttons {
            button.popdown();
        }
    }

    fn rest_now(&self) {
        if let Some(source) = self.idle.borrow_mut().take() {
            source.remove();
        }
        self.rest();
    }

    /// A key on the player. True when it was the player's.
    fn key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        use gdk::Key;
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let step = if shift { 30.0 } else { 5.0 };
        // Arrows and S act without showing the controls; every other key shows them.
        let quiet =
            matches!(key, Key::Left | Key::Right | Key::Up | Key::Down | Key::s | Key::S);
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        if control && matches!(key, Key::z | Key::Z) && !shift {
            // Edit's Undo, for the last automatic skip.
            return self.undo_skip();
        }
        if modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
            return false;
        }
        match key {
            Key::space | Key::k | Key::K => self.send(PlayerCommand::TogglePause),
            Key::Left => self.send(PlayerCommand::Seek(-step)),
            Key::Right => self.send(PlayerCommand::Seek(step)),
            Key::j | Key::J => self.send(PlayerCommand::Seek(-10.0)),
            Key::l | Key::L => self.send(PlayerCommand::Seek(10.0)),
            Key::Up => self.send(PlayerCommand::Volume(5.0)),
            Key::Down => self.send(PlayerCommand::Volume(-5.0)),
            Key::m | Key::M => {
                if let Some(player) = &self.player {
                    let _ = player.mpv().command(&["cycle", "mute"]);
                }
            }
            Key::f | Key::F | Key::F11 if self.mini.get().is_none() => self.fullscreen(None),
            Key::p | Key::P => self.set_mini(self.mini.get().is_none()),
            Key::s | Key::S if self.ui.skip.is_visible() => self.send(PlayerCommand::TakeSkip),
            Key::n => self.send(PlayerCommand::PlayNext),
            Key::N => self.send(PlayerCommand::Step(-1)),
            Key::bracketleft => self.send(PlayerCommand::Speed(-0.25)),
            Key::bracketright => self.send(PlayerCommand::Speed(0.25)),
            Key::BackSpace => self.send(PlayerCommand::SpeedTo(1.0)),
            Key::e | Key::E => self.ui.episodes.popup(),
            Key::t | Key::T => self.ui.tracks.popup(),
            Key::u | Key::U => {
                if !self.undo_skip() {
                    return false;
                }
            }
            Key::Return | Key::KP_Enter => {
                if self.ui.skip.is_visible() {
                    self.send(PlayerCommand::TakeSkip);
                } else if self.ui.up_next.is_visible() {
                    self.send(PlayerCommand::PlayNext);
                } else if self.ui.stall.is_visible() {
                    self.send(PlayerCommand::StallSwitch);
                } else {
                    return false;
                }
            }
            Key::Escape => {
                if self.ui.up_next.is_visible() {
                    self.send(PlayerCommand::CancelNext);
                } else if self.ui.stall.is_visible() {
                    self.send(PlayerCommand::StallWait);
                } else if self.mini.get().is_some() {
                    self.set_mini(false);
                } else if self.ctx.window.is_fullscreen() {
                    self.ctx.window.unfullscreen();
                } else {
                    self.close();
                }
            }
            _ => return false,
        }
        if !quiet {
            self.wake();
        }
        true
    }
}

/// Every two seconds, how evenly frames were drawn and what mpv says of its own timing.
fn log_pacing(pacing: Rc<video::Pacing>, player: Rc<Player>) {
    glib::timeout_add_seconds_local(2, move || {
        let (fps, worst) = pacing.take();
        let mpv = player.mpv();
        let get = |p: &str| mpv.get(p).map(|v| v.to_string()).unwrap_or_else(|| "-".into());
        tracing::info!(
            drawn_fps = format!("{fps:.1}"),
            worst_gap_ms = format!("{worst:.1}"),
            video_fps = get("estimated-vf-fps"),
            drops = get("frame-drop-count"),
            hwdec = get("hwdec-current"),
            "pacing"
        );
        glib::ControlFlow::Continue
    });
}

fn episode_line(number: &str, title: Option<&str>) -> String {
    match title {
        Some(title) if !title.is_empty() => format!("Episode {number} · {title}"),
        _ => format!("Episode {number}"),
    }
}

/// Fade `widget` in or out, unmapping it once it is out.
fn fade(widget: &gtk::Box, show: bool) {
    let target = adw::PropertyAnimationTarget::new(widget, "opacity");
    let (from, to) = if show { (0.0, 1.0) } else { (widget.opacity(), 0.0) };
    let animation = adw::TimedAnimation::new(widget, from, to, 300, target);
    if !show {
        let widget = widget.clone();
        animation.connect_done(move |_| widget.set_visible(false));
    } else {
        widget.set_visible(true);
    }
    animation.play();
}

fn icon_button(icon: &str, tip: &str, action: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tip));
    button.set_action_name(Some(action));
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.set_valign(gtk::Align::Center);
    button
}

fn card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("corner-card");
    card.add_css_class("osd");
    card.set_visible(false);
    card
}

fn build_ui(video: &gtk::Widget) -> Ui {
    // The top bar: back, the show and the episode, and at the end episodes, settings and
    // fullscreen, with the window's own close button.
    let back = icon_button("go-previous-symbolic", "Back", "player.back");
    let show = widgets::show_title("", "row");
    show.set_xalign(0.5);
    show.set_wrap(false);
    show.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let episode = gtk::Label::new(None);
    episode.add_css_class("caption");
    episode.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
    titles.set_valign(gtk::Align::Center);
    titles.append(&show);
    titles.append(&episode);
    let episodes = gtk::MenuButton::builder()
        .icon_name("view-list-symbolic")
        .tooltip_text("Episodes")
        .valign(gtk::Align::Center)
        .build();
    episodes.add_css_class("flat");
    let tracks = gtk::MenuButton::builder()
        .icon_name("media-view-subtitles-symbolic")
        .tooltip_text("Subtitles and Audio")
        .valign(gtk::Align::Center)
        .build();
    tracks.add_css_class("flat");
    let full = icon_button("view-fullscreen-symbolic", "Fullscreen", "player.fullscreen");
    let mini_button =
        icon_button("anistream-mini-player-symbolic", "Mini Player", "player.mini");
    let top = adw::HeaderBar::new();
    top.add_css_class("osd");
    top.add_css_class("player-top");
    top.pack_start(&back);
    top.set_title_widget(Some(&titles));
    top.pack_end(&full);
    top.pack_end(&mini_button);
    top.pack_end(&episodes);
    top.pack_end(&tracks);

    // The bottom bar.
    let play = icon_button("media-playback-pause-symbolic", "Pause", "player.toggle");
    play.add_css_class("play");
    let back10 =
        icon_button("media-seek-backward-symbolic", "Back 10 Seconds", "player.back-10");
    let forward10 =
        icon_button("media-seek-forward-symbolic", "Forward 10 Seconds", "player.forward-10");
    let position = gtk::Label::new(Some("0:00"));
    let remaining = gtk::Label::new(Some("-0:00"));
    for label in [&position, &remaining] {
        label.add_css_class("numeric");
        label.add_css_class("caption");
    }
    let timeline = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    timeline.set_hexpand(true);
    timeline.set_draw_value(false);
    timeline.add_css_class("timeline");
    timeline.update_property(&[gtk::accessible::Property::Label("Timeline")]);
    timeline.set_show_fill_level(true);
    timeline.set_restrict_to_fill_level(false);
    let marked = Marked::new(&timeline);

    let volume = gtk::Scale::with_range(gtk::Orientation::Vertical, 0.0, 100.0, 5.0);
    volume.set_inverted(true);
    volume.set_value(100.0);
    volume.set_size_request(-1, 140);
    volume.set_draw_value(false);
    let volume_popover = gtk::Popover::builder().child(&volume).build();
    let volume_button = gtk::MenuButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .tooltip_text("Volume")
        .popover(&volume_popover)
        .valign(gtk::Align::Center)
        .build();
    volume_button.add_css_class("flat");
    let speeds = gio::Menu::new();
    for speed in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
        let label = if speed == 1.0 { "Normal".to_owned() } else { format!("{speed}×") };
        let item = gio::MenuItem::new(Some(&label), None);
        item.set_action_and_target_value(Some("player.speed"), Some(&speed.to_variant()));
        speeds.append_item(&item);
    }
    let speed_button = gtk::MenuButton::builder()
        .icon_name("preferences-system-time-symbolic")
        .tooltip_text("Speed")
        .menu_model(&speeds)
        .valign(gtk::Align::Center)
        .build();
    speed_button.add_css_class("flat");
    let settings = gtk::MenuButton::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("Playback Settings")
        .valign(gtk::Align::Center)
        .build();
    settings.add_css_class("flat");

    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bottom.add_css_class("player-bar");
    bottom.add_css_class("osd");
    for widget in [
        play.upcast_ref::<gtk::Widget>(),
        back10.upcast_ref(),
        forward10.upcast_ref(),
        position.upcast_ref(),
    ] {
        bottom.append(widget);
    }
    bottom.append(&marked.widget);
    bottom.append(&remaining);
    bottom.append(&volume_button);
    bottom.append(&speed_button);
    bottom.append(&settings);

    // The starting screen: the banner full frame, the line and a spinner.
    let starting_art = gtk::Picture::new();
    starting_art.set_content_fit(gtk::ContentFit::Cover);
    starting_art.set_can_shrink(true);
    starting_art.add_css_class("art-fade");
    let spinner = adw::Spinner::new();
    spinner.set_size_request(32, 32);
    let starting_line = gtk::Label::new(Some("Starting"));
    starting_line.add_css_class("title-3");
    let middle = gtk::Box::new(gtk::Orientation::Vertical, 16);
    middle.set_halign(gtk::Align::Center);
    middle.set_valign(gtk::Align::Center);
    middle.append(&spinner);
    middle.append(&starting_line);
    let starting_stack = gtk::Overlay::new();
    starting_stack.set_child(Some(&starting_art));
    let shade = gtk::Box::new(gtk::Orientation::Vertical, 0);
    shade.add_css_class("player-dim");
    starting_stack.add_overlay(&shade);
    starting_stack.add_overlay(&middle);
    let starting = gtk::Box::new(gtk::Orientation::Vertical, 0);
    starting.add_css_class("player-starting");
    starting.append(&starting_stack);
    starting_stack.set_vexpand(true);
    starting.set_can_target(false);

    // The corner cards.
    let skip = card();
    let skip_button = gtk::Button::with_label("Skip Opening");
    skip_button.add_css_class("pill");
    skip_button.set_action_name(Some("player.skip"));
    skip.append(&skip_button);

    let up_next = card();
    let up_next_line = widgets::haze("Up next", "caption");
    let up_next_title = gtk::Label::new(None);
    up_next_title.set_xalign(0.0);
    up_next_title.set_wrap(true);
    up_next_title.set_max_width_chars(30);
    let up_next_bar = gtk::ProgressBar::new();
    up_next_bar.add_css_class("countdown");
    let up_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let play_now = gtk::Button::with_label("Play Now");
    play_now.add_css_class("pill");
    play_now.add_css_class("suggested-action");
    play_now.set_action_name(Some("player.play-next"));
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("pill");
    cancel.set_action_name(Some("player.cancel-next"));
    up_buttons.append(&play_now);
    up_buttons.append(&cancel);
    up_next.append(&up_next_line);
    up_next.append(&up_next_title);
    up_next.append(&up_next_bar);
    up_next.append(&up_buttons);

    let stall = card();
    let stall_title = gtk::Label::new(Some("This Source Is Slow"));
    stall_title.add_css_class("heading");
    stall_title.set_xalign(0.0);
    let stall_line = widgets::haze("", "caption");
    let stall_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let switch = gtk::Button::with_label("Switch Source");
    switch.add_css_class("pill");
    switch.add_css_class("suggested-action");
    switch.set_action_name(Some("player.stall-switch"));
    let wait = gtk::Button::with_label("Keep Waiting");
    wait.add_css_class("pill");
    wait.set_action_name(Some("player.stall-wait"));
    stall_buttons.append(&switch);
    stall_buttons.append(&wait);
    stall.append(&stall_title);
    stall.append(&stall_line);
    stall.append(&stall_buttons);

    // "Resumed at 12:04" and Start Over, top leading, under the top bar.
    let resumed = card();
    resumed.set_orientation(gtk::Orientation::Horizontal);
    resumed.set_spacing(12);
    resumed.set_halign(gtk::Align::Start);
    resumed.set_valign(gtk::Align::Start);
    resumed.set_margin_top(72);
    let resumed_line = gtk::Label::new(None);
    resumed_line.add_css_class("numeric");
    resumed_line.set_valign(gtk::Align::Center);
    let start_over = gtk::Button::with_label("Start Over");
    start_over.add_css_class("pill");
    start_over.set_action_name(Some("player.start-over"));
    resumed.append(&resumed_line);
    resumed.append(&start_over);

    let cards = gtk::Box::new(gtk::Orientation::Vertical, 8);
    cards.set_halign(gtk::Align::End);
    cards.set_valign(gtk::Align::End);
    cards.set_margin_end(24);
    cards.set_margin_bottom(96);
    cards.append(&skip);
    cards.append(&up_next);
    cards.append(&stall);
    cards.set_visible(false);

    let dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dim.add_css_class("player-dim");
    dim.set_can_target(false);

    // The mini player's controls: play and pause, back and forward 10 s in the middle, and the
    // way back to the full window in the corner.
    let mini = gtk::Box::new(gtk::Orientation::Vertical, 0);
    mini.set_visible(false);
    let full_window =
        icon_button("anistream-mini-player-symbolic", "Back to the Full Window", "player.mini");
    full_window.set_halign(gtk::Align::End);
    full_window.set_margin_top(8);
    full_window.set_margin_end(8);
    full_window.add_css_class("osd");
    mini.append(&full_window);
    let middle_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    middle_row.set_halign(gtk::Align::Center);
    middle_row.set_valign(gtk::Align::Center);
    middle_row.set_vexpand(true);
    middle_row.add_css_class("mini-controls");
    let mini_play = icon_button("media-playback-pause-symbolic", "Pause", "player.toggle");
    for button in [
        icon_button("media-seek-backward-symbolic", "Back 10 Seconds", "player.back-10"),
        mini_play.clone(),
        icon_button("media-seek-forward-symbolic", "Forward 10 Seconds", "player.forward-10"),
    ] {
        button.add_css_class("osd");
        middle_row.append(&button);
    }
    mini.append(&middle_row);

    let stage = gtk::Overlay::new();
    stage.add_css_class("player");
    stage.set_child(Some(video));
    stage.add_overlay(&dim);
    stage.add_overlay(&starting);
    stage.add_overlay(&cards);
    stage.add_overlay(&resumed);
    stage.add_overlay(&mini);

    // The bars over the video, which fills the page under them.
    let page = adw::ToolbarView::new();
    page.add_css_class("player-page");
    page.add_top_bar(&top);
    page.add_bottom_bar(&bottom);
    page.set_content(Some(&stage));
    page.set_extend_content_to_top_edge(true);
    page.set_extend_content_to_bottom_edge(true);
    page.set_focusable(true);

    let menu_buttons = vec![
        episodes.clone(),
        tracks.clone(),
        settings.clone(),
        speed_button.clone(),
        volume_button,
    ];
    Ui {
        roomy: vec![back10.upcast(), forward10.upcast(), speed_button.upcast()],
        page,
        top,
        bottom,
        dim,
        stage,
        show,
        episode,
        play,
        timeline,
        marked,
        position,
        remaining,
        volume,
        starting,
        starting_art,
        starting_line,
        cards,
        skip,
        skip_button,
        up_next,
        up_next_title,
        up_next_line,
        up_next_bar,
        stall,
        stall_line,
        resumed,
        resumed_line,
        episodes,
        tracks,
        settings,
        menu_buttons,
        mini,
        mini_play,
    }
}
