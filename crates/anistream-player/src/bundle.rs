//! The player's scripts, fonts and settings, carried inside the binary.
//!
//! anistream's player UI is a set of mpv scripts — upstream uosc, a patched thumbfast and our
//! own companion script — plus the fonts they draw with, and mpv only loads those from disk.
//! Everything under `assets/mpv/` is therefore embedded at build time and written out at launch
//! into `<cache>/mpv/<hash>/`, a directory named after the content it holds and mirroring the
//! source tree. A launch whose directory is already there writes nothing; a new build brings a
//! new hash and so a new directory, and the old one is cleared away behind it.
//!
//! `build.rs` walks the tree and generates the file table and the hash, so a file added under
//! `assets/mpv/` ships without being listed anywhere and the hash is never computed at launch.
//! Paths in the table always use `/`; they are split into components here, so the same table
//! serves Windows.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

// Defines `HASH: &str` and `FILES: &[(&str, &[u8])]`, sorted by path.
include!(concat!(env!("OUT_DIR"), "/mpv_bundle.rs"));

/// Directory under the cache that holds one subdirectory per bundle hash.
const PARENT: &str = "mpv";

/// Prefix of a directory still being written. Never a valid hash, so a half-written tree is
/// never mistaken for a finished one.
const STAGING: &str = ".staging-";

/// Prefix of a damaged tree moved out of the way before its replacement goes in.
const DISCARDED: &str = ".discarded-";

/// The bundled player files, written out and ready for mpv to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    root: PathBuf,
}

impl Bundle {
    /// Make sure this build's files exist under `cache_dir`, and return where they are.
    ///
    /// The tree is written into a staging sibling and renamed into place, so a crash or a
    /// second anistream starting at the same moment can never leave mpv loading half a uosc.
    /// When the directory is already there the only cost is one `stat` per file — checking
    /// sizes rather than trusting the name alone means a tree someone has partly deleted is
    /// rebuilt instead of failing in Lua at the next launch.
    ///
    /// Directories left by other builds are removed afterwards, best effort: one that cannot
    /// be removed (a file still open on Windows) is tried again next launch. An older
    /// anistream still running when a newer one starts loses its copy, which costs it only the
    /// files its mpv had not read yet, such as a uosc module required on first use; that
    /// overlap is rare enough not to be worth a lock.
    pub fn materialize(cache_dir: &Path) -> io::Result<Self> {
        let parent = cache_dir.join(PARENT);
        let root = parent.join(HASH);
        if !is_complete(&root) {
            fs::create_dir_all(&parent)?;
            let staging = parent.join(format!("{STAGING}{HASH}-{}", unique_suffix()));
            if let Err(e) = write_tree(&staging) {
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
            install(&parent, &staging, &root)?;
        }
        remove_stale(&parent);
        Ok(Self { root })
    }

    /// `<cache>/mpv/<hash>`, the counterpart of `assets/mpv/`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The companion script, a directory script mpv names `anistream` after its directory.
    pub fn companion(&self) -> PathBuf {
        self.path("scripts/anistream")
    }

    /// Upstream uosc, as a directory script.
    pub fn uosc(&self) -> PathBuf {
        self.path("scripts/uosc")
    }

    /// The patched thumbfast, a single-file script.
    pub fn thumbfast(&self) -> PathBuf {
        self.path("scripts/thumbfast.lua")
    }

    /// The directory for `--osd-fonts-dir`: uosc's icon and texture fonts plus Zen Kaku
    /// Gothic New.
    pub fn fonts(&self) -> PathBuf {
        self.path("fonts")
    }

    /// `uosc-opts.conf` as `(key, value)` pairs, in file order; empty when the file is absent.
    ///
    /// Each pair becomes `--script-opts-append=uosc-<key>=<value>`. Read from the embedded
    /// copy, so it costs no I/O and cannot disagree with what was written to disk.
    pub fn uosc_opts(&self) -> Vec<(String, String)> {
        embedded("uosc-opts.conf").map(parse_conf).unwrap_or_default()
    }

    /// `thumbfast-opts.conf` as `(key, value)` pairs, each one becoming
    /// `--script-opts-append=thumbfast-<key>=<value>`.
    pub fn thumbfast_opts(&self) -> Vec<(String, String)> {
        embedded("thumbfast-opts.conf").map(parse_conf).unwrap_or_default()
    }

    fn path(&self, relative: &str) -> PathBuf {
        join(&self.root, relative)
    }
}

/// `relative` (always `/`-separated) under `base`, in the platform's own separators.
fn join(base: &Path, relative: &str) -> PathBuf {
    relative.split('/').fold(base.to_path_buf(), |path, part| path.join(part))
}

fn embedded(relative: &str) -> Option<&'static [u8]> {
    FILES.iter().find(|(path, _)| *path == relative).map(|(_, bytes)| *bytes)
}

