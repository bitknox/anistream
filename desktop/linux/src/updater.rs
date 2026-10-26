//! The app updating itself from its AppImage. Once the engine's check finds a newer release with
//! an AppImage for this machine, it downloads into the cache and is checked against the size and
//! SHA-256 the release lists. Restart to Update then puts it in place of the AppImage this copy
//! runs from, quits, and opens the new version with the same arguments. When the AppImage's folder
//! can't be written to, the checked download is saved to Downloads instead, and the app says so.
//! A fixture run goes through the same steps and fetches and installs nothing. See
//! [`crate::updates`].

use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anistream_model::updates::{Release, ReleaseAsset, UpdateStatus};
use gtk::{glib, prelude::*};

use crate::updates::{self, Progress, Step};

/// What the download's thread tells the main loop.
enum Heard {
    Step(Step),
    Done(Option<Result<PathBuf, String>>),
}

/// Run whenever the update moves on.
type Listener = Box<dyn Fn()>;
/// Run with the version once a release the check told of is ready, and whether it was saved to
/// Downloads rather than made ready to install.
type Ready = Box<dyn Fn(&str, bool)>;

pub struct Updater {
    progress: RefCell<Progress>,
    release: RefCell<Option<Release>>,
    /// Whether to say so once the release the check told of is ready.
    tell: Cell<bool>,
    /// A fixture run: the steps at a believable pace, and nothing fetched or installed.
    staged: bool,
    /// Where downloads go, and the note a restart to update leaves for the copy it opens.
    dir: PathBuf,
    appimage: Option<PathBuf>,
    /// A fixture run rehearsing an AppImage in a folder it can't write to.
    #[cfg_attr(not(feature = "fixtures"), allow(dead_code))]
    unwritable: Cell<bool>,
    /// Counts the downloads started, so a stopped one's last words are not taken for a newer one's.
    started: Cell<u64>,
    stop: RefCell<Option<Arc<AtomicBool>>>,
    changed: RefCell<Vec<Listener>>,
    ready: RefCell<Option<Ready>>,
    quit: RefCell<Option<Box<dyn Fn()>>>,
}

impl Updater {
    /// Downloads into `cache`'s `update` folder. `staged` for a fixture run.
    pub fn new(cache: &std::path::Path, staged: bool) -> Rc<Self> {
        Rc::new(Self {
            progress: RefCell::default(),
            release: RefCell::default(),
            tell: Cell::new(false),
            staged,
            dir: cache.join("update"),
            appimage: updates::appimage(),
            unwritable: Cell::new(false),
            started: Cell::new(0),
            stop: RefCell::default(),
            changed: RefCell::default(),
            ready: RefCell::default(),
            quit: RefCell::default(),
        })
    }

    /// How the update stands.
    pub fn progress(&self) -> Progress {
        self.progress.borrow().clone()
    }

    /// Whether this copy installs `release` itself: it runs from an AppImage, or is a fixture
    /// run, and the release has an AppImage for this machine.
    pub fn installs(&self, release: &Release) -> bool {
        (self.staged || self.appimage.is_some())
            && updates::appimage_for(release, std::env::consts::ARCH).is_some()
    }

    /// The AppImage for this machine in the release on offer, when this copy installs it.
    fn asset(&self) -> Option<(String, ReleaseAsset)> {
        let release = self.release.borrow();
        let release = release.as_ref().filter(|r| self.installs(r))?;
        let asset = updates::appimage_for(release, std::env::consts::ARCH)?;
        Some((release.version.clone(), asset.clone()))
    }

    /// Run `f` whenever the update moves on.
    pub fn on_change(&self, f: impl Fn() + 'static) {
        self.changed.borrow_mut().push(Box::new(f));
    }

    /// Run `f` with the version once a release the check told of is ready to install, or saved
    /// to Downloads.
    pub fn on_ready(&self, f: impl Fn(&str, bool) + 'static) {
        self.ready.replace(Some(Box::new(f)));
    }

    /// How the app quits for the new version.
    pub fn on_quit(&self, f: impl Fn() + 'static) {
        self.quit.replace(Some(Box::new(f)));
    }

