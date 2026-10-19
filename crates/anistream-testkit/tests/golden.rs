//! `assert_golden` from a dependent crate's point of view: the golden is found under that
//! crate's own `tests/goldens`, through the manifest directory cargo hands the test.

use anistream_testkit::{assert_golden, check_golden, fixtures};

/// A small table built from the fixtures, so the lookup and the comparison are checked
/// together.
fn table() -> String {
    let rows = [fixtures::FINISHED, fixtures::MOVIE].map(|id| {
        let media = fixtures::media(id);
        let episodes = media.episodes.unwrap_or_default();
        let unit = if episodes == 1 { "episode" } else { "episodes" };
        format!("  {:<24}{episodes:>2} {unit}", media.title.display())
    });
    format!("anistream-testkit golden\n{}\n", rows.join("\n"))
}

#[test]
fn a_committed_golden_matches() {
    assert_golden("kit/sample", &table());
}

#[test]
fn a_changed_text_fails_against_it() {
    // Through `check_golden` with blessing off, not `assert_golden`: under `ANISTREAM_BLESS=1`
    // the latter would write this wrong text over the committed golden.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens");
    let changed = table().replace("12 episodes", "13 episodes");
    let message = check_golden(&dir, "kit/sample", &changed, false).unwrap_err();
    assert!(message.contains("+   Stardust Wayfarer       13 episodes"), "{message}");
}
