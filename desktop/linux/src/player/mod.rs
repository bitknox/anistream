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
use anistream_core::{ids::AnilistId, media::Translation};
use anistream_engine::engine::Engine;
use anistream_i18n::t;
use anistream_model::{
    command::{Command, PlayerCommand},
    event::Event,
    hosted::{
        MenuChoices, MenuEpisode, MenuSource, PlayerChoice, PlayerOp, PlayerUi, TrackKind,
    },
    input::{PadContext, PlayerCard, PlayerFocus, PlayerPanel},
    playback::{PlaybackId, SkipKind},
};
use anistream_mpv_host::{HostEvent, Player, PlayerOptions, PlayerState};
use gtk::{gdk, gio, glib};

use crate::{
    mpris::{Action as MediaAction, Mpris, Track},
    say, video, widgets,
    window::{Ctx, Shell, Size},
};

mod coming;
mod stats;
mod timeline;
pub(crate) mod tracks;

use timeline::{Marked, Preview};

/// The mini player's size.
const MINI: (i32, i32) = (480, 270);
/// The window's smallest size once the mini player has given it back.
const MINI_MIN: (i32, i32) = (360, 360);

/// The window as it was before the mini player: its size, and whether it filled the screen.
#[derive(Clone, Copy)]
struct Restore {
    width: i32,
    height: i32,
    maximized: bool,
    fullscreen: bool,
}

/// Whether a mini player toggle may go ahead, `since` the last one.
fn mini_ready(since: Option<Duration>) -> bool {
    since.is_none_or(|since| since >= MINI_SETTLE)
}

/// The frame to bring back after the mini player. A quick toggle finds the window still on its
/// way back from the mini size, which GTK reports as the default size and which is no frame
/// worth keeping: the one kept from before stands instead.
fn frame_to_keep(now: Restore, kept: Option<Restore>) -> Restore {
    let small = now.width <= MINI.0 && now.height <= MINI_MIN.1;
    match kept {
        Some(kept) if small => kept,
        _ => now,
    }
}

/// How long a mini player toggle is left to land before the next one counts.
const MINI_SETTLE: Duration = Duration::from_millis(400);
/// How long after a mini player toggle the window may still be on its way to its size.
const SETTLE: Duration = Duration::from_secs(2);

/// How long the controls stay up with nothing happening.
const IDLE: Duration = Duration::from_secs(2);
/// The controls' fades, in and out: the fade-in and fade-out tokens.
const FADE_IN_MS: u32 = 200;
const FADE_OUT_MS: u32 = 300;
/// The speed menu's speeds, which `[` and `]` step along.
const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
/// How long a speed asked for stands in for mpv's report of it.
const SPEED_ASKED: Duration = Duration::from_millis(1500);

/// The speed a step starts from: the one asked for last while it is recent, else mpv's.
fn step_from(asked: Option<(f64, Duration)>, reported: f64) -> f64 {
    match asked {
        Some((speed, ago)) if ago < SPEED_ASKED => speed,
        _ => reported,
    }
}

