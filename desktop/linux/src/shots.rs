//! Screenshots of the window, taken by the app itself: each shot puts the window in a state,
//! waits for the engine and the art to settle, and draws the window into a PNG. Run under the
//! fixture scene, so every run draws the same titles.
//!
//!   anistream-desktop --fixtures --shots <dir> --shot home,title,... [--size 1280x800]

use std::{path::PathBuf, rc::Rc, time::Duration};

use adw::prelude::*;
use anistream_model::command::Command;
use gtk::glib;

use crate::{pages::Dest, window::Shell};

/// The title the title shots open: the fixture scene's finished title, part-way through
/// episode 7.
const TITLE: u32 = 900_001;
/// The title the player shot plays: episode 4 is the next, from its start.
const PLAYING: (u32, &str) = (900_002, "4");

/// Every shot by name, in the order a run takes them.
pub const ALL: [&str; 25] = [
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
    "settings-upscaling",
    "settings-accounts",
    "about",
    "player",
    "player-marks",
    "mini",
    "resumed",
    "close",
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
                let marks = shot == "player-marks";
                taken = painted(&shell, &path, marks).await;
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
        "about" | "join" | "welcome" => close_dialogs(&shell.ctx.window),
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
        "player" | "player-marks" | "mini" => shell.player.close(),
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

/// The window drawn into `path` just after its next frame is painted, with the seek preview on
/// top when `marks`; captured at once if no frame comes within a second.
async fn painted(shell: &Rc<Shell>, path: &std::path::Path, marks: bool) -> Result<(), String> {
    let window = shell.ctx.window.clone();
    let take = {
        let (player, window, path) =
            (Rc::clone(&shell.player), window.clone(), path.to_owned());
        move || {
            let over = if marks { player.shot_popover() } else { None };
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
            .map(str::to_owned)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_are_named_or_all() {
        assert_eq!(named(Some("home, title")), ["home", "title"]);
        assert_eq!(named(None).len(), ALL.len());
    }
}
