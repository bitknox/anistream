//! anistream for the Linux desktop: GTK 4 and libadwaita, with the engine in-process on a tokio
//! runtime of its own and its player hosted on libmpv. The engine's events reach GTK's main loop
//! over a channel; the window only ever queues commands, so nothing on the loop waits for it.
//!
//!   anistream-desktop [--hwdec <mode>] [--ao <output>] [anistream://…]
//!   anistream-desktop --fixtures [--data <dir>] [--media <file>] [--downloading] [--showcase]
//!                     [--scene <home scene>]
//!                     [--shots <dir> [--shot <a,b,...>] [--prefix <name>]] [--size <w>x<h>]

mod catalog;
mod choices;
mod engine;
mod fit;
#[cfg(feature = "fixtures")]
mod fixtures;
mod flags;
mod hub;
mod motion;
mod mpris;
mod nagare;
mod pad;
mod pages;
mod party;
mod player;
mod portal;
mod prefs;
mod say;
mod shots;
#[cfg(feature = "fixtures")]
mod showcase;
mod states;
mod style;
mod trailer;
mod tv;
mod updater;
mod updates;
mod video;
mod widgets;
mod window;

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
    /// Plugin stores, their providers and episodes seeding, which the fixture engine has none of.
    showcase: bool,
    /// The state the fixture scene's Home starts in, by name: see `fixtures::HomeScene`.
    scene: Option<String>,
    shots: Option<PathBuf>,
    shot: Option<String>,
    prefix: String,
    size: Option<(i32, i32)>,
    hwdec: Option<String>,
    audio_output: Option<String>,
    /// `anistream://` links to open: a tracker's sign-in coming back from the browser.
    links: Vec<String>,
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
            "--showcase" => args.showcase = true,
            "--scene" => args.scene = it.next(),
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
            link if link.starts_with("anistream:") => args.links.push(link.to_owned()),
            other => tracing::warn!(arg = other, "an argument this app does not take"),
        }
    }
    args
}

fn main() -> glib::ExitCode {
    // Answered before GTK starts, so a package can check what it built without a display.
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("anistream {}", anistream_engine::updates::CURRENT); // i18n-skip
        return glib::ExitCode::SUCCESS;
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,anistream_engine=warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = args();
    // The language chosen in Settings (the config's own key, the one the terminal app reads),
    // else the system's. `ANISTREAM_LANG` still wins over both.
    anistream_i18n::init(saved_language(&args).as_deref());

    adw::init().expect("libadwaita");
    // A scene runs beside the user's own copy, never in it.
    let flags = if args.fixtures {
        gio::ApplicationFlags::NON_UNIQUE | gio::ApplicationFlags::HANDLES_OPEN
    } else {
        gio::ApplicationFlags::HANDLES_OPEN
    };
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    // GApplication is given only the links: the app's own options are its own.
    let argv: Vec<String> =
        std::iter::once("anistream-desktop".to_owned()).chain(args.links.clone()).collect();
    // Already running: the links go to that copy, and this one starts nothing of its own.
    if app.register(gio::Cancellable::NONE).is_ok() && app.is_remote() {
        return app.run_with_args(&argv);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("anistream-engine")
        .build()
        .expect("a runtime for the engine");

    // What the engine asks of the desktop, for the main loop.
    let (host_tx, host_rx) = async_channel::unbounded::<HostRequest>();
    let host = Arc::new(engine::DesktopHost::new(host_tx));
    let (started, paths) = match start(&runtime, &args, host) {
        Ok(started) => started,
        Err(e) => {
            tracing::error!(error = %e, "the engine did not start");
            return glib::ExitCode::FAILURE;
        }
    };
    let cache_dir = paths.cache_dir.clone();
    let engine::Started { engine, mut events, keep, rebuilt } = started;
    let engine = Arc::new(engine);
    // What casting to a TV with Chromecast built in reads of the engine: its hub, which hears the
    // events below, and where the card's commands go.
    let casting = (engine.casts(), engine.remote_sender());
    save_on_panic(Arc::downgrade(&engine), runtime.handle().clone());

    // The engine's events, in batches, onto the main loop.
    let (events_tx, events_rx) = async_channel::unbounded::<Vec<Event>>();
    let casts = engine.casts();
    runtime.spawn(async move {
        loop {
            let batch = events.next_events().await;
            for event in &batch {
                casts.observe(event);
            }
            // Said once the rebuild is done, so what is put back stays.
            if let Some(rebuilt) = &rebuilt
                && batch.iter().any(|event| matches!(event, Event::SourcesChanged))
            {
                rebuilt();
            }
            if batch.is_empty() || events_tx.send(batch).await.is_err() {
                return;
            }
        }
    });

    let player = PlayerOptions {
        hwdec: args.hwdec.clone().unwrap_or_else(|| "auto-safe".into()),
        cache_dir: Some(cache_dir.join("mpv").display().to_string()),
        audio_output: args.audio_output.clone(),
        ..PlayerOptions::default()
    };
    let files = Files {
        prefs: paths.config_file.with_file_name("desktop.ini"),
        cache: cache_dir,
        data: paths.data_dir.clone(),
        paths,
        runtime: runtime.handle().clone(),
        casting: tv::cast::Casting { hub: casting.0, engine: casting.1 },
    };
    let pending = Rc::new(RefCell::new(Some((events_rx, host_rx))));
    let shell_slot: Rc<RefCell<Option<Rc<Shell>>>> = Rc::default();
    let args = Rc::new(args);
    // The window, built the first time the app is activated or handed a link.
    let shown = Rc::new({
        let engine = Arc::downgrade(&engine);
        let shell_slot = Rc::clone(&shell_slot);
        let args = Rc::clone(&args);
        move |app: &adw::Application| -> Option<Rc<Shell>> {
            if let Some(shell) = shell_slot.borrow().as_ref() {
                shell.ctx.window.present();
                return Some(Rc::clone(shell));
            }
            let (events, requests) = pending.borrow_mut().take()?;
            let shell = build(app, engine.clone(), &args, &files, &player, events, requests);
            shell_slot.replace(Some(Rc::clone(&shell)));
            Some(shell)
        }
    });
    app.connect_activate({
        let shown = Rc::clone(&shown);
        move |app| {
            shown(app);
        }
    });
    app.connect_open({
        let shown = Rc::clone(&shown);
        move |app, files, _| {
            let Some(shell) = shown(app) else { return };
            for file in files {
                shell.open_link(&file.uri());
            }
        }
    });
    let code = app.run_with_args(&argv);
    shell_slot.replace(None);

    match Arc::try_unwrap(engine) {
        Ok(engine) => runtime.block_on(engine.shutdown()),
        Err(_) => tracing::warn!("the engine was still held at exit"),
    }
    drop(keep);
    runtime.shutdown_timeout(Duration::from_secs(2));
    code
}

/// A panic the app does not catch ends it. Before it does, the engine is asked to wind down, which
/// saves where the episode on screen had got to; a panic a callback guard catches leaves the
/// engine alone.
fn save_on_panic(
    engine: std::sync::Weak<anistream_engine::engine::Engine>,
    runtime: tokio::runtime::Handle,
) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(%info, "the app panicked");
        if !hub::in_guard()
            && tokio::runtime::Handle::try_current().is_err()
            && let Some(engine) = engine.upgrade()
        {
            tracing::error!("saving the last position before the app ends");
            runtime.block_on(engine.stopping());
        }
        previous(info);
    }));
}

