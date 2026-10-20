//! anistream — an anime streaming TUI.

mod cli;
mod covers;
mod tui;

use cli::{
    doctor::{doctor, refresh_data},
    library::{export_cli, import_cli, random_cli, stats_cli},
    login::{forget_token, login, migrate_token, print_login_url},
    play::{play_cli, stream_url},
    plugins::{PluginCommand, list_plugins},
    search::search_cli,
    store::store_command,
    sync::sync_once,
};

use anistream_engine::{
    artwork,
    catalog::{
        calendar::{load_calendar, load_calendar_past},
        detail::{load_detail, open_external, play_trailer},
        lists::{hide_from_continue, load_continue, load_seasonal},
        search::search,
    },
    data,
    dispatch::{Lent, spawn},
    downloads,
    jobs::JobGuard,
    now_epoch,
    outbox::{Envelope, Internal, Outbox},
    playback::resolve::Resolved,
    playback::{
        self,
        player::{player, shader_dir},
    },
    plugin_stores,
    services::Services,
    sources::{self, torrent::reload_sources},
    state::{EngineState, PlayHints},
    titles::{
        episodes::{EpisodeLoad, list_episodes, load_episodes, set_watched},
        matching::{clear_match, fix_match, manual_search, set_provider_preference},
    },
    tracking,
};

use std::{io, time::Duration};

use anistream_core::{
    config::{Config, Paths, ThemeMode},
    ids::AnilistId,
};
use anistream_meta::{Meta, dataset};
use anistream_model::{
    catalog::LibrarySegment,
    command::{Command, PlayerCommand},
    episodes::EpisodeRow,
    event::Event,
    jobs::JobId,
    notice::Notice,
    playback::{EpisodeEntry, NextEpisode},
    sources::only_reorders,
    stores::Secret,
};
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_tui::{
    app::App,
    keymap::Keymap,
    screens,
    theme::{self, player_theme},
};
use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{Event as TermEvent, EventStream, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(name = "anistream", version, about = "An anime streaming TUI")]
// Nothing here means anything to `store` or `plugin`, and `--stats plugin stamp …` ran the stamp
// without a word about the flag it dropped.
#[command(args_conflicts_with_subcommands = true)]
// One command per run. Resolved silently by the order of the checks in `main`, `--stats
// --random` printed the stats and dropped the rest without a word.
#[command(group(
    clap::ArgGroup::new("command").multiple(false).args([
        "doctor", "update", "refresh_data", "preview", "login", "login_url", "logout", "sync",
        "token_to_file", "search", "stats", "export", "import", "random", "stream_url", "play",
        "continue_watching", "plugins",
    ])
))]
// The options that only mean something to some commands. Alone, each used to start the
// interface as if it had not been typed.
#[command(group(clap::ArgGroup::new("json_command").multiple(true).args(["search", "stats", "random"])))]
#[command(group(clap::ArgGroup::new("episode_command").multiple(true).args(["stream_url", "play"])))]
#[command(group(
    clap::ArgGroup::new("tracker_command")
        .multiple(true)
        .args(["login", "login_url", "logout", "token_to_file"])
))]
struct Cli {
    /// Force a theme mode, overriding config.
    #[arg(long, value_parser = ["adaptive", "immersive"])]
    theme: Option<String>,

    /// Turn off terminal graphics and use text only.
    #[arg(long)]
    no_images: bool,

    /// Report what the terminal supports and exit.
    #[arg(long)]
    doctor: bool,

    /// Download and install the latest release, then exit.
    #[arg(long)]
    update: bool,

    /// Refresh the mapping datasets and exit.
    #[arg(long)]
    refresh_data: bool,

    /// Render one frame to stdout as text and exit.
    ///
    /// Lets the layout be inspected without an interactive terminal — useful for checking
    /// the design at a given size, and for seeing it at all over a non-tty connection.
    #[arg(long, value_name = "WIDTHxHEIGHT", value_parser = parse_size)]
    preview: Option<(u16, u16)>,

    /// Which screen `--preview` renders. Home when left out.
    #[arg(long, value_enum, requires = "preview")]
    screen: Option<PreviewScreen>,

    /// Sign in to a tracker and exit.
    ///
    /// With no value, runs the configured flow: `loopback` opens a browser and catches the
    /// token automatically, `paste` prints the URL and reads the token from stdin. Pass `-` to
    /// read from stdin regardless, or a token directly — though that puts a year-long account
    /// credential in your shell history.
    #[arg(long, value_name = "TOKEN", num_args = 0..=1, default_missing_value = "")]
    login: Option<String>,

    /// Which tracker `--login`, `--login-url`, `--logout` and `--token-to-file` are for.
    /// AniList when left out.
    #[arg(long, requires = "tracker_command")]
    tracker: Option<String>,

    /// Print the authorize URL for `--tracker` and exit.
    #[arg(long)]
    login_url: bool,

    /// Forget a tracker's stored token and exit.
    #[arg(long)]
    logout: bool,

    /// Drain the outbox and pull the library once, reporting what happened, then exit.
    ///
    /// The non-interactive way to see whether sync actually works, which is otherwise only
    /// observable as a badge.
    #[arg(long)]
    sync: bool,

    /// Move a stored token out of the OS keychain into a `0600` file, then exit.
    ///
    /// Costs one keychain prompt — the last one. macOS keys keychain access on the binary, and
    /// every `cargo build` produces a new one, so during development "Always Allow" can never
    /// stick. Set `trackers.token_storage = "file"` and run this once.
    #[arg(long)]
    token_to_file: bool,

    /// Search AniList and print the results, then exit.
    #[arg(long, value_name = "QUERY")]
    search: Option<String>,

    /// Print watch statistics and exit.
    #[arg(long)]
    stats: bool,

    /// Write a history export to a file, or `-` for stdout, then exit.
    #[arg(long, value_name = "PATH")]
    export: Option<String>,

    /// Merge a history export back in, then exit.
    #[arg(long, value_name = "PATH")]
    import: Option<String>,

    /// Pick something at random from your history and print it, then exit.
    #[arg(long)]
    random: bool,

    /// Emit JSON instead of text: for `--search`, `--stats` and `--random`.
    ///
    /// The point of the non-interactive commands: composable with `jq` and friends rather than a
    /// walled garden.
    // Refused by name beside the other commands: `--doctor --json` was told it lacked
    // `--search`, `--stats` or `--random`, as though it had asked for one of them.
    #[arg(long, requires = "json_command", conflicts_with_all = [
        "doctor", "update", "refresh_data", "preview", "login", "login_url", "logout", "sync",
        "token_to_file", "export", "import", "stream_url", "play", "continue_watching", "plugins",
    ])]
    json: bool,

    /// Resolve one episode to a playable URL, print it, and hold it open.
    ///
    /// The thing that was missing when playback failed silently: something to point mpv, VLC or
    /// curl at by hand, so "anistream is broken" and "my player is broken" can be told apart. The
    /// torrent session has to stay alive for the URL to serve anything, so this waits for Ctrl-C
    /// rather than exiting.
    #[arg(long, value_name = "ANILIST_ID")]
    stream_url: Option<u32>,

    /// Which episode `--stream-url` and `--play` act on. Accepts non-numeric ids like `OVA`.
    ///
    /// Left unset, `--stream-url` takes episode 1 and `--play` takes the next unwatched.
    #[arg(long, requires = "episode_command")]
    episode: Option<String>,

    /// Play one title without the TUI: resolve, hand to mpv, record history, exit.
    ///
    /// The same session the app runs — resume, skips, chapters, tracker queueing — narrated
    /// to stdout instead of drawn. Composable: `anistream --search x --json | jq` finds the
    /// id, this plays it.
    #[arg(long, value_name = "ANILIST_ID")]
    play: Option<u32>,

    /// Pick up where you left off: resume the most recent episode, or start the next one.
    #[arg(long = "continue")]
    continue_watching: bool,

    /// List installed plugins with what each one is permitted to reach, then exit.
    ///
    /// A plugin registry means running other people's code, so what a plugin may contact should
    /// be inspectable without reading its source.
    #[arg(long)]
    plugins: bool,

    #[command(subcommand)]
    command: Option<Subcommand>,
}

/// The screens `--preview` can render.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewScreen {
    Home,
    Calendar,
    Seasonal,
    Search,
    Library,
    Downloads,
    Providers,
    Accounts,
    Settings,
    /// The first Continue title's page.
    Title,
    /// The first Continue title's episode table.
    Episodes,
    Help,
    Palette,
}

/// `WIDTHxHEIGHT`, refused when it is not one, rather than quietly rendered at 120x34.
fn parse_size(text: &str) -> std::result::Result<(u16, u16), String> {
    let expected = || format!("expected WIDTHxHEIGHT, like 120x34, not {text:?}");
    let (width, height) = text.split_once(['x', 'X']).ok_or_else(expected)?;
    let (width, height): (u16, u16) = (
        width.trim().parse().map_err(|_| expected())?,
        height.trim().parse().map_err(|_| expected())?,
    );
    // Zero rows printed nothing at all; past a thousand a frame is a mistake, not a terminal.
    if !(1..=1000).contains(&width) || !(1..=1000).contains(&height) {
        return Err(format!("{width}x{height} cannot be drawn: each side must be 1 to 1000"));
    }
    Ok((width, height))
}

