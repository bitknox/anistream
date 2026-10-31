//! The plugin store server, as commands: `serve`, `invite`, `admin`, `publish`, and `stamp` for
//! plugins. Shared by the `anistream-store` binary the container image runs and by the app's
//! `anistream store …`, so both behave the same.
//!
//! Every setting of `serve` can also come from the environment (`ANISTREAM_STORE_*`), and on the
//! platforms that say so — Render, Railway, Fly, Heroku — the store finds its own address and
//! trusts the platform's proxy. A deploy template is then a list of variables, not a command line.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anistream_plugin_store::{
    Inspector, PluginInfo, crypto, manifest,
    protocol::{Role, ServerInfo},
    server::{self, AnyStore, NewInvite, Storage, TlsFiles},
};

/// Reads a plugin's manifest with the real WebAssembly host, lent nothing.
///
/// No HTTP client and no settings: describing itself is the one call a plugin makes with no
/// capabilities at all, so inspecting an upload cannot make it do anything.
pub struct WasmInspector {
    host: anistream_plugin::PluginHost,
    /// Where plugins are written for the host to read. Private to this process.
    scratch: tempfile::TempDir,
}

impl WasmInspector {
    /// A host with `limits`, caching compiled plugins in `cache` when given.
    pub fn new(limits: anistream_plugin::Limits, cache: Option<&Path>) -> Result<Self, String> {
        let host = anistream_plugin::PluginHost::with_cache(limits, None, cache)
            .map_err(|e| e.to_string())?;
        Self::with_host(host)
    }

    /// A host for a store's server: each upload compiled once, plainly, in as little memory as
    /// that takes. See [`anistream_plugin::PluginHost::for_inspection`].
    pub fn for_store(limits: anistream_plugin::Limits) -> Result<Self, String> {
        let host =
            anistream_plugin::PluginHost::for_inspection(limits).map_err(|e| e.to_string())?;
        Self::with_host(host)
    }

    fn with_host(host: anistream_plugin::PluginHost) -> Result<Self, String> {
        let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
        Ok(Self { host, scratch })
    }
}

#[async_trait::async_trait]
impl Inspector for WasmInspector {
    async fn inspect(&self, wasm: &[u8]) -> Result<PluginInfo, String> {
        // Named by content, so two inspections at once never read each other's bytes. The name is
        // not the plugin's id, which also keeps the host from granting it anyone's settings.
        let path = self.scratch.path().join(format!("{}.wasm", crypto::sha256_hex(wasm)));
        tokio::fs::write(&path, wasm).await.map_err(|e| e.to_string())?;
        let loaded = self.host.load(&path).await;
        let _ = tokio::fs::remove_file(&path).await;
        // Said without the scratch path, which means nothing to whoever uploaded the plugin and
        // would tell a browser where this server keeps its files.
        let plugin = loaded.map_err(|e| match e {
            anistream_plugin::PluginError::Load { message, .. } => message,
            anistream_plugin::PluginError::Trap { message, .. } => {
                format!("the plugin failed while describing itself: {message}")
            }
            anistream_plugin::PluginError::OutdatedAbi { .. } => {
                "the plugin was built for an older plugin interface and needs updating"
                    .to_owned()
            }
            anistream_plugin::PluginError::Deadline { .. } => {
                "the plugin took too long to describe itself".to_owned()
            }
        })?;
        let manifest = plugin.manifest();
        Ok(PluginInfo {
            id: manifest.id.trim().to_owned(),
            name: manifest.display_name.clone(),
            version: manifest.version.trim().to_owned(),
            allowed_hosts: manifest.allowed_hosts.clone(),
            translations: manifest.translation_types.clone(),
        })
    }
}

/// Where a store lives.
#[derive(clap::Args, Debug, Clone)]
pub struct StorageArgs {
    /// The store's directory: its database and plugin files.
    #[arg(env = "ANISTREAM_STORE_DIR")]
    pub dir: Option<PathBuf>,
    /// A Postgres database to keep everything in instead, for platforms with no disk.
    /// `DATABASE_URL` is used when neither this nor a directory is given.
    #[arg(long, env = "ANISTREAM_STORE_DATABASE_URL", conflicts_with = "dir")]
    pub database: Option<String>,
}

impl StorageArgs {
    pub fn storage(&self) -> Result<Storage, String> {
        self.storage_from(|name| std::env::var(name).ok())
    }

