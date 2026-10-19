//! anistream — an anime streaming TUI.

mod cli;
mod covers;
mod services;
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
use services::build_meta;

use anistream_engine::{
    artwork, data, downloads, playback,
    player::{player, shader_dir},
    plugin_stores,
    slot::PlayerSlot,
    sources, tracking,
};

use std::{io, sync::Arc, time::Duration};

use anistream_core::config::{Config, Paths, ThemeMode};
use anistream_meta::{Meta, dataset};
use anistream_model::{
    catalog::{Content, ContentFor},
    command::Command,
    episodes::EpisodeRow,
    event::Event,
    jobs::JobId,
    notice::Notice,
    sources::only_reorders,
};
use anistream_net::HttpClient;
use anistream_providers::{ProviderRegistry, vpn::VpnGuard};
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
    let meta = build_meta(&config, &http, &store);
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
    let stores = plugin_stores::Stores::new(&config, paths).map_err(|e| anyhow::anyhow!(e))?;
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
    match stores.rows() {
        Ok(rows) => app.apply(Event::Stores(rows)),
        Err(e) => app.apply(Event::Toast(Notice::alert(e))),
    }
    app.apply(Event::Providers(data::provider_rows(&registry)));
    let sync = tracking::Sync::build(&app.config, &store, &http);
    for state in sync.initial_states() {
        app.apply(Event::Sync(Box::new(state)));
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut run = PreviewRun {
        tx,
        rows: screens::visible_rows(
            ratatui::layout::Rect::new(0, 0, width, height),
            app.nav.section(),
        ),
        player_tx: None,
        slot: PlayerSlot::default(),
        prefetch: Prefetch::default(),
        meta,
        store,
        registry,
        http,
        sync,
        stores,
        paths: paths.clone(),
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
    tx: mpsc::UnboundedSender<Event>,
    rows: usize,
    player_tx: Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
    slot: PlayerSlot,
    prefetch: Prefetch,
    meta: Meta,
    store: Store,
    registry: ProviderRegistry,
    http: HttpClient,
    sync: tracking::Sync,
    stores: plugin_stores::Stores,
    paths: Paths,
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
        );
        if !loads {
            eprintln!("[preview: not run: {task:?}]");
            return;
        }
        let job = JobGuard::start(app, &task, &self.tx);
        dispatch(
            task,
            job,
            &mut self.player_tx,
            &self.paths,
            app,
            &self.slot,
            &self.prefetch,
            &self.meta,
            &self.store,
            &self.registry,
            &self.http,
            &self.sync,
            &self.stores,
            &self.tx,
        );
    }

    /// Apply updates until the work is done and the screen's covers are in, as the event loop
    /// would over the same moments.
    async fn settle(&mut self, app: &mut App, rx: &mut mpsc::UnboundedReceiver<Event>) {
        let deadline = tokio::time::Instant::now() + Self::PATIENCE;
        loop {
            if let Some(task) = app.take_pending() {
                self.dispatch(app, task);
            }
            let mut fetching = false;
            for url in app.visible_artwork(self.rows) {
                artwork::spawn_fetch(
                    url,
                    self.paths.image_cache(),
                    self.http.clone(),
                    self.tx.clone(),
                );
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
                Ok(Some(update)) => {
                    match update {
                        // Decoded here and now: a still frame has no loop to keep moving.
                        Event::ArtworkReady { url, bytes } => {
                            covers::hand_over(app, covers::now(url, &bytes));
                        }
                        update => app.apply(update),
                    }
                    if let Some(notice) = self.meta.take_notice() {
                        app.apply(Event::MetaStatus(data::meta_rows(&self.meta)));
                        app.apply(Event::Toast(Notice::info(notice)));
                    }
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
    let meta = build_meta(&config, &http, &store);

    let mut keymap = Keymap::new();
    let keybinding_problems = keymap.apply_overrides(&config.keys);
    for problem in &keybinding_problems {
        eprintln!("warning: keybinding: {problem}");
    }

    // Old logs, stale covers and the player's leftovers, off the critical path.
    {
        let paths = paths.clone();
        tokio::task::spawn_blocking(move || anistream_engine::prune::sweep_at_launch(&paths));
    }
    // With torrents off nothing else would ever prune what an earlier run streamed. With them
    // on, the session prunes on its way up, before it writes anything.
    if !config.providers.torrent.enabled {
        let (dir, limit) =
            (paths.cache_dir.join("torrents"), config.providers.torrent.cache_limit_bytes());
        let store = store.clone();
        tokio::spawn(async move {
            anistream_engine::prune::sweep_torrent_cache(
                dir,
                limit,
                &store,
                Default::default(),
            )
            .await;
        });
    }

    let engine =
        anistream_tui::image::ImageEngine::detect(images).with_cache_dir(paths.image_cache());
    tracing::info!(graphics = ?engine.graphics(), "image engine ready");
    // Without the torrent source, which comes up behind the interface once the VPN guard
    // passes: see `TorrentTasks::bring_up`. Mutable: a settings change can stand the torrent
    // stack down and back up, which replaces the guard along with the session it protects.
    let registry = sources::build_registry_deferred(&config, &http);
    let mut vpn_guard = None;
    tracing::info!(providers = ?registry.ids(), "provider registry ready");
    // Plugins join the chain when they have finished compiling. Nothing waits on this: the first
    // frame is worth more than a source you have not asked for yet.
    let plugins_loading = sources::spawn_plugin_load(
        registry.clone(),
        config.clone(),
        http.clone(),
        paths.clone(),
    );
    let stores = plugin_stores::Stores::new(&config, &paths).map_err(|e| anyhow::anyhow!(e))?;
    let mut app = App::with_images(config, palette, keymap, engine);
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
    // Joined stores as they were left, so Providers can count them on the first frame.
    match stores.rows() {
        Ok(rows) => app.apply(Event::Stores(rows)),
        Err(e) => app.apply(Event::Toast(Notice::alert(e))),
    }

    // Background work reports through this channel; nothing off the UI thread ever touches
    // `App` directly, which is what keeps a slow request from stalling a frame.
    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();

    // The daily update check: one cached request, one quiet toast when a release exists.
    anistream_engine::updates::spawn_check(
        app.config.updates.check,
        &paths.cache_dir,
        &http,
        &tx,
    );

    // The airing digest: what aired for shows you watch since the last launch. Same
    // philosophy as the update check — one line, only when there is something to say.
    anistream_engine::digest::spawn(
        app.config.notifications.airing_digest,
        app.config.notifications.desktop,
        store.clone(),
        meta.clone(),
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
                        let _ = tx.send(Event::SourcesReordered);
                    }
                }
            }
        });
    }

    // Each joined store asked for its current list, once, in the background. Updates are said
    // once at launch, the way a new release is, and never installed without being asked.
    if !app.stores.is_empty() {
        let (stores, tx) = (stores.clone(), tx.clone());
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

    // Trackers are built whether or not they hold a token: the Accounts overlay needs something
    // to offer a sign-in for, and the outbox accumulates against the id meanwhile.
    let sync = tracking::Sync::build(&app.config, &store, &http);
    for state in sync.initial_states() {
        app.apply(Event::Sync(Box::new(state)));
    }
    tracking::spawn_loops(sync.clone(), tx.clone());

    // Keep the mapping datasets current without ever blocking startup.
    tokio::spawn(keep_datasets_current(store.clone(), http.clone(), tx.clone()));

    // The torrent stack's live parts, held so a settings change can tear them down and
    // stand them back up without a restart. Both are replaced together: the download
    // manager and the guard's watcher are the two things holding a session alive, and
    // leaving either behind would mean traffic outliving the source it belongs to.
    // The copy the background tasks read. Kept in step with the app's own copy whenever a
    // setting is written, so "saved" and "in effect" are the same moment.
    let shared_config: anistream_engine::SharedConfig =
        Arc::new(std::sync::RwLock::new(app.config.clone()));
    // Where the torrent source reports in once it is up, tagged with the bring-up it answers.
    let (arrivals_tx, mut arrivals) = mpsc::unbounded_channel();
    let mut torrent_tasks = TorrentTasks::default();
    torrent_tasks.bring_up(&app.config, &http, &paths, &store, &tx, &arrivals_tx);

    // Both before the terminal changes state, so there is no moment in which a signal or a
    // panic could leave it raw.
    tui::terminal::install_panic_hook();
    let mut signals = tui::terminal::Signals::new().context("listening for signals")?;
    let mut terminal = setup_terminal().context("preparing the terminal")?;

    if let Some(task) = app.reload() {
        let job = JobGuard::start(&mut app, &task, &tx);
        spawn(task, job, &meta, &store, &registry, &tx);
    }

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    // Covers come back from the blocking pool on this, decoded, to be handed to the app.
    let (covers_tx, mut covers_rx) = mpsc::unbounded_channel();

    // The live player's control channel. Replaced on each new playback, so a stale sender from
    // a finished episode can never steer the current one.
    let mut player_tx: Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>> =
        None;
    // The player window kept between episodes, claimed by whichever playback comes next.
    let slot = PlayerSlot::default();
    // The next episode's streams, found while the current one's ending plays.
    let prefetch = Prefetch::default();
    let mpv = player(&app.config, &paths, &player_theme());
    if !mpv.is_available().await {
        // Said once at startup rather than at the moment you press Enter on an episode.
        app.apply(Event::Toast(Notice::alert(format!(
            "{} not found — install mpv to play anything",
            app.config.playback.mpv_binary
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
            artwork::spawn_fetch(url, paths.image_cache(), http.clone(), tx.clone());
        }

        // Auto-next is queued by the reducer rather than returned, because an episode ending
        // arrives as an update rather than a keystroke.
        if let Some(task) = app.take_pending() {
            let job = JobGuard::start(&mut app, &task, &tx);
            dispatch(
                task,
                job,
                &mut player_tx,
                &paths,
                &app,
                &slot,
                &prefetch,
                &meta,
                &store,
                &registry,
                &http,
                &sync,
                &stores,
                &tx,
            );
        }

        tokio::select! {
            Some(update) = rx.recv() => {
                // Handed to the terminal's clipboard on the way in, so the overlay can say
                // whether it was.
                let copied = match &update {
                    Event::InviteLink(view) => {
                        copy_to_clipboard(terminal.backend_mut(), &view.link.0)
                    }
                    _ => false,
                };
                // A finished episode should reach the tracker now, not at the next drain
                // tick — up to a minute later, the pushed progress looked simply absent.
                if matches!(update, Event::ProgressQueued) {
                    let (sync, tx) = (sync.clone(), tx.clone());
                    tokio::spawn(async move { tracking::drain_once(&sync, &tx).await });
                }
                // Say when the metadata ladder moved, once per transition, in both
                // directions — silently recovering would leave the last thing said untrue.
                // The status rows ride along so the header chip and the Providers screen
                // agree with the toast without waiting for a refresh.
                if let Some(notice) = meta.take_notice() {
                    app.apply(Event::MetaStatus(anistream_engine::data::meta_rows(&meta)));
                    app.apply(Event::Toast(Notice::info(notice)));
                }
                // A source setting changed. Awaited here rather than spawned: this owns the
                // session and the guard, and two rebuilds racing would leave one of their
                // sessions running with nothing holding it. Quick: the new torrent source
                // comes up behind the interface, as at launch.
                if matches!(
                    update,
                    Event::SettingsSaved | Event::SourcesChanged | Event::SourcesReordered
                ) && let Ok(mut shared) = shared_config.write()
                {
                    *shared = app.config.clone();
                }
                // Only the order changed: the chain as it stands is put in it, with no rebuild.
                if matches!(update, Event::SourcesReordered) {
                    sources::arrange(&registry, &app.config);
                    app.apply(Event::Providers(data::provider_rows(&registry)));
                }
                if matches!(update, Event::SourcesChanged) {
                    reload_sources(
                        &registry,
                        &mut torrent_tasks,
                        &mut vpn_guard,
                        &shared_config,
                        &http,
                        &paths,
                        &store,
                        &tx,
                        &arrivals_tx,
                    )
                    .await;
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
                torrent_tasks.arrived(
                    generation,
                    outcome,
                    &mut vpn_guard,
                    &registry,
                    &store,
                    &shared_config,
                    &tx,
                );
            }
            // Asked to end from outside — a kill, the terminal closing — which leaves the way
            // Q does, so the player is closed rather than orphaned and playing on.
            signal = signals.recv() => {
                tracing::info!(signal, "shutting down");
                tui::terminal::exit_on_second_signal();
                if let Some(task) = app.handle(anistream_tui::keymap::Action::Quit, rows) {
                    let job = JobGuard::start(&mut app, &task, &tx);
                    dispatch(
                        task, job, &mut player_tx, &paths, &app, &slot, &prefetch, &meta, &store,
                        &registry, &http, &sync, &stores, &tx,
                    );
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
                            let job = JobGuard::start(&mut app, &task, &tx);
                            dispatch(
                                task, job, &mut player_tx, &paths, &app, &slot, &prefetch, &meta, &store,
                                &registry, &http, &sync, &stores, &tx,
                            );
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

    stop_for_exit(&app, &player_tx, &mut rx).await;
    restore_terminal(&mut terminal)?;
    result
}

/// Refresh the mapping datasets, and keep trying in the background until a refresh succeeds.
///
/// A launch without a network used to try once and give up for the session, leaving the
/// trackers that key on MAL and Kitsu ids without the mappings they need until the next launch.
/// A failure is said once, in words; the retries back off from five minutes to an hour; and
/// the recovery is said too, so the last thing on screen stays true. A refresh only fetches what
/// is due, so a retry asks again for exactly the datasets that failed.
async fn keep_datasets_current(
    store: Store,
    http: HttpClient,
    tx: mpsc::UnboundedSender<Event>,
) {
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
    rx: &mut mpsc::UnboundedReceiver<Event>,
) {
    let under_way = app.playing.as_ref().is_some_and(|playing| playing.duration.is_some());
    let Some(controls) = player_tx.as_ref().filter(|_| under_way) else { return };
    if controls.send(anistream_model::command::PlayerCommand::Stop).is_err() {
        return;
    }
    let ended = async {
        while let Some(update) = rx.recv().await {
            if matches!(update, Event::PlaybackEnded { .. }) {
                break;
            }
        }
    };
    if tokio::time::timeout(EXIT_FLUSH, ended).await.is_err() {
        tracing::warn!("the player did not stop in time; its last position may be lost");
    }
}

/// Publish a list, then follow up with the broadcast dates it needs a second request for.
///
/// Two sends rather than one: the last-aired query depends on which titles came back, so it
/// cannot be folded into the list query. The list renders as soon as it arrives and gains the
/// broadcast line a moment later, rather than the whole screen waiting on an annotation.
async fn publish_list(
    meta: &Meta,
    tx: &mpsc::UnboundedSender<Event>,
    view: ContentFor,
    entries: Vec<anistream_model::catalog::Entry>,
    now: i64,
) {
    // Releasing titles only. A finished show has no broadcast to report, and including it would
    // spend rows of a capped response on titles that cannot use them.
    let ids: Vec<anistream_core::ids::AnilistId> =
        entries.iter().filter(|e| e.airing_in.is_some()).map(|e| e.id).collect();
    let _ = tx.send(Event::Content { view, content: Content::Entries(entries) });
    if ids.is_empty() {
        return;
    }
    match meta.last_aired(&ids).await {
        Ok(rows) => {
            let _ = tx.send(Event::LastAired(
                rows.into_iter()
                    // Aired rows only. AniList has answered this query with rows whose air
                    // time is still ahead, and naming next week's episode as "out just now"
                    // is exactly the lie the broadcast line exists not to tell.
                    .filter(|r| r.airing_at <= now)
                    .map(|r| (r.media_id, r.episode, now.saturating_sub(r.airing_at)))
                    .collect(),
            ));
        }
        // Not worth a toast: the list is already on screen and complete, and this only adds a
        // line to it. Logged so the Logs overlay can still explain a missing line.
        Err(e) => tracing::warn!(%e, "last-aired lookup failed"),
    }
}

/// Ends its [`Job`](anistream_tui::app::Job) when dropped.
///
/// Moved into the future that does the work, so the job ends exactly when that future does —
/// on success, on an early return, on a panic — with no path having to remember to say so.
/// That is what the status line lacked: its "…" messages were cleared only by the code paths
/// that thought to, and outlived their work on every other one.
struct JobGuard {
    id: JobId,
    tx: mpsc::UnboundedSender<Event>,
}

impl JobGuard {
    fn start(app: &mut App, task: &Command, tx: &mpsc::UnboundedSender<Event>) -> Option<Self> {
        app.begin_job(task).map(|id| Self { id, tx: tx.clone() })
    }

    /// Say what the work is held up on, and for how long if that is known.
    fn note(&self, note: impl Into<String>, wait: Option<Duration>) {
        let _ = self.tx.send(Event::JobNote { job: self.id, note: note.into(), wait });
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        let _ = self.tx.send(Event::JobDone(self.id));
    }
}

/// Run a task off the UI thread and report the result back through the channel.
fn spawn(
    task: Command,
    job: Option<JobGuard>,
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    tx: &mpsc::UnboundedSender<Event>,
) {
    let meta = meta.clone();
    let store = store.clone();
    let registry = registry.clone();
    let tx = tx.clone();

    tokio::spawn(async move {
        // Say so before waiting, not after. At thirty requests a minute a burst of navigation can
        // drain the budget, and the token bucket then delays every request — which presented as an
        // indefinite loading indicator and looked exactly like a hang. As a note on the job it
        // counts down and goes away with the work, where a status line kept saying "12s" after.
        if let Some(wait) = meta.rate_limit_wait().await
            && wait.as_millis() > 400
        {
            match &job {
                Some(job) => job.note("AniList limit", Some(wait)),
                // No job to note it on. A toast, which goes when it is done: as a status it
                // stayed, still counting "12s", after the wait was over.
                None => {
                    let _ = tx.send(Event::Toast(Notice::info(format!(
                        "waiting {}s on the AniList rate limit",
                        wait.as_secs().max(1)
                    ))));
                }
            }
        }
        let update = match task {
            Command::LoadContinue => match continue_entries(&meta, &store).await {
                // Nothing in the history at all is a different empty from a history with
                // nothing left to continue — everything hidden with `x` — and only the first
                // is "nothing watched yet". `Empty` says it is the first.
                Ok(entries)
                    if entries.is_empty() && store.stats().is_ok_and(|s| s.titles == 0) =>
                {
                    Event::Content { view: ContentFor::Continue, content: Content::Empty }
                }
                Ok(entries) => {
                    publish_list(&meta, &tx, ContentFor::Continue, entries, now_epoch()).await;
                    return;
                }
                Err(e) => {
                    Event::Content { view: ContentFor::Continue, content: Content::Failed(e) }
                }
            },
            Command::LoadSeasonal => {
                const PAGE: u32 = 40;
                let (season, year) = current_season();
                match meta.seasonal(season, year, &Default::default(), 1, PAGE).await {
                    Ok(page) => {
                        // Which season this is, which the screen never said, and that one
                        // page is not all of it when it is not.
                        let season = format!("{} {year}", season.as_str().to_lowercase());
                        let text = if page.has_next {
                            format!("{season} · the {PAGE} most popular")
                        } else {
                            season
                        };
                        let _ = tx.send(Event::Caption { view: ContentFor::Seasonal, text });
                        let entries = page
                            .items
                            .iter()
                            .map(|m| data::entry_from(m, Some(&store)))
                            .collect();
                        publish_list(&meta, &tx, ContentFor::Seasonal, entries, now_epoch())
                            .await;
                        return;
                    }
                    Err(e) => Event::Content {
                        view: ContentFor::Seasonal,
                        content: Content::Failed(e.to_string()),
                    },
                }
            }
            Command::LoadCalendar => {
                let anchor = now_epoch();
                match calendar_timeline(&meta, &store, anchor).await {
                    Ok(load) => {
                        // Sent before the content, so the reducer knows how to continue the
                        // list at the moment the list exists — a backfill triggered by the
                        // very first keypress has everything it needs.
                        let _ = tx.send(Event::CalendarWindow {
                            anchor,
                            pages: load.pages,
                            more_past: load.more_past,
                        });
                        Event::Content {
                            view: ContentFor::Calendar,
                            content: Content::Entries(load.entries),
                        }
                    }
                    Err(e) => Event::Content {
                        view: ContentFor::Calendar,
                        content: Content::Failed(e),
                    },
                }
            }
            Command::LoadCalendarPast { anchor, page } => {
                match calendar_past_page(&meta, &store, anchor, page).await {
                    Ok((entries, more_past)) => {
                        Event::CalendarBackfill { page: Some(page), entries, more_past }
                    }
                    // Not a toast. The user never asked for this page by name — they scrolled —
                    // and the calendar in front of them is unchanged and still correct. The
                    // reducer clears its in-flight flag so the next scroll tries again.
                    Err(e) => {
                        tracing::warn!(%e, page, "calendar backfill failed");
                        Event::CalendarBackfill {
                            page: None,
                            entries: Vec::new(),
                            more_past: true,
                        }
                    }
                }
            }
            Command::Search { query, filter } => {
                let view = ContentFor::Search { query: query.clone(), filter: filter.clone() };
                let browse = anistream_meta::anilist::BrowseFilter {
                    genres: filter.genre.iter().cloned().collect(),
                    format: filter.format.clone(),
                    status: filter.status.clone(),
                    min_score: None,
                    sort: filter.sort.clone(),
                    season: filter.season.clone(),
                    year: filter.year,
                };
                const PAGE: u32 = 30;
                match meta.search(&query, &browse, 1, PAGE).await {
                    Ok(page) => {
                        // Sent every time, empty when the page held everything, so a caption
                        // from an earlier run of the same search cannot outlive its answer.
                        let text = if page.has_next {
                            format!("showing the first {PAGE}")
                        } else {
                            String::new()
                        };
                        let _ = tx.send(Event::Caption { view: view.clone(), text });
                        let entries = page
                            .items
                            .iter()
                            .map(|m| data::entry_from(m, Some(&store)))
                            .collect();
                        publish_list(&meta, &tx, view, entries, now_epoch()).await;
                        return;
                    }
                    Err(e) => Event::Content { view, content: Content::Failed(e.to_string()) },
                }
            }
            Command::LoadDetail(id) => {
                // Sent before the detail itself, so the picker knows the current choice the
                // moment the title is on screen rather than only after it is opened once.
                let _ = tx.send(Event::ProviderPreference(
                    store.provider_preference(id).ok().flatten(),
                ));
                match meta.media(id).await {
                    Ok(media) => {
                        Event::Detail(Box::new(data::entry_from(&media, Some(&store))))
                    }
                    Err(e) => Event::Toast(Notice::alert(format!("could not load title: {e}"))),
                }
            }
            Command::CheckProviders => {
                registry.check_all(now_epoch()).await;
                // The metadata ladder rides the same refresh: one screen, one key,
                // every source's condition.
                let _ = tx.send(Event::MetaStatus(data::meta_rows(&meta)));
                Event::Providers(data::provider_rows(&registry))
            }
            // Handled in `dispatch`, which owns the live player's control channel and the
            // tracker set. Reaching here would mean a routing mistake, so it says so.
            Command::Play { .. }
            | Command::Prefetch { .. }
            | Command::PlayParty { .. }
            | Command::LoadSources { .. }
            | Command::PlaySource { .. }
            | Command::ManualSearch { .. }
            | Command::SetWatched { .. }
            | Command::HideFromContinue { .. }
            | Command::OpenExternal { .. }
            | Command::PlayTrailer { .. }
            | Command::Player(_)
            | Command::SyncNow
            | Command::Connect { .. }
            | Command::ConnectKitsu { .. }
            | Command::Disconnect { .. }
            | Command::SetStatus { .. }
            | Command::SetScore { .. }
            | Command::ResolveConflict { .. }
            | Command::SetProviderPreference { .. }
            | Command::LoadLibrary(_)
            | Command::DownloadEpisode { .. }
            | Command::DownloadMany { .. }
            | Command::DownloadPause { .. }
            | Command::DownloadCancel { .. }
            | Command::DownloadDelete { .. }
            | Command::DownloadClearCompleted
            | Command::LoadDownloads
            | Command::PlayLocal { .. }
            | Command::SaveSetting { .. }
            | Command::LoadStores
            | Command::PreviewStore { .. }
            | Command::JoinStore { .. }
            | Command::LeaveStore { .. }
            | Command::InstallPlugin { .. }
            | Command::RemovePlugin { .. }
            | Command::CreateInvite { .. }
            | Command::LoadMembers { .. }
            | Command::RemoveMember { .. }
            | Command::CancelInvite { .. }
            | Command::LoadLocalPlugins { .. }
            | Command::PublishPlugin { .. }
            | Command::UnpublishPlugin { .. }
            | Command::OpenAdmin { .. } => {
                tracing::error!(?task, "task reached the generic spawner");
                return;
            }
            Command::ClearMatch { id } => {
                if let Err(e) = store.clear_title_match(id) {
                    Event::Toast(Notice::alert(format!("could not reset the match: {e}")))
                } else {
                    match load_episodes(&meta, &store, &registry, id).await {
                        Ok(EpisodeLoad::Rows(rows)) => Event::Episodes { id, rows },
                        Ok(EpisodeLoad::Choose { provider_id, candidates }) => {
                            Event::MatchChoices { id, provider_id, candidates }
                        }
                        Err(reason) => {
                            let _ = tx.send(Event::Episodes { id, rows: Vec::new() });
                            Event::Toast(Notice::alert(reason))
                        }
                    }
                }
            }
            Command::FixMatch { id, provider_id, key } => {
                // Remembered as an override, which the ladder consults before anything else —
                // so the question is asked once, not on every visit.
                if let Err(e) = store.set_override(id, &provider_id, &key, now_epoch()) {
                    Event::Toast(Notice::alert(format!("could not save that match: {e}")))
                } else {
                    match load_episodes(&meta, &store, &registry, id).await {
                        Ok(EpisodeLoad::Rows(rows)) => Event::Episodes { id, rows },
                        // The override *is* the answer, so the ladder cannot come back
                        // undecided; if it somehow does, say so rather than looping.
                        Ok(EpisodeLoad::Choose { .. }) => Event::Toast(Notice::alert(
                            "that match did not stick — the source may have changed",
                        )),
                        Err(reason) => {
                            let _ = tx.send(Event::Episodes { id, rows: Vec::new() });
                            Event::Toast(Notice::alert(reason))
                        }
                    }
                }
            }
            Command::LoadEpisodes(id) => {
                match load_episodes(&meta, &store, &registry, id).await {
                    Ok(EpisodeLoad::Rows(rows)) => Event::Episodes { id, rows },
                    Ok(EpisodeLoad::Choose { provider_id, candidates }) => {
                        Event::MatchChoices { id, provider_id, candidates }
                    }
                    Err(reason) => {
                        // The Episodes screen shows its own empty state, so the reason belongs
                        // in a toast rather than replacing the whole view.
                        let _ = tx.send(Event::Episodes { id, rows: Vec::new() });
                        Event::Toast(Notice::alert(reason))
                    }
                }
            }
        };
        let _ = tx.send(update);
    });
}

/// Route a task to whoever performs it.
///
/// Playback is handled here rather than in [`spawn`] because it owns the live player's control
/// channel: replacing that sender is what stops a finished episode's keys steering the next one.
#[allow(clippy::too_many_arguments)]
fn dispatch(
    task: Command,
    job: Option<JobGuard>,
    player_tx: &mut Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
    paths: &Paths,
    app: &App,
    slot: &PlayerSlot,
    prefetch: &Prefetch,
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    http: &HttpClient,
    sync: &tracking::Sync,
    stores: &plugin_stores::Stores,
    tx: &mpsc::UnboundedSender<Event>,
) {
    let config = &app.config;
    // No player handle is passed in: every playback path builds its own from the config
    // above, which is the live one the Settings screen edits. A handle held across the
    // session is exactly how a turned-off upscaler kept running.
    match task {
        // Sync work needs the tracker set, which `spawn` deliberately does not carry.
        Command::SyncNow => {
            let (sync, tx) = (sync.clone(), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                tracking::drain_once(&sync, &tx).await;
                tracking::pull_once(&sync, &tx).await;
                let _ = tx.send(Event::Status(String::new()));
            });
        }
        Command::Connect { tracker } => {
            let (sync, tx) = (sync.clone(), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                tracking::connect(sync, tracker, tx).await;
            });
        }
        Command::ConnectKitsu { username, password } => {
            let (sync, tx) = (sync.clone(), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                tracking::connect_kitsu(sync, username, password.0, tx).await;
            });
        }
        Command::Disconnect { tracker } => tracking::disconnect(sync, &tracker, tx),
        Command::SetStatus { id, status } => {
            tracking::set_status(sync, id, status);
            // Queued, not sent — the drain owns the network. Reporting the new depth is what
            // makes the badge move immediately.
            for state in sync.state_after_enqueue() {
                let _ = tx.send(Event::Sync(Box::new(state)));
            }
        }
        Command::SetScore { id, score } => {
            tracking::set_score(sync, id, score);
            for state in sync.state_after_enqueue() {
                let _ = tx.send(Event::Sync(Box::new(state)));
            }
        }
        Command::ResolveConflict { id, keep_local } => {
            tracking::resolve_conflict(sync, id, keep_local);
        }
        Command::SetProviderPreference { id, provider } => {
            let now = now_epoch();
            let written = match &provider {
                Some(provider) => store.set_provider_preference(id, provider, now),
                None => store.clear_provider_preference(id),
            };
            let toast = match (written, &provider) {
                (Ok(()), Some(provider)) => {
                    Notice::info(format!("{provider} will be used for this title"))
                }
                (Ok(()), None) => Notice::info("back to automatic source selection"),
                // The overlay has already moved its marker, so a failed write has to be said
                // out loud or the UI would be showing a choice that was never saved.
                (Err(e), _) => Notice::alert(format!("could not save that choice: {e}")),
            };
            let _ = tx.send(Event::Toast(toast));
        }
        Command::LoadLibrary(segment) => {
            let (sync, tx, store) = (sync.clone(), tx.clone(), store.clone());
            tokio::spawn(async move {
                let _job = job;
                let update = tracking::load_library(&sync, &store, segment).await;
                let _ = tx.send(update);
            });
        }
        Command::Prefetch { id, episode } => {
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
                let (quiet, _) = mpsc::unbounded_channel();
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
        Command::Play { id, episode } => {
            // A fresh channel per playback. The one playing gives up its window only once
            // this one has something to play — see `Handoff`.
            let (prx, handoff) = take_controls(player_tx, successor(app, id, &episode));
            let prefetched = prefetch.take(id, &episode, config.playback.translation);
            // Tracker ids rather than trackers: playback only needs to know which queues to
            // append to, and passing the set would couple the player to the sync layer.
            let tracker_ids = signed_in_tracker_ids(sync);
            let extras = episode_extras(app, id, &episode);
            if extras.1.is_empty() {
                fill_episodes(id, &episode, meta, store, registry, player_tx);
            }
            spawn_playback(
                id,
                episode,
                None,
                config.syncplay.enabled,
                prx,
                handoff,
                config,
                meta,
                store,
                registry,
                http,
                &player(config, paths, &player_theme()),
                tracker_ids,
                extras,
                slot.clone(),
                shader_dir(config, paths),
                prefetched,
                job,
                tx,
            );
        }
        Command::PlaySource { id, episode, provider_id, source_id } => {
            // Identical to Play except the stream comes from the exact release the user
            // picked — same fresh channel, same session-replacement semantics. Never from the
            // streams found ahead: the pick names its own release.
            let (prx, handoff) = take_controls(player_tx, successor(app, id, &episode));
            let prefetched = None;
            let tracker_ids = signed_in_tracker_ids(sync);
            let extras = episode_extras(app, id, &episode);
            if extras.1.is_empty() {
                fill_episodes(id, &episode, meta, store, registry, player_tx);
            }
            spawn_playback(
                id,
                episode,
                Some((provider_id, source_id)),
                config.syncplay.enabled,
                prx,
                handoff,
                config,
                meta,
                store,
                registry,
                http,
                &player(config, paths, &player_theme()),
                tracker_ids,
                extras,
                slot.clone(),
                shader_dir(config, paths),
                prefetched,
                job,
                tx,
            );
        }
        Command::PlayParty { id, episode } => {
            // Same resolution as Play; the handoff at the end is the only difference.
            let (prx, handoff) = take_controls(player_tx, successor(app, id, &episode));
            let prefetched = None;
            let tracker_ids = signed_in_tracker_ids(sync);
            let extras = episode_extras(app, id, &episode);
            spawn_playback(
                id,
                episode,
                None,
                true,
                prx,
                handoff,
                config,
                meta,
                store,
                registry,
                http,
                &player(config, paths, &player_theme()),
                tracker_ids,
                extras,
                slot.clone(),
                shader_dir(config, paths),
                prefetched,
                job,
                tx,
            );
        }
        Command::LoadSources { id, episode } => {
            let (store, registry, meta, tx) =
                (store.clone(), registry.clone(), meta.clone(), tx.clone());
            let translation = config.playback.translation;
            tokio::spawn(async move {
                let _job = job;
                let update =
                    match source_slate(&meta, &store, &registry, id, &episode, translation)
                        .await
                    {
                        Ok(candidates) => Event::Sources { id, episode, candidates },
                        Err(reason) => Event::Toast(Notice::alert(reason)),
                    };
                let _ = tx.send(update);
            });
        }
        Command::SetWatched { id, episodes, watched } => {
            let now = anistream_store::now();
            for episode in &episodes {
                if watched {
                    let mut event = anistream_store::WatchEvent::new(id, episode.clone(), 0.0);
                    event.completed = true;
                    let _ = store.record_event(&event);
                } else {
                    let _ = store.forget_episode(id, episode);
                }
            }
            // Project to trackers only when marking: progress there is monotonic, so an
            // unmark keeps the remote high-water mark and only local history forgets.
            if watched {
                let reached = store.tracker_progress(id).unwrap_or(0);
                if reached > 0 {
                    let op = anistream_core::traits::TrackOp::SetProgress {
                        anilist_id: id,
                        episode: reached,
                    };
                    // Signed-in trackers only, like a finished episode: the default Kitsu
                    // entry has no account behind it until someone signs in.
                    for tracker in sync.trackers.iter().filter(|t| t.is_authenticated()) {
                        let _ = store.enqueue(tracker.id(), &op, now);
                    }
                }
            }
        }

        Command::HideFromContinue { id } => {
            // The reducer has already dropped the row from the screen; failing to persist
            // that would resurrect it on the next reload, so the failure is named.
            if let Err(e) = store.hide_from_continue(id) {
                let _ = tx.send(Event::Toast(Notice::alert(format!(
                    "could not remove from continue: {e}"
                ))));
            }
        }

        Command::PlayTrailer { url, title } => {
            let mpv_binary = config.playback.mpv_binary.clone();
            let tx = tx.clone();
            tokio::spawn(async move {
                let _job = job;
                // mpv streams YouTube through yt-dlp, and without it the window opens and
                // dies with nothing to say. Checked up front so the failure mode is a
                // browser tab rather than a mystery.
                if in_path(&["yt-dlp", "youtube-dl"])
                    && let Ok(mut child) = tokio::process::Command::new(&mpv_binary)
                        .arg(format!("--force-media-title={title} — trailer"))
                        .arg(&url)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                {
                    let _ = tx.send(Event::Toast(Notice::info("trailer playing in mpv")));
                    // Reaped so a closed trailer never lingers as a zombie.
                    let _ = child.wait().await;
                    return;
                }
                let update = match open::that_detached(&url) {
                    Ok(()) => Event::Toast(Notice::info("trailer opened in your browser")),
                    Err(e) => {
                        Event::Toast(Notice::alert(format!("could not open the trailer: {e}")))
                    }
                };
                let _ = tx.send(update);
            });
        }
        Command::OpenExternal { url } => {
            if let Err(e) = open::that_detached(&url) {
                let _ = tx.send(Event::Toast(Notice::alert(format!("could not open: {e}"))));
            }
        }

        Command::ManualSearch { id, query } => {
            let (registry, meta, tx) = (registry.clone(), meta.clone(), tx.clone());
            let translation = config.playback.translation;
            tokio::spawn(async move {
                let _job = job;
                let now = now_epoch();
                let attempt = registry.search(&query, translation, now).await;
                let provider_id = attempt.provider.clone().unwrap_or_default();
                let update = match attempt.value {
                    Some(hits) if !hits.is_empty() => match meta.media(id).await {
                        // Ranked against the real title so the percentages mean the same
                        // thing they mean when the ladder asks on its own.
                        Ok(media) => {
                            let target = media.match_target();
                            let candidates = anistream_meta::title::rank(&target, &hits)
                                .into_iter()
                                .map(|scored| anistream_model::sources::MatchCandidate {
                                    title: scored.hit.title.clone(),
                                    key: scored.hit.key.clone(),
                                    similarity: scored.score,
                                    rejected: scored.rejected,
                                })
                                .collect();
                            Event::MatchChoices { id, provider_id, candidates }
                        }
                        Err(e) => Event::Toast(Notice::alert(e.to_string())),
                    },
                    _ => Event::Toast(Notice::alert(format!("nothing found for {query:?}"))),
                };
                let _ = tx.send(update);
            });
        }
        Command::Player(command) => {
            if let Some(sender) = player_tx.as_ref() {
                let _ = sender.send(command);
            }
        }
        Command::LoadDownloads => downloads::publish_now(store, tx),

        Command::DownloadMany { id, episodes } => {
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
                        let (_, queued) = downloads::enqueue(
                            store,
                            registry,
                            meta,
                            id,
                            &episode,
                            translation,
                        )
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

        Command::DownloadEpisode { id, episode } => {
            let (store, registry, meta, tx) =
                (store.clone(), registry.clone(), meta.clone(), tx.clone());
            let translation = config.playback.translation;
            tokio::spawn(async move {
                let _job = job;
                // Resolving needs the provider chain, so this is a task rather than inline — the
                // same walk playback does, which is the point: a download is "play it, to disk".
                let update = match downloads::enqueue(
                    &store,
                    &registry,
                    &meta,
                    id,
                    &episode,
                    translation,
                )
                .await
                {
                    Ok((row, downloads::Queued::New)) => Event::Toast(Notice::info(format!(
                        "queued {} ep {}",
                        row.title, row.episode
                    ))),
                    Ok((row, downloads::Queued::Existing)) => Event::Toast(Notice::info(
                        format!("{} ep {} is already in downloads", row.title, row.episode),
                    )),
                    Err(refusal) => Event::Toast(Notice::alert(refusal.to_string())),
                };
                let _ = tx.send(update);
                let _ = tx.send(Event::Status(String::new()));
                downloads::publish_now(&store, &tx);
            });
        }

        Command::DownloadPause { id } => {
            // Toggled here rather than in the reducer because the authoritative state is the row,
            // and the reducer only has the flattened projection of it.
            if let Ok(Some(row)) =
                store.downloads().map(|all| all.into_iter().find(|d| d.id == id))
            {
                let next = match row.state {
                    anistream_store::DownloadState::Paused => {
                        anistream_store::DownloadState::Queued
                    }
                    anistream_store::DownloadState::Done
                    | anistream_store::DownloadState::Failed => row.state,
                    _ => anistream_store::DownloadState::Paused,
                };
                let _ = store.set_download_state(id, next);
            }
            downloads::publish_now(store, tx);
        }

        Command::DownloadCancel { id } => {
            // The row goes first: the manager notices it is gone on its next poll and drops the
            // torrent. Deleting the torrent from here would race with that loop.
            let _ = store.remove_download(id);
            downloads::publish_now(store, tx);
            let _ = tx.send(Event::Toast(Notice::info("download cancelled")));
        }

        Command::DownloadDelete { id } => {
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

        Command::DownloadClearCompleted => {
            let cleared = store.clear_completed_downloads().unwrap_or(0);
            downloads::publish_now(store, tx);
            let _ = tx.send(Event::Toast(Notice::info(format!("cleared {cleared} finished"))));
        }

        Command::PlayLocal { id, episode, title, path } => {
            // No provider walk and no torrent session — the file is on disk — but the full
            // playback session all the same: a downloaded episode records history, resumes,
            // and syncs exactly like a streamed one. It used to go straight to mpv with no
            // context, which made downloads second-class in every way that matters.
            // Nothing to look up, so nothing can fail before it plays.
            let (prx, handoff) = take_controls(player_tx, successor(app, id, &episode));
            handoff.now();
            let tracker_ids = signed_in_tracker_ids(sync);
            let (next, episodes) = episode_extras(app, id, &episode);
            if episodes.is_empty() {
                fill_episodes(id, &episode, meta, store, registry, player_tx);
            }
            let shaders = shader_dir(config, paths);
            let (config, store, http, mpv, tx, slot) = (
                config.clone(),
                store.clone(),
                http.clone(),
                player(config, paths, &player_theme()),
                tx.clone(),
                slot.clone(),
            );
            tokio::spawn(async move {
                let _job = job;
                let playback = config.playback.clone();
                let stream = anistream_core::stream::Stream::new(
                    path,
                    anistream_core::stream::StreamKind::Mp4,
                );
                // Best-effort, exactly as the streaming path treats them.
                let resume_at = store
                    .resume_position(id, &episode)
                    .inspect_err(
                        |e| tracing::warn!(error = %e, "could not read resume position"),
                    )
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
                    volume: playback
                        .persist_volume
                        .then_some(playback.persisted_volume)
                        .flatten(),
                    next,
                    episodes,
                };
                let settings = playback::PlaySettings {
                    shader_dir: shaders,
                    ..play_settings(&config, tracker_ids, config.syncplay.enabled)
                };
                playback::play(
                    vec![stream],
                    context,
                    store,
                    http,
                    mpv,
                    settings,
                    slot,
                    tx,
                    prx,
                )
                .await;
            });
        }

        // Written here rather than in the reducer: the reducer has already changed the in-memory
        // config, so the screen is correct whatever the disk does, and this keeps file IO out of
        // a pure function. Synchronous on purpose — one small file, and a settings change that
        // silently lost a race with the next one would be worse than a paused frame.
        Command::SaveSetting { table, key, value } => {
            // Against the file's order, not `config`'s: the reducer has already put the new
            // order in effect there.
            let reordered = (table, key) == (&["providers"][..], "order")
                && matches!(&value, anistream_core::settings::SettingValue::List(order)
                    if only_reorders(&app.saved_config.providers.order, order));
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
                    if let Some(label) = app.setting_label(table, key) {
                        let _ = tx.send(Event::Toast(Notice::info(format!("saved · {label}"))));
                    }
                    // Republish what the background tasks read, so the next download to
                    // finish honours the value that was just written.
                    let _ = tx.send(Event::SettingsSaved);
                    // A setting that redefines what sources exist takes effect now rather
                    // than at the next launch. The run loop owns the session and the guard,
                    // so it does the work; this only says that it is needed. An order that
                    // only moves sources is put in effect without a rebuild.
                    if reordered {
                        let _ = tx.send(Event::SourcesReordered);
                    } else if redefines_sources(table, key) {
                        let _ = tx.send(Event::SourcesChanged);
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
                    let _ =
                        tx.send(Event::Toast(Notice::alert(format!("not saved: {reason}"))));
                    // The Providers rows moved when the key was pressed and the chain did not,
                    // so the chain's own rows put them back.
                    if (table, key) == (&["providers"][..], "order") {
                        let _ = tx.send(Event::Providers(data::provider_rows(registry)));
                    }
                }
            }
        }
        task @ (Command::LoadStores
        | Command::PreviewStore { .. }
        | Command::JoinStore { .. }
        | Command::LeaveStore { .. }
        | Command::InstallPlugin { .. }
        | Command::RemovePlugin { .. }
        | Command::CreateInvite { .. }
        | Command::LoadMembers { .. }
        | Command::RemoveMember { .. }
        | Command::CancelInvite { .. }
        | Command::LoadLocalPlugins { .. }
        | Command::PublishPlugin { .. }
        | Command::UnpublishPlugin { .. }
        | Command::OpenAdmin { .. }) => {
            let context = StoreTaskContext {
                stores: stores.clone(),
                registry: registry.clone(),
                config: config.clone(),
                http: http.clone(),
                paths: paths.clone(),
                tx: tx.clone(),
            };
            tokio::spawn(async move {
                let _job = job;
                context.run(task).await;
            });
        }
        other => spawn(other, job, meta, store, registry, tx),
    }
}

/// What a plugin store task needs, moved into the task that runs it.
struct StoreTaskContext {
    stores: plugin_stores::Stores,
    registry: ProviderRegistry,
    config: Config,
    http: HttpClient,
    paths: Paths,
    tx: mpsc::UnboundedSender<Event>,
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

    async fn run(&self, task: Command) {
        match task {
            Command::LoadStores => {
                // What is on disk answers at once; the network fills in behind it.
                self.rows();
                match self.stores.refresh().await {
                    Ok(rows) => self.send(Event::Stores(rows)),
                    Err(e) => self.toast(Notice::alert(e)),
                }
            }
            Command::PreviewStore { link } => {
                self.send(Event::StorePreview(self.stores.preview(&link.0).await));
            }
            Command::JoinStore { link } => match self.stores.join(&link.0).await {
                Ok(joined) => {
                    self.rows();
                    self.send(Event::StoreJoined { store: joined.id.clone() });
                    self.toast(Notice::info(format!(
                        "joined {} as {}",
                        joined.name, joined.member.name
                    )));
                }
                Err(e) => self.toast(Notice::alert(e)),
            },
            Command::LeaveStore { store } => match self.stores.leave(&store).await {
                Ok(name) => {
                    self.rows();
                    self.toast(Notice::info(format!(
                        "left {name} — its plugins stay installed"
                    )));
                }
                Err(e) => self.toast(Notice::alert(e)),
            },
            Command::InstallPlugin { store, plugin } => {
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
            Command::RemovePlugin { plugin } => match self.stores.remove(&plugin).await {
                Ok(()) => {
                    self.registry.remove(&plugin);
                    self.send(Event::Providers(data::provider_rows(&self.registry)));
                    self.rows();
                    self.toast(Notice::info(format!("removed {plugin}")));
                }
                Err(e) => self.toast(Notice::alert(e)),
            },
            Command::CreateInvite { store, name } => {
                match self.stores.invite(&store, name).await {
                    Ok(view) => self.send(Event::InviteLink(view)),
                    Err(e) => self.toast(Notice::alert(e)),
                }
            }
            Command::LoadMembers { store } => self.members(&store).await,
            Command::RemoveMember { store, member } => {
                match self.stores.remove_member(&store, &member).await {
                    Ok(()) => {
                        self.toast(Notice::info("removed — their devices stopped working here"))
                    }
                    Err(e) => self.toast(Notice::alert(e)),
                }
                self.members(&store).await;
            }
            Command::CancelInvite { store, invite } => {
                match self.stores.cancel_invite(&store, &invite).await {
                    Ok(()) => {
                        self.toast(Notice::info("invite cancelled — the link no longer works"))
                    }
                    Err(e) => self.toast(Notice::alert(e)),
                }
                self.members(&store).await;
            }
            Command::LoadLocalPlugins { store } => match self.stores.local_plugins(&store).await {
                Ok(plugins) => self.send(Event::LocalPlugins { store, plugins }),
                Err(e) => self.toast(Notice::alert(e)),
            },
            Command::PublishPlugin { store, plugin } => {
                match self.stores.publish(&store, &plugin).await {
                    Ok(entry) => {
                        self.rows();
                        self.toast(Notice::info(format!(
                            "published {} {}",
                            entry.name, entry.version
                        )));
                    }
                    Err(e) => self.toast(Notice::alert(format!("{plugin} not published: {e}"))),
                }
            }
            Command::UnpublishPlugin { store, plugin } => {
                match self.stores.unpublish(&store, &plugin).await {
                    Ok(()) => {
                        self.rows();
                        self.toast(Notice::info(format!("{plugin} is no longer offered")));
                    }
                    Err(e) => self.toast(Notice::alert(e)),
                }
            }
            Command::OpenAdmin { store } => match self.stores.admin_link(&store).await {
                Ok(link) => match open::that_detached(&link) {
                    Ok(()) => self.toast(Notice::info(
                        "opened the admin page in your browser — the link works once, for ten minutes",
                    )),
                    Err(e) => self.toast(Notice::alert(format!(
                        "could not open a browser ({e}) — on the store's machine, `anistream store admin` prints a link"
                    ))),
                },
                Err(e) => self.toast(Notice::alert(e)),
            },
            other => tracing::error!(task = ?other, "not a plugin store task"),
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
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    http: &HttpClient,
    mpv: &anistream_player::Mpv,
    tracker_ids: Vec<String>,
    // What the player's Up Next card and Episodes menu show, read from the episode table.
    extras: (Option<playback::NextEpisode>, Vec<playback::EpisodeEntry>),
    slot: PlayerSlot,
    shader_dir: Option<std::path::PathBuf>,
    // This episode's streams, found ahead while the previous one's ending played.
    prefetched: Option<Resolved>,
    job: Option<JobGuard>,
    tx: &mpsc::UnboundedSender<Event>,
) {
    let (config, meta, store, registry, http, mpv, tx) = (
        config.clone(),
        meta.clone(),
        store.clone(),
        registry.clone(),
        http.clone(),
        mpv.clone(),
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
    tx: &mpsc::UnboundedSender<Event>,
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

/// What loading a title's episodes produced.
enum EpisodeLoad {
    Rows(Vec<EpisodeRow>),
    /// The ladder found candidates but could not choose. The user decides.
    Choose {
        provider_id: String,
        candidates: Vec<anistream_model::sources::MatchCandidate>,
    },
}

/// Resolve a title to a provider and list its episodes.
async fn load_episodes(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
) -> std::result::Result<EpisodeLoad, String> {
    if registry.is_empty() {
        return Err("no sources configured".into());
    }
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    let target = media.match_target();
    let now = now_epoch();

    // From the source playback would use: the one pinned for this title, or else the first in
    // the order that has it, so the table's numbering is the one Enter plays.
    let pinned = store.provider_preference(id).ok().flatten().and_then(|p| registry.pinned(&p));
    let registry = pinned.as_ref().unwrap_or(registry);
    let answer = anistream_providers::first_answer(
        store,
        registry,
        id,
        &target,
        anistream_core::media::Translation::Sub,
        now,
        |source, key| async move {
            source.episodes(&key, anistream_core::media::Translation::Sub, now).await
        },
    )
    .await;

    let mut episodes = match answer {
        anistream_providers::Answer::Answered { value, .. } => value,
        // Candidates exist, none confidently enough to pick. That is a question, not a
        // failure: the user can see which of these is their show, and the answer is
        // remembered as an override so it is asked once.
        anistream_providers::Answer::Ambiguous { provider_id, candidates } => {
            return Ok(EpisodeLoad::Choose {
                provider_id,
                candidates: candidates
                    .iter()
                    .map(|candidate| anistream_model::sources::MatchCandidate {
                        title: candidate.hit.title.clone(),
                        key: candidate.hit.key.clone(),
                        similarity: candidate.score,
                        rejected: candidate.rejected,
                    })
                    .collect(),
            });
        }
        anistream_providers::Answer::Unanswered { .. } => {
            return Err(format!("could not list episodes: {}", answer.explain()));
        }
    };

    // A torrent source knows an episode exists but not what it is called. The metadata we
    // already fetched above does, so name them before they reach the table.
    data::name_episodes(&mut episodes, &media.episode_titles(), &media.episode_thumbnails());

    Ok(EpisodeLoad::Rows(data::episode_rows(&episodes, store, id)))
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

/// Everything the torrent source runs in the background: the download manager and the
/// VPN guard's re-verification loop.
///
/// Grouped so they can be stopped and started as one. A source the user has just switched
/// off must stop *seeding*, not merely stop being offered — and that means the tasks
/// holding its session have to end with it.
#[derive(Default)]
struct TorrentTasks {
    /// Waiting for the VPN guard to pass, before there is a session.
    bring_up: Option<tokio::task::JoinHandle<()>>,
    /// Which bring-up the current one is. A result from an earlier one — the settings changed
    /// while it waited — describes a source that has since been switched off or replaced.
    generation: u64,
    downloads: Option<tokio::task::JoinHandle<()>>,
    watcher: Option<tokio::task::JoinHandle<()>>,
    seeding: Option<tokio::task::JoinHandle<()>>,
    /// Keeps the stream cache within its limit while the session writes to it.
    cache: Option<tokio::task::JoinHandle<()>>,
    session: Option<Arc<anistream_providers::torrent::TorrentSession>>,
}

impl TorrentTasks {
    fn start(
        guard: &Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
        store: &Store,
        config: &anistream_engine::SharedConfig,
        tx: &mpsc::UnboundedSender<Event>,
    ) -> Self {
        let Some((guard, session)) = guard.clone() else {
            // No session means no downloads, which is correct rather than a limitation:
            // every downloadable source here is a torrent.
            return Self::default();
        };

        let downloads =
            downloads::spawn(store.clone(), session.clone(), Arc::clone(config), tx.clone());
        downloads::publish_now(store, tx);

        let _ = tx.send(Event::Vpn(Some(sources::guard_view(&guard.state()))));
        let watcher = tokio::spawn(vpn_watch(guard, session.clone(), tx.clone()));
        let seeding =
            tokio::spawn(seeding_watch(session.clone(), Arc::clone(config), tx.clone()));
        let cache = tokio::spawn(anistream_engine::prune::torrent_cache_watch(
            session.clone(),
            store.clone(),
            Arc::clone(config),
        ));
        Self {
            bring_up: None,
            generation: 0,
            downloads: Some(downloads),
            watcher: Some(watcher),
            seeding: Some(seeding),
            cache: Some(cache),
            session: Some(session),
        }
    }

    /// Start bringing the torrent source up, when the configuration asks for it.
    ///
    /// In the background: the guard's check is a network request through a tunnel that may
    /// be down, and the first frame must not wait on it. The badge says what the guard found
    /// meanwhile, and the source joins the chain when it passes — see [`Self::arrived`].
    fn bring_up(
        &mut self,
        config: &Config,
        http: &HttpClient,
        paths: &Paths,
        store: &Store,
        tx: &mpsc::UnboundedSender<Event>,
        arrivals: &mpsc::UnboundedSender<(
            u64,
            std::result::Result<sources::TorrentUp, String>,
        )>,
    ) {
        self.generation += 1;
        if !sources::torrent_wanted(config) {
            return;
        }
        let _ = tx.send(Event::Vpn(Some(anistream_model::sources::GuardView::Checking)));
        let generation = self.generation;
        let (config, http, paths, store, tx, arrivals) = (
            config.clone(),
            http.clone(),
            paths.clone(),
            store.clone(),
            tx.clone(),
            arrivals.clone(),
        );
        self.bring_up = Some(tokio::spawn(async move {
            let outcome = sources::bring_up_torrent(config, http, paths, store, tx).await;
            let _ = arrivals.send((generation, outcome));
        }));
    }

    /// A bring-up finished: put the source in the chain and start what runs beside it, or say
    /// why it will not come up.
    #[allow(clippy::too_many_arguments)]
    fn arrived(
        &mut self,
        generation: u64,
        outcome: std::result::Result<sources::TorrentUp, String>,
        vpn_guard: &mut Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
        registry: &ProviderRegistry,
        store: &Store,
        config: &anistream_engine::SharedConfig,
        tx: &mpsc::UnboundedSender<Event>,
    ) {
        if generation != self.generation {
            // Superseded. Its session holds no torrents yet, so letting go of it is enough.
            return;
        }
        self.bring_up = None;
        let up = match outcome {
            Ok(up) => up,
            Err(reason) => {
                tracing::warn!(%reason, "torrent source not started");
                let _ = tx.send(Event::ProviderNote(reason));
                return;
            }
        };
        let snapshot =
            config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
        sources::place(registry, up.provider, &snapshot);
        if let Some(warning) = up.warning {
            let _ = tx.send(Event::Toast(Notice::info(warning)));
        }
        *vpn_guard = Some((up.guard, up.session));
        *self = Self { generation, ..Self::start(vpn_guard, store, config, tx) };
        let _ = tx.send(Event::Providers(data::provider_rows(registry)));
        tracing::info!(providers = ?registry.ids(), "torrent source up");
    }

    /// Stop everything and halt the session before letting go of it.
    ///
    /// Halting first is the whole point: dropping the handles would leave librqbit
    /// seeding whatever it holds until the process exits, which for a source the user
    /// just turned off is precisely the traffic they meant to stop.
    async fn stop(&mut self) {
        // A bring-up still waiting for the guard has no session to halt, only a wait to end.
        if let Some(waiting) = self.bring_up.take() {
            waiting.abort();
        }
        if let Some(session) = self.session.take() {
            session.halt().await;
        }
        for task in
            [self.downloads.take(), self.watcher.take(), self.seeding.take(), self.cache.take()]
                .into_iter()
                .flatten()
        {
            task.abort();
        }
    }
}

/// Keep the session's seeding in line with the settings, retire watched episodes that have
/// given back enough, and tell the Downloads screen what is still seeding.
///
/// Reads the shared config each tick rather than being told about changes: a setting saved on
/// the Settings screen reaches the session within one tick, upload cap included, with no
/// rebuild of the source (which would tear down the episode playing).
async fn seeding_watch(
    session: Arc<anistream_providers::torrent::TorrentSession>,
    config: anistream_engine::SharedConfig,
    tx: mpsc::UnboundedSender<Event>,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(10));
    let mut shown: Option<Vec<anistream_model::downloads::SeedRow>> = None;
    loop {
        ticker.tick().await;
        let goal =
            config.read().map(|c| c.providers.torrent.seeding.clone()).unwrap_or_default();
        session.configure_seeding(&goal).await;
        let rows: Vec<anistream_model::downloads::SeedRow> = session
            .reap_seeding()
            .await
            .into_iter()
            .map(|seed| anistream_model::downloads::SeedRow {
                name: seed.name,
                uploaded_bytes: seed.uploaded_bytes,
                ratio: seed.ratio,
                goal: goal.ratio,
                left_secs: seed.left.as_secs(),
            })
            .collect();
        // Only when it changed: an idle screen should not redraw every ten seconds for nothing.
        if shown.as_ref() != Some(&rows) {
            if tx.send(Event::Seeding(rows.clone())).is_err() {
                break;
            }
            shown = Some(rows);
        }
    }
}

/// The kill-switch half of the guard: a tunnel that drops mid-episode has to stop
/// torrenting, not merely be noticed at the next launch.
async fn vpn_watch(
    guard: VpnGuard,
    session: Arc<anistream_providers::torrent::TorrentSession>,
    tx: mpsc::UnboundedSender<Event>,
) {
    let mut ticker = tokio::time::interval(guard.verify_interval());
    // The first tick fires immediately; the guard was already verified at startup.
    ticker.tick().await;
    let mut was_leaking = false;
    let mut log = anistream_providers::vpn::GuardLog::after_passing();

    loop {
        ticker.tick().await;
        let state = guard.verify().await;
        let leaking = !state.is_protected();

        use anistream_providers::vpn::GuardNote;
        match log.observe(&state, std::time::Instant::now()) {
            GuardNote::Failing(reason) => tracing::warn!(
                %reason,
                action = ?guard.on_leak(),
                "vpn guard failing — halting torrent traffic"
            ),
            GuardNote::StillFailing { reason, minutes, checks } => {
                tracing::warn!(%reason, minutes, checks, "vpn guard still failing")
            }
            GuardNote::Recovered { minutes } => {
                tracing::info!(minutes, badge = %state.badge(), "vpn guard recovered")
            }
            GuardNote::Passed | GuardNote::Quiet => {}
        }

        if leaking && !was_leaking {
            // Marking the provider unavailable only stops *new* requests. Without this,
            // librqbit would carry on downloading and seeding the current episode over an
            // unprotected connection.
            session.halt().await;
        } else if !leaking
            && was_leaking
            && let Err(e) = session.resume().await
        {
            tracing::warn!(error = %e, "could not resume after recovery");
        }
        was_leaking = leaking;

        if tx.send(Event::Vpn(Some(sources::guard_view(&state)))).is_err() {
            break;
        }
    }
}

/// Rebuild the source chain from the configuration as it now stands.
///
/// Called when a setting redefines what sources exist. The chain is swapped *in place*
/// so every holder of a registry clone sees the change, and the plugins that compiled in
/// the background are carried across — they are not described by config and a rebuild
/// that forgot them would silently drop sources the user installed.
#[allow(clippy::too_many_arguments)]
async fn reload_sources(
    registry: &ProviderRegistry,
    torrent_tasks: &mut TorrentTasks,
    vpn_guard: &mut Option<(VpnGuard, Arc<anistream_providers::torrent::TorrentSession>)>,
    config: &anistream_engine::SharedConfig,
    http: &HttpClient,
    paths: &Paths,
    store: &Store,
    tx: &mpsc::UnboundedSender<Event>,
    arrivals: &mpsc::UnboundedSender<(u64, std::result::Result<sources::TorrentUp, String>)>,
) {
    // Old traffic stops before new traffic starts. The other order would leave two
    // sessions briefly alive, and one of them is the one being switched off.
    torrent_tasks.stop().await;
    *vpn_guard = None;

    let plugins = registry.of_kind(anistream_core::traits::ProviderKind::Plugin);
    let snapshot = config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
    let rebuilt = sources::build_registry_deferred(&snapshot, http);
    let mut providers = rebuilt.all();
    providers.extend(plugins);
    registry.replace(providers);
    // The plugins were carried across behind the rebuilt sources; a plugin the order names
    // ahead of one of those goes back in front of it.
    sources::arrange(registry, &snapshot);

    torrent_tasks.bring_up(&snapshot, http, paths, store, tx, arrivals);

    if !sources::torrent_wanted(&snapshot) {
        // No session: say so, and clear a badge that now describes nothing.
        let _ = tx.send(Event::Vpn(None));
    }
    let _ = tx.send(Event::ProviderNote(if sources::torrent_wanted(&snapshot) {
        "sources reloaded — torrents start once the VPN guard passes".into()
    } else {
        "torrents are off — the session was stopped".into()
    }));
    let _ = tx.send(Event::Providers(data::provider_rows(registry)));
    tracing::info!(providers = ?registry.ids(), "source chain reloaded");
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

/// An episode's streams and playback context, as resolution produces them.
type Resolved = (Vec<anistream_core::stream::Stream>, playback::PlaybackContext);

/// How long streams found ahead stay usable. Long enough to cover a viewer who lets the ending
/// and a post-credits scene play out; short enough that a signed stream URL is still good.
const PREFETCH_FRESH: Duration = Duration::from_secs(15 * 60);

/// The next episode's streams, found while the current one's ending plays.
///
/// One entry: only the episode about to play is worth holding, and a newer prefetch replaces
/// an older one. Taken, not copied, by the playback it was for.
#[derive(Clone, Default)]
struct Prefetch(Arc<std::sync::Mutex<Option<Prefetched>>>);

struct Prefetched {
    id: anistream_core::ids::AnilistId,
    episode: String,
    translation: anistream_core::media::Translation,
    found: std::time::Instant,
    resolved: Resolved,
}

impl Prefetch {
    fn keep(
        &self,
        id: anistream_core::ids::AnilistId,
        episode: &str,
        translation: anistream_core::media::Translation,
        resolved: Resolved,
    ) {
        tracing::info!(%episode, streams = resolved.0.len(), "next episode found ahead");
        let entry = Prefetched {
            id,
            episode: episode.to_owned(),
            translation,
            found: std::time::Instant::now(),
            resolved,
        };
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(entry);
    }

    /// The streams found ahead for exactly this episode, translation and title, if still fresh.
    fn take(
        &self,
        id: anistream_core::ids::AnilistId,
        episode: &str,
        translation: anistream_core::media::Translation,
    ) -> Option<Resolved> {
        let mut slot = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // Only a match is taken. Any other play — a show opened in passing, a source picked
        // for a different episode — used to take the entry with it, so the streams found for
        // the episode about to roll on were gone by the time it did.
        let entry = slot.as_ref()?;
        let fresh = entry.found.elapsed() < PREFETCH_FRESH;
        let matches =
            entry.id == id && entry.episode == episode && entry.translation == translation;
        if !fresh {
            *slot = None;
            return None;
        }
        if !matches {
            return None;
        }
        slot.take().map(|entry| entry.resolved)
    }
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
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    controls: &Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
) {
    let Some(controls) = controls.clone() else { return };
    let (meta, store, registry, episode) =
        (meta.clone(), store.clone(), registry.clone(), episode.to_owned());
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

/// What a new playback is about to play, as the window it takes over says while it is found.
fn successor(
    app: &App,
    id: anistream_core::ids::AnilistId,
    episode: &str,
) -> anistream_model::playback::Successor {
    // Now Playing is staged for the new playback before it is dispatched, so its title is the
    // one the terminal shows; a watch party stages nothing, and the open title is the one.
    let title = app
        .playing
        .as_ref()
        .filter(|playing| playing.id == Some(id))
        .map(|playing| playing.title.clone())
        .or_else(|| app.detail.as_ref().filter(|e| e.id == id).map(|e| e.title.clone()))
        .unwrap_or_default();
    anistream_model::playback::Successor {
        id,
        title,
        episode: episode.to_owned(),
        translation: app.config.playback.translation,
    }
}

/// Whether any of these executables is reachable on `PATH`.
///
/// Checked by looking rather than by running: the question is "will mpv find yt-dlp",
/// and mpv looks, it does not run `--version` first.
fn in_path(names: &[&str]) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        names
            .iter()
            .any(|name| dir.join(name).is_file() || dir.join(format!("{name}.exe")).is_file())
    })
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

/// What you were watching, most recent first.
///
/// Trending and continuing are different questions, and this screen used to answer the wrong one:
/// it fetched the current season, which made the section labelled CONTINUE a discovery screen and
/// left an episode abandoned halfway through — the single most likely reason to open the app —
/// with nowhere to appear. Discovery has three sections of its own; this one is personal.
///
/// Built from local history, so it works with no account at all.
///
/// **Ordering is the feature.** `continue_list` orders by `updated_at DESC` and
/// [`AniList::media_many`] preserves the order it was asked in — which it has to, because AniList
/// returns `id_in` results in its own order and letting that through would shuffle the rail into
/// something arbitrary. Together they are what put the thing you last watched at the top.
///
/// An empty result is a real answer here, not a failure to paper over: the screen says so and
/// points at where to find something, rather than silently showing a different kind of list.
async fn continue_entries(
    meta: &Meta,
    store: &Store,
) -> std::result::Result<Vec<anistream_model::catalog::Entry>, String> {
    let continuing = store.continue_list(CONTINUE_ROWS).unwrap_or_default();
    let ids: Vec<anistream_core::ids::AnilistId> =
        continuing.iter().map(|p| p.anilist_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    meta.media_many(&ids)
        .await
        .map(|media| media.iter().map(|m| data::entry_from(m, Some(store))).collect())
        .map_err(|e| e.to_string())
}

/// The calendar's first load: the timeline, plus what the backfill needs to continue it.
struct CalendarLoad {
    entries: Vec<anistream_model::catalog::Entry>,
    /// Pages of past actually fetched, so the backfill knows which page is next.
    pages: u32,
    /// Whether AniList reported broadcasts older than the ones loaded.
    more_past: bool,
}

/// The calendar as one timeline: what recently aired, then what is coming.
///
/// "Is there a tab for latest releases?" was a fair question with no good answer — the calendar
/// only looked forward, so the episodes you could actually watch right now appeared nowhere.
///
/// Two directions rather than one, and not for want of trying: a single ascending query over the
/// whole fortnight returns its page *oldest first*, and the page is exhausted long before it
/// reaches today. So the recent half is fetched newest-first and reversed.
///
/// **Both halves page.** The window was never the real limit — a single page is. Measured against
/// live data there are around 127 broadcasts in a week, and AniList caps `perPage` at 50 whatever
/// you ask for, so one page of past is a day and a half and one page of future is barely two days.
/// A seven-day window served by one page each was a seven-day window in name only.
async fn calendar_timeline(
    meta: &Meta,
    store: &Store,
    anchor: i64,
) -> std::result::Result<CalendarLoad, String> {
    let (past, future) = tokio::join!(
        calendar_past(meta, store, anchor, CALENDAR_INITIAL_PAST_PAGES),
        calendar_future(meta, store, anchor),
    );

    // Either half failing alone is still a usable screen, so only report a failure when both
    // are empty — a calendar showing just the upcoming week beats an error page.
    let past_error = past.as_ref().err().cloned();
    // A past half that failed outright still leaves `more_past` set: there is no evidence the
    // history is empty, only that this attempt did not reach it, and the backfill should retry.
    let (mut entries, pages, more_past) = past.unwrap_or((Vec::new(), 0, true));
    let boundary = entries.len();

    match future {
        Ok(upcoming) => entries.extend(upcoming),
        Err(e) => {
            if entries.is_empty() {
                return Err(past_error.unwrap_or(e));
            }
        }
    }
    if entries.is_empty() {
        return Err(past_error.unwrap_or_else(|| "nothing airing in this window".into()));
    }

    tracing::info!(
        recent = boundary,
        upcoming = entries.len() - boundary,
        pages,
        more_past,
        "calendar timeline"
    );
    Ok(CalendarLoad { entries, pages, more_past })
}

/// One page of past broadcasts, in chronological order.
///
/// The window's *upper* bound is the anchor and the sort is descending, so page numbering means
/// something stable for as long as the anchor holds: page three is the same fifty broadcasts
/// whether it is fetched with the first load or ten minutes later. That is what lets the backfill
/// ask for "the next page" without re-fetching what is on screen, and without a broadcast slipping
/// across a moving boundary and appearing twice. Re-reading the clock per page would break both.
async fn calendar_past_page(
    meta: &Meta,
    store: &Store,
    anchor: i64,
    page: u32,
) -> std::result::Result<(Vec<anistream_model::catalog::Entry>, bool), String> {
    let page = meta
        .airing_between_sorted(
            anchor - CALENDAR_MAX_PAST,
            anchor,
            page,
            CALENDAR_PER_PAGE,
            true,
        )
        .await
        .map_err(|e| e.to_string())?;

    let now = now_epoch();
    // Reversed back into chronological order, so the list reads downward through time.
    let entries =
        page.items.iter().rev().map(|a| data::entry_from_airing(a, now, Some(store))).collect();
    Ok((entries, page.has_next))
}

/// Walk back through pages of history until `pages` are spent or AniList runs out.
///
/// Returns the entries chronologically, how many pages were actually fetched, and whether there
/// is more behind them. Pages are fetched newest-first and each is internally reversed, so the
/// *blocks* have to be reversed too — page two sits above page one on a timeline reading down.
async fn calendar_past(
    meta: &Meta,
    store: &Store,
    anchor: i64,
    pages: u32,
) -> std::result::Result<(Vec<anistream_model::catalog::Entry>, u32, bool), String> {
    let mut blocks: Vec<Vec<anistream_model::catalog::Entry>> = Vec::new();
    let mut loaded = 0;
    let mut more = false;
    let mut failure = None;

    for page in 1..=pages.max(1) {
        match calendar_past_page(meta, store, anchor, page).await {
            Ok((entries, has_next)) => {
                loaded = page;
                more = has_next;
                blocks.push(entries);
                if !has_next {
                    break;
                }
            }
            // A page failing part-way still leaves a usable calendar. Keep what arrived, leave
            // `more` set so the backfill retries this page, and only surface the error if
            // nothing came back at all.
            Err(e) => {
                more = loaded > 0;
                failure = Some(e);
                break;
            }
        }
    }

    if blocks.is_empty() {
        return Err(failure.unwrap_or_else(|| "nothing aired in this window".into()));
    }
    blocks.reverse();
    Ok((blocks.concat(), loaded, more))
}

/// The upcoming half, walked the same way.
///
/// Ascending, so pages concatenate as they arrive with no reversing. There is no backfill for
/// this direction: the far end of the window is a fixed week out rather than open-ended history,
/// and these pages cover it.
async fn calendar_future(
    meta: &Meta,
    store: &Store,
    anchor: i64,
) -> std::result::Result<Vec<anistream_model::catalog::Entry>, String> {
    let mut entries: Vec<anistream_model::catalog::Entry> = Vec::new();
    let mut failure = None;

    for page in 1..=CALENDAR_FUTURE_PAGES {
        match meta
            .airing_between_sorted(
                anchor,
                anchor + CALENDAR_FUTURE,
                page,
                CALENDAR_PER_PAGE,
                false,
            )
            .await
        {
            Ok(fetched) => {
                let now = now_epoch();
                entries.extend(
                    fetched.items.iter().map(|a| data::entry_from_airing(a, now, Some(store))),
                );
                if !fetched.has_next {
                    break;
                }
            }
            Err(e) => {
                failure = Some(e.to_string());
                break;
            }
        }
    }

    match failure {
        Some(e) if entries.is_empty() => Err(e),
        _ => Ok(entries),
    }
}

/// Titles kept in the CONTINUE rail. Long enough to cover what you are actually mid-way through,
/// short enough that it stays a shortlist rather than a second library.
const CONTINUE_ROWS: u32 = 15;

/// The furthest back the calendar will ever reach, and so the floor of every past query.
///
/// It bounds the *backfill*, not the first load: because the sort is descending from the anchor,
/// page one is the fifty most recent broadcasts whatever this is set to. All it decides is when
/// scrolling up finally runs out. Three months is far enough back that reaching the end is a
/// deliberate journey rather than an accident.
const CALENDAR_MAX_PAST: i64 = 90 * 86_400;
/// How far forward the calendar reaches. A week covers a full broadcast cycle, so every airing
/// show you follow has exactly one upcoming episode in view.
const CALENDAR_FUTURE: i64 = 7 * 86_400;
/// Rows per request. AniList's ceiling: ask for 75 or 100 and the response says `perPage: 50`
/// and returns fifty. Depth is bought in pages, never here.
const CALENDAR_PER_PAGE: u32 = 50;
/// Pages of history fetched before the screen is first drawn. At roughly 127 broadcasts a week
/// three pages covers a full week and change, which is what the seven-day window always claimed
/// to offer. Scrolling past them backfills further without being asked.
const CALENDAR_INITIAL_PAST_PAGES: u32 = 3;
/// And forward, covering the week ahead by the same measure.
const CALENDAR_FUTURE_PAGES: u32 = 3;

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// The season containing today, derived from the epoch without a calendar dependency.
fn current_season() -> (anistream_meta::anilist::Season, u16) {
    use anistream_meta::anilist::Season;
    let days = now_epoch() / 86_400;
    // Civil-from-days, the standard algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (Season::of_month(month as u8), year as u16)
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
    fn a_job_starts_only_for_work_someone_waits_on() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut app = test_app();
        let score =
            Command::SetScore { id: anistream_core::ids::AnilistId::new(1), score: 7.0 };
        assert!(JobGuard::start(&mut app, &score, &tx).is_none());
        assert!(JobGuard::start(&mut app, &Command::CheckProviders, &tx).is_some());
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

    #[tokio::test]
    async fn a_job_ends_however_its_work_does_even_by_panicking() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut app = test_app();
        let job = JobGuard::start(&mut app, &Command::CheckProviders, &tx).unwrap();
        let id = job.id;

        let work = tokio::spawn(async move {
            let job = job;
            job.note("held up", None);
            panic!("the provider check fell over");
        });
        assert!(work.await.is_err(), "the task did panic");

        assert!(matches!(rx.recv().await, Some(Event::JobNote { job, .. }) if job == id));
        assert!(matches!(rx.recv().await, Some(Event::JobDone(job)) if job == id));
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
    fn streams_found_ahead_serve_only_the_episode_they_were_found_for() {
        use anistream_core::{ids::AnilistId, media::Translation};
        let context = |episode: &str| playback::PlaybackContext {
            anilist_id: AnilistId::new(7),
            mal_id: None,
            episode: episode.into(),
            title: "t".into(),
            translation: Translation::Sub,
            resume_at: None,
            runtime_secs: None,
            speed: None,
            volume: None,
            next: None,
            episodes: Vec::new(),
        };
        let found = || (Vec::new(), context("6"));
        let prefetch = Prefetch::default();

        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_some());
        assert!(
            prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none(),
            "taken once"
        );

        // Another episode, the other translation or another title start from scratch.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "8", Translation::Sub).is_none());
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Dub).is_none());
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(9), "6", Translation::Sub).is_none());

        // Too old to trust a signed stream URL.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        if let Some(entry) = prefetch.0.lock().unwrap().as_mut() {
            entry.found = std::time::Instant::now() - PREFETCH_FRESH - Duration::from_secs(1);
        }
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_none());

        // Any other play leaves them for the episode they are for: a show opened in passing
        // while the ending ran used to take them with it.
        prefetch.keep(AnilistId::new(7), "6", Translation::Sub, found());
        assert!(prefetch.take(AnilistId::new(9), "1", Translation::Sub).is_none());
        assert!(prefetch.take(AnilistId::new(7), "6", Translation::Dub).is_none());
        assert!(
            prefetch.take(AnilistId::new(7), "6", Translation::Sub).is_some(),
            "still there"
        );
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

    #[test]
    fn the_current_season_is_plausible() {
        let (_, year) = current_season();
        assert!((2024..2100).contains(&year), "derived year {year} is not plausible");
    }

    #[test]
    fn season_derivation_matches_known_dates() {
        use anistream_meta::anilist::Season;
        // Verified against the civil-from-days algorithm.
        let cases = [
            (1_704_067_200_i64, Season::Winter, 2024u16), // 2024-01-01
            (1_719_792_000, Season::Summer, 2024),        // 2024-07-01
            (1_727_740_800, Season::Fall, 2024),          // 2024-10-01
        ];
        for (epoch, season, year) in cases {
            let days = epoch / 86_400;
            let z = days + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let derived_year = if month <= 2 { y + 1 } else { y };
            assert_eq!(Season::of_month(month as u8), season, "for epoch {epoch}");
            assert_eq!(derived_year as u16, year, "for epoch {epoch}");
        }
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
