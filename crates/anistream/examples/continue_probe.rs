//! Does "continue where you left off" survive quitting an episode halfway?
//!
//! ```text
//! cargo run -p anistream --example continue_probe
//! ```
//!
//! Reported from real use: quitting an episode at ~50% left no visible trace, because the CONTINUE
//! rail was showing the current season and the resume position lived only in the database. The
//! distinction this checks is the one that was conflated — `playback.commit_threshold` governs when
//! an episode is *counted* and pushed to a tracker, and it must have nothing to do with whether you
//! can pick the episode back up.
//!
//! Writes to a temporary database, never the real one.

use anistream_core::{config::Config, ids::AnilistId};
use anistream_store::{Store, WatchEvent};

const SUBJECT: AnilistId = AnilistId::new(154_587);
const RUNTIME: f64 = 1440.0;

#[tokio::main]
async fn main() {
    let config = Config::default();
    let threshold = config.playback.commit_threshold;
    let store = Store::open_in_memory().expect("store");

    println!("── the two thresholds ─────────────────────────────────");
    println!("  commit_threshold   {:.0}%  (counts as watched, syncs)", threshold * 100.0);
    println!(
        "  resume ceiling     {:.0}%  (its own constant, not the sync one)",
        anistream_store::RESUME_CEILING * 100.0
    );
    println!();

    // Four episodes finished, then episode 5 abandoned at half.
    println!("── history ────────────────────────────────────────────");
    for episode in 1..=4 {
        store
            .record_event(&WatchEvent {
                duration_secs: Some(RUNTIME),
                completed: true,
                ..WatchEvent::new(SUBJECT, episode.to_string(), RUNTIME * 0.99)
            })
            .expect("record");
    }
    let abandoned = RUNTIME * 0.5;
    store
        .record_event(&WatchEvent {
            duration_secs: Some(RUNTIME),
            completed: false,
            ..WatchEvent::new(SUBJECT, "5", abandoned)
        })
        .expect("record");
    println!("  eps 1-4 finished, ep 5 quit at {:.0}s of {RUNTIME:.0}s (50%)", abandoned);

    println!();
    println!("── what the rail is built from ────────────────────────");
    let listed = store.continue_list(15).expect("continue list");
    println!("  titles in the rail  {}", listed.len());
    assert!(!listed.is_empty(), "a part-watched title must appear in the rail");
    let progress = store.progress(SUBJECT).expect("progress").expect("some");
    println!("  episodes_done       {}", progress.episodes_done);
    println!("  last touched        ep {}", progress.last_episode);

    println!();
    println!("── resume, at the sync threshold and without it ───────");
    // `resume_position` takes no threshold at all now. It used to, and every caller passed the
    // sync threshold — tying "can I carry on watching this" to "should this be reported as
    // watched". This probe is what caught it: trying to opt out by passing 0.0 made `is_complete`
    // true for every position, so resume silently returned nothing.
    let resume = store.resume_position(SUBJECT, "5").expect("resume");
    println!("  ep 5 at 50%         {resume:?}");
    assert_eq!(resume, Some(abandoned), "a half-watched episode must be resumable");

    println!();
    println!("── a finished episode must NOT be resumable ───────────");
    let finished = store.resume_position(SUBJECT, "4").expect("resume");
    println!("  ep 4 (completed)    {finished:?}");
    assert_eq!(finished, None, "resuming a finished episode would land on the credits");

    println!();
    println!("── and it still must not count as watched ─────────────");
    let counted = store.completed_episode_count(SUBJECT).expect("count");
    println!("  completed episodes  {counted}  (pushed to trackers)");
    assert_eq!(counted, 4, "half an episode must not inflate tracker progress");

    println!();
    println!("── a three-second glance is not 'continuing' ──────────");
    // Measured on a real database: opening a title and closing it after three seconds put it at the
    // top of the rail, above an episode genuinely left at 61%.
    let glanced = AnilistId::new(1);
    store
        .record_event(&WatchEvent {
            duration_secs: Some(RUNTIME),
            completed: false,
            ..WatchEvent::new(glanced, "1", 3.0)
        })
        .expect("record");
    let listed = store.continue_list(15).expect("continue list");
    println!("  titles in the rail  {} (the glance is newest, and excluded)", listed.len());
    assert!(
        !listed.iter().any(|p| p.anilist_id == glanced),
        "a three-second glance must not head the rail"
    );
    assert_eq!(listed.first().map(|p| p.anilist_id), Some(SUBJECT), "newest real watch first");

    println!();
    println!("── verdict ────────────────────────────────────────────");
    println!("  half-watched episode appears in CONTINUE   ●");
    println!("  resumes at the exact position              ●");
    println!("  finished episode is not offered            ●");
    println!("  sync progress unaffected by a partial      ●");
    println!("  a three-second glance is filtered out       ●");
    println!("  most recently watched is first             ●");
}
