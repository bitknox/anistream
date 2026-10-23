//! anistream for the Linux desktop, a spike: a GTK 4 and libadwaita window that starts the
//! engine in-process, plays a file from disk through the engine's hosted player protocol on
//! libmpv, and draws native controls over the video.
//!
//!   anistream-desktop <file> [--data <dir>] [--hwdec <mode>] [--ao <output>] [--quit-after <s>]
//!                            [--stats] [--exercise]

mod engine;
mod video;

use std::{cell::Cell, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_engine::engine::Engine;
use anistream_model::{
    command::{Command, PlayerCommand},
    event::Event,
    hosted::PlayerUi,
};
use anistream_mpv_host::{HostEvent, Player, PlayerOptions, PlayerState};
use gtk::glib;

struct Args {
    file: PathBuf,
    data: PathBuf,
    hwdec: String,
    quit_after: Option<u32>,
    stats: bool,
    audio_output: Option<String>,
    /// Use the controls by themselves: seek to 0:30 at 4 s and pause at 6 s, through the same
    /// handlers a click and a drag reach.
    exercise: bool,
}

fn args() -> Args {
    let mut file = None;
    let mut data = None;
    let mut hwdec = "auto-safe".to_owned();
    let mut quit_after = None;
    let mut stats = false;
    let mut audio_output = None;
    let mut exercise = false;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--data" => data = it.next().map(PathBuf::from),
            "--hwdec" => hwdec = it.next().unwrap_or(hwdec),
            "--quit-after" => quit_after = it.next().and_then(|s| s.parse().ok()),
            "--stats" => stats = true,
            "--ao" => audio_output = it.next(),
            "--exercise" => exercise = true,
            _ => file = Some(PathBuf::from(arg)),
        }
    }
    let file = file.expect("usage: anistream-desktop <file> [--data <dir>] [--hwdec <mode>] [--quit-after <s>] [--stats]");
    let data = data.unwrap_or_else(|| std::env::temp_dir().join(format!("anistream-desktop-{}", std::process::id())));
    Args { file: std::fs::canonicalize(&file).unwrap_or(file), data, hwdec, quit_after, stats, audio_output, exercise }
}

fn main() -> glib::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,anistream_engine=warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = Rc::new(args());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("anistream-engine")
        .build()
        .expect("a runtime for the engine");
    let (engine, mut events) = match runtime.block_on(engine::start(&args.data)) {
        Ok(started) => started,
        Err(e) => {
            tracing::error!(error = %e, "the engine did not start");
            return glib::ExitCode::FAILURE;
        }
    };
    let engine = Arc::new(engine);
    tracing::info!(data = %args.data.display(), "the engine started");

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
    let app = adw::Application::builder().application_id("tv.anistream.desktop.Spike").build();
    let events_rx = Rc::new(Cell::new(Some(events_rx)));
    app.connect_activate({
        let engine = Link(Arc::downgrade(&engine));
        let args = Rc::clone(&args);
        move |app| {
            if let Some(events) = events_rx.take() {
                build(app, &engine, &args, events);
            }
        }
    });
    let code = app.run_with_args::<&str>(&[]);

    match Arc::try_unwrap(engine) {
        Ok(engine) => runtime.block_on(engine.shutdown()),
        Err(_) => tracing::warn!("the engine was still held at exit"),
    }
    runtime.shutdown_timeout(Duration::from_secs(2));
    code
}

/// The engine, as the window's handlers hold it: not keeping it up, so the exit can shut it down.
#[derive(Clone)]
struct Link(std::sync::Weak<Engine>);

impl Link {
    fn send(&self, command: Command) {
        if let Some(engine) = self.0.upgrade() {
            engine.send(command);
        }
    }
}

const CSS: &str = "
.controls { border-radius: 14px; padding: 6px 14px; margin: 18px; }
.controls label { font-feature-settings: 'tnum'; }
.status { border-radius: 10px; padding: 6px 12px; margin: 18px; }
";

