//! End-to-end validation of the Simkl tracker.
//!
//! ```text
//! cargo run -p anistream --example simkl_probe            # read-only
//! cargo run -p anistream --example simkl_probe -- --write # marks one episode, then removes it
//! ```
//!
//! What unit tests cannot cover: whether the PIN-flow token actually authorises the API, and whether
//! Simkl's **MAL-id bridge** resolves — Simkl is the second tracker that keys on something other
//! than an AniList id, and the claim that the mapping layer made it nearly free is only worth
//! anything if it holds against real data.
//!
//! The sign-in itself is not run here. It needs a human to type a code into a web page, which is
//! exactly what `anistream` does interactively — this probe assumes a token is already stored.

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
    traits::TrackOp,
};
use anistream_net::HttpClient;
use anistream_store::Store;

/// Frieren — present in both mapping corpora, so the MAL bridge has something to resolve.
const SUBJECT: AnilistId = AnilistId::new(154_587);

#[tokio::main]
async fn main() {
    let write = std::env::args().any(|a| a == "--write");
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).expect("config");
    let store = Store::open(paths.database()).expect("store");
    let http = HttpClient::new(&config.network).expect("http");

    println!("── configuration ──────────────────────────────────────");
    println!("  enabled       {:?}", config.trackers.enabled);
    let client_id = config.trackers.simkl.client_id.clone().unwrap_or_default();
    println!(
        "  client id     {}",
        if client_id.is_empty() {
            "MISSING".into()
        } else {
            format!("{}…", &client_id[..12])
        }
    );
    if client_id.is_empty() {
        println!("  ✕ set trackers.simkl.client_id first");
        return;
    }

    println!();
    println!("── the mapping bridge ─────────────────────────────────");
    // The claim under test: a third tracker needed an auth flow and a push call, not a new identity
    // system, because `mal_id` was already materialised for MAL's sake.
    let mal_id = store.mapping_for(SUBJECT).ok().flatten().and_then(|m| m.mal_id);
    println!("  mapped titles {}", store.mapping_count().unwrap_or(0));
    println!("  anilist {} → mal {mal_id:?}", SUBJECT.get());
    let back = mal_id.and_then(|id| store.anilist_id_for_mal(id).ok().flatten());
    println!("  and back      {back:?}");
    if mal_id.is_none() {
        println!("  ✕ no mal id — run `anistream --refresh-data` first");
        return;
    }
    assert_eq!(back, Some(SUBJECT), "the bridge is not symmetric");

    let sync = anistream::tracking::Sync::build(&config, &store, &http);
    let Some(simkl) = sync.trackers.iter().find(|t| t.id() == "simkl") else {
        println!("  ✕ simkl is not in trackers.enabled");
        return;
    };

    println!();
    println!("── credentials ────────────────────────────────────────");
    println!("  authenticated {}", simkl.is_authenticated());
    if !simkl.is_authenticated() {
        println!();
        println!(
            "  Not signed in. Run anistream and press 9 for Accounts, or `--login --tracker simkl`."
        );
        return;
    }

    println!();
    println!("── library pull ───────────────────────────────────────");
    let started = std::time::Instant::now();
    match simkl.pull_library().await {
        Ok(entries) => {
            println!("  {} entries in {:?}", entries.len(), started.elapsed());
            for entry in entries.iter().take(3) {
                println!(
                    "    anilist {} · ep {} · {:?}",
                    entry.anilist_id.get(),
                    entry.progress,
                    entry.status
                );
            }
            // Worth reporting rather than asserting: entries Simkl knows but the mapping does not
            // are skipped by design, and the count is how you tell a mapping gap from an empty list.
            println!("  (titles with no anilist mapping are skipped, not guessed)");
        }
        Err(e) => {
            println!("  ✕ {e}");
            return;
        }
    }

    if !write {
        println!();
        println!("  ● read path works: the PIN token authorises and the MAL bridge resolves.");
        println!("  Re-run with --write to exercise a real push.");
        return;
    }

    println!();
    println!("── push (writes to your real Simkl list, then removes it) ─");
    match simkl.push(&[TrackOp::SetProgress { anilist_id: SUBJECT, episode: 1 }]).await {
        Ok(()) => println!("  pushed    episode 1 marked watched"),
        Err(e) => {
            println!("  ✕ push: {e}");
            return;
        }
    }

    // Read it back from Simkl rather than trusting the response.
    let confirmed = simkl
        .pull_library()
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|e| e.anilist_id == SUBJECT))
        .map(|e| e.progress);
    println!("  readback  {confirmed:?}");
    let landed = confirmed.is_some_and(|p| p >= 1);
    println!("  landed    {}", if landed { "●" } else { "✕" });

    // Put the account back as it was. Simkl removes history through its own endpoint, which the
    // tracker trait has no business exposing — so it is done directly here.
    println!("  removing the entry again…");
    let token = sync.tokens.get("simkl").unwrap_or_default();
    let mal_id = mal_id.expect("checked above");
    let removed = http
        .plain()
        .post("https://api.simkl.com/sync/history/remove")
        .header("simkl-api-key", &client_id)
        .bearer_auth(&token)
        .json(&serde_json::json!({ "shows": [{ "ids": { "mal": mal_id } }] }))
        .send()
        .await
        .map(|r| r.status());
    match removed {
        Ok(status) if status.is_success() => println!("  removed   {status}"),
        Ok(status) => println!("  ✕ REMOVE FAILED ({status}) — delete it by hand"),
        Err(e) => println!("  ✕ REMOVE FAILED — delete it by hand: {e}"),
    }

    println!();
    println!("── verdict ────────────────────────────────────────────");
    println!("  mal bridge resolves both ways        ●");
    println!("  PIN token authorises the API         ●");
    println!("  the push was visible on Simkl        {}", if landed { "●" } else { "✕" });
}
