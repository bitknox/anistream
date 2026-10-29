//! Screenshots of the window, taken by the app itself: each shot puts the window in a state,
//! waits for the engine and the art to settle, and draws the window into a PNG. Run under the
//! fixture scene, so every run draws the same titles.
//!
//!   anistream-desktop --fixtures --shots <dir> --shot home,title,... [--size 1280x800]

use std::{path::PathBuf, rc::Rc, time::Duration};

use adw::prelude::*;
use anistream_model::{
    command::Command, event::Event, input::PadButton, sync::DeviceCodePrompt,
};
use gtk::glib;

use crate::{hub::SignIn, pages::Dest, window::Shell};

/// The title the title shots open: the fixture scene's finished title, part-way through
/// episode 7.
const TITLE: u32 = 900_001;
/// The title the player shot plays: episode 4 is the next, from its start.
const PLAYING: (u32, &str) = (900_002, "4");

/// Every shot by name, in the order a run takes them.
pub const ALL: [&str; 69] = [
    "home",
    "welcome",
    "offline",
    "calendar",
    "seasonal",
    "search",
    "search-results",
    "search-filters",
    "search-filters-none",
    "search-failed",
    "search-paging",
    "title",
    "trailer-preview",
    "trailer-window",
    "discussion",
    "discussion-none",
    "discussion-waiting",
    "discussion-signed-out",
    "title-discussion",
    "title-watch-again",
    "show-settings",
    "library",
    "library-focus",
    "downloads",
    "providers",
    "provider",
    "stores",
    "store",
    "store-menu",
    "store-invite",
    "store-members",
    "store-joining",
    "store-publish",
    "store-invite-name",
    "store-device",
    "store-members-loading",
    "store-members-empty",
    "store-members-failed",
    "store-joining-closed",
    "store-publish-busy",
    "store-publish-failed",
    "store-publish-empty",
    "join",
    "join-link",
    "settings",
    "settings-general",
    "settings-integrations",
    "settings-upscaling",
    "settings-accounts",
    "accounts-kitsu",
    "accounts-code",
    "torrents-setup",
    "torrents-none",
    "torrents-on",
    "torrents-edit-proxy",
    "torrents-checking",
    "torrents-check-failed",
    "torrents-saved",
    "settings-updates",
    "update-ready",
    "update-saved",
    "about",
    "player",
    "player-marks",
    "player-tracks",
    "player-undo",
    "mini",
    "resumed",
    "close",
];

/// The watch party's shots, apart from the rest: each holds a party on the scene's own server,
/// and they take long enough that a run of every shot would not fit its time. `--shot party`
/// takes them all.
pub const PARTY: [&str; 15] = [
    "party-form",
    "party-joining",
    "party-refused",
    "party-invite",
    "party-panel",
    "party-ready",
    "party-chat",
    "party-left",
    "party-lost",
    "party-close",
    "party-nosource",
    "party-choose",
    "party-unaired",
    "party-nosources",
    "party-title-menu",
];

/// Play on TV's shots, on the scene's two TVs on loopback: `--shot tv` takes them, in this
/// order, as the last ones switch the TVs off.
pub const TV: [&str; 17] = [
    "tv-empty",
    "tv-address",
    "tv-unreachable",
    "tv-slow",
    "tv-picker",
    "tv-code",
    "tv-wrong",
    "tv-expired",
    "tv-remote",
    "tv-bar",
    "tv-settings",
    "tv-player",
    "tv-unpaired",
    "tv-bar-unpaired",
    "tv-lost",
    "tv-bar-lost",
    "tv-connecting",
];

/// The source picker's shots, apart from the rest: `--shot sources` takes them.
pub const SOURCES: [&str; 7] = [
    "source-picker",
    "source-picker-pinned",
    "source-picker-releases",
    "source-picker-loading",
    "source-pin-unavailable",
    "source-unsourced",
    "source-unsourced-none",
];

/// The game controller's shots: the screens as a controller leaves them, with the focus ring
/// showing. `--shot pad` takes them.
pub const PAD: [&str; 7] = [
    "pad-continue",
    "pad-title",
    "pad-library",
    "pad-sidebar",
    "pad-help",
    "pad-help-player",
    "pad-settings",
];

/// Stats for nerds' shots, apart from the rest so a run of every shot keeps to its time:
/// `--shot stats` takes them.
pub const STATS: [&str; 5] = [
    "player-stats",
    "player-stats-waiting",
    "player-stats-buffering",
    "player-stats-hdr",
    "player-stats-copied",
];

/// Take `shots` into `dir`, each named `<prefix><shot>.png`, then quit.
pub fn run(shell: Rc<Shell>, dir: PathBuf, prefix: String, shots: Vec<String>) {
    glib::spawn_future_local(async move {
        // The first page, its lists and their covers.
        wait(4.0).await;
        if let Some(child) = shell.ctx.window.content() {
            wide_widgets(&child, 0, shell.ctx.window.width());
        }
        for shot in &shots {
            let settle = stage(&shell, shot);
            wait(settle).await;
            let path = dir.join(format!("{prefix}{shot}.png"));
            let mut taken = Err(String::new());
            // A window that has just changed its size or decorations can have no frame drawn
            // yet: asked to draw, and tried again.
            for _ in 0..4 {
                // Taken just after a frame is painted, when the window is laid out: between
                // frames a relayout can be pending, and a widget waiting for one draws nothing.
                taken = painted(&shell, &path, shot).await;
                if taken.is_ok() {
                    break;
                }
                wait(1.5).await;
            }
            match taken {
                Ok(()) => tracing::info!(path = %path.display(), "a shot"),
                Err(e) => tracing::error!(shot, error = %e, "a shot did not save"),
            }
            unstage(&shell, shot);
            wait(0.5).await;
        }
        shell.stop_and_close();
        if let Some(app) = shell.ctx.window.application() {
            app.quit();
        }
    });
}

