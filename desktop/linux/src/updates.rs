//! Updates. Outside Flatpak the engine checks GitHub's latest release once a day and when asked.
//! GitHub's latest release is never a pre-release, so the `deps-libmpv` builds are never offered.
//!
//! Run from an AppImage, the app updates itself: a newer release names an AppImage for this
//! machine with its size and checksum, the app downloads it into its cache and checks it, and
//! Restart to Update puts it in place of the running AppImage, under the same name, then opens
//! it once this copy has quit. An AppImage in a folder the app can't write to, in /opt or on a
//! read-only mount, can't be replaced: the checked download is saved to Downloads instead, for
//! the viewer to put in its place. Built any other way, the check says a version is out and offers
//! its page. Inside Flatpak, Flatpak updates the app, from Software or `flatpak update`: the
//! check still says a version is out, and offers nothing to install.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anistream_i18n::t;
use anistream_model::updates::{Release, ReleaseAsset, UpdateStatus};
use sha2::{Digest, Sha256};

/// Whether this copy runs inside Flatpak, which then owns its updates.
pub fn flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Where the check stands, as the Updates row says it under "anistream 1.0".
pub fn words(status: Option<&UpdateStatus>, offline: bool, flatpak: bool) -> String {
    match status {
        Some(UpdateStatus::Checking) => t::updates_checking(),
        Some(UpdateStatus::UpToDate { .. }) => t::updates_up_to_date(),
        Some(UpdateStatus::Available { release, .. }) if flatpak => {
            t::updates_version_out_software(&release.version)
        }
        Some(UpdateStatus::Available { release, .. }) => {
            t::updates_version_out(&release.version)
        }
        // The engine gives the reason in English words, with no message id to translate yet.
        Some(UpdateStatus::Failed { reason }) => t::updates_check_failed(reason),
        None if flatpak => t::updates_flatpak(),
        None if offline => t::updates_checks_online(),
        None => t::updates_not_checked(),
    }
}

/// The AppImage this copy runs from, when it runs from one: the path the AppImage's runtime
/// gives it, which is the file an update replaces.
pub fn appimage() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("APPIMAGE")?);
    (path.is_absolute() && path.is_file() && !flatpak()).then_some(path)
}

/// The AppImage in `release` for the machine `arch`, as Rust names it: x86_64's or aarch64's.
pub fn appimage_for<'a>(release: &'a Release, arch: &str) -> Option<&'a ReleaseAsset> {
    match arch {
        "x86_64" => release.linux_x86_64.as_ref(),
        "aarch64" => release.linux_aarch64.as_ref(),
        _ => None,
    }
}

/// Where an update stands, once the check has found a release this copy can install.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Step {
    /// Nothing under way.
    #[default]
    Idle,
    /// The AppImage is downloading: so many bytes of so many.
    Downloading { received: u64, expected: u64 },
    /// Its size and checksum are being checked.
    Verifying,
    /// Downloaded and checked, at this path: Restart to Update puts it in place.
    Ready(PathBuf),
    /// Downloaded and checked, and saved at this path in Downloads, since the AppImage's own
    /// folder can't be written to: the viewer replaces the old file with it.
    Saved(PathBuf),
    /// It is going in place, and the app quits to open it.
    Installing,
    /// It stopped, for this reason, said as a sentence.
    Failed(String),
}

/// An update's step, for the version it is for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Progress {
    pub version: String,
    pub step: Step,
}

impl Progress {
    /// How far along, from nought to one, while that is known.
    pub fn fraction(&self) -> Option<f64> {
        match self.step {
            Step::Downloading { received, expected } if expected > 0 => {
                Some((received as f64 / expected as f64).min(1.0))
            }
            Step::Verifying | Step::Ready(_) | Step::Saved(_) | Step::Installing => Some(1.0),
            _ => None,
        }
    }
}

/// The line under the version, for the check and the update together. `installs` is whether
/// this copy installs the release itself.
pub fn line(
    status: Option<&UpdateStatus>,
    progress: &Progress,
    offline: bool,
    flatpak: bool,
    installs: bool,
) -> String {
    let version = &progress.version;
    match &progress.step {
        Step::Downloading { received, expected } if *expected > 0 => {
            t::updates_downloading_size(version, &size(*received), &size(*expected))
        }
        Step::Downloading { .. } => t::updates_downloading(version),
        Step::Verifying => t::updates_checking_download(version),
        Step::Ready(_) => t::updates_ready(version),
        Step::Saved(_) => saved_words(version, appimage().as_deref()),
        Step::Installing => t::updates_installing(version),
        Step::Failed(reason) => t::updates_stopped(version, reason),
        Step::Idle => match status {
            Some(UpdateStatus::Available { release, .. }) if !flatpak && !installs => {
                t::updates_version_out_appimage(&release.version)
            }
            _ => words(status, offline, flatpak),
        },
    }
}

