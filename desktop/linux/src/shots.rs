//! Screenshots of the window, taken by the app itself: each shot puts the window in a state,
//! waits for the engine and the art to settle, and draws the window into a PNG. Run under the
//! fixture scene, so every run draws the same titles.
//!
//!   anistream-desktop --fixtures --shots <dir> --shot home,title,... [--size 1280x800]

use std::{path::PathBuf, rc::Rc, time::Duration};

use adw::prelude::*;
use anistream_model::{command::Command, sync::DeviceCodePrompt};
use gtk::glib;

use crate::{hub::SignIn, pages::Dest, window::Shell};

/// The title the title shots open: the fixture scene's finished title, part-way through
/// episode 7.
const TITLE: u32 = 900_001;
/// The title the player shot plays: episode 4 is the next, from its start.
const PLAYING: (u32, &str) = (900_002, "4");

/// Every shot by name, in the order a run takes them.
pub const ALL: [&str; 35] = [
    "home",
    "welcome",
    "offline",
    "calendar",
    "seasonal",
    "search",
    "search-results",
    "title",
    "show-settings",
    "library",
    "downloads",
    "providers",
    "provider",
    "stores",
    "store",
    "join",
    "settings",
    "settings-general",
    "settings-upscaling",
    "settings-accounts",
    "accounts-kitsu",
    "accounts-code",
    "torrents-setup",
    "torrents-none",
    "torrents-on",
    "settings-updates",
    "update-ready",
    "about",
    "player",
    "player-marks",
    "player-tracks",
    "player-undo",
    "mini",
    "resumed",
    "close",
];

/// The watch party's shots, apart from the rest: each holds a party on the scene's own server,
/// and they take long enough that a run of every shot would not fit its time. `--shot party`
/// takes them all.
pub const PARTY: [&str; 15] = [
    "party-form",
    "party-joining",
    "party-refused",
    "party-invite",
    "party-panel",
    "party-ready",
    "party-chat",
    "party-left",
    "party-lost",
    "party-close",
    "party-nosource",
    "party-choose",
    "party-unaired",
    "party-nosources",
    "party-title-menu",
];

/// Take `shots` into `dir`, each named `<prefix><shot>.png`, then quit.
pub fn run(shell: Rc<Shell>, dir: PathBuf, prefix: String, shots: Vec<String>) {
    glib::spawn_future_local(async move {
        // The first page, its lists and their covers.
        wait(4.0).await;
        if let Some(child) = shell.ctx.window.content() {
            wide_widgets(&child, 0, shell.ctx.window.width());
        }
        for shot in &shots {
            let settle = stage(&shell, shot);
            wait(settle).await;
            let path = dir.join(format!("{prefix}{shot}.png"));
            let mut taken = Err(String::new());
            // A window that has just changed its size or decorations can have no frame drawn
            // yet: asked to draw, and tried again.
            for _ in 0..4 {
                // Taken just after a frame is painted, when the window is laid out: between
                // frames a relayout can be pending, and a widget waiting for one draws nothing.
                taken = painted(&shell, &path, shot).await;
                if taken.is_ok() {
                    break;
                }
                wait(1.5).await;
            }
            match taken {
                Ok(()) => tracing::info!(path = %path.display(), "a shot"),
                Err(e) => tracing::error!(shot, error = %e, "a shot did not save"),
            }
            unstage(&shell, shot);
            wait(0.5).await;
        }
        shell.stop_and_close();
        if let Some(app) = shell.ctx.window.application() {
            app.quit();
        }
    });
}

