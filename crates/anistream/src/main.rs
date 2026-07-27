//! anistream — an anime streaming TUI.

use anistream::{artwork, data, downloads, playback, sources, tracking};

use std::{io, sync::Arc, time::Duration};

use anistream_core::config::{Config, Paths, ThemeMode};
use anistream_meta::{anilist::AniList, dataset};
use anistream_net::HttpClient;
use anistream_providers::{ProviderRegistry, vpn::VpnGuard};
use anistream_store::Store;
use anistream_ui::{
    app::{App, Content, EpisodeRow, Task, Toast, Update},
    keymap::Keymap,
    screens, theme,
};
use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(name = "anistream", version, about = "An anime streaming TUI")]
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

    /// Refresh the mapping datasets and exit.
    #[arg(long)]
    refresh_data: bool,

    /// Render one frame to stdout as text and exit.
    ///
    /// Lets the layout be inspected without an interactive terminal — useful for checking
    /// the design at a given size, and for seeing it at all over a non-tty connection.
    #[arg(long, value_name = "WIDTHxHEIGHT")]
    preview: Option<String>,

    /// Which screen to preview: home, search, title, calendar, library, downloads, providers,
    /// accounts, settings, help, palette.
    #[arg(long, default_value = "home")]
    screen: String,

    /// Sign in to a tracker and exit.
    ///
    /// With no value, runs the configured flow: `loopback` opens a browser and catches the
    /// token automatically, `paste` prints the URL and reads the token from stdin. Pass `-` to
    /// read from stdin regardless, or a token directly — though that puts a year-long account
    /// credential in your shell history.
    #[arg(long, value_name = "TOKEN", num_args = 0..=1, default_missing_value = "")]
    login: Option<String>,

    /// Which tracker `--login` is for.
    #[arg(long, default_value = "anilist")]
    tracker: String,

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

    /// Emit JSON instead of text, for the commands that support it.
    ///
    /// The point of the non-interactive commands: composable with `jq` and friends rather than a
    /// walled garden.
    #[arg(long)]
    json: bool,

    /// Resolve one episode to a playable URL, print it, and hold it open.
    ///
    /// The thing that was missing when playback failed silently: something to point mpv, VLC or
    /// curl at by hand, so "anistream is broken" and "my player is broken" can be told apart. The
    /// torrent session has to stay alive for the URL to serve anything, so this waits for Ctrl-C
    /// rather than exiting.
    #[arg(long, value_name = "ANILIST_ID")]
    stream_url: Option<u32>,

    /// Which episode `--stream-url` should resolve. Accepts non-numeric ids like `OVA`.
    #[arg(long, default_value = "1")]
    episode: String,

    /// List installed plugins with what each one is permitted to reach, then exit.
    ///
    /// A plugin registry means running other people's code, so what a plugin may contact should
    /// be inspectable without reading its source.
    #[arg(long)]
    plugins: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::resolve().context("resolving config paths")?;
    init_logging(&paths);

    let mut config = Config::load(&paths).unwrap_or_else(|e| {
        // A malformed config must not stop the app from starting, but it must be loud —
        // silently ignoring it could mean silently ignoring a VPN setting.
        eprintln!("warning: {e}\nfalling back to defaults");
        Config::default()
    });
    if let Some(mode) = &cli.theme {
        config.theme.mode =
            if mode == "immersive" { ThemeMode::Immersive } else { ThemeMode::Adaptive };
    }

    let http = HttpClient::new(&config.network).context("building http client")?;
    let store = Store::open(paths.database()).context("opening the local database")?;

    if cli.doctor {
        return doctor(&config, &store).await;
    }
    if cli.refresh_data {
        return refresh_data(&store, &http).await;
    }
    if cli.login_url {
        return print_login_url(&config, &cli.tracker);
    }
    if let Some(token) = &cli.login {
        return login(&config, &cli.tracker, token, &http).await;
    }
    if cli.logout {
        return forget_token(&config, &cli.tracker);
    }
    if cli.sync {
        return sync_once(&config, &store, &http).await;
    }
    if cli.token_to_file {
        return migrate_token(&config, &cli.tracker);
    }
    if cli.plugins {
        return list_plugins(&config, &paths).await;
    }
    if let Some(id) = cli.stream_url {
        return stream_url(&config, &http, &store, &paths, id, &cli.episode).await;
    }
    if let Some(query) = &cli.search {
        return search_cli(&config, &http, &store, query, cli.json).await;
    }
    if cli.stats {
        return stats_cli(&store, cli.json);
    }
    if let Some(path) = &cli.export {
        return export_cli(&store, path);
    }
    if let Some(path) = &cli.import {
        return import_cli(&store, path);
    }
    if cli.random {
        return random_cli(&store, cli.json);
    }
    if let Some(size) = &cli.preview {
        return preview(config, http, store, &paths, size, &cli.screen).await;
    }

    run(config, paths, http, store).await
}

/// Render one frame with live data and print it as text.
async fn preview(
    config: Config,
    http: HttpClient,
    store: Store,
    paths: &Paths,
    size: &str,
    screen: &str,
) -> Result<()> {
    use anistream_ui::nav::{Overlay, Section};
    use ratatui::{Terminal, backend::TestBackend};

    let (width, height) = size
        .split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?)))
        .unwrap_or((120u16, 34u16));

    let palette = theme::resolve_with(config.theme.mode, None);
    let anilist = AniList::new(http.clone(), config.network.anilist_rate_limit);
    let engine = anistream_ui::image::ImageEngine::detect(true);
    let graphics = engine.graphics();
    let mut app = App::with_images(config, palette, Keymap::new(), engine);

    let entries = match screen {
        "calendar" => {
            let now = now_epoch();
            calendar_timeline(&anilist, &store, now).await.unwrap_or_default()
        }
        "search" => anilist
            .search("frieren", 1, 20)
            .await
            .map(|p| p.items.iter().map(|m| data::entry_from(m, Some(&store))).collect())
            .unwrap_or_default(),
        // Through the same function the running app uses, or `--preview` would render a screen
        // that does not exist — which defeats the point of having it.
        _ => continue_entries(&anilist, &store).await.unwrap_or_default(),
    };

    // The broadcast line needs its own request, so the preview has to make it too — otherwise
    // `--preview` would render a frame that is subtly different from the running app, which
    // defeats the point of having it.
    let mut entries = entries;
    let airing: Vec<anistream_core::ids::AnilistId> =
        entries.iter().filter(|e| e.airing_in.is_some()).map(|e| e.id).collect();
    if !airing.is_empty()
        && let Ok(rows) = anilist.last_aired(&airing).await
    {
        let now = now_epoch();
        for row in rows {
            if let Some(entry) = entries.iter_mut().find(|e| e.id == row.media_id) {
                entry.last_aired = Some((row.episode, now.saturating_sub(row.airing_at)));
            }
        }
    }

    match screen {
        "search" => {
            app.go_to_section(Section::Search);
            app.search_query = "frieren".into();
        }
        "calendar" => {
            app.go_to_section(Section::Calendar);
        }
        "settings" => {
            app.go_to_section(Section::Settings);
        }
        "accounts" => {
            app.go_to_section(Section::Accounts);
            for state in tracking::Sync::build(&app.config, &store, &http).initial_states() {
                app.apply(Update::Sync(Box::new(state)));
            }
        }
        "providers" => {
            let (registry, _, note) = sources::build_registry(&app.config, &http, paths).await;
            registry.check_all(now_epoch()).await;
            app.go_to_section(Section::Providers);
            if let Some(note) = note {
                app.apply(Update::ProviderNote(note));
            }
            app.apply(Update::Providers(data::provider_rows(&registry)));
        }
        _ => {}
    }
    app.apply(Update::Content(Content::Entries(entries)));
    app.nav.focus_stage();

    match screen {
        "title" => {
            // Fetch the full record so availability badges and the synopsis are real.
            if let Some(first) = app.selected_entry().map(|e| e.id)
                && let Ok(media) = anilist.media(first).await
            {
                app.apply(Update::Detail(Box::new(data::entry_from(&media, Some(&store)))));
            }
            app.handle(anistream_ui::keymap::Action::Open, 20);
        }
        "episodes" => {
            if let Some(id) = app.selected_entry().map(|e| e.id) {
                app.handle(anistream_ui::keymap::Action::Open, 20);
                if let Ok(media) = anilist.media(id).await {
                    app.apply(Update::Detail(Box::new(data::entry_from(&media, Some(&store)))));
                }
                app.handle(anistream_ui::keymap::Action::ShowEpisodes, 20);
                let (registry, _, note) =
                    sources::build_registry(&app.config, &http, paths).await;
                match load_episodes(&anilist, &store, &registry, id).await {
                    Ok(EpisodeLoad::Rows(rows)) => app.apply(Update::Episodes(rows)),
                    // A still frame cannot answer a question, so state it and carry on.
                    Ok(EpisodeLoad::Choose { candidates, .. }) => {
                        eprintln!("[episodes: {} possible matches, none confident]", candidates.len());
                    }
                    Err(reason) => {
                        eprintln!("[episodes: {reason}]");
                        if let Some(note) = note {
                            eprintln!("[sources: {note}]");
                        }
                    }
                }
            }
        }
        "help" => app.nav.open_overlay(Overlay::Help),
        "palette" => {
            app.nav.open_overlay(Overlay::CommandPalette);
            app.palette_query = "ep".into();
        }
        _ => {}
    }

    // The event loop normally drives artwork; a one-shot render has to fetch inline so the
    // preview shows what the real UI would.
    for url in app.visible_artwork(height as usize) {
        if let Ok(response) = http.plain().get(&url).send().await
            && response.status().is_success()
            && let Ok(bytes) = response.bytes().await
            && let Ok(decoded) = image::load_from_memory(&bytes)
        {
            app.apply(Update::Image {
                url,
                image: Box::new(anistream_ui::image::downscale(decoded, 900)),
            });
        }
    }
    eprintln!("[graphics: {graphics:?}, {} image(s) loaded]", app.images.len());

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