    /// What the engine's update check said.
    pub fn heard(self: &Rc<Self>, status: &UpdateStatus) {
        match status {
            UpdateStatus::Available { release, notify } => {
                self.release.replace(Some((**release).clone()));
                self.tell.set(self.tell.get() || *notify);
                let progress = self.progress();
                // A newer release than the one under way takes its place.
                if progress.version != release.version || progress.step == Step::Idle {
                    self.download();
                }
            }
            UpdateStatus::UpToDate => {
                self.release.replace(None);
                self.stop();
            }
            _ => {}
        }
        self.said();
    }

    /// Rehearse an AppImage in a folder the app can't write to, in a fixture run.
    #[cfg(feature = "fixtures")]
    pub fn rehearse_unwritable(&self) {
        self.unwritable.set(true);
    }

    /// Where the new version goes once checked, when not in place of the AppImage: Downloads,
    /// for an AppImage in a folder the app can't write to.
    fn saving_to(&self) -> Option<PathBuf> {
        if self.staged {
            return self.unwritable.get().then(updates::downloads_dir);
        }
        let dir = self.appimage.as_deref()?.parent()?;
        (!updates::writable(dir)).then(updates::downloads_dir)
    }

    /// Download and check this machine's AppImage, when the release on offer has one.
    pub fn download(self: &Rc<Self>) {
        let Some((version, asset)) = self.asset() else { return };
        if self.progress.borrow().step == Step::Installing {
            return;
        }
        self.halt();
        let mine = self.started.get() + 1;
        self.started.set(mine);
        let stop = Arc::new(AtomicBool::new(false));
        self.stop.replace(Some(Arc::clone(&stop)));
        self.set(&version, Step::Downloading { received: 0, expected: asset.size });

        let (tx, rx) = async_channel::unbounded::<Heard>();
        let saving = self.saving_to();
        if self.staged {
            rehearse(asset, saving.clone().unwrap_or_else(|| self.dir.clone()), tx, stop);
        } else {
            tidy(&self.dir, Some(&asset.name));
            let dir = self.dir.clone();
            let saving = saving.clone();
            std::thread::spawn(move || {
                let end =
                    match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                        Ok(runtime) => runtime.block_on(updates::fetch(
                            &asset,
                            &dir,
                            |step| {
                                let _ = tx.send_blocking(Heard::Step(step));
                            },
                            || stop.load(Ordering::Relaxed),
                        )),
                        Err(e) => Some(Err(format!("The download couldn't start: {e}."))),
                    };
                // Checked, and copied to Downloads when it can't go in place.
                let end = match (end, saving) {
                    (Some(Ok(path)), Some(downloads)) => {
                        Some(updates::save_to(&path, &downloads))
                    }
                    (end, _) => end,
                };
                let _ = tx.send_blocking(Heard::Done(end));
            });
        }
        let saved = saving.is_some();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(heard) = rx.recv().await {
                let Some(this) = weak.upgrade() else { return };
                if this.started.get() != mine {
                    return;
                }
                match heard {
                    Heard::Step(step) => this.set(&version, step),
                    // Stopped: already said.
                    Heard::Done(None) => return,
                    Heard::Done(Some(Err(reason))) => {
                        tracing::warn!(version, reason, "the update failed");
                        this.set(&version, Step::Failed(reason));
                    }
                    Heard::Done(Some(Ok(path))) => {
                        this.set(
                            &version,
                            if saved { Step::Saved(path) } else { Step::Ready(path) },
                        );
                        if this.tell.replace(false)
                            && let Some(ready) = this.ready.borrow().as_ref()
                        {
                            ready(&version, saved);
                        }
                    }
                }
            }
        });
    }

    /// Stop a download under way.
    pub fn stop(&self) {
        if self.progress.borrow().step == Step::Installing {
            return;
        }
        self.halt();
        let version = self.progress.borrow().version.clone();
        self.set(&version, Step::Idle);
    }

    fn halt(&self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.started.set(self.started.get() + 1);
    }

    /// Show the new version saved to Downloads in the file manager, selected.
    pub fn show_saved(&self, window: &impl IsA<gtk::Window>, failed: impl FnOnce() + 'static) {
        let Step::Saved(path) = self.progress().step else { return };
        if self.staged {
            tracing::info!(path = %path.display(), "the saved version, not shown in a scene");
            return;
        }
        let launcher = gtk::FileLauncher::new(Some(&gtk::gio::File::for_path(&path)));
        launcher.open_containing_folder(
            Some(window),
            gtk::gio::Cancellable::NONE,
            move |done| {
                if let Err(e) = done {
                    tracing::warn!(error = %e, "the folder did not open");
                    failed();
                }
            },
        );
    }

    /// Restart to Update: the new version goes in place of the AppImage, the app quits, and the
    /// new version opens once it has.
    pub fn install(self: &Rc<Self>) {
        let (version, path) = match &*self.progress.borrow() {
            Progress { version, step: Step::Ready(path) } => (version.clone(), path.clone()),
            _ => return,
        };
        self.set(&version, Step::Installing);
        let Some(appimage) = self.appimage.clone().filter(|_| !self.staged) else {
            let weak = Rc::downgrade(self);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                if let Some(this) = weak.upgrade() {
                    this.set(&version, Step::Failed("A fixture run installs nothing.".into()));
                }
            });
            return;
        };
        let marker = self.dir.join("updating");
        let opened = std::fs::write(&marker, &version)
            .map_err(|e| format!("The update couldn't be noted: {e}."))
            .and_then(|()| updates::install(&path, &appimage))
            .and_then(|()| relaunch(&appimage));
        if let Err(reason) = opened {
            let _ = std::fs::remove_file(&marker);
            tracing::warn!(version, reason, "the update didn't go in place");
            self.set(&version, Step::Failed(reason));
            return;
        }
        if let Some(quit) = self.quit.borrow().as_ref() {
            quit();
        }
    }

    /// After a restart to update: whether the new version is the one running, and which version
    /// it was for. Said once.
    pub fn after_restart(&self) -> Option<(bool, String)> {
        let marker = self.dir.join("updating");
        let version = std::fs::read_to_string(&marker).ok()?.trim().to_owned();
        let _ = std::fs::remove_file(&marker);
        let installed = !updates::older(anistream_engine::updates::CURRENT, &version);
        if installed {
            tidy(&self.dir, None);
        }
        Some((installed, version))
    }

    fn set(&self, version: &str, step: Step) {
        self.progress.replace(Progress { version: version.to_owned(), step });
        self.said();
    }

    fn said(&self) {
        for f in self.changed.borrow().iter() {
            f();
        }
    }
}

