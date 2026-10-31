//! `anistream store …` and `anistream plugin …`: a plugin store run or looked after from the
//! command line, and a plugin stamped for one.

use std::io;

use anistream_core::config::Paths;
use anyhow::Result;

use super::plugins::PluginCommand;
use crate::Subcommand;

/// `anistream store …` and `anistream plugin …`.
pub async fn store_command(command: Subcommand, paths: &Paths) -> Result<()> {
    use tracing_subscriber::{EnvFilter, fmt};
    // A server's log is its record of who joined and what was published. A one-off command has
    // nothing to say beyond its answer.
    let serving =
        matches!(command, Subcommand::Store(anistream_store_server::StoreCommand::Serve(_)));
    let level = if serving { "info" } else { "warn" };
    let _ = fmt()
        .with_writer(io::stderr)
        .with_env_filter(
            EnvFilter::try_from_env("ANISTREAM_LOG").unwrap_or_else(|_| EnvFilter::new(level)),
        )
        .try_init();
    let cache = paths.cache_dir.join("wasm");
    let outcome = match command {
        Subcommand::Store(command) => anistream_store_server::run(command, Some(&cache)).await,
        // Read in `main`: it opens the terminal interface and never gets here.
        Subcommand::Party { .. } => Ok(()),
        Subcommand::Plugin(PluginCommand::Stamp(args)) => {
            anistream_store_server::stamp(&args, Some(&cache)).await.map(|info| {
                println!("stamped {} {}", info.id, info.version);
                println!(
                    "  may contact   {}",
                    if info.allowed_hosts.is_empty() {
                        "nothing".into()
                    } else {
                        info.allowed_hosts.join(", ")
                    }
                );
            })
        }
    };
    outcome.map_err(|e| anyhow::anyhow!(e))
}
