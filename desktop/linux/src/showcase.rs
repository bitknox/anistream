//! Rows the fixture engine has none of, for screenshots of the screens that show them: providers
//! from plugin stores, the stores themselves, episodes seeding and an invite to check. Invented,
//! as the Mac's showcase invents them, and only ever on the fixture engine.

use anistream_model::{
    command::Command,
    downloads::SeedRow,
    event::Event,
    sources::{ProviderOrigin, ProviderRow, ProviderUpdate},
    stores::{
        AnonymousDeviceRow, InviteLinkView, InvitePreview, InvitedStore, LocalPluginRow,
        OpenJoinView, Secret, StoreInviteRow, StoreMemberRow, StorePluginRow,
        StorePluginStatus, StoreRow, StoreState,
    },
};

/// How the showcase's stores answer, for a shot of each state an owner's dialog has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mood {
    #[default]
    Usual,
    /// Nothing comes back: a store still thinking, and a next page of search on its way.
    Silent,
    /// Every question fails, as with the store's computer off.
    Failing,
    /// Only publishing fails.
    PublishFails,
    /// No invites waiting, and no plugins on this computer.
    Bare,
    /// Only people invited can join.
    Closed,
}

thread_local! {
    static MOOD: std::cell::Cell<Mood> = const { std::cell::Cell::new(Mood::Usual) };
}

/// Answer as `mood` until set again.
pub fn set_mood(mood: Mood) {
    MOOD.with(|m| m.set(mood));
}

/// The link the join shot pastes.
pub const INVITE: &str = "https://plugins.hotaru.example/join/fixture#key";

/// What `--showcase` shows in place of the engine's own.
pub fn events() -> Vec<Event> {
    vec![Event::Providers(providers()), Event::Stores(stores()), Event::Seeding(seeding())]
}

/// The answer to checking [`INVITE`].
pub fn invite() -> Event {
    let hotaru = stores().remove(0);
    Event::StorePreview(InvitePreview::Store(InvitedStore {
        link: Secret(INVITE.into()),
        name: "Hotaru Plugins".into(),
        url: "https://plugins.hotaru.example".into(),
        host: "plugins.hotaru.example".into(),
        fingerprint: "9f3c 1a2b 77de 0c41".into(),
        already_joined: false,
        owners: vec!["Ren".into()],
        plugins: hotaru
            .plugins
            .into_iter()
            .map(|p| StorePluginRow { status: StorePluginStatus::Available, ..p })
            .collect(),
        open: false,
        anonymous: false,
        device_label: "Computer".into(),
    }))
}

