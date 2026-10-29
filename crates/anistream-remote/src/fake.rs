//! A TV for tests and the apps' Fixtures builds: a real [`RemoteServer`] on the loopback
//! interface, with a scripted player behind it in place of an engine. It pairs, takes commands
//! and sends state exactly as a TV does; what it plays is made up.
//!
//! Built with the `testing` feature only. Never in an app's release build.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, PoisonError, Weak},
    time::Duration,
};

use anistream_core::ids::AnilistId;
use anistream_model::{
    command::{Command, PlayerCommand},
    event::Event,
    hosted::{PlayerReport, TrackKind},
    playback::{Ending, SkipKind},
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    Error,
    command::FrontIntent,
    discovery::{Advert, PairLink},
    identity::{Fingerprint, Identity},
    server::{CommandSink, RemoteServer, ServerOptions},
    state::Screen,
    trust::MemoryTrust,
};

/// A title the fake TV can play: what `play` with its id stages.
#[derive(Debug, Clone)]
pub struct FakeTitle {
    pub id: AnilistId,
    pub title: String,
    /// Episode titles by number, "1" first. An episode not listed plays without a title.
    pub episodes: Vec<String>,
    /// Each episode's length, in seconds.
    pub duration: f64,
}

/// The fake TV's player, as the script moves it.
#[derive(Debug, Clone)]
struct Playing {
    title: FakeTitle,
    episode: u32,
    position: f64,
    paused: bool,
    audio: i64,
    subtitle: Option<i64>,
}

/// Where the fake's server sends commands: the script's channel.
struct Sink(mpsc::UnboundedSender<Routed>);

enum Routed {
    Engine(Command),
    Front(FrontIntent),
}

impl CommandSink for Sink {
    fn engine(&self, command: Command) {
        let _ = self.0.send(Routed::Engine(command));
    }

    fn front(&self, intent: FrontIntent) {
        let _ = self.0.send(Routed::Front(intent));
    }
}

#[derive(Default)]
struct Script {
    playing: Option<Playing>,
    volume: f64,
    /// The code shown for a phone that asked, as the TV front end would show it.
    shown: Option<String>,
    query: String,
    commands: Vec<String>,
}

/// A TV on 127.0.0.1, running until dropped.
pub struct FakeTv {
    server: Arc<RemoteServer>,
    script: Arc<Mutex<Script>>,
    task: JoinHandle<()>,
}

impl FakeTv {
    /// Listen on any free loopback port under `name`, with `titles` to play. Spawns on the
    /// runtime it is called from.
    pub async fn start(name: &str, titles: Vec<FakeTitle>) -> Result<Self, Error> {
        let (tx, rx) = mpsc::unbounded_channel();
        let options = ServerOptions { tv_name: name.to_owned(), ..ServerOptions::default() };
        let server = RemoteServer::bind(
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            Identity::generate()?,
            Arc::new(MemoryTrust::new()),
            Arc::new(Sink(tx)),
            options,
        )
        .await?;
        let server = Arc::new(server);
        server.set_volume(40.0);
        let script = Arc::new(Mutex::new(Script { volume: 40.0, ..Script::default() }));
        let task = tokio::spawn(run(Arc::downgrade(&server), Arc::clone(&script), titles, rx));
        Ok(Self { server, script, task })
    }

    pub fn addr(&self) -> SocketAddr {
        self.server.local_addr()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.server.fingerprint()
    }

    pub fn advert(&self) -> Advert {
        self.server.advert()
    }

    /// A code opened from the TV's side, as its Pair a phone screen opens one.
    pub fn open_code(&self) -> String {
        self.server.open_code()
    }

    /// The link its QR code would carry, with `code`.
    pub fn pair_link(&self, code: Option<String>) -> String {
        PairLink { addr: self.addr(), tv: Some(self.fingerprint()), code }.to_string()
    }

    /// The code on screen for a phone that asked for one.
    pub fn shown_code(&self) -> Option<String> {
        lock(&self.script).shown.clone()
    }

    /// The commands it was sent, as the script names them, oldest first.
    pub fn commands(&self) -> Vec<String> {
        lock(&self.script).commands.clone()
    }

    /// Forget every phone, as the TV's settings would one by one.
    pub fn revoke_all(&self) {
        for phone in self.server.paired() {
            let _ = self.server.revoke(&phone.fingerprint);
        }
    }

    pub fn paired(&self) -> usize {
        self.server.paired().len()
    }

    /// The TV's own volume, out of 100.
    pub fn volume(&self) -> f64 {
        lock(&self.script).volume
    }
}

