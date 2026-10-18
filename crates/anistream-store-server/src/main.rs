//! `anistream-store`: the plugin store server on its own, for the container image and anyone who
//! runs a store without the app.

use anistream_store_server::{StampArgs, StoreCommand};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "anistream-store", version, about = "Run an anistream plugin store")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    #[command(flatten)]
    Store(StoreCommand),
    /// Write a plugin's manifest into it, so a store that cannot run plugins can read it.
    Stamp(StampArgs),
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    // A server's log is its record of who joined and what was published; a one-off command has
    // nothing to say beyond its answer.
    let level = if matches!(cli.command, Command::Store(StoreCommand::Serve(_))) {
        "info"
    } else {
        "warn"
    };
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("ANISTREAM_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .try_init();

    let outcome = match cli.command {
        Command::Store(command) => anistream_store_server::run(command, None).await,
        Command::Stamp(args) => anistream_store_server::stamp(&args, None).await.map(|info| {
            println!("stamped {} {}", info.id, info.version);
        }),
    };
    match outcome {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
