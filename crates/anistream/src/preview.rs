//! `anistream --preview`: one frame of the interface, with live data, printed as text.

use std::{sync::Arc, time::Duration};

use anistream_core::config::{Config, Paths};
use anistream_engine::{
    engine::{Engine, EventStream},
    host::DesktopHost,
    options::EngineOptions,
    sources,
};
use anistream_model::{command::Command, event::Event};
use anistream_net::HttpClient;
use anistream_store::Store;
use anistream_tui::{
    app::{App, FrameReport},
    keymap::Keymap,
    screens,
    theme::{self, player_theme},
};
use anyhow::Result;

use crate::{PreviewScreen, covers, describe_graphics, tui::run::show_config_problem};

/// Render one frame with live data and print it as text.
///
/// Reached the way a person reaches the screen in the running app: the same startup state —
/// sources with plugins loaded, sync and store rows — then the same keys, whose tasks are
/// sent to the engine exactly as the event loop sends them and whose updates are applied until
/// nothing is outstanding. Assembling each screen by hand instead drifted from the live one:
/// the title preview pressed Enter on a resumable row and rendered Now Playing, the episode
/// table never left LOADING, and Library and Providers showed state the app never had.
#[allow(clippy::too_many_arguments)]
pub async fn preview(
    config: Config,
    http: HttpClient,
    store: Store,
    paths: &Paths,
    (width, height): (u16, u16),
    screen: PreviewScreen,
    images: bool,
    config_problem: Option<anistream_core::config::ConfigProblem>,
) -> Result<()> {
    use anistream_tui::keymap::Action;
    use ratatui::{Terminal, backend::TestBackend};

    let palette = theme::resolve_with(config.theme.mode, None);
    let mut keymap = Keymap::new();
    for problem in keymap.apply_overrides(&config.keys) {
        eprintln!("[keybinding: {problem}]");
    }
    let image_engine =
        anistream_tui::image::ImageEngine::detect(images).with_cache_dir(paths.image_cache());
    let graphics = image_engine.graphics();

    // The sources a running app has a moment after launch: the torrent source once its guard
    // has answered, and the plugins once compiled.
    let (registry, guard, mut note, held) =
        sources::build_registry(&config, &http, paths, &store).await;
    let plugins = sources::spawn_plugin_load(
        registry.clone(),
        config.clone(),
        http.clone(),
        paths.clone(),
    );
    if let Ok(Some(reason)) = plugins.await {
        note = Some(reason);
    }
    // As the file's too: a still frame shows a `--theme` as the saved choice, as it always has.
    let settings = anistream_model::settings::view(&config, &config);
    let mut app = App::with_images(settings, palette, keymap, image_engine);
    if let Some(note) = note {
        app.apply(Event::ProviderNote(note));
    }
    // A failed guard included: the interface puts its verdict on the chip and toasts it, and
    // a preview without it showed a machine whose torrents were fine.
    if let Some(state) = guard.as_ref().map(|(guard, _)| guard.state()).or(held) {
        app.apply(Event::Vpn(Some(sources::guard_view(&state))));
    }
    if let Some(problem) = &config_problem {
        show_config_problem(&mut app, paths, problem);
    }
    // The engine on that chain, whole, and with none of its own work beside what it is sent:
    // nothing in a still frame brings a torrent source up, and nothing it does not draw runs.
    let host = Arc::new(DesktopHost::new(&config, paths));
    let mut options =
        EngineOptions::new(paths.clone(), config.clone(), http, store, player_theme());
    options.saved = Some(config);
    options.sources = Some(registry);
    options.background = false;
    let (engine, mut events) = Engine::start(options, host).await?;
    // What launch says: the stores, the chain's rows and the trackers' states.
    for update in events.launch().await {
        app.apply(update);
    }

    let mut run = PreviewRun {
        rows: screens::visible_rows(
            ratatui::layout::Rect::new(0, 0, width, height),
            app.nav.section(),
        ),
        engine,
        events,
    };

    // Launch: Home loads.
    if let Some(task) = app.reload() {
        run.dispatch(task);
    }
    run.settle(&mut app).await?;

    let section = |n: u8| Action::JumpSection(n);
    let keys: Vec<Action> = match screen {
        PreviewScreen::Home => Vec::new(),
        PreviewScreen::Calendar => vec![section(2)],
        PreviewScreen::Seasonal => vec![section(3)],
        PreviewScreen::Search => vec![section(4), Action::Open],
        PreviewScreen::Library => vec![section(5)],
        PreviewScreen::Downloads => vec![section(6)],
        PreviewScreen::Providers => vec![section(7)],
        PreviewScreen::Accounts => vec![section(8)],
        PreviewScreen::Settings => vec![section(9)],
        // Into the list; the title is opened below, not with Enter, which plays a row with
        // something to resume.
        PreviewScreen::Title | PreviewScreen::Episodes => vec![Action::Open],
        PreviewScreen::Help => vec![Action::Help],
        PreviewScreen::Palette => vec![Action::CommandPalette],
    };
    for action in keys {
        run.press(&mut app, action).await;
    }

    match screen {
        // A query typed and sent, as at the keyboard. A word rather than a title, so the
        // preview shows what search looks like without depending on one show.
        PreviewScreen::Search => {
            "summer".chars().for_each(|c| app.type_char(c));
            run.press(&mut app, Action::Open).await;
        }
        PreviewScreen::Title | PreviewScreen::Episodes => {
            if let Some(task) = app.open_selected() {
                run.dispatch(task);
                run.settle(&mut app).await?;
            } else {
                eprintln!("[title: nothing on Home to open — watch something first]");
            }
            if screen == PreviewScreen::Episodes {
                run.press(&mut app, Action::ShowEpisodes).await;
            }
        }
        PreviewScreen::Palette => "ep".chars().for_each(|c| app.type_char(c)),
        _ => {}
    }
    // Whatever the last keys started.
    run.settle(&mut app).await?;
    eprintln!(
        "[graphics: {}, {} image(s) loaded]",
        describe_graphics(graphics),
        app.images.len()
    );

    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    let mut report = FrameReport::default();
    terminal.draw(|f| report = screens::render(f, &app))?;
    app.took_frame(report);
    let buffer = terminal.backend().buffer().clone();

    for y in 0..buffer.area.height {
        let line: String =
            (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_owned()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
}

/// What `--preview` drives the app with: the engine, as the event loop holds it.
struct PreviewRun {
    rows: usize,
    engine: Engine,
    events: EventStream,
}

impl PreviewRun {
    /// How long a preview waits for the screen's work before drawing whatever it has.
    const PATIENCE: Duration = Duration::from_secs(30);

    /// Press a key, as the event loop does.
    async fn press(&mut self, app: &mut App, action: anistream_tui::keymap::Action) {
        if let Some(task) = app.handle(action, self.rows) {
            self.dispatch(task);
        }
    }

    /// Send a task to the engine as the event loop sends one — unless running it would do
    /// something a still frame must not: play, download, sign in, change a setting or a store.
    fn dispatch(&self, task: Command) {
        let loads = matches!(
            task,
            Command::LoadContinue
                | Command::LoadSeasonal
                | Command::LoadCalendar
                | Command::LoadCalendarPast { .. }
                | Command::Search { .. }
                | Command::LoadDetail(_)
                | Command::LoadEpisodes(_)
                | Command::CheckProviders
                | Command::LoadLibrary(_)
                | Command::LoadDownloads
                | Command::LoadStores
                // The covers a still frame shows.
                | Command::FetchArtwork { .. }
        );
        if !loads {
            eprintln!("[preview: not run: {task:?}]");
            return;
        }
        self.engine.send(task);
    }

    /// Apply updates until the work is done and the screen's covers are in, as the event loop
    /// would over the same moments. Fails if the engine stops, which it does only by panicking.
    async fn settle(&mut self, app: &mut App) -> Result<()> {
        let deadline = tokio::time::Instant::now() + Self::PATIENCE;
        loop {
            for task in app.take_commands() {
                self.dispatch(task);
            }
            let mut fetching = false;
            for url in app.visible_artwork(self.rows) {
                self.dispatch(Command::FetchArtwork { url });
                fetching = true;
            }
            // Quiet for this long with no work outstanding means done. Longer while covers are
            // on their way, which report back without a job.
            let quiet =
                if fetching { Duration::from_secs(3) } else { Duration::from_millis(400) };
            let wait = if app.jobs.is_empty() { quiet } else { Self::PATIENCE };
            let wait =
                wait.min(deadline.saturating_duration_since(tokio::time::Instant::now()));
            match tokio::time::timeout(wait, self.events.recv()).await {
                Ok(Some(update)) => match update {
                    // Decoded here and now: a still frame has no loop to keep moving.
                    Event::ArtworkReady { url, bytes } => {
                        covers::hand_over(app, covers::now(url, &bytes));
                    }
                    update => app.apply(update),
                },
                Ok(None) => anyhow::bail!("the engine stopped; the log says why"),
                Err(_) if app.jobs.is_empty() => return Ok(()),
                Err(_) => {
                    let waiting: Vec<&str> =
                        app.jobs.iter().map(|j| j.label.as_str()).collect();
                    eprintln!("[preview: drawn while still {}]", waiting.join(", "));
                    return Ok(());
                }
            }
        }
    }
}
