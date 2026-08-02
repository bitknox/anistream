//! End-to-end validation of the Kitsu tracker.
//!
//! ```text
//! cargo run -p anistream --example kitsu_probe            # read-only
//! cargo run -p anistream --example kitsu_probe -- --write  # pushes, then removes it
//! ```
//!
//! Three things unit tests cannot cover, in the order they can fail: whether **the ID
//! mapping resolves** both ways, whether the wire shape assumptions hold against the
//! live JSON:API — types, relationship linkage, `links.next` — and whether a password
//! token actually authorises reads and writes.
//!
//! The wire-shape half runs with no account at all, against Kitsu's public read
//! surface, so this probe says something useful before anyone signs in. `--write`
//! changes one entry on the real account and deletes it again, reading Kitsu back to
//! prove it rather than trusting the response.

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
    traits::TrackOp,
};
use anistream_net::HttpClient;
use anistream_store::Store;

/// Frieren — a stable id present in both mapping corpora.
const SUBJECT: AnilistId = AnilistId::new(154_587);

const API: &str = "https://kitsu.io/api/edge";
const JSON_API: &str = "application/vnd.api+json";

#[tokio::main]
async fn main() {
    let write = std::env::args().any(|a| a == "--write");
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).unwrap_or_default();
    let store = Store::open(paths.database()).expect("store");
    let http = HttpClient::new(&config.network).expect("http");

    println!("── mapping ────────────────────────────────────────────");
    println!("  mapped titles  {}", store.mapping_count().unwrap_or(0));
    let kitsu_id = store.mapping_for(SUBJECT).ok().flatten().and_then(|m| m.kitsu_id);
    println!("  anilist {} → kitsu {:?}", SUBJECT.get(), kitsu_id);
    if kitsu_id.is_none() {
        println!("  ✕ no kitsu id mapped — run `anistream --refresh-data` first");
        return;
    }
    let back = kitsu_id.and_then(|id| store.anilist_id_for_kitsu(id).ok().flatten());
    println!("  and back       {back:?}");
    assert_eq!(back, Some(SUBJECT), "the mapping is not symmetric");

    // The wire-shape checks need no account: library entries are publicly readable, so
    // the assumptions the tracker's pull is built on can be proven before a sign-in.
    println!();
    println!("── wire shape (no account needed) ─────────────────────");
    let kitsu_id = kitsu_id.expect("checked above");
    let title: serde_json::Value = http
        .plain()
        .get(format!("{API}/anime/{kitsu_id}?fields[anime]=canonicalTitle"))
        .header("Accept", JSON_API)
        .send()
        .await
        .expect("kitsu unreachable")
        .json()
        .await
        .expect("not json");
    println!(
        "  kitsu {} is    {:?}",
        kitsu_id,
        title["data"]["attributes"]["canonicalTitle"].as_str().unwrap_or("?")
    );

    // One public page, with `pull_library`'s exact field and include set. Scoped to one
    // anime because that is indexed the way a user filter is — the unscoped listing is a
    // full-table walk that times out, and proving *that* proves nothing about the pull.
    let page: serde_json::Value = http
        .plain()
        .get(format!(
            "{API}/library-entries?filter[kind]=anime&filter[animeId]={kitsu_id}\
             &fields[libraryEntries]=progress,status,ratingTwenty,anime\
             &include=anime&fields[anime]=id&page[limit]=2"
        ))
        .header("Accept", JSON_API)
        .send()
        .await
        .expect("kitsu unreachable")
        .json()
        .await
        .expect("not json");
    let first = &page["data"][0];
    let linkage = first["relationships"]["anime"]["data"]["id"].as_str();
    let next = page["links"]["next"].as_str().unwrap_or_default();
    println!("  entry type     {:?}", first["type"].as_str().unwrap_or("?"));
    println!("  attributes     {}", first["attributes"]);
    println!("  anime linkage  {linkage:?}");
    println!("  next page      on the api: {}", next.starts_with(API));
    assert_eq!(first["type"].as_str(), Some("libraryEntries"), "type name changed");
    assert!(linkage.is_some(), "include=anime no longer carries resource linkage");
    assert!(next.starts_with(API), "pagination left the api host");

    let sync = anistream::tracking::Sync::build(&config, &store, &http);
    let Some(kitsu) = sync.trackers.iter().find(|t| t.id() == "kitsu") else {
        println!();
        println!("  ● wire shape holds. Add \"kitsu\" to trackers.enabled to probe further.");
        return;
    };

    println!();
    println!("── credentials ────────────────────────────────────────");
    println!("  authenticated  {}", kitsu.is_authenticated());
    if !kitsu.is_authenticated() {
        println!("  sign in first: anistream --login --tracker kitsu");
        return;
    }
    if let Ok(pair) = sync.tokens.get_pair("kitsu") {
        println!("  refresh token  {}", if pair.refresh.is_some() { "yes" } else { "no" });
        if let Some(at) = pair.expires_at {
            println!("  expires in     {} days", (at - anistream_store::now()) / 86_400);
        }
    }

    println!();
    println!("── library pull ───────────────────────────────────────");
    let started = std::time::Instant::now();
    match kitsu.pull_library().await {
        Ok(entries) => {
            println!("  {} entries in {:?}", entries.len(), started.elapsed());
            for entry in entries.iter().take(3) {
                println!(
                    "    anilist {} · ep {} · {:?} · score {:?}",
                    entry.anilist_id.get(),
                    entry.progress,
                    entry.status,
                    entry.score
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
        println!("  ● read path works: token authorises, mapping resolves, shapes hold.");
        println!("  Re-run with --write to exercise a real push.");
        return;
    }

    println!();
    println!("── push (writes to your real Kitsu list, then removes it) ─");
    println!("  subject   Sousou no Frieren (kitsu {kitsu_id})");

    // Through the trait, exactly as the drain does — progress and a score in one
    // batch, so the coalesce-then-upsert path is what gets exercised.
    let ops = [
        TrackOp::SetProgress { anilist_id: SUBJECT, episode: 1 },
        TrackOp::SetScore { anilist_id: SUBJECT, score: 9.0, at: anistream_store::now() },
    ];
    match kitsu.push(&ops).await {
        Ok(()) => println!("  pushed    progress 1, score 9"),
        Err(e) => {
            println!("  ✕ push: {e}");
            return;
        }
    }

    // Read it back from Kitsu rather than trusting the response.
    let confirmed = kitsu
        .pull_library()
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|e| e.anilist_id == SUBJECT))
        .map(|e| (e.progress, e.score));
    println!("  readback  {confirmed:?}");
    let landed = matches!(confirmed, Some((1, Some(score))) if (score - 9.0).abs() < 0.01);
    println!("  landed    {}", if landed { "●" } else { "✕" });

    // Remove the entry so the account ends up as it started. The entry id comes from
    // the same lookup the push uses; the delete is plain JSON:API.
    println!("  removing the entry again…");
    let token = sync.tokens.get("kitsu").unwrap_or_default();
    let user: serde_json::Value = http
        .plain()
        .get(format!("{API}/users?filter[self]=true"))
        .bearer_auth(&token)
        .header("Accept", JSON_API)
        .send()
        .await
        .expect("kitsu unreachable")
        .json()
        .await
        .expect("not json");
    let user_id = user["data"][0]["id"].as_str().unwrap_or_default();
    let found: serde_json::Value = http
        .plain()
        .get(format!(
            "{API}/library-entries?filter[userId]={user_id}&filter[animeId]={kitsu_id}\
             &fields[libraryEntries]=status&page[limit]=1"
        ))
        .bearer_auth(&token)
        .header("Accept", JSON_API)
        .send()
        .await
        .expect("kitsu unreachable")
        .json()
        .await
        .expect("not json");
    let removed = match found["data"][0]["id"].as_str() {
        Some(entry_id) => http
            .plain()
            .delete(format!("{API}/library-entries/{entry_id}"))
            .bearer_auth(&token)
            .header("Accept", JSON_API)
            .send()
            .await
            .map(|r| r.status()),
        None => {
            println!("  the entry is already gone");
            return;
        }
    };
    match removed {
        Ok(status) if status.is_success() => println!("  removed   {status}"),
        Ok(status) => println!("  ✕ REMOVE FAILED ({status}) — delete it by hand on kitsu.app"),
        Err(e) => println!("  ✕ REMOVE FAILED — delete it by hand on kitsu.app: {e}"),
    }

    println!();
    println!("── verdict ────────────────────────────────────────────");
    println!("  mapping resolves both ways          ●");
    println!("  wire shapes hold against the api    ●");
    println!("  the push was visible on kitsu       {}", if landed { "●" } else { "✕" });
}
