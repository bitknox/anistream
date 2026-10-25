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
