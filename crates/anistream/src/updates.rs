//! The update loop: a quiet daily check, and `--update` to take the new version.
//!
//! Two deliberate restraints. The check is a single HTTPS request to GitHub's releases
//! API, at most once a day, cached on disk, and it only ever *says* a version exists —
//! nothing downloads without `--update` being asked for. And the updater trusts nothing
//! it did not verify: the archive's published SHA-256 is checked before a single byte
//! replaces the running binary.

use std::path::{Path, PathBuf};

use anistream_net::HttpClient;
use anistream_ui::{Toast, Update};
use anyhow::{Context, Result, bail};
use sha2::Digest;
use tokio::sync::mpsc;

const REPO: &str = "bitknox/anistream";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// The build's release-asset target triple. Matched to the names `release.yml` publishes.
const TARGET: &str = {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
};

/// `v0.2.0` → `[0, 2, 0]`. Anything unparseable compares as nothing at all.
fn parse_semver(tag: &str) -> Option<[u32; 3]> {
    let mut parts = tag.trim().trim_start_matches('v').splitn(3, '.');
    let mut out = [0u32; 3];
    for slot in &mut out {
        *slot = parts.next()?.trim().parse().ok()?;
    }
    Some(out)
}

fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_semver(candidate), parse_semver(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// Ask GitHub for the newest release tag.
async fn latest_tag(http: &HttpClient) -> Option<String> {
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
pub fn spawn_check(
    enabled: bool,
    cache_dir: &Path,
    http: &HttpClient,
    tx: &mpsc::UnboundedSender<Update>,
) {
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
            let _ = tx.send(Update::Toast(Toast::info(format!(
                "{tag} is available — run `anistream --update`"
            ))));
        }
    });
}

async fn download(http: &HttpClient, url: &str) -> Result<Vec<u8>> {
    let response =
        http.plain().get(url).send().await.with_context(|| format!("fetching {url}"))?;
    if !response.status().is_success() {
        bail!("{url} answered HTTP {}", response.status());
    }
    Ok(response.bytes().await.context("reading download")?.to_vec())
}

/// Replace the running binary with the freshly verified one.
///
/// Unix renames over the running file, which is atomic on the same filesystem. Windows
/// refuses to overwrite a running executable but happily lets it be *renamed*, so the
/// old binary steps aside as `.old` first and is swept up on the next update.
fn install(new_binary: &Path) -> Result<PathBuf> {
    let current = std::env::current_exe().context("locating the running binary")?;
    let staged = current.with_extension("new");
    std::fs::copy(new_binary, &staged).context("staging the new binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .context("marking the new binary executable")?;
        std::fs::rename(&staged, &current).context("replacing the binary")?;
    }
    #[cfg(windows)]
    {
        let old = current.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&current, &old).context("setting the old binary aside")?;
        std::fs::rename(&staged, &current).context("moving the new binary into place")?;
    }
    Ok(current)
}

/// `anistream --update`: fetch, verify, replace.
pub async fn self_update(http: &HttpClient) -> Result<()> {
    println!("current version  v{CURRENT}");
    let tag = latest_tag(http).await.context("could not reach GitHub's releases API")?;
    println!("latest release   {tag}");
    if !is_newer(&tag, CURRENT) {
        println!("already up to date");
        return Ok(());
    }

    let version = tag.trim_start_matches('v');
    let name = format!("anistream-{version}-{TARGET}");
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let base = format!("https://github.com/{REPO}/releases/download/{tag}/{name}.{ext}");

    println!("downloading      {name}.{ext}");
    let archive = download(http, &base).await?;
    let published = download(http, &format!("{base}.sha256")).await?;

    // The sidecar is `<hex>  <filename>`; only the hex matters here.
    let expected = String::from_utf8_lossy(&published)
        .split_whitespace()
        .next()
        .map(str::to_lowercase)
        .context("empty checksum file")?;
    let actual = format!("{:x}", sha2::Sha256::digest(&archive));
    if actual != expected {
        bail!("checksum mismatch — expected {expected}, got {actual}; not installing");
    }
    println!("checksum         ok");

    // Native tar everywhere: GNU tar for .tar.gz on Linux, bsdtar for both formats on
    // macOS and Windows (shipped since Windows 10).
    let staging = std::env::temp_dir().join(format!("anistream-update-{version}"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).context("creating a staging directory")?;
    let archive_path = staging.join(format!("{name}.{ext}"));
    std::fs::write(&archive_path, &archive).context("writing the archive")?;

    let status = std::process::Command::new("tar")
        .arg("-xf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&staging)
        .status()
        .context("running tar — is it on PATH?")?;
    if !status.success() {
        bail!("tar exited with {status}");
    }

    let binary_name = if cfg!(windows) { "anistream.exe" } else { "anistream" };
    let new_binary = staging.join(&name).join(binary_name);
    if !new_binary.exists() {
        bail!("the archive did not contain {}", new_binary.display());
    }

    let installed = install(&new_binary)?;
    let _ = std::fs::remove_dir_all(&staging);
    println!("installed        {tag} → {}", installed.display());
    println!("restart anistream to use it");
    Ok(())
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

    #[test]
    fn the_build_target_matches_a_published_asset_name() {
        // The four targets release.yml builds. A typo here means `--update` 404s.
        assert!(
            [
                "aarch64-apple-darwin",
                "x86_64-apple-darwin",
                "x86_64-unknown-linux-gnu",
                "x86_64-pc-windows-msvc",
            ]
            .contains(&TARGET)
        );
    }
}
