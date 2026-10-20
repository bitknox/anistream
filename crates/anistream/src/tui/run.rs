//! The interface's event loop: the terminal made ready, the engine side's work started, and
//! every event, key, signal and tick handled until the app quits and the terminal is given back.

use std::{io, sync::Arc, time::Duration};

use anistream_core::config::{Config, Paths};
use anistream_engine::{
    background::datasets::keep_datasets_current,
    dispatch::{handle_internal, receive},
    host::DesktopHost,
    outbox::{Envelope, Internal, Outbox},
    playback::player::{launch_notice, player},
    services::Services,
    sources,
    state::EngineState,
    tracking,
};
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
        seams::{ConfigSeam, command, route, use_config},
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

    // Old logs, stale covers and the player's leftovers, off the critical path.
    {
        let paths = paths.clone();
        tokio::task::spawn_blocking(move || {
            anistream_engine::background::prune::sweep_at_launch(&paths)
        });
    }
    // With torrents off nothing else would ever prune what an earlier run streamed. With them
    // on, the session prunes on its way up, before it writes anything.
    if !config.providers.torrent.enabled {
        let (dir, limit) =
            (paths.cache_dir.join("torrents"), config.providers.torrent.cache_limit_bytes());
        let store = store.clone();
        tokio::spawn(async move {
            anistream_engine::background::prune::sweep_torrent_cache(
                dir,
                limit,
                &store,
                Default::default(),
            )
            .await;
        });
    }

    let image_engine =
        anistream_tui::image::ImageEngine::detect(images).with_cache_dir(paths.image_cache());
    tracing::info!(graphics = ?image_engine.graphics(), "image engine ready");
    // Without the torrent source, which comes up behind the interface once the VPN guard
    // passes: see `TorrentTasks::bring_up`.
    let registry = sources::build_registry_deferred(&config, &http);
    tracing::info!(providers = ?registry.ids(), "provider registry ready");
    // Plugins join the chain when they have finished compiling. Nothing waits on this: the first
    // frame is worth more than a source you have not asked for yet.
    let plugins_loading = sources::spawn_plugin_load(
        registry.clone(),
        config.clone(),
        http.clone(),
        paths.clone(),
    );
    // What every task is lent, built around that chain.
    let host = Arc::new(DesktopHost);
    let services = Services::build(&config, paths, http, store, registry, player_theme(), host)
        .map_err(anyhow::Error::msg)?;
    // Where the torrent source reports in once it is up, tagged with the bring-up it answers.
    let (arrivals_tx, mut arrivals) = mpsc::unbounded_channel();
    // The effect layer's own state, on the config the app was launched with.
    let mut engine = EngineState::new(config.clone(), arrivals_tx);
    let mut app = App::with_images(config, palette, keymap, image_engine);
    // Said inside the app too: the warning above scrolls away behind the alternate screen
    // before anyone reads it, and a refused rebind otherwise looks like a key that does nothing.
    for problem in keybinding_problems {
        tracing::warn!(%problem, "keybinding not applied");
        app.push_toast(Notice::alert(format!("keybinding not applied: {problem}")));
    }

    // What the file says, as distinct from what is in effect: `--theme` changes only this
    // session, and the Settings screen has to be able to tell the two apart.
    app.saved_config = Config::load(&services.paths).unwrap_or_default();
    if let Some(problem) = &config_problem {
        show_config_problem(&mut app, &services.paths, problem);
    }
    // Joined stores as they were left, so Providers can count them on the first frame.
    let joined = match services.stores.rows() {
        Ok(rows) => {
            let joined = !rows.is_empty();
            app.apply(Event::Stores(rows));
            joined
        }
        Err(e) => {
            app.apply(Event::Toast(Notice::alert(e)));
            false
        }
    };

    // Background work reports through this channel; nothing off the UI thread ever touches
    // `App` directly, which is what keeps a slow request from stalling a frame. The engine
    // side's messages to itself ride it too, each in its place among the events.
    let (tx, mut rx) = Outbox::inbox();

    // The engine side told the interface's config, the file's included, before it reads it.
    let mut seam = ConfigSeam::default();
    use_config(&app, &mut seam, &mut |c| route(c, &mut engine, &services, &tx));

    // The daily update check: one cached request, one quiet toast when a release exists.
    anistream_engine::updates::spawn_check(
        engine.config.updates.check,
        &services.paths.cache_dir,
        &services.http,
        &tx,
    );

    // The airing digest: what aired for shows you watch since the last launch. Same
    // philosophy as the update check — one line, only when there is something to say.
    anistream_engine::background::digest::spawn(
        engine.config.notifications.airing_digest,
        engine.config.notifications.desktop,
        services.store.clone(),
        services.meta.clone(),
        services.host.clone(),
        tx.clone(),
    );

    // Report whatever the background plugin load concluded, once. A plugin that failed to load
    // must not be silent — the Providers screen is where a missing source gets explained.
    {
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Ok(outcome) = plugins_loading.await {
                match outcome {
                    Some(reason) => {
                        let _ = tx.send(Event::ProviderNote(reason));
                    }
                    // Ranked again by the order as it stands now, since the user may have
                    // moved a source while these compiled, and the Providers screen refreshed
                    // so late arrivals actually appear on it.
                    None => {
                        let _ = tx.internal(Internal::SourcesReordered);
                    }
                }
            }
        });
    }

    // Each joined store asked for its current list, once, in the background. Updates are said
    // once at launch, the way a new release is, and never installed without being asked. Asked
    // of the rows read above, which the interface was given whole.
    if joined {
        let (stores, tx) = (services.stores.clone(), tx.clone());
        tokio::spawn(async move {
            if let Ok(rows) = stores.refresh().await {
                let _ = tx.send(Event::Stores(rows));
            }
            let updates = stores.pending_updates();
            if !updates.is_empty() {
                let _ = tx.send(Event::Toast(Notice::info(format!(
                    "plugin {} ready: {} — Providers, Enter",
                    if updates.len() == 1 { "update" } else { "updates" },
                    updates.join(", ")
                ))));
            }
        });
    }

    for state in services.sync.initial_states() {
        app.apply(Event::Sync(Box::new(state)));
    }
    tracking::spawn_loops(services.sync.clone(), tx.clone());

    // Keep the mapping datasets current without ever blocking startup.
    tokio::spawn(keep_datasets_current(
        services.store.clone(),
        services.http.clone(),
        tx.clone(),
    ));

    // The torrent stack's live parts, held so a settings change can tear them down and
    // stand them back up without a restart. Both are replaced together: the download
    // manager and the guard's watcher are the two things holding a session alive, and
    // leaving either behind would mean traffic outliving the source it belongs to.
    engine.torrent_tasks.bring_up(&engine.config, &services, &tx, &engine.arrivals);

    // Both before the terminal changes state, so there is no moment in which a signal or a
    // panic could leave it raw.
    tui::terminal::install_panic_hook();
    let mut signals = tui::terminal::Signals::new().context("listening for signals")?;
    let mut terminal = setup_terminal().context("preparing the terminal")?;

    if let Some(task) = app.reload() {
        route(task, &mut engine, &services, &tx);
    }

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    // Covers come back from the blocking pool on this, decoded, to be handed to the app.
    let (covers_tx, mut covers_rx) = mpsc::unbounded_channel();

    let mpv = player(&engine.config, &services.paths, &player_theme());
    if let Some(notice) = launch_notice(&mpv, &engine.config.playback.mpv_binary).await {
        app.apply(Event::Toast(notice));
    }

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
            command(&app, &mut seam, task, &mut |c| route(c, &mut engine, &services, &tx));
        }

        // Auto-next is queued by the reducer rather than returned, because an episode ending
        // arrives as an update rather than a keystroke.
        if let Some(task) = app.take_pending() {
            command(&app, &mut seam, task, &mut |c| route(c, &mut engine, &services, &tx));
        }
        // Whatever moved the config since: a setting put back, say. The reactions below read
        // the engine side's copy.
        use_config(&app, &mut seam, &mut |c| route(c, &mut engine, &services, &tx));

        tokio::select! {
            Some(envelope) = rx.recv() => {
                let update = match envelope {
                    Envelope::Event(update) => update,
                    // What the engine side left itself to do, after everything sent before it
                    // and before anything sent after.
                    Envelope::Internal(message) => {
                        for said in handle_internal(message, &mut engine, &services, &tx).await {
                            app.apply(said);
                        }
                        continue;
                    }
                };
                // Handed to the terminal's clipboard on the way in, so the overlay can say
                // whether it was.
                let copied = match &update {
                    Event::InviteLink(view) => {
                        copy_to_clipboard(terminal.backend_mut(), &view.link.0)
                    }
                    _ => false,
                };
                // The engine side's part, and what it has to say ahead of the update.
                for said in receive(&update, &services, || tx.clone()) {
                    app.apply(said);
                }
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
            // The torrent source is up, or will not come up; only the latest bring-up counts.
            Some((generation, outcome)) = arrivals.recv() => {
                engine.torrent_tasks.arrived(
                    generation,
                    outcome,
                    &mut engine.vpn_guard,
                    &services,
                    &engine.shared_config,
                    &tx,
                );
            }
            // Asked to end from outside — a kill, the terminal closing — which leaves the way
            // Q does, so the player is closed rather than orphaned and playing on.
            signal = signals.recv() => {
                tracing::info!(signal, "shutting down");
                tui::terminal::exit_on_second_signal();
                if let Some(task) = app.handle(anistream_tui::keymap::Action::Quit, rows) {
                    command(&app, &mut seam, task, &mut |c| {
                        route(c, &mut engine, &services, &tx)
                    });
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
                            command(&app, &mut seam, task, &mut |c| {
                                route(c, &mut engine, &services, &tx)
                            });
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

    stop_for_exit(&app, &engine.player_tx, &mut rx).await;
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

/// How long quitting waits for the playback to record where it was left.
const EXIT_FLUSH: Duration = Duration::from_secs(2);

/// Stop what is playing, and wait briefly for it to record where it ended.
///
/// The tracker writes a history row every ten seconds of playback, so quitting with an episode
/// open lost up to that much of the resume point: the runtime took mpv down with it, and the
/// position since the last row went too. Stopped the way `x` stops it, the playback writes its
/// last row before it reports the ending, which is what this waits for. Only when a playback is
/// under way: one still finding a source has nothing to save, and the wait would only delay
/// the exit.
async fn stop_for_exit(
    app: &App,
    player_tx: &Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
    rx: &mut mpsc::UnboundedReceiver<Envelope>,
) {
    let under_way = app.playing.as_ref().is_some_and(|playing| playing.duration.is_some());
    let Some(controls) = player_tx.as_ref().filter(|_| under_way) else { return };
    if controls.send(anistream_model::command::PlayerCommand::Stop).is_err() {
        return;
    }
    let ended = async {
        while let Some(update) = rx.recv().await {
            if matches!(update, Envelope::Event(Event::PlaybackEnded { .. })) {
                break;
            }
        }
    };
    if tokio::time::timeout(EXIT_FLUSH, ended).await.is_err() {
        tracing::warn!("the player did not stop in time; its last position may be lost");
    }
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