#[derive(clap::Subcommand, Debug)]
enum Subcommand {
    /// Run a plugin store, or look after one from the command line.
    #[command(subcommand)]
    Store(anistream_store_server::StoreCommand),
    /// Work with plugin files.
    #[command(subcommand)]
    Plugin(PluginCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    // macOS hands every process 256 file descriptors — less than one healthy torrent
    // swarm, and the first casualty was mpv failing to *spawn* mid-binge. Ask for a
    // real allowance up front; the OS hard limit still has the final word.
    #[cfg(unix)]
    let _ = rlimit::increase_nofile_limit(8192);

    let args = Cli::parse();
    let paths = Paths::resolve().context("resolving config paths")?;
    // A store server is a long-running service in someone's terminal or a unit file, so it logs
    // where they are looking rather than to the app's log file.
    if let Some(command) = args.command {
        return store_command(command, &paths).await;
    }
    init_logging(&paths);

    // A malformed config must not stop the app from starting, but it must be loud — silently
    // ignoring it could mean silently ignoring a VPN setting. Logged, printed for the commands
    // that print, and kept for the interface, which says so for as long as it runs.
    let mut config_problem = None;
    let mut config = Config::read(&paths).unwrap_or_else(|problem| {
        let file = paths.config_file.display();
        tracing::error!(%file, %problem, "config not loaded; using the defaults");
        eprintln!("warning: {file}, {problem}\nfalling back to defaults");
        config_problem = Some(problem);
        Config::default()
    });
    if let Some(mode) = &args.theme {
        config.theme.mode =
            if mode == "immersive" { ThemeMode::Immersive } else { ThemeMode::Adaptive };
    }

    let http = HttpClient::new(&config.network).context("building http client")?;
    let store = Store::open(paths.database()).context("opening the local database")?;

    if args.update {
        return cli::update::self_update(&http).await;
    }
    if args.doctor {
        return doctor(&config, &paths, &store, !args.no_images).await;
    }
    if args.refresh_data {
        return refresh_data(&store, &http).await;
    }
    let tracker = args.tracker.as_deref().unwrap_or("anilist");
    if args.login_url {
        return print_login_url(&config, tracker);
    }
    if let Some(token) = &args.login {
        return login(&config, tracker, token, &http).await;
    }
    if args.logout {
        return forget_token(&config, tracker);
    }
    if args.sync {
        return sync_once(&config, &store, &http).await;
    }
    if args.token_to_file {
        return migrate_token(&config, tracker);
    }
    if args.plugins {
        return list_plugins(&config, &paths).await;
    }
    if let Some(id) = args.stream_url {
        let episode = args.episode.as_deref().unwrap_or("1");
        return stream_url(&config, &http, &store, &paths, id, episode).await;
    }
    if let Some(id) = args.play {
        let id = anistream_core::ids::AnilistId::new(id);
        // No `--episode` means the next unwatched, which is almost always the ask.
        let episode = match &args.episode {
            Some(episode) => episode.clone(),
            None => (store.completed_episode_count(id).unwrap_or(0) + 1).to_string(),
        };
        return play_cli(&config, &http, &store, &paths, id, episode).await;
    }
    if args.continue_watching {
        let Some(top) = store.continue_list(1).unwrap_or_default().into_iter().next() else {
            anyhow::bail!("nothing in your history yet — watch something first");
        };
        // Half-finished resumes; anything else moves on to the episode after the last one
        // that counted. The same reading of history the home screen's CONTINUE rail uses.
        let episode = if top.is_resumable(anistream_store::RESUME_CEILING) {
            top.last_episode.clone()
        } else if top.episodes_done > 0 {
            (top.episodes_done + 1).to_string()
        } else {
            top.last_episode.clone()
        };
        return play_cli(&config, &http, &store, &paths, top.anilist_id, episode).await;
    }
    if let Some(query) = &args.search {
        return search_cli(&config, &http, &store, query, args.json).await;
    }
    if args.stats {
        return stats_cli(&store, args.json);
    }
    if let Some(path) = &args.export {
        return export_cli(&store, path);
    }
    if let Some(path) = &args.import {
        return import_cli(&store, path);
    }
    if args.random {
        return random_cli(&store, args.json);
    }
    if let Some(size) = args.preview {
        let screen = args.screen.unwrap_or(PreviewScreen::Home);
        return preview(
            config,
            http,
            store,
            &paths,
            size,
            screen,
            !args.no_images,
            config_problem,
        )
        .await;
    }

    run(config, paths, http, store, !args.no_images, config_problem).await
}

/// Render one frame with live data and print it as text.
///
/// Reached the way a person reaches the screen in the running app: the same startup state —
/// sources with plugins loaded, sync and store rows — then the same keys, whose tasks are
/// dispatched exactly as the event loop dispatches them and whose updates are applied until
/// nothing is outstanding. Assembling each screen by hand instead drifted from the live one:
/// the title preview pressed Enter on a resumable row and rendered Now Playing, the episode
/// table never left LOADING, and Library and Providers showed state the app never had.
#[allow(clippy::too_many_arguments)]
async fn preview(
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
    let engine =
        anistream_tui::image::ImageEngine::detect(images).with_cache_dir(paths.image_cache());
    let graphics = engine.graphics();

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
    let services =
        Services::build(&config, paths.clone(), http, store, registry, player_theme())
            .map_err(anyhow::Error::msg)?;
    // Nothing in a still frame brings a torrent source up, so nothing reports on this.
    let state = EngineState::new(config.clone(), mpsc::unbounded_channel().0);
    let mut app = App::with_images(config, palette, keymap, engine);
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
    match services.stores.rows() {
        Ok(rows) => app.apply(Event::Stores(rows)),
        Err(e) => app.apply(Event::Toast(Notice::alert(e))),
    }
    app.apply(Event::Providers(data::provider_rows(&services.registry)));
    for state in services.sync.initial_states() {
        app.apply(Event::Sync(Box::new(state)));
    }

    let (tx, mut rx) = Outbox::inbox();
    let mut run = PreviewRun {
        tx,
        rows: screens::visible_rows(
            ratatui::layout::Rect::new(0, 0, width, height),
            app.nav.section(),
        ),
        engine: state,
        seam: ConfigSeam::default(),
        services,
    };

    // Launch: Home loads.
    if let Some(task) = app.reload() {
        run.dispatch(&mut app, task);
    }
    run.settle(&mut app, &mut rx).await;

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
                run.dispatch(&mut app, task);
                run.settle(&mut app, &mut rx).await;
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
    run.settle(&mut app, &mut rx).await;
    eprintln!(
        "[graphics: {}, {} image(s) loaded]",
        describe_graphics(graphics),
        app.images.len()
    );

    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|f| screens::render(f, &app))?;
    let buffer = terminal.backend().buffer().clone();