/// Put the window in the state `shot` shows, and say how long to let it settle.
fn stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    let window = &shell.ctx.window;
    match shot {
        "home" => {
            shell.go(Dest::Continue);
            1.0
        }
        "welcome" => {
            crate::pages::first_launch::open(&shell.ctx);
            1.5
        }
        "offline" => {
            shell.ctx.hub.send(Command::NetworkPath {
                cellular: false,
                constrained: false,
                offline: true,
            });
            shell.go(Dest::Seasonal);
            2.0
        }
        "resumed" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(6), move || {
                    player.shot_resumed(724.0);
                });
            }
            7.5
        }
        "calendar" => {
            shell.go(Dest::Calendar);
            3.0
        }
        "seasonal" => {
            shell.go(Dest::Seasonal);
            2.5
        }
        "search" => {
            shell.go(Dest::Search);
            2.0
        }
        "search-results" => {
            shell.go(Dest::Search);
            shell.search_page().search_for("hoshikuzu");
            3.0
        }
        "title" => {
            shell.go(Dest::Continue);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            3.5
        }
        "show-settings" => {
            shell.go(Dest::Continue);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let ctx = Rc::clone(&shell.ctx);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                if let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) {
                    let _ = WidgetExt::activate_action(&page, "title.show-settings", None);
                }
            });
            5.0
        }
        "library" => {
            shell.go(Dest::Library);
            3.0
        }
        "downloads" => {
            shell.go(Dest::Downloads);
            2.0
        }
        "providers" => {
            shell.go(Dest::Providers);
            2.0
        }
        "provider" => {
            shell.go(Dest::Providers);
            crate::pages::plugin::push_provider(&shell.ctx, "mukashi-archive");
            1.5
        }
        "stores" => {
            shell.go(Dest::Stores);
            2.0
        }
        "store" => {
            shell.go(Dest::Stores);
            crate::pages::stores::push_store(&shell.ctx, "hotaru");
            1.5
        }
        "join" => {
            join(shell);
            2.0
        }
        "settings" => {
            shell.settings_pane("playback");
            1.5
        }
        "settings-general" => {
            shell.settings_pane("general");
            1.5
        }
        "settings-upscaling" => {
            shell.settings_pane("playback");
            shell.settings_upscaling();
            upscaling(shell, "natural");
            2.0
        }
        "settings-accounts" => {
            shell.settings_pane("accounts");
            1.5
        }
        "torrents-setup" => {
            shell.settings_pane("sources");
            crate::pages::torrents::setup(&shell.ctx, true, None, Rc::new(|| {}));
            1.5
        }
        "torrents-none" => {
            shell.settings_pane("sources");
            crate::pages::torrents::setup(
                &shell.ctx,
                true,
                Some(anistream_model::protection::TorrentProtection::None),
                Rc::new(|| {}),
            );
            if let Some(dialog) = shell.ctx.window.visible_dialog()
                && let Some(send) = default_button(&dialog)
            {
                send.emit_clicked();
            }
            1.5
        }
        "torrents-on" => {
            use anistream_core::settings::SettingValue;
            use anistream_model::protection::{FailedCheck, TorrentProtection};
            shell.ctx.hub.send(Command::SetSetting {
                key: anistream_model::settings::SettingKey::TorrentIndex,
                value: SettingValue::Str("https://index.example.test/rss?q={query}".into()),
            });
            // A proxy nothing listens on: the check fails on this machine, and never looks up
            // where the run's own traffic leaves from.
            shell.ctx.hub.send(Command::SetTorrentProtection {
                mode: TorrentProtection::Proxy,
                proxy: Some(anistream_model::protection::TorrentProxy {
                    host: "127.0.0.1".into(),
                    port: 9,
                    user: None,
                    password: None,
                }),
                on_failure: FailedCheck::Pause,
                acknowledged: false,
            });
            shell.ctx.hub.send(Command::SetSetting {
                key: anistream_model::settings::SettingKey::Torrents,
                value: SettingValue::Bool(true),
            });
            shell.settings_pane("sources");
            3.0
        }
        "settings-updates" => {
            use anistream_model::updates::{Release, UpdateStatus};
            shell.settings_pane("integrations");
            // As the daily check tells of a release with an AppImage for each machine: a scene
            // goes through the update's steps, fetching nothing.
            let appimage = |arch: &str| anistream_model::updates::ReleaseAsset {
                name: format!("anistream-2.1.0-linux-{arch}.AppImage"),
                url: format!(
                    "https://releases.example/v2.1.0/anistream-2.1.0-linux-{arch}.AppImage"
                ),
                sha256: "5e".repeat(32),
                size: 118_400_000,
            };
            shell.ctx.hub.take(vec![anistream_model::event::Event::Update(
                UpdateStatus::Available {
                    release: Box::new(Release {
                        version: "2.1.0".into(),
                        published_at: 0,
                        page_url: "https://example.test/releases/v2.1.0".into(),
                        notes: String::new(),
                        macos: None,
                        ios: None,
                        android: None,
                        android_tv: None,
                        windows_x64: None,
                        windows_arm64: None,
                        linux_x86_64: Some(appimage("x86_64")),
                        linux_aarch64: Some(appimage("aarch64")),
                    }),
                    notify: true,
                },
            )]);
            1.5
        }
        // The download rehearsed and checked: the row offers Restart to Update, and so does the
        // notice.
        "update-ready" => {
            shell.settings_pane("integrations");
            4.5
        }
        "accounts-kitsu" => {
            shell.settings_pane("accounts");
            crate::pages::accounts::kitsu(&shell.ctx, Rc::new(|| {}));
            1.5
        }
        "accounts-code" => {
            shell.ctx.hub.known.borrow_mut().signing.insert("simkl".into(), SignIn::Job(None));
            shell.settings_pane("accounts");
            let prompt = DeviceCodePrompt {
                tracker: "simkl".into(),
                code: "7KQ2M4".into(),
                url: "https://example.test/pin".into(),
                expires_at: Some(glib::real_time() / 1_000_000 + 14 * 60 + 59),
            };
            crate::pages::accounts::device_code(&shell.ctx, Some(&prompt));
            1.5
        }
        "about" => {
            let _ = WidgetExt::activate_action(window, "win.about", None);
            1.5
        }
        "player" => {
            play(shell);
            9.0
        }
        "player-marks" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_marks("/out/media/lantern-road.srt");
                });
                let player = Rc::clone(&shell.player);
                // The preview opens the stream the first time it shows and has its frame a
                // moment later; shown again then, so the controls are still up for the shot.
                glib::timeout_add_local_once(Duration::from_millis(6500), move || {
                    player.shot_preview(0.4);
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(8500), move || {
                    player.shot_preview(0.4);
                });
            }
            10.0
        }
        "player-tracks" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_tracks(Some("/out/media/lantern-road.srt"));
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(8000), move || {
                    player.shot_tracks(None);
                });
            }
            9.5
        }
        "player-undo" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(7500), move || {
                    player.shot_auto_skipped();
                });
            }
            9.0
        }
        "mini" => {
            play(shell);
            let player = Rc::clone(&shell.player);
            glib::timeout_add_local_once(Duration::from_secs(5), move || player.set_mini(true));
            9.0
        }
        "close" => {
            window.close();
            1.5
        }
        party if party.starts_with("party-") => party_stage(shell, party),
        other => {
            tracing::warn!(shot = other, "no such shot");
            0.0
        }
    }
}

