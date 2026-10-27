//! Plugin store tasks, against a real store server on loopback.
//!
//! A task that changes what is joined or installed sends the rows as they now stand before it
//! says what happened, so the screen behind the toast is already right. The store is served
//! in-process from a temporary directory, as the engine's `tests/plugin_stores.rs` serves it; no
//! plugin is built for these, so publishing and installing are seen refusing.

use std::sync::Arc;

use anistream_model::{
    command::Command,
    event::Event,
    stores::{InvitePreview, OpenJoinView, Secret, StoreState},
};
use anistream_plugin_store::server::{self, Server, Storage};

use super::{ends, toasts};
use crate::harness::{Harness, names};

/// A store called "Ours" on `127.0.0.1`, and the link that makes its owner.
struct StoreServer {
    owner_link: String,
    _dir: tempfile::TempDir,
}

impl StoreServer {
    async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (listener, addr) = server::bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let base = format!("http://{addr}");
        let store =
            server::open(&Storage::Dir(dir.path().join("store")), Some(&base), Some("Ours"))
                .await
                .unwrap();
        let invite = store.fresh_owner_invite(&base, server::now()).await.unwrap().unwrap();
        let inspector = anistream_engine::plugin_stores::WasmInspector::new(
            anistream_plugin::Limits::default(),
            None,
        )
        .unwrap();
        let server = Arc::new(Server::new(
            store,
            Some(base.clone()),
            Some(Box::new(inspector)),
            None,
            false,
        ));
        tokio::spawn(server::serve(listener, server, None, std::future::pending()));
        Self { owner_link: invite.link(), _dir: dir }
    }
}

/// `h`, joined to `link`, and the store's id.
async fn joined(h: &mut Harness, link: &str) -> String {
    h.send(Command::JoinStore { link: Secret(link.to_owned()), label: None });
    h.settle().await;
    h.app().stores[0].id.clone()
}

fn link(update: &Event) -> String {
    match update {
        Event::InviteLink(view) => view.link.0.clone(),
        other => panic!("not an invite: {other:?}"),
    }
}

/// The members and the open invites, by name, sorted: the store lists them in an order of
/// its own.
fn people(update: &Event) -> (Vec<String>, Vec<String>) {
    let sorted = |mut names: Vec<String>| {
        names.sort();
        names
    };
    match update {
        Event::StoreMembers { members, invites, .. } => (
            sorted(members.iter().map(|m| m.name.clone()).collect()),
            sorted(invites.iter().map(|i| i.name.clone()).collect()),
        ),
        other => panic!("not the members: {other:?}"),
    }
}

#[tokio::test]
async fn with_nothing_joined_the_stores_load_twice_and_empty() {
    let mut h = Harness::new().await;
    let job = h.send(Command::LoadStores);
    let updates = h.settle().await;

    // From disk at once, then refreshed from each store, of which there are none.
    assert_eq!(names(&updates), ["Stores", "Stores", "JobDone"]);
    assert!(updates[..2].iter().all(|u| matches!(u, Event::Stores(rows) if rows.is_empty())));
    ends(&updates, job);
}

#[tokio::test]
async fn an_invite_is_previewed_then_joined() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let job = h.send(Command::PreviewStore { link: Secret(store.owner_link.clone()) });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["StorePreview", "JobDone"]);
    assert!(matches!(
        &updates[0],
        Event::StorePreview(InvitePreview::Store(preview))
            if preview.name == "Ours" && !preview.already_joined && preview.host == "127.0.0.1"
    ));
    // From the store's signed list: nobody runs it until this link makes its owner, and it
    // offers nothing yet.
    let Event::StorePreview(InvitePreview::Store(preview)) = &updates[0] else {
        unreachable!()
    };
    assert!(preview.owners.is_empty() && preview.plugins.is_empty(), "{preview:?}");
    // What the store will call this machine, for the confirmation to offer: its kind, never
    // its own name.
    assert_eq!(preview.device_label, "Computer");
    ends(&updates, job);

    let job =
        h.send(Command::JoinStore { link: Secret(store.owner_link.clone()), label: None });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Stores", "StoreJoined", "Toast", "JobDone"]);
    let Event::Stores(rows) = &updates[0] else { unreachable!() };
    assert!(matches!(&updates[1], Event::StoreJoined { store } if *store == rows[0].id));
    assert!(rows[0].owner);
    assert_eq!(toasts(&updates), ["info: Joined Ours as Owner."]);
    ends(&updates, job);

    let job = h.send(Command::LoadStores);
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Stores", "Stores", "JobDone"]);
    assert!(matches!(&updates[1], Event::Stores(rows) if rows[0].state == StoreState::Ready));
    // Named by its host, and checked as it was joined and again just now.
    let Event::Stores(rows) = &updates[1] else { unreachable!() };
    assert_eq!(rows[0].host, "127.0.0.1");
    assert!(rows[0].checked_at.is_some(), "checked");
    assert_eq!(rows[0].owners, ["Owner"], "run by whoever the owner link made");
    ends(&updates, job);
}