/// Report the environment, without starting the UI.
async fn doctor(config: &Config, store: &Store) -> Result<()> {
    let engine = anistream_ui::image::ImageEngine::detect(true);
    let background = theme::detect::detect_background();
    let palette = theme::resolve_with(config.theme.mode, background);

    println!("terminal");
    println!("  graphics    {:?}", engine.graphics());
    println!("  cell size   {:?}", engine.font_size());
    match background {
        Some(bg) => println!(
            "  background  #{:02X}{:02X}{:02X}  luminance {:.3}",
            bg.r,
            bg.g,
            bg.b,
            bg.luminance()
        ),
        None => println!("  background  not reported (OSC 11 unanswered)"),
    }
    println!("  palette     {:?}", palette.variant);

    println!();
    println!("data");
    println!("  mappings    {} titles", store.mapping_count().unwrap_or(0));
    println!("  with offset {}", store.mappings_with_offset_count().unwrap_or(0));
    for spec in dataset::MAPPING_DATASETS {
        match store.dataset_state(spec.name) {
            Ok(Some(state)) => println!(
                "  {:<11} fetched {} · {} entries{}",
                spec.name,
                state.fetched_at.map_or("never".into(), |t| format!("at {t}")),
                state.item_count.unwrap_or(0),
                state.last_error.map_or(String::new(), |e| format!(" · last error: {e}"))
            ),
            _ => println!("  {:<11} never fetched", spec.name),
        }
    }

    // The player was missing from this report entirely, which is a strange omission for the one
    // external program the app cannot work without. Reported from real use: mpv accepted a URL
    // and then never rendered anything, and `--doctor` had nothing to say about it.
    println!();
    println!("player");
    let mpv = anistream_player::Mpv::new(std::env::temp_dir())
        .with_binary(config.playback.mpv_binary.clone())
        .with_extra_args(config.playback.mpv_args.clone());
    match mpv.version().await {
        Some(version) => {
            println!("  mpv         ● {version}");
            // mpv reads its own config, and a bad `vo` or `hwdec` there will hang playback in a
            // way anistream cannot distinguish from a dead source. Point at it by name.
            let conf = dirs_config_mpv();
            match conf.as_ref().filter(|p| p.exists()) {
                Some(path) => println!(
                    "  mpv.conf    {} — try `mpv --no-config <file>` if playback hangs",
                    path.display()
                ),
                None => println!("  mpv.conf    none"),
            }
        }
        None => {
            println!("  mpv         ✕ not runnable as {:?}", config.playback.mpv_binary);
            println!("              install it (brew install mpv), or set playback.mpv_binary");
        }
    }

    println!();
    println!("sources");
    if !config.providers.torrent.enabled {
        println!("  torrent     disabled (set providers.torrent.enabled and a vpn mode)");
        return Ok(());
    }

    println!("  torrent     enabled, vpn mode {:?}", config.providers.torrent.vpn.mode);

    // Which indexer, if any. anistream ships none, so "enabled" on its own does not mean
    // the source has anything to search.
    match config.providers.torrent.indexer_host() {
        Some(host) => println!("  indexer     {host}"),
        None => println!("  indexer     none configured (set providers.torrent.rss_url)"),
    }
    if config.providers.torrent.trackers.is_empty() {
        println!(
            "  trackers    none configured — DHT is off in proxy mode, so no peers will be found"
        );
    } else {
        println!("  trackers    {} configured", config.providers.torrent.trackers.len());
    }

    // Actually run the guard. Reporting the configured mode without checking it would be
    // useless in the one situation where you reach for this command.
    match VpnGuard::new(config.providers.torrent.vpn.clone()) {
        Ok(guard) => {
            print!("  verifying egress through the proxy… ");
            use std::io::Write;
            let _ = std::io::stdout().flush();

            let state = guard.verify().await;
            println!("{}", state.badge());
            match state.reason() {
                Some(reason) => println!("  ✕ torrent source will NOT start: {reason}"),
                None => println!("  ● torrent source will start"),
            }
            if guard.must_disable_dht() {
                println!("  note        DHT disabled in proxy mode (UDP-associate unverified)");
            }

            // The distinction that actually matters. Everything above is application-level
            // and can be defeated by a bug here; only a firewall rule makes leaking
            // impossible the way an interface bind does.
            let enforcement = anistream_providers::vpn::detect_kernel_enforcement().await;
            println!();
            println!("  os-level kill switch");
            match enforcement {
                anistream_providers::vpn::KernelEnforcement::Enforced => {
                    println!("    ● ENFORCED — {}", enforcement.advice());
                }
                _ => {
                    println!("    ▲ {}", enforcement.advice());
                    println!();
                    println!("    Without it, anistream's guard is defence in depth, not a");
                    println!(
                        "    guarantee: it stops anistream leaking, but cannot stop a bug"
                    );
                    println!("    in anistream or any other process on this machine.");
                }
            }
        }
        Err(reason) => println!("  ✕ vpn guard misconfigured: {reason}"),
    }
    Ok(())
}

/// Print the URL to open in a browser to authorise a tracker.
fn print_login_url(config: &Config, tracker: &str) -> Result<()> {
    if tracker != "anilist" {
        anyhow::bail!("no sign-in flow for {tracker:?} yet");
    }
    let auth = &config.trackers.anilist;
    let missing = |field: &Option<String>| field.as_deref().unwrap_or("").trim().is_empty();
    if missing(&auth.client_id) || missing(&auth.client_secret) {
        print_setup_help(auth);
        return Ok(());
    }

    let flow = auth_flow(auth);
    let url = anistream_track::auth::authorize_url(
        auth.client_id.as_deref().unwrap_or_default(),
        flow,
        auth.redirect_port,
    )?;
    println!("{url}");
    if matches!(flow, anistream_track::auth::Flow::Paste) {
        println!();
        println!("Then: anistream --login   (reads the code from stdin)");
    }
    Ok(())
}

/// The unavoidable setup step, spelled out.
///
/// AniList has no public client and only supports the authorization code grant, so there is no
/// way to avoid asking. Printing exactly what to do beats a 401 the user has to decode.
fn print_setup_help(auth: &anistream_core::config::AniListAuthConfig) {
    println!("AniList credentials are not configured.");
    println!();
    println!("AniList supports only the authorization code grant — the implicit grant returns");
    println!("`unsupported_grant_type` and PKCE is ignored — so a client secret is required.");
    println!("anistream ships no credentials of its own, because a secret inside an");
    println!("open-source binary is not a secret. Register your own client instead:");
    println!();
    println!("  1. open https://anilist.co/settings/developer");
    println!("  2. \"Create New Client\"");
    println!("  3. name it anything; set the redirect URL to exactly");
    println!("       {}", anistream_track::auth::redirect_uri(auth.redirect_port));
    println!("  4. put both halves in your config:");
    println!();
    println!("       [trackers]");
    println!("       enabled = [\"anilist\"]");
    println!();
    println!("       [trackers.anilist]");
    println!("       client_id = \"<id>\"");
    println!("       client_secret = \"<secret>\"");
    println!();
    println!(
        "On a machine with no browser, set flow = \"paste\" and register the redirect URL"
    );
    println!("as https://anilist.co/api/v2/oauth/pin instead.");
}

fn auth_flow(auth: &anistream_core::config::AniListAuthConfig) -> anistream_track::auth::Flow {
    if auth.flow == "paste" {
        anistream_track::auth::Flow::Paste
    } else {
        anistream_track::auth::Flow::Loopback
    }
}