/// Leave the state a shot put the window in.
fn unstage(shell: &Rc<Shell>, shot: &str) {
    match shot {
        "title" => {
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "about" | "join" | "welcome" | "accounts-kitsu" | "torrents-setup" => {
            close_dialogs(&shell.ctx.window)
        }
        "torrents-none" => {
            close_dialogs(&shell.ctx.window);
            close_dialogs(&shell.ctx.window);
        }
        "torrents-on" => shell.ctx.hub.send(Command::SetSetting {
            key: anistream_model::settings::SettingKey::Torrents,
            value: anistream_core::settings::SettingValue::Bool(false),
        }),
        "accounts-code" => {
            crate::pages::accounts::device_code(&shell.ctx, None);
            shell.ctx.hub.known.borrow_mut().signing.remove("simkl");
        }
        "offline" => shell.ctx.hub.send(Command::NetworkPath {
            cellular: false,
            constrained: false,
            offline: false,
        }),
        "resumed" => shell.player.close(),
        "show-settings" => {
            close_dialogs(&shell.ctx.window);
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "settings-upscaling" => {
            upscaling(shell, "off");
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        "provider" | "store" => {
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        "player-tracks" => {
            shell.player.close_menus();
            shell.player.close();
        }
        "player" | "player-marks" | "player-undo" | "mini" => shell.player.close(),
        party if party.starts_with("party-") => party_unstage(shell, party),
        _ => {}
    }
}

/// Set upscaling, which the fixture scene keeps off for the player shots.
fn upscaling(shell: &Rc<Shell>, mode: &str) {
    shell.ctx.hub.send(Command::SetSetting {
        key: anistream_model::settings::SettingKey::Upscaling,
        value: anistream_core::settings::SettingValue::Str(mode.into()),
    });
}

/// Play the player shots' episode, paused once it plays so the controls stay up.
fn play(shell: &Rc<Shell>) {
    let (id, episode) = PLAYING;
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.play",
        Some(&(id, episode.to_owned()).to_variant()),
    );
    let hub = Rc::clone(&shell.ctx.hub);
    glib::timeout_add_local_once(Duration::from_secs(7), move || {
        hub.send(Command::Player(anistream_model::command::PlayerCommand::TogglePause));
    });
}

/// The join dialog with an invite pasted, and the showcase's answer to checking it.
#[cfg(feature = "fixtures")]
fn join(shell: &Rc<Shell>) {
    crate::pages::join::open(&shell.ctx, Some(crate::showcase::INVITE));
    let hub = Rc::clone(&shell.ctx.hub);
    glib::timeout_add_local_once(Duration::from_millis(900), move || {
        hub.show_instead(vec![crate::showcase::invite()]);
    });
}

#[cfg(not(feature = "fixtures"))]
fn join(shell: &Rc<Shell>) {
    crate::pages::join::open(&shell.ctx, None);
}

fn close_dialogs(window: &adw::ApplicationWindow) {
    if let Some(dialog) = window.visible_dialog() {
        dialog.force_close();
    }
}

/// A dialog's default button: what Return presses.
fn default_button(dialog: &adw::Dialog) -> Option<gtk::Button> {
    dialog.default_widget()?.downcast().ok()
}

/// Draw the window, dialogs and all, into a PNG at its own scale.
fn capture(
    window: &adw::ApplicationWindow,
    path: &std::path::Path,
    over: Option<(gtk::Widget, f32, f32)>,
) -> Result<(), String> {
    let (width, height) = (window.width(), window.height());
    if width <= 0 || height <= 0 {
        return Err("the window has no size".into());
    }
    // The window on its own first: with a popover on top, an empty window would still give a
    // picture, of the popover alone.
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let drawn = gtk::Snapshot::new();
    paintable.snapshot(&drawn, f64::from(width), f64::from(height));
    let page = drawn.to_node();
    let page = page.ok_or_else(|| {
        format!(
            "the window drew nothing: {width}x{height}, mapped {}, decorated {}, needs {}x{}",
            window.is_mapped(),
            window.is_decorated(),
            window.measure(gtk::Orientation::Horizontal, -1).0,
            window.measure(gtk::Orientation::Vertical, -1).0,
        )
    })?;
    let snapshot = gtk::Snapshot::new();
    snapshot.append_node(&page);
    // A popover is a surface of its own, which the window's drawing leaves out: drawn on top.
    if let Some((widget, x, y)) = over {
        let popover = gtk::WidgetPaintable::new(Some(&widget));
        snapshot.save();
        snapshot.translate(&gtk::graphene::Point::new(x, y));
        popover.snapshot(&snapshot, f64::from(widget.width()), f64::from(widget.height()));
        snapshot.restore();
    }
    let node = snapshot.to_node().ok_or("nothing drawn")?;
    let renderer = window.renderer().ok_or("no renderer")?;
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&bounds));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    texture.save_to_png(path).map_err(|e| e.to_string())
}

/// The window drawn into `path` just after its next frame is painted, with the shot's popover
/// on top; captured at once if no frame comes within a second.
async fn painted(shell: &Rc<Shell>, path: &std::path::Path, shot: &str) -> Result<(), String> {
    let window = shell.ctx.window.clone();
    let take = {
        let (player, window, path) =
            (Rc::clone(&shell.player), window.clone(), path.to_owned());
        let shot = shot.to_owned();
        move || {
            let over = match shot.as_str() {
                "player-marks" => player.shot_popover(),
                "player-tracks" => player.shot_tracks_popover(),
                "party-title-menu" => title_menu_popover(&window),
                party if party.starts_with("party-") => player.shot_party_popover(),
                _ => None,
            };
            capture(&window, &path, over)
        }
    };
    let Some(clock) = window.frame_clock() else { return take() };
    let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
    let once = std::cell::Cell::new(Some(take.clone()));
    let handler = clock.connect_after_paint(move |_| {
        // Drawn outside the frame, so the capture's own rendering does not run inside the
        // window's, but before anything else on the loop can ask for a new layout.
        if let Some(take) = once.take() {
            let tx = tx.clone();
            glib::idle_add_local_full(glib::Priority::HIGH, move || {
                let _ = tx.try_send(take());
                glib::ControlFlow::Break
            });
        }
    });
    window.queue_draw();
    let taken = glib::future_with_timeout(Duration::from_secs(1), rx.recv()).await;
    clock.disconnect(handler);
    match taken {
        Ok(Ok(taken)) => taken,
        _ => take(),
    }
}

async fn wait(seconds: f64) {
    glib::timeout_future(Duration::from_secs_f64(seconds)).await;
}

/// The shots asked for: a comma-separated list, or every one.
pub fn named(list: Option<&str>) -> Vec<String> {
    match list {
        None | Some("all") => ALL.iter().map(|s| (*s).to_owned()).collect(),
        Some(list) => list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .flat_map(|s| match s {
                "party" => PARTY.iter().map(|p| (*p).to_owned()).collect(),
                one => vec![one.to_owned()],
            })
            .collect(),
    }
}

/// Warn of each widget whose minimum width is over `limit`, the narrowest of a branch that is:
/// what keeps a window from fitting its size.
fn wide_widgets(widget: &gtk::Widget, depth: usize, limit: i32) {
    let (min, ..) = widget.measure(gtk::Orientation::Horizontal, -1);
    if min <= limit {
        return;
    }
    let mut over = false;
    let mut child = widget.first_child();
    while let Some(c) = child {
        if c.measure(gtk::Orientation::Horizontal, -1).0 > limit {
            over = true;
        }
        wide_widgets(&c, depth + 1, limit);
        child = c.next_sibling();
    }
    if !over {
        tracing::warn!(depth, min, kind = %widget.type_(), name = %widget.widget_name(), css = ?widget.css_classes(), "too wide");
    }
}

/// The room the party shots hold their party in.
#[cfg(feature = "fixtures")]
const ROOM: &str = "quiet-harbor-k7m2xq9ad4tnwe3p";

#[cfg(feature = "fixtures")]
thread_local! {
    /// The party shot's server, kept until the shot is taken.
    static PARTY_SERVER: std::cell::RefCell<Option<crate::fixtures::FakeParty>> =
        const { std::cell::RefCell::new(None) };
    /// The address the joining shot waits on, which never answers.
    static SILENT: std::cell::RefCell<Option<std::net::TcpListener>> =
        const { std::cell::RefCell::new(None) };
}

/// Put the window in the party state `shot` shows, and say how long to let it settle.
#[cfg(feature = "fixtures")]
fn party_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::{
        fixtures::FakeParty,
        party::{form, source},
    };
    use anistream_core::ids::AnilistId;
    use anistream_model::party::{NoSourceWhy, PartyInvite};

    let ctx = Rc::clone(&shell.ctx);
    let title = AnilistId::new(TITLE);
    let open_title = || {
        shell.go(Dest::Continue);
        let _ = WidgetExt::activate_action(
            &shell.ctx.window,
            "win.open-title",
            Some(&TITLE.to_variant()),
        );
    };
    let serve = |refuse: Option<&str>| {
        let party = FakeParty::start(refuse)?;
        let server = party.server();
        PARTY_SERVER.with(|p| p.replace(Some(party)));
        Some(server)
    };
    // Each scene's own notices, not the last one's still queued.
    shell.ctx.toasts.dismiss_all();
    match shot {
        "party-form" => {
            open_title();
            let ctx = Rc::clone(&ctx);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                form::open(&ctx, form::For::Episode(title, "7".into()));
            });
            3.5
        }
        "party-title-menu" => {
            open_title();
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                if let Some(more) = more_button(&window) {
                    more.popup();
                }
            });
            3.5
        }
        "party-joining" | "party-refused" => {
            let server = if shot == "party-joining" {
                crate::fixtures::silent_server().map(|(listener, address)| {
                    SILENT.with(|s| s.replace(Some(listener)));
                    address
                })
            } else {
                serve(Some("this room takes members by invitation only"))
            };
            open_title();
            let ctx = Rc::clone(&ctx);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                form::open(&ctx, form::For::Episode(title, "7".into()));
                form::rehearse(server, true);
            });
            4.5
        }
        "party-invite" | "party-unaired" => {
            let Some(server) = serve(None) else { return 0.0 };
            let (id, episode) =
                if shot == "party-invite" { (TITLE, "7") } else { (PLAYING.0, "6") };
            let link = PartyInvite {
                server,
                room: ROOM.into(),
                id: AnilistId::new(id),
                episode: episode.into(),
            }
            .link();
            shell.go(Dest::Continue);
            shell.open_link(&link);
            if shot == "party-unaired" {
                glib::timeout_add_local_once(Duration::from_millis(2500), || {
                    form::rehearse(None, true);
                });
                return 7.0;
            }
            3.5
        }
        "party-nosource" | "party-nosources" | "party-choose" => {
            let Some(server) = serve(None) else { return 0.0 };
            ctx.hub.send(Command::PartyJoin {
                server,
                room: ROOM.into(),
                name: Some("Ada".into()),
            });
            let ctx = Rc::clone(&ctx);
            let shot = shot.to_owned();
            glib::spawn_future_local(async move {
                for _ in 0..50 {
                    if ctx.hub.known.borrow().party.is_some() {
                        break;
                    }
                    wait(0.2).await;
                }
                match shot.as_str() {
                    "party-nosources" => source::open(
                        &ctx,
                        title,
                        "7",
                        NoSourceWhy::NoSources,
                        "no source is set up",
                    ),
                    "party-nosource" => source::open(
                        &ctx,
                        title,
                        "7",
                        NoSourceWhy::NotFound,
                        "could not match this title: sample-source has no episode 7",
                    ),
                    _ => {
                        source::rehearse_choose(&ctx, title, "7");
                        // Before the engine's own answer, which has no sources to give.
                        ctx.hub.take(vec![anistream_model::event::Event::Sources {
                            id: title,
                            episode: "7".into(),
                            candidates: choices(),
                        }]);
                    }
                }
            });
            4.0
        }
        playing => {
            let Some(server) = serve(None) else { return 0.0 };
            party_playing(shell, server, playing.to_owned());
            match playing {
                "party-lost" => 18.0,
                "party-left" | "party-ready" => 16.0,
                _ => 14.0,
            }
        }
    }
}

