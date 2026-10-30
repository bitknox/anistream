//! The update check: a quiet daily look for a newer release, and one whenever the viewer asks.
//!
//! The check is a request to GitHub's releases API, at most once a day, kept on disk, and for a
//! newer release one more for each app's checksum file. It only ever *says* a version exists, with
//! [`Event::Update`]: what it is, the file each app installs it from and that file's SHA-256.
//! Nothing downloads until the viewer chooses to update: the terminal with `anistream --update`,
//! which finds the release through [`latest_tag`] and [`is_newer`] here, and an app with its own
//! installer.
//!
//! Every push to main is a release, most of them a new patch, so the notice does not name each
//! one as it comes: see [`should_tell`]. Settings offers the newest whenever there is one.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anistream_model::{
    Msg,
    event::Event,
    notice::Notice,
    updates::{Release, ReleaseAsset, UpdateStatus},
};
use anistream_net::HttpClient;
use serde_json::{Value, json};

use crate::outbox::Outbox;

/// The repository whose releases the check looks for and the updater installs from.
pub const REPO: &str = "bitknox/anistream";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// GitHub's API, which the tests stand in for.
const API: &str = "https://api.github.com";

/// `v0.2.0` → `[0, 2, 0]`. Anything unparseable compares as nothing at all, a pre-release's tag
/// such as `v2.1.0-rc.1` among it.
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
    let latest = get_json(http, &latest_url(API)).await.ok()?;
    latest.get("tag_name")?.as_str().map(str::to_owned)
}

fn latest_url(api: &str) -> String {
    format!("{api}/repos/{REPO}/releases/latest")
}

/// How long a newer patch waits before it is told again.
const RETELL_SECS: i64 = 7 * 24 * 3600;
/// How long an answer from GitHub is gone by before it is asked again.
const ANSWER_SECS: i64 = 24 * 3600;
/// The most of the release's notes a [`Release`] carries.
const NOTES_MAX: usize = 500;

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

/// What the terminal's notice says, short enough for its status rule. The apps tell of a
/// release with notices of their own, from [`UpdateStatus::Available`].
fn notice_msg(tag: &str) -> Msg {
    Msg::NoticeUpdateAvailable { tag: tag.to_owned() }
}

/// A stamp file's "<unix time> <tag>".
fn read_stamp(path: &Path) -> Option<(i64, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let (at, tag) = text.trim().split_once(' ')?;
    Some((at.parse().ok()?, tag.to_owned()))
}

/// The files the check keeps in the cache directory.
#[derive(Debug, Clone)]
struct Files {
    /// The last answer, [`Answer`], so sessions within the day decide without a request.
    answer: PathBuf,
    /// The last notice, "<unix time> <tag>", so a release told is not told at every launch.
    told: PathBuf,
}

impl Files {
    fn in_dir(cache_dir: &Path) -> Self {
        Self {
            answer: cache_dir.join("update_release.json"),
            told: cache_dir.join("update_notice"),
        }
    }
}

/// What a check found, as it is kept between checks.
#[derive(Debug, Clone, PartialEq)]
struct Answer {
    /// When GitHub was asked, in Unix seconds.
    checked_at: i64,
    /// The version of the build that asked. A new build asks again rather than go by an answer
    /// an older one was given.
    asked_by: String,
    /// GitHub's newest release, the fields read here.
    latest: Value,
    /// The SHA-256 of each app's file of a release newer than the build that asked, by name.
    checksums: BTreeMap<String, String>,
}

impl Answer {
    fn tag(&self) -> &str {
        self.latest.get("tag_name").and_then(Value::as_str).unwrap_or_default()
    }

    /// Whether this can be gone by at `now`, for the build `current`, without asking again.
    fn fresh(&self, current: &str, now: i64) -> bool {
        self.asked_by == current
            && (0..ANSWER_SECS).contains(&now.saturating_sub(self.checked_at))
    }

    fn read(path: &Path) -> Option<Self> {
        let kept: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        let checksums = kept
            .get("checksums")?
            .as_object()?
            .iter()
            .filter_map(|(name, hex)| Some((name.clone(), sha256_of(hex.as_str()?, None)?)))
            .collect();
        Some(Self {
            checked_at: kept.get("checked_at")?.as_i64()?,
            asked_by: kept.get("asked_by")?.as_str()?.to_owned(),
            latest: kept.get("latest")?.clone(),
            checksums,
        })
    }

