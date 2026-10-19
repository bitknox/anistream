//! Plugin store tasks, against a real store server on loopback.
//!
//! A task that changes what is joined or installed sends the rows as they now stand before it
//! says what happened, so the screen behind the toast is already right. The store is served
//! in-process from a temporary directory, as `tests/plugin_stores.rs` serves it; no plugin is
//! built for these, so publishing and installing are seen refusing.

use std::sync::Arc;

use anistream_plugin_store::server::{self, Server, Storage};
use anistream_ui::app::{Secret, StoreState, Task, Update};

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
        let inspector = anistream::plugin_stores::WasmInspector::new(
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
    h.send(Task::JoinStore { link: Secret(link.to_owned()) });
    h.settle().await;
    h.app().stores[0].id.clone()
}

fn link(update: &Update) -> String {
    match update {
        Update::InviteLink(view) => view.link.0.clone(),
        other => panic!("not an invite: {other:?}"),
    }
}

/// The members and the open invites, by name, sorted: the store lists them in an order of
/// its own.
fn people(update: &Update) -> (Vec<String>, Vec<String>) {
    let sorted = |mut names: Vec<String>| {
        names.sort();
        names
    };
    match update {
        Update::StoreMembers { members, invites, .. } => (
            sorted(members.iter().map(|m| m.name.clone()).collect()),
            sorted(invites.iter().map(|i| i.name.clone()).collect()),
        ),
        other => panic!("not the members: {other:?}"),
    }
}

#[tokio::test]
async fn with_nothing_joined_the_stores_load_twice_and_empty() {
    let mut h = Harness::new().await;
    let job = h.send(Task::LoadStores);
    let updates = h.settle().await;

    // From disk at once, then refreshed from each store, of which there are none.
    assert_eq!(names(&updates), ["Stores", "Stores", "JobDone"]);
    assert!(updates[..2].iter().all(|u| matches!(u, Update::Stores(rows) if rows.is_empty())));
    ends(&updates, job);
}

#[tokio::test]
async fn an_invite_is_previewed_then_joined() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let job = h.send(Task::PreviewStore { link: Secret(store.owner_link.clone()) });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["StorePreview", "JobDone"]);
    assert!(matches!(
        &updates[0],
        Update::StorePreview(Ok(preview)) if preview.name == "Ours" && !preview.already_joined
    ));
    ends(&updates, job);

    let job = h.send(Task::JoinStore { link: Secret(store.owner_link.clone()) });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Stores", "StoreJoined", "Toast", "JobDone"]);
    let Update::Stores(rows) = &updates[0] else { unreachable!() };
    assert!(matches!(&updates[1], Update::StoreJoined { store } if *store == rows[0].id));
    assert!(rows[0].owner);
    assert_eq!(toasts(&updates), ["info: joined Ours as Owner"]);
    ends(&updates, job);

    let job = h.send(Task::LoadStores);
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Stores", "Stores", "JobDone"]);
    assert!(matches!(&updates[1], Update::Stores(rows) if rows[0].state == StoreState::Ready));
    ends(&updates, job);
}

#[tokio::test]
async fn a_link_that_is_not_one_previews_as_a_refusal() {
    let mut h = Harness::new().await;
    let job = h.send(Task::PreviewStore { link: Secret("not a link".into()) });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["StorePreview", "JobDone"]);
    assert!(
        matches!(&updates[0], Update::StorePreview(Err(why)) if why == "that is not a link")
    );
    ends(&updates, job);
}

