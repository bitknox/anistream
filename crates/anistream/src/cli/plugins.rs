//! `anistream --plugins`: the plugins installed here, and what each one may reach. And the
//! arguments of `anistream plugin …`, which runs beside the store's commands in `store`.

use std::time::Duration;

use anistream_core::config::{Config, Paths};
use anyhow::Result;

#[derive(clap::Subcommand, Debug)]
pub enum PluginCommand {
    /// Write a plugin's manifest into it, so a store that cannot run plugins can read it.
    Stamp(anistream_store_server::StampArgs),
}

/// List installed plugins and what each may reach.
///
/// Loaded with **no HTTP client**, so inspecting a plugin cannot cause it to make a request. A
/// plugin describing itself is the one call that runs with no capabilities at all.
pub async fn list_plugins(config: &Config, paths: &Paths) -> Result<()> {
    let dir = paths.plugin_dir();
    println!("plugin directory  {}", dir.display());

    let limits = anistream_plugin::Limits {
        memory_bytes: config.providers.plugins.memory_mb.max(1).saturating_mul(1024 * 1024),
        deadline: Duration::from_secs(config.providers.plugins.deadline_secs.max(1)),
        ..Default::default()
    };
    println!(
        // MiB, as the config documents it. The number is the file's own, so it keeps its unit:
        // "64 MB" was a decimal name on what is 67 MB.
        "limits            {} MiB memory · {} s per call",
        config.providers.plugins.memory_mb,
        limits.deadline.as_secs()
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