    for y in 0..buffer.area.height {
        let line: String =
            (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_owned()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
}

/// What `--preview` drives the app with: the same handles the event loop holds.
struct PreviewRun {
    tx: Outbox,
    rows: usize,
    engine: EngineState,
    seam: ConfigSeam,
    services: Services,
}

impl PreviewRun {
    /// How long a preview waits for the screen's work before drawing whatever it has.
    const PATIENCE: Duration = Duration::from_secs(30);

    /// Press a key, as the event loop does.
    async fn press(&mut self, app: &mut App, action: anistream_tui::keymap::Action) {
        if let Some(task) = app.handle(action, self.rows) {
            self.dispatch(app, task);
        }
    }

    /// Hand a task to the same dispatcher the event loop uses — unless running it would do
    /// something a still frame must not: play, download, sign in, change a setting or a store.
    fn dispatch(&mut self, app: &mut App, task: Command) {
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
        command(app, &mut self.seam, task, &mut self.engine, &self.services, &self.tx);
    }

    /// Apply updates until the work is done and the screen's covers are in, as the event loop
    /// would over the same moments.
    async fn settle(&mut self, app: &mut App, rx: &mut mpsc::UnboundedReceiver<Envelope>) {
        let deadline = tokio::time::Instant::now() + Self::PATIENCE;
        loop {
            if let Some(task) = app.take_pending() {
                self.dispatch(app, task);
            }
            let mut fetching = false;
            for url in app.visible_artwork(self.rows) {
                self.dispatch(app, Command::FetchArtwork { url });
                fetching = true;
            }
            // Quiet for this long with no work outstanding means done. Longer while covers are
            // on their way, which report back without a job.
            let quiet =
                if fetching { Duration::from_secs(3) } else { Duration::from_millis(400) };
            let wait = if app.jobs.is_empty() { quiet } else { Self::PATIENCE };
            let wait =
                wait.min(deadline.saturating_duration_since(tokio::time::Instant::now()));
            match tokio::time::timeout(wait, rx.recv()).await {
                // A still frame writes no setting, so nothing leaves itself work; were it to,
                // it would be done here, in its place, as the loop does it.
                Ok(Some(Envelope::Internal(message))) => {
                    let said =
                        handle_internal(message, &mut self.engine, &self.services, &self.tx)
                            .await;
                    said.into_iter().for_each(|said| app.apply(said));
                }
                Ok(Some(Envelope::Event(update))) => {
                    match update {
                        // Decoded here and now: a still frame has no loop to keep moving.
                        Event::ArtworkReady { url, bytes } => {
                            covers::hand_over(app, covers::now(url, &bytes));
                        }
                        update => app.apply(update),
                    }
                    meta_notice(&self.services.meta)
                        .into_iter()
                        .for_each(|said| app.apply(said));
                }
                Ok(None) => return,
                Err(_) if app.jobs.is_empty() => return,
                Err(_) => {
                    let waiting: Vec<&str> =
                        app.jobs.iter().map(|j| j.label.as_str()).collect();
                    eprintln!("[preview: drawn while still {}]", waiting.join(", "));
                    return;
                }
            }
        }
    }
}

async fn run(
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
    let services = Services::build(&config, paths, http, store, registry, player_theme())
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
    use_config(&app, &mut seam, &mut engine, &services, &tx);

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
        let job = JobGuard::start(&mut engine, &task, &tx);
        dispatch(task, job, &mut engine, &services, &tx);
    }

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    // Covers come back from the blocking pool on this, decoded, to be handed to the app.
    let (covers_tx, mut covers_rx) = mpsc::unbounded_channel();

    let mpv = player(&engine.config, &services.paths, &player_theme());
    if !mpv.is_available().await {
        // Said once at startup rather than at the moment you press Enter on an episode.
        app.apply(Event::Toast(Notice::alert(format!(
            "{} not found — install mpv to play anything",
            engine.config.playback.mpv_binary
        ))));
    } else if let Some(note) = mpv.mpv_version().await.and_then(old_mpv_note) {
        app.apply(Event::Toast(Notice::info(note)));
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
            command(&app, &mut seam, task, &mut engine, &services, &tx);
        }

        // Auto-next is queued by the reducer rather than returned, because an episode ending
        // arrives as an update rather than a keystroke.
        if let Some(task) = app.take_pending() {
            command(&app, &mut seam, task, &mut engine, &services, &tx);
        }
        // Whatever moved the config since: a setting put back, say. The reactions below read
        // the engine side's copy.
        use_config(&app, &mut seam, &mut engine, &services, &tx);

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
                    command(&app, &mut seam, task, &mut engine, &services, &tx);
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
                            command(&app, &mut seam, task, &mut engine, &services, &tx);
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

/// What the engine side was last told of the interface's config.
///
/// The interface changes its config in place, from a keypress or from the player, and the
/// engine side works from a copy. This keeps the copy current: compared on every pass, sent as
/// `UseConfig` when either half moved. Temporary, like `UseConfig`: P4 makes the engine the
/// config's owner, and both go.
#[derive(Default)]
struct ConfigSeam(Option<(Config, Config)>);

impl ConfigSeam {
    /// `UseConfig`, when the config in effect or the saved one has moved since the engine side
    /// was last told, and the first time it is asked.
    fn moved(&mut self, app: &App) -> Option<Command> {
        if let Some((config, saved)) = &self.0
            && *config == app.config
            && *saved == app.saved_config
        {
            return None;
        }
        self.0 = Some((app.config.clone(), app.saved_config.clone()));
        Some(Command::UseConfig {
            config: Box::new(app.config.clone()),
            saved: Box::new(app.saved_config.clone()),
        })
    }
}

/// Tell the engine side the interface's config, when it moved since the engine side was told.
///
/// Before the first command, before every command after it, and before the loop's reactions,
/// which read it too. So the work reads the config the screen shows at the moment it reads it,
/// as it read `app.config` itself before.
fn use_config(
    app: &App,
    seam: &mut ConfigSeam,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if let Some(config) = seam.moved(app) {
        dispatch(config, None, engine, services, tx);
    }
}

/// Send a command to the engine side as the interface sends one: the config ahead of it when
/// it moved, and a play's hints ahead of a play, then the command, its job started as it is
/// dispatched. Returns the job, for work someone waits on.
fn command(
    app: &App,
    seam: &mut ConfigSeam,
    task: Command,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Option<JobId> {
    use_config(app, seam, engine, services, tx);
    if let Some(hints) = play_hints(app, &task) {
        dispatch(hints, None, engine, services, tx);
    }
    let job = JobGuard::start(engine, &task, tx);
    let id = job.as_ref().map(|job| job.id);
    dispatch(task, job, engine, services, tx);
    id
}

/// Refresh the mapping datasets, and keep trying in the background until a refresh succeeds.
///
/// A launch without a network used to try once and give up for the session, leaving the
/// trackers that key on MAL and Kitsu ids without the mappings they need until the next launch.
/// A failure is said once, in words; the retries back off from five minutes to an hour; and
/// the recovery is said too, so the last thing on screen stays true. A refresh only fetches what
/// is due, so a retry asks again for exactly the datasets that failed.
async fn keep_datasets_current(store: Store, http: HttpClient, tx: Outbox) {
    let mut wait = Duration::from_secs(5 * 60);
    let mut said = false;
    loop {
        let outcomes = dataset::refresh_all(&store, &http, now_epoch(), false).await;
        let failed: Vec<&str> =
            outcomes.iter().filter(|(_, o)| o.is_failure()).map(|(name, _)| *name).collect();
        let Some(reason) = outcomes.iter().find_map(|(_, o)| match o {
            dataset::RefreshOutcome::Failed { reason } => Some(reason.as_str()),
            _ => None,
        }) else {
            if said {
                tracing::info!("mapping datasets refreshed after failing");
                let _ = tx.send(Event::Toast(Notice::info("mapping data refreshed")));
            }
            return;
        };
        tracing::warn!(datasets = ?failed, %reason, retry_secs = wait.as_secs(), "mapping refresh failed");
        if !said {
            said = true;
            let _ = tx.send(Event::Toast(Notice::alert(format!(
                "mapping data could not be refreshed ({}: {}) — trying again in {} min",
                failed.join(", "),
                plain_failure(reason),
                wait.as_secs() / 60
            ))));
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(3600));
    }
}

/// What the terminal's graphics support amounts to, in words.
fn describe_graphics(graphics: anistream_tui::image::Graphics) -> &'static str {
    match graphics {
        anistream_tui::image::Graphics::Native => "a graphics protocol, so covers are pictures",
        anistream_tui::image::Graphics::Halfblocks => {
            "no graphics protocol, so covers are drawn in half blocks"
        }
        anistream_tui::image::Graphics::Disabled => "off (--no-images)",
    }
}

/// A request failure without the plumbing: "could not be reached" rather than the client's
/// "error sending request for url (…)", which names the URL and not the problem.
fn plain_failure(reason: &str) -> &str {
    if reason.contains("error sending request") || reason.contains("timed out") {
        "the server could not be reached"
    } else {
        reason
    }
}

/// Say that the config did not load, on the status line for as long as the session runs and in
/// the log. The interface and its previews both, so a preview shows the frame a person would see.
fn show_config_problem(
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

/// Every source's condition, and the metadata ladder's beside them.
async fn check_providers(lent: Lent) {
    let Lent { meta, registry, tx, .. } = lent;
    registry.check_all(now_epoch()).await;
    // The metadata ladder rides the same refresh: one screen, one key,
    // every source's condition.
    let _ = tx.send(Event::MetaStatus(data::meta_rows(&meta)));
    let update = Event::Providers(data::provider_rows(&registry));
    let _ = tx.send(update);
}

/// The engine side's part in an event on its way to the front end, and what it has to say
/// ahead of the event, for the front end to apply first.
///
/// A finished episode's progress goes to the trackers now, not at the next drain tick: up to a
/// minute later, the pushed progress looked simply absent. And the metadata ladder's notice, when
/// it moved. `tx` is asked for only when there is work to answer on it.
fn receive(update: &Event, services: &Services, tx: impl FnOnce() -> Outbox) -> Vec<Event> {
    if matches!(update, Event::ProgressQueued) {
        let (sync, tx) = (services.sync.clone(), tx());
        tokio::spawn(async move { tracking::drain_once(&sync, &tx).await });
    }
    meta_notice(&services.meta)
}

/// What to say when the metadata ladder moved: once per transition, in both directions, since
/// silently recovering would leave the last thing said untrue. The status rows ride along, so
/// the header chip and the Providers screen agree with the toast without waiting for a refresh.
fn meta_notice(meta: &Meta) -> Vec<Event> {
    let Some(notice) = meta.take_notice() else { return Vec::new() };
    vec![Event::MetaStatus(data::meta_rows(meta)), Event::Toast(Notice::info(notice))]
}

/// Do what the engine side left itself to do, and return what it then has to say, for the
/// front end to apply at once, before anything sent since. Reached in the message's own place
/// among the events, after everything sent before it.
///
/// The metadata ladder's notice comes first, as on the way of an event. Then the copy of the
/// config the background tasks read is brought up to the one in effect, whatever the message.
async fn handle_internal(
    message: Internal,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Vec<Event> {
    let mut said = meta_notice(&services.meta);
    if let Ok(mut shared) = engine.shared_config.write() {
        *shared = engine.config.clone();
    }
    match message {
        Internal::SettingsSaved => {}
        // Only the order changed: the chain as it stands is put in it, with no rebuild.
        Internal::SourcesReordered => {
            sources::arrange(&services.registry, &engine.config);
            said.push(Event::Providers(data::provider_rows(&services.registry)));
        }
        // Awaited rather than spawned: this owns the session and the guard, and two
        // rebuilds racing would leave one of their sessions running with nothing holding
        // it. Quick: the new torrent source comes up behind the interface, as at launch.
        Internal::SourcesChanged => {
            reload_sources(
                services,
                &mut engine.torrent_tasks,
                &mut engine.vpn_guard,
                &engine.shared_config,
                tx,
                &engine.arrivals,
            )
            .await;
            // Passed on, ahead of what the rebuild reports: the screen says it reloads.
            said.push(Event::SourcesChanged);
        }
    }
    said
}

/// Route a command to whatever carries it out.
///
/// One arm for every command, and each names what runs it, so a command added to the
/// protocol does not build until it is routed. What asks AniList or the sources and answers
/// once runs through [`spawn`], behind the AniList limit. Playback runs on its own: it owns
/// the live player's control channel, and replacing that sender is what stops a finished
/// episode's keys steering the next one.
fn dispatch(
    task: Command,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    match task {
        // What the front end keeps the engine side told of, until P4.
        Command::UseConfig { config, saved } => take_config(config, saved, engine),
        Command::PlayHints { title, next, episodes } => {
            keep_hints(title, next, episodes, engine)
        }

        // The lists, the calendar, search and a title's page.
        Command::LoadContinue => spawn(job, services, tx, load_continue),
        Command::LoadSeasonal => spawn(job, services, tx, load_seasonal),
        Command::HideFromContinue { id } => hide_from_continue(id, services, tx),
        Command::LoadCalendar => spawn(job, services, tx, load_calendar),
        Command::LoadCalendarPast { anchor, page } => {
            spawn(job, services, tx, move |lent| load_calendar_past(lent, anchor, page))
        }
        Command::Search { query, filter } => {
            spawn(job, services, tx, move |lent| search(lent, query, filter))
        }
        Command::LoadDetail(id) => spawn(job, services, tx, move |lent| load_detail(lent, id)),
        Command::PlayTrailer { url, title } => play_trailer(url, title, job, engine, tx),
        Command::OpenExternal { url } => open_external(url, tx),

        // A title's episodes, and the source match they come from.
        Command::LoadEpisodes(id) => {
            spawn(job, services, tx, move |lent| list_episodes(lent, id))
        }
        Command::SetWatched { id, episodes, watched } => {
            set_watched(id, episodes, watched, services)
        }
        Command::ClearMatch { id } => {
            spawn(job, services, tx, move |lent| clear_match(lent, id))
        }
        Command::FixMatch { id, provider_id, key } => {
            spawn(job, services, tx, move |lent| fix_match(lent, id, provider_id, key))
        }
        Command::ManualSearch { id, query } => {
            manual_search(id, query, job, engine, services, tx)
        }
        Command::SetProviderPreference { id, provider } => {
            set_provider_preference(id, provider, services, tx)
        }

        // Playback.
        Command::Prefetch { id, episode } => prefetch(id, episode, engine, services),
        Command::Play { id, episode } => play(id, episode, job, engine, services, tx),
        Command::PlaySource { id, episode, provider_id, source_id } => {
            play_source(id, episode, provider_id, source_id, job, engine, services, tx)
        }
        Command::PlayParty { id, episode } => {
            play_party(id, episode, job, engine, services, tx)
        }
        Command::PlayLocal { id, episode, title, path } => {
            play_local(id, episode, title, path, job, engine, services, tx)
        }
        Command::LoadSources { id, episode } => {
            load_sources(id, episode, job, engine, services, tx)
        }
        Command::Player(command) => control_player(command, engine),

        // The sources.
        Command::CheckProviders => spawn(job, services, tx, check_providers),

        // The trackers.
        Command::SyncNow => sync_now(job, services, tx),
        Command::Connect { tracker } => connect(tracker, job, services, tx),
        Command::ConnectKitsu { username, password } => {
            connect_kitsu(username, password, job, services, tx)
        }
        Command::Disconnect { tracker } => tracking::disconnect(&services.sync, &tracker, tx),
        Command::SetStatus { id, status } => set_status(id, status, services, tx),
        Command::SetScore { id, score } => set_score(id, score, services, tx),
        Command::ResolveConflict { id, keep_local } => {
            tracking::resolve_conflict(&services.sync, id, keep_local)
        }
        Command::LoadLibrary(segment) => load_library(segment, job, services, tx),

        // Downloads.
        Command::LoadDownloads => downloads::publish_now(&services.store, tx),
        Command::DownloadEpisode { id, episode } => {
            download_episode(id, episode, job, engine, services, tx)
        }
        Command::DownloadMany { id, episodes } => {
            download_many(id, episodes, job, engine, services, tx)
        }
        Command::DownloadPause { id } => download_pause(id, services, tx),
        Command::DownloadCancel { id } => download_cancel(id, services, tx),
        Command::DownloadDelete { id } => download_delete(id, services, tx),
        Command::DownloadClearCompleted => download_clear_completed(services, tx),

        // Settings.
        Command::SaveSetting { table, key, value } => {
            save_setting(table, key, value, engine, services, tx)
        }

        // Off the loop, with no job: covers report back on their own.
        Command::FetchArtwork { url } => artwork::spawn_fetch(
            url,
            services.paths.image_cache(),
            services.http.clone(),
            tx.clone(),
        ),

        // The plugin stores.
        Command::LoadStores => {
            store_task(job, engine, services, tx, StoreTaskContext::load_stores)
        }
        Command::PreviewStore { link } => {
            store_task(job, engine, services, tx, move |context| context.preview_store(link))
        }
        Command::JoinStore { link } => {
            store_task(job, engine, services, tx, move |context| context.join_store(link))
        }
        Command::LeaveStore { store } => {
            store_task(job, engine, services, tx, move |context| context.leave_store(store))
        }
        Command::InstallPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.install_plugin(store, plugin)
            })
        }
        Command::RemovePlugin { plugin } => {
            store_task(job, engine, services, tx, move |context| context.remove_plugin(plugin))
        }
        Command::CreateInvite { store, name } => {
            store_task(job, engine, services, tx, move |context| {
                context.create_invite(store, name)
            })
        }
        Command::LoadMembers { store } => {
            store_task(job, engine, services, tx, move |context| context.load_members(store))
        }
        Command::RemoveMember { store, member } => {
            store_task(job, engine, services, tx, move |context| {
                context.remove_member(store, member)
            })
        }
        Command::CancelInvite { store, invite } => {
            store_task(job, engine, services, tx, move |context| {
                context.cancel_invite(store, invite)
            })
        }
        Command::LoadLocalPlugins { store } => {
            store_task(job, engine, services, tx, move |context| {
                context.load_local_plugins(store)
            })
        }
        Command::PublishPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.publish_plugin(store, plugin)
            })
        }
        Command::UnpublishPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.unpublish_plugin(store, plugin)
            })
        }
        Command::OpenAdmin { store } => {
            store_task(job, engine, services, tx, move |context| context.open_admin(store))
        }
    }
}

/// The config in effect and the file's, as the front end last said. Kept current by the front
/// end, which changes its own in place, until P4.
fn take_config(now: Box<Config>, saved: Box<Config>, engine: &mut EngineState) {
    let EngineState { config, saved_config, .. } = engine;
    *config = *now;
    *saved_config = *saved;
}

/// What the front end said the next play needs, for that play to take: from the episode table
/// the front end holds, until P4.
fn keep_hints(
    title: String,
    next: Option<NextEpisode>,
    episodes: Vec<EpisodeEntry>,
    engine: &mut EngineState,
) {
    let EngineState { hints, .. } = engine;
    *hints = Some(PlayHints { title, next, episodes });
}

