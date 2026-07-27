//! Verify that a leaking guard actually stops an in-flight download.
//!
//! This is the one privacy-critical path a unit test cannot reach, and the area where a real
//! bug already hid: `on_leak` was configured, logged, and did nothing. Marking the provider
//! unavailable stops *new* requests; it does not stop librqbit continuing to download and
//! seed the current episode.
//!
//! So this measures the thing that matters — bytes — rather than a flag. It downloads Big
//! Buck Bunny (Blender Foundation, CC-BY 3.0), confirms progress is being made, drops the
//! tunnel, and asserts that progress *stops*.
//!
//! ```text
//! cargo run -p anistream-providers --example halt_probe
//! ```
//!
//! Reconnects the tunnel afterwards. With Mullvad lockdown mode on, the machine has no
//! network at all while disconnected, which is the point.

use std::time::Duration;

use anistream_core::config::{Config, Paths};
use anistream_providers::{VpnGuard, torrent::TorrentSession};

/// Big Buck Bunny © Blender Foundation, CC-BY 3.0 — <https://peach.blender.org>.
const LEGAL_MAGNET: &str = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c\
&dn=Big+Buck+Bunny\
&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce\
&tr=udp%3A%2F%2Fexodus.desync.com%3A6969%2Fannounce\
&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce\
&tr=udp%3A%2F%2Ftracker.torrent.eu.org%3A451%2Fannounce";

async fn mullvad(args: &[&str]) {
    let _ = tokio::process::Command::new("mullvad").args(args).output().await;
}

fn report(label: &str, stats: &[anistream_providers::torrent::session::TorrentProgress]) {
    for s in stats {
        println!(
            "  {label:<10} paused={:<5} peers={:<3} {} bytes ({:.2}%)",
            s.paused,
            s.live_peers,
            s.downloaded,
            s.fraction() * 100.0
        );
    }
    if stats.is_empty() {
        println!("  {label:<10} (no torrents in session)");
    }
}

#[tokio::main]
async fn main() {
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).unwrap_or_default();
    let guard = match VpnGuard::new(config.providers.torrent.vpn.clone()) {
        Ok(g) => g,
        Err(e) => {
            println!("✕ guard misconfigured: {e}");
            return;
        }
    };

    println!("── setup ──────────────────────────────────────────────");
    println!("  on_leak      {:?}", guard.on_leak());
    let state = guard.verify().await;
    println!("  guard        {}", state.badge());
    if !state.is_protected() {
        println!("  ✕ tunnel is not up; connect Mullvad and retry");
        return;
    }

    // A fresh directory so progress starts at zero and is unambiguous.
    let dir = paths.cache_dir.join("torrents-halt-probe");
    let _ = tokio::fs::remove_dir_all(&dir).await;

    let session = match TorrentSession::start(guard.clone(), dir.clone(), "halt".into()).await {
        Ok(s) => s,
        Err(e) => {
            println!("  ✕ session: {e}");
            return;
        }
    };

    println!();
    println!("── downloading (Big Buck Bunny, CC-BY 3.0) ────────────");
    let active = match session.stream(LEGAL_MAGNET, None).await {
        Ok(a) => a,
        Err(e) => {
            println!("  ✕ {e}");
            return;
        }
    };
    println!("  url          {}", active.url());

    // A reader has to be pulling, or librqbit has nothing to prioritise and sits idle. This
    // is what a player would be doing, so it is also the realistic case.
    let url = active.url().to_owned();
    let reader = tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(600))
            .build()
            .expect("client");
        // Stream the response body rather than buffering it, so bytes keep being requested
        // for as long as the task lives.
        if let Ok(response) = client.get(&url).send().await {
            let mut stream = response.bytes_stream();
            use futures::StreamExt;
            while let Some(chunk) = stream.next().await {
                if chunk.is_err() {
                    break;
                }
            }
        }
    });

    // Let real bytes accumulate. Without confirmed progress, "it stopped" proves nothing.
    let mut baseline = 0u64;
    for tick in 0..30 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let stats = session.stats();
        baseline = stats.iter().map(|s| s.downloaded).sum();
        if tick % 5 == 4 {
            report(&format!("t+{}s", tick + 1), &stats);
        }
        if baseline > 4 * 1024 * 1024 {
            break;
        }
    }
    report("running", &session.stats());

    if baseline == 0 {
        println!("  ✕ no bytes arrived — cannot prove a halt stopped anything");
        return;
    }
    println!("  ● {baseline} bytes downloaded and still climbing");

    println!();
    println!("── dropping the tunnel ────────────────────────────────");
    mullvad(&["disconnect"]).await;
    tokio::time::sleep(Duration::from_secs(3)).await;

    let leaked = guard.verify().await;
    println!("  guard        {}", leaked.badge());
    if leaked.is_protected() {
        println!("  ✕ guard still reports protected — it did not notice");
        mullvad(&["connect"]).await;
        return;
    }

    println!();
    println!("── halt ───────────────────────────────────────────────");
    session.halt().await;
    let at_halt: u64 = session.stats().iter().map(|s| s.downloaded).sum();
    report("halted", &session.stats());

    // The real assertion: give it time to keep going, and confirm it does not.
    println!();
    println!("── does it stay stopped? (10s) ────────────────────────");
    tokio::time::sleep(Duration::from_secs(10)).await;
    let stats = session.stats();
    let after: u64 = stats.iter().map(|s| s.downloaded).sum();
    report("after", &stats);

    let all_paused = !stats.is_empty() && stats.iter().all(|s| s.paused);
    let no_peers = stats.iter().all(|s| s.live_peers == 0);
    let grew = after.saturating_sub(at_halt);

    println!();
    println!("── verdict ────────────────────────────────────────────");
    println!("  bytes at halt   {at_halt}");
    println!("  bytes after 10s {after}  (+{grew})");
    println!("  all paused      {all_paused}");
    println!("  peers dropped   {no_peers}");

    // A torrent already in flight may land a few in-flight pieces after pausing; what must
    // not happen is continued transfer. Anything beyond a piece or two means it kept going.
    let tolerance = 4 * 1024 * 1024;
    if grew > tolerance {
        println!("  ✕ FAILED: download continued after the tunnel dropped");
    } else if !all_paused && !stats.is_empty() {
        println!("  ✕ FAILED: torrents are not paused");
    } else {
        println!("  ● PASSED: traffic stopped when the guard began leaking");
    }

    println!();
    println!("── reconnecting and resuming ──────────────────────────");
    mullvad(&["connect"]).await;
    for _ in 0..15 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if guard.verify().await.is_protected() {
            break;
        }
    }
    println!("  guard        {}", guard.state().badge());
    match session.resume().await {
        Ok(()) => {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let resumed: u64 = session.stats().iter().map(|s| s.downloaded).sum();
            report("resumed", &session.stats());
            if resumed > after {
                println!("  ● resumed cleanly (+{} bytes)", resumed - after);
            } else {
                println!("  (no measurable progress yet — peers may still be reconnecting)");
            }
        }
        Err(e) => println!("  resume refused: {e}"),
    }

    reader.abort();
    session.halt().await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    println!();
    println!("done; probe download removed.");
}