/// The speed one step faster or slower than `now`, if there is one.
fn next_speed(now: f64, faster: bool) -> Option<f64> {
    if faster {
        SPEEDS.iter().copied().find(|s| *s > now + 0.001)
    } else {
        SPEEDS.iter().rev().copied().find(|s| *s < now - 0.001)
    }
}

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
    /// The whole second the seek bar's spoken value was last set for.
    said_second: Cell<i64>,
    state: RefCell<PlayerState>,
    over_bar: Cell<bool>,
    /// A screenshot run's: no Up next card, for a fixture clip short enough to be near its
    /// end from the start.
    pub hold_up_next: Cell<bool>,
    inhibit: Cell<Option<u32>>,
    /// The Up next card's plan, from the engine's word and what the viewer did with it.
    coming: RefCell<Option<coming::Coming>>,
    menus: RefCell<Option<Menus>>,
    /// The chapters changed or a file loaded: read them again once the file has a duration.
    chapters_due: Cell<bool>,
    /// The openings and endings the engine marked, once it has said; until then the file's
    /// chapters stand in.
    engine_marks: RefCell<Option<Vec<timeline::Segment>>>,
    preview: Rc<Preview>,
    /// The window as it was, while the mini player stands in for it.
    mini: Cell<Option<Restore>>,
    /// The frame kept for the last mini player, for a toggle that comes before the window has
    /// settled.
    frame: Cell<Option<(Restore, Instant)>>,
    /// When the mini player last toggled.
    mini_at: Cell<Option<Instant>>,
    /// Since when the starting screen has waited, the line's tick, and the engine's note on
    /// the start's job.
    starting_since: Cell<Option<Instant>>,
    starting_tick: RefCell<Option<glib::SourceId>>,
    starting_note: RefCell<Option<String>>,
    /// Where the playback resumed, while "Resumed at" shows: it goes 6 s of playback later.
    resumed_from: Cell<Option<f64>>,
    /// The last automatic skip, while Undo is offered for it.
    undo: Cell<Option<Undo>>,
    /// Since when the skip offer has shown, for a controller's A to take it only while it is new.
    skip_since: Cell<Option<Instant>>,
    /// The Subtitles and Audio menu's radio actions, set to the file's tracks as it opens.
    subtitle_action: gio::SimpleAction,
    audio_action: gio::SimpleAction,
    /// The other menus' radio and check actions, set to what the engine says is chosen.
    episode_action: gio::SimpleAction,
    source_action: gio::SimpleAction,
    upscaling_action: gio::SimpleAction,
    night_action: gio::SimpleAction,
    /// Stats for nerds, up or away: the More menu's check, I and the panel's close button.
    stats_action: gio::SimpleAction,
    speed_action: gio::SimpleAction,
    /// Sub or Dub, at the top of the Subtitles and Audio menu when the title has a dub.
    translation_action: gio::SimpleAction,
    /// The title playing, and whether it has a dub, once the engine has said.
    title_id: Cell<Option<AnilistId>>,
    /// Whether the episode is far enough on (85%) for the More menu to offer its discussion.
    late: Cell<bool>,
    has_dub: Cell<Option<bool>>,
    /// The episode on screen, and its translation, for its line and the Up Next card's.
    episode_line: RefCell<(String, Option<String>, Option<Translation>)>,
    /// The speed mpv plays at, as the speed button and the media controls show it.
    speed: Cell<f64>,
    /// The speed asked for last and when, until mpv has had time to report it.
    speed_asked: Cell<Option<(f64, Instant)>>,
    /// Since when the stream has stalled, while the card says so, and the card's tick.
    stall_since: Cell<Option<Instant>>,
    stall_tick: RefCell<Option<glib::SourceId>>,
    /// The media controls on the session bus, once served.
    mpris: RefCell<Option<Rc<Mpris>>>,
    /// The watch party's panel, on the people button.
    party: Rc<crate::party::panel::Panel>,
    stats: Rc<stats::Stats>,
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
    skip: Card,
    skip_button: gtk::Button,
    /// "Skipped opening · Undo", after an automatic skip.
    undo: Card,
    undo_line: gtk::Label,
    up_next: Card,
    up_next_title: gtk::Label,
    up_next_line: gtk::Label,
    up_next_bar: gtk::ProgressBar,
    up_next_still: gtk::Picture,
    up_next_still_frame: crate::fit::Fit,
    stall: Card,
    stall_line: gtk::Label,
    resumed: gtk::Box,
    resumed_line: gtk::Label,
    episodes: gtk::MenuButton,
    tracks: gtk::MenuButton,
    /// Watch together: the form that starts a party, or the party's panel once in one, with a
    /// dot on the people while in one.
    people: gtk::Button,
    people_dot: gtk::Box,
    /// Play on TV: hands what plays over to a TV.
    tv: gtk::Button,
    settings: gtk::MenuButton,
    speed: gtk::MenuButton,
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
        let party = crate::party::panel::Panel::new(ctx);
        party.popover.set_parent(&ui.people);
        let stats = stats::Stats::new();
        ui.stage.add_overlay(&stats.widget);
        stats.set_narrow(ctx.size() == Size::Narrow);
        ctx.on_size({
            let roomy = ui.roomy.clone();
            let stats = Rc::clone(&stats);
            move |size| {
                for widget in &roomy {
                    widget.set_visible(size != Size::Narrow);
                }
                stats.set_narrow(size == Size::Narrow);
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
            said_second: Cell::new(-1),
            state: RefCell::default(),
            over_bar: Cell::new(false),
            hold_up_next: Cell::new(false),
            inhibit: Cell::new(None),
            coming: RefCell::default(),
            menus: RefCell::default(),
            chapters_due: Cell::new(false),
            engine_marks: RefCell::new(None),
            preview,
            mini: Cell::new(None),
            starting_since: Cell::new(None),
            starting_tick: RefCell::default(),
            starting_note: RefCell::default(),
            resumed_from: Cell::new(None),
            undo: Cell::new(None),
            skip_since: Cell::new(None),
            subtitle_action: radio("subtitle", "off"),
            audio_action: radio("audio", "off"),
            episode_action: radio("episode", ""),
            source_action: radio("source", ""),
            upscaling_action: radio("upscaling", ""),
            night_action: gio::SimpleAction::new_stateful("night", None, &false.to_variant()),
            stats_action: gio::SimpleAction::new_stateful("stats", None, &false.to_variant()),
            speed_action: gio::SimpleAction::new_stateful(
                "speed",
                Some(glib::VariantTy::DOUBLE),
                &1.0f64.to_variant(),
            ),
            translation_action: radio("translation", "sub"),
            title_id: Cell::new(None),
            late: Cell::new(false),
            has_dub: Cell::new(None),
            episode_line: RefCell::default(),
            speed: Cell::new(1.0),
            speed_asked: Cell::new(None),
            frame: Cell::new(None),
            mini_at: Cell::new(None),
            stall_since: Cell::new(None),
            stall_tick: RefCell::default(),
            mpris: RefCell::default(),
            party,
            stats,
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

    /// Serve the media controls as `name` on the session bus: what they ask comes back here.
    pub fn serve_media_controls(self: &Rc<Self>, name: &str) {
        let view = Rc::downgrade(self);
        let mpris = Mpris::serve(name, move |action| {
            if let Some(view) = view.upgrade() {
                view.media_control(action);
            }
        });
        self.mpris.replace(Some(mpris));
        // A TV driven from here is what the media controls show and press while nothing
        // plays on this computer.
        let view = Rc::downgrade(self);
        self.ctx.tv.listen(move || {
            if let Some(view) = view.upgrade() {
                view.tv_media();
            }
        });
    }

    /// The TV is what plays: driven from here, playing something, and nothing open in the
    /// player on this computer.
    fn tv_drives(&self) -> bool {
        let tv = &self.ctx.tv;
        tv.live() && tv.now().is_some() && !self.shell().is_some_and(|s| s.is_player())
    }

    /// Show the TV's episode to the media controls, or nothing once the TV has stopped.
    fn tv_media(&self) {
        let Some(media) = self.media() else { return };
        if self.shell().is_some_and(|s| s.is_player()) {
            return;
        }
        let tv = &self.ctx.tv;
        let Some(now) = tv.now().filter(|_| tv.connected().is_some()) else {
            media.stopped();
            return;
        };
        let entry = self.ctx.hub.entry(now.id);
        let show = match &entry {
            _ if !now.title.is_empty() => now.title.clone(),
            Some(entry) => entry.title.clone(),
            None => String::new(),
        };
        media.track(Track {
            show,
            title: now.episode_line.clone(),
            art: entry.and_then(|e| e.banner_url.or(e.cover_url)),
        });
        media.state(now.paused, tv.playhead().or(Some(now.position)), now.duration);
        media.neighbours(true, false);
    }

    /// What the media controls asked, sent to the TV.
    fn tv_media_control(&self, action: MediaAction) {
        use anistream_engine::remote::protocol::command::RemoteCommand;
        let tv = &self.ctx.tv;
        let paused = tv.now().is_some_and(|n| n.paused);
        let command = match action {
            MediaAction::Raise => {
                self.ctx.window.present();
                tv.open_remote();
                return;
            }
            MediaAction::PlayPause => RemoteCommand::TogglePause,
            MediaAction::Play if paused => RemoteCommand::TogglePause,
            MediaAction::Pause if !paused => RemoteCommand::TogglePause,
            MediaAction::Next => RemoteCommand::Next,
            MediaAction::SeekBy(by) => RemoteCommand::Seek { by },
            MediaAction::SeekTo(position) => RemoteCommand::SeekTo { position },
            MediaAction::Volume(level) => {
                let now = tv.now().and_then(|n| n.volume).unwrap_or(100.0);
                RemoteCommand::Volume { step: level * 100.0 - now }
            }
            // The TV has no episode before, nor speeds, and Stop leaves it playing.
            MediaAction::Play
            | MediaAction::Pause
            | MediaAction::Previous
            | MediaAction::Stop
            | MediaAction::Rate(_) => return,
        };
        tv.send(command);
    }

    /// What GNOME's media controls, the lock screen or a media key asked.
    fn media_control(&self, action: MediaAction) {
        if self.tv_drives() {
            self.tv_media_control(action);
            return;
        }
        let paused = self.state.borrow().paused;
        match action {
            MediaAction::Raise => self.ctx.window.present(),
            MediaAction::PlayPause => self.send(PlayerCommand::TogglePause),
            MediaAction::Play if paused => self.send(PlayerCommand::TogglePause),
            MediaAction::Pause if !paused => self.send(PlayerCommand::TogglePause),
            MediaAction::Play | MediaAction::Pause => {}
            MediaAction::Stop => self.close(),
            MediaAction::Next => self.send(PlayerCommand::Step(1)),
            MediaAction::Previous => self.send(PlayerCommand::Step(-1)),
            MediaAction::SeekBy(by) => self.send(PlayerCommand::Seek(by)),
            MediaAction::SeekTo(to) => self.send(PlayerCommand::SeekTo(to)),
            MediaAction::Rate(rate) => self
                .send(PlayerCommand::SpeedTo(rate.clamp(SPEEDS[0], SPEEDS[SPEEDS.len() - 1]))),
            MediaAction::Volume(level) => {
                let now = self
                    .player
                    .as_ref()
                    .and_then(|p| p.mpv().get("volume"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(100.0);
                self.send(PlayerCommand::Volume(level * 100.0 - now));
            }
        }
    }

    fn media(&self) -> Option<Rc<Mpris>> {
        self.mpris.borrow().clone()
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
        self.ui.marked.set_buffered(Vec::new());
        self.engine_marks.replace(None);
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
        if let Some(media) = self.media() {
            media.stopped();
        }
        self.tv_media();
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
                    // The engine's player starts with its stats off: asked again, and the
                    // panel waits for the new file's figures.
                    if self.stats.shown() {
                        self.stats.restart();
                        self.send(PlayerCommand::Stats(true));
                    }
                }
                let Some(player) = &self.player else {
                    self.ctx.toast(&t::player_could_not_start());
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
                            self.ui.marked.set_buffered(Vec::new());
                            self.engine_marks.replace(None);
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
            Event::PartyJoined(_) | Event::PartyLeft { .. } => self.show_party_button(),
            Event::PlayerTimeline { marks, .. } => {
                // The engine's marks are the ones skipping uses; they replace the chapters'.
                self.engine_marks.replace(Some(timeline::from_marks(marks)));
                self.chapters_due.set(false);
                let duration = self.state.borrow().duration.unwrap_or(0.0);
                self.show_marks(duration);
            }
            Event::TitleDub { id, dub } if Some(*id) == self.title_id.get() => {
                let had = self.has_dub.replace(Some(dub.is_some()));
                if had != Some(dub.is_some()) && self.ui.tracks.is_active() {
                    self.fill_tracks();
                }
            }
            Event::PlayerStats { stats, .. } => self.stats.took(stats),
            Event::Clipboard { text } => {
                self.ctx.window.clipboard().set_text(text);
                self.stats.copied();
            }
            Event::PlayerBuffered { ranges, .. } => {
                self.ui.marked.set_buffered(ranges.iter().map(|r| (r.start, r.end)).collect());
            }
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
                    self.starting_note.replace(Some(say::msg(note)));
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
        self.ui.starting_line.set_label(&t::player_starting(crate::party::number(episode)));
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
                let seconds = waited.as_secs() as i64;
                let line = match view.starting_note.borrow().as_deref() {
                    Some(note) => t::player_note_waited(note, seconds),
                    None => t::player_finding_source(seconds),
                };
                view.ui.starting_line.set_label(&line);
            }
            glib::ControlFlow::Continue
        });
        self.starting_tick.replace(Some(tick));
    }

    /// "Resumed at 12:04" with Start Over, in the top leading corner.
    fn resumed(&self, position: f64) {
        self.ui.resumed_line.set_label(&t::player_resumed_at(&say::clock(position)));
        self.announce(&self.ui.resumed_line.label());
        self.resumed_from.set(Some(position));
        self.ui.resumed.set_visible(true);
        crate::motion::fade_in(&self.ui.resumed);
    }

    /// Say `text` to a screen reader once.
    fn announce(&self, text: &str) {
        self.ui
            .page
            .upcast_ref::<gtk::Widget>()
            .announce(text, gtk::AccessibleAnnouncementPriority::Medium);
    }

    fn resumed_done(&self) {
        if self.resumed_from.take().is_some() && self.ui.resumed.is_visible() {
            crate::motion::fade_out(&self.ui.resumed);
        }
    }

    fn player_ui(&self, ui: &PlayerUi) {
        match ui {
            PlayerUi::Starting {
                id,
                title,
                episode,
                episode_title,
                banner_url,
                cover_url,
                ..
            } => {
                // Whether the title has a dub, for the Sub and Dub switch: asked once a title.
                if self.title_id.replace(Some(*id)) != Some(*id) {
                    self.has_dub.set(None);
                    self.ctx.hub.send(Command::LoadDub(*id));
                }
                self.ui.show.set_label(title);
                self.late.set(false);
                let translation = self.episode_line.borrow().2;
                self.episode_line.replace((
                    episode.clone(),
                    episode_title.clone(),
                    translation,
                ));
                let line = self.show_episode_line();
                if let Some(media) = self.media() {
                    media.track(Track {
                        show: title.clone(),
                        title: line,
                        art: banner_url.clone().or_else(|| cover_url.clone()),
                    });
                }
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
            PlayerUi::Episode { number, title, show, translation, .. } => {
                self.ui.show.set_label(show);
                if self.episode_line.borrow().0 != *number {
                    self.late.set(false);
                }
                self.episode_line.replace((number.clone(), title.clone(), Some(*translation)));
                self.translation_action.set_state(&translation.as_str().to_variant());
                let line = self.show_episode_line();
                if let Some(media) = self.media() {
                    media.retitle(show, &line);
                }
            }
            PlayerUi::Menus {
                episodes, sources, upscaling, night_mode, translation, ..
            } => {
                self.translation_action.set_state(&translation.as_str().to_variant());
                if *translation == Translation::Dub {
                    self.has_dub.set(Some(true));
                }
                if let Some(media) = self.media() {
                    let at = episodes.iter().position(|e| e.current);
                    media.neighbours(
                        at.is_some_and(|at| at + 1 < episodes.len()),
                        at.is_some_and(|at| at > 0),
                    );
                }
                self.menus.replace(Some((
                    episodes.clone(),
                    sources.clone(),
                    upscaling.clone(),
                    *night_mode,
                )));
                self.rebuild_menus();
            }
            PlayerUi::Skip { kind, .. } => {
                self.ui.skip_button.set_label(&match kind {
                    SkipKind::Opening => t::player_skip_opening(),
                    SkipKind::Ending => t::player_skip_ending(),
                });
                if !self.ui.skip.shown() {
                    self.skip_since.set(Some(Instant::now()));
                    self.announce(&t::player_announce_skip(
                        &self.ui.skip_button.label().unwrap_or_default(),
                    ));
                }
                self.ui.skip.set(true);
            }
            PlayerUi::SkipCleared => {
                self.skip_since.set(None);
                self.ui.skip.set(false);
            }
            PlayerUi::AutoSkipped { kind, undo_to } => self.auto_skipped(*kind, *undo_to),
            PlayerUi::UpNext { .. } if self.hold_up_next.get() => {}
            PlayerUi::UpNext { next, countdown, auto, ending_at, .. } => {
                let translation = self.episode_line.borrow().2;
                self.ui.up_next_title.set_label(&episode_line(
                    &next.number,
                    translation,
                    next.title.as_deref(),
                ));
                self.ui.up_next_title.set_tooltip_text(Some(&self.ui.up_next_title.label()));
                self.ui.up_next_still.set_paintable(Some(&widgets::swatch(None)));
                self.ui.up_next_still_frame.set_visible(next.thumbnail.is_some());
                self.ctx.hub.art(&self.ui.up_next_still, next.thumbnail.as_deref());
                self.coming.replace(Some(coming::Coming::new(
                    *auto && !crate::window::screen_reader_running(),
                    *ending_at,
                    *countdown,
                )));
                self.show_up_next();
            }
            PlayerUi::UpNextCleared => {
                self.coming.replace(None);
                self.ui.up_next.set(false);
            }
            PlayerUi::Stall { seconds } => {
                self.stalled(*seconds);
                self.wake();
            }
            PlayerUi::StallCleared => self.stall_over(),
            PlayerUi::Alert { text, .. } => self.ctx.alert(&crate::catalog::shown(text)),
            PlayerUi::Thumbnails { url } => self.preview.thumbnails(url.as_deref()),
            PlayerUi::Resumed { position } => self.resumed(*position),
            _ => {}
        }
    }

    /// The episode on screen's line, in the top bar and as the media controls title it:
    /// "Episode 7 · Dub · The Lantern Road".
    fn show_episode_line(&self) -> String {
        let (number, title, translation) = self.episode_line.borrow().clone();
        let line = episode_line(&number, translation, title.as_deref());
        self.ui.episode.set_label(&line);
        line
    }

    /// "Source is buffering · 9 s", counting up from what the engine says, with Keep Waiting
    /// and Switch Source.
    fn stalled(&self, seconds: u64) {
        self.stall_since.set(Some(Instant::now() - Duration::from_secs(seconds)));
        let say = |view: &Self| {
            let held = view.stall_since.get().map_or(0, |since| since.elapsed().as_secs());
            view.ui.stall_line.set_label(&t::player_stall_title(held as i64));
        };
        say(self);
        if !self.ui.stall.shown() {
            self.announce(&self.ui.stall_line.label());
        }
        self.ui.stall.set(true);
        if self.stall_tick.borrow().is_some() {
            return;
        }
        let me = self.me.clone();
        let tick = glib::timeout_add_seconds_local(1, move || {
            let Some(view) = me.upgrade() else { return glib::ControlFlow::Break };
            if view.stall_since.get().is_none() {
                view.stall_tick.borrow_mut().take();
                return glib::ControlFlow::Break;
            }
            say(&view);
            glib::ControlFlow::Continue
        });
        self.stall_tick.replace(Some(tick));
    }

    fn stall_over(&self) {
        self.stall_since.set(None);
        if let Some(tick) = self.stall_tick.borrow_mut().take() {
            tick.remove();
        }
        self.ui.stall.set(false);
    }

    fn clear_cards(&self) {
        for card in [&self.ui.skip, &self.ui.undo, &self.ui.up_next, &self.ui.stall] {
            card.hide_now();
        }
        self.stall_over();
        self.coming.replace(None);
    }

    /// The Up next card for the playhead: it shows from where the ending starts, "Plays in 8 s"
    /// counts down on the playhead, and at zero this sends `PlayNext`, once.
    fn show_up_next(&self) {
        let (position, duration) = {
            let state = self.state.borrow();
            (state.position.unwrap_or(0.0), state.duration.unwrap_or(0.0))
        };
        let (card, due) = match self.coming.borrow().as_ref() {
            Some(plan) => (plan.card(position, duration), plan.due(position, duration)),
            None => (coming::Card::Away, false),
        };
        // A screen reader hears the card once, as it appears.
        if !matches!(card, coming::Card::Away) && !self.ui.up_next.shown() {
            let title = self.ui.up_next_title.label();
            self.announce(&match card {
                coming::Card::Counting { left, .. } => {
                    t::player_announce_up_next_counting(&title, left.ceil() as i64)
                }
                _ => t::player_announce_up_next_waiting(&title),
            });
        }
        match card {
            coming::Card::Away => self.ui.up_next.set(false),
            coming::Card::Waiting => {
                self.ui.up_next_line.set_visible(false);
                self.ui.up_next_bar.set_visible(false);
                self.ui.up_next.set(true);
            }
            coming::Card::Counting { left, fraction } => {
                self.ui.up_next_line.set_label(&t::player_plays_in(left.ceil() as i64));
                self.ui.up_next_line.set_visible(true);
                self.ui.up_next_bar.set_fraction(fraction);
                self.ui.up_next_bar.set_visible(true);
                self.ui.up_next.set(true);
            }
        }
        if due {
            if let Some(plan) = self.coming.borrow_mut().as_mut() {
                plan.played = true;
            }
            self.send(PlayerCommand::PlayNext);
        }
    }

    /// Cancel on the card, Escape, or the pad's B: the card goes and the engine stays.
    fn cancel_next(&self) {
        if let Some(plan) = self.coming.borrow_mut().as_mut() {
            plan.cancelled = true;
        }
        self.ui.up_next.set(false);
        self.send(PlayerCommand::CancelNext);
    }

    /// Play now on the card, Enter, or the pad's A.
    fn play_next(&self) {
        if let Some(plan) = self.coming.borrow_mut().as_mut() {
            plan.played = true;
        }
        self.send(PlayerCommand::PlayNext);
    }

    /// "Skipped opening · Undo" in the corner, for 6 s of playback: the button, U, or Ctrl+Z.
    fn auto_skipped(&self, kind: SkipKind, undo_to: f64) {
        self.forget_undo();
        self.undo.set(Some(Undo { to: undo_to, since: Instant::now(), landed: None }));
        self.ui.undo_line.set_label(&match kind {
            SkipKind::Opening => t::player_skipped_opening(),
            SkipKind::Ending => t::player_skipped_ending(),
        });
        self.ui.undo.set(true);
        self.announce(&t::player_announce_undo(&self.ui.undo_line.label()));
        // At most 15 s, even while paused, when no playback counts the 6 s down.
        let me = self.me.clone();
        glib::timeout_add_local_once(UNDO_MOST, move || {
            if let Some(view) = me.upgrade()
                && view.undo.get().is_some_and(|undo| undo.since.elapsed() >= UNDO_MOST)
            {
                view.forget_undo();
            }
        });
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
        self.ui.undo.set(false);
    }

    /// The Subtitles and Audio menu, from the file's tracks as they are now.
    fn fill_tracks(&self) {
        let list = self.player.as_ref().and_then(|p| p.mpv().get("track-list"));
        let tracks = list.as_ref().map(tracks::tracks).unwrap_or_default();
        let playing = !tracks.is_empty() && !self.ui.starting.is_visible();
        self.subtitle_action
            .set_state(&tracks::chosen(&tracks, TrackKind::Subtitle).to_variant());
        self.audio_action.set_state(&tracks::chosen(&tracks, TrackKind::Audio).to_variant());
        let dub = self.has_dub.get() == Some(true);
        self.ui.tracks.set_menu_model(Some(&tracks::menu(&tracks, playing, dub)));
        if dub
            && let Some(popover) = self.ui.tracks.popover().and_downcast::<gtk::PopoverMenu>()
        {
            popover.add_child(&tracks::translation_switch(), tracks::TRANSLATION_SLOT);
        }
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
            TrackKind::Subtitle => t::player_load_subtitle_file(),
            TrackKind::Audio => t::player_load_audio_file(),
        };
        let dialog = gtk::FileDialog::builder()
            .title(title.trim_end_matches('…'))
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
                    view.ctx.toast(&t::player_file_picker_failed());
                    return;
                }
            };
            let Some(path) = file.path() else {
                view.ctx.toast(&t::player_file_unreadable());
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
                    view.ctx.toast(&match kind {
                        TrackKind::Subtitle => t::player_subtitle_file_failed(),
                        TrackKind::Audio => t::player_audio_file_failed(),
                    });
                }
                None => view.ctx.toast(&t::player_could_not_start()),
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
                        Some(&t::player_inhibit_reason()),
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

    fn discussions_on(&self) -> bool {
        self.ctx
            .hub
            .known
            .borrow()
            .setting(anistream_model::settings::SettingKey::EpisodeDiscussions)
            .is_some_and(|row| crate::catalog::is_on(&row))
    }

    fn show_state(&self, state: PlayerState) {
        // From 85% of the episode, the point it counts as watched.
        if !self.late.get()
            && let (Some(position), Some(duration)) = (state.position, state.duration)
            && duration > 0.0
            && position >= duration * 0.85
        {
            self.late.set(true);
            self.rebuild_menus();
        }
        if let Some(media) = self.media() {
            media.state(state.paused, state.position, state.duration);
        }
        let ui = &self.ui;
        let (icon, tip) = if state.paused {
            ("media-playback-start-symbolic", t::action_play())
        } else {
            ("media-playback-pause-symbolic", t::action_pause())
        };
        for button in [&ui.play, &ui.mini_play] {
            button.set_icon_name(icon);
            button.set_tooltip_text(Some(&tip));
        }
        if let Some(duration) = state.duration {
            ui.timeline.set_range(0.0, duration.max(0.1));
        }
        if let Some(position) = state.position {
            ui.position.set_label(&say::clock(position));
            if let Some(duration) = state.duration {
                let left = (duration - position).max(0.0);
                // A minus sign, not a hyphen, and said as time left.
                ui.remaining.set_label(&format!("\u{2212}{}", say::clock(left)));
                ui.remaining.update_property(&[gtk::accessible::Property::Label(
                    &t::player_time_left(&say::spoken_duration(left)),
                )]);
            }
            if Instant::now() >= self.held_until.get() {
                ui.timeline.set_value(position);
                ui.marked.moved();
            }
            // The seek bar says a time, not a number of seconds, and only when the second changes.
            if let Some(duration) = state.duration {
                let second = position as i64;
                if self.said_second.replace(second) != second {
                    ui.timeline.update_property(&[gtk::accessible::Property::ValueText(
                        &t::player_position_spoken(
                            &say::spoken_duration(position),
                            &say::spoken_duration(duration),
                        ),
                    )]);
                }
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
            if self.engine_marks.borrow().is_some() {
                self.show_marks(duration);
            } else if state.has_picture && self.chapters_due.replace(false) {
                let chapters = player.mpv().get("chapter-list").map(|v| timeline::chapters(&v));
                ui.marked
                    .set(timeline::segments(&chapters.unwrap_or_default(), duration), duration);
            }
        }
        // Where an automatic skip landed, which Undo's 6 s of playback count from.
        if let (Some(mut undo), Some(position)) = (self.undo.get(), state.position)
            && undo.landed.is_none()
        {
            undo.landed = Some(position);
            self.undo.set(Some(undo));
        }
        if self.undo.get().is_some_and(|undo| !undo.open(state.position)) {
            self.forget_undo();
        }
        self.read_speed();
        let paused_now = state.paused && !self.state.borrow().paused;
        self.state.replace(state);
        self.show_up_next();
        // Controls stay up while paused.
        if paused_now {
            self.wake();
        }
    }

    /// The speed mpv plays at, onto the speed button, its menu and the media controls.
    fn read_speed(&self) {
        let Some(speed) =
            self.player.as_ref().and_then(|p| p.mpv().get("speed")).and_then(|v| v.as_f64())
        else {
            return;
        };
        if (speed - self.speed.replace(speed)).abs() < 0.001 && self.ui.speed.label().is_some()
        {
            return;
        }
        let label = speed_label(speed);
        self.ui.speed.set_label(&label);
        self.ui.speed.set_tooltip_text(Some(&t::player_speed_is(&label)));
        self.speed_action.set_state(&speed.to_variant());
        if let Some(media) = self.media() {
            media.rate(speed);
        }
    }

    /// One step along the speed menu's speeds, faster or slower: `]` and `[`. Quick presses step
    /// from the speed asked for last, since mpv reports the change back a moment later.
    fn step_speed(&self, faster: bool) {
        let asked = self.speed_asked.get().map(|(speed, at)| (speed, at.elapsed()));
        if let Some(speed) = next_speed(step_from(asked, self.speed.get()), faster) {
            self.ask_speed(speed);
        }
    }

    fn ask_speed(&self, speed: f64) {
        self.speed_asked.set(Some((speed, Instant::now())));
        self.send(PlayerCommand::SpeedTo(speed));
    }

    /// Draw the engine's marks on a timeline `duration` long.
    fn show_marks(&self, duration: f64) {
        if let Some(marks) = self.engine_marks.borrow().as_ref() {
            self.ui.marked.set(marks.clone(), duration);
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

    /// Whether keyboard focus is on one of the controls, which then stay up for it.
    fn focus_on_controls(&self) -> bool {
        let Some(focus) = GtkWindowExt::focus(&self.ctx.window) else { return false };
        let chrome = self.chrome();
        let mut at = Some(focus);
        while let Some(widget) = at {
            if chrome.iter().any(|c| c == &widget) {
                return true;
            }
            at = widget.parent();
        }
        false
    }

    /// Hide the controls, unless paused, scrubbing, a menu is open, the pointer is on a bar,
    /// keyboard focus is on a control or a screen reader runs.
    fn rest(&self) {
        let menu_open = self.ui.menu_buttons.iter().any(|b| b.is_active())
            || self.party.popover.is_visible();
        let card = self.ui.stall.shown();
        if self.state.borrow().paused
            || self.over_bar.get()
            || menu_open
            || card
            || self.focus_on_controls()
            || crate::window::screen_reader_running()
            || !self.shown.get()
        {
            if self.shown.get() {
                self.wake();
            }
            return;
        }
        self.hide_controls();
    }

    /// Take the controls away now, whatever holds them up.
    fn hide_controls(&self) {
        if !self.shown.replace(false) {
            return;
        }
        if let Some(source) = self.idle.borrow_mut().take() {
            source.remove();
        }
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

    /// Stats for nerds up or away, for the rest of this window's playbacks.
    fn toggle_stats(&self) {
        let shown = !self.stats.shown();
        self.stats.set_shown(shown);
        self.stats_action.set_state(&shown.to_variant());
        self.send(PlayerCommand::Stats(shown));
    }

    /// For a shot: Stats for nerds up with the engine's `fixture` held in place of its figures,
    /// `waited` before the file opened, and "Copied" up when `copied`.
    #[cfg(feature = "fixtures")]
    pub fn shot_stats(
        &self,
        fixture: Option<anistream_model::hosted::StatsFixture>,
        waited: Option<f64>,
        copied: bool,
    ) {
        if !self.stats.shown() {
            self.toggle_stats();
        }
        if let Some(fixture) = fixture {
            self.stats.stage(
                anistream_engine::playback::stats::fixture(fixture),
                waited.map(Duration::from_secs_f64),
                copied,
            );
        }
    }

    /// For a shot: Stats for nerds away again.
    #[cfg(feature = "fixtures")]
    pub fn shot_stats_away(&self) {
        if self.stats.shown() {
            self.toggle_stats();
        }
    }

    /// P and the mini button: the other way. The window takes a moment to change size and
    /// decoration, and a toggle before it has landed would read a size that is on its way
    /// somewhere else, so it waits.
    fn toggle_mini(&self) {
        if !mini_ready(self.mini_at.get().map(|at| at.elapsed())) {
            return;
        }
        self.set_mini(self.mini.get().is_none());
    }

    /// The mini player: the window itself, small, undecorated and with three controls, or back
    /// to what it was.
    pub fn set_mini(&self, on: bool) {
        let window = &self.ctx.window;
        self.mini_at.set(Some(Instant::now()));
        match (on, self.mini.get()) {
            (true, None) => {
                let (width, height) = window.default_size();
                let now = Restore {
                    width: if width > 0 { width } else { window.width() },
                    height: if height > 0 { height } else { window.height() },
                    maximized: window.is_maximized(),
                    fullscreen: window.is_fullscreen(),
                };
                let kept = self
                    .frame
                    .get()
                    .filter(|(_, at)| at.elapsed() < SETTLE)
                    .map(|(frame, _)| frame);
                let frame = frame_to_keep(now, kept);
                self.frame.set(Some((frame, Instant::now())));
                self.mini.set(Some(frame));
                // The bars go; the mini controls stand in for them.
                for widget in self.chrome() {
                    widget.set_visible(false);
                }
                self.ui.top.set_visible(false);
                self.ui.bottom.set_visible(false);
                // The corner cards would cover the small picture: Up Next still plays on.
                self.ui.cards.set_visible(false);
                self.stats.set_mini(true);
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
                self.ui.cards.set_visible(true);
                self.stats.set_mini(false);
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

    /// Fade the bars and the dim in or out, from wherever a fade under way had got to: in over
    /// 200 ms ease-out, out over 300 ms ease-in. Faded out, they are unmapped.
    fn animate(&self, show: bool) {
        if let Some(old) = self.fade.borrow_mut().take() {
            old.pause();
        }
        let chrome = self.chrome();
        let now = chrome.first().map_or(if show { 0.0 } else { 1.0 }, |w| w.opacity());
        let reduced = !gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations());
        let target = adw::CallbackAnimationTarget::new({
            let chrome = chrome.clone();
            move |value| {
                for widget in &chrome {
                    widget.set_opacity(value);
                }
            }
        });
        let (to, ms, easing) = if show {
            (1.0, FADE_IN_MS, adw::Easing::EaseOutCubic)
        } else {
            (0.0, FADE_OUT_MS, adw::Easing::EaseInCubic)
        };
        let animation = adw::TimedAnimation::new(
            &self.ui.page,
            now,
            to,
            if reduced { 0 } else { ms },
            target,
        );
        animation.set_easing(easing);
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
        // Radio items for the episode, the source and the look, a check for Night Mode: the
        // menus mark what is chosen themselves, so no label says "playing" or "on".
        let list = gio::Menu::new();
        for episode in episodes.iter().take(200) {
            let label = match &episode.title {
                Some(title) => [episode.label.as_str(), title.as_str()].join(" · "),
                None => episode.label.clone(),
            };
            list.append_item(&widgets::menu_item(&label, "player.episode", &episode.number));
        }
        self.ui.episodes.set_menu_model(Some(&list));
        widgets::mark_items(&self.ui.episodes, "player.episode", "amber-check");
        let current = episodes.iter().find(|e| e.current).map(|e| e.number.clone());
        self.episode_action.set_state(&current.unwrap_or_default().to_variant());

        let menu = gio::Menu::new();
        let source_menu = gio::Menu::new();
        for source in &sources {
            source_menu.append_item(&widgets::menu_item(
                &source.label,
                "player.source",
                &source.id,
            ));
        }
        if !sources.is_empty() {
            menu.append_section(Some(&t::show_settings_source()), &source_menu);
        }
        let current = sources.iter().find(|s| s.current).map(|s| s.id.clone());
        self.source_action.set_state(&current.unwrap_or_default().to_variant());
        let picture = gio::Menu::new();
        for choice in &upscaling.options {
            picture.append_item(&widgets::menu_item(
                &choice.label,
                "player.upscaling",
                &choice.id,
            ));
        }
        self.upscaling_action.set_state(&upscaling.current.to_variant());
        menu.append_section(Some(&t::player_upscaling()), &picture);
        let night_menu = gio::Menu::new();
        night_menu.append(Some(&t::player_night_mode()), Some("player.night"));
        self.night_action.set_state(&night.to_variant());
        menu.append_section(None, &night_menu);
        if self.ctx.tv.available() {
            let tv_menu = gio::Menu::new();
            tv_menu.append(Some(&t::tv_play_on_menu()), Some("player.tv"));
            menu.append_section(None, &tv_menu);
        }
        if self.late.get() && self.discussions_on() {
            let talk = gio::Menu::new();
            talk.append(Some(&t::discussion_open()), Some("player.discussion"));
            menu.append_section(None, &talk);
        }
        let stats_menu = gio::Menu::new();
        stats_menu.append(Some(&t::player_stats()), Some("player.stats"));
        menu.append_section(None, &stats_menu);
        self.ui.settings.set_menu_model(Some(&menu));
        widgets::mark_items(&self.ui.settings, "player.source", "amber-check");
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
        add("mini", None, Box::new(|v, _| v.toggle_mini()));
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
        add("party", None, Box::new(|v, _| v.show_party()));
        add("tv", None, Box::new(|v, _| v.play_on_tv()));
        self.ui.tv.set_visible(self.ctx.tv.available());
        // Once the TV has it, the player here closes.
        self.ctx.tv.on_handed_off({
            let weak = Rc::downgrade(self);
            move || {
                if let Some(view) = weak.upgrade()
                    && view.shell().is_some_and(|shell| shell.is_player())
                {
                    view.close();
                }
            }
        });
        self.party.popover.connect_closed({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.wake();
                }
            }
        });
        add(
            "copy-stats",
            None,
            Box::new(|v, _| v.send(PlayerCommand::CopyStats { about: stats::about() })),
        );
        // Discussion: playback pauses where it is, and stays paused when the sheet closes.
        add(
            "discussion",
            None,
            Box::new(|v, _| {
                let (Some(id), number) = (v.title_id.get(), v.episode_line.borrow().0.clone())
                else {
                    return;
                };
                if !v.state.borrow().paused {
                    v.send(PlayerCommand::TogglePause);
                }
                let _ = v.widget.activate_action(
                    "win.open-discussion",
                    Some(&(id.get(), number).to_variant()),
                );
            }),
        );
        add("play-next", None, Box::new(|v, _| v.play_next()));
        add("cancel-next", None, Box::new(|v, _| v.cancel_next()));
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
        // The stateful ones: the menus show their state as radio items and a check, and the
        // state follows what the engine and mpv then say, not the click.
        let stateful = |action: &gio::SimpleAction, f: Handler| {
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, arg| {
                if let Some(view) = weak.upgrade() {
                    f(&view, arg);
                    view.wake();
                }
            });
            group.add_action(action);
        };
        stateful(&self.stats_action, Box::new(|v, _| v.toggle_stats()));
        stateful(
            &self.night_action,
            Box::new(|v, _| {
                let on = v.menus.borrow().as_ref().is_some_and(|m| m.3);
                v.send(PlayerCommand::NightMode(!on));
            }),
        );
        stateful(
            &self.speed_action,
            Box::new(|v, a| {
                if let Some(speed) = a.and_then(f64::from_variant) {
                    v.ask_speed(speed);
                }
            }),
        );
        stateful(
            &self.episode_action,
            Box::new(|v, a| {
                if let Some(n) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Choose(PlayerChoice::Episode(n)));
                }
            }),
        );
        stateful(
            &self.source_action,
            Box::new(|v, a| {
                if let Some(id) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Choose(PlayerChoice::Source(id)));
                }
            }),
        );
        stateful(
            &self.upscaling_action,
            Box::new(|v, a| {
                if let Some(id) = a.and_then(String::from_variant) {
                    v.send(PlayerCommand::Upscaling(id));
                }
            }),
        );
        stateful(
            &self.translation_action,
            Box::new(|v, a| {
                let Some(target) = a.and_then(String::from_variant) else { return };
                let translation =
                    if target == "dub" { Translation::Dub } else { Translation::Sub };
                if v.translation_action.state().and_then(|s| s.get::<String>()).as_deref()
                    != Some(translation.as_str())
                {
                    v.send(PlayerCommand::Choose(PlayerChoice::Translation(translation)));
                }
                v.ui.tracks.popdown();
            }),
        );
        ui.page.insert_action_group("player", Some(&group));
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
            if picked.is_some_and(|w| w.is_ancestor(&view.ui.cards) || w == view.ui.cards) {
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
            { "title": "Episode", "time": 0.0 }, // i18n-skip
            { "title": "Opening", "time": 4.0 }, // i18n-skip
            { "title": "Episode", "time": 16.0 }, // i18n-skip
            { "title": "Ending", "time": 50.0 }, // i18n-skip
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
                player.mpv().command(&["sub-add", subtitle, "select", "Lantern Road", "eng"]); // i18n-skip
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
        self.party.popover.popdown();
    }

    /// Play on TV from the player: paused here, and carried on from here on the TV once it has
    /// started the episode.
    fn play_on_tv(&self) {
        let Some(id) = self.title_id.get() else { return };
        let episode = self.episode_line.borrow().0.clone();
        if episode.is_empty() {
            return;
        }
        let state = self.state.borrow().clone();
        if !state.paused {
            self.send(PlayerCommand::TogglePause);
        }
        if self.mini.get().is_some() {
            self.set_mini(false);
        }
        self.ctx.tv.offer(crate::tv::Request {
            id,
            title: self.ui.show.label().to_string(),
            episode,
            from: state.position.filter(|p| *p > 5.0),
            from_player: true,
        });
    }

    /// The party's panel when in one; the form that starts one when not.
    pub fn show_party(&self) {
        if self.ctx.hub.known.borrow().party.is_none() {
            crate::party::form::open(&self.ctx, crate::party::form::For::Playing);
            return;
        }
        self.wake();
        self.party.popover.popup();
    }

    /// The people button as the party stands: a dot while in one, and its name.
    fn show_party_button(&self) {
        let in_party = self.ctx.hub.known.borrow().party.is_some();
        // Faded rather than added, so nothing moves.
        self.ui.people_dot.set_opacity(if in_party { 1.0 } else { 0.0 });
        let name = t::party_title();
        self.ui.people.set_tooltip_text(Some(&name));
        self.ui.people.update_property(&[gtk::accessible::Property::Label(&name)]);
    }

    /// For a shot: the party's panel, under the people button, over the window.
    pub fn shot_party_popover(&self) -> Option<(gtk::Widget, f32, f32)> {
        let popover = &self.party.popover;
        if !popover.is_visible() {
            return None;
        }
        let window = &self.ctx.window;
        let button = self.ui.people.compute_bounds(window)?;
        let width = popover.width() as f32;
        let x = (button.x() + button.width() / 2.0 - width / 2.0)
            .clamp(0.0, (window.width() as f32 - width).max(0.0));
        Some((popover.clone().upcast(), x, button.y() + button.height()))
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
        // Tab and F6 bring the controls up before focus moves onto them.
        if matches!(key, Key::Tab | Key::ISO_Left_Tab | Key::F6) {
            self.wake();
            return false;
        }
        // The keys of a text field, an open party form or a focused button are its own.
        if self.party.popover.is_visible() {
            return false;
        }
        if let Some(focus) = GtkWindowExt::focus(&self.ctx.window) {
            let button_key = matches!(key, Key::space | Key::Return | Key::KP_Enter);
            let mut at = Some(focus);
            while let Some(widget) = at {
                if widget.is::<gtk::Editable>()
                    || (button_key
                        && (widget.is::<gtk::Button>() || widget.is::<gtk::MenuButton>()))
                {
                    return false;
                }
                at = widget.parent();
            }
        }
        // With a menu open its keys are its own: arrows move through it, Space and Enter pick,
        // and Escape closes it before anything else. T and E close the menu they opened.
        if let Some(open) = self.ui.menu_buttons.iter().find(|b| b.is_active()) {
            let toggles = match key {
                Key::t | Key::T => open == &self.ui.tracks,
                Key::e | Key::E => open == &self.ui.episodes,
                _ => false,
            };
            if toggles {
                open.popdown();
                return true;
            }
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
            Key::p | Key::P => self.toggle_mini(),
            Key::s | Key::S if self.ui.skip.shown() => self.send(PlayerCommand::TakeSkip),
            Key::n => self.play_next(),
            Key::N => self.send(PlayerCommand::Step(-1)),
            Key::bracketleft => self.step_speed(false),
            Key::bracketright => self.step_speed(true),
            Key::BackSpace => self.ask_speed(1.0),
            Key::e | Key::E => self.ui.episodes.popup(),
            Key::t | Key::T => self.ui.tracks.popup(),
            Key::i | Key::I => self.toggle_stats(),
            Key::u | Key::U => {
                if !self.undo_skip() {
                    return false;
                }
            }
            Key::Return | Key::KP_Enter => {
                if self.ui.skip.shown() {
                    self.send(PlayerCommand::TakeSkip);
                } else if self.ui.up_next.shown() {
                    self.play_next();
                } else if self.ui.stall.shown() {
                    self.send(PlayerCommand::StallSwitch);
                } else {
                    return false;
                }
            }
            Key::Escape => {
                if self.ui.up_next.shown() && self.mini.get().is_none() {
                    self.cancel_next();
                } else if self.ui.stall.shown() && self.mini.get().is_none() {
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

impl PlayerView {
    /// The player as a game controller's map reads it: whether the controls show, and the card
    /// over the video that takes A and B.
    pub fn pad_context(&self) -> PadContext {
        let card = if self.ui.up_next.shown() {
            Some(PlayerCard::UpNext)
        } else if self.ui.stall.shown() {
            Some(PlayerCard::Stall)
        } else if self.ui.skip.shown()
            && self.skip_since.get().is_some_and(|at| at.elapsed() < Duration::from_secs(8))
        {
            Some(PlayerCard::SkipFresh)
        } else {
            None
        };
        let menu_open = self.ui.menu_buttons.iter().any(|b| b.is_active());
        PadContext::Player { controls_shown: self.shown.get() || menu_open, card }
    }

    /// A controller moved along the controls: they stay up a while longer.
    pub fn pad_touch(&self) {
        if self.shown.get() {
            self.wake();
        }
    }

    /// B: the controls away, or the player closed when they are already away.
    pub fn pad_back(&self) {
        if self.shown.get() {
            self.hide_controls();
        } else {
            self.key(gdk::Key::Escape, gdk::ModifierType::empty());
        }
    }

    pub fn pad_command(&self, command: PlayerCommand) {
        self.send(command);
    }

    pub fn pad_panel(&self, panel: PlayerPanel) {
        match panel {
            PlayerPanel::Tracks => self.ui.tracks.popup(),
            PlayerPanel::Episodes => self.ui.episodes.popup(),
            PlayerPanel::More => self.ui.settings.popup(),
            PlayerPanel::Stats => self.toggle_stats(),
        }
        self.wake();
    }

    pub fn pad_reveal(&self, target: PlayerFocus) {
        self.wake();
        match target {
            PlayerFocus::SeekLine => self.ui.timeline.grab_focus(),
            PlayerFocus::Episodes => self.ui.episodes.grab_focus(),
        };
    }

    pub fn pad_play_pause(&self) {
        self.send(PlayerCommand::TogglePause);
        self.wake();
    }

    /// The right trigger held: twice the speed. What it was is handed back to restore.
    pub fn pad_hold_speed(&self) -> f64 {
        let was = self.speed.get();
        self.send(PlayerCommand::SpeedTo(2.0));
        self.wake();
        was
    }

    pub fn pad_restore_speed(&self, speed: f64) {
        self.send(PlayerCommand::SpeedTo(speed));
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

/// "Episode 7 · Dub · The Lantern Road": a dub marked on the episode line, a sub not, and the
/// name when there is one.
fn episode_line(number: &str, translation: Option<Translation>, title: Option<&str>) -> String {
    let line = say::marked(
        t::episode_number(crate::party::number(number)),
        translation == Some(Translation::Dub),
    );
    match title {
        Some(title) if !title.is_empty() => [line.as_str(), title].join(" · "),
        _ => line,
    }
}

/// A speed as its button and menu say it: "1×", "1.25×".
fn speed_label(speed: f64) -> String {
    let text = format!("{speed:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}×")
}

/// A radio action on string targets, standing on `state`.
fn radio(name: &str, state: &str) -> gio::SimpleAction {
    gio::SimpleAction::new_stateful(name, Some(glib::VariantTy::STRING), &state.to_variant())
}

/// A corner card over the video, faded in and out with the fade tokens.
struct Card {
    widget: gtk::Box,
    fade: crate::motion::Fader,
}

impl Card {
    fn new() -> Self {
        Self { widget: card(), fade: crate::motion::Fader::default() }
    }

    fn shown(&self) -> bool {
        self.fade.shown()
    }

    fn set(&self, show: bool) {
        self.fade.set(&self.widget, show);
    }

    fn hide_now(&self) {
        self.fade.hide_now(&self.widget);
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
    card.set_visible(false);
    card
}

fn build_ui(video: &gtk::Widget) -> Ui {
    // The top bar: back, the show and the episode, and at the end episodes, settings and
    // fullscreen, with the window's own close button.
    let back = icon_button("go-previous-symbolic", &t::action_back(), "player.back");
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
        .tooltip_text(t::player_episodes())
        .valign(gtk::Align::Center)
        .build();
    episodes.add_css_class("flat");
    let tracks = gtk::MenuButton::builder()
        .icon_name("media-view-subtitles-symbolic")
        .tooltip_text(t::player_subtitles_and_audio())
        .valign(gtk::Align::Center)
        .build();
    tracks.add_css_class("flat");
    let people_dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    people_dot.add_css_class("party-dot");
    people_dot.set_halign(gtk::Align::End);
    people_dot.set_valign(gtk::Align::Start);
    people_dot.set_opacity(0.0);
    people_dot.set_can_target(false);
    let people_icon = gtk::Overlay::new();
    people_icon.set_child(Some(&gtk::Image::from_icon_name("anistream-party-symbolic")));
    people_icon.add_overlay(&people_dot);
    let people = gtk::Button::builder()
        .child(&people_icon)
        .tooltip_text(t::party_title())
        .action_name("player.party")
        .valign(gtk::Align::Center)
        .build();
    people.add_css_class("flat");
    people.add_css_class("image-button");
    let tv = icon_button("video-display-symbolic", &t::tv_play_on(), "player.tv");
    let full =
        icon_button("view-fullscreen-symbolic", &t::player_fullscreen(), "player.fullscreen");
    let mini_button =
        icon_button("anistream-mini-player-symbolic", &t::player_mini_player(), "player.mini");
    let top = adw::HeaderBar::new();
    top.add_css_class("osd");
    top.add_css_class("player-top");
    top.pack_start(&back);
    top.set_title_widget(Some(&titles));
    top.pack_end(&full);
    top.pack_end(&mini_button);
    top.pack_end(&episodes);
    top.pack_end(&tracks);
    top.pack_end(&people);
    top.pack_end(&tv);

    // The bottom bar.
    let play =
        icon_button("media-playback-pause-symbolic", &t::action_pause(), "player.toggle");
    play.add_css_class("play");
    let back10 =
        icon_button("media-seek-backward-symbolic", &t::player_back_10(), "player.back-10");
    let forward10 = icon_button(
        "media-seek-forward-symbolic",
        &t::player_forward_10(),
        "player.forward-10",
    );
    let position = gtk::Label::new(Some("0:00"));
    let remaining = gtk::Label::new(Some("\u{2212}0:00"));
    for label in [&position, &remaining] {
        label.add_css_class("numeric");
        label.add_css_class("caption");
    }
    let timeline = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    timeline.set_hexpand(true);
    timeline.set_draw_value(false);
    timeline.add_css_class("timeline");
    timeline.update_property(&[gtk::accessible::Property::Label(&t::player_position())]);
    timeline.set_restrict_to_fill_level(false);
    let marked = Marked::new(&timeline);

    let volume = gtk::Scale::with_range(gtk::Orientation::Vertical, 0.0, 100.0, 5.0);
    volume.set_inverted(true);
    volume.set_value(100.0);
    volume.set_size_request(-1, 140);
    volume.set_draw_value(false);
    volume.update_property(&[gtk::accessible::Property::Label(&t::player_volume())]);
    volume.update_property(&[gtk::accessible::Property::ValueText(&t::player_volume_percent(
        100,
    ))]);
    volume.connect_value_changed(|scale| {
        scale.update_property(&[gtk::accessible::Property::ValueText(
            &t::player_volume_percent(scale.value().round() as i64),
        )]);
    });
    let volume_popover = gtk::Popover::builder().child(&volume).build();
    let volume_button = gtk::MenuButton::builder()
        .icon_name("audio-volume-high-symbolic")
        .tooltip_text(t::player_volume())
        .popover(&volume_popover)
        .valign(gtk::Align::Center)
        .build();
    volume_button.add_css_class("flat");
    let speeds = gio::Menu::new();
    for speed in SPEEDS {
        let item = gio::MenuItem::new(Some(&speed_label(speed)), None);
        item.set_action_and_target_value(Some("player.speed"), Some(&speed.to_variant()));
        speeds.append_item(&item);
    }
    // The speed as the button's own label, "1×", as mpv and the Mac show it.
    let speed_button = gtk::MenuButton::builder()
        .label(speed_label(1.0))
        .tooltip_text(t::player_speed_is(&speed_label(1.0)))
        .menu_model(&speeds)
        .valign(gtk::Align::Center)
        .always_show_arrow(false)
        .build();
    speed_button.add_css_class("flat");
    speed_button.add_css_class("numeric");
    speed_button.add_css_class("speed");
    let settings = gtk::MenuButton::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text(t::player_playback_settings())
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
    let starting_art =
        gtk::Picture::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
    starting_art.set_content_fit(gtk::ContentFit::Cover);
    starting_art.set_can_shrink(true);
    starting_art.add_css_class("art-fade");
    let spinner = adw::Spinner::new();
    spinner.set_size_request(32, 32);
    let starting_line = gtk::Label::new(Some(&t::player_starting_plain()));
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
    let skip = Card::new();
    let skip_button = gtk::Button::with_label(&t::player_skip_opening());
    skip_button.add_css_class("pill");
    skip_button.set_action_name(Some("player.skip"));
    skip.widget.append(&skip_button);

    // "Skipped opening · Undo", a small card in the same corner as Skip.
    let undo = Card::new();
    undo.widget.set_orientation(gtk::Orientation::Horizontal);
    undo.widget.set_spacing(6);
    undo.widget.add_css_class("undo-card");
    let undo_line = gtk::Label::new(Some(&t::player_skipped_opening()));
    let undo_dot = widgets::haze("·", "");
    let undo_button = gtk::Button::with_label(&t::action_undo());
    undo_button.add_css_class("flat");
    undo_button.add_css_class("undo-button");
    undo_button.set_action_name(Some("player.undo-skip"));
    undo_button.set_tooltip_text(Some(&t::player_undo_tooltip()));
    undo.widget.append(&undo_line);
    undo.widget.append(&undo_dot);
    undo.widget.append(&undo_button);

    // Up next as Windows draws it: the episode's still, "Up next", the episode and "Plays in
    // 8 s", then the countdown, and Cancel with an amber Play now.
    let up_next = Card::new();
    up_next.widget.add_css_class("up-next-card");
    let up_next_caption = widgets::haze(&t::player_up_next(), "caption");
    up_next_caption.set_xalign(0.0);
    let up_next_title = gtk::Label::new(None);
    up_next_title.set_xalign(0.0);
    up_next_title.set_wrap(true);
    up_next_title.set_lines(3);
    up_next_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    up_next_title.set_max_width_chars(24);
    up_next_title.add_css_class("heading");
    let up_next_line = widgets::haze("", "caption");
    up_next_line.set_xalign(0.0);
    up_next_line.set_margin_top(4);
    up_next_line.set_visible(false);
    up_next_line.add_css_class("numeric");
    let up_next_still =
        gtk::Picture::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
    up_next_still.set_content_fit(gtk::ContentFit::Cover);
    up_next_still.set_can_shrink(true);
    let still_plate = gtk::Box::new(gtk::Orientation::Vertical, 0);
    still_plate.add_css_class("up-next-still");
    still_plate.set_overflow(gtk::Overflow::Hidden);
    still_plate.append(&up_next_still);
    let up_next_still_frame = crate::fit::Fit::new(&still_plate, 16.0 / 9.0, 160, true, 0);
    up_next_still_frame.set_valign(gtk::Align::Start);
    up_next_still_frame.set_visible(false);
    let up_next_text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    up_next_text.set_valign(gtk::Align::Center);
    up_next_text.set_hexpand(true);
    up_next_text.append(&up_next_caption);
    up_next_text.append(&up_next_title);
    up_next_text.append(&up_next_line);
    let up_next_top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    up_next_top.append(&up_next_still_frame);
    up_next_top.append(&up_next_text);
    // The card is announced once; the bar only draws the countdown.
    let up_next_bar =
        gtk::ProgressBar::builder().accessible_role(gtk::AccessibleRole::Presentation).build();
    up_next_bar.add_css_class("countdown");
    let up_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    up_buttons.set_halign(gtk::Align::End);
    let play_now = gtk::Button::with_label(&t::action_play_now());
    play_now.add_css_class("suggested-action");
    play_now.set_action_name(Some("player.play-next"));
    let cancel = gtk::Button::with_label(&t::action_cancel());
    cancel.set_action_name(Some("player.cancel-next"));
    up_buttons.append(&cancel);
    up_buttons.append(&play_now);
    up_next.widget.set_size_request(380, -1);
    up_next.widget.append(&up_next_top);
    up_next.widget.append(&up_next_bar);
    up_next.widget.append(&up_buttons);

    // "Source is buffering · 9 s", with Keep Waiting and Switch Source, the suggested one last.
    let stall = Card::new();
    stall.widget.set_size_request(320, -1);
    let stall_line = gtk::Label::new(Some(&t::player_stall_title(0)));
    stall_line.add_css_class("heading");
    stall_line.add_css_class("numeric");
    stall_line.set_xalign(0.0);
    let stall_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    stall_buttons.set_halign(gtk::Align::End);
    let switch = gtk::Button::with_label(&t::player_switch_source());
    switch.add_css_class("pill");
    switch.add_css_class("suggested-action");
    switch.set_action_name(Some("player.stall-switch"));
    let wait = gtk::Button::with_label(&t::player_keep_waiting());
    wait.add_css_class("pill");
    wait.set_action_name(Some("player.stall-wait"));
    stall_buttons.append(&wait);
    stall_buttons.append(&switch);
    stall.widget.append(&stall_line);
    stall.widget.append(&stall_buttons);

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
    let start_over = gtk::Button::with_label(&t::player_start_over());
    start_over.add_css_class("pill");
    start_over.set_action_name(Some("player.start-over"));
    resumed.append(&resumed_line);
    resumed.append(&start_over);

    let cards = gtk::Box::new(gtk::Orientation::Vertical, 8);
    cards.set_halign(gtk::Align::End);
    cards.set_valign(gtk::Align::End);
    cards.set_margin_end(24);
    cards.set_margin_bottom(96);
    cards.append(&stall.widget);
    cards.append(&undo.widget);
    cards.append(&up_next.widget);
    cards.append(&skip.widget);

    let dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
    dim.add_css_class("player-dim");
    dim.set_can_target(false);

    // The mini player's controls: play and pause, back and forward 10 s in the middle, and the
    // way back to the full window in the corner.
    let mini = gtk::Box::new(gtk::Orientation::Vertical, 0);
    mini.set_visible(false);
    let full_window =
        icon_button("anistream-mini-player-symbolic", &t::player_mini_back(), "player.mini");
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
    let mini_play =
        icon_button("media-playback-pause-symbolic", &t::action_pause(), "player.toggle");
    for button in [
        icon_button("media-seek-backward-symbolic", &t::player_back_10(), "player.back-10"),
        mini_play.clone(),
        icon_button(
            "media-seek-forward-symbolic",
            &t::player_forward_10(),
            "player.forward-10",
        ),
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
        // The mini player's button too, so a narrow window's title keeps room beside the people
        // button.
        roomy: vec![
            back10.upcast(),
            forward10.upcast(),
            speed_button.clone().upcast(),
            mini_button.clone().upcast(),
            tv.clone().upcast(),
        ],
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
        undo,
        undo_line,
        up_next,
        up_next_title,
        up_next_line,
        up_next_bar,
        up_next_still,
        up_next_still_frame,
        stall,
        stall_line,
        resumed,
        resumed_line,
        episodes,
        tracks,
        people,
        people_dot,
        tv,
        settings,
        speed: speed_button,
        menu_buttons,
        mini,
        mini_play,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_dub_is_marked_on_the_episode_line() {
        assert_eq!(
            episode_line("10", Some(Translation::Dub), Some("The Lantern Road")),
            "Episode 10 · Dubbed · The Lantern Road"
        );
        assert_eq!(episode_line("10", Some(Translation::Sub), None), "Episode 10");
        assert_eq!(episode_line("3", None, Some("")), "Episode 3");
    }

    #[test]
    fn a_mini_toggle_waits_for_the_last_one_to_land() {
        assert!(mini_ready(None));
        assert!(!mini_ready(Some(Duration::from_millis(5))));
        assert!(mini_ready(Some(Duration::from_secs(1))));
    }

    #[test]
    fn a_quick_mini_toggle_keeps_the_frame_from_before() {
        let big = Restore { width: 1280, height: 800, maximized: false, fullscreen: false };
        let mid = Restore { width: 480, height: 360, maximized: false, fullscreen: false };
        assert_eq!(frame_to_keep(mid, Some(big)).width, 1280);
        // A first entry, or a window the viewer resized, is its own frame.
        assert_eq!(frame_to_keep(mid, None).width, 480);
        let other = Restore { width: 900, height: 600, ..big };
        assert_eq!(frame_to_keep(other, Some(big)).width, 900);
    }

    #[test]
    fn a_step_starts_from_the_speed_asked_for_not_the_one_mpv_last_reported() {
        let mut asked = None;
        for want in [1.25, 1.5, 2.0] {
            let next = next_speed(step_from(asked, 1.0), true).unwrap();
            assert_eq!(next, want);
            asked = Some((next, Duration::from_millis(80)));
        }
        let down = next_speed(step_from(asked, 1.0), false).unwrap();
        assert_eq!(down, 1.5);
        // Once mpv has had time to answer, its speed stands again.
        assert_eq!(step_from(Some((2.0, Duration::from_secs(3))), 1.0), 1.0);
    }

    #[test]
    fn quick_steps_walk_the_speeds_one_after_the_other() {
        // Four presses of `]` before mpv reports anything: each steps from the last ask.
        let mut at = 1.0;
        for want in [1.25, 1.5, 2.0] {
            at = next_speed(at, true).unwrap();
            assert_eq!(at, want);
        }
        assert_eq!(next_speed(at, true), None);
        // `]` then `[` comes back to where it began.
        assert_eq!(next_speed(next_speed(1.0, true).unwrap(), false), Some(1.0));
        assert_eq!(next_speed(0.5, false), None);
    }

    #[test]
    fn speeds_read_without_trailing_zeros() {
        assert_eq!(speed_label(1.0), "1×");
        assert_eq!(speed_label(1.25), "1.25×");
        assert_eq!(speed_label(0.5), "0.5×");
    }
}
