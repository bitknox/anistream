//! A plugin store end to end with the real WebAssembly host.
//!
//! An owner publishes the Rust reference plugin from their plugin directory, invites someone,
//! that person joins and installs it, and what lands on their disk is checked by loading it. Then
//! the owner removes them. Everything runs on loopback in temporary directories, with tokens in
//! files rather than the keychain.
//!
//! Skips itself when the reference plugin is not built, like the conformance suite:
//!
//! ```sh
//! cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml
//! ```

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anistream_core::config::{Config, Paths};
use anistream_engine::plugin_stores::{Stores, WasmInspector};
use anistream_model::{
    Msg,
    stores::{StorePluginStatus, StoreState},
};
use anistream_plugin_store::server::{self, Server, Storage};
use anistream_track::secret::{Backend, TokenStore};

const ID: &str = "example-rust";

fn component() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../plugins/example-rust/target/wasm32-wasip2/release/anistream_example_plugin.wasm",
    );
    if path.exists() {
        Some(path)
    } else if std::env::var_os("ANISTREAM_REQUIRE_PLUGINS").is_some() {
        panic!(
            "{} is not built, and ANISTREAM_REQUIRE_PLUGINS is set.\n  cargo build --release \
             --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml",
            path.display()
        );
    } else {
        eprintln!(
            "skipping: {} is not built.\n  cargo build --release --target wasm32-wasip2 \
             --manifest-path plugins/example-rust/Cargo.toml",
            path.display()
        );
        None
    }
}