/// A party playing the title's episode 7, held paused at the start as a room waiting to be
/// ready is, with Mika ready and Ren not, then the state `shot` shows.
#[cfg(feature = "fixtures")]
fn party_playing(shell: &Rc<Shell>, server: String, shot: String) {
    use anistream_core::ids::AnilistId;
    use anistream_model::command::PlayerCommand;
    let shell = Rc::clone(shell);
    shell.player.hold_up_next.set(true);
    let started = std::time::Instant::now();
    // Until `at` seconds into the scene, so a notice is still up when the shot is taken.
    let until = move |at: f64| wait((at - started.elapsed().as_secs_f64()).max(0.0));
    glib::spawn_future_local(async move {
        let hub = Rc::clone(&shell.ctx.hub);
        hub.send(Command::PartyJoin { server, room: ROOM.into(), name: Some("Ada".into()) });
        hub.send(Command::Play {
            id: AnilistId::new(TITLE),
            episode: "7".into(),
            from_start: true,
        });
        // Until this device's member has the episode open, for the others to open the same.
        let mut you = None;
        for _ in 0..100 {
            wait(0.2).await;
            you = hub
                .known
                .borrow()
                .party_members
                .as_ref()
                .and_then(|m| m.iter().find(|m| m.you).cloned());
            if you.as_ref().is_some_and(|y| y.file.is_some() && y.duration.is_some()) {
                break;
            }
        }
        let file = you.as_ref().and_then(|y| y.file.clone()).unwrap_or_default();
        let length = you.and_then(|y| y.duration).unwrap_or(60.0);
        hub.send(Command::Player(PlayerCommand::TogglePause));
        PARTY_SERVER.with(|p| {
            let p = p.borrow();
            let Some(party) = p.as_ref() else { return };
            party.add(ROOM, "Mika");
            party.opens("Mika", &file, length);
            party.add(ROOM, "Ren");
            let other = file.replace("Episode 7", "Episode 8");
            party.opens("Ren", if shot == "party-panel" { &other } else { &file }, length);
            party.ready("Mika", true);
            party.ready("Ren", false);
        });
        wait(0.8).await;
        // The joins said, so the shot shows its own notice.
        shell.ctx.toasts.dismiss_all();
        let party = |act: &dyn Fn(&crate::fixtures::FakeParty)| {
            PARTY_SERVER.with(|p| {
                if let Some(party) = p.borrow().as_ref() {
                    act(party);
                }
            });
        };
        match shot.as_str() {
            "party-panel" => shell.player.show_party(),
            "party-ready" => {
                party(&|p| p.ready("Ren", true));
                wait(0.4).await;
                hub.send(Command::PartyReady { ready: true });
                wait(0.6).await;
                shell.player.show_party();
            }
            "party-chat" => {
                until(11.5).await;
                party(&|p| p.says("Mika", "Starting in a minute, grabbing tea"));
            }
            "party-left" => {
                until(11.5).await;
                shell.player.show_party();
                until(13.0).await;
                hub.send(Command::PartyLeave);
            }
            "party-lost" => {
                party(&|p| p.go_down(true));
                wait(2.5).await;
                shell.player.show_party();
            }
            "party-close" => shell.ctx.window.close(),
            _ => {}
        }
    });
}