/// Put the window in the state `shot` shows, and say how long to let it settle.
fn stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    let window = &shell.ctx.window;
    match shot {
        "home" => {
            shell.go(Dest::Continue);
            1.0
        }
        "welcome" => {
            crate::pages::first_launch::open(&shell.ctx);
            1.5
        }
        "offline" => {
            shell.ctx.hub.send(Command::NetworkPath {
                cellular: false,
                constrained: false,
                offline: true,
            });
            shell.go(Dest::Seasonal);
            2.0
        }
        "resumed" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(6), move || {
                    player.shot_resumed(724.0);
                });
            }
            7.5
        }
        "calendar" => {
            shell.go(Dest::Calendar);
            3.0
        }
        "seasonal" => {
            shell.go(Dest::Seasonal);
            2.5
        }
        "search" => {
            shell.go(Dest::Search);
            2.0
        }
        "search-results" => {
            shell.go(Dest::Search);
            shell.search_page().search_for("hoshikuzu");
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-filters" => {
            // Browsing without a word typed: a genre left out, two formats, a status.
            use anistream_model::{catalog::AiringStatus, search::*};
            shell.go(Dest::Search);
            let query = SearchQuery::default()
                .picking_genre("Romance", Pick::Exclude)
                .toggling_format(MediaFormat::Tv)
                .toggling_format(MediaFormat::Movie)
                .toggling_status(AiringStatus::Finished);
            shell.search_page().browse(&query);
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-filters-none" => {
            // Filters nothing matches: two genres wanted, a year.
            use anistream_model::search::{Pick, SearchQuery};
            shell.go(Dest::Search);
            let query = SearchQuery::default()
                .picking_genre("Action", Pick::Include)
                .picking_genre("Mecha", Pick::Include)
                .in_year(Some(2024));
            shell.search_page().browse(&query);
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-failed" => {
            // AniList did not answer: the engine's words, in place of the results.
            shell.go(Dest::Search);
            shell.search_page().search_for("hoshikuzu");
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                let Some(query) = shell.search_page().asked() else { return };
                shell.ctx.hub.take(vec![anistream_model::event::Event::Content {
                    view: anistream_model::catalog::ContentFor::Search { query },
                    content: anistream_model::catalog::ContentList::Failed(
                        "AniList didn't answer in time. Check the connection and try again."
                            .into(),
                    ),
                }]);
            });
            3.0
        }
        #[cfg(feature = "fixtures")]
        "search-paging" => {
            // Browsing TV series, the next page on its way at the end of the list.
            use anistream_model::search::{MediaFormat, SearchQuery};
            crate::showcase::set_mood(crate::showcase::Mood::Silent);
            shell.go(Dest::Search);
            shell
                .search_page()
                .browse(&SearchQuery::default().toggling_format(MediaFormat::Tv));
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(1800), move || {
                let Some(query) = shell.search_page().asked() else { return };
                shell.ctx.hub.take(vec![anistream_model::event::Event::SearchPaging {
                    query,
                    has_next: true,
                }]);
                shell.search_page().more();
            });
            3.0
        }
        "title" => {
            shell.go(Dest::Continue);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            3.5
        }
        "trailer-preview" => {
            // The pointer rests on a cover in Seasonal: its preview opens, and its trailer plays.
            shell.go(Dest::Seasonal);
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                let cover = mapped_grid(shell.ctx.window.upcast_ref())
                    .and_then(|grid| first_cover(grid.upcast_ref()))
                    .and_then(|fit| fit.parent());
                let id = shell
                    .ctx
                    .hub
                    .known
                    .borrow()
                    .entries
                    .values()
                    .find(|entry| entry.title == "The Library Above the Clouds")
                    .map(|entry| entry.id.get());
                if let (Some(cover), Some(id)) = (cover, id) {
                    crate::trailer::show_preview(&shell.ctx.hub, &cover, id);
                }
            });
            6.0
        }
        "trailer-window" => {
            shell.go(Dest::Continue);
            if let Some(entry) = shell.ctx.hub.entry(anistream_core::ids::AnilistId::new(TITLE))
            {
                crate::trailer::play_window(&shell.ctx, &entry);
            }
            4.0
        }
        #[cfg(feature = "fixtures")]
        "discussion" | "discussion-none" | "discussion-waiting" | "discussion-signed-out" => {
            use crate::showcase::Thread;
            shell.go(Dest::Continue);
            crate::showcase::set_thread(match shot {
                "discussion" => Thread::Comments,
                "discussion-none" => Thread::Nobody,
                "discussion-waiting" => Thread::Waiting,
                _ => Thread::Signed,
            });
            crate::pages::discussion::open(
                &shell.ctx,
                anistream_core::ids::AnilistId::new(TITLE),
                3,
            );
            3.0
        }
        "title-watch-again" => {
            // Every episode watched: the main button offers episode 1 again.
            shell.go(Dest::Continue);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                hub.take(vec![Event::Progress {
                    id: anistream_core::ids::AnilistId::new(TITLE),
                    progress: Some(anistream_model::progress::TitleProgress {
                        done: 12,
                        next: 1,
                    }),
                    resume: None,
                    episodes: Vec::new(),
                }]);
            });
            4.0
        }
        "title-discussion" => {
            // The episode that has a thread says so under its row.
            shell.go(Dest::Continue);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                hub.take(vec![Event::DiscussionFound {
                    media: anistream_core::ids::AnilistId::new(TITLE),
                    episode: 1,
                    comments: 214,
                }]);
            });
            4.0
        }
        "show-settings" => {
            shell.go(Dest::Continue);
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            let ctx = Rc::clone(&shell.ctx);
            glib::timeout_add_local_once(Duration::from_millis(2500), move || {
                if let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) {
                    let _ = WidgetExt::activate_action(&page, "title.show-settings", None);
                }
            });
            5.0
        }
        "library" => {
            shell.go(Dest::Library);
            3.0
        }
        "library-focus" => {
            // Moved to by keyboard: the first cover holds the focus ring.
            shell.go(Dest::Library);
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                if let Some(grid) = mapped_grid(window.upcast_ref()) {
                    window.set_focus_visible(true);
                    grid.grab_focus();
                }
            });
            3.0
        }
        "downloads" => {
            shell.go(Dest::Downloads);
            2.0
        }
        "providers" => {
            shell.go(Dest::Providers);
            2.0
        }
        "provider" => {
            shell.go(Dest::Providers);
            crate::pages::plugin::push_provider(&shell.ctx, "mukashi-archive");
            1.5
        }
        "stores" => {
            shell.go(Dest::Stores);
            2.0
        }
        "store" => {
            shell.go(Dest::Stores);
            crate::pages::stores::push_store(&shell.ctx, "hotaru");
            1.5
        }
        #[cfg(feature = "fixtures")]
        owner if owner.starts_with("store-") => store_owner(shell, owner),
        "join" => {
            join(shell);
            2.0
        }
        #[cfg(feature = "fixtures")]
        "join-link" => {
            // The invite opened from a link on a web page: the dialog comes up already checking.
            shell.open_link("anistream://plugins.hotaru.example/join/fixture#key");
            let hub = Rc::clone(&shell.ctx.hub);
            glib::timeout_add_local_once(Duration::from_millis(900), move || {
                hub.show_instead(vec![crate::showcase::invite()]);
            });
            2.0
        }
        "settings" => {
            shell.settings_pane("playback");
            1.5
        }
        "pad-continue" => {
            shell.go(Dest::Continue);
            pad_press(
                shell,
                &[PadButton::DpadDown, PadButton::DpadDown, PadButton::DpadDown],
                1500,
            );
            3.0
        }
        "pad-title" => {
            shell.go(Dest::Continue);
            if let Some(pad) = shell.pad() {
                pad.shot_enter();
            }
            let _ =
                WidgetExt::activate_action(window, "win.open-title", Some(&TITLE.to_variant()));
            3.5
        }
        "pad-library" => {
            shell.go(Dest::Library);
            pad_press(
                shell,
                &[PadButton::DpadDown, PadButton::DpadRight, PadButton::DpadRight],
                2200,
            );
            3.5
        }
        "pad-sidebar" => {
            shell.go(Dest::Continue);
            pad_press(shell, &[PadButton::DpadDown, PadButton::DpadLeft], 1500);
            3.0
        }
        "pad-help" => {
            shell.go(Dest::Continue);
            if let Some(pad) = shell.pad() {
                pad.shot_enter();
                pad.shot_help();
            }
            1.5
        }
        "pad-help-player" => {
            play(shell);
            let shell = Rc::clone(shell);
            glib::timeout_add_local_once(Duration::from_secs(5), move || {
                if let Some(pad) = shell.pad() {
                    pad.shot_enter();
                    pad.shot_help();
                }
            });
            7.0
        }
        "pad-settings" => {
            shell.ctx.prefs.set_controller_seen();
            shell.settings_pane("general");
            1.5
        }
        "settings-general" => {
            shell.settings_pane("general");
            1.5
        }
        "settings-integrations" => {
            shell.settings_pane("integrations");
            1.5
        }
        "settings-upscaling" => {
            shell.settings_pane("playback");
            shell.settings_upscaling();
            upscaling(shell, "natural");
            2.0
        }
        "settings-accounts" => {
            shell.settings_pane("accounts");
            1.5
        }
        "torrents-setup" => {
            shell.settings_pane("sources");
            crate::pages::torrents::setup(&shell.ctx, true, None, Rc::new(|| {}));
            1.5
        }
        "torrents-none" => {
            shell.settings_pane("sources");
            crate::pages::torrents::setup(
                &shell.ctx,
                true,
                Some(anistream_model::protection::TorrentProtection::None),
                Rc::new(|| {}),
            );
            if let Some(dialog) = shell.ctx.window.visible_dialog()
                && let Some(send) = default_button(&dialog)
            {
                send.emit_clicked();
            }
            1.5
        }
        "torrents-on" => {
            by_proxy(shell);
            shell.settings_pane("sources");
            3.0
        }
        #[cfg(feature = "fixtures")]
        "torrents-edit-proxy" => {
            by_proxy(shell);
            shell.settings_pane("sources");
            once_proxy_saved(shell, |ctx| {
                crate::pages::torrents::edit_proxy(ctx);
            });
            3.0
        }
        #[cfg(feature = "fixtures")]
        "torrents-checking" | "torrents-check-failed" | "torrents-saved" => {
            use anistream_model::sources::GuardView;
            by_proxy(shell);
            shell.settings_pane("sources");
            // Answered here, never through the proxy typed in.
            shell.ctx.hub.check_answer.replace(Some(match shot {
                "torrents-checking" => None,
                "torrents-check-failed" => Some(GuardView::Leaking),
                _ => Some(GuardView::Protected { exit: "proxy.example".into() }),
            }));
            once_proxy_saved(shell, |ctx| {
                let dialog = crate::pages::torrents::edit_proxy(ctx);
                let child = dialog.upcast_ref::<gtk::Widget>();
                for (title, text) in
                    [("Host", "proxy.example"), ("Port", "1080"), ("User name", "viewer")]
                {
                    if let Some(row) = entry_titled(child, title) {
                        row.set_text(text);
                    }
                }
                if let Some(send) = default_button(&dialog) {
                    send.emit_clicked();
                }
            });
            3.0
        }
        "settings-updates" => {
            shell.settings_pane("integrations");
            offer_update(shell);
            1.5
        }
        // The download rehearsed and checked: the row offers Restart to Update, and so does the
        // notice.
        "update-ready" => {
            shell.settings_pane("integrations");
            4.5
        }
        // The same from an AppImage in a folder the app can't write to: saved to Downloads, and
        // Show in Files in the row and the notice.
        #[cfg(feature = "fixtures")]
        "update-saved" => {
            shell.settings_pane("integrations");
            shell.ctx.updater.rehearse_unwritable();
            offer_update(shell);
            shell.ctx.updater.download();
            4.5
        }
        "accounts-kitsu" => {
            shell.settings_pane("accounts");
            crate::pages::accounts::kitsu(&shell.ctx, Rc::new(|| {}));
            1.5
        }
        "accounts-code" => {
            shell.ctx.hub.known.borrow_mut().signing.insert("simkl".into(), SignIn::Job(None));
            shell.settings_pane("accounts");
            let prompt = DeviceCodePrompt {
                tracker: "simkl".into(),
                code: "7KQ2M4".into(),
                url: "https://example.test/pin".into(),
                expires_at: Some(glib::real_time() / 1_000_000 + 14 * 60 + 59),
            };
            crate::pages::accounts::device_code(&shell.ctx, Some(&prompt));
            1.5
        }
        "about" => {
            let _ = WidgetExt::activate_action(window, "win.about", None);
            1.5
        }
        "player" => {
            play(shell);
            9.0
        }
        "player-marks" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_marks("/out/media/lantern-road.srt");
                });
                let player = Rc::clone(&shell.player);
                // The preview opens the stream the first time it shows and has its frame a
                // moment later; shown again then, so the controls are still up for the shot.
                glib::timeout_add_local_once(Duration::from_millis(6500), move || {
                    player.shot_preview(0.4);
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(8500), move || {
                    player.shot_preview(0.4);
                });
            }
            10.0
        }
        "player-tracks" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_tracks(Some("/out/media/lantern-road.srt"));
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(8000), move || {
                    player.shot_tracks(None);
                });
            }
            9.5
        }
        stats if stats.starts_with("player-stats") => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                use anistream_model::hosted::StatsFixture;
                // The live panel reads mpv itself; the others hold the engine's made-up states.
                let (at, fixture, waited, copied) = match stats {
                    "player-stats-waiting" => {
                        (0.3, Some(StatsFixture::Waiting), Some(5.0), false)
                    }
                    "player-stats-buffering" => {
                        (2.0, Some(StatsFixture::Buffering), None, false)
                    }
                    "player-stats-hdr" => (2.0, Some(StatsFixture::Hdr), None, false),
                    "player-stats-copied" => (2.0, Some(StatsFixture::Playing), None, true),
                    _ => (2.0, None, None, false),
                };
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs_f64(at), move || {
                    player.shot_stats(fixture, waited, copied);
                });
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_secs(4), move || {
                    player.shot_tracks(Some("/out/media/lantern-road.srt"));
                });
                // The short clip is near its end from the start: no Up Next card over the shot.
                shell.player.hold_up_next.set(true);
            }
            // Opened before the file has: taken while it is still starting.
            if stats == "player-stats-waiting" { 1.0 } else { 9.5 }
        }
        "player-undo" => {
            play(shell);
            #[cfg(feature = "fixtures")]
            {
                let player = Rc::clone(&shell.player);
                glib::timeout_add_local_once(Duration::from_millis(7500), move || {
                    player.shot_auto_skipped();
                });
            }
            9.0
        }
        "mini" => {
            play(shell);
            let player = Rc::clone(&shell.player);
            glib::timeout_add_local_once(Duration::from_secs(5), move || player.set_mini(true));
            9.0
        }
        "close" => {
            window.close();
            1.5
        }
        party if party.starts_with("party-") => party_stage(shell, party),
        source if source.starts_with("source-") => source_stage(shell, source),
        tv if tv.starts_with("tv-") => tv_stage(shell, tv),
        other => {
            tracing::warn!(shot = other, "no such shot");
            0.0
        }
    }
}