/// A machine's directories under `root`.
fn machine(root: &Path) -> Paths {
    Paths {
        config_file: root.join("config").join("config.toml"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
    }
}

fn stores(root: &Path) -> (Paths, Stores) {
    let paths = machine(root);
    let tokens = TokenStore::with_backend(root.join("tokens"), Backend::File);
    let stores = Stores::with_tokens(&Config::default(), &paths, tokens).unwrap();
    (paths, stores)
}

/// What the store calls the device that joined it from `root`, by asking it with that device's
/// token.
async fn device_label(root: &Path, joined: &anistream_plugin_store::Joined) -> String {
    let tokens = TokenStore::with_backend(root.join("tokens"), Backend::File);
    let token = tokens.get(&joined.token_key()).unwrap();
    let client =
        anistream_plugin_store::StoreClient::new(std::time::Duration::from_secs(5)).unwrap();
    client.me(&joined.base, &token).await.unwrap().device.label
}

#[tokio::test]
async fn an_invite_join_names_the_device_by_its_kind_or_as_typed() {
    // The machine's own name was sent before, which is often its owner's. Now the store is
    // told the kind of device, or whatever the person joining typed in its place.
    let root = tempfile::tempdir().unwrap();
    let (listener, addr) = server::bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let base = format!("http://{addr}");
    let store =
        server::open(&Storage::Dir(root.path().join("store")), Some(&base), Some("Ours"))
            .await
            .unwrap();
    let owner_invite = store.fresh_owner_invite(&base, server::now()).await.unwrap().unwrap();
    let server = Arc::new(Server::new(store, Some(base), None, None, false));
    tokio::spawn(server::serve(listener, server, None, std::future::pending()));

    // The terminal app, which names no kind of device: a computer.
    let (_, owner) = stores(&root.path().join("owner"));
    let preview = owner.preview(&owner_invite.link()).await.unwrap();
    assert_eq!(preview.device_label, "Computer");
    let joined = owner.join(&owner_invite.link(), None).await.unwrap();
    let id = joined.id.clone();
    assert_eq!(device_label(&root.path().join("owner"), &joined).await, "Computer");

    // An iPad, joining as itself: the field left as it was offered.
    let invite = owner.invite(&id, Some("Alex".into())).await.unwrap();
    let (_, ipad) = stores(&root.path().join("ipad"));
    let ipad = ipad.labelled("iPad");
    let preview = ipad.preview(&invite.link.0).await.unwrap();
    assert_eq!(preview.device_label, "iPad");
    let joined = ipad.join(&invite.link.0, Some("   ")).await.unwrap();
    assert_eq!(device_label(&root.path().join("ipad"), &joined).await, "iPad");

    // A Mac whose owner typed a name of their own.
    let invite = owner.invite(&id, Some("Sam".into())).await.unwrap();
    let (_, mac) = stores(&root.path().join("mac"));
    let mac = mac.labelled("Mac");
    let joined = mac.join(&invite.link.0, Some("  Studio \n")).await.unwrap();
    assert_eq!(device_label(&root.path().join("mac"), &joined).await, "Studio");
}

#[tokio::test]
async fn an_owner_publishes_and_a_member_installs_a_real_plugin() {
    let Some(wasm) = component() else { return };
    let root = tempfile::tempdir().unwrap();

    // The store, served on loopback.
    let (listener, addr) = server::bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let base = format!("http://{addr}");
    let store =
        server::open(&Storage::Dir(root.path().join("store")), Some(&base), Some("Ours"))
            .await
            .unwrap();
    let owner_invite = store.fresh_owner_invite(&base, server::now()).await.unwrap().unwrap();
    let inspector = WasmInspector::new(anistream_plugin::Limits::default(), None).unwrap();
    let server = Arc::new(Server::new(
        store,
        Some(base.clone()),
        Some(Box::new(inspector)),
        None,
        false,
    ));
    tokio::spawn(server::serve(listener, server, None, std::future::pending()));

    // The owner, with the plugin dropped into their plugin directory by hand.
    let (owner_paths, owner) = stores(&root.path().join("owner"));
    std::fs::create_dir_all(owner_paths.plugin_dir()).unwrap();
    let original = std::fs::read(&wasm).unwrap();
    std::fs::write(owner_paths.plugin_dir().join(format!("{ID}.wasm")), &original).unwrap();

    let preview = owner.preview(&owner_invite.link()).await.unwrap();
    assert_eq!(preview.name, "Ours");
    assert!(!preview.already_joined);
    let joined = owner.join(&owner_invite.link(), None).await.unwrap();
    assert!(joined.is_owner());

    let local = owner.local_plugins(&joined.id).await.unwrap();
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].id, ID);
    assert_eq!(local[0].published, None);

    let entry = owner.publish(&joined.id, ID).await.unwrap();
    assert_eq!(entry.id, ID);
    assert!(!entry.allowed_hosts.is_empty(), "the reference plugin declares a host");
    let rows = owner.rows().unwrap();
    assert_eq!(
        rows[0].plugins[0].status,
        StorePluginStatus::Installed,
        "the owner's own copy is the published one"
    );

    // Someone new, invited from the app and joining on their own machine.
    let invite = owner.invite(&joined.id, Some("Alex".into())).await.unwrap();
    let (member_paths, member) = stores(&root.path().join("member"));
    let as_member = member.join(&invite.link.0, None).await.unwrap();
    assert_eq!(as_member.member.name, "Alex");
    assert!(!as_member.is_owner());

    let rows = member.rows().unwrap();
    let listed = &rows[0].plugins[0];
    assert_eq!(listed.status, StorePluginStatus::Available);
    assert_eq!(listed.hosts, entry.allowed_hosts);

    let (installed, path) = member.install(&joined.id, ID).await.unwrap();
    assert_eq!(installed.version, entry.version);
    assert_eq!(path, member_paths.plugin_dir().join(format!("{ID}.wasm")));
    // What was published, and so what was installed, is the plugin stamped with its manifest:
    // the same code, now readable by a store that cannot run it. The owner's copy is that file.
    let installed_bytes = std::fs::read(&path).unwrap();
    let published = std::fs::read(owner_paths.plugin_dir().join(format!("{ID}.wasm"))).unwrap();
    assert_eq!(installed_bytes, published);
    assert!(installed_bytes.starts_with(&original[..8]));
    let carried = anistream_plugin_store::manifest::read(&installed_bytes).unwrap().unwrap();
    assert_eq!(carried.id, ID);
    assert_eq!(carried.allowed_hosts, entry.allowed_hosts);
    assert_eq!(member.rows().unwrap()[0].plugins[0].status, StorePluginStatus::Installed);

    // What landed loads, and is the plugin the store listed.
    let host =
        anistream_plugin::PluginHost::new(anistream_plugin::Limits::default(), None).unwrap();
    let loaded = host.load(&path).await.unwrap();
    assert_eq!(loaded.manifest().id, ID);
    assert_eq!(loaded.manifest().allowed_hosts, entry.allowed_hosts);

    // The owner removes them: their next refresh says so, and what they installed stays.
    let (members, _) = owner.members(&joined.id).await.unwrap();
    let alex = members.iter().find(|m| m.name == "Alex").unwrap();
    owner.remove_member(&joined.id, &alex.id).await.unwrap();
    let rows = member.refresh().await.unwrap();
    assert_eq!(rows[0].state, StoreState::Revoked);
    assert!(path.exists());
    assert!(member.install(&joined.id, ID).await.is_err(), "no more downloads");
}