/// Sign in: run the configured flow, or take a code that was obtained by hand.
///
/// The code is exchanged for a token here rather than stored as-is — an authorization code is
/// single-use and short-lived, so keeping one would be storing something already spent.
async fn login(config: &Config, tracker: &str, given: &str, http: &HttpClient) -> Result<()> {
    if tracker == "mal" {
        return login_mal(config, http).await;
    }
    // Simkl and Trakt sign in with a device code, which is a *better* fit for a CLI than the
    // loopback flow: nothing to register, and it works over SSH where opening a browser on the
    // wrong machine does not.
    if let Some(endpoints) = device_endpoints_for(tracker) {
        return login_device(config, tracker, endpoints, http).await;
    }
    if tracker != "anilist" {
        anyhow::bail!("no sign-in flow for {tracker:?} yet");
    }
    let auth = &config.trackers.anilist;
    let missing = |field: &Option<String>| field.as_deref().unwrap_or("").trim().is_empty();
    if missing(&auth.client_id) || missing(&auth.client_secret) {
        print_setup_help(auth);
        anyhow::bail!("cannot sign in without client_id and client_secret");
    }
    let (client_id, client_secret) = (
        auth.client_id.as_deref().unwrap_or_default(),
        auth.client_secret.as_deref().unwrap_or_default(),
    );
    let flow = auth_flow(auth);
    let redirect = match flow {
        anistream_track::auth::Flow::Loopback => {
            anistream_track::auth::redirect_uri(auth.redirect_port)
        }
        // AniList checks the redirect again at exchange time, so it has to be the same value
        // that was authorised against.
        anistream_track::auth::Flow::Paste => anistream_track::auth::PIN_REDIRECT.to_owned(),
    };

    let raw = if !given.is_empty() && given != "-" {
        given.to_owned()
    } else if given == "-" {
        read_line("code: ")?
    } else {
        let url = anistream_track::auth::authorize_url(client_id, flow, auth.redirect_port)?;
        match flow {
            anistream_track::auth::Flow::Loopback => {
                // Listen before opening the browser: a fast redirect could otherwise arrive
                // before the socket exists.
                let waiting = tokio::spawn(anistream_track::auth::wait_for_code_from(
                    auth.redirect_port,
                    "AniList",
                ));
                println!("Opening your browser to authorise anistream…");
                println!("  {url}");
                if let Err(e) = open::that_detached(&url) {
                    println!("(could not open it automatically: {e} — open the URL above)");
                }
                println!();
                println!("Waiting for the redirect on 127.0.0.1:{} …", auth.redirect_port);
                waiting.await??
            }
            anistream_track::auth::Flow::Paste => {
                println!("Open this, authorise, then paste the code AniList shows you:");
                println!("  {url}");
                println!();
                read_line("code: ")?
            }
        }
    };

    // Accepts a bare code or a whole pasted redirect URL — pasting the address bar is an
    // obvious thing to try.
    let Some(code) = anistream_track::auth::extract_code(&raw) else {
        anyhow::bail!(
            "that does not look like a code — paste the `code` value or the whole redirect URL"
        );
    };

    println!("Exchanging the code for a token…");
    let token = anistream_track::auth::exchange_code(
        http.plain(),
        client_id,
        client_secret,
        &redirect,
        &code,
    )
    .await?;

    let store = tracking::token_store(config);
    let storage = store.set(tracker, &token)?;
    println!("● signed in — token stored in the {}", storage.describe());
    if let Some(exp) = anistream_track::auth::token_expiry(&token) {
        // Read out of the token rather than assumed, so "sign in again around then" is a fact.
        let days = (exp - anistream_store::now()) / 86_400;
        println!("  valid for about {days} days");
    }
    if storage.is_degraded() {
        println!("  note: no OS keychain was available, so it is a file readable only by you");
    }
    println!();
    println!("Check it with: anistream --sync");
    Ok(())
}

/// Sign in to MyAnimeList.
///
/// A separate function from the AniList flow rather than a branch inside it, because almost nothing
/// is shared: PKCE instead of a secret, a form-encoded exchange instead of JSON, and a token pair
/// with an expiry instead of a bare year-long token. Forcing them into one function would mean a
/// parameter for every difference.
/// Device-flow endpoints for a tracker, if it uses one.
fn device_endpoints_for(tracker: &str) -> Option<anistream_track::DeviceEndpoints> {
    match tracker {
        "simkl" => Some(anistream_track::device::SIMKL),
        "trakt" => Some(anistream_track::device::TRAKT),
        _ => None,
    }
}

/// Sign in with the OAuth device flow, from the command line.
async fn login_device(
    config: &Config,
    tracker: &str,
    endpoints: anistream_track::DeviceEndpoints,
    http: &HttpClient,
) -> Result<()> {
    let (client_id, client_secret) = match tracker {
        "simkl" => (config.trackers.simkl.client_id.clone(), None),
        _ => (
            config.trackers.trakt.client_id.clone(),
            config.trackers.trakt.client_secret.clone(),
        ),
    };
    let Some(client_id) = client_id.filter(|id| !id.trim().is_empty()) else {
        anyhow::bail!(
            "set trackers.{tracker}.client_id in your config first — register an app at {}",
            if tracker == "simkl" {
                "https://simkl.com/settings/developer"
            } else {
                "https://trakt.tv/oauth/applications"
            }
        );
    };

    let plain = http.plain().clone();
    let code = anistream_track::device::request_code(&plain, &endpoints, &client_id)
        .await
        .with_context(|| format!("requesting a {tracker} device code"))?;

    // Printed rather than only opened: over SSH the browser that opens is on the wrong machine, and
    // the code is the only thing the user actually needs.
    println!();
    println!("  1. open  {}", code.verification_url);
    println!("  2. enter {}", code.user_code);
    println!();
    println!("waiting for approval (expires in {}s)…", code.expires_in);
    if open::that_detached(&code.verification_url).is_err() {
        println!("(could not open a browser for you — the URL above is the whole of it)");
    }
    // Flushed explicitly, because this is the one place in the app where *unflushed output is a
    // broken feature*. Rust block-buffers stdout when it is not a terminal, so piping this command
    // — or running it from a wrapper, or a background job — held the code in the buffer until the
    // process exited, by which time it had expired. There is nothing to type and no way to know why.
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let pair = anistream_track::device::poll_for_token(
        &plain,
        &endpoints,
        &client_id,
        client_secret.as_deref(),
        &code,
    )
    .await
    .with_context(|| format!("{tracker} sign-in"))?;

    let tokens = tracking::token_store(config);
    let storage = tokens
        .set_pair(tracker, &pair.access, pair.refresh.as_deref(), pair.expires_at)
        .with_context(|| format!("storing the {tracker} token"))?;
    println!("signed in to {tracker} — token in the {}", storage.describe());
    if pair.refresh.is_none() {
        // Worth saying: without one, this token cannot be renewed and a re-sign-in is eventually
        // required. Simkl's do not expire, so there is nothing to renew.
        println!("no refresh token issued (Simkl's tokens do not expire)");
    }
    Ok(())
}

async fn login_mal(config: &Config, http: &HttpClient) -> Result<()> {
    use anistream_track::mal;

    let auth = &config.trackers.mal;
    let Some(client_id) = auth.client_id.as_deref().filter(|id| !id.trim().is_empty()) else {
        println!("No MyAnimeList client id configured.");
        println!();
        println!("  1. open https://myanimelist.net/apiconfig and create an app");
        println!(
            "  2. App Type: **other** — that makes it a public client, so no secret is needed"
        );
        println!("  3. App Redirect URL, exactly:");
        println!("       {}", anistream_track::auth::redirect_uri(auth.redirect_port));
        println!("  4. put the Client ID in your config:");
        println!();
        println!("       [trackers]");
        println!("       enabled = [\"anilist\", \"mal\"]");
        println!();
        println!("       [trackers.mal]");
        println!("       client_id = \"<id>\"");
        anyhow::bail!("cannot sign in without a client id");
    };

    // The verifier has to survive until the exchange, so it is generated before the browser opens
    // and held across the wait. MAL only supports `plain`, so it is also the challenge.
    let pkce = mal::Pkce::generate();
    let redirect = anistream_track::auth::redirect_uri(auth.redirect_port);
    let url = mal::authorize_url(client_id, &pkce, &redirect);

    // Listen before opening the browser, or a fast redirect arrives before the socket exists.
    let waiting = tokio::spawn(anistream_track::auth::wait_for_code_from(
        auth.redirect_port,
        "MyAnimeList",
    ));
    println!("Opening your browser to authorise anistream on MyAnimeList…");
    println!("  {url}");
    if let Err(e) = open::that_detached(&url) {
        println!("(could not open it automatically: {e} — open the URL above)");
    }
    println!();
    println!("Waiting for the redirect on 127.0.0.1:{} …", auth.redirect_port);
    let code = waiting.await??;

    println!("Exchanging the code for a token…");
    let pair = mal::exchange_code(
        http.plain(),
        client_id,
        &code,
        &pkce.verifier,
        &redirect,
        anistream_store::now(),
    )
    .await?;

    let store = tracking::token_store(config);
    let storage =
        store.set_pair("mal", &pair.access, pair.refresh.as_deref(), pair.expires_at)?;
    println!("● signed in to MyAnimeList — token stored in the {}", storage.describe());
    if let Some(expires_at) = pair.expires_at {
        let days = (expires_at - anistream_store::now()) / 86_400;
        println!("  valid for about {days} days, and renewed automatically before it lapses");
    }
    if pair.refresh.is_none() {
        // Worth flagging: without one, sync stops in a month and the only fix is signing in again.
        println!(
            "  ▲ no refresh token was issued — you will have to sign in again when it expires"
        );
    }
    println!();
    println!("Check it with: anistream --sync");
    Ok(())
}