impl Drop for FakeTv {
    fn drop(&mut self) {
        self.task.abort();
        self.server.shutdown();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How far the scripted playhead moves per tick of media time.
const TICK: Duration = Duration::from_secs(1);

async fn run(
    server: Weak<RemoteServer>,
    script: Arc<Mutex<Script>>,
    titles: Vec<FakeTitle>,
    mut rx: mpsc::UnboundedReceiver<Routed>,
) {
    let mut tick = tokio::time::interval(TICK);
    loop {
        let routed = tokio::select! {
            routed = rx.recv() => match routed { Some(routed) => Some(routed), None => return },
            _ = tick.tick() => None,
        };
        let Some(server) = server.upgrade() else { return };
        let mut script = lock(&script);
        match routed {
            None => advance(&server, &mut script),
            Some(Routed::Engine(command)) => engine(&server, &mut script, &titles, command),
            Some(Routed::Front(intent)) => front(&server, &mut script, intent),
        }
    }
}

fn advance(server: &RemoteServer, script: &mut Script) {
    let Some(playing) = &mut script.playing else { return };
    if playing.paused {
        return;
    }
    playing.position = (playing.position + TICK.as_secs_f64()).min(playing.title.duration);
    server.observe(&position(playing));
    if playing.position >= 90.0 {
        server.observe(&Event::SkipCleared);
    }
}

fn position(playing: &Playing) -> Event {
    Event::Playback {
        position: playing.position,
        duration: Some(playing.title.duration),
        paused: playing.paused,
    }
}

fn stage(server: &RemoteServer, playing: &Playing) {
    let number = playing.episode.to_string();
    let episode_title = playing
        .title
        .episodes
        .get(playing.episode as usize - 1)
        .cloned()
        .filter(|t| !t.is_empty());
    server.observe(&Event::PlaybackStaged {
        id: playing.title.id,
        title: playing.title.title.clone(),
        episode: number,
        episode_title,
        speed: 1.0,
    });
    tracks(server, playing);
    server.observe(&position(playing));
    if playing.position < 90.0 {
        server.observe(&Event::SkipAvailable { kind: SkipKind::Opening, to: 90.0 });
    }
}

fn tracks(server: &RemoteServer, playing: &Playing) {
    let sub = |id: i64, lang: &str, title: &str| {
        format!(
            r#"{{"id":{id},"type":"sub","lang":"{lang}","title":"{title}","selected":{}}}"#,
            playing.subtitle == Some(id)
        )
    };
    let audio = |id: i64, lang: &str| {
        format!(
            r#"{{"id":{id},"type":"audio","lang":"{lang}","selected":{}}}"#,
            playing.audio == id
        )
    };
    let json = format!(
        r#"[{{"id":1,"type":"video"}},{},{},{},{}]"#,
        audio(1, "jpn"),
        audio(2, "eng"),
        sub(1, "eng", "Full"),
        sub(2, "eng", "Signs and songs"),
    );
    server.observe_report(&PlayerReport::Property { name: "track-list".into(), json });
}

fn engine(server: &RemoteServer, script: &mut Script, titles: &[FakeTitle], command: Command) {
    let named = match &command {
        Command::Play { id, episode, .. } => format!("play {id} {episode}"),
        Command::Player(player) => format!("{player:?}"),
        other => format!("{other:?}"),
    };
    script.commands.push(named);
    match command {
        Command::Play { id, episode, .. } => {
            let title =
                titles.iter().find(|t| t.id == id).cloned().unwrap_or_else(|| FakeTitle {
                    id,
                    title: format!("Title {id}"),
                    episodes: Vec::new(),
                    duration: 1440.0,
                });
            let playing = Playing {
                title,
                episode: episode.parse().unwrap_or(1).max(1),
                position: 0.0,
                paused: false,
                audio: 1,
                subtitle: Some(1),
            };
            server.set_screen(Screen::Player);
            stage(server, &playing);
            script.playing = Some(playing);
        }
        Command::Player(player) => {
            let Some(playing) = &mut script.playing else { return };
            match player {
                PlayerCommand::TogglePause => playing.paused = !playing.paused,
                PlayerCommand::Seek(by) => {
                    playing.position =
                        (playing.position + by).clamp(0.0, playing.title.duration);
                }
                PlayerCommand::SeekTo(to) => {
                    playing.position = to.clamp(0.0, playing.title.duration)
                }
                PlayerCommand::TakeSkip => {
                    playing.position = 90.0;
                    server.observe(&Event::SkipCleared);
                }
                PlayerCommand::SpeedTo(speed) => server.observe(&Event::PlaybackSpeed(speed)),
                PlayerCommand::Step(by) => {
                    let next = i64::from(playing.episode) + by;
                    playing.episode = u32::try_from(next.max(1)).unwrap_or(1);
                    playing.position = 0.0;
                    playing.paused = false;
                    server.observe(&Event::PlaybackEnded {
                        watched: by > 0,
                        ending: Ending::Complete { next: Some(playing.episode.to_string()) },
                        rolling_on: true,
                    });
                    stage(server, playing);
                    return;
                }
                PlayerCommand::ChooseTrack { kind, id } => {
                    match kind {
                        TrackKind::Audio => playing.audio = id.unwrap_or(playing.audio),
                        TrackKind::Subtitle => playing.subtitle = id,
                    }
                    tracks(server, playing);
                    return;
                }
                PlayerCommand::Stop => {
                    script.playing = None;
                    server.observe(&Event::playback_ended(false, Ending::Stopped));
                    server.set_screen(Screen::Home);
                    return;
                }
                _ => return,
            }
            server.observe(&position(playing));
        }
        _ => {}
    }
}

fn front(server: &RemoteServer, script: &mut Script, intent: FrontIntent) {
    match intent {
        FrontIntent::Volume { step } => {
            script.volume = (script.volume + step).clamp(0.0, 100.0);
            script.commands.push(format!("volume {step}"));
            server.set_volume(script.volume);
        }
        FrontIntent::Text { text, submit } => {
            script
                .commands
                .push(format!("text {text}{}", if submit { " (search)" } else { "" }));
            script.query = text;
            if script.playing.is_none() {
                server.set_screen(Screen::Search { query: script.query.clone() });
            }
        }
        FrontIntent::ShowPairingCode { code, .. } => script.shown = Some(code),
        FrontIntent::HidePairingCode => script.shown = None,
        FrontIntent::Open { id } => {
            script.commands.push(format!("open {id}"));
            if script.playing.is_none() {
                server.set_screen(Screen::Title { id });
            }
        }
        FrontIntent::Navigate { key } => script.commands.push(format!("{key:?}")),
        FrontIntent::Mute { muted } => script.commands.push(format!("mute {muted}")),
        FrontIntent::Paired { .. } => {}
    }
}
