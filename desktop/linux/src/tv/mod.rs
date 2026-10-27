//! Play on TV and the TV's remote, as the Mac has them: the engine's sender finds TVs, pairs with
//! one and keeps a link to it; this only holds what the sheet, Settings and the entries to it
//! draw, on GTK's main loop. The protocol is the engine's: the app's one part of the network is
//! asking Avahi, the system's own mDNS, which TVs it has seen (`avahi`).
//!
//! - [`Tv::offer`]: Play on TV from a title, an episode or the player, straight to the TV driven
//!   already, or the picker to choose one;
//! - the picker's rows, a pairing under way, and the link's card while a TV is driven;
//! - the paired TVs, for Settings.

mod avahi;
#[cfg(feature = "fixtures")]
pub mod fixture;
pub mod paired;
pub mod sheet;

use std::{
    cell::{Cell, RefCell},
    future::Future,
    path::Path,
    rc::{Rc, Weak},
    sync::Arc,
    time::Instant,
};

use anistream_core::ids::AnilistId;
use anistream_engine::remote::{
    protocol::{
        client::Hello,
        command::RemoteCommand,
        discovery::Browser,
        identity::{Fingerprint, Identity},
        known::{KnownTv, KnownTvs},
    },
    sender::{
        LinkPhase, NowCard, Pairing, Problem, RemoteCard, TvLink, TvRow, TvRowKind, TvSender,
    },
};
use anistream_i18n::t;
use gtk::{gio, glib, prelude::*};
use tokio::runtime::Handle;

use crate::say;

/// What a TV lists this computer as: the kind of device, never the name it was given, which is
/// often its owner's.
const DEVICE: &str = "Linux computer"; // i18n-skip

/// The file this install's key is kept in, under the data directory's `remote`.
const IDENTITY_FILE: &str = "sender-identity";

/// What was asked to play on a TV.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    /// Where this computer was in the episode, carried over once the TV has started it.
    pub from: Option<f64>,
    /// Handed over from the player, which closes once the TV has it.
    pub from_player: bool,
}

/// A pairing under way: the TV, whether a code is to be typed, and how the last try went.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Step {
    /// Empty until the TV has answered.
    pub tv_name: String,
    pub needs_code: bool,
    /// A try is out, or the TV is being asked for its code.
    pub working: bool,
    /// What the last try that failed ran into.
    pub problem: Option<Problem>,
    /// The code on the TV ran out: none is typed until the TV shows a new one.
    pub expired: bool,
}

/// Which of the sheet's faces is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sheet {
    Picker,
    Remote,
}

type Listener = Rc<dyn Fn()>;

pub struct Tv {
    me: Weak<Self>,
    handle: Handle,
    sender: Option<Arc<TvSender>>,
    browser: Arc<dyn Browser>,
    rows: RefCell<Vec<TvRow>>,
    paired: RefCell<Vec<KnownTv>>,
    short: String,
    browsers: Cell<u32>,
    request: RefCell<Option<Request>>,
    pairing: RefCell<Option<Step>>,
    steps: RefCell<Option<Arc<tokio::sync::Mutex<Pairing>>>>,
    /// Counts up as a pairing starts or is let go, so an answer to one let go is not taken.
    pairing_turn: Cell<u64>,
    link: RefCell<Option<Arc<TvLink>>>,
    connected: RefCell<Option<Fingerprint>>,
    /// Counts up with each link, so a card from one let go is not taken.
    link_turn: Cell<u64>,
    card: RefCell<Option<RemoteCard>>,
    /// When the card's playhead was last said, and when the link went, for the seek line.
    card_at: Cell<Instant>,
    lost_at: Cell<Option<Instant>>,
    /// The session ended out of sight, the TV away or forgetting this computer: the link is let
    /// go, and the card stays for the sidebar to say so until the remote is opened again.
    ended: Cell<bool>,
    /// What was asked of the TV, until its card shows it.
    starting: RefCell<Option<Request>>,
    problem: RefCell<Option<Problem>>,
    sheet: Cell<Option<Sheet>>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next_listener: Cell<u64>,
    /// Run once a hand-over from the player has reached the TV: the player closes.
    handed_off: RefCell<Option<Box<dyn Fn()>>>,
}