/// Prompt on stderr and read one line from stdin.
///
/// stderr so the prompt does not end up in a pipe, and stdin so a year-long account credential
/// never has to appear in shell history.
fn read_line(prompt: &str) -> Result<String> {
    use std::io::Write;
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line)
}

/// Move a token from the keychain to a `0600` file.
fn migrate_token(config: &Config, tracker: &str) -> Result<()> {
    let store = tracking::token_store(config);
    match store.migrate_to_file(tracker) {
        Ok(_) => {
            println!("● moved the {tracker} token to {}", tracking::token_dir().display());
            println!("  it is a 0600 file, readable only by you (and root)");
            if config.trackers.token_storage != "file" {
                // Otherwise the next run would go back to the keychain, find nothing, and look
                // like the migration silently failed.
                println!();
                println!("Set this so it is actually used:");
                println!("  [trackers]");
                println!("  token_storage = \"file\"");
                println!();
                println!("Or for a single run: ANISTREAM_TOKEN_STORAGE=file anistream …");
            }
            Ok(())
        }
        Err(anistream_track::secret::SecretError::Missing(_)) => {
            println!("No {tracker} token found in the keychain — nothing to move.");
            println!("Sign in with: anistream --login");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn forget_token(config: &Config, tracker: &str) -> Result<()> {
    tracking::token_store(config).clear(tracker)?;
    println!("forgot the {tracker} token");
    let _ = config;
    Ok(())
}

/// Drain and pull once, reporting what happened.
async fn sync_once(config: &Config, store: &Store, http: &HttpClient) -> Result<()> {
    let sync = tracking::Sync::build(config, store, http);
    if sync.trackers.is_empty() {
        println!("no tracker enabled — set trackers.enabled in your config");
        return Ok(());
    }

    for tracker in &sync.trackers {
        let id = tracker.id();
        println!("── {id} ─────────────────────────────────────────────");
        println!("  authenticated  {}", tracker.is_authenticated());
        println!("  queued         {}", store.outbox_depth(Some(id)).unwrap_or(0));
        if !tracker.is_authenticated() {
            println!("  ▲ not signed in — run: anistream --login-url");
            continue;
        }

        let now = anistream_store::now();
        match anistream_track::drain(store, tracker.as_ref(), now).await {
            Ok(report) => println!(
                "  drain          sent {} · failed {} · remaining {}{}",
                report.sent,
                report.failed,
                report.remaining,
                if report.needs_reauth { " · NEEDS RE-AUTH" } else { "" }
            ),
            Err(e) => println!("  ✕ drain: {e}"),
        }

        let last = store.get_meta_i64(&format!("last_pull:{id}")).unwrap_or(None).unwrap_or(0);
        match anistream_track::pull(store, tracker.as_ref(), now, last).await {
            Ok(report) => {
                let _ = store.set_meta_i64(&format!("last_pull:{id}"), now);
                println!(
                    "  pull           {} titles · queued {} · adopted {} · conflicts {}",
                    report.seen,
                    report.queued,
                    report.adopted,
                    report.conflicts.len()
                );
                for conflict in report.conflicts.iter().take(5) {
                    println!(
                        "    ▲ {} {}: mine {} / theirs {}",
                        store
                            .cached_title(conflict.anilist_id)
                            .unwrap_or(None)
                            .unwrap_or_else(|| conflict.anilist_id.get().to_string()),
                        conflict.field.label(),
                        conflict.local,
                        conflict.remote
                    );
                }
            }
            Err(e) => println!("  ✕ pull: {e}"),
        }
    }
    Ok(())
}

/// List installed plugins and what each may reach.
///
/// Loaded with **no HTTP client**, so inspecting a plugin cannot cause it to make a request. A
/// plugin describing itself is the one call that runs with no capabilities at all.
async fn list_plugins(config: &Config, paths: &Paths) -> Result<()> {
    let dir = paths.plugin_dir();
    println!("plugin directory  {}", dir.display());

    let limits = anistream_plugin::Limits {
        memory_bytes: config.providers.plugins.memory_mb.saturating_mul(1024 * 1024),
        deadline: Duration::from_secs(config.providers.plugins.deadline_secs.max(1)),
        ..Default::default()
    };
    println!(
        "limits            {} MiB memory · {:?} per call",
        config.providers.plugins.memory_mb, limits.deadline
    );
    if !config.providers.order.iter().any(|p| p == "plugins") {
        println!(
            "note              \"plugins\" is not in providers.order, so none will be used"
        );
    }
    println!();

    let host = anistream_plugin::PluginHost::new(limits, None)?;
    let loaded = host.load_dir(&dir).await;
    if loaded.is_empty() {
        println!("No plugins installed.");
        println!();
        println!(
            "Drop a WebAssembly component into that directory. To build the reference one:"
        );
        println!(
            "  cargo build --release --target wasm32-wasip2 \\\n    \
             --manifest-path plugins/example-rust/Cargo.toml"
        );
        return Ok(());
    }

    for result in loaded {
        match result {
            Ok(plugin) => {
                let manifest = plugin.manifest();
                println!("● {}  {}", manifest.id, manifest.version);
                println!("  name          {}", manifest.display_name);
                println!("  translations  {}", manifest.translation_types.join(", "));
                // The line that matters: everything this plugin can reach, enforced host-side.
                if manifest.allowed_hosts.is_empty() {
                    println!("  may contact    nothing (declares no hosts)");
                } else {
                    println!("  may contact   {}", manifest.allowed_hosts.join(", "));
                }
                println!("  cannot        read files, open sockets, read your environment");
            }
            Err(e) => println!("✕ {e}"),
        }
        println!();
    }
    Ok(())
}

/// Search AniList from the shell.
///
/// Titles are remembered as they pass through, so a later `--stats` or sync conflict can name a
/// show you have only searched for.
async fn search_cli(
    config: &Config,
    http: &HttpClient,
    store: &Store,
    query: &str,
    json: bool,
) -> Result<()> {
    let anilist = AniList::new(http.clone(), config.network.anilist_rate_limit);
    let page = anilist.search(query, 1, 20).await?;

    for media in &page.items {
        let _ = store.remember_title(media.id, media.title.display());
    }

    if json {
        // Built explicitly rather than serialising `Media`: this is a published surface, and
        // leaking the AniList response shape would make every upstream field change a breaking one.
        let rows: Vec<serde_json::Value> = page
            .items
            .iter()
            .map(|m| {
                serde_json::json!({
                    "anilist_id": m.id.get(),
                    "title": m.title.display(),
                    "romaji": m.title.romaji,
                    "english": m.title.english,
                    "format": m.format.map(|f| format!("{f:?}").to_uppercase()),
                    "episodes": m.episodes,
                    "year": m.season_year,
                    "score": m.average_score,
                    "progress": store.progress(m.id).ok().flatten().map(|p| p.episodes_done),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    println!("{:<9} {:>4} {:>5} {:>4}  title", "id", "eps", "year", "%");
    println!("{}", "─".repeat(70));
    for media in &page.items {
        println!(
            "{:<9} {:>4} {:>5} {:>4}  {}",
            media.id.get(),
            media.episodes.map_or("—".into(), |e| e.to_string()),
            media.season_year.map_or("—".into(), |y| y.to_string()),
            media.average_score.map_or("—".into(), |s| s.to_string()),
            media.title.display()
        );
    }
    Ok(())
}

fn stats_cli(store: &Store, json: bool) -> Result<()> {
    let stats = store.stats()?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "titles": stats.titles,
                "episodes_completed": stats.episodes_completed,
                "episodes_started": stats.episodes_started,
                "watched_secs": stats.watched_secs,
                "watched_human": stats.watched_human(),
                "first_at": stats.first_at,
                "last_at": stats.last_at,
                "top_provider": stats.top_provider.as_ref().map(|(id, n)| {
                    serde_json::json!({ "id": id, "episodes": n })
                }),
            }))?
        );
        return Ok(());
    }

    if stats.titles == 0 {
        println!("Nothing watched yet.");
        return Ok(());
    }
    println!("  titles          {}", stats.titles);
    println!("  episodes        {} finished", stats.episodes_completed);
    println!("  started         {}", stats.episodes_started);
    println!("  watch time      {}", stats.watched_human());
    if let Some((provider, count)) = &stats.top_provider {
        println!("  mostly via      {provider} ({count} episodes)");
    }
    Ok(())
}

/// Write an export. `-` means stdout, so it composes with a pipe.
fn export_cli(store: &Store, path: &str) -> Result<()> {
    let export = store.export(anistream_store::now())?;
    let json = serde_json::to_string_pretty(&export)?;

    if path == "-" {
        println!("{json}");
    } else {
        std::fs::write(path, &json).with_context(|| format!("writing the export to {path}"))?;
        eprintln!("wrote {} titles to {path}", export.titles.len());
    }
    Ok(())
}

fn import_cli(store: &Store, path: &str) -> Result<()> {
    let raw = if path == "-" {
        std::io::read_to_string(std::io::stdin()).context("reading the export from stdin")?
    } else {
        std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?
    };
    let export: anistream_store::stats::Export =
        serde_json::from_str(&raw).context("parsing the export")?;

    let advanced = store.import(&export, anistream_store::now())?;
    // Progress is monotonic, so an unchanged count is a normal outcome rather than a failure —
    // re-importing the same file, or restoring an older backup, both land here.
    println!(
        "imported {} of {} titles ({} already up to date)",
        advanced,
        export.titles.len(),
        export.titles.len().saturating_sub(advanced as usize)
    );
    Ok(())
}

fn random_cli(store: &Store, json: bool) -> Result<()> {
    let Some(id) = store.random_watched()? else {
        if json {
            println!("null");
        } else {
            println!("Nothing in your history yet.");
        }
        return Ok(());
    };
    let title = store.cached_title(id).unwrap_or(None);
    let progress = store.progress(id).ok().flatten();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "anilist_id": id.get(),
                "title": title,
                "episodes_completed": progress.as_ref().map(|p| p.episodes_done),
                "next_episode": progress.as_ref().map(|p| p.episodes_done + 1),
            }))?
        );
    } else {
        let name = title.unwrap_or_else(|| format!("anilist {}", id.get()));
        match progress {
            Some(p) => println!("{name} — next up: episode {}", p.episodes_done + 1),
            None => println!("{name}"),
        }
    }
    Ok(())
}