/// Whether every embedded file is present under `root` at its embedded size.
fn is_complete(root: &Path) -> bool {
    FILES.iter().all(|(relative, bytes)| {
        fs::metadata(join(root, relative))
            .is_ok_and(|meta| meta.is_file() && meta.len() == bytes.len() as u64)
    })
}

/// A name part no other staging directory, in this process or another, will share.
fn unique_suffix() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))
}

fn write_tree(dir: &Path) -> io::Result<()> {
    for (relative, bytes) in FILES {
        let path = join(dir, relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, bytes)?;
    }
    Ok(())
}

/// Rename the finished `staging` tree to `root`.
///
/// A damaged `root` is moved aside with one rename rather than deleted file by file, so the
/// name goes from the damaged tree to a complete one without passing through a half-deleted
/// one; the moved tree is cleared with the other stale entries. Losing the rename to a second
/// anistream that installed the same build a moment earlier is not an error: its tree is as
/// good as ours.
fn install(parent: &Path, staging: &Path, root: &Path) -> io::Result<()> {
    if root.exists() {
        let aside = parent.join(format!("{DISCARDED}{HASH}-{}", unique_suffix()));
        let _ = fs::rename(root, aside);
    }
    match fs::rename(staging, root) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_dir_all(staging);
            if is_complete(root) { Ok(()) } else { Err(e) }
        }
    }
}

/// Remove everything under `<cache>/mpv` except this build's tree and staging directories
/// this build's other instances may still be writing.
fn remove_stale(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else { return };
    let own_staging = format!("{STAGING}{HASH}-");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == HASH || name.starts_with(&own_staging) {
            continue;
        }
        let path = entry.path();
        let removed = if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(e) = removed {
            tracing::debug!(path = %path.display(), error = %e, "stale mpv bundle not removed");
        }
    }
}