/// What the Updates row says of a new version saved to Downloads: what to do with it, and where
/// the file it replaces is, when this copy runs from one.
pub fn saved_words(version: &str, old: Option<&Path>) -> String {
    match old {
        Some(old) => t::updates_saved_replace(version, &home_short(old)),
        None => t::updates_saved_replace_plain(version),
    }
}

/// The notice for a new version saved to Downloads: short, the steps are in the Updates row.
pub fn saved_notice(version: &str) -> String {
    t::updates_saved(version)
}

/// A path with the home folder as `~`, as a file manager's path bar reads it.
fn home_short(path: &Path) -> String {
    let home = gtk::glib::home_dir();
    match path.strip_prefix(&home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

/// Whether the app can write in `dir`: what replacing the AppImage there needs. Tried with a
/// file of its own, so a read-only mount answers as truly as the folder's permissions.
pub fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".anistream-update-check-{}", std::process::id()));
    let made = std::fs::OpenOptions::new().write(true).create_new(true).open(&probe);
    let _ = std::fs::remove_file(&probe);
    made.is_ok()
}

/// The viewer's Downloads folder.
pub fn downloads_dir() -> PathBuf {
    gtk::glib::user_special_dir(gtk::glib::UserDirectory::Downloads)
        .unwrap_or_else(|| gtk::glib::home_dir().join("Downloads")) // i18n-skip
}

/// Save the checked download at `ready` into `dir` under its own name, able to run, and say
/// where it went.
pub fn save_to(ready: &Path, dir: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let name = ready.file_name().ok_or_else(t::updates_why_download)?;
    let path = dir.join(name);
    let failed = |e: std::io::Error| {
        tracing::warn!(error = %e, dir = %dir.display(), "the new version was not saved");
        t::updates_why_save(&e.to_string())
    };
    std::fs::create_dir_all(dir).map_err(failed)?;
    std::fs::copy(ready, &path).map_err(failed)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).map_err(failed)?;
    Ok(path)
}

/// A size in megabytes, or kilobytes under one: "34 MB".
pub fn size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{} MB", (bytes as f64 / 1_000_000.0).round())
    } else {
        format!("{} KB", (bytes / 1000).max(1))
    }
}

/// What is wrong with the file at `path` for `asset`, or nothing when it matches.
pub fn check(path: &Path, asset: &ReleaseAsset) -> Result<(), String> {
    let unread = |e: std::io::Error| t::updates_why_read(&e.to_string());
    let saved = std::fs::metadata(path).map_err(unread)?;
    if saved.len() != asset.size {
        return Err(t::updates_why_size(&size(saved.len()), &size(asset.size)));
    }
    let mut file = std::fs::File::open(path).map_err(unread)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file.read(&mut buffer).map_err(unread)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if hex == asset.sha256.to_ascii_lowercase() {
        Ok(())
    } else {
        Err(t::updates_why_checksum())
    }
}

/// Download `asset` into `dir` and check it, saying how far it has come each tenth of a second
/// or so. A file already there and whole is kept; one that does not match is deleted. Stops,
/// with `None`, once `stopped` says so.
pub async fn fetch(
    asset: &ReleaseAsset,
    dir: &Path,
    progress: impl Fn(Step),
    stopped: impl Fn() -> bool,
) -> Option<Result<PathBuf, String>> {
    let path = dir.join(&asset.name);
    let saved = |e: std::io::Error| t::updates_why_save(&e.to_string());
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Some(Err(saved(e)));
    }
    if path.is_file() && check(&path, asset).is_ok() {
        return Some(Ok(path));
    }
    // No limit on the whole: a large file on a slow line takes as long as it takes. A server
    // that stops sending is given up on all the same.
    let client = match reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60))
        .user_agent(concat!("anistream-desktop/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            tracing::warn!(error = %e, "the download client did not build");
            return Some(Err(t::updates_why_start()));
        }
    };
    let lost = |_| t::updates_why_connection();
    let mut response = match client.get(&asset.url).send().await {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            let code = response.status().as_u16();
            return Some(Err(t::updates_why_server(i64::from(code))));
        }
        Err(e) => return Some(Err(lost(e))),
    };
    let partial = dir.join(format!("{}.partial", asset.name));
    let mut file = match std::fs::File::create(&partial) {
        Ok(file) => file,
        Err(e) => return Some(Err(saved(e))),
    };
    let mut received = 0u64;
    let mut told = std::time::Instant::now();
    loop {
        if stopped() {
            let _ = std::fs::remove_file(&partial);
            return None;
        }
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => return Some(Err(lost(e))),
        };
        if let Err(e) = file.write_all(&chunk) {
            return Some(Err(saved(e)));
        }
        received += chunk.len() as u64;
        // More than the release lists is wrong already: the check says how.
        if received > asset.size {
            break;
        }
        if told.elapsed().as_millis() >= 100 {
            told = std::time::Instant::now();
            progress(Step::Downloading { received, expected: asset.size });
        }
    }
    drop(file);
    progress(Step::Verifying);
    if let Err(e) = std::fs::rename(&partial, &path) {
        return Some(Err(saved(e)));
    }
    if let Err(wrong) = check(&path, asset) {
        let _ = std::fs::remove_file(&path);
        return Some(Err(wrong));
    }
    Some(Ok(path))
}