fn build(
    app: &adw::Application,
    engine: &Link,
    args: &Rc<Args>,
    events: async_channel::Receiver<Vec<Event>>,
) {
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("a display"),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // The player: reports go straight to the engine from mpv's thread; the rest to the main loop.
    let (host_tx, host_rx) = async_channel::unbounded::<HostEvent>();
    let sink = {
        let engine = engine.clone();
        Arc::new(move |event: HostEvent| match event {
            HostEvent::Report { playback, report } => {
                engine.send(Command::PlayerReport { playback, report });
            }
            other => {
                let _ = host_tx.send_blocking(other);
            }
        })
    };
    let options = PlayerOptions {
        hwdec: args.hwdec.clone(),
        cache_dir: Some(args.data.join("cache").display().to_string()),
        audio_output: args.audio_output.clone(),
        ..PlayerOptions::default()
    };
    let player = match Player::new(&options, sink) {
        Ok(player) => Rc::new(player),
        Err(e) => {
            tracing::error!(error = %e, "mpv did not start");
            app.quit();
            return;
        }
    };

    // Play the file once mpv can draw: the engine stages it and drives the player.
    let video = video::video_area(player.mpv().clone(), {
        let engine = engine.clone();
        let path = args.file.display().to_string();
        move || {
            tracing::info!(%path, "playing");
            engine.send(Command::PlayLocal {
                id: AnilistId::new(1),
                episode: "1".into(),
                title: "Signal Test".into(),
                path,
            });
        }
    });

    // The controls, native widgets over the video.
    let play = gtk::Button::from_icon_name("media-playback-pause-symbolic");
    play.add_css_class("flat");
    play.add_css_class("circular");
    play.set_tooltip_text(Some("Pause"));
    let position = gtk::Label::new(Some("0:00"));
    let length = gtk::Label::new(Some("0:00"));
    for label in [&position, &length] {
        label.add_css_class("numeric");
    }
    let seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    seek.set_hexpand(true);
    seek.set_draw_value(false);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    controls.add_css_class("osd");
    controls.add_css_class("controls");
    controls.set_valign(gtk::Align::End);
    controls.append(&play);
    controls.append(&position);
    controls.append(&seek);
    controls.append(&length);

    let status = gtk::Label::new(Some("Starting the engine"));
    status.add_css_class("osd");
    status.add_css_class("status");
    status.set_halign(gtk::Align::Start);
    status.set_valign(gtk::Align::Start);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&video.widget));
    overlay.add_overlay(&controls);
    overlay.add_overlay(&status);

    let title = adw::WindowTitle::new("anistream", "Linux spike");
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&title));
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&overlay));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("anistream")
        .default_width(1280)
        .default_height(760)
        .content(&toolbar)
        .build();

    play.connect_clicked({
        let engine = engine.clone();
        move |_| {
            engine.send(Command::Player(PlayerCommand::TogglePause));
        }
    });
    // A drag on the bar seeks; the playhead's own updates are held off while it does.
    let held_until = Rc::new(Cell::new(std::time::Instant::now()));
    seek.connect_change_value({
        let engine = engine.clone();
        let held_until = Rc::clone(&held_until);
        move |_, _, value| {
            held_until.set(std::time::Instant::now() + Duration::from_millis(400));
            engine.send(Command::Player(PlayerCommand::SeekTo(value)));
            glib::Propagation::Proceed
        }
    });
    window.connect_close_request({
        let player = Rc::clone(&player);
        move |_| {
            player.viewer_closed();
            glib::Propagation::Proceed
        }
    });

    // What the player says the interface should show.
    glib::spawn_future_local({
        let (play, seek, position, length, status) =
            (play.clone(), seek.clone(), position.clone(), length.clone(), status.clone());
        let (player, window, gl) = (Rc::clone(&player), window.clone(), Rc::clone(&video.gl));
        let held_until = Rc::clone(&held_until);
        async move {
            while let Ok(event) = host_rx.recv().await {
                match event {
                    HostEvent::Opened { .. } => {
                        status_show(&status, "Opening");
                        report_display(&player, &window, gl.get());
                    }
                    HostEvent::Closed => status_show(&status, "Playback ended"),
                    HostEvent::Fullscreen(on) => {
                        let on = on.unwrap_or(!window.is_fullscreen());
                        window.set_fullscreened(on);
                    }
                    HostEvent::State(state) => {
                        show_state(&state, &play, &seek, &position, &length, held_until.get());
                        if state.has_picture {
                            status.set_visible(false);
                        }
                    }
                    HostEvent::Report { .. } => {}
                }
            }
        }
    });

    // What the engine says.
    glib::spawn_future_local({
        let (player, title, status) = (Rc::clone(&player), title.clone(), status.clone());
        async move {
            while let Ok(batch) = events.recv().await {
                for event in batch {
                    match event {
                        Event::PlayerOps { playback, ops } => {
                            for op in &ops {
                                tracing::info!(playback = playback.get(), ?op, "op");
                                player.apply(playback, op);
                            }
                        }
                        Event::PlayerUi { ui, .. } => match ui {
                            PlayerUi::Starting { episode, title: show, .. } => {
                                title.set_title(&show);
                                status_show(&status, &format!("Starting episode {episode}"));
                            }
                            PlayerUi::Episode { label, show, .. } => {
                                title.set_title(&show);
                                title.set_subtitle(&label);
                            }
                            PlayerUi::Alert { text, .. } => status_show(&status, &text),
                            other => tracing::debug!(?other, "player interface"),
                        },
                        Event::Toast(notice) => tracing::info!(text = %notice.text, "notice"),
                        _ => {}
                    }
                }
            }
        }
    });

    if args.stats {
        glib::timeout_add_seconds_local(2, {
            let (pacing, player) = (Rc::clone(&video.pacing), Rc::clone(&player));
            move || {
                let (fps, worst) = pacing.take();
                let mpv = player.mpv();
                let get = |p: &str| mpv.get(p).map(|v| v.to_string()).unwrap_or_else(|| "-".into());
                tracing::info!(
                    drawn_fps = format!("{fps:.1}"),
                    worst_gap_ms = format!("{worst:.1}"),
                    video_fps = get("estimated-vf-fps"),
                    display_fps = get("display-fps"),
                    drops = get("frame-drop-count"),
                    delayed = get("vo-delayed-frame-count"),
                    mistimed = get("mistimed-frame-count"),
                    hwdec = get("hwdec-current"),
                    "pacing"
                );
                glib::ControlFlow::Continue
            }
        });
    }
    if args.exercise {
        glib::timeout_add_seconds_local_once(4, {
            let seek = seek.clone();
            move || {
                tracing::info!("exercise: seeking to 0:30");
                seek.emit_by_name::<bool>("change-value", &[&gtk::ScrollType::Jump, &30.0f64]);
            }
        });
        glib::timeout_add_seconds_local_once(6, {
            let play = play.clone();
            move || {
                tracing::info!("exercise: pausing");
                play.emit_clicked();
            }
        });
    }
    if let Some(seconds) = args.quit_after {
        glib::timeout_add_seconds_local_once(seconds, {
            let window = window.clone();
            move || window.close()
        });
    }
    window.present();
}