async fn refresh_data(store: &Store, http: &HttpClient) -> Result<()> {
    let now = now_epoch();
    for (name, outcome) in dataset::refresh_all(store, http, now, true).await {
        println!("{name:<12} {outcome:?}");
    }
    println!("{} titles mapped", store.mapping_count().unwrap_or(0));
    Ok(())
}

async fn run(config: Config, paths: Paths, http: HttpClient, store: Store) -> Result<()> {
    let palette = theme::resolve(config.theme.mode);
    let anilist = AniList::new(http.clone(), config.network.anilist_rate_limit);

    let mut keymap = Keymap::new();
    for problem in keymap.apply_overrides(&config.keys) {
        eprintln!("warning: keybinding: {problem}");
    }

    let engine =
        anistream_ui::image::ImageEngine::detect(true).with_cache_dir(paths.image_cache());
    tracing::info!(graphics = ?engine.graphics(), "image engine ready");
    let (registry, vpn_guard, provider_note) =
        sources::build_registry(&config, &http, &paths).await;
    tracing::info!(providers = ?registry.ids(), "provider registry ready");
    // Plugins join the chain when they have finished compiling. Nothing waits on this: the first
    // frame is worth more than a source you have not asked for yet.
    let plugins_loading = sources::spawn_plugin_load(
        registry.clone(),
        config.clone(),
        http.clone(),
        paths.clone(),
    );
    let mut app = App::with_images(config, palette, keymap, engine);
    if let Some(note) = provider_note {
        app.apply(Update::ProviderNote(note));
    }

    // Background work reports through this channel; nothing off the UI thread ever touches
    // `App` directly, which is what keeps a slow request from stalling a frame.
    let (tx, mut rx) = mpsc::unbounded_channel::<Update>();

    // Report whatever the background plugin load concluded, once. A plugin that failed to load
    // must not be silent — the Providers screen is where a missing source gets explained.
    {
        let tx = tx.clone();
        let providers_tx = tx.clone();
        let registry = registry.clone();
        tokio::spawn(async move {
            if let Ok(outcome) = plugins_loading.await {
                match outcome {
                    Some(reason) => {
                        let _ = tx.send(Update::ProviderNote(reason));
                    }
                    // Refresh the Providers screen so late arrivals actually appear on it.
                    None => {
                        let _ = providers_tx
                            .send(Update::Providers(data::provider_rows(&registry)));
                    }
                }
            }
        });
    }

    // Trackers are built whether or not they hold a token: the Accounts overlay needs something
    // to offer a sign-in for, and the outbox accumulates against the id meanwhile.
    let sync = tracking::Sync::build(&app.config, &store, &http);
    for state in sync.initial_states() {
        app.apply(Update::Sync(Box::new(state)));
    }
    tracking::spawn_loops(sync.clone(), tx.clone());

    // Keep the mapping datasets current without ever blocking startup.
    {
        let store = store.clone();
        let http = http.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let outcomes = dataset::refresh_all(&store, &http, now_epoch(), false).await;
            if let Some((name, outcome)) = outcomes.iter().find(|(_, o)| o.is_failure()) {
                let _ = tx.send(Update::Toast(Toast::alert(format!(
                    "{name} mapping refresh failed: {outcome:?}"
                ))));
            }
        });
    }

    // The download manager, when there is a torrent session to run it on. No session means no
    // downloads — which is correct rather than a limitation: every downloadable source here is a
    // torrent, and the session only exists when the VPN guard is satisfied.
    let torrent_session = vpn_guard.as_ref().map(|(_, session)| Arc::clone(session));
    if let Some(session) = torrent_session.clone() {
        downloads::spawn(store.clone(), session, app.config.clone(), tx.clone());
        downloads::publish_now(&store, &tx);
    }

    // Periodic re-verification: the kill-switch half of the guard. A tunnel that drops
    // mid-episode has to stop torrenting, not merely be noticed at the next launch.
    if let Some((guard, session)) = vpn_guard.clone() {
        let tx = tx.clone();
        let _ = tx.send(Update::Vpn {
            badge: guard.state().badge(),
            leaking: !guard.state().is_protected(),
        });

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(guard.verify_interval());
            // The first tick fires immediately; the guard was already verified at startup.
            ticker.tick().await;
            let mut was_leaking = false;

            loop {
                ticker.tick().await;
                let state = guard.verify().await;
                let leaking = !state.is_protected();

                if leaking && !was_leaking {
                    tracing::warn!(
                        reason = state.reason().unwrap_or("unknown"),
                        action = ?guard.on_leak(),
                        "vpn guard failing — halting torrent traffic"
                    );
                    // Marking the provider unavailable only stops *new* requests. Without
                    // this, librqbit would carry on downloading and seeding the current
                    // episode over an unprotected connection.
                    session.halt().await;
                } else if !leaking
                    && was_leaking
                    && let Err(e) = session.resume().await
                {
                    tracing::warn!(error = %e, "could not resume after recovery");
                }
                was_leaking = leaking;

                if tx.send(Update::Vpn { badge: state.badge(), leaking }).is_err() {
                    break;
                }
            }
        });
    }

    let mut terminal = setup_terminal().context("preparing the terminal")?;

    if let Some(task) = app.reload() {
        spawn(task, &anilist, &store, &registry, &tx);
    }

    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));

    // The live player's control channel. Replaced on each new playback, so a stale sender from
    // a finished episode can never steer the current one.
    let mut player_tx: Option<mpsc::UnboundedSender<anistream_ui::PlayerCommand>> = None;
    let mpv = anistream_player::Mpv::new(paths.runtime_dir())
        .with_binary(app.config.playback.mpv_binary.clone())
        .with_extra_args(app.config.playback.mpv_args.clone());
    if !mpv.is_available().await {
        // Said once at startup rather than at the moment you press Enter on an episode.
        app.apply(Update::Toast(Toast::alert(format!(
            "{} not found — install mpv to play anything",
            app.config.playback.mpv_binary
        ))));
    }

    let result = loop {
        terminal.draw(|frame| screens::render(frame, &app))?;
        if app.should_quit {
            break Ok(());
        }

        let rows = screens::visible_rows(terminal.size()?.into(), app.nav.section());

        // Prefetch only what is on screen plus a short lookahead. `visible_artwork` claims
        // each URL, so a re-render cannot re-request one already in flight.
        for url in app.visible_artwork(rows) {
            artwork::spawn_fetch(url, paths.image_cache(), http.clone(), tx.clone());
        }

        // Auto-next is queued by the reducer rather than returned, because an episode ending
        // arrives as an update rather than a keystroke.
        if let Some(task) = app.take_pending() {
            dispatch(
                task,
                &mut player_tx,
                &paths,
                &app.config,
                &anilist,
                &store,
                &registry,
                &http,
                &mpv,
                &sync,
                &tx,
            );
        }

        tokio::select! {
            Some(update) = rx.recv() => app.apply(update),
            _ = ticker.tick() => app.tick_toasts(),
            // The eyecatch is the only thing in this app that animates, so the fast tick only
            // exists while one is running rather than burning a frame budget all the time.
            _ = tokio::time::sleep(Duration::from_millis(anistream_ui::eyecatch::FRAME_MS)),
                if app.is_animating() =>
            {
                app.tick_animation();
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        // Text first: while a field has focus, plain characters are input,
                        // not bindings.
                        let handled_as_text = app.is_typing()
                            && match key.code {
                                KeyCode::Char(c) if key.modifiers.is_empty()
                                    || key.modifiers == crossterm::event::KeyModifiers::SHIFT =>
                                {
                                    app.type_char(c);
                                    true
                                }
                                KeyCode::Backspace => {
                                    app.backspace();
                                    true
                                }
                                _ => false,
                            };

                        let context = if app.playing.is_some() {
                            anistream_ui::keymap::Context::Playing
                        } else {
                            anistream_ui::keymap::Context::Browsing
                        };

                        if !handled_as_text
                            && let Some(action) = app.keymap.action_for(key, context)
                            && let Some(task) = app.handle(action, rows)
                        {
                            dispatch(
                                task, &mut player_tx, &paths, &app.config, &anilist, &store,
                                &registry, &http, &mpv, &sync, &tx,
                            );
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => {}
                    Some(Err(e)) => break Err(anyhow::anyhow!(e)),
                    None => break Ok(()),
                    _ => {}
                }
            }
        }
    };

    restore_terminal(&mut terminal)?;
    result
}