/// Leave the state a shot put the window in.
fn unstage(shell: &Rc<Shell>, shot: &str) {
    if shot.starts_with("pad-") {
        if let Some(pad) = shell.pad() {
            pad.shot_leave();
        }
        close_dialogs(&shell.ctx.window);
    }
    match shot {
        "title" | "pad-title" | "title-discussion" => {
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "trailer-preview" => crate::trailer::close_preview(),
        "trailer-window"
        | "discussion"
        | "discussion-none"
        | "discussion-waiting"
        | "discussion-signed-out" => close_dialogs(&shell.ctx.window),
        "about" | "join" | "join-link" | "welcome" | "accounts-kitsu" | "torrents-setup" => {
            close_dialogs(&shell.ctx.window)
        }
        #[cfg(feature = "fixtures")]
        "search-failed" | "search-paging" => {
            crate::showcase::set_mood(crate::showcase::Mood::Usual);
            shell.search_page().browse(&anistream_model::search::SearchQuery::default());
        }
        "torrents-none" => {
            close_dialogs(&shell.ctx.window);
            close_dialogs(&shell.ctx.window);
        }
        "torrents-on"
        | "torrents-edit-proxy"
        | "torrents-checking"
        | "torrents-check-failed"
        | "torrents-saved" => {
            close_dialogs(&shell.ctx.window);
            #[cfg(feature = "fixtures")]
            {
                shell.ctx.hub.check_answer.replace(None);
                crate::pages::torrents::forget_checks();
            }
            shell.ctx.hub.send(Command::SetSetting {
                key: anistream_model::settings::SettingKey::Torrents,
                value: anistream_core::settings::SettingValue::Bool(false),
            });
        }
        "accounts-code" => {
            crate::pages::accounts::device_code(&shell.ctx, None);
            shell.ctx.hub.known.borrow_mut().signing.remove("simkl");
        }
        "offline" => shell.ctx.hub.send(Command::NetworkPath {
            cellular: false,
            constrained: false,
            offline: false,
        }),
        "resumed" => shell.player.close(),
        #[cfg(feature = "fixtures")]
        "search-filters" | "search-filters-none" => {
            shell.search_page().browse(&anistream_model::search::SearchQuery::default())
        }
        #[cfg(feature = "fixtures")]
        stats if stats.starts_with("player-stats") => {
            shell.player.shot_stats_away();
            shell.player.hold_up_next.set(false);
            shell.player.close();
        }
        "show-settings" => {
            close_dialogs(&shell.ctx.window);
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "settings-upscaling" => {
            upscaling(shell, "off");
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        #[cfg(feature = "fixtures")]
        owner if owner.starts_with("store-") => {
            crate::showcase::set_mood(crate::showcase::Mood::Usual);
            if let Some(more) = more_button(&shell.ctx.window) {
                more.popdown();
            }
            close_dialogs(&shell.ctx.window);
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        "provider" | "store" => {
            if let Some(nav) = shell.ctx.nav() {
                nav.pop();
            }
        }
        "player-tracks" => {
            shell.player.close_menus();
            shell.player.close();
        }
        "player" | "player-marks" | "player-undo" | "mini" | "pad-help-player" => {
            shell.player.close()
        }
        party if party.starts_with("party-") => party_unstage(shell, party),
        source if source.starts_with("source-") => source_unstage(shell),
        tv if tv.starts_with("tv-") => tv_unstage(shell, tv),
        _ => {}
    }
}

/// A newer release with an AppImage for each machine, as the daily check tells of one: a scene
/// goes through the update's steps, fetching nothing.
fn offer_update(shell: &Rc<Shell>) {
    use anistream_model::updates::{Release, UpdateStatus};
    // As the daily check tells of a release with an AppImage for each machine: a scene
    // goes through the update's steps, fetching nothing.
    let appimage = |arch: &str| anistream_model::updates::ReleaseAsset {
        name: format!("anistream-2.1.0-linux-{arch}.AppImage"),
        url: format!("https://releases.example/v2.1.0/anistream-2.1.0-linux-{arch}.AppImage"),
        sha256: "5e".repeat(32),
        size: 118_400_000,
    };
    shell.ctx.hub.take(vec![anistream_model::event::Event::Update(UpdateStatus::Available {
        release: Box::new(Release {
            version: "2.1.0".into(),
            published_at: 0,
            page_url: "https://example.test/releases/v2.1.0".into(),
            notes: String::new(),
            macos: None,
            ios: None,
            android: None,
            android_tv: None,
            windows_x64: None,
            windows_arm64: None,
            linux_x86_64: Some(appimage("x86_64")),
            linux_aarch64: Some(appimage("aarch64")),
        }),
        notify: true,
    })]);
}

/// Set upscaling, which the fixture scene keeps off for the player shots.
fn upscaling(shell: &Rc<Shell>, mode: &str) {
    shell.ctx.hub.send(Command::SetSetting {
        key: anistream_model::settings::SettingKey::Upscaling,
        value: anistream_core::settings::SettingValue::Str(mode.into()),
    });
}

/// Play the player shots' episode, paused once it plays so the controls stay up.
/// A controller pressing `buttons` one after another, the first `after` milliseconds from now.
fn pad_press(shell: &Rc<Shell>, buttons: &[PadButton], after: u64) {
    let shell = Rc::clone(shell);
    let buttons = buttons.to_vec();
    glib::timeout_add_local_once(Duration::from_millis(after), move || {
        let Some(pad) = shell.pad() else { return };
        pad.shot_enter();
        for button in buttons {
            pad.shot_press(button);
        }
    });
}

fn play(shell: &Rc<Shell>) {
    let (id, episode) = PLAYING;
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.play",
        Some(&(id, episode.to_owned()).to_variant()),
    );
    let hub = Rc::clone(&shell.ctx.hub);
    glib::timeout_add_local_once(Duration::from_secs(7), move || {
        hub.send(Command::Player(anistream_model::command::PlayerCommand::TogglePause));
    });
}

/// The join dialog with an invite pasted, and the showcase's answer to checking it.
#[cfg(feature = "fixtures")]
fn join(shell: &Rc<Shell>) {
    crate::pages::join::open(&shell.ctx, Some(crate::showcase::INVITE));
    let hub = Rc::clone(&shell.ctx.hub);
    glib::timeout_add_local_once(Duration::from_millis(900), move || {
        hub.show_instead(vec![crate::showcase::invite()]);
    });
}

/// The showcase's store that this computer owns, with its owner's tools: the menu, or one of
/// its dialogs answered by the showcase.
#[cfg(feature = "fixtures")]
fn store_owner(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::{
        pages::store_owner,
        showcase::{Mood, set_mood},
    };
    let ctx = &shell.ctx;
    shell.go(Dest::Stores);
    crate::pages::stores::push_store(ctx, "mika");
    match shot {
        "store-menu" => {
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(800), move || {
                if let Some(more) = more_button(&window) {
                    more.popup();
                }
            });
        }
        "store-invite" => {
            store_owner::invite(ctx, "mika", false);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(field) = first::<adw::EntryRow>(dialog.upcast_ref()) {
                    field.set_text("Kai");
                }
                if let Some(make) = default_button(&dialog) {
                    make.emit_clicked();
                }
            });
        }
        "store-invite-name" => {
            store_owner::invite(ctx, "mika", false);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(field) = first::<adw::EntryRow>(dialog.upcast_ref()) {
                    field.set_text("Kai");
                }
            });
        }
        "store-device" => {
            store_owner::invite(ctx, "mika", true);
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                if let Some(make) = window.visible_dialog().as_ref().and_then(default_button) {
                    make.emit_clicked();
                }
            });
        }
        "store-members" => store_owner::members(ctx, "mika"),
        "store-members-loading" => {
            set_mood(Mood::Silent);
            store_owner::members(ctx, "mika");
        }
        "store-members-empty" => {
            set_mood(Mood::Bare);
            store_owner::members(ctx, "mika");
        }
        "store-members-failed" => {
            set_mood(Mood::Failing);
            store_owner::members(ctx, "mika");
        }
        "store-joining" => store_owner::joining(ctx, "mika"),
        "store-joining-closed" => {
            set_mood(Mood::Closed);
            store_owner::joining(ctx, "mika");
        }
        "store-publish" => store_owner::publish(ctx, "mika"),
        "store-publish-empty" => {
            set_mood(Mood::Bare);
            store_owner::publish(ctx, "mika");
        }
        // The first plugin's Publish pressed: the store still working on it, or refusing it.
        "store-publish-busy" | "store-publish-failed" => {
            if shot == "store-publish-failed" {
                set_mood(Mood::PublishFails);
            }
            store_owner::publish(ctx, "mika");
            let window = ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(1000), move || {
                let Some(dialog) = window.visible_dialog() else { return };
                if let Some(publish) = button_labelled(dialog.upcast_ref(), "Publish") {
                    publish.emit_clicked();
                }
            });
            return 2.5;
        }
        _ => {}
    }
    2.0
}

