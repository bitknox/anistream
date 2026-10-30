//! Play on TV on the fixture scene: two TVs on loopback, "Living room" and "Bedroom", each a real
//! remote server with a scripted player playing the fixture titles, found by a browser that
//! lists them as Avahi would. Nothing reaches the network, and no real TV is ever asked.

use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex, PoisonError},
};

use anistream_cast::fake::{FakeOptions, FakeReceiver, advert_txt};
use anistream_engine::remote::{
    protocol::{
        discovery::{Browser, DiscoverySink, FoundTv},
        fake::{FakeTitle, FakeTv},
    },
    sender::{CastBrowser, CastSink, FoundCast},
};
use anistream_testkit::{fixtures, sources};
use tokio::runtime::Handle;

use super::{Tv, cast::Casting, open_sender};

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
    casts: Arc<FixtureCasts>,
    /// The TVs with Chromecast built in on loopback, by name.
    receivers: RefCell<Vec<(String, FakeReceiver)>>,
}

/// The model over the scene's TVs, its paired list under `data` as a real run keeps it.
pub fn tv(handle: Handle, data: &Path, casting: Option<Casting>) -> Rc<Tv> {
    let browser = Arc::new(FixtureBrowser::default());
    let casts = Arc::new(FixtureCasts::default());
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
    let sender = open_sender(&handle, data, casting)
        .inspect_err(|e| tracing::warn!(error = %e, "no Play on TV"))
        .ok()
        .map(Arc::new);
    let tv = Tv::with(
        handle.clone(),
        sender,
        Arc::clone(&browser) as Arc<dyn Browser>,
        Arc::clone(&casts) as Arc<dyn CastBrowser>,
    );
    SCENE.with(|s| {
        s.replace(Some(Rc::new(Scene {
            handle,
            tvs: RefCell::new(tvs),
            browser,
            casts,
            receivers: RefCell::default(),
        })))
    });
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

    /// The scene's TVs running anistream are found at another address than loopback, where the
    /// TVs with Chromecast built in are: the picker leaves out a Cast TV at the address of one
    /// running anistream.
    pub fn apart(&self) {
        self.browser.apart();
    }

    /// A TV with Chromecast built in joins the network: a receiver on loopback set up as
    /// `options` say, found under `name` by a browse.
    pub fn cast(&self, name: &str, options: FakeOptions) {
        let fake = self
            .handle
            .block_on(self.handle.spawn(async move { FakeReceiver::start(options).await }));
        let receiver = match fake {
            Ok(Ok(receiver)) => receiver,
            Ok(Err(e)) => {
                return tracing::warn!(error = %e, name, "a fixture cast TV did not start");
            }
            Err(e) => {
                return tracing::warn!(error = %e, name, "a fixture cast TV did not start");
            }
        };
        let id = name.to_lowercase().replace(' ', "-");
        self.casts.add(FoundCast {
            instance: format!("Chromecast-{id}"),
            address: receiver.address,
            txt: advert_txt(&id, name, "Chromecast"),
        });
        self.receivers.borrow_mut().push((name.to_owned(), receiver));
    }

    /// Every TV with Chromecast built in leaves the network.
    pub fn cast_clear(&self) {
        let gone: Vec<_> = self.receivers.borrow_mut().drain(..).collect();
        let _entered = self.handle.enter();
        self.casts.clear();
        drop(gone);
    }

    /// Someone else starts something on the TV `name`.
    pub fn cast_take_over(&self, name: &str) {
        self.with_receiver(name, FakeReceiver::take_over);
    }

    /// The TV `name` stops answering, as when switched off at the wall.
    pub fn cast_go_silent(&self, name: &str) {
        self.with_receiver(name, FakeReceiver::go_silent);
    }

    fn with_receiver(&self, name: &str, act: impl FnOnce(&FakeReceiver)) {
        let receivers = self.receivers.borrow();
        if let Some((_, receiver)) = receivers.iter().find(|(n, _)| n == name) {
            let _entered = self.handle.enter();
            act(receiver);
        }
    }
}

/// Lists the scene's TVs with Chromecast built in as found, as Avahi would.
#[derive(Default)]
struct FixtureCasts {
    found: Mutex<Vec<FoundCast>>,
    sink: Mutex<Option<CastSink>>,
}

impl FixtureCasts {
    fn add(&self, found: FoundCast) {
        lock(&self.found).push(found.clone());
        if let Some(sink) = lock(&self.sink).as_ref() {
            sink.found(found);
        }
    }

    fn clear(&self) {
        let gone: Vec<_> = lock(&self.found).drain(..).collect();
        if let Some(sink) = lock(&self.sink).as_ref() {
            for found in gone {
                sink.lost(&found.instance);
            }
        }
    }
}

impl CastBrowser for FixtureCasts {
    fn start(&self, sink: CastSink) {
        for found in lock(&self.found).iter() {
            sink.found(found.clone());
        }
        lock(&self.sink).replace(sink);
    }

    fn stop(&self) {
        lock(&self.sink).take();
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

    /// Every TV found is found at a documentation address instead of loopback, so a Cast TV
    /// on loopback is not mistaken for the same TV.
    fn apart(&self) {
        let mut found = lock(&self.found);
        for tv in found.iter_mut() {
            for address in &mut tv.addresses {
                address.set_ip(std::net::Ipv4Addr::new(192, 0, 2, 10).into());
            }
        }
        if let Some(sink) = lock(&self.sink).as_ref() {
            for tv in found.iter() {
                sink.found(tv.clone());
            }
        }
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