/// Parse a `key=value` file: one pair per line, split at the first `=`, both sides trimmed.
///
/// Only whole-line `#` comments exist. A trailing `# …` cannot be a comment, because values
/// legitimately contain `#` and `=` — a colour, a list of `name=value` pairs. A line with no
/// `=` or an empty key is skipped with a warning rather than failing the launch over one typo.
fn parse_conf(bytes: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| match line.split_once('=') {
            Some((key, value)) if !key.trim().is_empty() => {
                Some((key.trim().to_owned(), value.trim().to_owned()))
            }
            _ => {
                tracing::warn!(line, "ignoring a malformed line in a bundled mpv option file");
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn every_embedded_file_is_written_byte_for_byte() {
        let cache = tempfile::tempdir().unwrap();
        let bundle = Bundle::materialize(cache.path()).unwrap();

        assert_eq!(bundle.root(), cache.path().join("mpv").join(HASH));
        for (relative, bytes) in FILES {
            let written = fs::read(join(bundle.root(), relative)).unwrap();
            assert!(written == *bytes, "{relative} differs from the embedded copy");
        }
        assert!(bundle.uosc().join("main.lua").is_file());
        assert!(bundle.thumbfast().is_file());
        assert!(bundle.fonts().join("ZenKakuGothicNew-Regular.ttf").is_file());
        assert!(bundle.fonts().join("uosc_icons.otf").is_file());
        assert!(bundle.companion().ends_with(Path::new("scripts").join("anistream")));
        // Nothing is left behind next to the finished tree.
        let siblings: Vec<_> = fs::read_dir(cache.path().join("mpv")).unwrap().collect();
        assert_eq!(siblings.len(), 1, "staging directory left behind: {siblings:?}");
    }

    #[test]
    fn the_table_covers_the_whole_tree_and_nothing_hidden() {
        // The table is generated, so this pins the generator: the vendored pieces are all
        // there, paths use `/` on every platform, and Finder's droppings are not shipped.
        let paths: Vec<&str> = FILES.iter().map(|(path, _)| *path).collect();
        for expected in [
            "scripts/uosc/main.lua",
            "scripts/uosc/elements/Timeline.lua",
            "scripts/thumbfast.lua",
            "fonts/uosc_icons.otf",
            "fonts/ZenKakuGothicNew-Bold.ttf",
            "thumbfast-opts.conf",
            "licenses/LICENSE-uosc",
        ] {
            assert!(paths.contains(&expected), "{expected} is not embedded");
        }
        assert!(paths.iter().all(|path| !path.contains('\\')));
        assert!(paths.iter().all(|path| path.split('/').all(|part| !part.starts_with('.'))));
        assert!(paths.windows(2).all(|pair| pair[0] < pair[1]), "table is not sorted");
        assert_eq!(HASH.len(), 16);
    }

    #[test]
    fn a_second_launch_trusts_the_existing_tree() {
        let cache = tempfile::tempdir().unwrap();
        let first = Bundle::materialize(cache.path()).unwrap();
        // A file the bundle does not know about survives only if nothing was rewritten.
        let marker = first.root().join("marker");
        fs::write(&marker, b"left alone").unwrap();
        let modified = fs::metadata(first.thumbfast()).unwrap().modified().unwrap();

        let second = Bundle::materialize(cache.path()).unwrap();
        assert_eq!(first, second);
        assert!(marker.is_file(), "the existing tree was replaced");
        assert_eq!(fs::metadata(second.thumbfast()).unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn a_damaged_tree_is_rebuilt() {
        let cache = tempfile::tempdir().unwrap();
        let bundle = Bundle::materialize(cache.path()).unwrap();
        fs::remove_file(bundle.uosc().join("main.lua")).unwrap();
        fs::write(bundle.thumbfast(), b"truncated").unwrap();

        let again = Bundle::materialize(cache.path()).unwrap();
        assert!(is_complete(again.root()));
        assert!(again.uosc().join("main.lua").is_file());
        // The damaged copy was moved aside and then cleared with the other stale entries.
        let siblings: Vec<_> = fs::read_dir(cache.path().join("mpv")).unwrap().collect();
        assert_eq!(siblings.len(), 1, "discarded tree left behind: {siblings:?}");
    }

    #[test]
    fn trees_from_other_builds_are_removed() {
        let cache = tempfile::tempdir().unwrap();
        let parent = cache.path().join("mpv");
        let old = parent.join("0123456789abcdef").join("scripts");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("main.lua"), b"-- old").unwrap();
        let crashed = parent.join(format!("{STAGING}0123456789abcdef-1-0"));
        fs::create_dir_all(&crashed).unwrap();
        fs::write(parent.join("stray-file"), b"").unwrap();
        // Another instance of this very build, mid-write: not ours to delete.
        let sibling = parent.join(format!("{STAGING}{HASH}-1-0"));
        fs::create_dir_all(&sibling).unwrap();

        let bundle = Bundle::materialize(cache.path()).unwrap();
        assert!(bundle.root().is_dir());
        assert!(!parent.join("0123456789abcdef").exists());
        assert!(!crashed.exists());
        assert!(!parent.join("stray-file").exists());
        assert!(sibling.exists());
    }

    #[test]
    fn conf_lines_split_at_the_first_equals_sign() {
        let text = "\
# a comment
   # an indented comment

network=yes
  max_width = 320  \r
color=foreground=E4E0D6,background=161A2E
font=Zen Kaku Gothic New
hint=#F2A64B # not a comment
=no key
no equals sign
empty=
";
        assert_eq!(
            parse_conf(text.as_bytes()),
            [
                ("network", "yes"),
                ("max_width", "320"),
                ("color", "foreground=E4E0D6,background=161A2E"),
                ("font", "Zen Kaku Gothic New"),
                ("hint", "#F2A64B # not a comment"),
                ("empty", ""),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
        );
    }

    #[test]
    fn the_shipped_thumbfast_options_enable_network_playback() {
        // Without this thumbfast silently refuses every anistream stream, torrents included.
        let cache = tempfile::tempdir().unwrap();
        let bundle = Bundle::materialize(cache.path()).unwrap();
        let opts = bundle.thumbfast_opts();
        assert!(opts.contains(&("network".to_owned(), "yes".to_owned())), "{opts:?}");
    }

    fn mpv_available() -> bool {
        Command::new("mpv")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    /// Run `mpv` with `args` to completion, killing it if it outlives `limit`.
    fn run_mpv(args: &[String], limit: Duration) {
        let mut child = Command::new("mpv")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if started.elapsed() > limit {
                let _ = child.kill();
                let _ = child.wait();
                panic!("mpv did not exit within {limit:?}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn uosc_and_thumbfast_load_in_mpv_without_lua_errors() {
        // CI has no mpv; what is under test is the vendored Lua, not the machine.
        if !mpv_available() {
            // CI's mpv job installs mpv and sets this, so a missing mpv fails there
            // instead of passing as a skip.
            assert!(
                std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
            );
            eprintln!("skipping: mpv is not on PATH");
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let bundle = Bundle::materialize(cache.path()).unwrap();
        let log = cache.path().join("mpv.log");

        let mut args: Vec<String> = [
            "--no-config",
            "--load-scripts=no",
            "--idle=no",
            "--vo=null",
            "--ao=null",
            "--osc=no",
            "--osd-bar=no",
            "--osd-font=Zen Kaku Gothic New",
        ]
        .map(str::to_owned)
        .to_vec();
        args.push(format!("--osd-fonts-dir={}", bundle.fonts().display()));
        args.push(format!("--script={}", bundle.uosc().display()));
        args.push(format!("--script={}", bundle.thumbfast().display()));
        for (key, value) in bundle.thumbfast_opts() {
            args.push(format!("--script-opts-append=thumbfast-{key}={value}"));
        }
        args.push(format!("--log-file={}", log.display()));
        // A short file rather than an idle player, so mpv exits by itself and the scripts run
        // their file-loaded and end-file handlers as well as their startup code.
        args.push("av://lavfi:testsrc2=size=640x360:rate=24:duration=2".to_owned());
        run_mpv(&args, Duration::from_secs(30));

        let log = fs::read_to_string(&log).unwrap();
        for script in ["uosc", "thumbfast"] {
            assert!(
                log.contains(&format!("[{script}] Loading lua script")),
                "{script} was never loaded:\n{log}"
            );
            let errors: Vec<&str> = log
                .lines()
                .filter(|line| {
                    line.contains(&format!("[e][{script}]"))
                        || line.contains(&format!("[f][{script}]"))
                })
                .collect();
            assert!(errors.is_empty(), "{script} logged errors:\n{}", errors.join("\n"));
        }
        assert!(!log.contains("Lua error"), "a script raised:\n{log}");
    }

    /// Drives the anistream patch in thumbfast against a stand-in for its child mpv that
    /// records the arguments it was started with.
    #[cfg(unix)]
    #[test]
    fn thumbfast_opens_the_host_url_and_inherits_the_http_identity() {
        use std::io::Write as _;
        use std::os::unix::{fs::PermissionsExt as _, net::UnixStream};

        if !mpv_available() {
            // CI's mpv job installs mpv and sets this, so a missing mpv fails there
            // instead of passing as a skip.
            assert!(
                std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
            );
            eprintln!("skipping: mpv is not on PATH");
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let bundle = Bundle::materialize(cache.path()).unwrap();
        let record = cache.path().join("child-args");
        let fake = cache.path().join("fake-mpv");
        fs::write(
            &fake,
            format!(
                "#!/bin/sh\n{{ printf '%s\\n' \"$@\"; echo '<end>'; }} >> '{}'\n",
                record.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let socket = cache.path().join("ipc.sock");

        let args: Vec<String> = vec![
            "--no-config".into(),
            "--load-scripts=no".into(),
            "--idle=yes".into(),
            "--vo=null".into(),
            "--ao=null".into(),
            format!("--script={}", bundle.thumbfast().display()),
            format!("--script-opts-append=thumbfast-mpv_path={}", fake.display()),
            // Spawning on load makes each file start a child without simulating a hover.
            "--script-opts-append=thumbfast-spawn_first=yes".into(),
            "--http-header-fields-append=Referer: https://cdn.example/a,b".into(),
            "--http-header-fields-append=X-Token: one\\,two".into(),
            "--user-agent=Agent/1.0 (x, y)".into(),
            "--referrer=https://site.example/watch".into(),
            format!("--input-ipc-server={}", socket.display()),
        ];
        let mut mpv = Command::new("mpv")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let guard = KillOnDrop(&mut mpv);

        let started = Instant::now();
        let mut ipc = loop {
            if let Ok(stream) = UnixStream::connect(&socket) {
                break stream;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "mpv never opened its socket");
            std::thread::sleep(Duration::from_millis(50));
        };
        let mut send = |command: serde_json::Value| {
            let line = serde_json::json!({ "command": command }).to_string() + "\n";
            ipc.write_all(line.as_bytes()).unwrap();
        };
        // The spawns the stand-in has finished recording. It writes its arguments a few at a
        // time and `<end>` last, so a read can land partway through one: what follows the last
        // `<end>` is a spawn still being written, or nothing, and is not counted.
        let invocations = || -> Vec<Vec<String>> {
            let text = fs::read_to_string(&record).unwrap_or_default();
            let mut finished: Vec<&str> = text.split("<end>\n").collect();
            finished.pop();
            finished
                .into_iter()
                .map(|chunk| chunk.lines().map(str::to_owned).collect())
                .collect()
        };
        let wait_for = |count: usize| -> Vec<Vec<String>> {
            let started = Instant::now();
            loop {
                let seen = invocations();
                if seen.len() >= count {
                    return seen;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(10),
                    "expected {count} thumbnailer spawns, saw {seen:?}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        let key = "user-data/anistream/thumbnail-url";
        let (url_1, url_2) = ("http://127.0.0.1:9/thumb/1", "http://127.0.0.1:9/thumb/2");

        // 1. A torrent-style file: the host names the thumbnail URL before loading it.
        let video = "av://lavfi:testsrc2=size=640x360:rate=24:duration=60";
        send(serde_json::json!(["set_property", key, url_1]));
        send(serde_json::json!(["loadfile", video]));
        let first = wait_for(1).remove(0);
        assert_eq!(first.last().map(String::as_str), Some(url_1), "{first:?}");
        for expected in [
            "--http-header-fields-append=Referer: https://cdn.example/a,b",
            "--http-header-fields-append=X-Token: one\\,two",
            "--user-agent=Agent/1.0 (x, y)",
            "--referrer=https://site.example/watch",
        ] {
            assert!(first.iter().any(|arg| arg == expected), "no {expected} in {first:?}");
        }

        // 2. The URL changes mid-file: the running child is retired, and the next thumbnail
        // request starts one on the new URL. Straight after the change: mpv runs one client's
        // commands in order, so the request finds the new URL set, though the script's
        // observer of it may not have run yet.
        send(serde_json::json!(["set_property", key, url_2]));
        send(serde_json::json!(["script-message-to", "thumbfast", "thumb", "5", "", ""]));
        let second = wait_for(2).remove(1);
        assert_eq!(second.last().map(String::as_str), Some(url_2), "{second:?}");

        // 3. The next file has no thumbnail URL, so thumbfast opens the file itself. A
        // different frame size, because spawn_first only fires when the thumbnail size changes.
        let video = "av://lavfi:testsrc2=size=480x360:rate=24:duration=60";
        send(serde_json::json!(["set_property", key, ""]));
        send(serde_json::json!(["loadfile", video]));
        let third = wait_for(3).remove(2);
        assert_eq!(third.last().map(String::as_str), Some(video), "{third:?}");

        send(serde_json::json!(["quit"]));
        drop(guard);
    }

    /// thumbfast's child runs in mpv's encoding mode, where mpv before 0.40 applies its
    /// `encoding` profile after the command line and turns the child's `--keep-open=always`
    /// off; the first seek to meet an end of file then ended the file, and the child drew
    /// nothing after it. The patched client script sets it again once the child is up. This
    /// asks a real child what it ended up with, through the file thumbfast writes its seeks to.
    #[cfg(unix)]
    #[test]
    fn thumbfasts_child_keeps_its_file_open_past_an_end() {
        use std::io::Write as _;
        use std::os::unix::net::UnixStream;

        if !mpv_available() {
            // CI's mpv job installs mpv and sets this, so a missing mpv fails there
            // instead of passing as a skip.
            assert!(
                std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
            );
            eprintln!("skipping: mpv is not on PATH");
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let bundle = Bundle::materialize(cache.path()).unwrap();
        let commands = cache.path().join("commands");
        let answer = cache.path().join("answer");
        let socket = cache.path().join("ipc.sock");
        let mut mpv = Command::new("mpv")
            .args(["--no-config", "--load-scripts=no", "--pause", "--vo=null", "--ao=null"])
            .arg(format!("--script={}", bundle.thumbfast().display()))
            // thumbfast adds the parent's pid to both, and they stay in this test's directory.
            .arg(format!("--script-opts-append=thumbfast-socket={}", commands.display()))
            .arg(format!(
                "--script-opts-append=thumbfast-thumbnail={}",
                cache.path().join("thumb").display()
            ))
            .arg("--script-opts-append=thumbfast-spawn_first=yes")
            .arg(format!("--input-ipc-server={}", socket.display()))
            .arg("av://lavfi:testsrc2=size=640x360:rate=24:duration=60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let commands = PathBuf::from(format!("{}{}", commands.display(), mpv.id()));
        let guard = KillOnDrop(&mut mpv);

        // The client script clears the file as it starts, so the question is asked until the
        // child answers; until then the file is not there to append to.
        let question = format!(
            "expand-properties run /bin/sh -c \"printf %s ${{keep-open}} > '{}'\"\n",
            answer.display()
        );
        let started = Instant::now();
        let keep_open = loop {
            if let Some(said) = fs::read_to_string(&answer).ok().filter(|s| !s.is_empty()) {
                break said;
            }
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "thumbfast's child never answered"
            );
            if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&commands) {
                file.write_all(question.as_bytes()).unwrap();
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        assert_eq!(keep_open, "always", "the child's keep-open was turned off");

        // Quitting the parent ends its file, and with it the child: thumbfast starts it as a
        // subprocess that lasts as long as the playback.
        let mut ipc = UnixStream::connect(&socket).unwrap();
        ipc.write_all(b"{\"command\":[\"quit\"]}\n").unwrap();
        drop(guard);
    }

    #[cfg(unix)]
    struct KillOnDrop<'a>(&'a mut std::process::Child);

    #[cfg(unix)]
    impl Drop for KillOnDrop<'_> {
        fn drop(&mut self) {
            let started = Instant::now();
            while self.0.try_wait().ok().flatten().is_none() {
                if started.elapsed() > Duration::from_secs(5) {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}