/// The first button under `widget` reading `label`, depth first.
#[cfg(feature = "fixtures")]
fn button_labelled(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Some(button) = widget.downcast_ref::<gtk::Button>()
        && button.label().as_deref() == Some(label)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = button_labelled(&next, label) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// The first entry row under `widget` titled `title`, depth first.
#[cfg(feature = "fixtures")]
fn entry_titled(widget: &gtk::Widget, title: &str) -> Option<adw::EntryRow> {
    if let Some(row) = widget.downcast_ref::<adw::EntryRow>()
        && row.title() == title
    {
        return Some(row.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = entry_titled(&next, title) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Run `then` once the proxy `by_proxy` sets has reached the app, so a form opens filled in.
#[cfg(feature = "fixtures")]
fn once_proxy_saved(shell: &Rc<Shell>, then: impl FnOnce(&Rc<crate::window::Ctx>) + 'static) {
    let ctx = Rc::clone(&shell.ctx);
    let mut then = Some(then);
    let mut tries = 0;
    glib::timeout_add_local(Duration::from_millis(100), move || {
        tries += 1;
        let saved = crate::pages::torrents::current(&ctx)
            .proxy
            .is_some_and(|proxy| proxy.host == "127.0.0.1");
        if !saved && tries < 40 {
            return glib::ControlFlow::Continue;
        }
        if let Some(then) = then.take() {
            then(&ctx);
        }
        glib::ControlFlow::Break
    });
}

/// Torrents on through a proxy nothing listens on: its check fails on this machine, and never
/// looks up where the run's own traffic leaves from.
fn by_proxy(shell: &Rc<Shell>) {
    use anistream_core::settings::SettingValue;
    use anistream_model::protection::{FailedCheck, TorrentProtection, TorrentProxy};
    shell.ctx.hub.send(Command::SetSetting {
        key: anistream_model::settings::SettingKey::TorrentIndex,
        value: SettingValue::Str("https://index.example.test/rss?q={query}".into()),
    });
    shell.ctx.hub.send(Command::SetTorrentProtection {
        mode: TorrentProtection::Proxy,
        proxy: Some(TorrentProxy {
            host: "127.0.0.1".into(),
            port: 9,
            user: None,
            password: None,
        }),
        on_failure: FailedCheck::Pause,
        acknowledged: false,
    });
    shell.ctx.hub.send(Command::SetSetting {
        key: anistream_model::settings::SettingKey::Torrents,
        value: SettingValue::Bool(true),
    });
}

/// The first widget of type `T` under `widget`, depth first.
#[cfg(feature = "fixtures")]
fn first<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> Option<T> {
    if let Some(found) = widget.downcast_ref::<T>() {
        return Some(found.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = first(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

#[cfg(not(feature = "fixtures"))]
fn join(shell: &Rc<Shell>) {
    crate::pages::join::open(&shell.ctx, None);
}

fn close_dialogs(window: &adw::ApplicationWindow) {
    if let Some(dialog) = window.visible_dialog() {
        dialog.force_close();
    }
}

/// A dialog's default button: what Return presses.
fn default_button(dialog: &adw::Dialog) -> Option<gtk::Button> {
    dialog.default_widget()?.downcast().ok()
}

/// Draw the window, dialogs and all, into a PNG at its own scale.
fn capture(
    window: &adw::ApplicationWindow,
    path: &std::path::Path,
    over: Option<(gtk::Widget, f32, f32)>,
) -> Result<(), String> {
    let (width, height) = (window.width(), window.height());
    if width <= 0 || height <= 0 {
        return Err("the window has no size".into());
    }
    // The window on its own first: with a popover on top, an empty window would still give a
    // picture, of the popover alone.
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let drawn = gtk::Snapshot::new();
    paintable.snapshot(&drawn, f64::from(width), f64::from(height));
    let page = drawn.to_node();
    let page = page.ok_or_else(|| {
        format!(
            "the window drew nothing: {width}x{height}, mapped {}, decorated {}, needs {}x{}",
            window.is_mapped(),
            window.is_decorated(),
            window.measure(gtk::Orientation::Horizontal, -1).0,
            window.measure(gtk::Orientation::Vertical, -1).0,
        )
    })?;
    let snapshot = gtk::Snapshot::new();
    snapshot.append_node(&page);
    // A popover is a surface of its own, which the window's drawing leaves out: drawn on top.
    if let Some((widget, x, y)) = over {
        let popover = gtk::WidgetPaintable::new(Some(&widget));
        snapshot.save();
        snapshot.translate(&gtk::graphene::Point::new(x, y));
        popover.snapshot(&snapshot, f64::from(widget.width()), f64::from(widget.height()));
        snapshot.restore();
    }
    let node = snapshot.to_node().ok_or("nothing drawn")?;
    let renderer = window.renderer().ok_or("no renderer")?;
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&bounds));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    texture.save_to_png(path).map_err(|e| e.to_string())
}

/// The window drawn into `path` just after its next frame is painted, with the shot's popover
/// on top; captured at once if no frame comes within a second.
async fn painted(shell: &Rc<Shell>, path: &std::path::Path, shot: &str) -> Result<(), String> {
    let window = shell.ctx.window.clone();
    let take = {
        let (player, window, path) =
            (Rc::clone(&shell.player), window.clone(), path.to_owned());
        let shot = shot.to_owned();
        move || {
            let over = match shot.as_str() {
                "player-marks" => player.shot_popover(),
                "player-tracks" => player.shot_tracks_popover(),
                "party-title-menu" | "store-menu" => title_menu_popover(&window),
                "trailer-preview" => crate::trailer::shot_preview(window.upcast_ref()),
                party if party.starts_with("party-") => player.shot_party_popover(),
                _ => None,
            };
            capture(&window, &path, over)
        }
    };
    let Some(clock) = window.frame_clock() else { return take() };
    let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
    let once = std::cell::Cell::new(Some(take.clone()));
    let handler = clock.connect_after_paint(move |_| {
        // Drawn outside the frame, so the capture's own rendering does not run inside the
        // window's, but before anything else on the loop can ask for a new layout.
        if let Some(take) = once.take() {
            let tx = tx.clone();
            glib::idle_add_local_full(glib::Priority::HIGH, move || {
                let _ = tx.try_send(take());
                glib::ControlFlow::Break
            });
        }
    });
    window.queue_draw();
    let taken = glib::future_with_timeout(Duration::from_secs(1), rx.recv()).await;
    clock.disconnect(handler);
    match taken {
        Ok(Ok(taken)) => taken,
        _ => take(),
    }
}

async fn wait(seconds: f64) {
    glib::timeout_future(Duration::from_secs_f64(seconds)).await;
}

/// The shots asked for: a comma-separated list, or every one.
pub fn named(list: Option<&str>) -> Vec<String> {
    match list {
        None | Some("all") => ALL.iter().map(|s| (*s).to_owned()).collect(),
        Some(list) => list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .flat_map(|s| match s {
                "party" => PARTY.iter().map(|p| (*p).to_owned()).collect(),
                "sources" => SOURCES.iter().map(|p| (*p).to_owned()).collect(),
                "pad" => PAD.iter().map(|p| (*p).to_owned()).collect(),
                "stats" => STATS.iter().map(|p| (*p).to_owned()).collect(),
                "tv" => TV.iter().map(|p| (*p).to_owned()).collect(),
                one => vec![one.to_owned()],
            })
            .collect(),
    }
}

/// Warn of each widget whose minimum width is over `limit`, the narrowest of a branch that is:
/// what keeps a window from fitting its size.
fn wide_widgets(widget: &gtk::Widget, depth: usize, limit: i32) {
    let (min, ..) = widget.measure(gtk::Orientation::Horizontal, -1);
    if min <= limit {
        return;
    }
    let mut over = false;
    let mut child = widget.first_child();
    while let Some(c) = child {
        if c.measure(gtk::Orientation::Horizontal, -1).0 > limit {
            over = true;
        }
        wide_widgets(&c, depth + 1, limit);
        child = c.next_sibling();
    }
    if !over {
        tracing::warn!(depth, min, kind = %widget.type_(), name = %widget.widget_name(), css = ?widget.css_classes(), "too wide");
    }
}

/// The room the party shots hold their party in.
#[cfg(feature = "fixtures")]
const ROOM: &str = "quiet-harbor-k7m2xq9ad4tnwe3p";

#[cfg(feature = "fixtures")]
thread_local! {
    /// The party shot's server, kept until the shot is taken.
    static PARTY_SERVER: std::cell::RefCell<Option<crate::fixtures::FakeParty>> =
        const { std::cell::RefCell::new(None) };
    /// The TV the slow pairing shot waits on, on this computer, which never answers.
    static SILENT_TV: std::cell::RefCell<Option<std::net::TcpListener>> =
        const { std::cell::RefCell::new(None) };
    /// The address the joining shot waits on, which never answers.
    static SILENT: std::cell::RefCell<Option<std::net::TcpListener>> =
        const { std::cell::RefCell::new(None) };
}

/// Put the window in the party state `shot` shows, and say how long to let it settle.
#[cfg(feature = "fixtures")]
fn party_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::{
        fixtures::FakeParty,
        party::{form, source},
    };
    use anistream_core::ids::AnilistId;
    use anistream_model::party::{NoSourceWhy, PartyInvite};

    let ctx = Rc::clone(&shell.ctx);
    let title = AnilistId::new(TITLE);
    let open_title = || {
        shell.go(Dest::Continue);
        let _ = WidgetExt::activate_action(
            &shell.ctx.window,
            "win.open-title",
            Some(&TITLE.to_variant()),
        );
    };
    let serve = |refuse: Option<&str>| {
        let party = FakeParty::start(refuse)?;
        let server = party.server();
        PARTY_SERVER.with(|p| p.replace(Some(party)));
        Some(server)
    };
    // Each scene's own notices, not the last one's still queued.
    shell.ctx.toasts.dismiss_all();
    match shot {
        "party-form" => {
            open_title();
            let ctx = Rc::clone(&ctx);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                form::open(&ctx, form::For::Episode(title, "7".into()));
            });
            3.5
        }
        "party-title-menu" => {
            open_title();
            let window = shell.ctx.window.clone();
            glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                if let Some(more) = more_button(&window) {
                    more.popup();
                }
            });
            3.5
        }
        "party-joining" | "party-refused" => {
            let server = if shot == "party-joining" {
                crate::fixtures::silent_server().map(|(listener, address)| {
                    SILENT.with(|s| s.replace(Some(listener)));
                    address
                })
            } else {
                serve(Some("this room takes members by invitation only"))
            };
            open_title();
            let ctx = Rc::clone(&ctx);
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                form::open(&ctx, form::For::Episode(title, "7".into()));
                form::rehearse(server, true);
            });
            4.5
        }
        "party-invite" | "party-unaired" => {
            let Some(server) = serve(None) else { return 0.0 };
            let (id, episode) =
                if shot == "party-invite" { (TITLE, "7") } else { (PLAYING.0, "6") };
            let link = PartyInvite {
                server,
                room: ROOM.into(),
                id: AnilistId::new(id),
                episode: episode.into(),
            }
            .link();
            shell.go(Dest::Continue);
            shell.open_link(&link);
            if shot == "party-unaired" {
                glib::timeout_add_local_once(Duration::from_millis(2500), || {
                    form::rehearse(None, true);
                });
                return 7.0;
            }
            3.5
        }
        "party-nosource" | "party-nosources" | "party-choose" => {
            let Some(server) = serve(None) else { return 0.0 };
            ctx.hub.send(Command::PartyJoin {
                server,
                room: ROOM.into(),
                name: Some("Ada".into()),
            });
            let ctx = Rc::clone(&ctx);
            let shot = shot.to_owned();
            glib::spawn_future_local(async move {
                for _ in 0..50 {
                    if ctx.hub.known.borrow().party.is_some() {
                        break;
                    }
                    wait(0.2).await;
                }
                match shot.as_str() {
                    "party-nosources" => source::open(
                        &ctx,
                        title,
                        "7",
                        NoSourceWhy::NoSources,
                        "no source is set up",
                    ),
                    "party-nosource" => source::open(
                        &ctx,
                        title,
                        "7",
                        NoSourceWhy::NotFound,
                        "could not match this title: sample-source has no episode 7",
                    ),
                    _ => {
                        source::rehearse_choose(&ctx, title, "7");
                        // Before the engine's own answer, which has no sources to give.
                        ctx.hub.take(vec![anistream_model::event::Event::Sources {
                            id: title,
                            episode: "7".into(),
                            candidates: choices(),
                        }]);
                    }
                }
            });
            4.0
        }
        playing => {
            let Some(server) = serve(None) else { return 0.0 };
            party_playing(shell, server, playing.to_owned());
            match playing {
                "party-lost" => 18.0,
                "party-left" | "party-ready" => 16.0,
                _ => 14.0,
            }
        }
    }
}

