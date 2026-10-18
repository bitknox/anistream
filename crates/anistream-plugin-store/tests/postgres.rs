//! The store's rules on Postgres, with plugin files in the same database — what a store on Heroku,
//! Render's free tier or Cloud Run runs.
//!
//! Needs a Postgres to talk to, so it runs only when `ANISTREAM_TEST_POSTGRES` holds a URL to a
//! database it may fill, and skips itself otherwise. Any Postgres will do; PGlite works without
//! installing one:
//!
//! ```sh
//! bunx --package @electric-sql/pglite-socket pglite-server --port 5499 &
//! ANISTREAM_TEST_POSTGRES='postgres://postgres@127.0.0.1:5499/postgres?sslmode=disable' \
//!   cargo test -p anistream-plugin-store --test postgres
//! ```

use anistream_plugin_store::{
    client::verify_index,
    protocol::{PART_SIZE, Role},
    server::{self, NewInvite, Storage, StoreError},
};
use anistream_store_core::inspect::fake::{FakeInspector, plugin};

const NOW: i64 = 1_800_000_000;
const BASE: &str = "https://plugins.example";

#[tokio::test]
async fn the_store_keeps_its_rules_on_postgres() {
    let Ok(url) = std::env::var("ANISTREAM_TEST_POSTGRES") else {
        eprintln!("skipping: set ANISTREAM_TEST_POSTGRES to a Postgres URL to run this");
        return;
    };
    let storage = Storage::Postgres(url);
    let first = server::open(&storage, Some(BASE), Some("On Postgres")).await.unwrap();
    let key = first.public_key();
    // Closed before opening again: PGlite, which this is often run against, takes one connection
    // at a time. Opening again finds the same store: same key, same name.
    drop(first);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let store = server::open(&storage, None, None).await.unwrap();
    assert_eq!(store.public_key(), key);
    assert_eq!(store.name().await, "On Postgres");

    // Invites: owner, member, a race for one invite, a device.
    let owner_invite = store.fresh_owner_invite(BASE, NOW).await.unwrap().unwrap();
    let (_, owner) = store.redeem(&owner_invite.code, "desk", NOW).await.unwrap();
    assert_eq!(owner.role, Role::Owner);
    assert!(store.has_owner().await.unwrap());

    let new = NewInvite::member("Alex", Role::Member, Some(&owner.id), NOW + 60);
    let (_, invite) = store.create_invite(new, BASE, NOW).await.unwrap();
    let (first, second) = tokio::join!(
        store.redeem(&invite.code, "laptop", NOW),
        store.redeem(&invite.code, "phone", NOW)
    );
    assert_eq!(first.is_ok() as u8 + second.is_ok() as u8, 1, "one invite, one member");
    let (laptop, alex) = first.or(second).unwrap();
    let (_, device) =
        store.create_invite(NewInvite::device(&alex, NOW + 60), BASE, NOW).await.unwrap();
    let (phone, same) = store.redeem(&device.code, "phone", NOW).await.unwrap();
    assert_eq!(same.id, alex.id);

    let members = store.members().await.unwrap();
    assert_eq!(members.len(), 2);
    let alex_row = members.iter().find(|m| m.id == alex.id).unwrap();
    assert_eq!(alex_row.devices, 2);
    assert!(store.authenticate(&laptop, NOW + 1_000).await.unwrap().is_some());

    store.rename(&alex.id, "Alexandra").await.unwrap();
    store.set_role(&alex.id, Role::Owner, &owner.id).await.unwrap();
    store.set_role(&alex.id, Role::Member, &owner.id).await.unwrap();

    // Admin page sessions.
    let (link, _) = store.create_admin_link(&owner.id, BASE, NOW).await.unwrap();
    let (session, _) = store.open_session(link.rsplit('=').next().unwrap(), NOW).await.unwrap();
    assert_eq!(store.session(&session, NOW).await.unwrap().unwrap().id, owner.id);
    assert!(matches!(
        store.open_session(link.rsplit('=').next().unwrap(), NOW).await,
        Err(StoreError::LinkUnusable)
    ));

    // Failed attempts.
    for _ in 0..anistream_store_core::store::ALLOWED_FAILURES {
        store.fail("203.0.113.7", NOW).await.unwrap();
    }
    assert!(store.blocked("203.0.113.7", NOW).await.unwrap());

    // Plugins, in the database, published from parts.
    let mut wasm = plugin("example", "1.0.0", &["api.example"]);
    wasm.resize(PART_SIZE as usize + 1_234, b' ');
    let (upload, parts) = store.begin_upload(&owner.id, wasm.len() as u64, NOW).await.unwrap();
    assert_eq!(parts, 2);
    for (index, part) in wasm.chunks(PART_SIZE as usize).enumerate() {
        store.put_part(&upload, &owner.id, index as u64, part.to_vec(), NOW).await.unwrap();
    }
    let entry =
        store.finish_upload(&upload, &owner.id, Some(&FakeInspector), None, NOW).await.unwrap();
    assert_eq!(entry.size, wasm.len() as u64);
    let (_, bytes) = store.plugin_bytes("example", "1.0.0").await.unwrap().unwrap();
    assert_eq!(bytes, wasm);
    // The second part, cut by Postgres rather than after reading the whole plugin.
    let part = store.plugin_range(&entry, PART_SIZE, PART_SIZE).await.unwrap().unwrap();
    assert_eq!(part, wasm[PART_SIZE as usize..]);

    let index = verify_index(&store.signed_index().await.unwrap(), &key, 0).unwrap();
    assert_eq!(index.plugins, vec![entry]);
    let sequence = index.sequence;
    store
        .publish(&plugin("example", "1.1.0", &[]), Some(&FakeInspector), NOW + 5)
        .await
        .unwrap();
    assert!(matches!(
        store
            .publish(&plugin("example", "1.1.0", &["x.example"]), Some(&FakeInspector), NOW)
            .await,
        Err(StoreError::Conflict(_))
    ));
    store.unpublish("example", NOW).await.unwrap();
    let index = verify_index(&store.signed_index().await.unwrap(), &key, 0).unwrap();
    assert_eq!(index.sequence, sequence + 2);
    assert!(index.plugins.is_empty());

    // Removing someone ends everything they had.
    store.remove_member(&alex.id, &owner.id, NOW).await.unwrap();
    assert!(store.authenticate(&laptop, NOW).await.unwrap().is_none());
    assert!(store.authenticate(&phone, NOW).await.unwrap().is_none());
}