/// What a store's owner is answered, for the showcase's stores: who is in Mika's store, who can
/// join it, the plugins this computer could publish there and an invite link. `None` for any
/// other command, which goes to the engine.
pub fn answer(command: &Command) -> Option<Vec<Event>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let ours = |store: &str| stores().iter().any(|s| s.id == store);
    let members = |store: &str| Event::StoreMembers {
        store: store.into(),
        members: vec![
            member("mika", "Mika", true, 2, true),
            member("ren", "Ren", false, 1, false),
            member("sora", "Sora", false, 3, false),
        ],
        invites: vec![
            StoreInviteRow {
                id: "kai".into(),
                name: "Kai".into(),
                device: false,
                expires_at: now + 5 * 86_400,
            },
            StoreInviteRow {
                id: "device".into(),
                name: String::new(),
                device: true,
                expires_at: now + 86_400,
            },
        ],
    };
    let open_join = |store: &str, open: bool| Event::OpenJoin {
        store: store.into(),
        view: OpenJoinView {
            open,
            link: open
                .then(|| Secret("https://plugins.mika.example/join/open?open#key".into())),
            devices: 3,
            recent: vec![
                AnonymousDeviceRow {
                    id: "a".into(),
                    joined_at: now - 2 * 86_400,
                    last_seen: Some(now - 3 * 3_600),
                },
                AnonymousDeviceRow {
                    id: "b".into(),
                    joined_at: now - 6 * 86_400,
                    last_seen: None,
                },
            ],
        },
    };
    let mood = MOOD.with(std::cell::Cell::get);
    let owners = matches!(
        command,
        Command::CreateInvite { .. }
            | Command::LoadMembers { .. }
            | Command::LoadLocalPlugins { .. }
            | Command::PublishPlugin { .. }
    );
    let failed = |why: &str| {
        vec![Event::Toast(anistream_model::notice::Notice::alert(
            anistream_model::Msg::verbatim(why.to_owned()),
        ))]
    };
    match (mood, command) {
        (Mood::Silent, Command::SearchMore { .. }) => return Some(Vec::new()),
        (Mood::Silent, _) if owners => return Some(Vec::new()),
        (Mood::Failing, _) if owners => {
            return Some(failed(
                "couldn't reach plugins.mika.example: the connection timed out",
            ));
        }
        (Mood::PublishFails, Command::PublishPlugin { plugin, .. }) => {
            return Some(failed(&format!(
                "{plugin} not published: the store refused a plugin it can't verify"
            )));
        }
        (Mood::Bare, Command::LoadMembers { store }) => {
            let Event::StoreMembers { members, .. } = members(store) else { return None };
            return Some(vec![
                Event::StoreMembers { store: store.clone(), members, invites: Vec::new() },
                open_join(store, false),
            ]);
        }
        (Mood::Bare, Command::LoadLocalPlugins { store }) => {
            return Some(vec![Event::LocalPlugins {
                store: store.clone(),
                plugins: Vec::new(),
            }]);
        }
        (Mood::Closed, Command::LoadMembers { store }) => {
            return Some(vec![members(store), open_join(store, false)]);
        }
        _ => {}
    }
    Some(match command {
        Command::CreateInvite { store, name } if ours(store) => {
            vec![Event::InviteLink(InviteLinkView {
                store: store.clone(),
                link: Secret("https://plugins.mika.example/join/x7kp2q#key".into()),
                for_whom: name.clone(),
                expires_at: now + 7 * 86_400,
            })]
        }
        Command::LoadMembers { store } if ours(store) => {
            vec![members(store), open_join(store, true)]
        }
        Command::RemoveMember { store, .. } | Command::CancelInvite { store, .. }
            if ours(store) =>
        {
            vec![members(store)]
        }
        Command::SetOpenJoin { store, open } if ours(store) => vec![open_join(store, *open)],
        Command::RemoveAnonymous { store, .. } if ours(store) => vec![open_join(store, true)],
        Command::LoadLocalPlugins { store } if ours(store) => vec![Event::LocalPlugins {
            store: store.clone(),
            plugins: vec![
                local("machi-library-films", "Machi Library Films", "0.3.0", None),
                local("lantern-road-guide", "Lantern Road Guide", "1.2.0", Some("1.1.0")),
                local("atelier-hotaru-shorts", "Atelier Hotaru Shorts", "1.0.2", Some("1.0.2")),
            ],
        }],
        Command::PublishPlugin { store, .. } | Command::OpenAdmin { store } if ours(store) => {
            Vec::new()
        }
        _ => return None,
    })
}

fn member(id: &str, name: &str, owner: bool, devices: u32, you: bool) -> StoreMemberRow {
    StoreMemberRow { id: id.into(), name: name.into(), owner, devices, last_seen: None, you }
}

fn local(id: &str, name: &str, version: &str, published: Option<&str>) -> LocalPluginRow {
    LocalPluginRow {
        id: id.into(),
        name: name.into(),
        version: version.into(),
        hosts: Vec::new(),
        published: published.map(Into::into),
    }
}

fn hosts(hosts: &[&str]) -> Vec<String> {
    hosts.iter().map(|h| (*h).to_owned()).collect()
}

fn hotaru() -> ProviderOrigin {
    ProviderOrigin::Store { id: "hotaru".into(), name: Some("Hotaru Plugins".into()) }
}