/// Open the AppImage at `appimage` once this copy has quit, with the arguments it started with.
fn relaunch(appimage: &std::path::Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let mut command = std::process::Command::new("sh");
    command
        .arg("-c")
        .arg(updates::RELAUNCH)
        .arg("anistream-update")
        .arg(std::process::id().to_string())
        .arg(appimage)
        .args(updates::relaunch_args(std::env::args().skip(1)))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        // Its own group, so nothing sent to this copy's reaches it.
        .process_group(0);
    if let (Some(appdir), Some(dirs)) =
        (std::env::var_os("APPDIR"), std::env::var_os("XDG_DATA_DIRS"))
    {
        let dirs =
            updates::data_dirs_without(&dirs.to_string_lossy(), &appdir.to_string_lossy());
        command.env("XDG_DATA_DIRS", dirs);
    }
    for var in updates::MOUNT_VARS {
        command.env_remove(var);
    }
    command.spawn().map(drop).map_err(|e| {
        format!(
            "The new version is in place but couldn't be opened: {e}. Open anistream again."
        )
    })
}

/// Remove what earlier updates left in `dir`, apart from files named for `keep`.
fn tidy(dir: &std::path::Path, keep: Option<&str>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if keep.is_none_or(|keep| !name.to_string_lossy().starts_with(keep)) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// A fixture run's download: the steps at a believable pace, and nothing fetched.
fn rehearse(
    asset: ReleaseAsset,
    dir: PathBuf,
    tx: async_channel::Sender<Heard>,
    stop: Arc<AtomicBool>,
) {
    glib::spawn_future_local(async move {
        for step in 0..=20u64 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let received = asset.size * step / 20;
            let _ = tx
                .send(Heard::Step(Step::Downloading { received, expected: asset.size }))
                .await;
            glib::timeout_future(Duration::from_millis(150)).await;
        }
        let _ = tx.send(Heard::Step(Step::Verifying)).await;
        glib::timeout_future(Duration::from_millis(600)).await;
        if !stop.load(Ordering::Relaxed) {
            let _ = tx.send(Heard::Done(Some(Ok(dir.join(&asset.name))))).await;
        }
    });
}