/// A party playing the title's episode 7, held paused at the start as a room waiting to be
/// ready is, with Mika ready and Ren not, then the state `shot` shows.
#[cfg(feature = "fixtures")]
fn party_playing(shell: &Rc<Shell>, server: String, shot: String) {
    use anistream_core::ids::AnilistId;
    use anistream_model::command::PlayerCommand;
    let shell = Rc::clone(shell);
    shell.player.hold_up_next.set(true);
    let started = std::time::Instant::now();
    // Until `at` seconds into the scene, so a notice is still up when the shot is taken.
    let until = move |at: f64| wait((at - started.elapsed().as_secs_f64()).max(0.0));
    glib::spawn_future_local(async move {
        let hub = Rc::clone(&shell.ctx.hub);
        hub.send(Command::PartyJoin { server, room: ROOM.into(), name: Some("Ada".into()) });
        hub.send(Command::Play {
            id: AnilistId::new(TITLE),
            episode: "7".into(),
            from_start: true,
        });
        // Until this device's member has the episode open, for the others to open the same.
        let mut you = None;
        for _ in 0..100 {
            wait(0.2).await;
            you = hub
                .known
                .borrow()
                .party_members
                .as_ref()
                .and_then(|m| m.iter().find(|m| m.you).cloned());
            if you.as_ref().is_some_and(|y| y.file.is_some() && y.duration.is_some()) {
                break;
            }
        }
        let file = you.as_ref().and_then(|y| y.file.clone()).unwrap_or_default();
        let length = you.and_then(|y| y.duration).unwrap_or(60.0);
        hub.send(Command::Player(PlayerCommand::TogglePause));
        PARTY_SERVER.with(|p| {
            let p = p.borrow();
            let Some(party) = p.as_ref() else { return };
            party.add(ROOM, "Mika");
            party.opens("Mika", &file, length);
            party.add(ROOM, "Ren");
            let other = file.replace("Episode 7", "Episode 8");
            party.opens("Ren", if shot == "party-panel" { &other } else { &file }, length);
            party.ready("Mika", true);
            party.ready("Ren", false);
        });
        wait(0.8).await;
        // The joins said, so the shot shows its own notice.
        shell.ctx.toasts.dismiss_all();
        let party = |act: &dyn Fn(&crate::fixtures::FakeParty)| {
            PARTY_SERVER.with(|p| {
                if let Some(party) = p.borrow().as_ref() {
                    act(party);
                }
            });
        };
        match shot.as_str() {
            "party-panel" => shell.player.show_party(),
            "party-ready" => {
                party(&|p| p.ready("Ren", true));
                wait(0.4).await;
                hub.send(Command::PartyReady { ready: true });
                wait(0.6).await;
                shell.player.show_party();
            }
            "party-chat" => {
                until(11.5).await;
                party(&|p| p.says("Mika", "Starting in a minute, grabbing tea"));
            }
            "party-left" => {
                until(11.5).await;
                shell.player.show_party();
                until(13.0).await;
                hub.send(Command::PartyLeave);
            }
            "party-lost" => {
                party(&|p| p.go_down(true));
                wait(2.5).await;
                shell.player.show_party();
            }
            "party-close" => shell.ctx.window.close(),
            _ => {}
        }
    });
}