fn providers() -> Vec<ProviderRow> {
    vec![
        ProviderRow {
            id: "hotaru-open-archive".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: Some(180),
            usable: true,
            name: "Hotaru Open Archive".into(),
            version: Some("2.3.0".into()),
            hosts: hosts(&["open-archive.hotaru.example", "media.open-archive.hotaru.example"]),
            origin: hotaru(),
            ..ProviderRow::default()
        },
        ProviderRow {
            id: "mukashi-archive".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: Some(320),
            usable: true,
            name: "Mukashi Archive".into(),
            version: Some("1.4.0".into()),
            hosts: hosts(&["mukashi-archive.example"]),
            origin: hotaru(),
            update: Some(ProviderUpdate {
                version: "1.5.0".into(),
                new_hosts: hosts(&["media2.mukashi-archive.example"]),
            }),
            ..ProviderRow::default()
        },
        ProviderRow {
            id: "kobunko-film-library".into(),
            kind: "plugin".into(),
            state: "degraded".into(),
            latency_ms: Some(2_400),
            usable: true,
            name: "Kobunko Film Library".into(),
            version: Some("0.9.1".into()),
            hosts: hosts(&["films.kobunko.example"]),
            origin: hotaru(),
            disabled: true,
            ..ProviderRow::default()
        },
        ProviderRow {
            id: "machi-library-films".into(),
            kind: "plugin".into(),
            state: "down".into(),
            last_error: Some("the site did not answer in 15 s.".into()),
            name: "Machi Library Films".into(),
            version: Some("0.3.0".into()),
            hosts: hosts(&["films.machi-library.example"]),
            origin: ProviderOrigin::ByHand,
            ..ProviderRow::default()
        },
        ProviderRow {
            id: "torrent".into(),
            kind: "torrent".into(),
            state: "ready".into(),
            usable: true,
            name: "Torrent source".into(),
            ..ProviderRow::default()
        },
    ]
}

fn plugin(
    id: &str,
    name: &str,
    version: &str,
    at: &[&str],
    status: StorePluginStatus,
) -> StorePluginRow {
    StorePluginRow {
        id: id.into(),
        name: name.into(),
        version: version.into(),
        hosts: hosts(at),
        status,
    }
}

fn store(id: &str, name: &str, host: &str, fingerprint: &str, state: StoreState) -> StoreRow {
    StoreRow {
        id: id.into(),
        name: name.into(),
        url: format!("https://{host}"),
        host: host.into(),
        fingerprint: fingerprint.into(),
        member: "Mika".into(),
        owner: false,
        state,
        checked_at: None,
        plugins: Vec::new(),
        owners: Vec::new(),
        open: false,
        anonymous: false,
    }
}

fn stores() -> Vec<StoreRow> {
    use StorePluginStatus::{AddedByHand, Available, Installed, OtherStore, Update};
    let hotaru = StoreRow {
        owners: vec!["Ren".into()],
        plugins: vec![
            plugin(
                "hotaru-open-archive",
                "Hotaru Open Archive",
                "2.3.0",
                &["open-archive.hotaru.example", "media.open-archive.hotaru.example"],
                Installed,
            ),
            plugin(
                "mukashi-archive",
                "Mukashi Archive",
                "1.5.0",
                &["mukashi-archive.example", "media2.mukashi-archive.example"],
                Update {
                    from: "1.4.0".into(),
                    new_hosts: hosts(&["media2.mukashi-archive.example"]),
                },
            ),
            plugin(
                "kobunko-film-library",
                "Kobunko Film Library",
                "0.9.1",
                &["films.kobunko.example"],
                Installed,
            ),
            plugin(
                "atelier-hotaru-shorts",
                "Atelier Hotaru Shorts",
                "1.0.2",
                &["shorts.atelier-hotaru.example"],
                Available,
            ),
        ],
        ..store(
            "hotaru",
            "Hotaru Plugins",
            "plugins.hotaru.example",
            "9f3c 1a2b 77de 0c41",
            StoreState::Ready,
        )
    };
    let mika = StoreRow {
        owner: true,
        open: true,
        plugins: vec![
            plugin(
                "machi-library-films",
                "Machi Library Films",
                "0.3.0",
                &["films.machi-library.example"],
                AddedByHand,
            ),
            plugin(
                "kobunko-film-library",
                "Kobunko Film Library",
                "0.9.1",
                &["films.kobunko.example"],
                OtherStore("Hotaru Plugins".into()),
            ),
        ],
        ..store(
            "mika",
            "Mika's plugins",
            "plugins.mika.example",
            "2b7e 90c4 d1f3 5a68",
            StoreState::Ready,
        )
    };
    let tsuki = StoreRow {
        plugins: vec![plugin(
            "sora-festival",
            "Sora Animation Festival",
            "1.1.0",
            &["archive.sora-festival.example"],
            Available,
        )],
        ..store(
            "tsuki",
            "Tsuki Club",
            "tsuki-club.example",
            "e815 3c6a 0b9d 42f7",
            StoreState::Offline("the store did not answer".into()),
        )
    };
    let ame = StoreRow {
        plugins: vec![plugin(
            "ame-shorts",
            "Ame Shorts",
            "0.2.0",
            &["shorts.ame.example"],
            Available,
        )],
        ..store(
            "ame",
            "Ame Plugins",
            "plugins.ame.example",
            "71aa c3e0 58b2 9d14",
            StoreState::Revoked,
        )
    };
    vec![hotaru, mika, tsuki, ame]
}

