//! The store's rules, on SQLite and a directory. The same rules run on Postgres and D1; these
//! are where they are pinned down.

use std::sync::Arc;

use anistream_store_core::{
    blobs::{Blobs, SqlBlobs},
    crypto,
    db::Database,
    inspect::fake::{FakeInspector, plugin},
    invite::Invite,
    protocol::Role,
    store::{
        ADMIN_LINK_SECS, NewInvite, OPEN_JOIN_WINDOW_SECS, OPEN_JOINS_ALLOWED, SESSION_SECS,
        Store, StoreError,
    },
};

use super::{files::LocalBlobs, sqlite::SqliteDb};

const NOW: i64 = 1_800_000_000;
const BASE: &str = "https://plugins.example";

type TestStore = Store<Arc<dyn Database>, Box<dyn Blobs>>;

async fn open() -> (tempfile::TempDir, TestStore) {
    let dir = tempfile::tempdir().unwrap();
    let db: Arc<dyn Database> = Arc::new(SqliteDb::in_memory().unwrap());
    let blobs: Box<dyn Blobs> = Box::new(LocalBlobs::new(dir.path()));
    let store = Store::open(db, blobs, Some(BASE), Some("Test")).await.unwrap();
    (dir, store)
}

async fn member_invite(store: &TestStore, name: &str, role: Role, expires_at: i64) -> Invite {
    store
        .create_invite(NewInvite::member(name, role, None, expires_at), BASE, NOW)
        .await
        .unwrap()
        .1
}

async fn join(
    store: &TestStore,
    name: &str,
    role: Role,
) -> (String, anistream_store_core::protocol::Member) {
    let invite = member_invite(store, name, role, NOW + 60).await;
    store.redeem(&invite.code, "device", NOW).await.unwrap()
}

#[tokio::test]
async fn the_key_is_made_once_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let db: Arc<dyn Database> = Arc::new(SqliteDb::open(&dir.path().join("store.db")).unwrap());
    let first = Store::open(
        db.clone(),
        Box::new(LocalBlobs::new(dir.path())) as Box<dyn Blobs>,
        None,
        None,
    )
    .await
    .unwrap();
    let key = first.public_key();
    assert_eq!(first.name().await, anistream_store_core::store::DEFAULT_NAME);
    drop(first);
    let again =
        Store::open(db, Box::new(LocalBlobs::new(dir.path())) as Box<dyn Blobs>, None, None)
            .await
            .unwrap();
    assert_eq!(again.public_key(), key);
}

#[tokio::test]
async fn an_invite_link_carries_the_stores_address_and_key() {
    let (_dir, store) = open().await;
    let invite = member_invite(&store, "Alex", Role::Member, NOW + 60).await;
    assert_eq!(invite.base, BASE);
    assert_eq!(invite.key, store.public_key());
    assert_eq!(Invite::parse(&invite.link()).unwrap(), invite);
}

#[tokio::test]
async fn an_invite_works_once() {
    let (_dir, store) = open().await;
    let invite = member_invite(&store, "Alex", Role::Member, NOW + 60).await;
    let (token, member) = store.redeem(&invite.code, "laptop", NOW).await.unwrap();
    assert_eq!(member.name, "Alex");
    assert_eq!(member.role, Role::Member);
    assert!(matches!(
        store.redeem(&invite.code, "phone", NOW).await,
        Err(StoreError::InviteUnusable)
    ));
    let (who, device) = store.authenticate(&token, NOW).await.unwrap().unwrap();
    assert_eq!(who, member);
    assert_eq!(device.label, "laptop");
}

#[tokio::test]
async fn racing_for_one_invite_lets_exactly_one_through() {
    let (_dir, store) = open().await;
    let invite = member_invite(&store, "Alex", Role::Member, NOW + 60).await;
    let (a, b) = tokio::join!(
        store.redeem(&invite.code, "one", NOW),
        store.redeem(&invite.code, "two", NOW)
    );
    assert_eq!(a.is_ok() as u8 + b.is_ok() as u8, 1);
    assert_eq!(store.members().await.unwrap().len(), 1);
}

