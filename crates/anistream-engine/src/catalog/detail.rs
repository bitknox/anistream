//! A title's page, and what it opens: the trailer, and links in the browser.

use anistream_core::ids::AnilistId;
use anistream_model::{event::Event, notice::Notice};

use crate::{
    data, dispatch::Lent, jobs::JobGuard, outbox::Outbox, services::Services,
    state::EngineState,
};

/// A title's page, with the source chosen for it sent ahead.
pub async fn load_detail(lent: Lent, id: AnilistId) {
    let Lent { meta, store, tx, .. } = lent;
    // Sent before the detail itself, so the picker knows the current choice the
    // moment the title is on screen rather than only after it is opened once.
    let _ = tx.send(Event::ProviderPreference(store.provider_preference(id).ok().flatten()));
    let update = match meta.media(id).await {
        Ok(media) => Event::Detail(Box::new(data::entry_with_detail(&media, Some(&store)))),
        Err(e) => Event::Toast(Notice::alert(format!("could not load title: {e}"))),
    };
    let _ = tx.send(update);
}

/// Play a title's trailer in mpv, or open it in the browser when mpv cannot stream it.
///
/// Without the `desktop` feature there is no mpv, and the trailer is opened wherever the host
/// opens a link: on a phone, the app that plays the site's videos.
pub fn play_trailer(
    url: String,
    title: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    #[cfg(feature = "desktop")]
    let config = engine.config.effective();
    #[cfg(feature = "desktop")]
    let mpv_binary = config.playback.mpv_binary.clone();
    #[cfg(not(feature = "desktop"))]
    let _ = (engine, title);
    let (host, tx) = (services.host.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        // mpv streams YouTube through yt-dlp, and without it the window opens and
        // dies with nothing to say. Checked up front so the failure mode is a
        // browser tab rather than a mystery.
        #[cfg(feature = "desktop")]
        if in_path(&["yt-dlp", "youtube-dl"])
            && let Ok(mut child) = tokio::process::Command::new(&mpv_binary)
                .arg(format!("--force-media-title={title} — trailer"))
                .arg(&url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        {
            let _ = tx.send(Event::Toast(Notice::info("trailer playing in mpv")));
            // Reaped so a closed trailer never lingers as a zombie.
            let _ = child.wait().await;
            return;
        }
        let update = match host.open_url(&url) {
            Ok(()) => Event::Toast(Notice::info("trailer opened in your browser")),
            Err(e) => Event::Toast(Notice::alert(format!("could not open the trailer: {e}"))),
        };
        let _ = tx.send(update);
    });
}

/// Whether any of these executables is reachable on `PATH`.
///
/// Checked by looking rather than by running: the question is "will mpv find yt-dlp",
/// and mpv looks, it does not run `--version` first.
#[cfg(feature = "desktop")]
fn in_path(names: &[&str]) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        names
            .iter()
            .any(|name| dir.join(name).is_file() || dir.join(format!("{name}.exe")).is_file())
    })
}

/// Open a link in the browser.
pub fn open_external(url: String, services: &Services, tx: &Outbox) {
    if let Err(e) = services.host.open_url(&url) {
        let _ = tx.send(Event::Toast(Notice::alert(format!("could not open: {e}"))));
    }
}