#[tokio::test]
async fn an_owner_invites_someone_lists_them_and_takes_their_access_away() {
    let store = StoreServer::start().await;
    let mut owner = Harness::new().await;
    let id = joined(&mut owner, &store.owner_link).await;

    let job = owner.send(Task::CreateInvite { store: id.clone(), name: Some("Alex".into()) });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["InviteLink", "JobDone"]);
    let Update::InviteLink(view) = &updates[0] else { unreachable!() };
    assert_eq!((view.store.as_str(), view.for_whom.as_deref()), (id.as_str(), Some("Alex")));
    assert!(!view.copied, "no terminal here to copy it to");
    ends(&updates, job);

    // Joined on another machine.
    let mut alex = Harness::new().await;
    joined(&mut alex, &link(&updates[0])).await;

    let job = owner.send(Task::LoadMembers { store: id.clone() });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["StoreMembers", "JobDone"]);
    assert_eq!(people(&updates[0]), (vec!["Alex".into(), "Owner".into()], vec![]));
    ends(&updates, job);
    let Update::StoreMembers { members, .. } = &updates[0] else { unreachable!() };
    let member = members.iter().find(|m| m.name == "Alex").unwrap().id.clone();

    // Removed: said, then the list as it now stands.
    let job = owner.send(Task::RemoveMember { store: id.clone(), member });
    let updates = owner.settle().await;
    assert_eq!(names(&updates), ["Toast", "StoreMembers", "JobDone"]);
    assert_eq!(toasts(&updates), ["info: removed — their devices stopped working here"]);
    assert_eq!(people(&updates[1]), (vec!["Owner".into()], vec![]));
    ends(&updates, job);

    // Their next refresh finds out.
    alex.send(Task::LoadStores);
    let updates = alex.settle().await;
    assert_eq!(names(&updates), ["Stores", "Stores", "JobDone"]);
    assert!(
        matches!(&updates[1], Update::Stores(rows) if rows[0].state == StoreState::Revoked)
    );
}

#[tokio::test]
async fn a_cancelled_invite_is_said_then_gone_from_the_list() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let id = joined(&mut h, &store.owner_link).await;
    h.send(Task::CreateInvite { store: id.clone(), name: Some("Sam".into()) });
    h.settle().await;
    h.send(Task::LoadMembers { store: id.clone() });
    let listed = h.settle().await;
    let Update::StoreMembers { invites, .. } = &listed[0] else { panic!("{listed:?}") };
    let invite = invites[0].id.clone();

    let job = h.send(Task::CancelInvite { store: id, invite });
    let updates = h.settle().await;
    assert!(job.is_none(), "cancelling has no activity row");
    assert_eq!(names(&updates), ["Toast", "StoreMembers"]);
    assert_eq!(toasts(&updates), ["info: invite cancelled — the link no longer works"]);
    assert_eq!(people(&updates[1]), (vec!["Owner".into()], vec![]));
}

#[tokio::test]
async fn plugins_that_are_not_there_are_refused_by_name() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let id = joined(&mut h, &store.owner_link).await;

    let job = h.send(Task::LoadLocalPlugins { store: id.clone() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["LocalPlugins", "JobDone"]);
    let Update::LocalPlugins { store, plugins } = &updates[0] else { unreachable!() };
    assert_eq!(store, &id);
    assert!(plugins.is_empty(), "nothing in the plugin directory");
    ends(&updates, job);

    let job = h.send(Task::PublishPlugin { store: id.clone(), plugin: "absent".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(
        toasts(&updates),
        ["alert: absent not published: absent is not in the plugin directory any more"]
    );
    ends(&updates, job);

    let job = h.send(Task::UnpublishPlugin { store: id.clone(), plugin: "absent".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: absent is not published here"]);
    ends(&updates, job);

    // The install refreshed the list on its way, so the rows come before the refusal.
    let job = h.send(Task::InstallPlugin { store: id.clone(), plugin: "absent".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Stores", "Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: absent not installed: Ours no longer offers absent"]);
    ends(&updates, job);
}

#[tokio::test]
async fn removing_a_plugin_sends_the_chain_and_the_rows_before_saying_so() {
    let mut h = Harness::new().await;
    let job = h.send(Task::RemovePlugin { plugin: "absent".into() });
    let updates = h.settle().await;

    assert!(job.is_none(), "removing has no activity row");
    assert_eq!(names(&updates), ["Providers", "Stores", "Toast"]);
}

#[tokio::test]
async fn leaving_sends_the_rows_then_says_the_plugins_stay() {
    let store = StoreServer::start().await;
    let mut h = Harness::new().await;
    let id = joined(&mut h, &store.owner_link).await;
    let job = h.send(Task::LeaveStore { store: id });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Stores", "Toast", "JobDone"]);
    assert!(matches!(&updates[0], Update::Stores(rows) if rows.is_empty()));
    assert_eq!(toasts(&updates), ["info: left Ours — its plugins stay installed"]);
    ends(&updates, job);
}