fn status_show(status: &gtk::Label, text: &str) {
    status.set_label(text);
    status.set_visible(true);
}

/// The display the window is on, for the engine's upscaling: its size in pixels, scale and
/// refresh rate, and whether the GL context runs compute shaders (desktop GL 4.3 and up).
fn report_display(player: &Player, window: &adw::ApplicationWindow, gl: Option<(i32, i32, bool)>) {
    let Some(surface) = window.surface() else { return };
    let Some(monitor) = gtk::prelude::WidgetExt::display(window).monitor_at_surface(&surface) else {
        return;
    };
    let geometry = monitor.geometry();
    let scale = monitor.scale();
    let refresh = monitor.refresh_rate();
    let compute = gl.is_some_and(|(major, minor, gles)| !gles && (major, minor) >= (4, 3));
    player.display(
        (f64::from(geometry.width()) * scale) as u32,
        (f64::from(geometry.height()) * scale) as u32,
        scale,
        (refresh > 0).then(|| f64::from(refresh) / 1000.0),
        compute,
    );
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

fn show_state(
    state: &PlayerState,
    play: &gtk::Button,
    seek: &gtk::Scale,
    position: &gtk::Label,
    length: &gtk::Label,
    held_until: std::time::Instant,
) {
    let (icon, tip) = if state.paused {
        ("media-playback-start-symbolic", "Play")
    } else {
        ("media-playback-pause-symbolic", "Pause")
    };
    play.set_icon_name(icon);
    play.set_tooltip_text(Some(tip));
    if let Some(duration) = state.duration {
        seek.set_range(0.0, duration.max(0.1));
        length.set_label(&clock(duration));
    }
    if let Some(pos) = state.position {
        position.set_label(&clock(pos));
        if std::time::Instant::now() >= held_until {
            seek.set_value(pos);
        }
    }
}