fn seeding() -> Vec<SeedRow> {
    vec![
        SeedRow {
            name: "Hoshikuzu no Tabibito - 06.mkv".into(),
            uploaded_bytes: 90_000_000,
            ratio: 0.2,
            goal: 1.0,
            left_secs: 64_800,
        },
        SeedRow {
            name: "Hoshikuzu no Tabibito - 05.mkv".into(),
            uploaded_bytes: 360_000_000,
            ratio: 0.8,
            goal: 1.0,
            left_secs: 21_600,
        },
    ]
}

/// How the showcase's discussion reads, for a shot of each state the sheet has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Thread {
    #[default]
    Comments,
    Nobody,
    Waiting,
    Signed,
}

thread_local! {
    static THREAD: std::cell::Cell<Thread> = const { std::cell::Cell::new(Thread::Comments) };
}

/// Answer an opened discussion as `thread` until set again.
pub fn set_thread(thread: Thread) {
    THREAD.with(|t| t.set(thread));
}

/// The discussion a shot opens: made-up people saying made-up things about a made-up show.
pub fn discussion(
    media: anistream_core::ids::AnilistId,
    episode: u32,
) -> anistream_model::discussion::DiscussionView {
    use anistream_model::{
        Msg,
        discussion::{
            CommentBlock, CommentSpan, CommentView, DiscussionState, DiscussionView, PostState,
        },
    };
    let thread = THREAD.with(std::cell::Cell::get);
    let text = |s: &str| CommentSpan { text: s.into(), ..Default::default() };
    let say = |spans: Vec<CommentSpan>, quote: bool| CommentBlock { spans, quote };
    let comment = |id: u32, author: &str, hours: u32, body: Vec<CommentBlock>| CommentView {
        id,
        author: author.into(),
        avatar: None,
        created_at: 0,
        age: Msg::DiscussionAgeHours { hours: hours.into() },
        body,
        like_count: 0,
        liked: false,
        own: false,
        pending: PostState::Posted,
        replies: Vec::new(),
        hidden_replies: 0,
        markdown: String::new(),
    };
    let mut first = comment(
        1,
        "marlowe",
        2,
        vec![
            say(
                vec![
                    text("That ending shot of the lighthouse. I did not expect the "),
                    CommentSpan {
                        text: "tide to reverse".into(),
                        spoiler: true,
                        ..Default::default()
                    },
                    text(" and I am still thinking about it."),
                ],
                false,
            ),
            say(
                vec![CommentSpan {
                    text: "the score alone carries the last minute".into(),
                    bold: true,
                    ..Default::default()
                }],
                false,
            ),
        ],
    );
    first.like_count = 14;
    first.liked = true;
    let mut answer = comment(
        2,
        "tamsin",
        1,
        vec![
            say(vec![text("the score alone carries the last minute")], true),
            say(vec![text("Agreed. The quiet before it did half the work.")], false),
        ],
    );
    answer.like_count = 3;
    let mut deeper =
        comment(3, "ostrava", 1, vec![say(vec![text("Same, I rewatched it twice.")], false)]);
    deeper.hidden_replies = 0;
    let mut hidden = comment(4, "wren", 0, vec![say(vec![text("Third time for me.")], false)]);
    hidden.hidden_replies = 0;
    answer.hidden_replies = 2;
    answer.replies = vec![deeper, hidden];
    first.replies = vec![answer];
    let mut mine =
        comment(5, "you", 0, vec![say(vec![text("Posting from the shot run.")], false)]);
    mine.own = true;
    mine.pending = PostState::Sending;
    mine.markdown = "Posting from the shot run.".into();
    let mut third = comment(
        6,
        "juniper",
        3,
        vec![say(
            vec![
                text("Notes on the animation: "),
                CommentSpan {
                    text: "a link to the sketch".into(),
                    link: Some("https://notes.example/sketch".into()),
                    ..Default::default()
                },
            ],
            false,
        )],
    );
    third.like_count = 1;
    let comments = match thread {
        Thread::Comments | Thread::Signed => vec![first, third, mine],
        Thread::Nobody | Thread::Waiting => Vec::new(),
    };
    DiscussionView {
        media,
        episode,
        title: format!("Lantern Harbour, Episode {episode} discussion"),
        comment_count: if comments.is_empty() { 0 } else { 214 },
        url: "https://anilist.example/forum/thread/1".into(),
        comments,
        state: match thread {
            Thread::Comments | Thread::Signed => DiscussionState::Ready { more: false },
            Thread::Nobody => DiscussionState::NoThread,
            Thread::Waiting => DiscussionState::Loading,
        },
        can_post: thread == Thread::Comments,
        signed_in: thread != Thread::Signed,
        load_images: false,
    }
}