/// Find the next episode's streams while this one's ending plays, for the play that follows to
/// take.
fn prefetch(id: AnilistId, episode: String, engine: &EngineState, services: &Services) {
    let Services { meta, store, registry, prefetch, .. } = services;
    let EngineState { config, .. } = engine;
    let (meta, store, registry, playback, prefetch) = (
        meta.clone(),
        store.clone(),
        registry.clone(),
        config.playback.clone(),
        prefetch.clone(),
    );
    tokio::spawn(async move {
        // Quiet: the steps of a search nobody is waiting on must not take the status
        // line from the episode playing. Nothing reads this channel.
        let quiet = Outbox::from(mpsc::unbounded_channel().0);
        let found = resolve_for_playback(
            &meta, &store, &registry, id, &episode, None, &playback, None, &quiet,
        )
        .await;
        match found {
            Ok(resolved) => prefetch.keep(id, &episode, playback.translation, resolved),
            Err(reason) => {
                tracing::info!(%reason, "could not find the next episode ahead")
            }
        }
    });
}

/// Play an episode from whichever source the resolution ladder chooses.
///
/// No player handle is kept across the session: every play builds its own from the config in
/// effect, which is the live one the Settings screen edits, as the front end last said. A
/// handle held across the session is exactly how a turned-off upscaler kept running.
fn play(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, sync, prefetch, player_theme, .. } = services;
    let EngineState { config, hints, player_tx, .. } = engine;
    let PlayHints { title, next, episodes } = hints.take().unwrap_or_default();
    // A fresh channel per playback. The one playing gives up its window only once
    // this one has something to play — see `Handoff`.
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, title, config));
    let prefetched = prefetch.take(id, &episode, config.playback.translation);
    // Tracker ids rather than trackers: playback only needs to know which queues to
    // append to, and passing the set would couple the player to the sync layer.
    let tracker_ids = signed_in_tracker_ids(sync);
    let extras = (next, episodes);
    if extras.1.is_empty() {
        fill_episodes(id, &episode, services, player_tx);
    }
    spawn_playback(
        id,
        episode,
        None,
        config.syncplay.enabled,
        prx,
        handoff,
        config,
        services,
        &player(config, paths, player_theme),
        tracker_ids,
        extras,
        shader_dir(config, paths),
        prefetched,
        job,
        tx,
    );
}

/// Play the release the user picked in the Sources overlay.
#[allow(clippy::too_many_arguments)]
fn play_source(
    id: AnilistId,
    episode: String,
    provider_id: String,
    source_id: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, sync, player_theme, .. } = services;
    let EngineState { config, hints, player_tx, .. } = engine;
    // Identical to Play except the stream comes from the exact release the user
    // picked — same fresh channel, same session-replacement semantics. Never from the
    // streams found ahead: the pick names its own release.
    let PlayHints { title, next, episodes } = hints.take().unwrap_or_default();
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, title, config));
    let prefetched = None;
    let tracker_ids = signed_in_tracker_ids(sync);
    let extras = (next, episodes);
    if extras.1.is_empty() {
        fill_episodes(id, &episode, services, player_tx);
    }
    spawn_playback(
        id,
        episode,
        Some((provider_id, source_id)),
        config.syncplay.enabled,
        prx,
        handoff,
        config,
        services,
        &player(config, paths, player_theme),
        tracker_ids,
        extras,
        shader_dir(config, paths),
        prefetched,
        job,
        tx,
    );
}

/// Play an episode for a Syncplay party rather than in a session of its own.
fn play_party(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, sync, player_theme, .. } = services;
    let EngineState { config, hints, player_tx, .. } = engine;
    // Same resolution as Play; the handoff at the end is the only difference.
    let PlayHints { title, next, episodes } = hints.take().unwrap_or_default();
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, title, config));
    let prefetched = None;
    let tracker_ids = signed_in_tracker_ids(sync);
    let extras = (next, episodes);
    spawn_playback(
        id,
        episode,
        None,
        true,
        prx,
        handoff,
        config,
        services,
        &player(config, paths, player_theme),
        tracker_ids,
        extras,
        shader_dir(config, paths),
        prefetched,
        job,
        tx,
    );
}

/// Play a downloaded episode from disk, in the full playback session all the same.
#[allow(clippy::too_many_arguments)]
fn play_local(
    id: AnilistId,
    episode: String,
    title: String,
    path: String,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, http, store, sync, slot, player_theme, .. } = services;
    let EngineState { config, hints, player_tx, .. } = engine;
    // No provider walk and no torrent session — the file is on disk — but the full
    // playback session all the same: a downloaded episode records history, resumes,
    // and syncs exactly like a streamed one. It used to go straight to mpv with no
    // context, which made downloads second-class in every way that matters.
    // Nothing to look up, so nothing can fail before it plays.
    let PlayHints { title: named, next, episodes } = hints.take().unwrap_or_default();
    let (prx, handoff) = take_controls(player_tx, successor(id, &episode, named, config));
    handoff.now();
    let tracker_ids = signed_in_tracker_ids(sync);
    if episodes.is_empty() {
        fill_episodes(id, &episode, services, player_tx);
    }
    let shaders = shader_dir(config, paths);
    let (config, store, http, mpv, tx, slot) = (
        config.clone(),
        store.clone(),
        http.clone(),
        player(config, paths, player_theme),
        tx.clone(),
        slot.clone(),
    );
    tokio::spawn(async move {
        let _job = job;
        let playback = config.playback.clone();
        let stream =
            anistream_core::stream::Stream::new(path, anistream_core::stream::StreamKind::Mp4);
        // Best-effort, exactly as the streaming path treats them.
        let resume_at = store
            .resume_position(id, &episode)
            .inspect_err(|e| tracing::warn!(error = %e, "could not read resume position"))
            .unwrap_or(None);
        let mal_id = store.mapping_for(id).ok().flatten().and_then(|m| m.mal_id);
        let context = playback::PlaybackContext {
            anilist_id: id,
            mal_id,
            episode,
            title,
            translation: playback.translation,
            resume_at,
            // A local file's real length is knowable, but the hint only serves
            // the aniskip fallback and a lie would filter its answers.
            runtime_secs: None,
            speed: playback.persist_speed.then_some(playback.persisted_speed).flatten(),
            volume: playback.persist_volume.then_some(playback.persisted_volume).flatten(),
            next,
            episodes,
        };
        let settings = playback::PlaySettings {
            shader_dir: shaders,
            ..play_settings(&config, tracker_ids, config.syncplay.enabled)
        };
        playback::play(vec![stream], context, store, http, mpv, settings, slot, tx, prx).await;
    });
}

/// The releases an episode can be played from, for the Sources overlay.
fn load_sources(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { meta, store, registry, .. } = services;
    let EngineState { config, .. } = engine;
    let (store, registry, meta, tx) =
        (store.clone(), registry.clone(), meta.clone(), tx.clone());
    let translation = config.playback.translation;
    tokio::spawn(async move {
        let _job = job;
        let update =
            match source_slate(&meta, &store, &registry, id, &episode, translation).await {
                Ok(candidates) => Event::Sources { id, episode, candidates },
                Err(reason) => Event::Toast(Notice::alert(reason)),
            };
        let _ = tx.send(update);
    });
}

/// Steer whatever is playing, through the controls of the playback started last.
fn control_player(command: PlayerCommand, engine: &EngineState) {
    let EngineState { player_tx, .. } = engine;
    if let Some(sender) = player_tx.as_ref() {
        let _ = sender.send(command);
    }
}

/// Push what is queued to the trackers and pull their lists, now rather than at the next tick.
///
/// Not through [`spawn`]: sync work needs the tracker set, which `spawn` deliberately does not
/// move into its task.
fn sync_now(job: Option<JobGuard>, services: &Services, tx: &Outbox) {
    let Services { sync, .. } = services;
    let (sync, tx) = (sync.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        tracking::drain_once(&sync, &tx).await;
        tracking::pull_once(&sync, &tx).await;
        let _ = tx.send(Event::Status(String::new()));
    });
}

/// Sign in to a tracker, off the UI thread: it opens a browser and waits.
fn connect(tracker: String, job: Option<JobGuard>, services: &Services, tx: &Outbox) {
    let Services { sync, .. } = services;
    let (sync, tx) = (sync.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        tracking::connect(sync, tracker, tx).await;
    });
}

/// Sign in to Kitsu with a username and password.
fn connect_kitsu(
    username: String,
    password: Secret,
    job: Option<JobGuard>,
    services: &Services,
    tx: &Outbox,
) {
    let Services { sync, .. } = services;
    let (sync, tx) = (sync.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        tracking::connect_kitsu(sync, username, password.0, tx).await;
    });
}

/// Move a title to another of the trackers' lists, queued for the drain.
fn set_status(id: AnilistId, status: LibrarySegment, services: &Services, tx: &Outbox) {
    let Services { sync, .. } = services;
    tracking::set_status(sync, id, status);
    // Queued, not sent — the drain owns the network. Reporting the new depth is what
    // makes the badge move immediately.
    for state in sync.state_after_enqueue() {
        let _ = tx.send(Event::Sync(Box::new(state)));
    }
}

/// Score a title on the trackers, queued for the drain.
fn set_score(id: AnilistId, score: f32, services: &Services, tx: &Outbox) {
    let Services { sync, .. } = services;
    tracking::set_score(sync, id, score);
    for state in sync.state_after_enqueue() {
        let _ = tx.send(Event::Sync(Box::new(state)));
    }
}

/// One of the trackers' lists, as the Library screen shows it.
fn load_library(
    segment: LibrarySegment,
    job: Option<JobGuard>,
    services: &Services,
    tx: &Outbox,
) {
    let Services { store, sync, .. } = services;
    let (sync, tx, store) = (sync.clone(), tx.clone(), store.clone());
    tokio::spawn(async move {
        let _job = job;
        let update = tracking::load_library(&sync, &store, segment).await;
        let _ = tx.send(update);
    });
}

/// Queue one episode for download.
fn download_episode(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { meta, store, registry, .. } = services;
    let EngineState { config, .. } = engine;
    let (store, registry, meta, tx) =
        (store.clone(), registry.clone(), meta.clone(), tx.clone());
    let translation = config.playback.translation;
    tokio::spawn(async move {
        let _job = job;
        // Resolving needs the provider chain, so this is a task rather than inline — the
        // same walk playback does, which is the point: a download is "play it, to disk".
        let update =
            match downloads::enqueue(&store, &registry, &meta, id, &episode, translation).await
            {
                Ok((row, downloads::Queued::New)) => Event::Toast(Notice::info(format!(
                    "queued {} ep {}",
                    row.title, row.episode
                ))),
                Ok((row, downloads::Queued::Existing)) => Event::Toast(Notice::info(format!(
                    "{} ep {} is already in downloads",
                    row.title, row.episode
                ))),
                Err(refusal) => Event::Toast(Notice::alert(refusal.to_string())),
            };
        let _ = tx.send(update);
        let _ = tx.send(Event::Status(String::new()));
        downloads::publish_now(&store, &tx);
    });
}

/// Queue several episodes for download, one after another.
fn download_many(
    id: AnilistId,
    episodes: Vec<String>,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { meta, store, registry, .. } = services;
    let EngineState { config, .. } = engine;
    let (store, registry, meta, tx) =
        (store.clone(), registry.clone(), meta.clone(), tx.clone());
    let translation = config.playback.translation;
    tokio::spawn(async move {
        let _job = job;
        // Sequential enqueues, not parallel: each one is only a resolve + a queue
        // row, and the download manager's own concurrency cap paces the fetches.
        let outcome = downloads::enqueue_each(&episodes, |episode| {
            let (store, registry, meta, tx) = (&store, &registry, &meta, &tx);
            async move {
                let (_, queued) =
                    downloads::enqueue(store, registry, meta, id, &episode, translation)
                        .await?;
                // Each row as it lands, so the Downloads screen fills while the rest
                // resolve rather than saying nothing is queued until the end.
                downloads::publish_now(store, tx);
                Ok(queued)
            }
        })
        .await;
        let _ = tx.send(Event::Toast(outcome.toast()));
    });
}