impl Tv {
    /// The sender over the paired TVs under `data`, browsing with Avahi. With no sender (its key
    /// or its list could not be read) Play on TV is not offered.
    pub fn new(handle: Handle, data: &Path) -> Rc<Self> {
        let sender = open_sender(&handle, data)
            .inspect_err(|e| tracing::warn!(error = %e, "no Play on TV"))
            .ok()
            .map(Arc::new);
        let tv = Self::with(handle, sender, avahi::Avahi::start());
        tv.watch_network();
        tv
    }

    fn with(
        handle: Handle,
        sender: Option<Arc<TvSender>>,
        browser: Arc<dyn Browser>,
    ) -> Rc<Self> {
        let short = sender.as_ref().map(|s| s.short_fingerprint()).unwrap_or_default();
        let tv = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            handle,
            sender,
            browser,
            rows: RefCell::default(),
            paired: RefCell::default(),
            short,
            browsers: Cell::new(0),
            request: RefCell::default(),
            pairing: RefCell::default(),
            steps: RefCell::default(),
            pairing_turn: Cell::new(0),
            link: RefCell::default(),
            connected: RefCell::default(),
            link_turn: Cell::new(0),
            card: RefCell::default(),
            card_at: Cell::new(Instant::now()),
            lost_at: Cell::new(None),
            ended: Cell::new(false),
            starting: RefCell::default(),
            problem: RefCell::default(),
            sheet: Cell::new(None),
            listeners: RefCell::default(),
            next_listener: Cell::new(0),
            handed_off: RefCell::default(),
        });
        tv.refresh();
        tv.follow_changes();
        tv
    }

    /// Whether Play on TV is there: the sender started.
    pub fn available(&self) -> bool {
        self.sender.is_some()
    }

    /// Run `f` whenever what this holds changes. Forget it with [`Self::forget`].
    pub fn listen(&self, f: impl Fn() + 'static) -> u64 {
        let id = self.next_listener.get() + 1;
        self.next_listener.set(id);
        self.listeners.borrow_mut().push((id, Rc::new(f)));
        id
    }

    pub fn forget(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(i, _)| *i != id);
    }

    /// What closes the player once a hand-over reached the TV.
    pub fn on_handed_off(&self, f: impl Fn() + 'static) {
        self.handed_off.replace(Some(Box::new(f)));
    }

    fn changed(&self) {
        let listeners: Vec<Listener> =
            self.listeners.borrow().iter().map(|(_, f)| Rc::clone(f)).collect();
        for listener in listeners {
            listener();
        }
    }

    /// Read the rows and the paired TVs again.
    fn refresh(&self) {
        let Some(sender) = &self.sender else { return };
        self.rows.replace(sender.rows());
        self.paired.replace(sender.paired());
        self.changed();
    }

    /// The sender says when its rows or paired TVs may read differently.
    fn follow_changes(&self) {
        let Some(sender) = &self.sender else { return };
        let mut changes = sender.changes();
        let (tx, rx) = async_channel::unbounded::<()>();
        self.handle.spawn(async move {
            while changes.changed().await.is_ok() {
                if tx.send(()).await.is_err() {
                    return;
                }
            }
        });
        let me = self.me.clone();
        glib::spawn_future_local(async move {
            while rx.recv().await.is_ok() {
                // A burst of changes, read once.
                while rx.try_recv().is_ok() {}
                let Some(tv) = me.upgrade() else { return };
                tv.refresh();
            }
        });
    }

    /// Run `work` on the engine's runtime, and `then` with what it gave back on the main loop.
    fn run<T: Send + 'static>(
        &self,
        work: impl Future<Output = T> + Send + 'static,
        then: impl FnOnce(&Self, T) + 'static,
    ) {
        let task = self.handle.spawn(work);
        let me = self.me.clone();
        glib::spawn_future_local(async move {
            let Ok(out) = task.await else { return };
            if let Some(tv) = me.upgrade() {
                then(&tv, out);
            }
        });
    }

    // What the sheet and Settings read.

    pub fn rows(&self) -> Vec<TvRow> {
        self.rows.borrow().clone()
    }

    pub fn paired(&self) -> Vec<KnownTv> {
        self.paired.borrow().clone()
    }

    /// This computer's key, as a TV's settings list it.
    pub fn short_fingerprint(&self) -> &str {
        &self.short
    }

    pub fn sheet(&self) -> Option<Sheet> {
        self.sheet.get()
    }

    pub fn request(&self) -> Option<Request> {
        self.request.borrow().clone()
    }

    pub fn pairing(&self) -> Option<Step> {
        self.pairing.borrow().clone()
    }

    pub fn connected(&self) -> Option<Fingerprint> {
        *self.connected.borrow()
    }

    pub fn card(&self) -> Option<RemoteCard> {
        self.card.borrow().clone()
    }

    pub fn problem(&self) -> Option<Problem> {
        *self.problem.borrow()
    }

    /// The link is up, and the card's controls reach the TV.
    pub fn live(&self) -> bool {
        self.card.borrow().as_ref().is_some_and(|c| c.phase == LinkPhase::Connected)
    }

    /// The session ended out of sight, its card kept: see [`Self::ended`]'s field.
    pub fn ended(&self) -> bool {
        self.ended.get()
    }

    /// The TV let this computer go: pair again.
    pub fn lost(&self) -> bool {
        self.card
            .borrow()
            .as_ref()
            .is_some_and(|c| matches!(c.phase, LinkPhase::Unpaired | LinkPhase::KeyChanged))
    }

    /// What the TV plays, as the card shows it: what was asked of it while it starts, else what
    /// the TV last said.
    pub fn now(&self) -> Option<NowCard> {
        let card = self.card.borrow();
        let Some(request) = self.starting.borrow().clone() else {
            return card.as_ref().and_then(|c| c.now.clone());
        };
        Some(NowCard {
            id: request.id,
            title: request.title,
            episode_line: t::tv_episode(&request.episode),
            episode: request.episode,
            loading: true,
            position: request.from.unwrap_or(0.0),
            duration: None,
            paused: false,
            speed: 1.0,
            volume: card.as_ref().and_then(|c| c.now.as_ref()).and_then(|n| n.volume),
            audio: Vec::new(),
            subtitles: Vec::new(),
            subtitles_off: false,
            skip: None,
        })
    }

    /// Where the TV's playhead is now: moved on from the card on this computer's clock while it
    /// plays, and held where the link was lost.
    pub fn playhead(&self) -> Option<f64> {
        if self.starting.borrow().is_some() {
            return None;
        }
        let card = self.card.borrow();
        let now = card.as_ref()?.now.as_ref()?;
        let until = self.lost_at.get().map_or_else(Instant::now, |at| at.min(Instant::now()));
        let elapsed = until.saturating_duration_since(self.card_at.get()).as_secs_f64();
        Some(now.position_after(elapsed))
    }

    // Play on TV.

    /// Play on TV, from a title, an episode or the player: straight to the TV driven already,
    /// otherwise the picker to choose one.
    pub fn offer(&self, request: Request) {
        if !self.available() {
            return;
        }
        self.problem.replace(None);
        self.request.replace(Some(request));
        match self.connected() {
            Some(tv) if self.live() => self.play_on(tv),
            _ => self.sheet.set(Some(Sheet::Picker)),
        }
        self.changed();
    }

    /// The picker, to pair or connect, with nothing to play.
    pub fn open_picker(&self) {
        if !self.available() {
            return;
        }
        self.request.replace(None);
        self.problem.replace(None);
        self.sheet.set(Some(Sheet::Picker));
        self.changed();
    }

    /// The card of the TV driven, or the picker when there is none.
    pub fn open_remote(&self) {
        if self.ended.get()
            && let Some(tv) = self.connected()
        {
            self.connect(tv);
        }
        self.sheet.set(Some(if self.connected().is_some() {
            Sheet::Remote
        } else {
            Sheet::Picker
        }));
        self.changed();
    }

    /// The sheet closed. A pairing under way is let go; the link stays, for the card to come
    /// back to, unless the TV already ended the session.
    pub fn sheet_closed(&self) {
        if self.sheet.get().is_none() {
            return;
        }
        self.sheet.set(None);
        self.request.replace(None);
        self.drop_pairing();
        if self.card.borrow().as_ref().is_some_and(|c| c.over) {
            self.end();
        }
        self.changed();
    }

    /// The session is over. Played out, it simply ends; lost or forgotten, the link goes and
    /// the card stays, so the sidebar can say what became of the TV.
    fn end(&self) {
        if self.ended.get() {
            return;
        }
        if self.live() {
            self.disconnect();
            return;
        }
        self.link_turn.set(self.link_turn.get() + 1);
        let link = self.link.take();
        {
            let _entered = self.handle.enter();
            drop(link);
        }
        self.starting.replace(None);
        self.ended.set(true);
    }

    /// Look for TVs while a picker is up. Counted, as Settings and the sheet can both be up.
    pub fn start_browsing(&self) {
        let Some(sender) = &self.sender else { return };
        self.browsers.set(self.browsers.get() + 1);
        if self.browsers.get() == 1 {
            let _entered = self.handle.enter();
            sender.browse(Arc::clone(&self.browser));
        }
    }

    pub fn stop_browsing(&self) {
        let Some(sender) = &self.sender else { return };
        if self.browsers.get() == 0 {
            return;
        }
        self.browsers.set(self.browsers.get() - 1);
        if self.browsers.get() == 0 {
            let _entered = self.handle.enter();
            sender.stop_browsing();
        }
    }

    /// A row of the picker chosen: a paired TV is connected to and plays what was asked; a new
    /// one starts pairing.
    pub fn choose(&self, row: &TvRow) {
        match row.kind {
            TvRowKind::Paired | TvRowKind::PairedAway => {
                if let Some(tv) = row.fingerprint {
                    self.use_tv(tv);
                }
            }
            TvRowKind::New => self.begin(&row.key),
            TvRowKind::NeedsUpdate => {}
        }
    }

    // Pairing.

    /// Pair from `target`: a row's key, an `anistream-remote://pair` link or an address typed
    /// in. A TV asked for a code shows one; a link that carried the code pairs at once.
    pub fn begin(&self, target: &str) {
        let Some(sender) = self.sender.clone() else { return };
        self.drop_pairing();
        let turn = self.pairing_turn.get();
        self.sheet.set(Some(Sheet::Picker));
        self.pairing.replace(Some(Step { needs_code: true, working: true, ..Step::default() }));
        self.changed();
        let target = target.trim().to_owned();
        self.run(
            async move {
                let steps = sender.pair(&target).await?;
                let name = steps.tv_name().to_owned();
                let needs = steps.needs_code();
                Ok::<_, Problem>((steps, name, needs))
            },
            move |tv, answer| {
                if tv.pairing_turn.get() != turn || tv.pairing.borrow().is_none() {
                    return;
                }
                match answer {
                    Ok((steps, tv_name, needs_code)) => {
                        let steps = Arc::new(tokio::sync::Mutex::new(steps));
                        tv.steps.replace(Some(Arc::clone(&steps)));
                        tv.pairing.replace(Some(Step {
                            tv_name,
                            needs_code,
                            working: true,
                            problem: None,
                            expired: false,
                        }));
                        tv.changed();
                        if needs_code {
                            tv.ask_code(steps);
                        } else {
                            tv.finish(steps, None);
                        }
                    }
                    Err(problem) => tv.fail(problem),
                }
            },
        );
    }

    fn ask_code(&self, steps: Arc<tokio::sync::Mutex<Pairing>>) {
        let turn = self.pairing_turn.get();
        self.run(async move { steps.lock().await.request_code().await }, move |tv, answer| {
            if tv.pairing_turn.get() != turn {
                return;
            }
            match answer {
                Ok(()) => {
                    if let Some(step) = tv.pairing.borrow_mut().as_mut() {
                        step.working = false;
                    }
                    tv.changed();
                }
                Err(problem) => tv.fail(problem),
            }
        });
    }

    /// The six digits typed.
    pub fn submit(&self, code: &str) {
        let Some(steps) = self.steps.borrow().clone() else { return };
        {
            let mut pairing = self.pairing.borrow_mut();
            let Some(step) = pairing.as_mut().filter(|s| !s.working && !s.expired) else {
                return;
            };
            step.working = true;
            step.problem = None;
        }
        self.changed();
        self.finish(steps, Some(code.to_owned()));
    }

    /// Ask the TV for a code again, as after one ran out.
    pub fn request_code_again(&self) {
        let Some(steps) = self.steps.borrow().clone() else { return };
        if let Some(step) = self.pairing.borrow_mut().as_mut() {
            step.working = true;
            step.problem = None;
            step.expired = false;
        }
        self.changed();
        self.ask_code(steps);
    }

    /// Back from pairing to the picker.
    pub fn cancel_pairing(&self) {
        self.drop_pairing();
        self.changed();
    }

    fn drop_pairing(&self) {
        self.pairing_turn.set(self.pairing_turn.get() + 1);
        self.pairing.replace(None);
        let steps = self.steps.take();
        let _entered = self.handle.enter();
        drop(steps);
    }

    fn finish(&self, steps: Arc<tokio::sync::Mutex<Pairing>>, code: Option<String>) {
        let turn = self.pairing_turn.get();
        self.run(async move { steps.lock().await.pair(code).await }, move |tv, answer| {
            if tv.pairing_turn.get() != turn {
                return;
            }
            match answer {
                Ok(known) => {
                    tv.drop_pairing();
                    tv.refresh();
                    tv.use_tv(known.fingerprint);
                }
                Err(problem) => tv.fail(problem),
            }
        });
    }

    fn fail(&self, problem: Problem) {
        {
            let mut pairing = self.pairing.borrow_mut();
            let Some(step) = pairing.as_mut() else { return };
            step.working = false;
            step.problem = Some(problem);
            step.expired = problem == Problem::NoCode;
        }
        self.changed();
    }

    // The link.

    /// Connect to a paired TV, then play what was asked on it, or show its card.
    fn use_tv(&self, tv: Fingerprint) {
        self.connect(tv);
        if self.request.borrow().is_some() {
            self.play_on(tv);
        } else {
            self.sheet.set(Some(Sheet::Remote));
            self.changed();
        }
    }

    /// Hold a link to the TV, reading its card until it closes.
    fn connect(&self, tv: Fingerprint) {
        let Some(sender) = &self.sender else { return };
        if self.connected() == Some(tv) && self.link.borrow().is_some() {
            return;
        }
        self.disconnect();
        let link = {
            let _entered = self.handle.enter();
            sender.link(&tv)
        };
        let Some(link) = link.map(Arc::new) else { return };
        let turn = self.link_turn.get();
        let mut cards = link.card();
        self.take(cards.borrow_and_update().clone());
        self.link.replace(Some(link));
        self.connected.replace(Some(tv));
        let (tx, rx) = async_channel::unbounded::<RemoteCard>();
        self.handle.spawn(async move {
            while cards.changed().await.is_ok() {
                let card = cards.borrow_and_update().clone();
                if tx.send(card).await.is_err() {
                    return;
                }
            }
        });
        let me = self.me.clone();
        glib::spawn_future_local(async move {
            while let Ok(card) = rx.recv().await {
                let Some(tv) = me.upgrade() else { return };
                if tv.link_turn.get() != turn {
                    return;
                }
                let over = card.over;
                tv.take(card);
                // The engine says when the session is over: the TV stopped playing, or stayed
                // away too long. The card stays while it is up, to say so.
                if over && tv.sheet.get() != Some(Sheet::Remote) {
                    tv.end();
                }
                tv.changed();
            }
        });
        self.changed();
    }

    /// The card the TV sent. The playhead moves on from where and when the TV last said it: a
    /// card that only says the link changed keeps that moment, so the line does not jump back,
    /// and stops where the link was lost.
    fn take(&self, next: RemoteCard) {
        let previous = self.card.borrow().clone();
        let same = previous.as_ref().is_some_and(|c| c.rev == next.rev && c.now == next.now);
        if next.phase == LinkPhase::Connected {
            self.lost_at.set(None);
        } else if self.lost_at.get().is_none()
            && previous.as_ref().is_some_and(|c| c.phase == LinkPhase::Connected)
        {
            self.lost_at.set(Some(Instant::now()));
        }
        if !same {
            self.card_at.set(Instant::now());
        }
        let started = self.starting.borrow().as_ref().is_some_and(|request| {
            next.now
                .as_ref()
                .is_some_and(|n| n.id == request.id && n.episode == request.episode)
        });
        if started || next.over {
            self.starting.replace(None);
        }
        self.card.replace(Some(next));
    }

    fn play_on(&self, tv: Fingerprint) {
        let Some(request) = self.request.take() else { return };
        let link = self.link.borrow().clone();
        let Some(link) = link.filter(|_| self.connected() == Some(tv)) else { return };
        self.starting.replace(Some(request.clone()));
        self.sheet.set(Some(Sheet::Remote));
        self.changed();
        let asked = request.clone();
        self.run(
            async move {
                link.play(asked.id, asked.title.clone(), asked.episode.clone(), asked.from)
                    .await
            },
            move |tv, answer| {
                let mine = tv.starting.borrow().as_ref() == Some(&request);
                match answer {
                    Ok(()) => {
                        // The TV playing it already sends nothing new.
                        let playing = tv.card.borrow().as_ref().and_then(|c| c.now.clone());
                        if mine
                            && playing.is_some_and(|n| {
                                n.id == request.id && n.episode == request.episode
                            })
                        {
                            tv.starting.replace(None);
                        }
                        if request.from_player
                            && let Some(handed_off) = tv.handed_off.borrow().as_ref()
                        {
                            handed_off();
                        }
                    }
                    Err(problem) => {
                        if mine {
                            tv.starting.replace(None);
                        }
                        tv.problem.replace(Some(problem));
                    }
                }
                tv.changed();
            },
        );
    }

    /// Stop driving the TV. What it plays plays on.
    pub fn disconnect(&self) {
        self.link_turn.set(self.link_turn.get() + 1);
        let link = self.link.take();
        {
            let _entered = self.handle.enter();
            drop(link);
        }
        self.connected.replace(None);
        self.card.replace(None);
        self.starting.replace(None);
        self.lost_at.set(None);
        self.ended.set(false);
        self.changed();
    }

    /// Send the TV a command from the card.
    pub fn send(&self, command: RemoteCommand) {
        let Some(link) = self.link.borrow().clone() else { return };
        if self.problem.replace(None).is_some() {
            self.changed();
        }
        self.run(async move { link.send(command).await }, |tv, answer| {
            if let Err(problem) = answer {
                tv.problem.replace(Some(problem));
                tv.changed();
            }
        });
    }

    /// Forget a paired TV, on both sides when it can be reached.
    pub fn forget_tv(&self, tv: Fingerprint) {
        let Some(sender) = self.sender.clone() else { return };
        if self.connected() == Some(tv) {
            self.disconnect();
            if self.sheet.get() == Some(Sheet::Remote) {
                self.sheet.set(None);
            }
        }
        self.paired.borrow_mut().retain(|p| p.fingerprint != tv);
        self.changed();
        self.run(async move { sender.forget(&tv).await }, |tv, _| tv.refresh());
    }

    /// The card's Pair Again: the TV forgot this computer, so this computer forgets it too and
    /// pairs afresh.
    pub fn pair_again(&self) {
        if let Some(tv) = self.connected() {
            self.forget_tv(tv);
        }
        self.open_picker();
    }

    /// Try the TV again now rather than after the link's wait: as the network comes back or the
    /// window comes to the front. A connected link stays as it is.
    pub fn retry_now(&self) {
        if let Some(link) = self.link.borrow().as_ref() {
            link.retry_now();
            return;
        }
        // Given up on while away: found again, it is driven again.
        if self.ended.get()
            && !self.lost()
            && let Some(tv) = self.connected()
        {
            self.connect(tv);
        }
    }

    fn watch_network(&self) {
        let monitor = gio::NetworkMonitor::default();
        let me = self.me.clone();
        monitor.connect_network_changed(move |_, available| {
            if available && let Some(tv) = me.upgrade() {
                tv.retry_now();
            }
        });
    }
}

