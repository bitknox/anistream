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
    background::datasets::keep_datasets_current,
    data,
    dispatch::{dispatch, handle_internal, meta_notice, receive},
    jobs::JobGuard,
    outbox::{Envelope, Internal, Outbox},
    playback::{self, player::player},
    services::Services,
    sources,
    state::EngineState,
    tracking,
};

use std::{io, time::Duration};

use anistream_core::config::{Config, Paths, ThemeMode};
use anistream_model::{command::Command, event::Event, jobs::JobId, notice::Notice};
use anistream_net::HttpClient;
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
    use anistream_engine::playback::player::shader_dir;
    use anistream_engine::settings::{SETTING_LABELS, setting_label};
    use anistream_model::episodes::EpisodeRow;

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