#[tokio::test]
async fn a_link_that_is_not_one_previews_as_a_refusal() {
    let mut h = Harness::new().await;
    let job = h.send(Command::PreviewStore { link: Secret("not a link".into()) });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["StorePreview", "JobDone"]);
    assert!(matches!(
        &updates[0],
        Event::StorePreview(InvitePreview::Failed { reason }) if reason == "that is not a link"
    ));
    ends(&updates, job);
}

#[tokio::test]
async fn an_owner_invites_someone_lists_them_and_takes_their_access_away() {
    let store = StoreServer::start().await;
    let mut owner = Harness::new().await;
    let id = joined(&mut owner, &store.owner_link).await;

    let job =
        owner.send(Command::CreateInvite { store: id.clone(), name: Some("Alex".into()) });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["InviteLink", "JobDone"]);
    let Event::InviteLink(view) = &updates[0] else { unreachable!() };
    assert_eq!((view.store.as_str(), view.for_whom.as_deref()), (id.as_str(), Some("Alex")));
    assert!(!owner.app().invite_copied, "no terminal here to copy it to");
    ends(&updates, job);

    // Joined on another machine.
    let mut alex = Harness::new().await;
    joined(&mut alex, &link(&updates[0])).await;

    // The people, then who can join: only they, until the owner says otherwise.
    let job = owner.send(Command::LoadMembers { store: id.clone() });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["StoreMembers", "OpenJoin", "JobDone"]);
    assert!(
        matches!(&updates[1], Event::OpenJoin { view, .. } if !view.open && view.devices == 0)
    );
    assert_eq!(people(&updates[0]), (vec!["Alex".into(), "Owner".into()], vec![]));
    ends(&updates, job);
    let Event::StoreMembers { members, .. } = &updates[0] else { unreachable!() };
    let member = members.iter().find(|m| m.name == "Alex").unwrap().id.clone();

    // Removed: said, then the list as it now stands.
    let job = owner.send(Command::RemoveMember { store: id.clone(), member });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["Toast", "StoreMembers", "JobDone"]);
    assert_eq!(
        toasts(&updates),
        ["info: Removed. Their devices stopped working with this plugin store."]
    );
    assert_eq!(people(&updates[1]), (vec!["Owner".into()], vec![]));
    ends(&updates, job);

    // Their next refresh finds out.
    alex.send(Command::LoadStores);
    let updates = alex.settle().await;
    assert_eq!(names(&updates), ["Stores", "Stores", "JobDone"]);
    assert!(matches!(&updates[1], Event::Stores(rows) if rows[0].state == StoreState::Revoked));
}

#[tokio::test]
async fn a_cancelled_invite_is_said_then_gone_from_the_list() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let id = joined(&mut h, &store.owner_link).await;
    h.send(Command::CreateInvite { store: id.clone(), name: Some("Sam".into()) });
    h.settle().await;
    h.send(Command::LoadMembers { store: id.clone() });
    let listed = h.settle().await;
    let Event::StoreMembers { invites, .. } = &listed[0] else { panic!("{listed:?}") };
    let invite = invites[0].id.clone();

    let job = h.send(Command::CancelInvite { store: id, invite });
    let updates = h.settle().await;
    assert!(job.is_none(), "cancelling has no activity row");
    assert_eq!(names(&updates), ["Toast", "StoreMembers"]);
    assert_eq!(toasts(&updates), ["info: Invite cancelled. The link no longer works."]);
    assert_eq!(people(&updates[1]), (vec!["Owner".into()], vec![]));
}

