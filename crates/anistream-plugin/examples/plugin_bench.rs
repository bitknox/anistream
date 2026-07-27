//! What a plugin call actually costs.
//!
//! Run with `cargo run -p anistream-plugin --example plugin_bench --release`.
//!
//! Worth measuring rather than assuming, because the two reference plugins are wildly different
//! shapes: a Rust parser is tens of kilobytes, a JavaScript one embeds a whole engine. The host
//! instantiates a fresh store per call — deliberately, so calls cannot leak state into one another
//! — which makes instantiation cost part of the per-call cost.

use std::{path::PathBuf, time::Instant};

use anistream_plugin::{Limits, PluginHost};

#[tokio::main]
async fn main() {
    let plugins = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let candidates = [
        (
            "example-rust",
            plugins.join(
                "example-rust/target/wasm32-wasip2/release/anistream_example_plugin.wasm",
            ),
        ),
        ("example-ts", plugins.join("example-ts/anistream-example-plugin-ts.wasm")),
    ];

    let host =
        PluginHost::new(Limits { memory_bytes: 256 * 1024 * 1024, ..Limits::default() }, None)
            .expect("host");

    println!(
        "{:<14} {:>10} {:>12} {:>12} {:>12}",
        "plugin", "size", "compile", "first call", "per call"
    );
    println!("{}", "─".repeat(64));

    for (name, path) in candidates {
        if !path.exists() {
            println!("{name:<14} {:>10}", "not built");
            continue;
        }
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        // Compilation happens once, at load.
        let started = Instant::now();
        let plugin = match host.load(&path).await {
            Ok(plugin) => plugin,
            Err(e) => {
                println!("{name:<14} ✕ {e}");
                continue;
            }
        };
        let compile = started.elapsed();

        // The first call after loading, which includes instantiating the component.
        let started = Instant::now();
        let _ = plugin.list_episodes("example:x", "sub").await;
        let first = started.elapsed();

        // Steady state. Each call is a fresh store, so this is not a warm-cache figure — it is
        // what every call costs.
        const ROUNDS: u32 = 20;
        let started = Instant::now();
        for _ in 0..ROUNDS {
            let _ = plugin.list_episodes("example:x", "sub").await;
        }
        let per_call = started.elapsed() / ROUNDS;

        println!(
            "{name:<14} {:>9.1}M {compile:>12.2?} {first:>12.2?} {per_call:>12.2?}",
            size as f64 / 1_048_576.0
        );
    }

    println!();
    println!(
        "A fresh store per call is deliberate: it is what stops one search leaking state into"
    );
    println!("the next. Instantiation is therefore part of every call, not a one-off.");
}
