//! anistream for the Linux desktop: GTK 4 and libadwaita, with the engine in-process on a tokio
//! runtime of its own and its player hosted on libmpv. The engine's events reach GTK's main loop
//! over a channel; the window only ever queues commands, so nothing on the loop waits for it.
//!
//!   anistream-desktop [--hwdec <mode>] [--ao <output>]
//!   anistream-desktop --fixtures [--data <dir>] [--media <file>] [--downloading]
//!                     [--shots <dir> [--shot <a,b,...>] [--prefix <name>]] [--size <w>x<h>]

mod catalog;
mod engine;
mod fit;
#[cfg(feature = "fixtures")]
mod fixtures;
mod hub;
mod nagare;
mod pages;
mod player;
mod portal;
mod prefs;
mod shots;
mod style;
mod video;
mod widgets;
mod window;
mod words;

use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

use adw::prelude::*;
use anistream_core::config::Paths;
use anistream_engine::engine::Engine;
use anistream_model::{command::Command, event::Event};
use anistream_mpv_host::PlayerOptions;
use gtk::{gio, glib};

use crate::{engine::HostRequest, hub::Hub, prefs::Prefs, window::Shell};

/// The application's id: its D-Bus name, its desktop file and its icon.
const APP_ID: &str = "tv.anistream.desktop";

#[derive(Debug, Default)]
struct Args {
    fixtures: bool,
    data: Option<PathBuf>,
    media: Option<String>,
    downloading: bool,
    shots: Option<PathBuf>,
    shot: Option<String>,
    prefix: String,
    size: Option<(i32, i32)>,
    hwdec: Option<String>,
    audio_output: Option<String>,
}

fn args() -> Args {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--fixtures" => args.fixtures = true,
            "--data" => args.data = it.next().map(PathBuf::from),
            "--media" => args.media = it.next(),
            "--downloading" => args.downloading = true,
            "--shots" => args.shots = it.next().map(PathBuf::from),
            "--shot" => args.shot = it.next(),
            "--prefix" => args.prefix = it.next().unwrap_or_default(),
            "--size" => {
                args.size = it.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                });
            }
            "--hwdec" => args.hwdec = it.next(),
            "--ao" => args.audio_output = it.next(),
            other => tracing::warn!(arg = other, "an argument this app does not take"),
        }
    }
    args
}

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,anistream_engine=warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = args();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("anistream-engine")
        .build()
        .expect("a runtime for the engine");

    // What the engine asks of the desktop, for the main loop.
    let (host_tx, host_rx) = async_channel::unbounded::<HostRequest>();
    let host = Arc::new(engine::DesktopHost::new(host_tx));
    let (started, prefs_file, cache_dir) = match start(&runtime, &args, host) {
        Ok(started) => started,
        Err(e) => {
            tracing::error!(error = %e, "the engine did not start");
            return glib::ExitCode::FAILURE;
        }
    };
    let engine::Started { engine, mut events, keep } = started;
    let engine = Arc::new(engine);

    // The engine's events, in batches, onto the main loop.
    let (events_tx, events_rx) = async_channel::unbounded::<Vec<Event>>();
    runtime.spawn(async move {
        loop {
            let batch = events.next_events().await;
            if batch.is_empty() || events_tx.send(batch).await.is_err() {
                return;
            }
        }
    });

    adw::init().expect("libadwaita");
    // A scene runs beside the user's own copy, never in it.
    let flags = if args.fixtures {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
    };
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    let player = PlayerOptions {
        hwdec: args.hwdec.clone().unwrap_or_else(|| "auto-safe".into()),
        cache_dir: Some(cache_dir.join("mpv").display().to_string()),
        audio_output: args.audio_output.clone(),
        ..PlayerOptions::default()
    };
    let files = Files { prefs: prefs_file, cache: cache_dir };
    let pending = Rc::new(RefCell::new(Some((events_rx, host_rx))));
    let shell_slot: Rc<RefCell<Option<Rc<Shell>>>> = Rc::default();
    let args = Rc::new(args);
    app.connect_activate({
        let engine = Arc::downgrade(&engine);
        let shell_slot = Rc::clone(&shell_slot);
        let args = Rc::clone(&args);
        move |app| {
            if let Some(shell) = shell_slot.borrow().as_ref() {
                shell.ctx.window.present();
                return;
            }
            let Some((events, requests)) = pending.borrow_mut().take() else { return };
            let shell = build(app, engine.clone(), &args, &files, &player, events, requests);
            shell_slot.replace(Some(shell));
        }
    });
    let code = app.run_with_args::<&str>(&[]);
    shell_slot.replace(None);

    match Arc::try_unwrap(engine) {
        Ok(engine) => runtime.block_on(engine.shutdown()),
        Err(_) => tracing::warn!("the engine was still held at exit"),
    }
    drop(keep);
    runtime.shutdown_timeout(Duration::from_secs(2));
    code
}