/// Put the checked download at `ready` in place of the AppImage at `appimage`, under its name and
/// able to run. Copied beside it first and renamed over it, so the AppImage is either the old one
/// or the new one whole, and the copy running goes on running the old one until it quits.
pub fn install(ready: &Path, appimage: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let dir = appimage.parent().ok_or_else(t::updates_why_install)?;
    let name = appimage.file_name().ok_or_else(t::updates_why_install)?;
    let beside = dir.join(format!(".{}.update", name.to_string_lossy()));
    let wrote = |e: std::io::Error| {
        tracing::warn!(error = %e, dir = %dir.display(), "the new version was not written");
        t::updates_why_save(&e.to_string())
    };
    std::fs::copy(ready, &beside).map_err(wrote)?;
    let placed = std::fs::set_permissions(&beside, std::fs::Permissions::from_mode(0o755))
        .and_then(|()| std::fs::rename(&beside, appimage));
    if let Err(e) = placed {
        let _ = std::fs::remove_file(&beside);
        return Err(wrote(e));
    }
    Ok(())
}

/// The script that opens the new version: it waits for the copy that started it to quit, at
/// most half a minute, and then runs what it is given.
pub const RELAUNCH: &str = "pid=$1; shift; i=0; while kill -0 \"$pid\" 2>/dev/null && [ $i -lt 150 ]; do sleep 0.2; i=$((i+1)); done; exec \"$@\""; // i18n-skip

/// The arguments the app opens again with after an update: the ones it started with, less the
/// links it was opened by.
pub fn relaunch_args(started: impl IntoIterator<Item = String>) -> Vec<String> {
    started.into_iter().filter(|arg| !arg.starts_with("anistream:")).collect()
}

/// The environment the AppImage's runtime and its AppRun set for this copy, which point into
/// its mount and are set again for the new one: left out of what opens it.
pub const MOUNT_VARS: [&str; 8] = [
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "GSETTINGS_SCHEMA_DIR",
    "GIO_EXTRA_MODULES",
    "GDK_PIXBUF_MODULE_FILE",
    "ANISTREAM_FONTS",
];

/// `XDG_DATA_DIRS` without the entries under `appdir`, which AppRun put in front.
pub fn data_dirs_without(dirs: &str, appdir: &str) -> String {
    dirs.split(':')
        .filter(|dir| !dir.is_empty() && !dir.starts_with(appdir))
        .collect::<Vec<_>>()
        .join(":")
}

/// Whether the version `a` is older than `b`, number by number: 2.0.10 is newer than 2.0.9.
pub fn older(a: &str, b: &str) -> bool {
    let parts =
        |v: &str| -> Vec<u64> { v.split(['.', '-']).map(|p| p.parse().unwrap_or(0)).collect() };
    let (x, y) = (parts(a), parts(b));
    for i in 0..x.len().max(y.len()) {
        let (p, q) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if p != q {
            return p < q;
        }
    }
    false
}

