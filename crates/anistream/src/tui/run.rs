//! The interface's event loop: the terminal made ready, the engine started, and every event,
//! key, signal and tick handled until the app quits and the terminal is given back.

use std::{io, sync::Arc, time::Duration};

use anistream_core::config::{Config, Paths};
use anistream_engine::{engine::Engine, host::DesktopHost, options::EngineOptions};
use anistream_model::{command::Command, event::Event, notice::Notice};
use anistream_net::HttpClient;
use anistream_store::Store;
use anistream_tui::{
    app::App,
    keymap::Keymap,
    screens,
    theme::{self, player_theme},
};
use anyhow::{Context, Result};
use crossterm::{
    event::{Event as TermEvent, EventStream, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

use crate::{
    covers,
    tui::{
        self,
        clipboard::copy_to_clipboard,
        seams::{ConfigSeam, command, use_config},
    },
};

pub async fn run(
    config: Config,
    paths: Paths,
    http: HttpClient,
    store: Store,
    images: bool,
    config_problem: Option<anistream_core::config::ConfigProblem>,
) -> Result<()> {
    let palette = theme::resolve(config.theme.mode);

    let mut keymap = Keymap::new();
    let keybinding_problems = keymap.apply_overrides(&config.keys);
    for problem in &keybinding_problems {
        eprintln!("warning: keybinding: {problem}");
    }

    let image_engine =
        anistream_tui::image::ImageEngine::detect(images).with_cache_dir(paths.image_cache());
    tracing::info!(graphics = ?image_engine.graphics(), "image engine ready");

    // The engine, started as the app launches it: its source chain built behind the interface,
    // and its own work beside the commands. The player is dressed in the immersive palette
    // whatever the terminal is: see `player_theme`.
    let options =
        EngineOptions::new(paths.clone(), config.clone(), http, store, player_theme());
    let (engine, mut from_engine) = Engine::start(options, Arc::new(DesktopHost)).await?;

    let mut app = App::with_images(config, palette, keymap, image_engine);
    // Said inside the app too: the warning above scrolls away behind the alternate screen
    // before anyone reads it, and a refused rebind otherwise looks like a key that does nothing.
    for problem in keybinding_problems {
        tracing::warn!(%problem, "keybinding not applied");
        app.push_toast(Notice::alert(format!("keybinding not applied: {problem}")));
    }

    // What the file says, as distinct from what is in effect: `--theme` changes only this
    // session, and the Settings screen has to be able to tell the two apart.
    app.saved_config = Config::load(&paths).unwrap_or_default();
    if let Some(problem) = &config_problem {
        show_config_problem(&mut app, &paths, problem);
    }

    // The engine told the interface's config, the file's included, before it reads it.
    let mut seam = ConfigSeam::default();
    use_config(&app, &mut seam, &engine);

    // Both before the terminal changes state, so there is no moment in which a signal or a
    // panic could leave it raw.
    tui::terminal::install_panic_hook();
    let mut signals = tui::terminal::Signals::new().context("listening for signals")?;
    let mut terminal = setup_terminal().context("preparing the terminal")?;

    if let Some(task) = app.reload() {
        engine.send(task);
    }

    // What was true at launch, applied before the first frame so that it shows the app as it
    // was launched: the joined stores, the trackers' states, and what the check of the player
    // found, which the first frame has always waited for. Nothing the engine has said since
    // is in it; that is read in the loop, after the first frame, as it always was.
    for update in from_engine.launch().await {
        app.apply(update);
    }

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    // Covers come back from the blocking pool on this, decoded, to be handed to the app.
    let (covers_tx, mut covers_rx) = mpsc::unbounded_channel();

    // Painting on change rather than on every tick: see `Repaint`. `idle_tick` is whether
    // nothing but the ticker woke this pass, the one case that may skip it.
    let mut repaint = tui::terminal::Repaint::default();
    let mut idle_tick = false;
    let result = loop {
        let now = std::time::Instant::now();
        // Work on screen keeps its clocks and its pulse moving, the eyecatch its wipe.
        let live = !app.jobs.is_empty() || app.is_animating();
        if !idle_tick || repaint.due(live, now) {
            // Broken out of rather than returned from: a terminal that can no longer be drawn
            // on still has to be given back, and the player closed.
            if let Err(e) = terminal.draw(|frame| screens::render(frame, &app)) {
                break Err(e.into());
            }
            repaint.painted(terminal.backend_mut().take_wrote(), now);
        }
        idle_tick = false;
        if app.should_quit {
            break Ok(());
        }

        let size = match terminal.size() {
            Ok(size) => size,
            Err(e) => break Err(e.into()),
        };
        let rows = screens::visible_rows(size.into(), app.nav.section());

        // Prefetch only what is on screen plus a short lookahead. `visible_artwork` claims
        // each URL, so a re-render cannot re-request one already in flight.
        for url in app.visible_artwork(rows) {
            let task = Command::FetchArtwork { url };
            command(&app, &mut seam, task, &engine);
        }

        // Auto-next is queued by the reducer rather than returned, because an episode ending
        // arrives as an update rather than a keystroke.
        if let Some(task) = app.take_pending() {
            command(&app, &mut seam, task, &engine);
        }
        // Whatever moved the config since: a setting put back, say. What the engine does
        // with the events it reads next reads its copy.
        use_config(&app, &mut seam, &engine);

        tokio::select! {
            // One at a time, each in the order the engine applied it, with the loop's own
            // reactions and the next frame between them, as when the loop read the work itself.
            Some(update) = from_engine.recv() => {
                // Handed to the terminal's clipboard on the way in, so the overlay can say
                // whether it was.
                let copied = match &update {
                    Event::InviteLink(view) => {
                        copy_to_clipboard(terminal.backend_mut(), &view.link.0)
                    }
                    _ => false,
                };
                let ended = matches!(update, Event::PlaybackEnded { .. });
                match update {
                    // Decoded off the loop; it comes back on the arm below.
                    Event::ArtworkReady { url, bytes } => covers::spawn(url, bytes, &covers_tx),
                    update => app.apply(update),
                }
                if copied {
                    app.invite_copied = true;
                }
                // mpv's window is gone; unless the reducer queued the next episode — in
                // which case another window is about to open — the viewer's next keypress
                // belongs to the terminal, so ask for focus back rather than leaving it
                // wherever the window manager dropped it.
                if ended && !app.will_play_next() && app.playing.is_none() {
                    tui::focus::refocus_terminal();
                }
            }
            Some(decoded) = covers_rx.recv() => covers::hand_over(&mut app, decoded),
            // Asked to end from outside — a kill, the terminal closing — which leaves the way
            // Q does, so the player is closed rather than orphaned and playing on.
            signal = signals.recv() => {
                tracing::info!(signal, "shutting down");
                tui::terminal::exit_on_second_signal();
                if let Some(task) = app.handle(anistream_tui::keymap::Action::Quit, rows) {
                    command(&app, &mut seam, task, &engine);
                }
                // Whatever an open overlay made of the key, a signal is not a request to close it.
                app.should_quit = true;
            }
            _ = ticker.tick() => {
                // A toast going is a change; one counting down is not.
                let toasts = app.toasts.len();
                app.tick_toasts();
                idle_tick = app.toasts.len() == toasts;
            }
            // The eyecatch is the only thing in this app that animates, so the fast tick only
            // exists while one is running rather than burning a frame budget all the time.
            _ = tokio::time::sleep(Duration::from_millis(anistream_tui::eyecatch::FRAME_MS)),
                if app.is_animating() =>
            {
                app.tick_animation();
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(TermEvent::Key(key))) if key.kind == KeyEventKind::Press => {
                        if let Some(task) = app.press(key, rows) {
                            command(&app, &mut seam, task, &engine);
                        }
                    }
                    // Arrives whole with bracketed paste on, so a pasted link cannot fire
                    // bindings on its way into a field.
                    Some(Ok(TermEvent::Paste(text))) => app.paste(&text),
                    // The next frame redraws at the new size on its own; the list's window does
                    // not follow by itself, and a shorter terminal would otherwise leave the
                    // highlighted row below the bottom edge.
                    Some(Ok(TermEvent::Resize(width, height))) => {
                        let area = ratatui::layout::Rect::new(0, 0, width, height);
                        app.fit_to(screens::visible_rows(area, app.nav.section()));
                    }
                    Some(Err(e)) => break Err(anyhow::anyhow!(e)),
                    None => break Ok(()),
                    _ => {}
                }
            }
        }
    };

    // A playback that has started, as opposed to one still finding its source, is stopped and
    // given a moment to record where it was left.
    let under_way = app.playing.as_ref().is_some_and(|playing| playing.duration.is_some());
    engine.shutdown(under_way).await;
    restore_terminal(&mut terminal)?;
    result
}