/// Where the window keeps its own preferences, and the cache directory.
struct Files {
    prefs: PathBuf,
    cache: PathBuf,
}

/// The engine started, where the window keeps its own preferences, and the cache directory.
type Start = (engine::Started, PathBuf, PathBuf);

/// The engine on the user's own directories, or on the fixture scene under `--data`.
fn start(
    runtime: &tokio::runtime::Runtime,
    args: &Args,
    host: Arc<engine::DesktopHost>,
) -> Result<Start, String> {
    if args.fixtures {
        return start_fixtures(runtime, args, host);
    }
    let paths = Paths::resolve().map_err(|e| format!("{e:?}"))?;
    let prefs = paths.config_file.with_file_name("desktop.ini");
    let cache = paths.cache_dir.clone();
    let started = runtime.block_on(engine::start(paths, host))?;
    Ok((started, prefs, cache))
}

#[cfg(feature = "fixtures")]
fn start_fixtures(
    runtime: &tokio::runtime::Runtime,
    args: &Args,
    host: Arc<engine::DesktopHost>,
) -> Result<Start, String> {
    let root = args.data.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("anistream-scene-{}", std::process::id()))
    });
    let scene = fixtures::Scene { media: args.media.clone(), downloading: args.downloading };
    let paths = engine::paths_under(&root);
    let prefs = paths.config_file.with_file_name("desktop.ini");
    let cache = paths.cache_dir.clone();
    let started = runtime.block_on(fixtures::start(&root, &scene, host))?;
    tracing::info!(root = %root.display(), "the fixture scene");
    Ok((started, prefs, cache))
}

#[cfg(not(feature = "fixtures"))]
fn start_fixtures(
    _: &tokio::runtime::Runtime,
    _: &Args,
    _: Arc<engine::DesktopHost>,
) -> Result<Start, String> {
    Err("this build has no fixture scene: build it with --features fixtures".into())
}

fn build(
    app: &adw::Application,
    engine: std::sync::Weak<Engine>,
    args: &Args,
    files: &Files,
    player: &PlayerOptions,
    events: async_channel::Receiver<Vec<Event>>,
    requests: async_channel::Receiver<HostRequest>,
) -> Rc<Shell> {
    if let Some(display) = gtk::gdk::Display::default() {
        style::install(&display);
        style::app_icon(&display, APP_ID, &files.cache);
    }
    let hub = Hub::new(engine);
    let prefs = Rc::new(Prefs::open(files.prefs.clone()));
    let shell = Shell::build(app, Rc::clone(&hub), prefs, player, args.fixtures);
    if let Some((width, height)) = args.size {
        shell.ctx.window.unmaximize();
        shell.ctx.window.set_default_size(width, height);
    }
    hub.send(Command::LoadDownloads);

    glib::spawn_future_local({
        let hub = Rc::clone(&hub);
        async move {
            while let Ok(batch) = events.recv().await {
                hub.take(batch);
            }
        }
    });
    glib::spawn_future_local({
        let shell = Rc::downgrade(&shell);
        async move {
            while let Ok(request) = requests.recv().await {
                let Some(shell) = shell.upgrade() else { return };
                shell.host(request);
            }
        }
    });

    shell.ctx.window.present();
    if let Some(dir) = &args.shots {
        let shots = shots::named(args.shot.as_deref());
        shots::run(Rc::clone(&shell), dir.clone(), args.prefix.clone(), shots);
    }
    shell
}
