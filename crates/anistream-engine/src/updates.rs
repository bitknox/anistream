//! The update check: a quiet daily look for a newer release.
//!
//! The check is a single HTTPS request to GitHub's releases API, at most once a day, cached on
//! disk, and it only ever *says* a version exists — nothing downloads without `--update` being
//! asked for. Taking the new version is the binary's `--update`, which finds the release through
//! [`latest_tag`] and [`is_newer`] here.

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

/// The daily check. Fire-and-forget from startup; a failure is silence, never a toast —
/// an update notice that cries wolf on every flaky network would be turned off in a week.
pub fn spawn_check(enabled: bool, cache_dir: &Path, http: &HttpClient, tx: &Outbox) {
    if !enabled {
        return;
    }
    let stamp = cache_dir.join("update_check");
    let (http, tx) = (http.clone(), tx.clone());
    tokio::spawn(async move {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);

        // The stamp holds the last answer, so sessions inside the window still get the
        // notice without a request.
        let cached = std::fs::read_to_string(&stamp).ok().and_then(|s| {
            let (at, tag) = s.trim().split_once(' ')?;
            Some((at.parse::<i64>().ok()?, tag.to_owned()))
        });
        let tag = match cached {
            Some((at, tag)) if now.saturating_sub(at) < 24 * 3600 => tag,
            _ => {
                let Some(tag) = latest_tag(&http).await else { return };
                let _ = std::fs::write(&stamp, format!("{now} {tag}"));
                tag
            }
        };

        if is_newer(&tag, CURRENT) {
            let _ = tx.send(Event::Toast(Notice::info(format!(
                "{tag} is available — run `anistream --update`"
            ))));
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
    }
}