/// How the showcase's media server answers, for a shot of each state of the add flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    #[default]
    Usual,
    /// Nobody answers at the address.
    Unreachable,
    /// The sign-in is turned down.
    Credentials,
}

/// Which of a title's four availability cases the showcase answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Case {
    #[default]
    PlaysAlsoOn,
    OnService,
    NotInSources,
}

#[derive(Default)]
struct Lawful {
    on: bool,
    flow: Flow,
    case: Case,
    bare: bool,
    chosen: bool,
    servers_added: bool,
}

thread_local! {
    static LAWFUL: std::cell::RefCell<Lawful> = std::cell::RefCell::new(Lawful::default());
}

/// Turn the lawful sources showcase on, answering as `flow` and `case`. With `bare`, nothing is
/// added yet: no folders, no servers.
pub fn lawful(flow: Flow, case: Case, bare: bool) {
    LAWFUL.with(|l| {
        *l.borrow_mut() = Lawful { on: true, flow, case, bare, ..Lawful::default() };
    });
}

fn lawful_on() -> bool {
    LAWFUL.with(|l| l.borrow().on)
}

fn local_view() -> anistream_model::local::LocalView {
    use anistream_core::ids::AnilistId;
    use anistream_model::local::{
        FolderRow, GroupState, LocalGroup, LocalView, NotMatchedReason,
    };
    let (bare, chosen) = LAWFUL.with(|l| (l.borrow().bare, l.borrow().chosen));
    if bare {
        return LocalView { folders: Vec::new(), groups: Vec::new(), scanning: false };
    }
    let group = |folder: &str, name: &str, files: u32, state: GroupState| LocalGroup {
        id: format!("{folder}|{name}"),
        name: name.into(),
        files,
        state,
        anilist: None,
        matched_title: None,
        reason: None,
    };
    let media = "/media/shows";
    let mut harbour = group(media, "harbour light nights", 11, GroupState::NeedsLook);
    harbour.anilist = Some(AnilistId::new(900_003));
    harbour.matched_title = Some("Harbour Light Nights".into());
    if chosen {
        harbour.state = GroupState::Matched;
    }
    let mut lantern = group(media, "lantern road", 12, GroupState::Matched);
    lantern.anilist = Some(AnilistId::new(900_001));
    lantern.matched_title = Some("Lantern Road".into());
    let mut clip = group(media, "clip 0412", 1, GroupState::NotMatched);
    clip.reason = Some(NotMatchedReason::NoResult);
    LocalView {
        folders: vec![
            FolderRow { path: media.into(), files: 24, available: true },
            FolderRow {
                path: "/run/media/mika/travel-drive/anime".into(),
                files: 36,
                available: false,
            },
            FolderRow { path: "/home/mika/Videos/old-rips".into(), files: 0, available: false },
        ],
        groups: vec![
            lantern,
            harbour,
            clip,
            group(
                "/run/media/mika/travel-drive/anime",
                "tidewater diary",
                36,
                GroupState::Matched,
            ),
            group(media, "birthday 2023", 3, GroupState::Kept),
        ],
        scanning: false,
    }
}