/// The sender, with this install's key and the TVs paired before.
fn open_sender(handle: &Handle, data: &Path) -> Result<TvSender, String> {
    let dir = data.join("remote");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let identity = identity_in(&dir)?;
    let known = KnownTvs::open(&dir).map_err(|e| e.to_string())?;
    let hello = Hello {
        name: DEVICE.to_owned(),
        app: format!("anistream-linux/{}", anistream_engine::updates::CURRENT),
    };
    let _entered = handle.enter();
    Ok(TvSender::new(identity, hello, known))
}

/// This install's key, made the first time: a `0600` file, as the engine keeps its tokens on
/// this computer.
fn identity_in(dir: &Path) -> Result<Identity, String> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let path = dir.join(IDENTITY_FILE);
    if let Ok(kept) = std::fs::read(&path)
        && let Ok(identity) = Identity::from_pkcs8(&kept)
    {
        return Ok(identity);
    }
    let identity = Identity::generate().map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(&identity.to_pkcs8()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(identity)
}

/// What a problem with a TV says, in the language in use.
pub fn problem_text(problem: Problem) -> String {
    match problem {
        Problem::WrongCode => t::tv_problem_wrong_code(),
        Problem::NoCode => t::tv_code_expired(),
        Problem::Full => t::tv_problem_full(),
        Problem::Busy => t::tv_problem_busy(),
        Problem::Version => t::tv_problem_version(),
        Problem::NotNow => t::tv_problem_not_now(),
        Problem::Unpaired => t::tv_problem_unpaired(),
        Problem::KeyChanged => t::tv_problem_key_changed(),
        Problem::Unreachable => t::tv_problem_unreachable(),
        Problem::Gone => t::tv_problem_gone(),
        Problem::NotALink => t::tv_problem_not_a_link(),
        Problem::Other => t::tv_talk_failed(),
    }
}