/// Pause a download, or carry on with one that was paused.
fn download_pause(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // Toggled here rather than in the reducer because the authoritative state is the row,
    // and the reducer only has the flattened projection of it.
    if let Ok(Some(row)) = store.downloads().map(|all| all.into_iter().find(|d| d.id == id)) {
        let next = match row.state {
            anistream_store::DownloadState::Paused => anistream_store::DownloadState::Queued,
            anistream_store::DownloadState::Done | anistream_store::DownloadState::Failed => {
                row.state
            }
            _ => anistream_store::DownloadState::Paused,
        };
        let _ = store.set_download_state(id, next);
    }
    downloads::publish_now(store, tx);
}

/// Cancel a download.
fn download_cancel(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // The row goes first: the manager notices it is gone on its next poll and drops the
    // torrent. Deleting the torrent from here would race with that loop.
    let _ = store.remove_download(id);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::info("download cancelled")));
}

/// Delete a download and its file.
fn download_delete(id: i64, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // Path first, row second: once the row is gone nothing else knows where the
    // file was. A running download's partial file is cleaned up by the manager's
    // own poll, and deleting here as well is a harmless no-op race.
    let path = store
        .downloads()
        .ok()
        .and_then(|all| all.into_iter().find(|d| d.id == id))
        .and_then(|d| d.path);
    let _ = store.remove_download(id);
    let note = match path {
        Some(path) => match std::fs::remove_file(&path) {
            Ok(()) => Notice::info("download and file deleted"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Notice::info("download removed — file was already gone")
            }
            Err(e) => Notice::alert(format!("removed the entry, not the file: {e}")),
        },
        None => Notice::info("download removed"),
    };
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(note));
}

/// Clear the finished downloads from the list.
fn download_clear_completed(services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    let cleared = store.clear_completed_downloads().unwrap_or(0);
    downloads::publish_now(store, tx);
    let _ = tx.send(Event::Toast(Notice::info(format!("cleared {cleared} finished"))));
}

/// Write a setting to the file, and say how that went.
///
/// Written here rather than in the reducer: the reducer has already changed the in-memory
/// config, so the screen is correct whatever the disk does, and this keeps file IO out of a
/// pure function. Synchronous on purpose — one small file, and a settings change that silently
/// lost a race with the next one would be worse than a paused frame.
fn save_setting(
    table: &'static [&'static str],
    key: &'static str,
    value: anistream_core::settings::SettingValue,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, registry, .. } = services;
    let EngineState { saved_config, .. } = engine;
    // Against the file's order, not `config`'s: the reducer has already put the new
    // order in effect there.
    let reordered = (table, key) == (&["providers"][..], "order")
        && matches!(&value, anistream_core::settings::SettingValue::List(order)
            if only_reorders(&saved_config.providers.order, order));
    let written = anistream_core::settings::write_key(paths, table, key, value);
    // Read back whatever the write left, so the Settings screen shows the file rather
    // than its own hopes — and can put a refused setting back. A file that does not
    // load is treated the way startup treats it.
    let _ = tx.send(Event::SettingWritten {
        on_disk: Box::new(Config::load(paths).unwrap_or_default()),
        failed: written.is_err().then_some((table, key)),
    });
    match written {
        Ok(()) => {
            // A toast, named as the Settings screen names it. As a status it stayed
            // in the status line for the rest of the session, over the state it shows,
            // and it said the file's key — "saved commit_threshold". What has no row
            // there, the remembered speed and volume, is saved without a word.
            if let Some(label) = setting_label(table, key) {
                let _ = tx.send(Event::Toast(Notice::info(format!("saved · {label}"))));
            }
            // Republish what the background tasks read, so the next download to
            // finish honours the value that was just written.
            let _ = tx.internal(Internal::SettingsSaved);
            // A setting that redefines what sources exist takes effect now rather
            // than at the next launch. The engine side's loop owns the session and
            // the guard, so it does the work, once what was said above is in; this
            // only says that it is needed. An order that only moves sources is put in
            // effect without a rebuild.
            if reordered {
                let _ = tx.internal(Internal::SourcesReordered);
            } else if redefines_sources(table, key) {
                let _ = tx.internal(Internal::SourcesChanged);
            }
        }
        // The guard reaching the UI: enabling torrents with no VPN configured fails
        // validation here, so no arrow key can write a config that would leak. And a
        // file that did not load at launch is refused by name and line.
        Err(e) => {
            let reason = match e {
                anistream_core::Error::Config(reason) => reason,
                other => other.to_string(),
            };
            let _ = tx.send(Event::Toast(Notice::alert(format!("not saved: {reason}"))));
            // The Providers rows moved when the key was pressed and the chain did not,
            // so the chain's own rows put them back.
            if (table, key) == (&["providers"][..], "order") {
                let _ = tx.send(Event::Providers(data::provider_rows(registry)));
            }
        }
    }
}

/// What a plugin store task needs, moved into the task that runs it.
struct StoreTaskContext {
    stores: plugin_stores::Stores,
    registry: ProviderRegistry,
    config: Config,
    http: HttpClient,
    paths: Paths,
    tx: Outbox,
}

impl StoreTaskContext {
    fn send(&self, update: Event) {
        let _ = self.tx.send(update);
    }

    fn toast(&self, toast: Notice) {
        self.send(Event::Toast(toast));
    }

