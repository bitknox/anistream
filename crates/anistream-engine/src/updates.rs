//! The update check: a quiet daily look for a newer release.
//!
//! The check is a single HTTPS request to GitHub's releases API, at most once a day, cached on
//! disk, and it only ever *says* a version exists — nothing downloads without `--update` being
//! asked for. Taking the new version is the binary's `--update`, which finds the release through
//! [`latest_tag`] and [`is_newer`] here.
//!
//! Every push to main is a release, most of them a new patch, so the notice does not name each
//! one as it comes: see [`should_tell`].

use std::path::Path;

use anistream_model::{event::Event, notice::Notice};
use anistream_net::HttpClient;

use crate::outbox::Outbox;

/// The repository whose releases the check looks for and the updater installs from.
pub const REPO: &str = "bitknox/anistream";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// `v0.2.0` → `[0, 2, 0]`. Anything unparseable compares as nothing at all.
fn parse_semver(tag: &str) -> Option<[u32; 3]> {
    let mut parts = tag.trim().trim_start_matches('v').splitn(3, '.');
    let mut out = [0u32; 3];
    for slot in &mut out {
        *slot = parts.next()?.trim().parse().ok()?;
    }
    Some(out)
}

/// Whether `candidate` names a newer release than `current`. A tag that does not parse is never
/// newer, so a mangled one cannot announce an update.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_semver(candidate), parse_semver(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// Ask GitHub for the newest release tag.
pub async fn latest_tag(http: &HttpClient) -> Option<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let response = http.plain().get(&url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body: serde_json::Value = response.json().await.ok()?;
    body.get("tag_name")?.as_str().map(str::to_owned)
}

/// How long a newer patch waits before it is told again.
const RETELL_SECS: i64 = 7 * 24 * 3600;

/// Whether the notice should say that `latest` is out, given `told`, the last notice's time and
/// tag, if there has been one.
///
/// A newer release is told the first time it is found. After that a new patch waits a week from
/// the last notice, which keeps it to one a week while releases come with most merges. A new
/// minor or major, which the version in Cargo.toml is raised for, is told the first time it is
/// found, whenever the last notice was.
fn should_tell(latest: &str, current: &str, told: Option<(i64, &str)>, now: i64) -> bool {
    if !is_newer(latest, current) {
        return false;
    }
    let Some((at, told_tag)) = told else { return true };
    let (Some(new), Some(old)) = (parse_semver(latest), parse_semver(told_tag)) else {
        return true;
    };
    new[..2] > old[..2] || now.saturating_sub(at) >= RETELL_SECS
}

/// What the notice says, short enough for the terminal's status rule. Only the terminal app
/// updates itself, so only its notice names `--update`.
fn notice_text(tag: &str) -> String {
    if cfg!(feature = "desktop") {
        format!("{tag} is out. Run `anistream --update` to install it.")
    } else {
        format!("anistream {tag} is out.")
    }
}

/// A stamp file's "<unix time> <tag>".
fn read_stamp(path: &Path) -> Option<(i64, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let (at, tag) = text.trim().split_once(' ')?;
    Some((at.parse().ok()?, tag.to_owned()))
}

/// The daily check. Fire-and-forget from startup; a failure is silence, never a toast —
/// an update notice that cries wolf on every flaky network would be turned off in a week.
pub fn spawn_check(enabled: bool, cache_dir: &Path, http: &HttpClient, tx: &Outbox) {
    if !enabled {
        return;
    }
    let stamp = cache_dir.join("update_check");
    // The last notice, so a release already told is not told again at every launch.
    let told = cache_dir.join("update_notice");
    let (http, tx) = (http.clone(), tx.clone());
    tokio::spawn(async move {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);

        // The stamp holds the last answer, so sessions inside the window decide without a
        // request.
        let tag = match read_stamp(&stamp) {
            Some((at, tag)) if now.saturating_sub(at) < 24 * 3600 => tag,
            _ => {
                let Some(tag) = latest_tag(&http).await else { return };
                let _ = std::fs::write(&stamp, format!("{now} {tag}"));
                tag
            }
        };

        let last = read_stamp(&told);
        if should_tell(&tag, CURRENT, last.as_ref().map(|(at, tag)| (*at, tag.as_str())), now) {
            let _ = std::fs::write(&told, format!("{now} {tag}"));
            let _ = tx.send(Event::Toast(Notice::info(notice_text(&tag))));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_reads_tags_the_way_releases_write_them() {
        assert!(is_newer("v0.3.0", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("v0.1.9", "0.2.0"));
        // Garbage never counts as an upgrade — a mangled tag must not trigger a notice.
        assert!(!is_newer("nightly", "0.2.0"));
        assert!(!is_newer("", "0.2.0"));
        // Patches past 9 compare as numbers, as main's releases run on.
        assert!(is_newer("v2.0.10", "2.0.9"));
    }

    const DAY: i64 = 24 * 3600;
    const NOW: i64 = 1_800_000_000;

    #[test]
    fn a_newer_release_is_told_the_first_time_it_is_found() {
        assert!(should_tell("v2.0.1", "2.0.0", None, NOW));
        // Nothing newer, nothing to tell.
        assert!(!should_tell("v2.0.0", "2.0.0", None, NOW));
        assert!(!should_tell("v1.9.0", "2.0.0", None, NOW));
        // A last notice that does not read is no reason to stay quiet.
        assert!(should_tell("v2.0.1", "2.0.0", Some((NOW, "garbage")), NOW));
    }

    #[test]
    fn a_newer_patch_is_told_at_most_once_a_week() {
        let told = Some((NOW - DAY, "v2.0.1"));
        // The same patch, and the next ones as main releases them, wait out the week.
        assert!(!should_tell("v2.0.1", "2.0.0", told, NOW));
        assert!(!should_tell("v2.0.2", "2.0.0", told, NOW));
        assert!(!should_tell("v2.0.9", "2.0.0", told, NOW + 5 * DAY));
        // A week after the last notice, the newest is told.
        assert!(should_tell("v2.0.9", "2.0.0", told, NOW + 6 * DAY));
        // Taken in between: a newer patch still waits for the week.
        assert!(!should_tell("v2.0.4", "2.0.3", Some((NOW, "v2.0.3")), NOW + 2 * DAY));
    }

    #[test]
    fn a_new_minor_or_major_is_told_at_once() {
        let told = Some((NOW - DAY, "v2.0.5"));
        assert!(should_tell("v2.1.0", "2.0.0", told, NOW));
        assert!(should_tell("v3.0.0", "2.0.0", told, NOW));
        // Its own patches are patches again.
        assert!(!should_tell("v2.1.1", "2.0.0", Some((NOW - DAY, "v2.1.0")), NOW));
    }

    #[test]
    fn the_notice_names_the_release_and_how_to_take_it() {
        let text = notice_text("v9.0.0");
        assert!(text.contains("v9.0.0"), "{text}");
        assert_eq!(text.contains("anistream --update"), cfg!(feature = "desktop"), "{text}");
    }
}
