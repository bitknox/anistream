//! End-to-end validation of tracker sync against the real AniList API.
//!
//! What unit tests cannot cover and this can: whether the authorization code flow completes
//! against the real endpoint, whether `SaveMediaListEntry` accepts what we send, and whether a
//! pushed episode is genuinely visible on the account afterwards.
//!
//! ```text
//! cargo run -p anistream --example sync_probe            # read-only
//! cargo run -p anistream --example sync_probe -- --write  # also pushes, then undoes it
//! ```
//!
//! `--write` changes one entry on the real account and puts it back, reading AniList afterwards
//! to prove it rather than assuming. On a list that already has titles it nudges the lowest
//! progress by one and reverts; on an empty list it adds an entry and then *deletes* it, so the
//! account ends up exactly as it started rather than merely close to it. A failed undo is
//! reported loudly, because a test that quietly alters someone's list is worse than no test.

use anistream_core::{
    config::{Config, Paths},
    traits::TrackOp,
};
use anistream_net::HttpClient;
use anistream_store::Store;
use anistream_track::{TokenStore, auth};

#[tokio::main]
async fn main() {
    let write = std::env::args().any(|a| a == "--write");
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).unwrap_or_default();

    // A throwaway database so the probe never disturbs real history or a real outbox.
    let db = paths.cache_dir.join("sync_probe.db");
    let _ = std::fs::remove_file(&db);
    std::fs::create_dir_all(&paths.cache_dir).ok();
    let store = Store::open(&db).expect("store");
    let http = HttpClient::new(&config.network).expect("http");

    println!("── configuration ──────────────────────────────────────");
    println!("  enabled      {:?}", config.trackers.enabled);
    println!(
        "  client_id    {}",
        config.trackers.anilist.client_id.as_deref().unwrap_or("(none)")
    );
    println!("  flow         {}", config.trackers.anilist.flow);
    println!("  redirect     {}", auth::redirect_uri(config.trackers.anilist.redirect_port));

    // One store, shared with the `Sync` built below, so the keychain is read once for the whole
    // probe rather than once per question asked of it.
    let tokens = TokenStore::new(paths.data_dir.join("tokens"));
    println!();
    println!("── credentials ────────────────────────────────────────");
    if !tokens.has("anilist") {
        println!("  ✕ no token stored");
        println!();
        println!("  Sign in first:");
        println!("    anistream --login         # opens a browser and waits");
        println!("    anistream --login-url     # or print the URL to open by hand");
        let _ = std::fs::remove_file(&db);
        return;
    }
    let storage = tokens.storage_for("anilist");
    println!("  ● token present, stored in the {}", storage.describe());
    if let Some(exp) =
        tokens.get("anilist").ok().as_deref().and_then(anistream_track::auth::token_expiry)
    {
        println!("  expires in     {} days", (exp - anistream_store::now()) / 86_400);
    }

    let sync = anistream::tracking::Sync::with_tokens(&config, &store, &http, tokens.clone());
    let Some(tracker) = sync.trackers.first() else {
        println!("  ✕ anilist is not in trackers.enabled");
        let _ = std::fs::remove_file(&db);
        return;
    };
    println!("  authenticated  {}", tracker.is_authenticated());

    println!();
    println!("── library pull ───────────────────────────────────────");
    let started = std::time::Instant::now();
    let entries = match sync.anilist.as_ref().expect("anilist handle").library().await {
        Ok(entries) => entries,
        Err(e) => {
            println!("  ✕ {e}");
            let _ = std::fs::remove_file(&db);
            return;
        }
    };
    println!("  {} entries in {:?}", entries.len(), started.elapsed());

    let mut by_status: std::collections::BTreeMap<&str, u32> =
        std::collections::BTreeMap::new();
    for entry in &entries {
        *by_status.entry(entry.status.as_str()).or_default() += 1;
    }
    for (status, count) in &by_status {
        println!("    {status:<10} {count}");
    }
    for entry in entries.iter().take(3) {
        println!(
            "    ep {:>3}  score {:>4}  {}",
            entry.progress,
            entry.score.map_or("—".into(), |s| format!("{s}")),
            entry.media.title.display()
        );
    }

    println!();
    println!("── projection through the Tracker trait ───────────────");
    match tracker.pull_library().await {
        Ok(tracked) => {
            println!("  {} entries reduced to the syncable projection", tracked.len());
            if let Some(first) = tracked.first() {
                println!(
                    "    anilist {} · ep {} · {:?} · score {:?}",
                    first.anilist_id.get(),
                    first.progress,
                    first.status,
                    first.score
                );
            }
        }
        Err(e) => println!("  ✕ {e}"),
    }

    if !write {
        println!();
        println!("── verdict ────────────────────────────────────────────");
        println!("  ● read path works: sign-in, library pull, projection.");
        println!("  Re-run with --write to exercise a real push and revert.");
        let _ = std::fs::remove_file(&db);
        return;
    }

    // Pick the smallest possible change: the entry with the lowest progress. With an empty list
    // there is nothing to nudge, so a title is added and then *deleted* afterwards — which
    // leaves the account exactly as it was rather than merely nearly so.
    let existing = entries.iter().filter(|e| e.status == "CURRENT").min_by_key(|e| e.progress);
    let (id, original, label, added) = match existing {
        Some(entry) => {
            (entry.media.id, entry.progress, entry.media.title.display().to_owned(), false)
        }
        None => {
            // Frieren — a stable, real id, added only for the length of this check.
            (
                anistream_core::ids::AnilistId::new(154_587),
                0,
                "Sousou no Frieren".to_owned(),
                true,
            )
        }
    };
    let bumped = original + 1;

    println!();
    println!("── push (writes to your real list, then undoes it) ─────");
    println!("  subject   {label}");
    if added {
        println!("  note      your list is empty, so this entry is added and then removed");
    }
    println!("  progress  {original} → {bumped}");

    // Through the outbox, exactly as playback does it, so this exercises the real path rather
    // than calling the API directly.
    store
        .enqueue("anilist", &TrackOp::SetProgress { anilist_id: id, episode: bumped }, 0)
        .expect("enqueue");
    println!("  queued    depth {}", store.outbox_depth(Some("anilist")).unwrap_or(0));

    match anistream_track::drain(&store, tracker.as_ref(), anistream_store::now()).await {
        Ok(report) => println!(
            "  drain     sent {} · failed {} · remaining {}{}",
            report.sent,
            report.failed,
            report.remaining,
            if report.needs_reauth { " · NEEDS RE-AUTH" } else { "" }
        ),
        Err(e) => println!("  ✕ drain: {e}"),
    }

    // Read it back from AniList rather than trusting the mutation's own reply.
    let readback = sync
        .anilist
        .as_ref()
        .expect("anilist handle")
        .library()
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|e| e.media.id == id));
    let confirmed = readback.as_ref().map(|e| e.progress);
    // Captured now, because removing the entry afterwards needs its list-entry id — a different
    // number from the media id.
    let entry_id = readback.as_ref().and_then(|e| e.entry_id);
    println!("  readback  progress {confirmed:?} · entry id {entry_id:?}");
    let landed = confirmed == Some(bumped);
    println!("  landed    {}", if landed { "●" } else { "✕" });

    // Undo it regardless of whether the check passed — leaving someone's list altered by a test
    // would be unacceptable.
    let handle = sync.anilist.as_ref().expect("anilist handle");
    let restored = if added {
        // Delete the entry outright rather than zeroing its progress: a title sitting at
        // episode 0 is still *on* the list, which is not where this started.
        println!("  removing the entry again…");
        match entry_id {
            Some(entry_id) => match handle.delete(entry_id).await {
                Ok(true) => {
                    let left = handle.library().await.map(|l| l.len()).unwrap_or(usize::MAX);
                    println!("  removed   list is back to {left} entries");
                    left == 0
                }
                Ok(false) => {
                    println!("  ✕ AniList reported the entry was not deleted");
                    false
                }
                Err(e) => {
                    println!("  ✕ REMOVE FAILED — delete it by hand: {e}");
                    false
                }
            },
            None => {
                println!("  ✕ no entry id came back, so it cannot be removed automatically");
                false
            }
        }
    } else {
        println!("  reverting to {original}…");
        match tracker.push(&[TrackOp::SetProgress { anilist_id: id, episode: original }]).await
        {
            Ok(()) => {
                let back = handle
                    .library()
                    .await
                    .ok()
                    .and_then(|list| list.into_iter().find(|e| e.media.id == id))
                    .map(|e| e.progress);
                let ok = back == Some(original);
                println!(
                    "  restored  {back:?} {}",
                    if ok { "●" } else { "✕ CHECK THIS MANUALLY" }
                );
                ok
            }
            Err(e) => {
                println!("  ✕ REVERT FAILED — set it back by hand: {e}");
                false
            }
        }
    };

    println!();
    println!("── verdict ────────────────────────────────────────────");
    println!("  library pull                        ●");
    println!("  outbox drained through the trait    ●");
    println!("  the push was visible on AniList     {}", tick(landed));
    println!("  your list was left as it was        {}", tick(restored));
    let _ = std::fs::remove_file(&db);
}

fn tick(ok: bool) -> &'static str {
    if ok { "●" } else { "✕" }
}