#[tokio::test]
async fn a_joined_stores_key_in_a_link_to_another_address_is_refused() {
    // A store is known by its public key, so anyone can put a joined store's key in a link to
    // their own server. Following it would re-point the store at that server and send it the
    // token being replaced.
    let root = tempfile::tempdir().unwrap();
    let (listener, addr) = server::bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let base = format!("http://{addr}");
    let store = server::open(&Storage::Dir(root.path().join("store")), Some(&base), None)
        .await
        .unwrap();
    let invite = store.fresh_owner_invite(&base, server::now()).await.unwrap().unwrap();
    let inspector = WasmInspector::new(anistream_plugin::Limits::default(), None).unwrap();
    let server =
        Arc::new(Server::new(store, Some(base), Some(Box::new(inspector)), None, false));
    tokio::spawn(server::serve(listener, server, None, std::future::pending()));

    let (_, stores) = stores(&root.path().join("member"));
    let joined = stores.join(&invite.link(), None).await.unwrap();

    // Same key, same shape of code, another address. Nothing listens there: the refusal has to
    // come before anything is sent.
    let elsewhere = anistream_plugin_store::Invite {
        base: "http://127.0.0.1:9".into(),
        code: anistream_plugin_store::crypto::new_code().unwrap(),
        key: invite.key,
        open: false,
    };
    let preview = stores.preview(&elsewhere.link()).await.unwrap_err();
    assert!(matches!(preview, Msg::StoreMoved { .. }), "{preview:?}");
    let join = stores.join(&elsewhere.link(), None).await.unwrap_err();
    assert!(matches!(join, Msg::StoreMoved { .. }), "{join:?}");

    let rows = stores.rows().unwrap();
    assert_eq!(rows[0].url, joined.base, "the record still points at the real store");
    assert_eq!(rows[0].state, StoreState::Ready);
}

#[tokio::test]
async fn an_open_store_is_joined_without_a_name_and_its_owner_decides_who_can_join() {
    let root = tempfile::tempdir().unwrap();
    let (listener, addr) = server::bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let base = format!("http://{addr}");
    let store =
        server::open(&Storage::Dir(root.path().join("store")), Some(&base), Some("Ours"))
            .await
            .unwrap();
    let invite = store.fresh_owner_invite(&base, server::now()).await.unwrap().unwrap();
    store.set_contact("owner@store.example").await.unwrap();
    let server = Arc::new(Server::new(store, Some(base), None, None, false));
    tokio::spawn(server::serve(listener, server, None, std::future::pending()));

    let (_, owner) = stores(&root.path().join("owner"));
    let id = owner.join(&invite.link(), None).await.unwrap().id;
    let closed = owner.open_join(&id).await.unwrap().expect("a store with open joining");
    assert!(!closed.open && closed.link.is_none());
    let opened = owner.set_open(&id, true).await.unwrap();
    let link = opened.link.expect("an open store's link").0;
    assert!(owner.rows().unwrap()[0].open, "the owner's row says so after the change");

    // Someone with only the link: the preview says it is open and joins without a name.
    let (_, guest) = stores(&root.path().join("guest"));
    let preview = guest.preview(&link).await.unwrap();
    assert!(preview.open && preview.anonymous);
    assert_eq!(preview.owners, ["Owner"]);
    assert_eq!(preview.device_label, "", "an open link sends no name, so none is offered");
    // A name typed anyway is not sent.
    let joined = guest.join(&link, Some("Living room")).await.unwrap();
    assert!(joined.anonymous);
    assert_eq!(joined.member.name, "");
    assert_eq!(device_label(&root.path().join("guest"), &joined).await, "");
    let row = &guest.rows().unwrap()[0];
    assert!(row.open && row.anonymous);
    assert_eq!(row.state, StoreState::Ready);
    let another = guest.invite(&id, None).await.unwrap_err();
    assert!(matches!(another, Msg::StoreInviteNeedsLink { .. }), "{another:?}");

    // The owner pasting the store's own link changes nothing: it would trade their name and
    // their owner rights for a member with neither.
    let refused = owner.preview(&link).await.unwrap_err();
    assert!(matches!(refused, Msg::StoreJoinWouldUnname { .. }), "{refused:?}");
    assert!(owner.join(&link, None).await.is_err());
    assert!(owner.rows().unwrap()[0].owner);

    let view = owner.open_join(&id).await.unwrap().unwrap();
    assert_eq!(view.devices, 1);
    assert_eq!(view.recent.len(), 1);
    let (members, _) = owner.members(&id).await.unwrap();
    assert_eq!(members.len(), 1, "counted, not listed");

    // Invites only again: the link stops, the device keeps working until it is removed.
    let closed = owner.set_open(&id, false).await.unwrap();
    assert!(!closed.open && closed.devices == 1);
    assert!(!owner.rows().unwrap()[0].open);
    let (_, late) = stores(&root.path().join("late"));
    let stopped = late.preview(&link).await.unwrap_err();
    assert_eq!(stopped, Msg::StoreOpenLinkUnusable);
    assert_eq!(guest.refresh().await.unwrap()[0].state, StoreState::Ready);
    let cleared = owner.remove_anonymous(&id, None).await.unwrap();
    assert_eq!(cleared.devices, 0);
    assert_eq!(guest.refresh().await.unwrap()[0].state, StoreState::Revoked);
}