    /// Kept for the next check. A cache that cannot be written only means asking again.
    fn write(&self, path: &Path) {
        let kept = json!({
            "checked_at": self.checked_at,
            "asked_by": self.asked_by,
            "latest": self.latest,
            "checksums": self.checksums,
        });
        let _ = std::fs::write(path, kept.to_string());
    }

    /// The newer release this answer names for the build `current`, or `None` when it names
    /// none: the same version or an older one, a draft or a pre-release, or a tag that does not
    /// read as a version.
    fn newer(&self, current: &str) -> Option<Release> {
        let latest = &self.latest;
        let flag = |key: &str| latest.get(key).and_then(Value::as_bool).unwrap_or(false);
        if flag("draft") || flag("prerelease") || !is_newer(self.tag(), current) {
            return None;
        }
        let version = self.tag().trim_start_matches('v').to_owned();
        let text = |key: &str| latest.get(key).and_then(Value::as_str).unwrap_or_default();
        let published_at = text("published_at")
            .parse::<jiff::Timestamp>()
            .map(|at| at.as_second())
            .unwrap_or_default();
        let file = |name: String| self.asset(&name);
        Some(Release {
            published_at,
            page_url: match text("html_url") {
                "" => format!("https://github.com/{REPO}/releases/tag/{}", self.tag()),
                url => url.to_owned(),
            },
            notes: short_notes(text("body")),
            macos: file(format!("anistream-{version}-macos.dmg")),
            ios: file(format!("anistream-{version}-ios-unsigned.ipa")),
            android: file(format!("anistream-{version}-android-arm64.apk")),
            android_tv: file(format!("anistream-{version}-android-tv.apk")),
            windows_x64: file(format!("anistream-{version}-windows-x64-setup.exe")),
            windows_arm64: file(format!("anistream-{version}-windows-arm64-setup.exe")),
            linux_x86_64: file(format!("anistream-{version}-linux-x86_64.AppImage")),
            linux_aarch64: file(format!("anistream-{version}-linux-aarch64.AppImage")),
            version,
        })
    }

    /// The release's file `name`, when it has it and a checksum for it was read.
    fn asset(&self, name: &str) -> Option<ReleaseAsset> {
        let found = asset_named(&self.latest, name)?;
        Some(ReleaseAsset {
            name: name.to_owned(),
            url: found.get("browser_download_url")?.as_str()?.to_owned(),
            sha256: self.checksums.get(name)?.clone(),
            size: found.get("size")?.as_u64()?,
        })
    }

    /// What this answer says, for the build `current`. Whether to tell of a newer release is
    /// the caller's to say.
    fn status(&self, current: &str) -> UpdateStatus {
        match self.newer(current) {
            Some(release) => {
                UpdateStatus::Available { release: Box::new(release), notify: false }
            }
            None => UpdateStatus::UpToDate,
        }
    }
}

/// The asset of `latest` named `name`.
fn asset_named<'a>(latest: &'a Value, name: &str) -> Option<&'a Value> {
    latest
        .get("assets")?
        .as_array()?
        .iter()
        .find(|asset| asset.get("name").and_then(Value::as_str) == Some(name))
}

/// What a checksum file says, as 64 lowercase hex digits: `<hex>` or `<hex>  <file>`, as
/// `shasum -a 256` writes it. `None` when it says anything else, or names a file that is not
/// `name`.
fn sha256_of(text: &str, name: Option<&str>) -> Option<String> {
    let mut words = text.split_whitespace();
    let hex = words.next()?;
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // `shasum` marks a file read in binary mode with a `*` before its name.
    if let (Some(name), Some(named)) = (name, words.next())
        && named.trim_start_matches('*').rsplit('/').next() != Some(name)
    {
        return None;
    }
    Some(hex.to_ascii_lowercase())
}