fn server_rows() -> Vec<anistream_model::servers::ServerRow> {
    use anistream_model::servers::ServerRow;
    if LAWFUL.with(|l| l.borrow().bare && !l.borrow().servers_added) {
        return Vec::new();
    }
    vec![ServerRow {
        id: "mikas-jellyfin".into(),
        name: "Mika's Jellyfin".into(),
        address: "https://jelly.example:8920".into(),
        version: "10.10.3".into(),
        user: "mika".into(),
        libraries: libraries(),
        series: 48,
        matched: 41,
        report: true,
        signed_out: false,
    }]
}

fn libraries() -> Vec<anistream_model::servers::LibraryChoice> {
    use anistream_model::servers::LibraryChoice;
    let lib = |id: &str, name: &str, kind: &str, ticked| LibraryChoice {
        id: id.into(),
        name: name.into(),
        kind: Some(kind.into()),
        ticked,
    };
    vec![
        lib("shows", "Shows", "tvshows", true),
        lib("films", "Films", "movies", false),
        lib("home", "Home videos", "homevideos", false),
    ]
}

fn licensed_view() -> anistream_model::licensed::LicensedView {
    use anistream_model::licensed::{LicensedSettings, LicensedView, REGIONS};
    let bare = LAWFUL.with(|l| l.borrow().bare);
    LicensedView {
        settings: LicensedSettings {
            services: if bare { Vec::new() } else { vec!["Lanternflix".into()] },
            region: Some("DK".into()),
            show_others: true,
        },
        region: Some("DK".into()),
        available: ["Lanternflix", "Harbourplay", "Mochi Video", "Tidewater TV"]
            .map(String::from)
            .to_vec(),
        regions: REGIONS.iter().map(|r| (*r).to_owned()).collect(),
    }
}

fn availability(
    id: anistream_core::ids::AnilistId,
) -> anistream_model::licensed::TitleAvailability {
    use anistream_model::licensed::{
        AvailabilityCase, EpisodeLink, ServiceLink, TitleAvailability,
    };
    let link = |service: &str, notes: Option<&str>| ServiceLink {
        service: service.into(),
        url: format!(
            "https://{}.example/title/{}",
            service.to_lowercase().replace(' ', "-"),
            id.get()
        ),
        notes: notes.map(Into::into),
    };
    let case = LAWFUL.with(|l| l.borrow().case);
    let empty = TitleAvailability {
        id,
        case: AvailabilityCase::NotInSources,
        main: None,
        also_on: Vec::new(),
        yours: Vec::new(),
        others: Vec::new(),
        episodes: Vec::new(),
        choose_services: false,
    };
    match case {
        Case::PlaysAlsoOn => TitleAvailability {
            case: AvailabilityCase::PlaysAlsoOn,
            also_on: vec![link("Lanternflix", None)],
            others: vec!["Harbourplay".into()],
            ..empty
        },
        Case::OnService => TitleAvailability {
            case: AvailabilityCase::OnService,
            main: Some(link("Lanternflix", None)),
            yours: vec![link("Lanternflix", None)],
            others: vec!["Harbourplay".into(), "Mochi Video".into()],
            episodes: vec![EpisodeLink {
                episode: 4,
                service: "Lanternflix".into(),
                url: "https://lanternflix.example/watch/4".into(),
            }],
            ..empty
        },
        Case::NotInSources => TitleAvailability { choose_services: true, ..empty },
    }
}