#[tokio::test]
async fn an_expired_or_cancelled_invite_is_unusable() {
    let (_dir, store) = open().await;
    let expired = member_invite(&store, "Alex", Role::Member, NOW - 1).await;
    assert!(matches!(
        store.redeem(&expired.code, "x", NOW).await,
        Err(StoreError::InviteUnusable)
    ));

    let (id, cancelled) = store
        .create_invite(NewInvite::member("Sam", Role::Member, None, NOW + 60), BASE, NOW)
        .await
        .unwrap();
    store.cancel_invite(&id, NOW).await.unwrap();
    assert!(matches!(
        store.redeem(&cancelled.code, "x", NOW).await,
        Err(StoreError::InviteUnusable)
    ));
    assert!(store.pending_invites(NOW).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_made_up_code_is_indistinguishable_from_a_used_one() {
    let (_dir, store) = open().await;
    let guess = crypto::new_code().unwrap();
    assert!(matches!(store.redeem(&guess, "x", NOW).await, Err(StoreError::InviteUnusable)));
    assert!(matches!(
        store.redeem("../../etc", "x", NOW).await,
        Err(StoreError::InviteUnusable)
    ));
}

#[tokio::test]
async fn a_device_invite_adds_a_device_to_the_same_member() {
    let (_dir, store) = open().await;
    let (_, alex) = join(&store, "Alex", Role::Member).await;
    let (_, device) =
        store.create_invite(NewInvite::device(&alex, NOW + 60), BASE, NOW).await.unwrap();
    let (phone, same) = store.redeem(&device.code, "phone", NOW).await.unwrap();
    assert_eq!(same.id, alex.id);
    assert!(store.authenticate(&phone, NOW).await.unwrap().is_some());
    let members = store.members().await.unwrap();
    assert_eq!(members.len(), 1, "a second device is not a second person");
    assert_eq!(members[0].devices, 2);
}

#[tokio::test]
async fn removing_a_member_revokes_every_device_and_their_pending_invites() {
    let (_dir, store) = open().await;
    let (_, owner) = join(&store, "Owner", Role::Owner).await;
    let (laptop, alex) = join(&store, "Alex", Role::Member).await;
    let (_, phone) =
        store.create_invite(NewInvite::device(&alex, NOW + 60), BASE, NOW).await.unwrap();
    store.remove_member(&alex.id, &owner.id, NOW).await.unwrap();
    assert!(store.authenticate(&laptop, NOW).await.unwrap().is_none());
    assert!(matches!(
        store.redeem(&phone.code, "phone", NOW).await,
        Err(StoreError::InviteUnusable)
    ));
    assert_eq!(store.members().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_removed_owner_cannot_come_back_through_an_invite_they_made() {
    let (_dir, store) = open().await;
    let (_, a) = join(&store, "A", Role::Owner).await;
    let (_, b) = join(&store, "B", Role::Owner).await;
    let (_, kept) = store
        .create_invite(NewInvite::member("x", Role::Owner, Some(&b.id), NOW + 60), BASE, NOW)
        .await
        .unwrap();
    store.remove_member(&b.id, &a.id, NOW).await.unwrap();
    assert!(matches!(
        store.redeem(&kept.code, "x", NOW).await,
        Err(StoreError::InviteUnusable)
    ));
}

#[tokio::test]
async fn an_owner_cannot_remove_themselves_or_change_their_own_role() {
    let (_dir, store) = open().await;
    let (_, owner) = join(&store, "Owner", Role::Owner).await;
    assert!(matches!(
        store.remove_member(&owner.id, &owner.id, NOW).await,
        Err(StoreError::Forbidden(_))
    ));
    assert!(matches!(
        store.set_role(&owner.id, Role::Member, &owner.id).await,
        Err(StoreError::Forbidden(_))
    ));
}

#[tokio::test]
async fn only_the_latest_owner_invite_works_until_one_is_claimed() {
    let (_dir, store) = open().await;
    let first = store.fresh_owner_invite(BASE, NOW).await.unwrap().unwrap();
    let second = store.fresh_owner_invite(BASE, NOW).await.unwrap().unwrap();
    assert!(matches!(
        store.redeem(&first.code, "x", NOW).await,
        Err(StoreError::InviteUnusable)
    ));
    let (_, owner) = store.redeem(&second.code, "desk", NOW).await.unwrap();
    assert_eq!(owner.role, Role::Owner);
    assert!(store.fresh_owner_invite(BASE, NOW).await.unwrap().is_none());
}

#[tokio::test]
async fn a_claim_needs_the_code_and_an_unowned_store() {
    let (_dir, store) = open().await;
    assert!(matches!(
        store.claim("nope", "code", BASE, NOW).await,
        Err(StoreError::Forbidden(_))
    ));
    let (invite, _) = store.claim("code", "code", BASE, NOW).await.unwrap();
    store.redeem(&invite.code, "desk", NOW).await.unwrap();
    assert!(matches!(
        store.claim("code", "code", BASE, NOW).await,
        Err(StoreError::Conflict(_))
    ));
}

#[tokio::test]
async fn a_stale_last_seen_is_refreshed_and_a_recent_one_left_alone() {
    let (_dir, store) = open().await;
    let (token, _) = join(&store, "Alex", Role::Member).await;
    store.authenticate(&token, NOW + 5).await.unwrap();
    assert_eq!(store.members().await.unwrap()[0].last_seen, Some(NOW));
    store.authenticate(&token, NOW + 600).await.unwrap();
    assert_eq!(store.members().await.unwrap()[0].last_seen, Some(NOW + 600));
}

#[tokio::test]
async fn a_member_is_renamed_within_limits() {
    let (_dir, store) = open().await;
    let (_, owner) = join(&store, "Owner", Role::Owner).await;
    store.rename(&owner.id, "  Johan ").await.unwrap();
    assert_eq!(store.members().await.unwrap()[0].name, "Johan");
    assert!(matches!(store.rename(&owner.id, " ").await, Err(StoreError::BadRequest(_))));
    assert!(matches!(store.rename("nobody", "x").await, Err(StoreError::NotFound(_))));
}

#[tokio::test]
async fn members_carry_their_devices_and_one_can_be_revoked() {
    let (_dir, store) = open().await;
    let (laptop, alex) = join(&store, "Alex", Role::Member).await;
    let (_, device) =
        store.create_invite(NewInvite::device(&alex, NOW + 60), BASE, NOW).await.unwrap();
    let (phone, _) = store.redeem(&device.code, "phone", NOW + 5).await.unwrap();
    let row = store.members().await.unwrap().remove(0);
    assert_eq!(row.devices, 2);
    store.revoke_any_device(&row.device_list[0].id, NOW).await.unwrap();
    assert!(store.authenticate(&laptop, NOW).await.unwrap().is_none());
    assert!(store.authenticate(&phone, NOW).await.unwrap().is_some(), "the member stays");
}

#[tokio::test]
async fn a_sign_in_link_works_once_and_the_session_ends_with_the_owner_role() {
    let (_dir, store) = open().await;
    let (_, owner) = join(&store, "Owner", Role::Owner).await;
    let (_, alex) = join(&store, "Alex", Role::Owner).await;
    let (link, expires_at) = store.create_admin_link(&alex.id, BASE, NOW).await.unwrap();
    assert_eq!(expires_at, NOW + ADMIN_LINK_SECS);
    assert!(link.starts_with("https://plugins.example/admin/#login="), "{link}");
    let code = link.rsplit('=').next().unwrap();
    let (token, member) = store.open_session(code, NOW).await.unwrap();
    assert_eq!(member.id, alex.id);
    assert!(matches!(store.open_session(code, NOW).await, Err(StoreError::LinkUnusable)));
    assert_eq!(store.session(&token, NOW).await.unwrap().unwrap().id, alex.id);
    store.set_role(&alex.id, Role::Member, &owner.id).await.unwrap();
    assert!(store.session(&token, NOW).await.unwrap().is_none());
    assert!(matches!(
        store.create_admin_link(&alex.id, BASE, NOW).await,
        Err(StoreError::Forbidden(_))
    ));
}

#[tokio::test]
async fn a_sign_in_link_expires_and_a_session_too() {
    let (_dir, store) = open().await;
    let (_, owner) = join(&store, "Owner", Role::Owner).await;
    let (link, _) = store.create_admin_link(&owner.id, BASE, NOW).await.unwrap();
    let code = link.rsplit('=').next().unwrap();
    assert!(matches!(
        store.open_session(code, NOW + ADMIN_LINK_SECS).await,
        Err(StoreError::LinkUnusable)
    ));
    let (link, _) = store.create_admin_link(&owner.id, BASE, NOW).await.unwrap();
    let (token, _) = store.open_session(link.rsplit('=').next().unwrap(), NOW).await.unwrap();
    assert!(store.session(&token, NOW + SESSION_SECS).await.unwrap().is_none());
}

#[tokio::test]
async fn failures_are_counted_per_key_and_forgotten_after_the_window() {
    use anistream_store_core::store::{ALLOWED_FAILURES, FAILURE_WINDOW_SECS};
    let (_dir, store) = open().await;
    for _ in 0..ALLOWED_FAILURES {
        assert!(!store.blocked("a", NOW).await.unwrap());
        store.fail("a", NOW).await.unwrap();
    }
    assert!(store.blocked("a", NOW).await.unwrap());
    assert!(!store.blocked("b", NOW).await.unwrap());
    assert!(!store.blocked("a", NOW + FAILURE_WINDOW_SECS).await.unwrap());
}

#[tokio::test]
async fn publishing_moves_the_sequence_and_signs_the_index() {
    use crate::client::verify_index;
    let (_dir, store) = open().await;
    let key = store.public_key();
    let empty = verify_index(&store.signed_index().await.unwrap(), &key, 0).unwrap();
    assert_eq!(empty.sequence, 0);
    assert!(empty.plugins.is_empty());

    let wasm = plugin("example", "1.0.0", &["api.example"]);
    let entry = store.publish(&wasm, Some(&FakeInspector), NOW).await.unwrap();
    assert_eq!(entry.sha256, crypto::sha256_hex(&wasm));

    let index = verify_index(&store.signed_index().await.unwrap(), &key, 0).unwrap();
    assert_eq!(index.sequence, 1);
    assert_eq!(index.issued_at, NOW);
    assert_eq!(index.plugins, vec![entry.clone()]);
    let (_, bytes) = store.plugin_bytes("example", "1.0.0").await.unwrap().unwrap();
    assert_eq!(bytes, wasm);
}

#[tokio::test]
async fn a_published_version_never_changes() {
    use crate::client::verify_index;
    let (_dir, store) = open().await;
    let wasm = plugin("example", "1.0.0", &[]);
    store.publish(&wasm, Some(&FakeInspector), NOW).await.unwrap();
    store.publish(&wasm, Some(&FakeInspector), NOW + 1).await.unwrap();
    let index =
        verify_index(&store.signed_index().await.unwrap(), &store.public_key(), 0).unwrap();
    assert_eq!(index.sequence, 1, "the same bytes again is a retry, and changes nothing");
    let changed = plugin("example", "1.0.0", &["else.example"]);
    assert!(matches!(
        store.publish(&changed, Some(&FakeInspector), NOW).await,
        Err(StoreError::Conflict(_))
    ));
}

#[tokio::test]
async fn a_new_version_replaces_the_old_file() {
    let (dir, store) = open().await;
    store.publish(&plugin("example", "1.0.0", &[]), Some(&FakeInspector), NOW).await.unwrap();
    store.publish(&plugin("example", "1.1.0", &[]), Some(&FakeInspector), NOW).await.unwrap();
    assert!(store.plugin_bytes("example", "1.0.0").await.unwrap().is_none());
    assert!(store.plugin_bytes("example", "1.1.0").await.unwrap().is_some());
    assert!(!dir.path().join("plugins/example/1.0.0").exists(), "the old file is gone");
    assert_eq!(store.plugins().await.unwrap().len(), 1);
}

#[tokio::test]
async fn plugins_with_unsafe_or_reserved_ids_are_refused() {
    let (_dir, store) = open().await;
    for id in ["../evil", "torrent"] {
        let result = store.publish(&plugin(id, "1.0.0", &[]), Some(&FakeInspector), NOW).await;
        assert!(matches!(result, Err(StoreError::InvalidPlugin(_))), "{id}");
    }
    let result = store.publish(b"not a plugin", Some(&FakeInspector), NOW).await;
    assert!(matches!(result, Err(StoreError::InvalidPlugin(_))));
}

#[tokio::test]
async fn unpublishing_moves_the_sequence_and_removes_the_file() {
    use crate::client::verify_index;
    let (_dir, store) = open().await;
    store.publish(&plugin("example", "1.0.0", &[]), Some(&FakeInspector), NOW).await.unwrap();
    store.unpublish("example", NOW).await.unwrap();
    let index =
        verify_index(&store.signed_index().await.unwrap(), &store.public_key(), 0).unwrap();
    assert_eq!(index.sequence, 2);
    assert!(index.plugins.is_empty());
    assert!(matches!(store.unpublish("example", NOW).await, Err(StoreError::NotFound(_))));
}

#[tokio::test]
async fn files_can_live_in_the_database_itself() {
    // What a Postgres-only store does; SQLite stands in for Postgres here.
    let db: Arc<dyn Database> = Arc::new(SqliteDb::in_memory().unwrap());
    let blobs: Box<dyn Blobs> = Box::new(SqlBlobs::open(db.clone()).await.unwrap());
    let store = Store::open(db, blobs, Some(BASE), None).await.unwrap();
    let wasm = plugin("example", "1.0.0", &[]);
    let entry = store.publish(&wasm, Some(&FakeInspector), NOW).await.unwrap();
    assert_eq!(store.plugin_bytes("example", "1.0.0").await.unwrap().unwrap().1, wasm);
    // A range comes out of the database on its own, counted from 0 as the protocol counts.
    let range = store.plugin_range(&entry, 3, 5).await.unwrap().unwrap();
    assert_eq!(range, wasm[3..8]);
}

#[tokio::test]
async fn a_range_is_read_from_a_file_without_the_rest() {
    let (_dir, store) = open().await;
    let wasm = plugin("example", "1.0.0", &[]);
    let entry = store.publish(&wasm, Some(&FakeInspector), NOW).await.unwrap();
    let tail = store.plugin_range(&entry, 4, entry.size).await.unwrap().unwrap();
    assert_eq!(tail, wasm[4..]);
    assert_eq!(store.published("example", "1.0.0").await.unwrap(), Some(entry));
    assert_eq!(store.published("example", "0.9.0").await.unwrap(), None);
}

// --- open joining ---------------------------------------------------------------------------

/// Open the store to anyone with its link, and return the link's code.
async fn opened(store: &TestStore) -> String {
    store.set_open(true).await.unwrap();
    store.open_code().await.unwrap().expect("an open store has a code")
}

#[tokio::test]
async fn a_store_lets_only_invited_people_join_until_its_owner_opens_it() {
    let (_dir, store) = open().await;
    assert_eq!(store.open_code().await.unwrap(), None);
    assert!(store.open_link(BASE).await.unwrap().is_none());
    let state = store.open_state(BASE).await.unwrap();
    assert!(!state.open && state.link.is_none() && state.devices == 0);
    let guess = crypto::new_code().unwrap();
    assert!(matches!(
        store.join_open(&guess, "192.0.2.1", NOW).await,
        Err(StoreError::OpenUnusable)
    ));
    assert!(matches!(store.check_open(&guess).await, Err(StoreError::OpenUnusable)));
}

#[tokio::test]
async fn anyone_with_the_link_joins_as_a_member_with_no_name() {
    let (_dir, store) = open().await;
    join(&store, "Owner", Role::Owner).await;
    let code = opened(&store).await;
    let link = store.open_link(BASE).await.unwrap().unwrap();
    assert!(link.open);
    assert_eq!(Invite::parse(&link.link()).unwrap(), link);
    assert!(link.link().contains("?open#"), "{}", link.link());

    store.check_open(&code).await.unwrap();
    let (token, member) = store.join_open(&code, "192.0.2.1", NOW).await.unwrap();
    assert_eq!(member.name, "");
    assert_eq!(member.role, Role::Member);
    let (who, device) = store.authenticate(&token, NOW).await.unwrap().unwrap();
    assert_eq!(who, member);
    assert_eq!(device.label, "", "nothing about the device is kept");

    // The same link again, from the same place: another device, another member with no name.
    let (again, _) = store.join_open(&code, "192.0.2.1", NOW).await.unwrap();
    assert_ne!(again, token);

    // Counted, never listed among the people.
    let members = store.members().await.unwrap();
    assert_eq!(members.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Owner"]);
    assert_eq!(store.owner_names().await.unwrap(), ["Owner"]);
    let state = store.open_state(BASE).await.unwrap();
    assert!(state.open);
    assert_eq!(state.link.as_deref(), Some(link.link().as_str()));
    assert_eq!(state.devices, 2);
    assert_eq!(state.device_list.len(), 2);
    assert!(state.device_list.iter().all(|d| d.label.is_empty() && d.created_at == NOW));

    // Members see that the store is open; the list itself does not move for it.
    let key = store.public_key();
    let index = crate::client::verify_index(&store.signed_index().await.unwrap(), &key, 0);
    let index = index.unwrap();
    assert!(index.open);
    assert_eq!(index.sequence, 0);
}

#[tokio::test]
async fn an_anonymous_member_has_no_name_or_role_to_change() {
    let (_dir, store) = open().await;
    let (_, owner) = join(&store, "Owner", Role::Owner).await;
    let code = opened(&store).await;
    let (_, anonymous) = store.join_open(&code, "192.0.2.1", NOW).await.unwrap();
    assert!(matches!(
        store.set_role(&anonymous.id, Role::Owner, &owner.id).await,
        Err(StoreError::NotFound(_))
    ));
    assert!(matches!(store.rename(&anonymous.id, "Sam").await, Err(StoreError::NotFound(_))));
    assert_eq!(store.owner_names().await.unwrap(), ["Owner"]);
}

#[tokio::test]
async fn open_joins_are_limited_per_address_and_an_ipv6_network_counts_as_one() {
    let (_dir, store) = open().await;
    let code = opened(&store).await;
    for _ in 0..OPEN_JOINS_ALLOWED {
        store.join_open(&code, "198.51.100.7", NOW).await.unwrap();
    }
    assert!(matches!(
        store.join_open(&code, "198.51.100.7", NOW).await,
        Err(StoreError::TooManyJoins)
    ));
    // Another address is not held up by this one, and the window passes.
    store.join_open(&code, "198.51.100.8", NOW).await.unwrap();
    store.join_open(&code, "198.51.100.7", NOW + OPEN_JOIN_WINDOW_SECS).await.unwrap();

    // Every address in one /64 is one connection's.
    for host in 0..OPEN_JOINS_ALLOWED {
        store.join_open(&code, &format!("2001:db8:1:2::{host:x}"), NOW).await.unwrap();
    }
    assert!(matches!(
        store.join_open(&code, "2001:db8:1:2:ffff::1", NOW).await,
        Err(StoreError::TooManyJoins)
    ));
    store.join_open(&code, "2001:db8:1:3::1", NOW).await.unwrap();
}

#[tokio::test]
async fn switching_back_to_invites_stops_new_joins_and_keeps_the_devices_that_joined() {
    let (_dir, store) = open().await;
    let code = opened(&store).await;
    let (token, _) = store.join_open(&code, "192.0.2.1", NOW).await.unwrap();

    store.set_open(false).await.unwrap();
    assert!(matches!(
        store.join_open(&code, "192.0.2.2", NOW).await,
        Err(StoreError::OpenUnusable)
    ));
    assert!(!store.is_open_code(&code).await.unwrap());
    assert!(store.authenticate(&token, NOW).await.unwrap().is_some(), "kept until removed");
    let state = store.open_state(BASE).await.unwrap();
    assert!(!state.open && state.link.is_none());
    assert_eq!(state.devices, 1);

    // Opened again, with a new link: the old one stays dead.
    let fresh = opened(&store).await;
    assert_ne!(fresh, code);
    assert!(matches!(
        store.join_open(&code, "192.0.2.2", NOW).await,
        Err(StoreError::OpenUnusable)
    ));
    store.join_open(&fresh, "192.0.2.2", NOW).await.unwrap();
    // Opening an open store keeps its link.
    store.set_open(true).await.unwrap();
    assert_eq!(store.open_code().await.unwrap(), Some(fresh));
}

#[tokio::test]
async fn an_owner_revokes_one_anonymous_device_or_removes_them_all() {
    let (_dir, store) = open().await;
    let (owner_token, _) = join(&store, "Owner", Role::Owner).await;
    let (alex_token, _) = join(&store, "Alex", Role::Member).await;
    let code = opened(&store).await;
    let mut tokens = Vec::new();
    for at in 0..3 {
        tokens.push(store.join_open(&code, "192.0.2.1", NOW + at).await.unwrap().0);
    }
    let state = store.open_state(BASE).await.unwrap();
    assert_eq!(state.devices, 3);
    // Newest first: the one that joined last.
    let newest = &state.device_list[0];
    assert_eq!(newest.created_at, NOW + 2);
    store.revoke_any_device(&newest.id, NOW).await.unwrap();
    assert!(store.authenticate(&tokens[2], NOW).await.unwrap().is_none());
    assert!(store.authenticate(&tokens[0], NOW).await.unwrap().is_some());
    assert_eq!(store.open_state(BASE).await.unwrap().devices, 2);

    assert_eq!(store.remove_anonymous(NOW).await.unwrap(), 2);
    for token in &tokens {
        assert!(store.authenticate(token, NOW).await.unwrap().is_none());
    }
    assert_eq!(store.open_state(BASE).await.unwrap().devices, 0);
    // The people stay, and so does the setting.
    assert!(store.authenticate(&owner_token, NOW).await.unwrap().is_some());
    assert!(store.authenticate(&alex_token, NOW).await.unwrap().is_some());
    assert!(store.open_state(BASE).await.unwrap().open);
}

#[tokio::test]
async fn the_open_code_is_not_an_invite() {
    // A client from before open links sends the code to redeem or preview as an invite. The
    // store tells it apart, and it joins no one there.
    let (_dir, store) = open().await;
    let code = opened(&store).await;
    assert!(store.is_open_code(&code).await.unwrap());
    assert!(matches!(
        store.redeem(&code, "laptop", NOW).await,
        Err(StoreError::InviteUnusable)
    ));
    assert!(matches!(store.check_invite(&code, NOW).await, Err(StoreError::InviteUnusable)));
    let invite = member_invite(&store, "Alex", Role::Member, NOW + 60).await;
    assert!(!store.is_open_code(&invite.code).await.unwrap());
    assert!(matches!(
        store.join_open(&invite.code, "x", NOW).await,
        Err(StoreError::OpenUnusable)
    ));
}
