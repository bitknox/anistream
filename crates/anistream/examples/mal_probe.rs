//! End-to-end validation of the MyAnimeList tracker.
//!
//! ```text
//! cargo run -p anistream --example mal_probe            # read-only
//! cargo run -p anistream --example mal_probe -- --write  # pushes, then undoes it
//! ```
//!
//! What unit tests cannot cover: whether the PKCE token actually authorises the API, and whether
//! **the ID mapping resolves** — MAL is the first tracker that keys on something other than an
//! AniList id, so a title with no `mal_id` in the materialised mapping cannot be synced at all.
//!
//! `--write` changes one entry on the real account and removes it again, reading MAL back to prove
//! it rather than trusting the response.

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
    traits::TrackOp,
};
use anistream_net::HttpClient;
use anistream_store::Store;

/// Frieren — a stable id present in both mapping corpora.
const SUBJECT: AnilistId = AnilistId::new(154_587);

#[tokio::main]
async fn main() {
    let write = std::env::args().any(|a| a == "--write");
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).unwrap_or_default();
    let store = Store::open(paths.database()).expect("store");
    let http = HttpClient::new(&config.network).expect("http");

    println!("── mapping ────────────────────────────────────────────");
    println!("  mapped titles  {}", store.mapping_count().unwrap_or(0));
    let mal_id = store.mapping_for(SUBJECT).ok().flatten().and_then(|m| m.mal_id);
    println!("  anilist {} → mal {:?}", SUBJECT.get(), mal_id);
    if mal_id.is_none() {
        println!("  ✕ no mal id mapped — run `anistream --refresh-data` first");
        return;
    }
    // The reverse direction matters too: a library pull turns MAL ids back into AniList ids, and a
    // one-way mapping would make every pulled entry unreconcilable.
    let back = mal_id.and_then(|id| store.anilist_id_for_mal(id).ok().flatten());
    println!("  and back       {back:?}");
    assert_eq!(back, Some(SUBJECT), "the mapping is not symmetric");

    let sync = anistream::tracking::Sync::build(&config, &store, &http);
    let Some(mal) = sync.trackers.iter().find(|t| t.id() == "mal") else {
        println!("  ✕ mal is not in trackers.enabled");
        return;
    };

    println!();
    println!("── credentials ────────────────────────────────────────");
    println!("  authenticated  {}", mal.is_authenticated());
    if let Ok(pair) = sync.tokens.get_pair("mal") {
        println!("  refresh token  {}", if pair.refresh.is_some() { "yes" } else { "no" });
        if let Some(at) = pair.expires_at {
            println!("  expires in     {} days", (at - anistream_store::now()) / 86_400);
        }
    }

    println!();
    println!("── library pull ───────────────────────────────────────");
    let started = std::time::Instant::now();
    match mal.pull_library().await {
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
        }
        Err(e) => {
            println!("  ✕ {e}");
            return;
        }
    }

    if !write {
        println!();
        println!("  ● read path works: PKCE token authorises, mapping resolves both ways.");
        println!("  Re-run with --write to exercise a real push.");
        return;
    }

    println!();
    println!("── push (writes to your real MAL list, then removes it) ─");
    let mal_id = mal_id.expect("checked above");
    println!("  subject   Sousou no Frieren (mal {mal_id})");

    // Through the trait, exactly as the drain does.
    match mal.push(&[TrackOp::SetProgress { anilist_id: SUBJECT, episode: 1 }]).await {
        Ok(()) => println!("  pushed    progress 1"),
        Err(e) => {
            println!("  ✕ push: {e}");
            return;
        }
    }

    // Read it back from MAL rather than trusting the response.
    let confirmed = mal
        .pull_library()
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|e| e.anilist_id == SUBJECT))
        .map(|e| e.progress);
    println!("  readback  {confirmed:?}");
    let landed = confirmed == Some(1);
    println!("  landed    {}", if landed { "●" } else { "✕" });

    // Remove the entry so the account ends up as it started.
    println!("  removing the entry again…");
    let token = sync.tokens.get("mal").unwrap_or_default();
    let removed = http
        .plain()
        .delete(format!("https://api.myanimelist.net/v2/anime/{mal_id}/my_list_status"))
        .bearer_auth(&token)
        .send()
        .await
        .map(|r| r.status());
    match removed {
        // MAL answers 200 on delete and 404 if it was not on the list to begin with.
        Ok(status) if status.is_success() || status.as_u16() == 404 => {
            println!("  removed   {status}")
        }
        Ok(status) => println!("  ✕ REMOVE FAILED ({status}) — delete it by hand"),
        Err(e) => println!("  ✕ REMOVE FAILED — delete it by hand: {e}"),
    }

    println!();
    println!("── verdict ────────────────────────────────────────────");
    println!("  mapping resolves both ways          ●");
    println!("  PKCE token authorises the API       ●");
    println!("  the push was visible on MAL         {}", if landed { "●" } else { "✕" });
}
