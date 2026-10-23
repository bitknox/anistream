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
pub const ALL: [&str; 12] = [
    "home",
    "calendar",
    "seasonal",
    "search",
    "search-results",
    "title",
    "library",
    "settings",
    "settings-accounts",
    "about",
    "player",
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
            match capture(&shell.ctx.window, &path) {
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
        "library" => {
            shell.go(Dest::Library);
            3.0
        }
        "settings" => {
            shell.settings_pane("playback");
            1.5
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
            let (id, episode) = PLAYING;
            let _ = WidgetExt::activate_action(
                window,
                "win.play",
                Some(&(id, episode.to_owned()).to_variant()),
            );
            // Paused once it plays, so the controls stay up.
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_secs(7), move || {
                hub.send(Command::Player(anistream_model::command::PlayerCommand::TogglePause));
            });
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
        "about" => close_dialogs(&shell.ctx.window),
        "player" => shell.player.close(),
        _ => {}
    }
}

fn close_dialogs(window: &adw::ApplicationWindow) {
    if let Some(dialog) = window.visible_dialog() {
        dialog.force_close();
    }
}

/// Draw the window, dialogs and all, into a PNG at its own scale.
fn capture(window: &adw::ApplicationWindow, path: &std::path::Path) -> Result<(), String> {
    let (width, height) = (window.width(), window.height());
    if width <= 0 || height <= 0 {
        return Err("the window has no size".into());
    }
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let node = snapshot.to_node().ok_or("nothing drawn")?;
    let renderer = window.renderer().ok_or("no renderer")?;
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&bounds));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    texture.save_to_png(path).map_err(|e| e.to_string())
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
