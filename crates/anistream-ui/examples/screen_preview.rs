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
    app.apply(app.content_here(Content::Entries(vec![entry.clone()])));
    app.detail = Some(entry.clone());
    app.apply(app.episodes_here(vec![EpisodeRow {
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
        deferred: 0,
        needs_reauth: false,
        last: Some("pulled 214 titles".into()),
        pull_error: None,
    })));
    app.go_to_section(anistream_ui::Section::Library);
    app.apply(app.content_here(Content::Entries(vec![
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

    plugin_stores();

    app.nav.push(StageView::NowPlaying);
    app.playing = Some(NowPlaying {
        id: None,
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

/// Providers, the store list, one store as its owner sees it, its members, and the overlays.
fn plugin_stores() {
    use anistream_ui::app::{
        InviteLinkView, Secret, StoreInviteRow, StoreMemberRow, StorePluginRow,
        StorePluginStatus as S, StorePreview, StoreRow, StoreState,
    };
    use anistream_ui::{Overlay, Section};

    let mut app = App::new(Config::default(), Palette::dark(), Keymap::new());
    let plugin = |id: &str, name: &str, version: &str, hosts: &[&str], status| StorePluginRow {
        id: id.into(),
        name: name.into(),
        version: version.into(),
        hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
        status,
    };
    app.go_to_section(Section::Providers);
    dump("providers · nothing joined", &app);

    app.apply(Update::Stores(vec![
        StoreRow {
            id: "aaaa".into(),
            name: "Our plugins".into(),
            url: "https://plugins.example".into(),
            fingerprint: "489f 49ea 0372 41ac".into(),
            member: "Johan".into(),
            owner: true,
            state: StoreState::Ready,
            plugins: vec![
                plugin("example", "Example", "1.4.0", &["api.example"], S::Installed),
                plugin(
                    "catalogue",
                    "Catalogue",
                    "2.1.0",
                    &["catalogue.example", "img.example"],
                    S::Update { from: "2.0.3".into(), new_hosts: vec!["img.example".into()] },
                ),
                plugin("subs", "Subtitles", "0.3.0", &["subs.example"], S::Available),
            ],
        },
        StoreRow {
            id: "bbbb".into(),
            name: "Sam's sources".into(),
            url: "https://sam.example".into(),
            fingerprint: "0c41 77de 1a2b 9f3c".into(),
            member: "Johan".into(),
            owner: false,
            state: StoreState::Ready,
            plugins: vec![plugin("other", "Other", "1.0.0", &["other.example"], S::Available)],
        },
    ]));
    dump("providers · two stores joined", &app);

    app.handle(anistream_ui::Action::Open, 20);
    dump("plugin stores", &app);

    app.nav.push(StageView::Store("aaaa".into()));
    app.store_line_selected = 1;
    dump("one store, as its owner", &app);

    app.show_plugin("aaaa", "catalogue");
    dump("plugin overlay · an update that adds a host", &app);
    app.nav.close_overlay();

    app.invite_link = Some(InviteLinkView {
        store: "aaaa".into(),
        link: Secret(
            "https://plugins.example/join/3cUaTZ67FSY-V64hjqO1Lg#3fyhjW3QkgK6udJFyp6nl7pINInnv8n6iRK3RGsqLoI"
                .into(),
        ),
        for_whom: Some("Alex".into()),
        expires_at: 4_102_444_800,
        copied: true,
    });
    app.nav.open_overlay(Overlay::InviteLink);
    dump("invite link", &app);
    app.nav.close_overlay();

    app.members_for = Some("aaaa".into());
    app.store_members = vec![
        StoreMemberRow {
            id: "1".into(),
            name: "Johan".into(),
            owner: true,
            devices: 2,
            last_seen: Some(4_102_444_800),
            you: true,
        },
        StoreMemberRow {
            id: "2".into(),
            name: "Alex".into(),
            owner: false,
            devices: 1,
            last_seen: Some(1_700_000_000),
            you: false,
        },
    ];
    app.store_invites = vec![StoreInviteRow {
        id: "3".into(),
        name: "Sam".into(),
        device: false,
        expires_at: 4_102_444_800,
    }];
    app.nav.push(StageView::StoreMembers("aaaa".into()));
    app.member_selected = 1;
    dump("members", &app);

    app.go_to_section(Section::Providers);
    app.handle(anistream_ui::Action::Open, 20);
    app.store_confirm = Some(anistream_ui::app::StoreConfirm::Join(StorePreview {
        link: Secret(String::new()),
        name: "Our plugins".into(),
        url: "https://plugins.example".into(),
        fingerprint: "489f 49ea 0372 41ac".into(),
        already_joined: false,
    }));
    app.nav.open_overlay(Overlay::StoreConfirm);
    dump("joining · confirm", &app);
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
