//! Play on TV on the fixture scene: two TVs on loopback, "Living room" and "Bedroom", each a real
//! remote server with a scripted player playing the fixture titles, found by a browser that
//! lists them as Avahi would. Nothing reaches the network, and no real TV is ever asked.

use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex, PoisonError},
};

use anistream_engine::remote::protocol::{
    discovery::{Browser, DiscoverySink, FoundTv},
    fake::{FakeTitle, FakeTv},
};
use anistream_testkit::{fixtures, sources};
use tokio::runtime::Handle;

use super::{Tv, open_sender};

pub const LIVING_ROOM: &str = "Living room";
pub const BEDROOM: &str = "Bedroom";

thread_local! {
    static SCENE: RefCell<Option<Rc<Scene>>> = const { RefCell::new(None) };
}

/// The scene's TVs, for the shots to drive.
pub fn scene() -> Option<Rc<Scene>> {
    SCENE.with(|s| s.borrow().clone())
}

pub struct Scene {
    handle: Handle,
    tvs: RefCell<Vec<(String, Option<FakeTv>)>>,
    browser: Arc<FixtureBrowser>,
}

/// The model over the scene's TVs, its paired list under `data` as a real run keeps it.
pub fn tv(handle: Handle, data: &Path) -> Rc<Tv> {
    let browser = Arc::new(FixtureBrowser::default());
    let mut tvs = Vec::new();
    for name in [LIVING_ROOM, BEDROOM] {
        match start(&handle, name) {
            Ok(fake) => {
                browser.add(&fake);
                tvs.push((name.to_owned(), Some(fake)));
            }
            Err(e) => tracing::warn!(error = %e, name, "a fixture TV did not start"),
        }
    }
    let sender = open_sender(&handle, data)
        .inspect_err(|e| tracing::warn!(error = %e, "no Play on TV"))
        .ok()
        .map(Arc::new);
    let tv = Tv::with(handle.clone(), sender, Arc::clone(&browser) as Arc<dyn Browser>);
    SCENE.with(|s| s.replace(Some(Rc::new(Scene { handle, tvs: RefCell::new(tvs), browser }))));
    tv
}

fn start(handle: &Handle, name: &str) -> Result<FakeTv, String> {
    let titles = fixtures::ALL
        .iter()
        .map(|id| FakeTitle {
            id: *id,
            title: fixtures::title(*id),
            episodes: sources::episodes(*id)
                .into_iter()
                .map(|e| e.title.unwrap_or_default())
                .collect(),
            duration: fixtures::runtime_secs(*id),
        })
        .collect();
    let name = name.to_owned();
    handle
        .block_on(handle.spawn(async move { FakeTv::start(&name, titles).await }))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

impl Scene {
    fn with<T>(&self, name: &str, read: impl FnOnce(&FakeTv) -> T) -> Option<T> {
        let tvs = self.tvs.borrow();
        let fake = tvs.iter().find(|(n, _)| n == name)?.1.as_ref()?;
        let _entered = self.handle.enter();
        Some(read(fake))
    }

    /// The link a TV's QR code carries, with a code it opened itself: pairs with no typing.
    pub fn pair_link(&self, name: &str) -> Option<String> {
        self.with(name, |fake| fake.pair_link(Some(fake.open_code())))
    }

    /// The TV forgets every device, as its settings would.
    pub fn revoke_all(&self, name: &str) {
        self.with(name, FakeTv::revoke_all);
    }

    /// The TV goes away: switched off, and gone from the network.
    pub fn switch_off(&self, name: &str) {
        let fake = {
            let mut tvs = self.tvs.borrow_mut();
            tvs.iter_mut().find(|(n, _)| n == name).and_then(|(_, fake)| fake.take())
        };
        let _entered = self.handle.enter();
        drop(fake);
        self.browser.remove(name);
    }

    /// No TV answers a browse, as on a network that drops multicast.
    pub fn silence(&self, on: bool) {
        self.browser.silence(on);
    }
}

/// Lists the scene's TVs as found, as Avahi would on the same network.
#[derive(Default)]
struct FixtureBrowser {
    found: Mutex<Vec<FoundTv>>,
    sink: Mutex<Option<DiscoverySink>>,
    silent: Mutex<bool>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FixtureBrowser {
    fn add(&self, fake: &FakeTv) {
        let advert = fake.advert();
        lock(&self.found).push(FoundTv::from_txt(
            &advert.instance,
            vec![fake.addr()],
            &advert.txt,
        ));
    }

    fn remove(&self, name: &str) {
        lock(&self.found).retain(|tv| tv.instance != name);
        if let Some(sink) = lock(&self.sink).as_ref() {
            sink.lost(name);
        }
    }

    fn silence(&self, on: bool) {
        *lock(&self.silent) = on;
    }
}

impl Browser for FixtureBrowser {
    fn start(&self, sink: DiscoverySink) {
        if !*lock(&self.silent) {
            for tv in lock(&self.found).iter() {
                sink.found(tv.clone());
            }
        }
        lock(&self.sink).replace(sink);
    }

    fn stop(&self) {
        lock(&self.sink).take();
    }
}

impl Tv {
    /// Pair with `target` with nothing shown, as a TV paired on an earlier day.
    pub fn pair_quietly(&self, target: &str) {
        let Some(sender) = self.sender.clone() else { return };
        let target = target.to_owned();
        self.run(
            async move {
                let mut steps = sender.pair(&target).await?;
                steps.pair(None).await
            },
            |tv, answer| match answer {
                Ok(_) => tv.refresh(),
                Err(problem) => tracing::warn!(%problem, "a fixture TV did not pair"),
            },
        );
    }
}