/// Leave the party state a shot put the window in.
#[cfg(feature = "fixtures")]
fn party_unstage(shell: &Rc<Shell>, shot: &str) {
    shell.player.close_menus();
    if let Some(more) = more_button(&shell.ctx.window) {
        more.popdown();
    }
    // Twice: the alert, and the dialog it opened.
    close_dialogs(&shell.ctx.window);
    close_dialogs(&shell.ctx.window);
    shell.ctx.hub.send(Command::PartyLeave);
    if shell.is_player() {
        shell.player.close();
    }
    if !matches!(shot, "party-nosource" | "party-nosources" | "party-choose") {
        let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
    }
    shell.player.hold_up_next.set(false);
    shell.go(Dest::Continue);
    let toasts = shell.ctx.toasts.clone();
    glib::timeout_add_local_once(Duration::from_millis(400), move || {
        PARTY_SERVER.with(|p| p.replace(None));
        SILENT.with(|s| s.replace(None));
        toasts.dismiss_all();
    });
}

/// What the sources list for the choose shot's episode.
#[cfg(feature = "fixtures")]
fn choices() -> Vec<anistream_core::traits::SourceCandidate> {
    use anistream_core::traits::SourceCandidate;
    let choice =
        |id: &str, title: &str, quality, size: &str, seeders, auto_pick| SourceCandidate {
            id: id.into(),
            provider_id: "sample-source".into(),
            title: title.into(),
            quality: Some(quality),
            seeders: Some(seeders),
            size: Some(size.into()),
            dual_audio: false,
            dubbed: false,
            auto_pick,
        };
    vec![
        choice("a", "Stardust Wayfarer - 07 (1080p)", 1080, "1.4 GiB", 42, true),
        choice("b", "Stardust Wayfarer - 07 (720p)", 720, "690 MiB", 18, false),
        choice("c", "Stardust Wayfarer - 07 (480p)", 480, "310 MiB", 5, false),
    ]
}