    /// The store rows as they now stand, from disk.
    fn rows(&self) {
        match self.stores.rows() {
            Ok(rows) => self.send(Event::Stores(rows)),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    async fn members(&self, store: &str) {
        match self.stores.members(store).await {
            Ok((members, invites)) => {
                self.send(Event::StoreMembers { store: store.to_owned(), members, invites });
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// The joined stores, as they are on disk and then as each store says.
    async fn load_stores(self) {
        // What is on disk answers at once; the network fills in behind it.
        self.rows();
        match self.stores.refresh().await {
            Ok(rows) => self.send(Event::Stores(rows)),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// What a pasted invite link would join, asked of the store it names before it is spent.
    async fn preview_store(self, link: Secret) {
        self.send(Event::StorePreview(self.stores.preview(&link.0).await));
    }

    /// Spend an invite link, and join its store.
    async fn join_store(self, link: Secret) {
        match self.stores.join(&link.0).await {
            Ok(joined) => {
                self.rows();
                self.send(Event::StoreJoined { store: joined.id.clone() });
                self.toast(Notice::info(format!(
                    "joined {} as {}",
                    joined.name, joined.member.name
                )));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Leave a store; what was installed from it stays.
    async fn leave_store(self, store: String) {
        match self.stores.leave(&store).await {
            Ok(name) => {
                self.rows();
                self.toast(Notice::info(format!("left {name} — its plugins stay installed")));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Install a store's plugin, or update it, and bring it into the running chain.
    async fn install_plugin(self, store: String, plugin: String) {
        match self.stores.install(&store, &plugin).await {
            Ok((entry, path)) => {
                self.rows();
                self.load_installed(&entry.name, &entry.version, &path).await;
            }
            Err(e) => {
                // The install refreshed the list on its way, so the screen catches up
                // with whatever made it refuse.
                self.rows();
                self.toast(Notice::alert(format!("{plugin} not installed: {e}")));
            }
        }
    }

    /// Delete an installed plugin, and take it out of the chain.
    async fn remove_plugin(self, plugin: String) {
        match self.stores.remove(&plugin).await {
            Ok(()) => {
                self.registry.remove(&plugin);
                self.send(Event::Providers(data::provider_rows(&self.registry)));
                self.rows();
                self.toast(Notice::info(format!("removed {plugin}")));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Make an invite link to a store.
    async fn create_invite(self, store: String, name: Option<String>) {
        match self.stores.invite(&store, name).await {
            Ok(view) => self.send(Event::InviteLink(view)),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// A store's members and its open invites.
    async fn load_members(self, store: String) {
        self.members(&store).await
    }

    /// Take a member's access away, and list who is left.
    async fn remove_member(self, store: String, member: String) {
        match self.stores.remove_member(&store, &member).await {
            Ok(()) => self.toast(Notice::info("removed — their devices stopped working here")),
            Err(e) => self.toast(Notice::alert(e)),
        }
        self.members(&store).await;
    }

    /// Cancel an invite, and list what is left.
    async fn cancel_invite(self, store: String, invite: String) {
        match self.stores.cancel_invite(&store, &invite).await {
            Ok(()) => self.toast(Notice::info("invite cancelled — the link no longer works")),
            Err(e) => self.toast(Notice::alert(e)),
        }
        self.members(&store).await;
    }

    /// The local plugins a store's owner could publish there.
    async fn load_local_plugins(self, store: String) {
        match self.stores.local_plugins(&store).await {
            Ok(plugins) => self.send(Event::LocalPlugins { store, plugins }),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Publish a local plugin to a store.
    async fn publish_plugin(self, store: String, plugin: String) {
        match self.stores.publish(&store, &plugin).await {
            Ok(entry) => {
                self.rows();
                self.toast(Notice::info(format!("published {} {}", entry.name, entry.version)));
            }
            Err(e) => self.toast(Notice::alert(format!("{plugin} not published: {e}"))),
        }
    }

    /// Stop offering a plugin in a store.
    async fn unpublish_plugin(self, store: String, plugin: String) {
        match self.stores.unpublish(&store, &plugin).await {
            Ok(()) => {
                self.rows();
                self.toast(Notice::info(format!("{plugin} is no longer offered")));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Open a store's admin page in the browser, signed in by a link that works once.
    async fn open_admin(self, store: String) {
        match self.stores.admin_link(&store).await {
            Ok(link) => match open::that_detached(&link) {
                Ok(()) => self.toast(Notice::info(
                    "opened the admin page in your browser — the link works once, for ten minutes",
                )),
                Err(e) => self.toast(Notice::alert(format!(
                    "could not open a browser ({e}) — on the store's machine, `anistream store admin` prints a link"
                ))),
            },
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Bring a just-installed plugin into the running chain, so it is usable without a restart.
    async fn load_installed(&self, name: &str, version: &str, path: &std::path::Path) {
        if !sources::plugins_enabled(&self.config) {
            self.toast(Notice::info(format!(
                "installed {name} {version} — \"plugins\" is off in providers.order, so it is not used"
            )));
            return;
        }
        match sources::load_plugin(&self.config, &self.http, &self.paths, path).await {
            Ok(provider) => {
                // Only ever swaps a plugin for a plugin. Whatever the id, a store's download does
                // not get to take a built-in source's place in the chain.
                let id = provider.manifest().id.clone();
                if self.registry.get(&id).is_some_and(|existing| {
                    existing.manifest().kind != anistream_core::traits::ProviderKind::Plugin
                }) {
                    self.toast(Notice::alert(format!(
                        "installed {name}, but {id} is a built-in source's name — not loaded"
                    )));
                    return;
                }
                // Where the order puts it, as the next launch would, rather than at the end. An
                // update was swapped in place and keeps it.
                self.registry.put(provider);
                sources::arrange(&self.registry, &self.config);
                self.send(Event::Providers(data::provider_rows(&self.registry)));
                self.toast(Notice::info(format!("installed {name} {version}")));
            }
            Err(e) => self
                .toast(Notice::alert(format!("installed {name}, but it does not load: {e}"))),
        }
    }
}

/// Run a plugin store task off the UI thread, with what it needs moved into it.
fn store_task<W, F>(
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
    work: W,
) where
    W: FnOnce(StoreTaskContext) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let context = StoreTaskContext {
        stores: services.stores.clone(),
        registry: services.registry.clone(),
        config: engine.config.clone(),
        http: services.http.clone(),
        paths: services.paths.clone(),
        tx: tx.clone(),
    };
    tokio::spawn(async move {
        let _job = job;
        work(context).await;
    });
}

/// Resolve a stream and start playing it.
///
/// Everything slow happens inside the spawned task, behind the eyecatch: matching the title,
/// walking the provider chain, and reading back where you left off.
#[allow(clippy::too_many_arguments)]
fn spawn_playback(
    id: anistream_core::ids::AnilistId,
    episode: String,
    // `(provider_id, source_id)` when the user picked an exact release in the Sources
    // overlay; `None` lets the resolution ladder choose.
    pick: Option<(String, String)>,
    // Hand the resolved stream to a Syncplay party instead of a private session.
    party: bool,
    commands: mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>,
    handoff: Handoff,
    config: &Config,
    services: &Services,
    mpv: &anistream_player::Mpv,
    tracker_ids: Vec<String>,
    // What the player's Up Next card and Episodes menu show, read from the episode table.
    extras: (Option<playback::NextEpisode>, Vec<playback::EpisodeEntry>),
    shader_dir: Option<std::path::PathBuf>,
    // This episode's streams, found ahead while the previous one's ending played.
    prefetched: Option<Resolved>,
    job: Option<JobGuard>,
    tx: &Outbox,
) {
    let (config, meta, store, registry, http, mpv, slot, tx) = (
        config.clone(),
        services.meta.clone(),
        services.store.clone(),
        services.registry.clone(),
        services.http.clone(),
        mpv.clone(),
        services.slot.clone(),
        tx.clone(),
    );

    tokio::spawn(async move {
        let playback = config.playback.clone();
        let resolved = match prefetched {
            Some(found) => {
                tracing::info!(episode = %episode, "playing from the streams found ahead");
                Ok(found)
            }
            None => {
                resolve_for_playback(
                    &meta,
                    &store,
                    &registry,
                    id,
                    &episode,
                    pick,
                    &playback,
                    job.as_ref(),
                    &tx,
                )
                .await
            }
        };
        let context = match resolved {
            Ok(pair) => pair,
            // The episode being watched carries on, and the window with it: only the lookup
            // failed, and closing what was playing over it would punish the viewer for asking.
            Err(reason) if handoff.live() => {
                let _ = tx.send(Event::Toast(Notice::alert(reason.clone())));
                let _ = tx.send(Event::PlaybackKept);
                handoff.keep(commands, reason).await;
                return;
            }
            Err(reason) => {
                // The failure ladder's first rung: name the failure rather than showing an
                // empty screen. Releasing the eyecatch is what the alert does for us.
                let _ = tx.send(Event::Toast(Notice::alert(reason)));
                // A window left open for this episode would wait for nothing.
                slot.release();
                let _ = tx.send(Event::PlaybackEnded {
                    watched: false,
                    ending: anistream_model::playback::Ending::Failed,
                });
                // The playback this one took over left its "playing" line up for it to replace.
                let _ = tx.send(Event::Status(String::new()));
                return;
            }
        };
        let (streams, mut context) = context;
        (context.next, context.episodes) = extras;
        // Resolved: from here the wait is mpv opening, which Now Playing shows itself, and the
        // session lasts the episode — far too long to hold a "starting" job open for.
        drop(job);
        // Say why this release, so an automatic pick is never a mystery. The active-provider
        // header is sent from playback itself, per attempt, so failover never credits the
        // stream that just failed.
        if let Some(note) = streams.first().and_then(|s| s.pick_note.clone()) {
            let _ = tx.send(Event::Toast(Notice::info(note)));
        }

        let settings =
            playback::PlaySettings { shader_dir, ..play_settings(&config, tracker_ids, party) };
        handoff.complete().await;
        playback::play(streams, context, store, http, mpv, settings, slot, tx, commands).await;
    });
}

/// Everything needed before mpv can be spawned: a stream, and the context history needs.
#[allow(clippy::too_many_arguments)]
async fn resolve_for_playback(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
    episode: &str,
    pick: Option<(String, String)>,
    playback: &anistream_core::config::PlaybackConfig,
    job: Option<&JobGuard>,
    tx: &Outbox,
) -> std::result::Result<(Vec<anistream_core::stream::Stream>, playback::PlaybackContext), String>
{
    // The step under way, on the job when there is one; the CLI has no activity row and reads
    // the status channel instead.
    let step = |text: String| match job {
        Some(job) => job.note(text, None),
        None => {
            let _ = tx.send(Event::Status(text));
        }
    };
    if registry.is_empty() {
        return Err("no sources configured — see the Providers screen".into());
    }
    // Phase-labelled, because the wait between Enter and mpv is where every tool in
    // this category earns its "is it frozen?" issues. The status line names the rung
    // being climbed; the eyecatch's own timer says how long it has taken.
    step("looking up the title".into());
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    let now = now_epoch();

    // "Use this source for this series", applied *before* the title is matched.
    //
    // The order matters: a `ProviderKey` is only meaningful to the provider that issued it, so
    // matching against the whole chain and then resolving against a pinned source would hand one
    // provider's key to another and look like a missing episode.
    //
    // Skipped when the user picked an exact release from the Sources overlay. That pick is the
    // more immediate instruction of the two, it already names its own provider, and honouring the
    // remembered preference instead would refuse a release the user is looking straight at.
    //
    // A preference naming a provider that is not registered — a plugin removed, or one still
    // compiling in the background — falls back to the normal chain. Refusing to play because a
    // remembered choice is momentarily unresolvable would be a worse answer than playing.
    let preference =
        pick.is_none().then(|| store.provider_preference(id).ok().flatten()).flatten();
    let pinned = preference.as_deref().and_then(|id| registry.pinned(id));
    if let Some(name) = &preference
        && pinned.is_none()
    {
        tracing::debug!(provider = %name, "pinned source is not registered, using the chain");
    }
    let registry = pinned.as_ref().unwrap_or(registry);
    let target = media.match_target();

    step(format!("searching sources for ep {episode}"));
    let mut streams = match &pick {
        // A picked release goes straight to its provider, no failover: substituting a
        // different stream for the one the user chose would silently undo the choice. Matched
        // on that provider, whose key it is.
        Some((provider_id, source_id)) => {
            let source = registry
                .pinned(provider_id)
                .ok_or_else(|| format!("{provider_id} is not a source any more"))?;
            let resolution = anistream_providers::resolve(
                store,
                &source,
                id,
                &target,
                playback.translation,
                now,
            )
            .await;
            let key = resolution.key().cloned().ok_or_else(|| {
                format!("could not match this title: {}", resolution.explain())
            })?;
            source
                .resolve_source(
                    provider_id,
                    &key,
                    episode,
                    playback.translation,
                    source_id,
                    now,
                )
                .await
                .map_err(|e| format!("could not start that release: {e}"))?
        }
        None => {
            let answer = anistream_providers::first_answer(
                store,
                registry,
                id,
                &target,
                playback.translation,
                now,
                |source, key| async move {
                    source.resolve(&key, episode, playback.translation, now).await
                },
            )
            .await;
            match answer {
                anistream_providers::Answer::Answered { value, .. } => value,
                anistream_providers::Answer::Ambiguous { .. } => {
                    return Err(format!("could not match this title: {}", answer.explain()));
                }
                anistream_providers::Answer::Unanswered { .. } => return Err(answer.explain()),
            }
        }
    };

    // Toward the configured quality, preferring a step down over upscaling. The whole list is
    // returned rather than the winner: the sort is a preference, and playback falls over to
    // the next entry when a stream turns out not to decode.
    streams.sort_by_key(|s| s.quality_rank(playback.quality));
    if streams.is_empty() {
        return Err("no playable stream".into());
    }

    // Resume and skip data are both best-effort: a database hiccup or a missing MAL id must
    // not stop an episode from playing.
    let resume_at = store
        .resume_position(id, episode)
        .inspect_err(|e| tracing::warn!(error = %e, "could not read resume position"))
        .unwrap_or(None);
    let mal_id = store.mapping_for(id).ok().flatten().and_then(|m| m.mal_id);

    let context = playback::PlaybackContext {
        anilist_id: id,
        mal_id,
        episode: episode.to_owned(),
        // The name the terminal shows the title by, so the player's top bar, the window and
        // the terminal agree. The match target's first title is whichever the matcher tries
        // first, which is often the romaji one.
        title: media.title.display().to_owned(),
        translation: playback.translation,
        resume_at,
        runtime_secs: media.duration.map(|minutes| minutes * 60),
        speed: playback.persist_speed.then_some(playback.persisted_speed).flatten(),
        volume: playback.persist_volume.then_some(playback.persisted_volume).flatten(),
        // Filled in by the caller, which has the episode table; headless play has none.
        next: None,
        episodes: Vec::new(),
    };
    Ok((streams, context))
}

/// The selectable releases for one episode, across every usable provider.
///
/// Shares the title-matching rung with playback: the slate is only meaningful for the
/// provider key the ladder would actually use.
async fn source_slate(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
    episode: &str,
    translation: anistream_core::media::Translation,
) -> std::result::Result<Vec<anistream_core::traits::SourceCandidate>, String> {
    if registry.is_empty() {
        return Err("no sources configured — see the Providers screen".into());
    }
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    let target = media.match_target();
    let now = now_epoch();
    // Each provider's releases under its own key: one provider's key means nothing to another.
    // A provider that cannot match the title has nothing to offer here, and the rest still do.
    let mut slate = Vec::new();
    let mut reasons = Vec::new();
    for provider_id in registry.usable_ids() {
        let Some(source) = registry.pinned(&provider_id) else { continue };
        let resolution =
            anistream_providers::resolve(store, &source, id, &target, translation, now).await;
        match resolution.key() {
            Some(key) => slate.extend(source.sources(key, episode, translation, now).await),
            None => reasons.push(format!("{provider_id}: {}", resolution.explain())),
        }
    }
    if slate.is_empty() && !reasons.is_empty() {
        return Err(format!("could not match this title: {}", reasons.join(", ")));
    }
    Ok(slate)
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

/// Ask the terminal to put `text` on the clipboard (OSC 52). Returns whether the request was
/// written; whether the terminal honours it is up to its settings, which is why the link is also
/// shown in full.
fn copy_to_clipboard(out: &mut impl io::Write, text: &str) -> bool {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    write!(out, "\x1b]52;c;{encoded}\x07").and_then(|()| out.flush()).is_ok()
}

/// Log to a file, never to the terminal.
///
/// Anything written to stdout would corrupt the alternate screen, so diagnostics go to disk
/// and the Logs overlay reads them back.
fn init_logging(paths: &Paths) {
    use tracing_subscriber::{EnvFilter, fmt};

    let dir = paths.cache_dir.join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let appender = tracing_appender::rolling::daily(&dir, "anistream.log");
    let _ = fmt()
        .with_writer(appender)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_env("ANISTREAM_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

/// The queues a finished episode should be appended to: signed-in trackers only.
///
/// Signed-in rather than enabled, because Kitsu is enabled out of the box — queueing for
/// an account that may never exist would grow a badge nobody can act on. A signed-in
/// tracker that is merely offline still queues, which is the case the outbox exists for;
/// and a tracker signed into *later* catches up anyway, since the first pull reconciles
/// progress out of the watch log.
fn signed_in_tracker_ids(sync: &tracking::Sync) -> Vec<String> {
    sync.trackers.iter().filter(|t| t.is_authenticated()).map(|t| t.id().to_owned()).collect()
}

/// The name the Settings screen gives the setting stored at `table`.`key`, for saying it was
/// saved. The config key (`commit_threshold`) is the file's vocabulary, not the screen's.
///
/// `None` for what is stored without a row of its own, the remembered speed and volume: saved
/// in passing, and not something anyone asked to have confirmed.
///
/// A table of its own, so the engine side can name a setting without asking the interface,
/// and held to the Settings screen's own rows by a test here, the one place that sees both.
/// P4 moves the settings' ids and labels into the model, and this reads them from there.
fn setting_label(table: &[&str], key: &str) -> Option<&'static str> {
    SETTING_LABELS.iter().find(|(t, k, _)| *t == table && *k == key).map(|(_, _, label)| *label)
}

/// Every setting the interface writes, by table and key, with the name its row goes by.
const SETTING_LABELS: &[(&[&str], &str, &str)] = &[
    // No row on Settings: `K` and `J` on Providers change it.
    (&["providers"], "order", "source order"),
    (&["theme"], "mode", "theme"),
    (&["theme"], "motion", "motion"),
    (&["playback"], "translation", "translation"),
    (&["playback"], "quality", "quality"),
    (&["playback"], "subtitle_language", "subtitles"),
    (&["playback"], "commit_threshold", "counts as watched at"),
    (&["playback"], "auto_next", "auto-play next"),
    (&["playback"], "skip_opening", "skip opening automatically"),
    (&["playback"], "skip_filler", "skip filler automatically"),
    (&["playback"], "mark_chapters", "mark opening and ending"),
    (&["playback"], "fullscreen", "start fullscreen"),
    (&["playback"], "upscaling", "upscaling"),
    (&["playback"], "player_ui", "player interface"),
    (&["playback"], "night_mode", "night mode"),
    (&["downloads"], "directory", "download folder"),
    (&["downloads"], "merge_subtitles", "merge subtitles in"),
    (&["downloads"], "keep_seeding", "keep seeding"),
    (&["downloads"], "on_complete", "after each download"),
    (&["providers", "torrent"], "enabled", "torrent source"),
    (&["providers", "torrent", "seeding"], "ratio", "seed after watching"),
    (&["providers", "torrent", "seeding"], "hours", "seed for at most"),
    (&["providers", "torrent", "seeding"], "upload_limit_kib", "upload limit"),
    (&["presence"], "enabled", "discord presence"),
    (&["presence"], "show_title", "presence shows the title"),
    (&["syncplay"], "enabled", "syncplay parties"),
    (&["syncplay"], "server", "syncplay server"),
    (&["syncplay"], "room", "syncplay room"),
    (&["updates"], "check", "check for updates"),
];

/// Whether writing this key changes which sources exist, and so needs the chain rebuilt.
///
/// Listed rather than inferred: a rebuild tears down a live torrent session, which is far
/// too blunt to trigger on a guess about a key name.
fn redefines_sources(table: &[&str], key: &str) -> bool {
    matches!(
        (table, key),
        (["providers", "torrent"], "enabled")
            | (["providers", "torrent", "vpn"], "mode")
            | (["providers"], "order" | "disabled" | "remote_url")
    )
}

/// What one playback runs under, from the config as it stands now.
fn play_settings(
    config: &Config,
    tracker_ids: Vec<String>,
    party: bool,
) -> playback::PlaySettings {
    let playback = &config.playback;
    playback::PlaySettings {
        threshold: playback.commit_threshold,
        auto_skip: playback.skip_opening,
        mark_chapters: playback.mark_chapters,
        fullscreen: playback.fullscreen,
        subtitle_language: Some(playback.subtitle_language.clone()),
        auto_next: playback.auto_next,
        night_mode: playback.night_mode,
        remember_tracks: playback.persist_speed,
        look: playback.upscaling.into(),
        // Filled in by the caller, which knows where the cache is.
        shader_dir: None,
        tone_mapping: playback.tone_mapping.clone(),
        tracker_ids,
        presence: config.presence.clone(),
        syncplay: config.syncplay.clone(),
        party,
    }
}

/// What a play needs of what the interface shows, sent ahead of it as `PlayHints`: the name the
/// window it takes over announces it by, and what the player's Up Next card and Episodes menu
/// show. `None` for anything that does not play.
///
/// The front end's half of the seam, since the episode table and Now Playing are the `App`'s.
/// Temporary, like `PlayHints`: P4 has the engine answer it from its own record of the titles.
fn play_hints(app: &App, task: &Command) -> Option<Command> {
    let (Command::Play { id, episode }
    | Command::PlaySource { id, episode, .. }
    | Command::PlayParty { id, episode }
    | Command::PlayLocal { id, episode, .. }) = task
    else {
        return None;
    };
    let (next, episodes) = episode_extras(app, *id, episode);
    Some(Command::PlayHints { title: successor_title(app, *id), next, episodes })
}

/// The name a new playback of `id` goes by on screen, for the window it takes over to say.
fn successor_title(app: &App, id: anistream_core::ids::AnilistId) -> String {
    // Now Playing is staged for the new playback before it is dispatched, so its title is the
    // one the terminal shows; a watch party stages nothing, and the open title is the one.
    app.playing
        .as_ref()
        .filter(|playing| playing.id == Some(id))
        .map(|playing| playing.title.clone())
        .or_else(|| app.detail.as_ref().filter(|e| e.id == id).map(|e| e.title.clone()))
        .unwrap_or_default()
}

/// What the player's Up Next card and Episodes menu need from the episode table.
fn episode_extras(
    app: &App,
    id: anistream_core::ids::AnilistId,
    episode: &str,
) -> (Option<playback::NextEpisode>, Vec<playback::EpisodeEntry>) {
    let next = app.next_up(id, episode).map(|row| playback::NextEpisode {
        number: row.number.clone(),
        title: row.title.clone(),
        thumbnail: row.thumbnail.clone(),
    });
    let rows = app
        .episode_rows_for(id)
        .iter()
        .map(|row| playback::EpisodeEntry {
            number: row.number.clone(),
            title: row.title.clone(),
            watched: row.completed,
            filler: row.skippable,
        })
        .collect();
    (next, rows)
}

/// Load a title's episode list for a playback the episode table on screen could not describe.
///
/// The table belongs to whichever title is open, so a playback started from Home or Continue
/// had no episode list: the player's Episodes menu was empty and, with no next episode known,
/// there was no Up Next card, no fetching ahead and no keeping the window for the binge.
/// Loaded in the background after playback starts, never in front of it, and handed to the
/// playback through its controls.
fn fill_episodes(
    id: anistream_core::ids::AnilistId,
    episode: &str,
    services: &Services,
    controls: &Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
) {
    let Some(controls) = controls.clone() else { return };
    let (meta, store, registry, episode) = (
        services.meta.clone(),
        services.store.clone(),
        services.registry.clone(),
        episode.to_owned(),
    );
    tokio::spawn(async move {
        match load_episodes(&meta, &store, &registry, id).await {
            Ok(EpisodeLoad::Rows(rows)) if !rows.is_empty() => {
                let next = following(&rows, &episode);
                let _ = controls
                    .send(anistream_model::command::PlayerCommand::Episodes { rows, next });
            }
            Ok(_) => {}
            Err(reason) => tracing::debug!(%reason, "no episode list for the player"),
        }
    });
}

/// The episode after `episode` in a list, when the list has it.
fn following(rows: &[EpisodeRow], episode: &str) -> Option<String> {
    let next = episode.trim().parse::<u32>().ok()?.checked_add(1)?.to_string();
    rows.iter().any(|row| row.number == next).then_some(next)
}

/// A fresh control channel for a new playback, and the handoff from the playback it replaces.
///
/// The old playback, once told, parks its window rather than closing it, and the new one claims
/// it — which is how pressing N inside the player stays in the same window.
fn take_controls(
    player_tx: &mut Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
    successor: anistream_model::playback::Successor,
) -> (mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>, Handoff) {
    let previous = player_tx.take();
    let (ptx, prx) = mpsc::unbounded_channel();
    *player_tx = Some(ptx);
    (prx, Handoff { previous, successor })
}

/// How long a new playback waits for the old one to park its window before opening its own.
/// Bounded, because a playback still starting reads its controls only once it is playing.
const HANDOVER_WAIT: Duration = Duration::from_secs(10);

/// The playback a new one takes the window from, told only once the new one has something to
/// play. Handing over at once closed the window on the episode being watched whenever the new
/// one's lookup then failed, and a timeout was enough for that.
struct Handoff {
    previous: Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
    successor: anistream_model::playback::Successor,
}

impl Handoff {
    /// Whether a playback is still running in the window.
    fn live(&self) -> bool {
        self.previous.as_ref().is_some_and(|p| !p.is_closed())
    }

    /// Ask for the window without waiting: nothing can fail between here and playing.
    fn now(self) {
        if let Some(previous) = self.previous {
            let _ = previous
                .send(anistream_model::command::PlayerCommand::Handover(self.successor));
        }
    }

    /// Ask for the window and wait until it is parked: claiming it any earlier finds the slot
    /// empty and opens a second window beside it.
    async fn complete(self) {
        let Some(previous) = self.previous else { return };
        if previous
            .send(anistream_model::command::PlayerCommand::Handover(self.successor))
            .is_ok()
        {
            let _ = tokio::time::timeout(HANDOVER_WAIT, previous.closed()).await;
        }
    }

    /// The new playback found nothing, so the old one keeps the window: say why on the video
    /// it is showing, and pass on the controls the terminal now sends to the new channel until
    /// the old playback ends. A later playback's handover reaches it the same way.
    async fn keep(
        self,
        mut controls: mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>,
        reason: String,
    ) {
        use anistream_model::command::PlayerCommand;
        let Some(previous) = self.previous else { return };
        let _ = previous.send(PlayerCommand::Alert(reason));
        loop {
            tokio::select! {
                command = controls.recv() => match command {
                    // The list loaded for the episode that is not playing: it would give the one
                    // that is the wrong neighbours.
                    Some(PlayerCommand::Episodes { .. }) => {}
                    Some(command) => {
                        if previous.send(command).is_err() {
                            break;
                        }
                    }
                    None => break,
                },
                () = previous.closed() => break,
            }
        }
    }
}

/// What a new playback is about to play, as the window it takes over says while it is found:
/// named as the front end said ahead of the play, in the translation in effect.
fn successor(
    id: anistream_core::ids::AnilistId,
    episode: &str,
    title: String,
    config: &Config,
) -> anistream_model::playback::Successor {
    anistream_model::playback::Successor {
        id,
        title,
        episode: episode.to_owned(),
        translation: config.playback.translation,
    }
}

/// What an older mpv costs, in a sentence, or `None` when it costs nothing.
///
/// Ubuntu 24.04 ships 0.37 and Debian 12 ships 0.35, and the `.deb` installs whichever the system
/// has, so this is a common case rather than an edge one.
fn old_mpv_note(version: anistream_player::protocol::MpvVersion) -> Option<String> {
    (!version.has_companion_support()).then(|| {
        format!(
            "mpv {version} plays, but anistream's player (skip button, Up Next, menus, \
             thumbnails) needs mpv 0.36 or newer"
        )
    })
}

#[cfg(test)]
mod contract;
#[cfg(test)]
mod harness;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new(Config::default(), theme::Palette::dark(), Keymap::new())
    }

    #[test]
    fn the_engine_side_hears_of_a_config_change_before_the_next_command() {
        let mut app = test_app();
        let mut seam = ConfigSeam::default();
        assert!(matches!(seam.moved(&app), Some(Command::UseConfig { .. })), "said at once");
        assert_eq!(seam.moved(&app), None, "and not again while nothing moves");

        app.config.playback.auto_next = false;
        let Some(Command::UseConfig { config, saved }) = seam.moved(&app) else {
            panic!("a change in effect is said");
        };
        assert!(!config.playback.auto_next);
        assert!(saved.playback.auto_next, "the file is told apart from what is in effect");

        // A write read back, or one put back, moves only the file's half.
        app.saved_config.playback.auto_next = false;
        assert!(seam.moved(&app).is_some());
    }

    #[test]
    fn a_play_is_sent_with_what_the_screen_knows_of_its_title() {
        use anistream_core::ids::AnilistId;
        let id = AnilistId::new(7);
        let mut app = test_app();
        let play = Command::Play { id, episode: "2".into() };
        // Nothing of it on screen: nothing to announce, and a list the playback loads itself.
        let nothing =
            Command::PlayHints { title: String::new(), next: None, episodes: Vec::new() };
        assert_eq!(play_hints(&app, &play), Some(nothing));
        assert_eq!(play_hints(&app, &Command::LoadEpisodes(id)), None, "only a play has hints");

        // The title open, as opening it from a list leaves it.
        app.detail = Some(anistream_model::catalog::Entry::new(id, "Hoshikuzu no Tabibito"));
        let row = |n: &str| EpisodeRow {
            number: n.into(),
            title: Some(format!("Part {n}")),
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: n == "1",
            kind: None,
            skippable: false,
        };
        app.apply(Event::Episodes { id, rows: vec![row("1"), row("2"), row("3")] });
        let local = Command::PlayLocal {
            id,
            episode: "2".into(),
            title: "Hoshikuzu no Tabibito".into(),
            path: "/downloads/2.mp4".into(),
        };
        for task in [play, local] {
            let Some(Command::PlayHints { title, next, episodes }) = play_hints(&app, &task)
            else {
                panic!("{task:?} has hints");
            };
            assert_eq!(title, "Hoshikuzu no Tabibito");
            assert_eq!(next.map(|n| n.number).as_deref(), Some("3"));
            let listed: Vec<(&str, bool)> =
                episodes.iter().map(|e| (e.number.as_str(), e.watched)).collect();
            assert_eq!(listed, [("1", true), ("2", false), ("3", false)]);
        }
    }

    #[tokio::test]
    async fn a_lookup_that_fails_leaves_the_controls_with_the_episode_playing() {
        use anistream_model::command::PlayerCommand;
        let (previous, mut playing) = mpsc::unbounded_channel();
        let mut player_tx = Some(previous);
        let successor = anistream_model::playback::Successor {
            id: anistream_core::ids::AnilistId::new(1),
            title: "A Show".into(),
            episode: "5".into(),
            translation: anistream_core::media::Translation::Sub,
        };
        let (controls, handoff) = take_controls(&mut player_tx, successor);
        assert!(handoff.live());
        assert!(playing.try_recv().is_err(), "the window was asked for before 5 was found");

        let terminal = player_tx.clone().unwrap();
        let kept = tokio::spawn(handoff.keep(controls, "a: timed out".into()));
        terminal.send(PlayerCommand::TogglePause).unwrap();
        terminal.send(PlayerCommand::Episodes { rows: Vec::new(), next: None }).unwrap();
        terminal.send(PlayerCommand::Stop).unwrap();
        assert_eq!(playing.recv().await, Some(PlayerCommand::Alert("a: timed out".into())));
        assert_eq!(playing.recv().await, Some(PlayerCommand::TogglePause));
        assert_eq!(playing.recv().await, Some(PlayerCommand::Stop), "5's list passed on to 4");

        // It stands aside once the playback it kept has ended.
        drop(playing);
        tokio::time::timeout(Duration::from_secs(5), kept).await.unwrap().unwrap();
    }

    #[test]
    fn the_player_wears_the_immersive_palette_whatever_the_terminal_is() {
        // What the companion is handed, which mpv's arguments are spelled from word for word:
        // `--script-opts-append=anistream-<key>=<value>`, and the font as `--osd-font` too.
        let theme: Vec<(&str, &str)> = vec![
            ("text", "#E4E0D6"),
            ("dim", "#9298B5"),
            ("obi", "#F2A64B"),
            ("state", "#7FC7D9"),
            ("alert", "#E06B54"),
            ("ground", "#161A2E"),
            ("font", "Zen Kaku Gothic New"),
        ];
        let given = player_theme().options();
        let given: Vec<(&str, &str)> =
            given.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(given, theme);
    }

    #[test]
    fn the_player_reflects_the_config_as_it_stands_now() {
        // The bug this encodes: the handle was built once at launch, so turning Anime4K
        // off left `--glsl-shaders` in the arguments of every episode for the rest of the
        // session. Shaders are now set per file over IPC and never ride the command line,
        // but what does — the HDR curve — must still follow the settings screen.
        // All three under a temporary directory: filling the rest from `Paths::resolve()`
        // handed the player the real data directory of whoever ran the tests.
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let mut config = Config::default();

        config.playback.upscaling = anistream_core::config::Upscaling::Sharp;
        config.playback.tone_mapping = "bt.2390".into();
        let player_now = player(&config, &paths, &player_theme());
        let args = player_now.args_for_test();
        assert!(!args.iter().any(|a| a.starts_with("--glsl-shaders")), "{args:?}");
        assert!(args.iter().any(|a| a == "--tone-mapping=bt.2390"), "{args:?}");

        config.playback.tone_mapping = "auto".into();
        let args = player(&config, &paths, &player_theme()).args_for_test().to_vec();
        assert!(!args.iter().any(|a| a.starts_with("--tone-mapping")), "auto is mpv's own");

        // A chain of the user's own is theirs: no runtime chain may replace it.
        assert!(shader_dir(&config, &paths).is_some());
        config.playback.mpv_args = vec!["--glsl-shaders=~~/mine.glsl".into()];
        assert!(shader_dir(&config, &paths).is_none());
    }

    #[test]
    fn the_following_episode_is_only_offered_when_the_list_has_it() {
        let rows: Vec<EpisodeRow> = ["1", "2", "3"]
            .into_iter()
            .map(|n| EpisodeRow {
                number: n.into(),
                title: None,
                thumbnail: None,
                description: None,
                duration_secs: None,
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
            })
            .collect();
        assert_eq!(following(&rows, "1").as_deref(), Some("2"));
        assert_eq!(following(&rows, "3"), None, "a finale has no next");
        assert_eq!(following(&rows, "OVA"), None);
    }

    #[test]
    fn one_command_per_run_and_no_option_without_its_command() {
        use clap::error::ErrorKind;
        let parse = |args: &[&str]| {
            Cli::try_parse_from(std::iter::once("anistream").chain(args.iter().copied()))
        };
        let refused = |args: &[&str]| parse(args).map(|_| ()).unwrap_err().kind();

        // Two commands were resolved by the order of the checks in `main`, silently.
        assert_eq!(refused(&["--stats", "--random"]), ErrorKind::ArgumentConflict);
        assert_eq!(refused(&["--doctor", "--plugins"]), ErrorKind::ArgumentConflict);
        assert_eq!(refused(&["--continue", "--play", "1"]), ErrorKind::ArgumentConflict);

        // Alone, these started the interface as if they had not been typed.
        for alone in [
            &["--json"][..],
            &["--screen", "title"],
            &["--episode", "3"],
            &["--tracker", "mal"],
        ] {
            assert_eq!(refused(alone), ErrorKind::MissingRequiredArgument, "{alone:?}");
        }

        // `--json` means nothing to these, and the refusal names the command it was given with.
        for (args, command) in
            [(&["--doctor", "--json"][..], "--doctor"), (&["--plugins", "--json"], "--plugins")]
        {
            let error = parse(args).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{args:?}");
            assert!(error.to_string().contains(command), "{error}");
        }

        // A command's flags beside a subcommand were dropped without a word.
        assert_eq!(
            refused(&["--stats", "plugin", "stamp", "a.wasm"]),
            ErrorKind::ArgumentConflict
        );
        assert!(parse(&["plugin", "stamp", "a.wasm"]).is_ok());
        for fine in [
            &["--stats", "--json"][..],
            &["--search", "summer", "--json"],
            &["--preview", "80x24", "--screen", "seasonal"],
            &["--play", "1", "--episode", "OVA"],
            &["--login", "--tracker", "mal"],
            &["--logout", "--tracker", "kitsu"],
            &[],
        ] {
            assert!(parse(fine).is_ok(), "{fine:?}: {:?}", parse(fine).err());
        }
    }

    #[test]
    fn a_preview_must_name_a_size_and_a_screen_that_exist() {
        use clap::error::ErrorKind;
        let parse = |args: &[&str]| {
            Cli::try_parse_from(std::iter::once("anistream").chain(args.iter().copied()))
        };
        // Each of these rendered Home at 120x34, or nothing, with exit status 0.
        for wrong in [&["--preview", "abc"][..], &["--preview", "0x0"], &["--preview", "120x"]]
        {
            assert_eq!(
                parse(wrong).unwrap_err().kind(),
                ErrorKind::ValueValidation,
                "{wrong:?}"
            );
        }
        assert_eq!(
            parse(&["--preview", "80x24", "--screen", "bogus"]).unwrap_err().kind(),
            ErrorKind::InvalidValue
        );
        let cli = parse(&["--preview", "80X24", "--screen", "episodes"]).unwrap();
        assert_eq!(cli.preview, Some((80, 24)));
        assert_eq!(cli.screen, Some(PreviewScreen::Episodes));
    }

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

    #[test]
    fn only_source_defining_keys_force_a_rebuild() {
        // A rebuild tears down a live torrent session, so it must not fire on a setting
        // that has nothing to do with which sources exist.
        assert!(redefines_sources(&["providers", "torrent"], "enabled"));
        assert!(redefines_sources(&["providers", "torrent", "vpn"], "mode"));
        assert!(redefines_sources(&["providers"], "order"));

        assert!(!redefines_sources(&["playback"], "upscaling"));
        assert!(!redefines_sources(&["playback"], "quality"));
        assert!(!redefines_sources(&["downloads"], "directory"));
        assert!(!redefines_sources(&["theme"], "mode"));
    }

    // The work's words held to the interface's. Only the binary sees both sides, so these live
    // here rather than beside the work they check.

    #[test]
    fn a_verdict_reads_on_the_badge_as_the_guard_words_it() {
        use anistream_providers::vpn::GuardState;

        // Every verdict a check can reach. Not yet checked is said before any check runs, and
        // does not alert: nothing runs until it passes.
        let egress = anistream_providers::vpn::Egress {
            asn_org: Some("Tunnel Example AB".into()),
            country: Some("Sweden".into()),
            ..Default::default()
        };
        for state in [
            GuardState::Protected { egress: Box::new(egress) },
            GuardState::Leaking { reason: "egress is the home network".into() },
            GuardState::Unreachable { reason: "the proxy did not answer".into() },
            GuardState::Unprotected,
        ] {
            let badge = anistream_tui::app::vpn_badge(&sources::guard_view(&state));
            assert_eq!(badge, (state.badge(), !state.is_protected()), "{state:?}");
        }
        let checking =
            anistream_tui::app::vpn_badge(&sources::guard_view(&GuardState::Unverified));
        assert_eq!(checking, (GuardState::Unverified.badge(), false));
    }

    #[test]
    fn every_kind_the_filler_index_knows_reads_as_the_index_words_it() {
        for kind in anistream_meta::filler::EpisodeKind::ALL {
            let shown = anistream_tui::screens::episode_kind_label(data::row_kind(kind));
            assert_eq!(shown, kind.label(), "{kind:?}");
        }
        assert_eq!(
            anistream_tui::screens::episode_kind_label(
                anistream_model::episodes::EpisodeKind::Filler
            ),
            "filler"
        );
    }

    #[test]
    fn a_saved_setting_is_named_as_the_settings_screen_names_it() {
        let app = test_app();
        // Every row that writes, and the order, which Providers writes.
        let mut written: Vec<(&[&str], &str)> = app
            .setting_rows()
            .iter()
            .filter_map(|row| row.editable.as_ref().map(|edit| (edit.table, edit.key)))
            .collect();
        written.push((&["providers"], "order"));
        for (table, key) in &written {
            let named = setting_label(table, key);
            assert!(named.is_some(), "{table:?}.{key} is saved without a word");
            assert_eq!(named, app.setting_label(table, key), "{table:?}.{key}");
        }
        // And nothing named that the interface does not write.
        for (table, key, _) in SETTING_LABELS {
            assert!(written.contains(&(*table, *key)), "{table:?}.{key} has no row");
        }
        // Saved in passing, from the player.
        for key in ["persisted_speed", "persisted_volume"] {
            assert_eq!(setting_label(&["playback"], key), None, "{key}");
        }
    }

    #[test]
    fn a_skip_offered_in_the_terminal_reads_as_the_player_words_it() {
        use anistream_player::SkipKind;

        for kind in [SkipKind::Opening, SkipKind::Ending] {
            assert_eq!(
                anistream_tui::screens::skip_label(playback::offered(kind)),
                kind.label()
            );
        }
    }
}
