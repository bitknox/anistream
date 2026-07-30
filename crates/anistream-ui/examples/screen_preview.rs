//! Dump screens as plain text, so a layout can be *looked at* rather than imagined.
//!
//! Run with `cargo run -p anistream-ui --example screen_preview`. Colour is dropped — this is
//! for checking composition, truncation and alignment, which is where layout bugs actually
//! live.

use anistream_core::{config::Config, ids::AnilistId};
use anistream_ui::{
    Keymap, Palette,
    app::{App, Content, Entry, EpisodeRow, NowPlaying, Update},
    nav::StageView,
    screens,
};

const WIDTH: u16 = 100;
const HEIGHT: u16 = 24;

fn main() {
    let mut app = App::new(Config::default(), Palette::dark(), Keymap::new());
    let entry = Entry {
        secondary: Some("Frieren: Beyond Journey's End".into()),
        format: Some("TV".into()),
        episodes: Some(28),
        year: Some(2023),
        score: Some(91),
        studio: Some("Madhouse".into()),
        progress: Some((11, 12)),
        ..Entry::new(AnilistId::new(154_587), "Sousou no Frieren")
    };
    app.apply(Update::Content(Content::Entries(vec![entry.clone()])));
    app.detail = Some(entry.clone());
    app.apply(Update::Episodes(vec![EpisodeRow {
        number: "11".into(),
        title: Some("Frieren the Slayer".into()),
        duration_secs: Some(1435),
        watched: 0.38,
        completed: false,
        kind: None,
        skippable: false,
        thumbnail: None,
        description: None,
    }]));

    // Library, with a tracker connected and a queue waiting.
    app.apply(Update::Sync(Box::new(anistream_ui::SyncState {
        tracker: "anilist".into(),
        connected: true,
        user: Some("johan".into()),
        storage: Some("OS keychain".into()),
        storage_degraded: false,
        outbox: 2,
        needs_reauth: false,
        last: Some("pulled 214 titles".into()),
    })));
    app.go_to_section(anistream_ui::Section::Library);
    app.apply(Update::Content(Content::Entries(vec![
        Entry { progress: Some((11, 12)), episodes: Some(28), ..entry.clone() },
        Entry {
            progress: Some((4, 5)),
            episodes: Some(12),
            ..Entry::new(AnilistId::new(1), "Dandadan")
        },
    ])));
    app.library_segment = anistream_ui::LibrarySegment::Watching;
    dump("library", &app);

    app.nav.open_overlay(anistream_ui::Overlay::Accounts);
    dump("accounts overlay", &app);
    app.nav.close_overlay();

    app.apply(Update::Conflicts(vec![
        anistream_ui::ConflictRow {
            anilist_id: AnilistId::new(154_587),
            title: "Sousou no Frieren".into(),
            field: "status".into(),
            local: "Completed".into(),
            remote: "Current".into(),
        },
        anistream_ui::ConflictRow {
            anilist_id: AnilistId::new(1),
            title: "Dandadan".into(),
            field: "score".into(),
            local: "9".into(),
            remote: "7".into(),
        },
    ]));
    app.nav.open_overlay(anistream_ui::Overlay::Conflicts);
    dump("conflicts overlay", &app);
    app.nav.close_overlay();
    app.nav.open_overlay(anistream_ui::Overlay::ListStatus);
    dump("list status overlay", &app);
    app.nav.close_overlay();

    app.nav.push(StageView::NowPlaying);
    app.playing = Some(NowPlaying {
        title: "Sousou no Frieren".into(),
        episode: "11".into(),
        episode_title: Some("Frieren the Slayer".into()),
        position: 552.0,
        duration: Some(1435.0),
        paused: false,
        speed: 1.0,
        skip: None,
    });
    dump("now playing", &app);

    if let Some(playing) = &mut app.playing {
        playing.skip = Some(("opening", 93.2));
        playing.paused = true;
        playing.speed = 1.25;
    }
    dump("now playing · paused, skip offered", &app);

    // The eyecatch, sampled across its sweep.
    app.eyecatch = Some(anistream_ui::Eyecatch::new("Sousou no Frieren  ·  ep 012"));
    for frame in 0..=(anistream_ui::eyecatch::SWEEP_FRAMES + 1) {
        if frame == 4 || frame == 7 || frame == anistream_ui::eyecatch::SWEEP_FRAMES {
            dump(&format!("eyecatch frame {frame}"), &app);
        }
        app.tick_animation();
    }
}

fn dump(label: &str, app: &App) {
    let buf = screens::render_to_buffer(app, WIDTH, HEIGHT);
    println!("\n\x1b[1m── {label} ──\x1b[0m");
    println!("┌{}┐", "─".repeat(WIDTH as usize));
    for y in 0..HEIGHT {
        let mut line = String::new();
        for x in 0..WIDTH {
            line.push_str(buf[(x, y)].symbol());
        }
        println!("│{line}│");
    }
    println!("└{}┘", "─".repeat(WIDTH as usize));
}