#[cfg(not(feature = "fixtures"))]
fn party_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn party_unstage(_: &Rc<Shell>, _: &str) {}

/// The poster grid on screen.
/// The first cover under `widget`.
fn first_cover(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.is::<crate::fit::Fit>() {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = first_cover(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

fn mapped_grid(widget: &gtk::Widget) -> Option<gtk::GridView> {
    if let Some(grid) = widget.downcast_ref::<gtk::GridView>()
        && grid.is_mapped()
    {
        return Some(grid.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = mapped_grid(&next) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Play on TV's scenes. Each starts from the window as it is between shots: no sheet up and no
/// TV driven.
#[cfg(feature = "fixtures")]
fn tv_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use crate::tv::{
        fixture::{self, BEDROOM, LIVING_ROOM},
        sheet,
    };
    let Some(scene) = fixture::scene() else { return 0.0 };
    let tv = Rc::clone(&shell.ctx.tv);
    let later = |ms: u64, f: Box<dyn FnOnce()>| {
        glib::timeout_add_local_once(Duration::from_millis(ms), f);
    };
    // A paired TV, then Play on TV for episode 3 from 5:12, chosen in the picker: the remote.
    let drive = |name: &'static str| {
        if let Some(link) = scene.pair_link(name) {
            tv.pair_quietly(&link);
        }
        let window = shell.ctx.window.clone();
        later(
            1500,
            Box::new(move || {
                let _ = WidgetExt::activate_action(
                    &window,
                    "win.play-on-tv",
                    Some(&(TITLE, "3".to_owned(), 312.0).to_variant()),
                );
            }),
        );
        let tv = Rc::clone(&tv);
        later(
            2500,
            Box::new(move || {
                if let Some(row) = tv.rows().into_iter().find(|r| r.name == name) {
                    tv.choose(&row);
                }
            }),
        );
    };
    shell.go(Dest::Continue);
    match shot {
        "tv-empty" => {
            scene.silence(true);
            tv.open_picker();
            11.5
        }
        "tv-address" => {
            scene.silence(true);
            tv.open_picker();
            later(800, Box::new(|| sheet::shot_address("192.168.1.20:47811")));
            2.0
        }
        "tv-unreachable" => {
            scene.silence(true);
            tv.open_picker();
            // Nothing listens on port 1 of this computer: refused at once.
            let tv = Rc::clone(&tv);
            later(500, Box::new(move || tv.begin("127.0.0.1:1")));
            3.0
        }
        "tv-slow" => {
            // A TV on this computer that takes the connection and never answers, as one on a
            // slow network: nothing leaves the machine.
            scene.silence(true);
            tv.open_picker();
            if let Some((listener, address)) = crate::fixtures::silent_server() {
                SILENT_TV.with(|s| s.replace(Some(listener)));
                let tv = Rc::clone(&tv);
                later(500, Box::new(move || tv.begin(&address)));
            }
            3.0
        }
        "tv-picker" => {
            if let Some(link) = scene.pair_link(BEDROOM) {
                tv.pair_quietly(&link);
            }
            let _ = WidgetExt::activate_action(
                &shell.ctx.window,
                "win.open-title",
                Some(&TITLE.to_variant()),
            );
            let window = shell.ctx.window.clone();
            later(
                1500,
                Box::new(move || {
                    let _ = WidgetExt::activate_action(
                        &window,
                        "win.play-on-tv",
                        Some(&(TITLE, "7".to_owned(), 0.0).to_variant()),
                    );
                }),
            );
            4.0
        }
        "tv-code" | "tv-wrong" | "tv-expired" => {
            tv.open_picker();
            // Chosen once the browse lists it, as a click on its row would.
            let begin = Rc::clone(&tv);
            later(
                1000,
                Box::new(move || {
                    if let Some(row) = begin.rows().into_iter().find(|r| r.name == LIVING_ROOM)
                    {
                        begin.choose(&row);
                    }
                }),
            );
            let tries = match shot {
                "tv-code" => {
                    later(2000, Box::new(|| sheet::shot_code("482")));
                    return 3.0;
                }
                "tv-wrong" => 1,
                _ => 4,
            };
            for n in 0..tries {
                later(2000 + n * 1500, Box::new(|| sheet::shot_code("000000")));
            }
            2.5 + tries as f64 * 1.5
        }
        "tv-remote" => {
            drive(LIVING_ROOM);
            8.0
        }
        "tv-bar" => {
            drive(LIVING_ROOM);
            let window = shell.ctx.window.clone();
            later(7000, Box::new(move || close_dialogs(&window)));
            8.5
        }
        "tv-settings" => {
            drive(LIVING_ROOM);
            let window = shell.ctx.window.clone();
            let settings = Rc::clone(shell);
            later(
                6000,
                Box::new(move || {
                    close_dialogs(&window);
                    settings.settings_pane("integrations");
                }),
            );
            8.0
        }
        "tv-player" => {
            play(shell);
            9.0
        }
        "tv-unpaired" => {
            drive(LIVING_ROOM);
            later(6000, Box::new(move || scene.revoke_all(LIVING_ROOM)));
            9.0
        }
        "tv-bar-unpaired" | "tv-bar-lost" => {
            // Driven, the sheet closed, then the TV forgets this computer or goes away: the
            // sidebar says so.
            let lost = shot == "tv-bar-lost";
            let name = if lost { BEDROOM } else { LIVING_ROOM };
            drive(name);
            let window = shell.ctx.window.clone();
            later(6000, Box::new(move || close_dialogs(&window)));
            later(
                6500,
                Box::new(move || {
                    if lost {
                        scene.switch_off(name);
                    } else {
                        scene.revoke_all(name);
                    }
                }),
            );
            9.5
        }
        "tv-lost" => {
            drive(LIVING_ROOM);
            later(6000, Box::new(move || scene.switch_off(LIVING_ROOM)));
            9.0
        }
        "tv-connecting" => {
            // Paired before, and away now: the remote waits on it, holding what was asked.
            if let Some(link) = scene.pair_link(BEDROOM) {
                tv.pair_quietly(&link);
            }
            later(1000, Box::new(move || scene.switch_off(BEDROOM)));
            let window = shell.ctx.window.clone();
            later(
                1800,
                Box::new(move || {
                    let _ = WidgetExt::activate_action(
                        &window,
                        "win.play-on-tv",
                        Some(&(TITLE, "3".to_owned(), 312.0).to_variant()),
                    );
                }),
            );
            let tv = Rc::clone(&tv);
            later(
                2600,
                Box::new(move || {
                    if let Some(row) = tv.rows().into_iter().find(|r| r.name == BEDROOM) {
                        tv.choose(&row);
                    }
                }),
            );
            5.0
        }
        _ => 0.0,
    }
}

#[cfg(feature = "fixtures")]
fn tv_unstage(shell: &Rc<Shell>, shot: &str) {
    SILENT_TV.with(|s| s.replace(None));
    // As Done would: the model lets the sheet go, then anything else up closes.
    shell.ctx.tv.sheet_closed();
    close_dialogs(&shell.ctx.window);
    shell.ctx.tv.cancel_pairing();
    shell.ctx.tv.disconnect();
    if let Some(scene) = crate::tv::fixture::scene() {
        scene.silence(false);
    }
    match shot {
        "tv-picker" => {
            let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
        }
        "tv-player" => shell.player.close(),
        _ => {}
    }
}

#[cfg(not(feature = "fixtures"))]
fn tv_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn tv_unstage(_: &Rc<Shell>, _: &str) {}

/// The title page's More button, on the page on screen.
fn more_button(window: &adw::ApplicationWindow) -> Option<gtk::MenuButton> {
    fn find(widget: &gtk::Widget) -> Option<gtk::MenuButton> {
        if let Some(button) = widget.downcast_ref::<gtk::MenuButton>()
            && button.tooltip_text().as_deref() == Some("More")
            && button.is_mapped()
        {
            return Some(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(found) = find(&next) {
                return Some(found);
            }
            child = next.next_sibling();
        }
        None
    }
    find(window.upcast_ref())
}

/// The More menu's popover, under its button, over the window.
fn title_menu_popover(window: &adw::ApplicationWindow) -> Option<(gtk::Widget, f32, f32)> {
    let button = more_button(window)?;
    let popover = button.popover()?;
    if !popover.is_visible() {
        return None;
    }
    let bounds = button.compute_bounds(window)?;
    let width = popover.width() as f32;
    let x = (bounds.x() + bounds.width() / 2.0 - width / 2.0)
        .clamp(0.0, (window.width() as f32 - width).max(0.0));
    Some((popover.upcast(), x, bounds.y() + bounds.height()))
}

/// What the sources have of the title the source picker's shots open: all of it, part of it,
/// failed, off, and a torrent source with a batch and singles. `pinned` is the source pinned, and
/// `release` the one of its releases.
#[cfg(feature = "fixtures")]
fn source_options(
    pinned: Option<&str>,
    release: Option<&str>,
) -> Vec<anistream_model::sources::SourceOption> {
    use anistream_core::traits::SourceCandidate;
    use anistream_model::sources::{SourceNote, SourceOption};
    let candidate = |id: &str, title: &str, quality, seeders, size: &str| SourceCandidate {
        id: id.into(),
        provider_id: "torrent".into(),
        title: title.into(),
        quality: Some(quality),
        seeders: Some(seeders),
        size: Some(size.into()),
        dual_audio: false,
        dubbed: false,
        auto_pick: false,
    };
    let option = |id: &str, name: &str, covered, note, releases| SourceOption {
        provider_id: id.into(),
        name: name.into(),
        covered,
        of: 13,
        pinned: pinned == Some(id),
        note,
        releases,
        pinned_release: release.filter(|_| pinned == Some(id)).map(str::to_owned),
    };
    vec![
        option("hotaru-open-archive", "Hotaru Open Archive", 13, None, Vec::new()),
        option("mukashi-archive", "Mukashi Archive", 9, None, Vec::new()),
        option(
            "torrent",
            "Torrents",
            6,
            None,
            vec![
                candidate(
                    "batch-1080",
                    "Hoshikuzu no Tabibito (Batch) 1080p",
                    1080,
                    84,
                    "14.2 GiB",
                ),
                candidate(
                    "batch-720",
                    "Hoshikuzu no Tabibito (Batch) 720p",
                    720,
                    31,
                    "6.8 GiB",
                ),
                candidate(
                    "single-1080",
                    "Hoshikuzu no Tabibito - 01 1080p",
                    1080,
                    12,
                    "1.1 GiB",
                ),
            ],
        ),
        option(
            "machi-library-films",
            "Machi Library Films",
            0,
            Some(SourceNote::Failed { reason: "the site did not answer in 15 s.".into() }),
            Vec::new(),
        ),
        option(
            "sora-festival",
            "Sora Animation Festival",
            0,
            Some(SourceNote::Off),
            Vec::new(),
        ),
    ]
}

/// The source picker's scenes: the title's page, then the picker over it.
#[cfg(feature = "fixtures")]
fn source_stage(shell: &Rc<Shell>, shot: &str) -> f64 {
    use anistream_core::ids::AnilistId;
    use anistream_model::event::Event;

    let hub = Rc::clone(&shell.ctx.hub);
    shell.ctx.toasts.dismiss_all();
    let unsourced = shot.starts_with("source-unsourced");
    let title = if unsourced { PLAYING.0 } else { TITLE };
    hub.known.borrow_mut().source_options.clear();
    let listing = match shot {
        "source-picker-pinned" => Some((source_options(Some("mukashi-archive"), None), false)),
        "source-picker-releases" => {
            Some((source_options(Some("torrent"), Some("batch-720")), false))
        }
        "source-picker-loading" => Some((Vec::new(), true)),
        _ => Some((source_options(None, None), false)),
    };
    hub.listing.replace(listing);
    if unsourced {
        // The next episode, or it and every one after, is listed by no source.
        hub.unsourced.set(Some((title, 4, if shot == "source-unsourced" { 4 } else { 99 })));
    }
    shell.go(Dest::Continue);
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.open-title",
        Some(&title.to_variant()),
    );
    let ctx = Rc::clone(&shell.ctx);
    let shot = shot.to_owned();
    glib::timeout_add_local_once(Duration::from_millis(2500), move || match shot.as_str() {
        "source-pin-unavailable" => ctx.hub.take(vec![Event::PinUnavailable {
            id: AnilistId::new(title),
            provider_id: "mukashi-archive".into(),
            name: "Mukashi Archive".into(),
        }]),
        "source-unsourced" | "source-unsourced-none" => {
            // The page scrolled to the dimmed episode.
            if let Some(scroll) = ctx
                .nav()
                .and_then(|nav| nav.visible_page())
                .and_then(|page| first::<gtk::ScrolledWindow>(page.upcast_ref()))
            {
                scroll.vadjustment().set_value(330.0);
            }
        }
        _ => {
            if let Some(page) = ctx.nav().and_then(|nav| nav.visible_page()) {
                let _ = WidgetExt::activate_action(&page, "title.source-picker", None);
            }
        }
    });
    5.0
}

/// Leave the source picker's scene: the picker closed, the title's page left, the engine's own
/// answers back.
#[cfg(feature = "fixtures")]
fn source_unstage(shell: &Rc<Shell>) {
    close_dialogs(&shell.ctx.window);
    shell.ctx.hub.listing.replace(None);
    shell.ctx.hub.unsourced.set(None);
    shell.ctx.toasts.dismiss_all();
    let _ = WidgetExt::activate_action(&shell.ctx.window, "win.back", None);
}

#[cfg(not(feature = "fixtures"))]
fn source_stage(_: &Rc<Shell>, _: &str) -> f64 {
    0.0
}

#[cfg(not(feature = "fixtures"))]
fn source_unstage(_: &Rc<Shell>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shots_are_named_or_all() {
        assert_eq!(named(Some("home, title")), ["home", "title"]);
        assert_eq!(named(None).len(), ALL.len());
        assert_eq!(named(Some("home,party")).len(), PARTY.len() + 1);
    }
}