/// Where the window keeps its own preferences, the cache and data directories, and the engine's
/// runtime, which Play on TV's sender runs on.
struct Files {
    prefs: PathBuf,
    cache: PathBuf,
    data: PathBuf,
    /// Where the engine's config file is: the window reads and writes the language there.
    paths: Paths,
    runtime: tokio::runtime::Handle,
    /// What Play on TV casts through.
    casting: tv::cast::Casting,
}

impl Files {
    fn casting(&self) -> tv::cast::Casting {
        tv::cast::Casting {
            hub: Arc::clone(&self.casting.hub),
            engine: self.casting.engine.clone(),
        }
    }
}

/// The engine started, and the directories and config file it runs on.
type Start = (engine::Started, Paths);

/// The language saved in the config the app is about to load: the user's own, or a fixture
/// scene's under `--data`. `None` follows the system's.
fn saved_language(args: &Args) -> Option<String> {
    let file = if args.fixtures {
        #[cfg(feature = "fixtures")]
        {
            engine::paths_under(args.data.as_deref()?).config_file
        }
        #[cfg(not(feature = "fixtures"))]
        {
            return None;
        }
    } else {
        Paths::resolve().ok()?.config_file
    };
    prefs::saved_language(&file)
}

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
    let started = runtime.block_on(engine::start(paths.clone(), host))?;
    Ok((started, paths))
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
    let home = match args.scene.as_deref() {
        None => fixtures::HomeScene::default(),
        Some(name) => {
            fixtures::HomeScene::named(name).ok_or_else(|| format!("no scene named {name}"))?
        } // i18n-skip
    };
    let scene =
        fixtures::Scene { media: args.media.clone(), downloading: args.downloading, home };
    let paths = engine::paths_under(&root);
    fixtures::keep_runtime(runtime.handle().clone());
    let started = runtime.block_on(fixtures::start(&root, &scene, host))?;
    tracing::info!(root = %root.display(), "the fixture scene");
    Ok((started, paths))
}

#[cfg(not(feature = "fixtures"))]
fn start_fixtures(
    _: &tokio::runtime::Runtime,
    _: &Args,
    _: Arc<engine::DesktopHost>,
) -> Result<Start, String> {
    Err("this build has no fixture scene: build it with --features fixtures".into()) // i18n-skip
}

/// Play on TV's sender: on the scene's TVs on loopback for a fixture run, never a real one.
fn play_on_tv(args: &Args, files: &Files) -> Rc<tv::Tv> {
    #[cfg(feature = "fixtures")]
    if args.fixtures {
        return tv::fixture::tv(files.runtime.clone(), &files.data, Some(files.casting()));
    }
    let _ = args;
    tv::Tv::new(files.runtime.clone(), &files.data, Some(files.casting()))
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
        style::action_icons(&display, &files.cache);
    }
    let hub = Hub::new(engine);
    let prefs = Rc::new(Prefs::open(files.prefs.clone()).with_config(files.paths.clone()));
    let updater = updater::Updater::new(&files.cache, args.fixtures);
    let tv = play_on_tv(args, files);
    let shell = Shell::build(app, Rc::clone(&hub), prefs, player, updater, tv, args.fixtures);
    // A scene's media controls sit beside the user's own copy's, as MPRIS has instances do.
    let media_name = if args.fixtures {
        format!("{}.instance{}", mpris::NAME, std::process::id())
    } else {
        mpris::NAME.to_owned()
    };
    shell.player.serve_media_controls(&media_name);
    if let Some((width, height)) = args.size {
        shell.ctx.window.unmaximize();
        shell.ctx.window.set_default_size(width, height);
    }
    hub.send(Command::LoadDownloads);
    #[cfg(feature = "fixtures")]
    if args.fixtures && args.showcase {
        hub.show_instead(showcase::events());
    }

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