    fn storage_from(&self, env: impl Fn(&str) -> Option<String>) -> Result<Storage, String> {
        if let Some(url) = self.database.as_ref().filter(|u| !u.is_empty()) {
            return Ok(Storage::Postgres(url.clone()));
        }
        if let Some(dir) = &self.dir {
            return Ok(Storage::Dir(dir.clone()));
        }
        let env = |name| env(name).filter(|value: &String| !value.is_empty());
        // Heroku, Render and most hosted Postgres name it this; nothing else here does.
        if let Some(url) = env("DATABASE_URL") {
            return Ok(Storage::Postgres(url));
        }
        // The container image's volume, last: a platform that attaches a database to the same
        // image should get the database, not an empty directory that goes away with the
        // container.
        if let Some(dir) = env("ANISTREAM_STORE_DEFAULT_DIR") {
            return Ok(Storage::Dir(dir.into()));
        }
        Err("say where the store lives: a directory, or --database postgres://… \
             (ANISTREAM_STORE_DIR or ANISTREAM_STORE_DATABASE_URL)"
            .into())
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct ServeArgs {
    #[command(flatten)]
    pub storage: StorageArgs,
    /// The address people reach the store at, such as https://plugins.example. Invite links are
    /// built from it. Found on its own on Render, Railway, Fly and Heroku.
    #[arg(long, env = "ANISTREAM_STORE_URL")]
    pub url: Option<String>,
    /// The store's name, as members see it.
    #[arg(long, env = "ANISTREAM_STORE_NAME")]
    pub name: Option<String>,
    /// Where to listen. Defaults to 127.0.0.1:7878, or to every interface on `PORT` when a
    /// platform sets one.
    #[arg(long, env = "ANISTREAM_STORE_LISTEN")]
    pub listen: Option<SocketAddr>,
    /// A PEM certificate chain, to serve HTTPS here rather than behind a proxy.
    #[arg(long, env = "ANISTREAM_STORE_TLS_CERT", requires = "tls_key")]
    pub tls_cert: Option<PathBuf>,
    /// The PEM private key for --tls-cert.
    #[arg(long, env = "ANISTREAM_STORE_TLS_KEY", requires = "tls_cert")]
    pub tls_key: Option<PathBuf>,
    /// Behind a platform's proxy: take the client's address and the store's own from its
    /// `X-Forwarded-*` headers. Only safe when that proxy is the only way in.
    #[arg(long, env = "ANISTREAM_STORE_TRUST_PROXY")]
    pub trust_proxy: bool,
    /// A code that claims the store, while nobody owns it, from its admin page. For platforms
    /// where the log is not somewhere to read an invite from.
    #[arg(long, env = "ANISTREAM_STORE_SETUP_CODE", hide_env_values = true)]
    pub setup_code: Option<String>,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum StoreCommand {
    /// Serve a store, creating it on the first run.
    ///
    /// Prints an owner invite until someone uses it. Everything after that — inviting people,
    /// removing them, publishing — is done from the store's page in the app, or its admin page.
    Serve(ServeArgs),
    /// Print a one-time invite link.
    Invite {
        #[command(flatten)]
        storage: StorageArgs,
        /// Who it is for, as the members list will show them.
        #[arg(long)]
        name: String,
        /// Make them an owner, who can invite and remove people and publish.
        #[arg(long)]
        owner: bool,
    },
    /// Print a one-time link that signs a browser in to the store's admin page.
    Admin {
        #[command(flatten)]
        storage: StorageArgs,
        /// Which owner to sign in as. The first to join, when not given.
        #[arg(long = "as", value_name = "NAME")]
        owner: Option<String>,
    },
    /// Publish a plugin, or a new version of one.
    Publish {
        #[command(flatten)]
        storage: StorageArgs,
        /// The plugin's `.wasm` file.
        #[arg(long)]
        plugin: PathBuf,
    },
}

/// Write a plugin's manifest into it, so a store that cannot run plugins can read it.
#[derive(clap::Args, Debug, Clone)]
pub struct StampArgs {
    /// The plugin's `.wasm` file.
    pub plugin: PathBuf,
    /// Where to write the stamped plugin. The same file, when not given.
    #[arg(long, short)]
    pub out: Option<PathBuf>,
}

/// Seconds since the epoch.
fn now() -> i64 {
    server::now()
}

/// Run a store command. `cache` is where compiled plugins are kept between runs.
pub async fn run(command: StoreCommand, cache: Option<&Path>) -> Result<(), String> {
    match command {
        StoreCommand::Serve(args) => serve(args, cache).await,
        StoreCommand::Invite { storage, name, owner } => invite(&storage, &name, owner).await,
        StoreCommand::Admin { storage, owner } => admin(&storage, owner.as_deref()).await,
        StoreCommand::Publish { storage, plugin } => publish(&storage, &plugin, cache).await,
    }
}

/// The store's address as the platform it runs on reports it, and whether that platform puts a
/// proxy in front of everything.
fn platform() -> Option<String> {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    var("RENDER_EXTERNAL_URL")
        .or_else(|| var("RAILWAY_PUBLIC_DOMAIN").map(|d| format!("https://{d}")))
        .or_else(|| var("FLY_APP_NAME").map(|app| format!("https://{app}.fly.dev")))
        .or_else(|| var("HEROKU_APP_DEFAULT_DOMAIN_NAME").map(|d| format!("https://{d}")))
}

/// Where the store runs, for the update steps its admin page shows: named by
/// `ANISTREAM_STORE_HOST`, or recognised from what the platform sets. `in_app` is a store served
/// by `anistream store serve`.
fn host(in_app: bool) -> String {
    let set = |name: &str| std::env::var(name).is_ok_and(|v| !v.trim().is_empty());
    if let Ok(named) = std::env::var("ANISTREAM_STORE_HOST")
        && !named.trim().is_empty()
    {
        return named.trim().to_ascii_lowercase();
    }
    let found = if set("FLY_APP_NAME") {
        "fly"
    } else if set("RENDER") {
        "render"
    } else if ["RAILWAY_ENVIRONMENT", "RAILWAY_ENVIRONMENT_NAME", "RAILWAY_PROJECT_ID"]
        .iter()
        .any(|name| set(name))
    {
        "railway"
    } else if set("DYNO") {
        "heroku"
    } else if in_app {
        "anistream"
    } else if set("ANISTREAM_STORE_DEFAULT_DIR") {
        // The container image sets this, and the platforms above run that image too.
        "docker"
    } else {
        "binary"
    };
    found.to_owned()
}

/// Whether this is a platform whose proxy is the only way in.
fn behind_platform_proxy() -> bool {
    // Railway has renamed its variables before; any of these says the store is there.
    [
        "RENDER",
        "RAILWAY_ENVIRONMENT",
        "RAILWAY_ENVIRONMENT_NAME",
        "RAILWAY_PROJECT_ID",
        "FLY_APP_NAME",
        "DYNO",
    ]
    .iter()
    .any(|name| std::env::var(name).is_ok())
}

async fn open_existing(storage: &StorageArgs) -> Result<(AnyStore, String), String> {
    let storage = storage.storage()?;
    if !storage.exists() {
        return Err(format!("{} is not a store — `serve` creates one", storage.describe()));
    }
    let store = server::open(&storage, None, None).await?;
    let base = store.url().await.ok_or_else(|| {
        "this store has no address recorded — serve it once with --url, or use its admin page"
            .to_owned()
    })?;
    Ok((store, base))
}

/// Run from the app, the store shares the app's compilation cache, where a plugin compiled once
/// stays compiled. On its own, as in the container image, it may be on a half-gigabyte machine,
/// and compiles each upload plainly instead.
fn inspector(cache: Option<&Path>) -> Result<WasmInspector, String> {
    let limits = anistream_plugin::Limits::default();
    match cache {
        Some(cache) => WasmInspector::new(limits, Some(cache)),
        None => WasmInspector::for_store(limits),
    }
}

/// Serve until interrupted.
pub async fn serve(args: ServeArgs, cache: Option<&Path>) -> Result<(), String> {
    let storage = args.storage.storage()?;
    let first_run = !storage.exists();
    let trust_proxy = args.trust_proxy || behind_platform_proxy();
    // A platform's form can save a field left blank as an empty variable, which means unset.
    let given = |value: &Option<String>| value.clone().filter(|v| !v.trim().is_empty());
    let url = given(&args.url).or_else(platform);
    let setup_code = given(&args.setup_code);
    let store = server::open(&storage, url.as_deref(), args.name.as_deref()).await?;
    let address = store.url().await;
    if address.is_none() && !trust_proxy {
        return Err("a new store needs --url, the address people will reach it at — or \
                    --trust-proxy, behind a platform's proxy that says where requests were sent"
            .into());
    }
    let acceptor = args
        .tls_cert
        .clone()
        .zip(args.tls_key.clone())
        .map(|(cert, key)| server::tls_acceptor(&TlsFiles { cert, key }))
        .transpose()?;
    let listen = args.listen.unwrap_or_else(|| {
        match std::env::var("PORT").ok().and_then(|p| p.parse::<u16>().ok()) {
            Some(port) => SocketAddr::from(([0, 0, 0, 0], port)),
            None => SocketAddr::from(([127, 0, 0, 1], 7878)),
        }
    });
    let (listener, local) =
        server::bind(listen).await.map_err(|e| format!("could not listen on {listen}: {e}"))?;

    let shown = address.clone().unwrap_or_else(|| "taken from each request".into());
    println!("{}", if first_run { "created a new plugin store" } else { "plugin store" });
    println!("  name        {}", store.name().await);
    println!("  storage     {}", storage.describe());
    println!("  address     {shown}");
    println!("  key         {}", store.public_key().fingerprint());
    let host = host(cache.is_some());
    println!(
        "  version     {}, schema {}, on {host}",
        env!("CARGO_PKG_VERSION"),
        anistream_plugin_store::schema::VERSION
    );
    println!(
        "  listening   {local}   {}",
        if acceptor.is_some() { "(HTTPS)" } else { "(plain HTTP)" }
    );
    println!();

    // Said up front, because the failure would otherwise show up on someone else's machine as an
    // invite that "does not work".
    if let Some(address) = &address
        && matches!(
            anistream_plugin_store::invite::check_base(address),
            Err(anistream_plugin_store::InviteError::Insecure(_))
        )
    {
        println!(
            "warning: {address} is plain HTTP on another machine's network, and anistream"
        );
        println!("         only joins stores over HTTPS. Give --url the https:// address of a");
        println!("         proxy in front of this server, or pass --tls-cert and --tls-key.");
        println!();
    }
    if acceptor.is_none() && !local.ip().is_loopback() && !trust_proxy {
        println!(
            "warning: serving plain HTTP on {local}. Members' tokens would cross the network"
        );
        println!(
            "         readable. Put HTTPS in front of it, or pass --tls-cert and --tls-key."
        );
        println!();
    }

    if store.has_owner().await.map_err(|e| e.to_string())? {
        println!("Manage it from the store's page in anistream, or in a browser: choose");
        println!("\"open the admin page\" there, or run `anistream-store admin`.");
    } else {
        let mut invited = false;
        if let Some(address) = &address
            && let Some(invite) =
                store.fresh_owner_invite(address, now()).await.map_err(|e| e.to_string())?
        {
            println!(
                "Nobody owns this store yet. Join it from anistream with this link to become"
            );
            println!("its owner (Providers, Enter, join a store):");
            println!();
            println!("  {}", invite.link());
            println!();
            println!(
                "It works once, for seven days. Each start prints a new one until it is used."
            );
            invited = true;
        }
        let page = address
            .as_deref()
            .map_or("/admin/ at its address".into(), |a| format!("{a}/admin/"));
        match (setup_code.is_some(), invited) {
            (true, true) => {
                println!(
                    "Or open {page} and claim it with the setup code it was deployed with."
                )
            }
            (true, false) => {
                println!("Nobody owns this store yet. Open {page} and claim it with the setup");
                println!("code it was deployed with.");
            }
            (false, false) => {
                println!(
                    "Nobody owns this store yet, and with no address of its own it cannot"
                );
                println!(
                    "build an owner invite. Give it --url, or a setup code to claim it with"
                );
                println!("from its admin page (ANISTREAM_STORE_SETUP_CODE).");
            }
            (false, true) => {}
        }
    }
    println!();

    let server = Arc::new(
        server::Server::new(
            store,
            address,
            Some(Box::new(inspector(cache)?)),
            setup_code,
            trust_proxy,
        )
        .describing(ServerInfo::new(env!("CARGO_PKG_VERSION"), Some(&host))),
    );
    server::serve(listener, server, acceptor, shutdown()).await.map_err(|e| e.to_string())?;
    println!("stopped");
    Ok(())
}

/// Ctrl-C, or the SIGTERM a platform sends when it stops a container.
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            () = async {
                match term.as_mut() {
                    Some(term) => { term.recv().await; }
                    None => std::future::pending::<()>().await,
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Print an invite link.
pub async fn invite(storage: &StorageArgs, name: &str, owner: bool) -> Result<(), String> {
    let (store, base) = open_existing(storage).await?;
    let role = if owner { Role::Owner } else { Role::Member };
    let now = now();
    let new = NewInvite::member(name, role, None, AnyStore::default_expiry(now));
    let (_, invite) = store.create_invite(new, &base, now).await.map_err(|e| e.to_string())?;
    println!("{}", invite.link());
    eprintln!(
        "works once, for seven days — for {name}{}",
        if owner { ", as an owner" } else { "" }
    );
    Ok(())
}

/// Print a sign-in link for the admin page.
pub async fn admin(storage: &StorageArgs, owner: Option<&str>) -> Result<(), String> {
    let (store, base) = open_existing(storage).await?;
    let found = store.owner_named(owner).await.map_err(|e| e.to_string())?;
    let owner = found.ok_or_else(|| match owner {
        Some(name) => format!("no owner named {name} — the members list shows who owns it"),
        None => {
            "nobody owns this store yet — join it with the owner invite `serve` printed".into()
        }
    })?;
    let (link, _) =
        store.create_admin_link(&owner.id, &base, now()).await.map_err(|e| e.to_string())?;
    println!("{link}");
    eprintln!("signs in as {} — works once, for ten minutes", owner.name);
    Ok(())
}

/// Publish a plugin file.
pub async fn publish(
    storage: &StorageArgs,
    plugin: &Path,
    cache: Option<&Path>,
) -> Result<(), String> {
    let storage = storage.storage()?;
    if !storage.exists() {
        return Err(format!("{} is not a store — `serve` creates one", storage.describe()));
    }
    let store = server::open(&storage, None, None).await?;
    let bytes =
        tokio::fs::read(plugin).await.map_err(|e| format!("{}: {e}", plugin.display()))?;
    let inspector = inspector(cache)?;
    let entry =
        store.publish(&bytes, Some(&inspector), now()).await.map_err(|e| e.to_string())?;
    println!("published {} {}", entry.id, entry.version);
    println!(
        "  may contact   {}",
        if entry.allowed_hosts.is_empty() {
            "nothing".into()
        } else {
            entry.allowed_hosts.join(", ")
        }
    );
    Ok(())
}

/// Ask a plugin what it is and write the answer into it.
pub async fn stamp(args: &StampArgs, cache: Option<&Path>) -> Result<PluginInfo, String> {
    let bytes = tokio::fs::read(&args.plugin)
        .await
        .map_err(|e| format!("{}: {e}", args.plugin.display()))?;
    let info = inspector(cache)?.inspect(&bytes).await?;
    let stamped = manifest::stamp(&bytes, &info).map_err(|e| e.to_string())?;
    let out = args.out.as_deref().unwrap_or(&args.plugin);
    let partial = out.with_extension("wasm.part");
    tokio::fs::write(&partial, stamped)
        .await
        .map_err(|e| format!("{}: {e}", partial.display()))?;
    tokio::fs::rename(&partial, out).await.map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_images_directory_gives_way_to_a_platforms_database() {
        let env = |vars: &'static [(&'static str, &'static str)]| {
            move |name: &str| vars.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string())
        };
        let none = StorageArgs { dir: None, database: None };
        let image = env(&[("ANISTREAM_STORE_DEFAULT_DIR", "/data")]);
        assert!(
            matches!(none.storage_from(image), Ok(Storage::Dir(d)) if d == Path::new("/data"))
        );

        // Heroku attaches Postgres to the same image.
        let heroku = env(&[
            ("ANISTREAM_STORE_DEFAULT_DIR", "/data"),
            ("DATABASE_URL", "postgres://db/s"),
        ]);
        assert!(matches!(none.storage_from(heroku), Ok(Storage::Postgres(_))));

        // What someone says outright beats both.
        let dir = StorageArgs { dir: Some("/srv/store".into()), database: None };
        assert!(
            matches!(dir.storage_from(heroku), Ok(Storage::Dir(d)) if d == Path::new("/srv/store"))
        );

        // Set but empty is not set.
        assert!(none.storage_from(env(&[("DATABASE_URL", "")])).is_err());
    }
}