/// The release page to open, while a newer version is out.
pub fn page(status: Option<&UpdateStatus>) -> Option<&str> {
    match status {
        Some(UpdateStatus::Available { release, .. }) => Some(&release.page_url),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use anistream_model::updates::Release;

    fn available() -> UpdateStatus {
        UpdateStatus::Available {
            release: Box::new(Release {
                version: "1.4.0".into(),
                published_at: 0,
                page_url: "https://example.test/releases/v1.4.0".into(),
                notes: String::new(),
                macos: None,
                ios: None,
                android: None,
                android_tv: None,
                windows_x64: None,
                windows_arm64: None,
                linux_x86_64: None,
                linux_aarch64: None,
                beta: false,
            }),
            notify: true,
        }
    }

    #[test]
    fn a_newer_version_is_said_with_its_page_and_flatpak_says_who_installs_it() {
        let status = available();
        assert_eq!(words(Some(&status), false, false), "Version 1.4.0 is out");
        assert_eq!(
            words(Some(&status), false, true),
            "Version 1.4.0 is out. Software installs it."
        );
        assert_eq!(page(Some(&status)), Some("https://example.test/releases/v1.4.0"));
        assert_eq!(
            page(Some(&UpdateStatus::UpToDate { stable_behind: None, running_beta: false })),
            None
        );
    }

    #[test]
    fn before_a_check_the_row_says_why_there_is_none() {
        assert_eq!(words(None, true, false), "Checks once you're back online");
        assert_eq!(words(None, false, true), "Flatpak keeps anistream up to date");
        let failed = UpdateStatus::Failed { reason: "no answer".into() };
        assert_eq!(words(Some(&failed), false, false), "Couldn't check: no answer");
    }

    fn asset(bytes: &[u8]) -> ReleaseAsset {
        let hex: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02X}")).collect();
        ReleaseAsset {
            name: "anistream-1.4.0-linux-x86_64.AppImage".into(),
            url: "https://releases.example/anistream-1.4.0-linux-x86_64.AppImage".into(),
            sha256: hex,
            size: bytes.len() as u64,
        }
    }

    #[test]
    fn each_machine_takes_its_own_appimage() {
        let UpdateStatus::Available { mut release, .. } = available() else { unreachable!() };
        assert_eq!(appimage_for(&release, "x86_64"), None);
        release.linux_x86_64 = Some(asset(b"x"));
        assert!(appimage_for(&release, "x86_64").is_some());
        assert_eq!(appimage_for(&release, "aarch64"), None);
        assert_eq!(appimage_for(&release, "riscv64"), None);
    }

    #[test]
    fn the_line_follows_the_update_through_its_steps() {
        let status = available();
        let at = |step| Progress { version: "1.4.0".into(), step };
        let line = |step| line(Some(&status), &at(step), false, false, true);
        assert_eq!(line(Step::Idle), "Version 1.4.0 is out");
        assert_eq!(
            line(Step::Downloading { received: 34_200_000, expected: 118_400_000 }),
            "Downloading version 1.4.0 · 34 MB of 118 MB"
        );
        assert_eq!(line(Step::Verifying), "Checking the download of version 1.4.0…");
        assert_eq!(line(Step::Ready("/x".into())), "Version 1.4.0 is ready to install");
        assert_eq!(line(Step::Installing), "Installing version 1.4.0…");
        assert_eq!(
            line(Step::Saved("/x".into())),
            "Version 1.4.0 saved to Downloads. Replace the old file with it to update."
        );
        assert_eq!(
            saved_words("1.4.0", Some(&gtk::glib::home_dir().join("Apps/anistream.AppImage"))),
            "Version 1.4.0 saved to Downloads. Replace the old file, ~/Apps/anistream.AppImage, \
             with it to update."
        );
        assert_eq!(saved_notice("1.4.0"), "Version 1.4.0 saved to Downloads");
        assert_eq!(
            line(Step::Failed("The download stopped: the connection was lost.".into())),
            "The update to version 1.4.0 stopped. The download stopped: the connection was lost."
        );
        // Built any other way, the page has the file.
        assert_eq!(
            super::line(Some(&status), &Progress::default(), false, false, false),
            "Version 1.4.0 is out. Its page has the AppImage."
        );
        assert_eq!(
            at(Step::Downloading { received: 50, expected: 200 }).fraction(),
            Some(0.25)
        );
        assert_eq!(at(Step::Downloading { received: 0, expected: 0 }).fraction(), None);
        assert_eq!(size(900), "1 KB");
    }

    #[test]
    fn a_download_is_checked_by_its_size_and_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.AppImage");
        std::fs::write(&path, b"the new version").unwrap();
        assert_eq!(check(&path, &asset(b"the new version")), Ok(()));
        let longer = ReleaseAsset { size: 3_000_000, ..asset(b"the new version") };
        assert_eq!(
            check(&path, &longer),
            Err("The download is 1 KB, not the 3 MB listed for this version.".into())
        );
        let other = asset(b"the old version");
        assert_eq!(
            check(&path, &other),
            Err("The download doesn't match the checksum listed for this version.".into())
        );
    }

    #[test]
    fn the_new_version_goes_in_place_under_the_same_name_and_runs() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (ready, appimage) =
            (dir.path().join("ready"), dir.path().join("anistream.AppImage"));
        std::fs::write(&ready, b"new").unwrap();
        std::fs::write(&appimage, b"old").unwrap();
        install(&ready, &appimage).unwrap();
        assert_eq!(std::fs::read(&appimage).unwrap(), b"new");
        assert_eq!(std::fs::metadata(&appimage).unwrap().permissions().mode() & 0o777, 0o755);
        // Nothing left beside it.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);

        let missing = dir.path().join("gone").join("anistream.AppImage");
        assert!(install(&ready, &missing).is_err());
    }

    #[test]
    fn a_folder_that_cannot_be_written_to_gets_the_new_version_in_downloads() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (opt, downloads) = (dir.path().join("opt"), dir.path().join("Downloads"));
        std::fs::create_dir(&opt).unwrap();
        assert!(writable(&opt));
        assert_eq!(std::fs::read_dir(&opt).unwrap().count(), 0, "the check leaves nothing");
        std::fs::set_permissions(&opt, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root writes anywhere, so only an ordinary user can see the folder refuse.
        let root = std::fs::write(opt.join("x"), b"").is_ok();
        if !root {
            assert!(!writable(&opt));
        }
        std::fs::set_permissions(&opt, std::fs::Permissions::from_mode(0o755)).unwrap();

        let ready = dir.path().join("anistream-2.1.0-linux-x86_64.AppImage");
        std::fs::write(&ready, b"new").unwrap();
        let saved = save_to(&ready, &downloads).unwrap();
        assert_eq!(saved, downloads.join("anistream-2.1.0-linux-x86_64.AppImage"));
        assert_eq!(std::fs::read(&saved).unwrap(), b"new");
        assert_eq!(std::fs::metadata(&saved).unwrap().permissions().mode() & 0o777, 0o755);
    }

    #[test]
    fn the_new_version_opens_as_this_one_did_less_its_links_and_mount() {
        let args = ["--hwdec", "vaapi", "anistream://auth/anilist?code=x"].map(String::from);
        assert_eq!(relaunch_args(args), ["--hwdec", "vaapi"]);
        assert_eq!(
            data_dirs_without(
                "/tmp/.mount_ab/usr/share:/usr/local/share:/usr/share",
                "/tmp/.mount_ab"
            ),
            "/usr/local/share:/usr/share"
        );
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert!(older("2.0.9", "2.0.10"));
        assert!(!older("2.1.0", "2.1.0"));
        assert!(!older("2.1.1", "2.1.0"));
        assert!(older("2.1", "2.1.1"));
    }

    /// A server on loopback that answers each request with `body`, `times` times.
    fn serve(body: &'static [u8], times: usize) -> String {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let at = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(times) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap() > 2 {
                    line.clear();
                }
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
            }
        });
        format!("http://{at}/anistream-1.4.0-linux-x86_64.AppImage")
    }

    #[test]
    fn a_download_that_matches_is_kept_and_one_that_does_not_is_deleted() {
        let runtime =
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let body: &[u8] = b"the new version, whole";
        let good = ReleaseAsset { url: serve(body, 1), ..asset(body) };
        let steps = RefCell::new(Vec::new());
        let end = runtime.block_on(fetch(
            &good,
            dir.path(),
            |s| steps.borrow_mut().push(s),
            || false,
        ));
        let path = end.unwrap().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), body);
        assert_eq!(steps.borrow().last(), Some(&Step::Verifying));
        // Already there and whole: kept, with nothing fetched.
        let again = ReleaseAsset { url: "http://127.0.0.1:9/gone".into(), ..good.clone() };
        assert_eq!(
            runtime.block_on(fetch(&again, dir.path(), |_| {}, || false)),
            Some(Ok(path))
        );

        let tampered = ReleaseAsset {
            name: "other.AppImage".into(),
            url: serve(b"the new version, wrong", 1),
            ..asset(body)
        };
        let end = runtime.block_on(fetch(&tampered, dir.path(), |_| {}, || false));
        assert_eq!(
            end,
            Some(
                Err("The download doesn't match the checksum listed for this version.".into())
            )
        );
        assert!(!dir.path().join("other.AppImage").exists());

        let stopped = ReleaseAsset {
            name: "stopped.AppImage".into(),
            url: serve(body, 1),
            ..asset(body)
        };
        assert_eq!(runtime.block_on(fetch(&stopped, dir.path(), |_| {}, || true)), None);
        assert!(!dir.path().join("stopped.AppImage.partial").exists());
    }
}
