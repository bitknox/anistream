//! Point the mender at a live HLS stream and report whether mpv decodes it.
//!
//! A live probe, like the others: it needs the real internet and a real mpv, so it is not part
//! of `cargo test`. It exists because the unit tests can only prove the sniffing logic against
//! bytes this repository wrote — whether a source's actual disguise is seen through is a
//! question only the source can answer.
//!
//! ```sh
//! cargo run -p anistream --example mend_probe -- <master.m3u8 url> [referer]
//! ```
//!
//! `--vo=null` means nothing is displayed; the line to look for is mpv's own
//! `VO: [null] <width>x<height>`.

use anistream_core::config::NetworkConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("ANISTREAM_LOG").unwrap_or_else(|_| "debug".into()))
        .init();

    let Some(url) = std::env::args().nth(1) else {
        eprintln!("usage: mend_probe <master.m3u8 url> [referer]");
        std::process::exit(2);
    };
    // Referer-locked CDNs answer a bare request with an interstitial or a 403, so the probe
    // takes the same header the provider would have attached.
    let headers = match std::env::args().nth(2) {
        Some(referer) => vec![("referer".to_string(), referer)],
        None => Vec::new(),
    };

    let http = anistream_net::HttpClient::new(&NetworkConfig::default())?;
    let server = anistream::mend::serve(http, &url, headers, "probe").await?;
    println!("mended url  {}", server.url());

    let output = tokio::process::Command::new("mpv")
        .args([
            "--no-config",
            "--vo=null",
            "--ao=null",
            "--length=6",
            "--msg-level=all=info",
            server.url(),
        ])
        .output()
        .await?;

    let reported = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let interesting: Vec<&str> = reported
        .lines()
        .filter(|line| {
            ["VO:", "AO:", "Video", "error", "Error", "failed", "Failed"]
                .iter()
                .any(|needle| line.contains(needle))
        })
        .collect();

    for line in &interesting {
        println!("mpv         {line}");
    }
    if !interesting.iter().any(|line| line.contains("VO:")) {
        eprintln!("\nno video output — the stream was not mended into something playable");
        std::process::exit(1);
    }
    Ok(())
}