/// The first section of a release's notes, as plain text and short: what the release's pull
/// request says, up to the install notes that follow it on every release.
fn short_notes(body: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in body.lines() {
        let line = line.trim_end();
        let bare = line.trim_start();
        // The next section's heading ends the first. A heading's marks go.
        if bare.starts_with("## ") && !lines.is_empty() {
            break;
        }
        let text = if bare.starts_with('#') {
            bare.trim_start_matches('#').trim_start()
        } else {
            line
        };
        let text = plain(text);
        if text.is_empty() && lines.last().is_none_or(String::is_empty) {
            continue;
        }
        lines.push(text);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    let mut out = String::new();
    for line in &lines {
        let next = out.chars().count() + usize::from(!out.is_empty()) + line.chars().count();
        if next > NOTES_MAX {
            if out.is_empty() {
                // One long line: cut it at a word.
                let cut: String = line.chars().take(NOTES_MAX).collect();
                out = cut.rsplit_once(' ').map_or(cut.clone(), |(kept, _)| kept.to_owned());
            }
            out.push('…');
            return out;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

/// A line of Markdown as plain text: links as their words, without emphasis or code marks.
fn plain(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let (before, from) = rest.split_at(open);
        out.push_str(before);
        match from.find("](").and_then(|mid| Some((mid, from[mid..].find(')')? + mid))) {
            Some((mid, close)) => {
                out.push_str(&from[1..mid]);
                rest = &from[close + 1..];
            }
            None => {
                out.push('[');
                rest = &from[1..];
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}

/// The body GitHub answers `url` with, or why there is none, in words Settings can show.
async fn get(http: &HttpClient, url: &str) -> Result<Vec<u8>, String> {
    let response = http
        .plain()
        .get(url)
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| "GitHub could not be reached".to_owned())?;
    if !response.status().is_success() {
        return Err(format!("GitHub answered {}", response.status().as_u16()));
    }
    let body = response.bytes().await.map_err(|_| "GitHub could not be reached".to_owned())?;
    Ok(body.to_vec())
}

async fn get_json(http: &HttpClient, url: &str) -> Result<Value, String> {
    serde_json::from_slice(&get(http, url).await?)
        .map_err(|_| "GitHub's answer could not be read".to_owned())
}

/// Ask GitHub for its newest release, and when it is newer than `current`, the checksum of each
/// app's file. A checksum file that is missing or does not read leaves its file out, and the
/// rest of the answer stands.
async fn ask(http: &HttpClient, api: &str, current: &str, now: i64) -> Result<Answer, String> {
    let full = get_json(http, &latest_url(api)).await?;
    if full.get("tag_name").and_then(Value::as_str).is_none() {
        return Err("GitHub's answer could not be read".to_owned());
    }
    // Only what is read here is kept: the full answer lists every file with its uploader.
    let pick = |key: &str| full.get(key).cloned().unwrap_or(Value::Null);
    let assets: Vec<Value> = full
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|asset| {
            json!({
                "name": asset.get("name"),
                "browser_download_url": asset.get("browser_download_url"),
                "size": asset.get("size"),
            })
        })
        .collect();
    let latest = json!({
        "tag_name": pick("tag_name"),
        "html_url": pick("html_url"),
        "published_at": pick("published_at"),
        "body": pick("body"),
        "draft": pick("draft"),
        "prerelease": pick("prerelease"),
        "assets": assets,
    });
    let mut answer = Answer {
        checked_at: now,
        asked_by: current.to_owned(),
        latest,
        checksums: BTreeMap::new(),
    };

    let Some(release) = answer.newer(current) else { return Ok(answer) };
    let version = release.version;
    for name in [
        format!("anistream-{version}-macos.dmg"),
        format!("anistream-{version}-ios-unsigned.ipa"),
        format!("anistream-{version}-android-arm64.apk"),
        format!("anistream-{version}-android-tv.apk"),
        format!("anistream-{version}-windows-x64-setup.exe"),
        format!("anistream-{version}-windows-arm64-setup.exe"),
        format!("anistream-{version}-linux-x86_64.AppImage"),
        format!("anistream-{version}-linux-aarch64.AppImage"),
    ] {
        let Some(url) = asset_named(&answer.latest, &format!("{name}.sha256"))
            .and_then(|sidecar| sidecar.get("browser_download_url")?.as_str())
        else {
            continue;
        };
        let Ok(body) = get(http, url).await else { continue };
        if let Some(hex) = sha256_of(&String::from_utf8_lossy(&body), Some(&name)) {
            answer.checksums.insert(name, hex);
        }
    }
    Ok(answer)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The daily check. Fire-and-forget from startup; a failure is silence, never a toast —
/// an update notice that cries wolf on every flaky network would be turned off in a week.
///
/// `terminal` is whether the front end is the terminal app, the only one told of a release
/// with a toast: its words say to run `anistream --update`, which an app has no use for.
pub fn spawn_check(
    enabled: bool,
    terminal: bool,
    cache_dir: &Path,
    http: &HttpClient,
    tx: &Outbox,
) {
    if !enabled {
        return;
    }
    let (files, http, tx) = (Files::in_dir(cache_dir), http.clone(), tx.clone());
    tokio::spawn(async move { daily(&http, API, CURRENT, &files, now(), terminal, &tx).await });
}

async fn daily(
    http: &HttpClient,
    api: &str,
    current: &str,
    files: &Files,
    now: i64,
    terminal: bool,
    tx: &Outbox,
) {
    let kept = Answer::read(&files.answer);
    let answer = match kept {
        Some(kept) if kept.fresh(current, now) => kept,
        kept => match ask(http, api, current, now).await {
            Ok(answer) => {
                answer.write(&files.answer);
                answer
            }
            // Offline, a newer release found before is still the newest known; anything else
            // the check cannot vouch for today.
            Err(_) => match kept {
                Some(kept) if kept.newer(current).is_some() => kept,
                _ => return,
            },
        },
    };

    let status = match answer.status(current) {
        UpdateStatus::Available { release, .. } => {
            let tag = answer.tag();
            let last = read_stamp(&files.told);
            let notify = should_tell(
                tag,
                current,
                last.as_ref().map(|(at, tag)| (*at, tag.as_str())),
                now,
            );
            if notify {
                let _ = std::fs::write(&files.told, format!("{now} {tag}"));
                // The terminal's notice. An app shows its own, from the status.
                if terminal {
                    let _ = tx.send(Event::Toast(Notice::info(notice_msg(tag))));
                }
            }
            UpdateStatus::Available { release, notify }
        }
        status => status,
    };
    let _ = tx.send(Event::Update(status));
}

/// [`Command::CheckForUpdates`](anistream_model::command::Command::CheckForUpdates): ask now,
/// whatever the daily check's setting, and keep the answer for the daily check.
pub fn check_now(cache_dir: &Path, http: &HttpClient, tx: &Outbox) {
    let (files, http, tx) = (Files::in_dir(cache_dir), http.clone(), tx.clone());
    tokio::spawn(async move { asked(&http, API, CURRENT, &files, now(), &tx).await });
}

async fn asked(
    http: &HttpClient,
    api: &str,
    current: &str,
    files: &Files,
    now: i64,
    tx: &Outbox,
) {
    let _ = tx.send(Event::Update(UpdateStatus::Checking));
    let status = match ask(http, api, current, now).await {
        Ok(answer) => {
            answer.write(&files.answer);
            let status = answer.status(current);
            // Shown where the viewer asked, so the notice does not tell of it again straight
            // after.
            if matches!(status, UpdateStatus::Available { .. }) {
                let _ = std::fs::write(&files.told, format!("{now} {}", answer.tag()));
            }
            status
        }
        Err(reason) => UpdateStatus::Failed { reason },
    };
    let _ = tx.send(Event::Update(status));
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        net::Ipv4Addr,
        sync::{Arc, Mutex},
    };

    use anistream_core::config::NetworkConfig;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::mpsc,
    };

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
        // Nor does a pre-release's tag.
        assert!(!is_newer("v2.1.0-rc.1", "2.0.0"));
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
    fn the_terminals_notice_names_the_release_and_how_to_take_it() {
        assert_eq!(notice_msg("v9.0.0"), Msg::NoticeUpdateAvailable { tag: "v9.0.0".into() });
        let text = notice_msg("v9.0.0").english();
        assert!(text.contains("v9.0.0") && text.contains("anistream --update"), "{text}");
    }

    #[test]
    fn a_checksum_file_reads_as_shasum_writes_it_and_nothing_else() {
        let hex = "A".repeat(64);
        let lower = "a".repeat(64);
        assert_eq!(sha256_of(&format!("{hex}  x.dmg\n"), Some("x.dmg")), Some(lower.clone()));
        assert_eq!(
            sha256_of(&format!("{hex} *dist/x.dmg"), Some("x.dmg")),
            Some(lower.clone())
        );
        assert_eq!(sha256_of(&hex, Some("x.dmg")), Some(lower));
        // Another file's, a short one, or not hex at all.
        assert_eq!(sha256_of(&format!("{hex}  y.dmg"), Some("x.dmg")), None);
        assert_eq!(sha256_of(&"a".repeat(63), None), None);
        assert_eq!(sha256_of(&"g".repeat(64), None), None);
        assert_eq!(sha256_of("<html>Not Found</html>", None), None);
        assert_eq!(sha256_of("", None), None);
    }

    #[test]
    fn the_notes_are_the_first_section_in_plain_words() {
        let body = "## Faster seeking\r\n\r\nSeeking in the **player** waits less, see \
                    [the docs](https://anistream.tv/docs).\n\n### Details\n- `mpv` keeps \
                    its cache\n\n## Terminal\n\nThe installers are…";
        assert_eq!(
            short_notes(body),
            "Faster seeking\n\nSeeking in the player waits less, see the docs.\n\nDetails\n- \
             mpv keeps its cache"
        );
        assert_eq!(short_notes(""), "");

        let long: String = (0..100).map(|i| format!("Line {i} of the notes.\n")).collect();
        let short = short_notes(&long);
        assert!(short.chars().count() <= NOTES_MAX + 1, "{}", short.len());
        assert!(short.ends_with(".…"), "cut at the end of a line: {short}");
        let one_line = "word ".repeat(200);
        let cut = short_notes(&one_line);
        assert!(cut.ends_with("word…"), "cut at a word: {cut}");
    }

    /// A stand-in for GitHub: its releases API, and the files of one release.
    #[derive(Clone, Default)]
    struct FakeGitHub {
        /// The body for each path, or the status to answer with instead.
        routes: Arc<Mutex<HashMap<String, Result<String, u16>>>>,
        hits: Arc<Mutex<Vec<String>>>,
        base: String,
    }

    impl FakeGitHub {
        async fn start() -> Self {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let fake = Self { base, ..Self::default() };
            let serving = fake.clone();
            tokio::spawn(async move {
                while let Ok((mut socket, _)) = listener.accept().await {
                    let serving = serving.clone();
                    tokio::spawn(async move {
                        let mut buf = vec![0; 8192];
                        let n = socket.read(&mut buf).await.unwrap_or(0);
                        let head = String::from_utf8_lossy(&buf[..n]);
                        let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                        serving.hits.lock().unwrap().push(path.clone());
                        let route = serving.routes.lock().unwrap().get(&path).cloned();
                        let (status, body) = match route {
                            Some(Ok(body)) => (200, body),
                            Some(Err(status)) => (status, String::new()),
                            None => (404, String::new()),
                        };
                        let reply = format!(
                            "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = socket.write_all(reply.as_bytes()).await;
                    });
                }
            });
            fake
        }

        fn route(&self, path: &str, answer: Result<String, u16>) {
            self.routes.lock().unwrap().insert(path.to_owned(), answer);
        }

        fn latest_path() -> String {
            format!("/repos/{REPO}/releases/latest")
        }

        /// Release `tag` as `releases/latest`, with each app's file and a checksum beside it.
        fn release(&self, tag: &str, prerelease: bool) {
            let version = tag.trim_start_matches('v');
            let mut assets = Vec::new();
            for (i, suffix) in [
                "macos.dmg",
                "ios-unsigned.ipa",
                "android-arm64.apk",
                "android-tv.apk",
                "windows-x64-setup.exe",
                "windows-arm64-setup.exe",
                "linux-x86_64.AppImage",
                "linux-aarch64.AppImage",
            ]
            .iter()
            .enumerate()
            {
                let name = format!("anistream-{version}-{suffix}");
                let hex = format!("{i}").repeat(64);
                self.route(&format!("/download/{name}.sha256"), Ok(format!("{hex}  {name}\n")));
                assets.push(json!({
                    "name": name,
                    "size": 1000 + i,
                    "browser_download_url": format!("{}/download/{name}", self.base),
                    "uploader": {"login": "github-actions[bot]"},
                }));
                assets.push(json!({
                    "name": format!("{name}.sha256"),
                    "size": 100,
                    "browser_download_url": format!("{}/download/{name}.sha256", self.base),
                }));
            }
            let latest = json!({
                "tag_name": tag,
                "html_url": format!("https://github.com/{REPO}/releases/tag/{tag}"),
                "published_at": "2026-10-22T10:23:59Z",
                "draft": false,
                "prerelease": prerelease,
                "body": "## Smoother calendar\n\nThe calendar scrolls without dropping frames.\n\n## Terminal\n\nInstallers.",
                "assets": assets,
            });
            self.route(&Self::latest_path(), Ok(latest.to_string()));
        }

        fn api_hits(&self) -> usize {
            self.hits.lock().unwrap().iter().filter(|p| **p == Self::latest_path()).count()
        }
    }

    struct Run {
        github: FakeGitHub,
        files: Files,
        dir: tempfile::TempDir,
        /// A client that resolves no host name: only the stand-in, on an address, answers.
        http: HttpClient,
    }

    impl Run {
        async fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let files = Files::in_dir(dir.path());
            let http = HttpClient::offline(&NetworkConfig {
                timeout_secs: 5,
                ..NetworkConfig::default()
            })
            .unwrap();
            Self { github: FakeGitHub::start().await, files, dir, http }
        }

        /// The terminal's check.
        async fn daily(&self, current: &str, now: i64) -> Vec<Event> {
            self.daily_for(true, current, now).await
        }

        async fn daily_for(&self, terminal: bool, current: &str, now: i64) -> Vec<Event> {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let (base, files) = (&self.github.base, &self.files);
            daily(&self.http, base, current, files, now, terminal, &Outbox::from(tx)).await;
            drain(&mut rx)
        }

        async fn asked(&self, current: &str, now: i64) -> Vec<Event> {
            let (tx, mut rx) = mpsc::unbounded_channel();
            asked(&self.http, &self.github.base, current, &self.files, now, &Outbox::from(tx))
                .await;
            drain(&mut rx)
        }
    }

    fn drain(rx: &mut mpsc::UnboundedReceiver<Event>) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// The status among `events`, and whether a notice of the terminal's came with it.
    fn status(events: &[Event]) -> (Option<UpdateStatus>, bool) {
        let mut found = None;
        let mut toast = false;
        for event in events {
            match event {
                Event::Update(status) => found = Some(status.clone()),
                Event::Toast(_) => toast = true,
                other => panic!("the check said {other:?}"),
            }
        }
        (found, toast)
    }

    fn available(status: Option<UpdateStatus>) -> (Release, bool) {
        match status {
            Some(UpdateStatus::Available { release, notify }) => (*release, notify),
            other => panic!("expected a release, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_newer_release_comes_with_each_apps_file_and_its_checksum() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let events = run.daily("2.0.2", NOW).await;
        let (status, toast) = status(&events);
        let (release, notify) = available(status);
        assert!(notify, "told the first time it is found");
        assert!(toast, "the terminal's notice");
        assert_eq!(release.version, "2.0.3");
        assert_eq!(release.published_at, 1_792_664_639);
        assert_eq!(release.page_url, format!("https://github.com/{REPO}/releases/tag/v2.0.3"));
        assert_eq!(
            release.notes,
            "Smoother calendar\n\nThe calendar scrolls without dropping frames."
        );
        let mac = release.macos.expect("the Mac's file");
        assert_eq!(mac.name, "anistream-2.0.3-macos.dmg");
        assert_eq!(mac.url, format!("{}/download/anistream-2.0.3-macos.dmg", run.github.base));
        assert_eq!(mac.sha256, "0".repeat(64));
        assert_eq!(mac.size, 1000);
        assert_eq!(release.ios.expect("the iPhone's file").sha256, "1".repeat(64));
        let android = release.android.expect("Android's file");
        assert_eq!(android.name, "anistream-2.0.3-android-arm64.apk");
        assert_eq!((android.sha256, android.size), ("2".repeat(64), 1002));
        let tv = release.android_tv.expect("the TV's file");
        assert_eq!(tv.name, "anistream-2.0.3-android-tv.apk");
        assert_eq!((tv.sha256, tv.size), ("3".repeat(64), 1003));
        let windows = release.windows_x64.expect("Windows' x64 installer");
        assert_eq!(windows.name, "anistream-2.0.3-windows-x64-setup.exe");
        assert_eq!((windows.sha256, windows.size), ("4".repeat(64), 1004));
        let windows = release.windows_arm64.expect("Windows' Arm64 installer");
        assert_eq!(windows.name, "anistream-2.0.3-windows-arm64-setup.exe");
        assert_eq!((windows.sha256, windows.size), ("5".repeat(64), 1005));
        let linux = release.linux_x86_64.expect("Linux's x86_64 AppImage");
        assert_eq!(linux.name, "anistream-2.0.3-linux-x86_64.AppImage");
        assert_eq!((linux.sha256, linux.size), ("6".repeat(64), 1006));
        let linux = release.linux_aarch64.expect("Linux's aarch64 AppImage");
        assert_eq!(linux.name, "anistream-2.0.3-linux-aarch64.AppImage");
        assert_eq!((linux.sha256, linux.size), ("7".repeat(64), 1007));
    }

    #[tokio::test]
    async fn an_app_is_told_of_a_release_by_its_status_and_never_by_the_terminals_notice() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let (status, toast) = status(&run.daily_for(false, "2.0.2", NOW).await);
        let (release, notify) = available(status);
        assert_eq!((release.version.as_str(), notify), ("2.0.3", true));
        assert!(!toast, "the notice says to run a terminal command");
    }

    #[tokio::test]
    async fn the_same_version_is_up_to_date_and_fetches_no_checksums() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let (status_now, toast) = status(&run.daily("2.0.3", NOW).await);
        assert_eq!(status_now, Some(UpdateStatus::UpToDate));
        assert!(!toast);
        assert_eq!(run.github.hits.lock().unwrap().len(), 1, "only the API was asked");
        // An older release, as when a build of main is ahead of the newest, is no update either.
        let (status_now, _) = status(&run.asked("2.0.4", NOW).await[1..]);
        assert_eq!(status_now, Some(UpdateStatus::UpToDate));
    }

    #[tokio::test]
    async fn a_pre_release_or_a_draft_is_never_an_update() {
        let run = Run::new().await;
        run.github.release("v2.1.0", true);
        assert_eq!(status(&run.daily("2.0.3", NOW).await).0, Some(UpdateStatus::UpToDate));
        // Nor a tag a pre-release is named by.
        run.github.release("v2.1.0-rc.1", false);
        assert_eq!(status(&run.asked("2.0.3", NOW).await[1..]).0, Some(UpdateStatus::UpToDate));
    }

    #[tokio::test]
    async fn a_bad_checksum_file_leaves_its_file_out_and_the_rest_stands() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let mac = "/download/anistream-2.0.3-macos.dmg.sha256";
        let ios = "/download/anistream-2.0.3-ios-unsigned.ipa.sha256";
        run.github.route(mac, Ok("<html>a sign-in page</html>".into()));
        run.github.route(ios, Err(500));
        let (release, _) = available(status(&run.daily("2.0.2", NOW).await).0);
        assert_eq!(release.macos, None, "a checksum that does not read is no checksum");
        assert_eq!(release.ios, None, "nor is one that could not be fetched");
        assert!(release.android.is_some(), "Android's file stands");
        assert_eq!(release.version, "2.0.3");
    }

    #[tokio::test]
    async fn a_checksum_for_another_file_does_not_count() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let apk = "/download/anistream-2.0.3-android-arm64.apk.sha256";
        let other = format!("{}  anistream-2.0.2-android-arm64.apk", "a".repeat(64));
        run.github.route(apk, Ok(other));
        let (release, _) = available(status(&run.daily("2.0.2", NOW).await).0);
        assert_eq!(release.android, None);
    }

    #[tokio::test]
    async fn without_a_network_the_daily_check_is_silent_and_check_now_says_so() {
        let run = Run::new().await;
        // GitHub's own address, which the test's client cannot resolve.
        let (tx, mut rx) = mpsc::unbounded_channel();
        daily(&run.http, API, "2.0.2", &run.files, NOW, true, &Outbox::from(tx.clone())).await;
        assert!(drain(&mut rx).is_empty(), "a failed daily check says nothing");

        asked(&run.http, API, "2.0.2", &run.files, NOW, &Outbox::from(tx)).await;
        let events = drain(&mut rx);
        assert_eq!(events.len(), 2, "{events:?}");
        assert!(matches!(events[0], Event::Update(UpdateStatus::Checking)), "{events:?}");
        assert!(
            matches!(&events[1], Event::Update(UpdateStatus::Failed { reason })
                if reason == "GitHub could not be reached"),
            "{events:?}"
        );

        // GitHub up but refusing, as at its rate limit.
        run.github.route(&FakeGitHub::latest_path(), Err(403));
        let events = run.asked("2.0.2", NOW).await;
        assert!(
            matches!(&events[1], Event::Update(UpdateStatus::Failed { reason })
                if reason == "GitHub answered 403"),
            "{events:?}"
        );
        // Or answering with something that is not a release.
        run.github.route(&FakeGitHub::latest_path(), Ok("<html></html>".into()));
        let events = run.asked("2.0.2", NOW).await;
        assert!(
            matches!(&events[1], Event::Update(UpdateStatus::Failed { reason })
                if reason == "GitHub's answer could not be read"),
            "{events:?}"
        );
    }

    #[tokio::test]
    async fn the_daily_check_asks_once_a_day_and_offline_keeps_the_release_it_found() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let _ = run.daily("2.0.2", NOW).await;
        // Within the day the kept answer decides, with no request, and the notice is not told
        // twice.
        let (release, notify) = available(status(&run.daily("2.0.2", NOW + 3600).await).0);
        assert_eq!(run.github.api_hits(), 1);
        assert!(!notify, "told once");
        assert!(release.macos.is_some(), "the kept answer has the checksums");

        // A day on it asks again; when GitHub cannot be reached, the release found before is
        // still offered.
        run.github.route(&FakeGitHub::latest_path(), Err(502));
        let (release, notify) = available(status(&run.daily("2.0.2", NOW + DAY + 1).await).0);
        assert_eq!(run.github.api_hits(), 2);
        assert_eq!((release.version.as_str(), notify), ("2.0.3", false));

        // Once the build is that release, a new build asks GitHub rather than going by what the
        // old one was told, and it is up to date.
        run.github.release("v2.0.3", false);
        let (now_says, _) = status(&run.daily("2.0.3", NOW + DAY + 2).await);
        assert_eq!(now_says, Some(UpdateStatus::UpToDate));
        assert_eq!(run.github.api_hits(), 3);
    }

    #[tokio::test]
    async fn notices_follow_the_rules_while_settings_always_has_the_release() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        assert!(available(status(&run.daily("2.0.2", NOW).await).0).1);
        // The next patch, a day later: offered, not told.
        run.github.release("v2.0.4", false);
        let (release, notify) = available(status(&run.daily("2.0.2", NOW + DAY + 1).await).0);
        assert_eq!((release.version.as_str(), notify), ("2.0.4", false));
        // A week on, told.
        run.github.release("v2.0.5", false);
        assert!(available(status(&run.daily("2.0.2", NOW + 8 * DAY).await).0).1);
        // A new minor, the next day: told at once.
        run.github.release("v2.1.0", false);
        assert!(available(status(&run.daily("2.0.2", NOW + 9 * DAY + 1).await).0).1);
    }

    #[tokio::test]
    async fn check_now_asks_whatever_was_kept_and_counts_as_told() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let _ = run.daily("2.0.2", NOW).await;
        run.github.release("v3.0.0", false);

        let events = run.asked("2.0.2", NOW + 60).await;
        assert!(matches!(events[0], Event::Update(UpdateStatus::Checking)), "{events:?}");
        let (release, notify) = available(status(&events[1..]).0);
        assert_eq!(run.github.api_hits(), 2, "asked, though the day's answer was kept");
        assert_eq!(release.version, "3.0.0");
        assert!(!notify, "shown in Settings, where it was asked for");

        // The launch after says nothing new of it: the new major was seen.
        let (release, notify) = available(status(&run.daily("2.0.2", NOW + 120).await).0);
        assert_eq!((release.version.as_str(), notify), ("3.0.0", false));
        assert_eq!(run.github.api_hits(), 2, "and goes by what check now kept");
    }

    #[tokio::test]
    async fn the_daily_check_is_off_when_switched_off() {
        let run = Run::new().await;
        run.github.release("v2.0.3", false);
        let (tx, mut rx) = mpsc::unbounded_channel();
        spawn_check(false, true, run.dir.path(), &run.http, &Outbox::from(tx));
        tokio::task::yield_now().await;
        assert!(drain(&mut rx).is_empty());
        assert_eq!(run.github.hits.lock().unwrap().len(), 0);
    }
}
