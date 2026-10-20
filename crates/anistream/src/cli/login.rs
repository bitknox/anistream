//! `anistream --login`, `--login-url`, `--logout` and `--token-to-file`: a tracker signed in
//! to, or out of, from the command line, and where its token is kept.

use anistream_core::config::{Config, Paths};
use anistream_engine::tracking;
use anistream_net::HttpClient;
use anistream_store::Store;
use anyhow::{Context, Result};

/// Print the URL to open in a browser to authorise a tracker.
pub fn print_login_url(config: &Config, tracker: &str) -> Result<()> {
    if tracker == "kitsu" {
        anyhow::bail!(
            "kitsu has no authorize URL — it signs in with your account credentials: \
             anistream --login --tracker kitsu"
        );
    }
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

/// Lift a tracker's drain hold after a command-line sign-in.
///
/// A rejected credential holds that tracker's queue so a dead token is not re-presented every
/// minute for months. Signing in is what ends that, and the CLI flows never reach the in-app
/// `activate` that already does it — so without this, `--login` leaves the queue waiting out
/// an hour it no longer owes. Best-effort: a hold that outlives its reason costs a delay, and
/// failing a successful sign-in over it would be worse.
fn lift_reauth_hold(tracker: &str) {
    let Ok(paths) = Paths::resolve() else { return };
    let Ok(store) = Store::open(paths.database()) else { return };
    let _ = anistream_track::sync::clear_reauth_hold(&store, tracker);
}

/// Sign in: run the configured flow, or take a code that was obtained by hand.
///
/// The code is exchanged for a token here rather than stored as-is — an authorization code is
/// single-use and short-lived, so keeping one would be storing something already spent.
pub async fn login(
    config: &Config,
    paths: &Paths,
    tracker: &str,
    given: &str,
    http: &HttpClient,
) -> Result<()> {
    if tracker == "mal" {
        return login_mal(config, paths, http).await;
    }
    if tracker == "kitsu" {
        return login_kitsu(config, paths, http).await;
    }
    // Simkl and Trakt sign in with a device code, which is a *better* fit for a CLI than the
    // loopback flow: nothing to register, and it works over SSH where opening a browser on the
    // wrong machine does not.
    if let Some(endpoints) = device_endpoints_for(tracker) {
        return login_device(config, paths, tracker, endpoints, http).await;
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

    let store = tracking::token_store(config, paths);
    lift_reauth_hold(tracker);
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
    paths: &Paths,
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

    let tokens = tracking::token_store(config, paths);
    lift_reauth_hold(tracker);
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

/// Sign in to MyAnimeList.
///
/// A separate function from the AniList flow rather than a branch inside it, because almost nothing
/// is shared: PKCE instead of a secret, a form-encoded exchange instead of JSON, and a token pair
/// with an expiry instead of a bare year-long token. Forcing them into one function would mean a
/// parameter for every difference.
async fn login_mal(config: &Config, paths: &Paths, http: &HttpClient) -> Result<()> {
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

    let store = tracking::token_store(config, paths);
    lift_reauth_hold("mal");
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

/// Sign in to Kitsu with a username and password.
///
/// The one tracker with no app to register: Kitsu's token endpoint accepts the password
/// grant bare. The password's only journey is stdin → the token exchange — it is never
/// written anywhere, and what lands in the keychain is the usual access/refresh pair.
async fn login_kitsu(config: &Config, paths: &Paths, http: &HttpClient) -> Result<()> {
    println!("Kitsu signs in with your account credentials — no app registration exists.");
    println!("The password is exchanged for a token and dropped; only the token is stored.");
    println!();
    let username = read_line("email or username: ")?;
    let username = username.trim();
    if username.is_empty() {
        anyhow::bail!("nothing entered");
    }
    let password = read_secret("password: ")?;
    if password.is_empty() {
        anyhow::bail!("nothing entered");
    }

    println!("Exchanging the credentials for a token…");
    let pair = anistream_track::kitsu::login(
        http.plain(),
        username,
        &password,
        anistream_store::now(),
    )
    .await?;

    let store = tracking::token_store(config, paths);
    lift_reauth_hold("kitsu");
    let storage =
        store.set_pair("kitsu", &pair.access, pair.refresh.as_deref(), pair.expires_at)?;
    println!("● signed in to Kitsu — token stored in the {}", storage.describe());
    if let Some(expires_at) = pair.expires_at {
        let days = (expires_at - anistream_store::now()) / 86_400;
        println!("  valid for about {days} days, and renewed automatically before it lapses");
    }
    println!();
    println!("Check it with: anistream --sync");
    Ok(())
}

/// Read one line from stdin without echoing it, where the platform allows.
///
/// Unix gets `stty -echo` around the read; elsewhere the input echoes, and saying so
/// beats pretending. A password should never appear on screen silently.
fn read_secret(prompt: &str) -> Result<String> {
    #[cfg(unix)]
    let echo_off = std::process::Command::new("stty")
        .arg("-echo")
        .stdin(std::process::Stdio::inherit())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    #[cfg(not(unix))]
    let echo_off = false;

    if !echo_off {
        eprintln!("(the terminal could not hide input, so it will echo)");
    }
    let line = read_line(prompt);
    #[cfg(unix)]
    if echo_off {
        let _ = std::process::Command::new("stty")
            .arg("echo")
            .stdin(std::process::Stdio::inherit())
            .status();
        // The Enter that ended the hidden line never echoed either.
        eprintln!();
    }
    Ok(line?.trim().to_owned())
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
pub fn migrate_token(config: &Config, paths: &Paths, tracker: &str) -> Result<()> {
    // The desktop's own store, whichever backend is configured: the move reads the keychain
    // regardless, and writes where the file backend reads.
    let backend = anistream_track::secret::Backend::from_env_or(&config.trackers.token_storage);
    let store = anistream_track::secret::LocalSecrets::new(tracking::token_dir(paths), backend);
    match store.migrate_to_file(tracker) {
        Ok(_) => {
            println!("● moved the {tracker} token to {}", tracking::token_dir(paths).display());
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

pub fn forget_token(config: &Config, paths: &Paths, tracker: &str) -> Result<()> {
    tracking::token_store(config, paths).clear(tracker)?;
    println!("forgot the {tracker} token");
    let _ = config;
    Ok(())
}