/// Publish a list, then follow up with the broadcast dates it needs a second request for.
///
/// Two sends rather than one: the last-aired query depends on which titles came back, so it
/// cannot be folded into the list query. The list renders as soon as it arrives and gains the
/// broadcast line a moment later, rather than the whole screen waiting on an annotation.
async fn publish_list(
    anilist: &AniList,
    tx: &mpsc::UnboundedSender<Update>,
    entries: Vec<anistream_ui::app::Entry>,
    now: i64,
) {
    // Releasing titles only. A finished show has no broadcast to report, and including it would
    // spend rows of a capped response on titles that cannot use them.
    let ids: Vec<anistream_core::ids::AnilistId> =
        entries.iter().filter(|e| e.airing_in.is_some()).map(|e| e.id).collect();
    let _ = tx.send(Update::Content(Content::Entries(entries)));
    if ids.is_empty() {
        return;
    }
    match anilist.last_aired(&ids).await {
        Ok(rows) => {
            let _ = tx.send(Update::LastAired(
                rows.into_iter()
                    .map(|r| (r.media_id, r.episode, now.saturating_sub(r.airing_at)))
                    .collect(),
            ));
        }
        // Not worth a toast: the list is already on screen and complete, and this only adds a
        // line to it. Logged so the Logs overlay can still explain a missing line.
        Err(e) => tracing::warn!(%e, "last-aired lookup failed"),
    }
}