/// Leave the party state a shot put the window in.
#[cfg(feature = "fixtures")]
fn party_unstage(shell: &Rc<Shell>, shot: &str) {
    shell.player.close_menus();
    if let Some(more) = more_button(&shell.ctx.window) {
        more.popdown();
    }
    // Twice: the alert, and the dialog it opened.
    close_dialogs(&shell.ctx.window);
    close_dialogs(&shell.ctx.window);
    shell.ctx.hub.send(Command::PartyLeave);
    if shell.is_player() {
        shell.player.close();
    }
    if !matches!(shot, "party-nosource" | "party-nosources" | "party-choose") {
        let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
    }
    shell.player.hold_up_next.set(false);
    shell.go(Dest::Continue);
    let toasts = shell.ctx.toasts.clone();
    glib::timeout_add_local_once(Duration::from_millis(400), move || {
        PARTY_SERVER.with(|p| p.replace(None));
        SILENT.with(|s| s.replace(None));
        toasts.dismiss_all();
    });
}

/// What the sources list for the choose shot's episode.
#[cfg(feature = "fixtures")]
fn choices() -> Vec<anistream_core::traits::SourceCandidate> {
    use anistream_core::traits::SourceCandidate;
    let choice =
        |id: &str, title: &str, quality, size: &str, seeders, auto_pick| SourceCandidate {
            id: id.into(),
            provider_id: "sample-source".into(),
            title: title.into(),
            quality: Some(quality),
            seeders: Some(seeders),
            size: Some(size.into()),
            dual_audio: false,
            dubbed: false,
            auto_pick,
        };
    vec![
        choice("a", "Stardust Wayfarer - 07 (1080p)", 1080, "1.4 GiB", 42, true),
        choice("b", "Stardust Wayfarer - 07 (720p)", 720, "690 MiB", 18, false),
        choice("c", "Stardust Wayfarer - 07 (480p)", 480, "310 MiB", 5, false),
    ]
}

#[cfg(not(feature = "fixtures"))]
fn party_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn party_unstage(_: &Rc<Shell>, _: &str) {}

/// The title page's More button, on the page on screen.
fn more_button(window: &adw::ApplicationWindow) -> Option<gtk::MenuButton> {
    fn find(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && button.tooltip_text().as_deref() == Some("More")
            && button.is_mapped()
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    find(window.upcast_ref())
}

/// The More menu's popover, under its button, over the window.
fn title_menu_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    let button = more_button(window)?;
    let popover = button.popover()?;
    if !popover.is_visible() {
        return None;
    }
    let bounds = button.compute_bounds(window)?;
    let width = popover.width() as f32;
    let x = (bounds.x() + bounds.width() / 2.0 - width / 2.0)
        .clamp(0.0, (window.width() as f32 - width).max(0.0));
    Some((popover.upcast(), x, bounds.y() + bounds.height()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_are_named_or_all() {
        assert_eq!(named(Some("home, title")), ["home", "title"]);
        assert_eq!(named(None).len(), ALL.len());
        assert_eq!(named(Some("home,party")).len(), PARTY.len() + 1);
    }
}