#[tokio::test]
async fn plugins_that_are_not_there_are_refused_by_name() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let id = joined(&mut h, &store.owner_link).await;

    let job = h.send(Command::LoadLocalPlugins { store: id.clone() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["LocalPlugins", "JobDone"]);
    let Event::LocalPlugins { store, plugins } = &updates[0] else { unreachable!() };
    assert_eq!(store, &id);
    assert!(plugins.is_empty(), "nothing in the plugin directory");
    ends(&updates, job);

    let job = h.send(Command::PublishPlugin { store: id.clone(), plugin: "absent".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: absent is no longer in the plugin folder."]);
    ends(&updates, job);

    let job = h.send(Command::UnpublishPlugin { store: id.clone(), plugin: "absent".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: absent is not published here"]);
    ends(&updates, job);

    // The install refreshed the list on its way, so the rows come before the refusal.
    let job = h.send(Command::InstallPlugin { store: id.clone(), plugin: "absent".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Stores", "Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: Ours no longer offers absent."]);
    ends(&updates, job);
}

#[tokio::test]
async fn removing_a_plugin_sends_the_chain_and_the_rows_before_saying_so() {
    let mut h = Harness::new().await;
    let job = h.send(Command::RemovePlugin { plugin: "absent".into() });
    let updates = h.settle().await;

    assert!(job.is_none(), "removing has no activity row");
    assert_eq!(names(&updates), ["Providers", "Providers", "Stores", "Toast"]);
}

#[tokio::test]
async fn leaving_sends_the_rows_then_says_the_plugins_stay() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let id = joined(&mut h, &store.owner_link).await;
    let job = h.send(Command::LeaveStore { store: id });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Stores", "Toast", "JobDone"]);
    assert!(matches!(&updates[0], Event::Stores(rows) if rows.is_empty()));
    assert_eq!(toasts(&updates), ["info: Left Ours. Its plugins stay installed."]);
    ends(&updates, job);
}

fn open_view(update: &Event) -> &OpenJoinView {
    match update {
        Event::OpenJoin { view, .. } => view,
        other => panic!("not who can join: {other:?}"),
    }
}

#[tokio::test]
async fn an_owner_opens_the_store_and_anyone_with_its_link_joins_without_a_name() {
    let store = StoreServer::start().await;
    let mut owner = Harness::new().await;
    let id = joined(&mut owner, &store.owner_link).await;

    // Opened: the row says so before the toast does, then the link to share.
    let job = owner.send(Command::SetOpenJoin { store: id.clone(), open: true });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["Stores", "OpenJoin", "Toast", "JobDone"]);
    assert!(matches!(&updates[0], Event::Stores(rows) if rows[0].open));
    let link = open_view(&updates[1]).link.clone().expect("an open store's link").0;
    assert_eq!(
        toasts(&updates),
        [
            "info: Anyone with the plugin store's link can join now. Copy it from the members screen."
        ]
    );
    ends(&updates, job);

    // Someone with the link, on another machine: the preview says it is open, and the join
    // keeps no name.
    let mut guest = Harness::new().await;
    guest.send(Command::PreviewStore { link: Secret(link.clone()) });
    let updates = guest.settle().await;
    let Event::StorePreview(InvitePreview::Store(preview)) = &updates[0] else {
        panic!("{updates:?}")
    };
    assert!(preview.open && preview.anonymous);
    assert_eq!(preview.owners, ["Owner"]);
    assert_eq!(preview.device_label, "", "an open link sends no name");
    let job = guest.send(Command::JoinStore { link: Secret(link.clone()), label: None });
    let updates = guest.settle().await;
    assert_eq!(names(&updates), ["Stores", "StoreJoined", "Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["info: Joined Ours without a name."]);
    assert!(matches!(&updates[0], Event::Stores(rows) if rows[0].anonymous && rows[0].open));
    ends(&updates, job);

    // Counted for the owner, not listed among the people.
    owner.send(Command::LoadMembers { store: id.clone() });
    let updates = owner.settle().await;
    assert_eq!(people(&updates[0]), (vec!["Owner".into()], vec![]));
    let view = open_view(&updates[1]);
    assert_eq!((view.devices, view.recent.len()), (1, 1));
    let device = view.recent[0].id.clone();

    // Invites only again: the device stays, then goes when the owner removes it.
    let job = owner.send(Command::SetOpenJoin { store: id.clone(), open: false });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["Stores", "OpenJoin", "Toast", "JobDone"]);
    assert!(!open_view(&updates[1]).open && open_view(&updates[1]).link.is_none());
    assert_eq!(
        toasts(&updates),
        ["info: Only people you invite can join now. The device that joined with the link \
             keeps working until you remove it."]
    );
    ends(&updates, job);
    let job = owner.send(Command::RemoveAnonymous { store: id, device: Some(device) });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["OpenJoin", "Toast", "JobDone"]);
    assert_eq!(open_view(&updates[0]).devices, 0);
    ends(&updates, job);

    guest.send(Command::LoadStores);
    let updates = guest.settle().await;
    assert!(matches!(&updates[1], Event::Stores(rows) if rows[0].state == StoreState::Revoked));
}