/// The notice for coming back from a service after an episode.
pub fn licensed_return() -> Event {
    Event::LicensedReturn(anistream_model::licensed::LicensedReturn {
        id: anistream_core::ids::AnilistId::new(900_001),
        episode: 4,
        service: "Lanternflix".into(),
        prompt: anistream_model::Msg::verbatim("Did you watch episode 4 on Lanternflix?"),
        done: anistream_model::Msg::verbatim("Episode 4 marked as watched."),
    })
}

/// What the lawful sources showcase answers to the commands of folders, servers and licensed
/// services. `None` when it is off or the command is another kind.
pub fn lawful_answer(command: &Command) -> Option<Vec<Event>> {
    use anistream_model::{
        command::LicensedCommand as L,
        local::{LocalCommand as C, LocalEvent},
        servers::{
            CheckedServer, FoundServer, ServerCommand as S, ServerEvent, ServerFailure,
            ServerStep,
        },
    };
    if !lawful_on() {
        return None;
    }
    let flow = LAWFUL.with(|l| l.borrow().flow);
    let server = |e| Event::Server(e);
    let servers = || server(ServerEvent::Servers(server_rows()));
    Some(match command {
        Command::Local(
            C::Load | C::Scan | C::AddFolder { .. } | C::RemoveFolder { .. } | C::Keep { .. },
        ) => {
            vec![Event::Local(LocalEvent::View(local_view()))]
        }
        Command::Local(C::Choose { .. }) => {
            LAWFUL.with(|l| l.borrow_mut().chosen = true);
            vec![Event::Local(LocalEvent::View(local_view()))]
        }
        Command::Server(S::Discover) => vec![
            server(ServerEvent::Found(FoundServer {
                id: "mikas-jellyfin".into(),
                name: "Mika's Jellyfin".into(),
                address: "https://jelly.example:8920".into(),
            })),
            server(ServerEvent::DiscoveryDone),
        ],
        Command::Server(S::Check { address }) => match flow {
            Flow::Unreachable => vec![server(ServerEvent::Failed {
                step: ServerStep::Address,
                failure: ServerFailure::Unreachable { address: address.clone() },
            })],
            _ => vec![server(ServerEvent::Checked(CheckedServer {
                id: "mikas-jellyfin".into(),
                name: "Mika's Jellyfin".into(),
                version: "10.10.3".into(),
                address: address.clone(),
                quick_connect: true,
            }))],
        },
        Command::Server(S::SignIn { .. }) => match flow {
            Flow::Credentials => vec![server(ServerEvent::Failed {
                step: ServerStep::SignIn,
                failure: ServerFailure::Credentials,
            })],
            _ => vec![
                server(ServerEvent::SignedIn { server: "mikas-jellyfin".into() }),
                server(ServerEvent::Libraries {
                    server: "mikas-jellyfin".into(),
                    libraries: libraries(),
                }),
            ],
        },
        Command::Server(S::StartQuickConnect { .. }) => {
            vec![server(ServerEvent::QuickConnectCode { code: "482913".into() })]
        }
        Command::Server(S::LoadLibraries { server: id }) => {
            vec![server(ServerEvent::Libraries { server: id.clone(), libraries: libraries() })]
        }
        Command::Server(S::SetLibraries { .. }) => {
            LAWFUL.with(|l| l.borrow_mut().servers_added = true);
            vec![servers()]
        }
        Command::Server(
            S::Load | S::Refresh { .. } | S::SetReport { .. } | S::Remove { .. },
        ) => {
            vec![servers()]
        }
        Command::Licensed(
            L::Load { .. }
            | L::SetServices { .. }
            | L::SetRegion { .. }
            | L::SetShowOthers { .. },
        ) => {
            vec![Event::Licensed(licensed_view())]
        }
        Command::Licensed(L::LoadAvailability { id, .. }) => {
            vec![Event::Availability(availability(*id))]
        }
        Command::Licensed(L::Returned) => vec![licensed_return()],
        _ => return None,
    })
}

/// What the lawful sources showcase sets in place of the engine's own.
pub fn lawful_events() -> Vec<Event> {
    use anistream_model::{local::LocalEvent, servers::ServerEvent};
    vec![
        Event::Local(LocalEvent::View(local_view())),
        Event::Server(ServerEvent::Servers(server_rows())),
        Event::Licensed(licensed_view()),
    ]
}