/// What a TV in the picker says under its name.
pub fn row_detail(kind: TvRowKind) -> String {
    match kind {
        TvRowKind::Paired => t::tv_paired(),
        TvRowKind::PairedAway => t::tv_row_away(),
        TvRowKind::New => t::tv_row_new(),
        TvRowKind::NeedsUpdate => t::tv_row_needs_update(),
    }
}

/// What Settings says about a paired TV under its name: "Connected · 192.168.1.20:47811", or
/// "Paired 12 Oct · seen 3 hours ago".
pub fn paired_note(tv: &KnownTv, connected: bool, now: i64) -> String {
    if connected {
        return t::tv_connected_at(&tv.address.to_string());
    }
    let paired = glib::DateTime::from_unix_local(tv.paired_at)
        .ok()
        .and_then(|d| d.format("%-d %b").ok())
        .map(|d| t::tv_paired_on(&d))
        .unwrap_or_else(t::tv_paired);
    match tv.last_seen {
        Some(seen) if now - seen < 60 => t::tv_paired_seen_now(&paired),
        Some(seen) => t::tv_paired_seen(&paired, &t::time_ago(&say::countdown(now - seen))),
        None => paired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_made_once_and_kept_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("a scratch directory");
        let first = identity_in(dir.path()).expect("a key");
        let again = identity_in(dir.path()).expect("the same key");
        assert_eq!(first.fingerprint(), again.fingerprint());
        let mode = std::fs::metadata(dir.path().join(IDENTITY_FILE))
            .expect("the file")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn settings_say_where_a_tv_is_or_when_it_was_seen() {
        let tv = KnownTv {
            fingerprint: Identity::generate().expect("a key").fingerprint(),
            name: "Living room".into(),
            address: "192.168.1.20:47811".parse().expect("an address"),
            paired_at: 1_760_000_000,
            last_seen: Some(1_760_000_000),
        };
        assert_eq!(paired_note(&tv, true, 0), "Connected · 192.168.1.20:47811");
        let note = paired_note(&tv, false, 1_760_000_030);
        assert!(note.starts_with("Paired "), "{note}");
        assert!(note.ends_with(" · seen just now"), "{note}");
        let unseen = KnownTv { last_seen: None, ..tv };
        assert!(!paired_note(&unseen, false, 0).contains("seen"));
    }
}