/// Run a task off the UI thread and report the result back through the channel.
fn spawn(
    task: Task,
    anilist: &AniList,
    store: &Store,
    registry: &ProviderRegistry,
    tx: &mpsc::UnboundedSender<Update>,
) {
    let anilist = anilist.clone();
    let store = store.clone();
    let registry = registry.clone();
    let tx = tx.clone();

    tokio::spawn(async move {
        // Say so before waiting, not after. At thirty requests a minute a burst of navigation can
        // drain the budget, and the token bucket then delays every request — which presented as an
        // indefinite loading indicator and looked exactly like a hang.
        if let Some(wait) = anilist.rate_limit_wait().await
            && wait.as_millis() > 400
        {
            let _ = tx.send(Update::Status(format!(
                "waiting {}s on the AniList rate limit",
                wait.as_secs().max(1)
            )));
        }
        let update = match task {
            Task::LoadContinue => match continue_entries(&anilist, &store).await {
                Ok(entries) => {
                    publish_list(&anilist, &tx, entries, now_epoch()).await;
                    return;
                }
                Err(e) => Update::Content(Content::Failed(e)),
            },
            Task::LoadSeasonal => {
                let (season, year) = current_season();
                match anilist.seasonal(season, year, &Default::default(), 1, 40).await {
                    Ok(page) => {
                        let entries = page
                            .items
                            .iter()
                            .map(|m| data::entry_from(m, Some(&store)))
                            .collect();
                        publish_list(&anilist, &tx, entries, now_epoch()).await;
                        return;
                    }
                    Err(e) => Update::Content(Content::Failed(e.to_string())),
                }
            }
            Task::LoadCalendar => {
                let now = now_epoch();
                match calendar_timeline(&anilist, &store, now).await {
                    Ok(entries) => Update::Content(Content::Entries(entries)),
                    Err(e) => Update::Content(Content::Failed(e)),
                }
            }
            Task::Search(query) => match anilist.search(&query, 1, 30).await {
                Ok(page) => {
                    let entries =
                        page.items.iter().map(|m| data::entry_from(m, Some(&store))).collect();
                    publish_list(&anilist, &tx, entries, now_epoch()).await;
                    return;
                }
                Err(e) => Update::Content(Content::Failed(e.to_string())),
            },
            Task::LoadDetail(id) => match anilist.media(id).await {
                Ok(media) => Update::Detail(Box::new(data::entry_from(&media, Some(&store)))),
                Err(e) => Update::Toast(Toast::alert(format!("could not load title: {e}"))),
            },
            Task::CheckProviders => {
                registry.check_all(now_epoch()).await;
                Update::Providers(data::provider_rows(&registry))
            }
            // Handled in `dispatch`, which owns the live player's control channel and the
            // tracker set. Reaching here would mean a routing mistake, so it says so.
            Task::Play { .. }
            | Task::Player(_)
            | Task::SyncNow
            | Task::Connect { .. }
            | Task::Disconnect { .. }
            | Task::SetStatus { .. }
            | Task::ResolveConflict { .. }
            | Task::LoadLibrary(_)
            | Task::DownloadEpisode { .. }
            | Task::DownloadPause { .. }
            | Task::DownloadCancel { .. }
            | Task::DownloadClearCompleted
            | Task::LoadDownloads
            | Task::PlayLocal { .. }
            | Task::SaveSetting { .. } => {
                tracing::error!(?task, "task reached the generic spawner");
                return;
            }
            Task::FixMatch { id, provider_id, key } => {
                // Remembered as an override, which the ladder consults before anything else —
                // so the question is asked once, not on every visit.
                if let Err(e) = store.set_override(id, &provider_id, &key, now_epoch()) {
                    Update::Toast(Toast::alert(format!("could not save that match: {e}")))
                } else {
                    match load_episodes(&anilist, &store, &registry, id).await {
                        Ok(EpisodeLoad::Rows(rows)) => Update::Episodes(rows),
                        // The override *is* the answer, so the ladder cannot come back
                        // undecided; if it somehow does, say so rather than looping.
                        Ok(EpisodeLoad::Choose { .. }) => Update::Toast(Toast::alert(
                            "that match did not stick — the source may have changed",
                        )),
                        Err(reason) => {
                            let _ = tx.send(Update::Episodes(Vec::new()));
                            Update::Toast(Toast::alert(reason))
                        }
                    }
                }
            }
            Task::LoadEpisodes(id) => {
                match load_episodes(&anilist, &store, &registry, id).await {
                    Ok(EpisodeLoad::Rows(rows)) => Update::Episodes(rows),
                    Ok(EpisodeLoad::Choose { provider_id, candidates }) => {
                        Update::MatchChoices { id, provider_id, candidates }
                    }
                    Err(reason) => {
                        // The Episodes screen shows its own empty state, so the reason belongs
                        // in a toast rather than replacing the whole view.
                        let _ = tx.send(Update::Episodes(Vec::new()));
                        Update::Toast(Toast::alert(reason))
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
    task: Task,
    player_tx: &mut Option<mpsc::UnboundedSender<anistream_ui::PlayerCommand>>,
    paths: &Paths,
    config: &Config,
    anilist: &AniList,
    store: &Store,
    registry: &ProviderRegistry,
    http: &HttpClient,
    mpv: &anistream_player::Mpv,
    sync: &tracking::Sync,
    tx: &mpsc::UnboundedSender<Update>,
) {
    match task {
        // Sync work needs the tracker set, which `spawn` deliberately does not carry.
        Task::SyncNow => {
            let (sync, tx) = (sync.clone(), tx.clone());
            tokio::spawn(async move {
                tracking::drain_once(&sync, &tx).await;
                tracking::pull_once(&sync, &tx).await;
                let _ = tx.send(Update::Status(String::new()));
            });
        }
        Task::Connect { tracker } => {
            let (sync, tx) = (sync.clone(), tx.clone());
            tokio::spawn(tracking::connect(sync, tracker, tx));
        }
        Task::Disconnect { tracker } => tracking::disconnect(sync, &tracker, tx),
        Task::SetStatus { id, status } => {
            tracking::set_status(sync, id, status);
            // Queued, not sent — the drain owns the network. Reporting the new depth is what
            // makes the badge move immediately.
            let _ = tx.send(Update::Sync(Box::new(sync.state_after_enqueue())));
        }
        Task::ResolveConflict { id, keep_local } => {
            tracking::resolve_conflict(sync, id, keep_local);
        }
        Task::LoadLibrary(segment) => {
            let (sync, tx, store) = (sync.clone(), tx.clone(), store.clone());
            tokio::spawn(async move {
                let update = tracking::load_library(&sync, &store, segment).await;
                let _ = tx.send(update);
            });
        }
        Task::Play { id, episode } => {
            // A fresh channel per playback, and dropping the old sender is what tells a
            // previous session its controls are gone.
            let (ptx, prx) = mpsc::unbounded_channel();
            *player_tx = Some(ptx);
            // Tracker ids rather than trackers: playback only needs to know which queues to
            // append to, and passing the set would couple the player to the sync layer.
            let tracker_ids = sync.trackers.iter().map(|t| t.id().to_owned()).collect();
            spawn_playback(
                id,
                episode,
                prx,
                config,
                anilist,
                store,
                registry,
                http,
                mpv,
                tracker_ids,
                tx,
            );
        }
        Task::Player(command) => {
            if let Some(sender) = player_tx.as_ref() {
                let _ = sender.send(command);
            }
        }
        Task::LoadDownloads => downloads::publish_now(store, tx),

        Task::DownloadEpisode { id, episode } => {
            let (store, registry, anilist, tx) =
                (store.clone(), registry.clone(), anilist.clone(), tx.clone());
            let translation = config.playback.translation;
            tokio::spawn(async move {
                // Resolving needs the provider chain, so this is a task rather than inline — the
                // same walk playback does, which is the point: a download is "play it, to disk".
                let update = match downloads::enqueue(
                    &store,
                    &registry,
                    &anilist,
                    id,
                    &episode,
                    translation,
                )
                .await
                {
                    Ok(row) => Update::Toast(Toast::info(format!(
                        "queued {} ep {}",
                        row.title, row.episode
                    ))),
                    Err(reason) => Update::Toast(Toast::alert(reason)),
                };
                let _ = tx.send(update);
                let _ = tx.send(Update::Status(String::new()));
                downloads::publish_now(&store, &tx);
            });
        }

        Task::DownloadPause { id } => {
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

        Task::DownloadCancel { id } => {
            // The row goes first: the manager notices it is gone on its next poll and drops the
            // torrent. Deleting the torrent from here would race with that loop.
            let _ = store.remove_download(id);
            downloads::publish_now(store, tx);
            let _ = tx.send(Update::Toast(Toast::info("download cancelled")));
        }

        Task::DownloadClearCompleted => {
            let cleared = store.clear_completed_downloads().unwrap_or(0);
            downloads::publish_now(store, tx);
            let _ = tx.send(Update::Toast(Toast::info(format!("cleared {cleared} finished"))));
        }

        Task::PlayLocal { path } => {
            // Straight to mpv with no provider walk and no torrent session: the file is on disk,
            // and routing a local file through the resolve ladder would be theatre.
            let (mpv, tx) = (mpv.clone(), tx.clone());
            let title = std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            tokio::spawn(async move {
                let stream = anistream_core::stream::Stream::new(
                    path,
                    anistream_core::stream::StreamKind::Mp4,
                );
                let request = anistream_core::traits::PlaybackRequest {
                    title: title.clone(),
                    ..Default::default()
                };
                match mpv.play(&stream, &request).await {
                    Ok(_) => {
                        let _ = tx.send(Update::Toast(Toast::info(format!("playing {title}"))));
                    }
                    Err(e) => {
                        let _ = tx.send(Update::Toast(Toast::alert(format!("mpv: {e}"))));
                    }
                }
            });
        }

        // Written here rather than in the reducer: the reducer has already changed the in-memory
        // config, so the screen is correct whatever the disk does, and this keeps file IO out of
        // a pure function. Synchronous on purpose — one small file, and a settings change that
        // silently lost a race with the next one would be worse than a paused frame.
        Task::SaveSetting { table, key, value } => {
            match anistream_core::settings::write_key(paths, table, key, value) {
                Ok(()) => {
                    let _ = tx.send(Update::Status(format!("saved {key}")));
                }
                // The guard reaching the UI: enabling torrents with no VPN configured fails
                // validation here, so no arrow key can write a config that would leak.
                Err(e) => {
                    let _ = tx.send(Update::Toast(Toast::alert(format!("not saved: {e}"))));
                }
            }
        }
        other => spawn(other, anilist, store, registry, tx),
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
    commands: mpsc::UnboundedReceiver<anistream_ui::PlayerCommand>,
    config: &Config,
    anilist: &AniList,
    store: &Store,
    registry: &ProviderRegistry,
    http: &HttpClient,
    mpv: &anistream_player::Mpv,
    tracker_ids: Vec<String>,
    tx: &mpsc::UnboundedSender<Update>,
) {
    let (config, anilist, store, registry, http, mpv, tx) = (
        config.clone(),
        anilist.clone(),
        store.clone(),
        registry.clone(),
        http.clone(),
        mpv.clone(),
        tx.clone(),
    );

    tokio::spawn(async move {
        let playback = config.playback.clone();
        let context =
            match resolve_for_playback(&anilist, &store, &registry, id, &episode, &playback)
                .await
            {
                Ok(pair) => pair,
                Err(reason) => {
                    // The failure ladder's first rung: name the failure rather than showing an
                    // empty screen. Releasing the eyecatch is what the alert does for us.
                    let _ = tx.send(Update::Toast(Toast::alert(reason)));
                    let _ = tx.send(Update::PlaybackEnded { watched: false });
                    return;
                }
            };
        let (stream, context) = context;

        playback::play(
            stream,
            context,
            store,
            http,
            mpv,
            playback.commit_threshold,
            playback.skip_opening,
            Some(playback.subtitle_language.clone()),
            tracker_ids,
            config.presence.clone(),
            tx,
            commands,
        )
        .await;
    });
}

/// Everything needed before mpv can be spawned: a stream, and the context history needs.
async fn resolve_for_playback(
    anilist: &AniList,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
    episode: &str,
    playback: &anistream_core::config::PlaybackConfig,
) -> std::result::Result<(anistream_core::stream::Stream, playback::PlaybackContext), String> {
    if registry.is_empty() {
        return Err("no sources configured — see the Providers screen".into());
    }
    let media = anilist.media(id).await.map_err(|e| e.to_string())?;
    let now = now_epoch();

    let resolution = anistream_providers::resolve(
        store,
        registry,
        id,
        &media.match_target(),
        playback.translation,
        now,
    )
    .await;
    let key = resolution
        .key()
        .cloned()
        .ok_or_else(|| format!("could not match this title: {}", resolution.explain()))?;

    let attempt = registry.resolve(&key, episode, playback.translation, now).await;
    let summary = attempt.summary();
    let mut streams = attempt.value.ok_or(summary)?;

    // Toward the configured quality, preferring a step down over upscaling.
    streams.sort_by_key(|s| s.quality_rank(playback.quality));
    let stream = streams.into_iter().next().ok_or("no playable stream")?;

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
        title: media
            .match_target()
            .titles
            .first()
            .cloned()
            .unwrap_or_else(|| format!("anilist {}", id.get())),
        translation: playback.translation,
        resume_at,
        speed: playback.persist_speed.then_some(playback.persisted_speed).flatten(),
    };
    Ok((stream, context))
}

/// What loading a title's episodes produced.
enum EpisodeLoad {
    Rows(Vec<EpisodeRow>),
    /// The ladder found candidates but could not choose. The user decides.
    Choose { provider_id: String, candidates: Vec<anistream_ui::MatchCandidate> },
}

/// Resolve a title to a provider and list its episodes.
async fn load_episodes(
    anilist: &AniList,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
) -> std::result::Result<EpisodeLoad, String> {
    if registry.is_empty() {
        return Err("no sources configured".into());
    }
    let media = anilist.media(id).await.map_err(|e| e.to_string())?;
    let target = media.match_target();
    let now = now_epoch();

    let resolution = anistream_providers::resolve(
        store,
        registry,
        id,
        &target,
        anistream_core::media::Translation::Sub,
        now,
    )
    .await;

    let key = match &resolution {
        anistream_providers::Resolution::Resolved { key, .. } => key.clone(),
        // Candidates exist, none confidently enough to pick. That is a question, not a
        // failure: the user can see which of these is their show, and the answer is
        // remembered as an override so it is asked once.
        anistream_providers::Resolution::Ambiguous { provider_id, candidates } => {
            return Ok(EpisodeLoad::Choose {
                provider_id: provider_id.clone(),
                candidates: candidates
                    .iter()
                    .map(|candidate| anistream_ui::MatchCandidate {
                        title: candidate.hit.title.clone(),
                        key: candidate.hit.key.clone(),
                        similarity: candidate.score,
                        rejected: candidate.rejected,
                    })
                    .collect(),
            });
        }
        anistream_providers::Resolution::NotFound { .. } => {
            return Err(format!("could not match this title: {}", resolution.explain()));
        }
    };

    let attempt = registry.episodes(&key, anistream_core::media::Translation::Sub, now).await;
    let summary = attempt.summary();
    let mut episodes = attempt.value.ok_or(summary)?;

    // A torrent source knows an episode exists but not what it is called. The metadata we
    // already fetched above does, so name them before they reach the table.
    data::name_episodes(&mut episodes, &media.episode_titles(), &media.episode_thumbnails());

    Ok(EpisodeLoad::Rows(data::episode_rows(&episodes, store, id)))
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
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

/// Resolve one episode and print the URL, keeping the stream server alive.
///
/// Exists because a silent playback failure gave the user nothing to test with. Splitting
/// "anistream cannot resolve this" from "my player cannot play it" needs a URL in hand, and a
/// torrent-backed URL only serves while the session that owns it is running.
async fn stream_url(
    config: &Config,
    http: &HttpClient,
    store: &Store,
    paths: &Paths,
    id: u32,
    episode: &str,
) -> Result<()> {
    use anistream_core::ids::AnilistId;

    let id = AnilistId::new(id);
    let anilist = AniList::new(http.clone(), config.network.anilist_rate_limit);

    let started = std::time::Instant::now();
    let (registry, guard, note) = sources::build_registry(config, http, paths).await;
    println!("sources     {:?} in {:?}", registry.ids(), started.elapsed());
    if let Some(note) = &note {
        println!("note        {note}");
    }
    if registry.is_empty() {
        anyhow::bail!("no sources registered — see --doctor");
    }

    let media = anilist.media(id).await.context("looking up the title")?;
    println!("title       {}", media.title.display());

    let now = now_epoch();
    let resolution = anistream_providers::resolve(
        store,
        &registry,
        id,
        &media.match_target(),
        config.playback.translation,
        now,
    )
    .await;
    let Some(key) = resolution.key().cloned() else {
        anyhow::bail!("could not match this title: {}", resolution.explain());
    };
    println!("matched     {key:?} via {}", resolution.explain());

    let attempt = registry.resolve(&key, episode, config.playback.translation, now).await;
    let streams = match attempt.value {
        Some(streams) if !streams.is_empty() => streams,
        // An empty list is the silent case: a successful call that answers "nothing".
        Some(_) => anyhow::bail!("resolved to zero streams: {}", attempt.summary()),
        None => anyhow::bail!("every source failed: {}", attempt.summary()),
    };

    println!();
    for (i, stream) in streams.iter().enumerate() {
        println!("[{i}] {:?}  {:?}", stream.kind, stream.quality);
        println!("    {}", stream.url);
        for (name, value) in &stream.headers {
            println!("    header  {name}: {value}");
        }
    }

    println!();
    println!(
        "Try it directly — if these fail, the player or the source is at fault, not anistream:"
    );
    println!("  mpv --no-config '{}'", streams[0].url);
    println!("  curl -sI '{}'", streams[0].url);
    println!();
    if guard.is_some() {
        println!("The torrent session is serving this URL. Press Ctrl-C when you are done.");
        tokio::signal::ctrl_c().await.ok();
    }
    Ok(())
}

/// Where mpv keeps its own configuration, if it is in the usual place.
///
/// Not authoritative — mpv honours `MPV_HOME` and a portable-config directory too — so this is a
/// pointer for the human reading `--doctor`, not something the app acts on.
fn dirs_config_mpv() -> Option<std::path::PathBuf> {
    if let Ok(home) = std::env::var("MPV_HOME") {
        return Some(std::path::PathBuf::from(home).join("mpv.conf"));
    }
    std::env::var_os("HOME")
        .map(|home| std::path::PathBuf::from(home).join(".config/mpv/mpv.conf"))
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
    anilist: &AniList,
    store: &Store,
) -> std::result::Result<Vec<anistream_ui::app::Entry>, String> {
    let continuing = store.continue_list(CONTINUE_ROWS).unwrap_or_default();
    let ids: Vec<anistream_core::ids::AnilistId> =
        continuing.iter().map(|p| p.anilist_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    anilist
        .media_many(&ids)
        .await
        .map(|media| media.iter().map(|m| data::entry_from(m, Some(store))).collect())
        .map_err(|e| e.to_string())
}

/// The calendar as one timeline: what recently aired, then what is coming.
///
/// "Is there a tab for latest releases?" was a fair question with no good answer — the calendar
/// only looked forward, so the episodes you could actually watch right now appeared nowhere.
///
/// Two requests rather than one, and not for want of trying: a single ascending query over the
/// whole fortnight returns its page *oldest first*, and with several hundred broadcasts a week
/// that page is exhausted long before it reaches today. So the recent half is fetched newest-first
/// and reversed. The cost is one extra request out of thirty a minute.
async fn calendar_timeline(
    anilist: &AniList,
    store: &Store,
    now: i64,
) -> std::result::Result<Vec<anistream_ui::app::Entry>, String> {
    let (recent, upcoming) = tokio::join!(
        anilist.airing_between_sorted(now - CALENDAR_PAST, now, 1, CALENDAR_RECENT_ROWS, true),
        anilist.airing_between_sorted(
            now,
            now + CALENDAR_FUTURE,
            1,
            CALENDAR_UPCOMING_ROWS,
            false
        ),
    );

    // Either half failing alone is still a usable screen, so only report a failure when both
    // are empty — a calendar showing just the upcoming week beats an error page.
    let mut entries: Vec<anistream_ui::app::Entry> = Vec::new();
    if let Ok(page) = &recent {
        // Reversed back into chronological order, so the list reads downward through time.
        entries.extend(
            page.items.iter().rev().map(|a| data::entry_from_airing(a, now, Some(store))),
        );
    }
    let boundary = entries.len();
    if let Ok(page) = &upcoming {
        entries.extend(page.items.iter().map(|a| data::entry_from_airing(a, now, Some(store))));
    }

    if entries.is_empty() {
        let reason = recent
            .err()
            .or(upcoming.err())
            .map(|e| e.to_string())
            .unwrap_or_else(|| "nothing airing in this window".into());
        return Err(reason);
    }
    tracing::info!(recent = boundary, upcoming = entries.len() - boundary, "calendar timeline");
    Ok(entries)
}

/// Titles kept in the CONTINUE rail. Long enough to cover what you are actually mid-way through,
/// short enough that it stays a shortlist rather than a second library.
const CONTINUE_ROWS: u32 = 15;

/// How far back the calendar reaches. A week covers a full broadcast cycle, so every airing show
/// you follow has exactly one recent episode in view.
const CALENDAR_PAST: i64 = 7 * 86_400;
/// And how far forward.
const CALENDAR_FUTURE: i64 = 7 * 86_400;
/// Rows kept from each half. Enough that a week of a followed show is always in view, and few
/// enough that the two halves together stay one scrollable list rather than a firehose.
const CALENDAR_RECENT_ROWS: u32 = 30;
const CALENDAR_UPCOMING_ROWS: u32 = 40;

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
mod tests {
    use super::*;

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
}