/// Say that the config did not load, on the status line for as long as the session runs and in
/// the log. The interface and its previews both, so a preview shows the frame a person would see.
pub fn show_config_problem(
    app: &mut App,
    paths: &Paths,
    problem: &anistream_core::config::ConfigProblem,
) {
    app.notice = Some(config_notice(&paths.config_file, problem));
    app.log(
        anistream_model::notice::NoticeKind::Alert,
        format!("{}, {problem} — using the defaults", paths.config_file.display()),
    );
}

/// The interface's standing notice for a config file that did not load: which file, which line,
/// what is wrong there, and what that means for the session.
///
/// One line, so the parser's list of every key it would have accepted is left to the log.
fn config_notice(
    file: &std::path::Path,
    problem: &anistream_core::config::ConfigProblem,
) -> String {
    let name = file
        .file_name()
        .map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned());
    let what = problem.message.split(", expected").next().unwrap_or(&problem.message);
    let place = problem.line.map_or_else(String::new, |line| format!(" line {line}"));
    format!("{name}{place}: {what} — using the defaults until it is fixed")
}

/// The terminal the interface draws on.
type Tui = Terminal<tui::terminal::Quiet<CrosstermBackend<io::Stdout>>>;

fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    // Best effort: the legacy Windows console has no bracketed paste and says so with an error,
    // and a paste arriving as keystrokes is a lesser thing than the app not starting.
    let _ = execute!(stdout, crossterm::event::EnableBracketedPaste);
    let terminal = Terminal::new(tui::terminal::Quiet::new(CrosstermBackend::new(stdout)))?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    Ok(tui::terminal::restore(terminal.backend_mut())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_config_is_named_by_file_and_line_on_one_line() {
        let file = std::path::Path::new("/home/someone/.config/anistream/config.toml");
        let problem = Config::parse("[playback]\nqualty = 720\n").unwrap_err();
        assert_eq!(
            config_notice(file, &problem),
            "config.toml line 2: unknown field `qualty` — using the defaults until it is fixed"
        );
        let rule = Config::parse("[playback]\ncommit_threshold = 3.0\n").unwrap_err();
        let notice = config_notice(file, &rule);
        assert!(notice.starts_with("config.toml: playback.commit_threshold"), "{notice}");
    }
}
