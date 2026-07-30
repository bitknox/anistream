//! The conformance suite: does a real component actually load, run, and stay inside its box?
//!
//! Integration tests against **three** compiled components, because everything interesting about a
//! plugin host is in the parts unit tests cannot reach — whether bindgen's glue matches the WIT,
//! whether epoch interruption really stops a loop, whether the allowlist holds when a guest
//! genuinely tries to leave.
//!
//! | Component | Language | Job |
//! |---|---|---|
//! | `example-rust` | Rust | The reference implementation. |
//! | `example-ts` | JavaScript | Proves the ABI is language-agnostic rather than Rust-shaped. |
//! | `test-hostile` | Rust | Attacks the host: spins, allocates, tries to escape. |
//!
//! [`assert_reference_behaviour`] is the load-bearing one: the *same* assertions run against the
//! Rust and JavaScript components. Two guests sharing nothing but a `.wit` file, producing
//! identical results, is what makes "providers are pluggable across languages" a demonstration
//! rather than an argument.
//!
//! The suite skips what has not been built rather than failing — `cargo test` on a fresh clone
//! should not require a wasm toolchain and an npm install.
//!
//! ```sh
//! cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml
//! cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-hostile/Cargo.toml
//! cd plugins/example-ts && npm install && npm run build
//! cargo test -p anistream-plugin
//! ```

use std::{path::PathBuf, time::Duration};

use anistream_plugin::{
    engine::{GuestError, LoadedPlugin, PluginError, PluginHost},
    sandbox::Limits,
};

fn plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

/// A built component, or `None` with the command that would build it.
fn built(path: PathBuf, how: &str) -> Option<PathBuf> {
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipping: {} is not built.\n  {how}", path.display());
        None
    }
}

/// The Rust reference component.
fn component() -> Option<PathBuf> {
    built(
        plugins_dir()
            .join("example-rust/target/wasm32-wasip2/release/anistream_example_plugin.wasm"),
        "cargo build --release --target wasm32-wasip2 \
         --manifest-path plugins/example-rust/Cargo.toml",
    )
}

/// The JavaScript component, built with `jco componentize`.
fn component_js() -> Option<PathBuf> {
    built(
        plugins_dir().join("example-ts/anistream-example-plugin-ts.wasm"),
        "cd plugins/example-ts && npm install && npm run build",
    )
}

/// A host at the **default** limits.
///
/// Deliberately not raised for the JavaScript component. It embeds a whole JS engine and is 170×
/// the size of the Rust one, so a larger ceiling seemed obviously necessary — measured, it is not.
/// Asserting the default works is the stronger claim: a plugin author in either language needs no
/// special configuration.
fn default_host() -> PluginHost {
    PluginHost::new(Limits::default(), None).unwrap()
}

/// Every assertion that must hold for *any* correct plugin, in any language.
///
/// This is the polyglot claim, made testable. If a second language needed even one different
/// expectation here, the ABI would be leaking the host's implementation language.
async fn assert_reference_behaviour(plugin: &LoadedPlugin, expected_id: &str) {
    // Identity.
    let manifest = plugin.manifest();
    assert_eq!(manifest.id, expected_id);
    assert!(!manifest.version.is_empty(), "a plugin must report a version");
    assert_eq!(manifest.allowed_hosts, vec!["httpbin.org"]);
    assert!(manifest.translation_types.contains(&"sub".to_string()));
    assert!(manifest.translation_types.contains(&"dub".to_string()));

    // Describing itself again, with capabilities this time, must agree.
    assert_eq!(plugin.describe().await.expect("describe").id, expected_id);

    // Records cross the ABI intact, including `option` fields.
    let episodes = plugin
        .list_episodes("example:frieren", "sub")
        .await
        .expect("no trap")
        .expect("episodes");
    assert_eq!(episodes.len(), 3);
    assert_eq!(episodes[0].number, "1");
    assert_eq!(episodes[0].duration_secs, Some(1_440));
    assert!(
        episodes[0].title.as_deref().unwrap().contains("frieren"),
        "got {:?}",
        episodes[0].title
    );

    // `not-found` survives, which the host's failover rules depend on.
    let missing = plugin.list_episodes("not-an-example-id", "sub").await.expect("no trap");
    assert!(matches!(missing, Err(GuestError::NotFound)), "got {missing:?}");

    // The lent AES, from inside a guest that carries no crypto of its own.
    let streams =
        plugin.resolve("example:frieren", "1", "sub").await.expect("no trap").expect("streams");
    assert_eq!(streams.len(), 1);
    assert_eq!(
        streams[0].url, "https://cdn.example.test/master.m3u8",
        "the host's AES did not produce the expected plaintext"
    );
    assert_eq!(streams[0].kind, "hls");
    assert_eq!(streams[0].quality, Some(1080));
    assert!(streams[0].headers.iter().any(|(n, _)| n == "referer"));
    assert_eq!(streams[0].subtitles.len(), 1);
    assert_eq!(streams[0].subtitles[0].language, "eng");
    assert!(!streams[0].subtitles[0].hard);
    assert_eq!(streams[0].subtitles[0].format.as_deref(), Some("vtt"));
    assert_eq!(streams[0].download_source, None, "this source streams only");

    // The 1.0.0 episode surface: a synopsis, an air date, and a positive canon claim all
    // cross the ABI rather than being silently dropped.
    assert!(
        episodes[0].description.as_deref().unwrap_or_default().contains("echoed"),
        "got {:?}",
        episodes[0].description
    );
    assert_eq!(episodes[0].air_date.as_deref(), Some("2026-01-01"));
    assert_eq!(episodes[0].filler, Some(false), "a claim of canon, not an absence of claim");

    // The Sources overlay surface: a candidate slate, and a pick routed back by its own id —
    // which must resolve to the *picked* quality, not the automatic one.
    let candidates = plugin
        .sources("example:frieren", "1", "sub")
        .await
        .expect("no trap")
        .expect("candidates");
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].quality, Some(1080));
    assert!(candidates[1].size.is_some());
    let picked = plugin
        .resolve_source("example:frieren", "1", "sub", &candidates[1].id)
        .await
        .expect("no trap")
        .expect("streams");
    assert_eq!(picked[0].quality, Some(720), "the pick undone would be a silent substitution");
}

#[tokio::test]
async fn a_real_component_loads_and_describes_itself() {
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");

    let manifest = plugin.manifest();
    assert_eq!(manifest.id, "example-rust");
    assert_eq!(manifest.display_name, "Example (Rust)");
    assert!(!manifest.version.is_empty());
    assert_eq!(manifest.allowed_hosts, vec!["httpbin.org"]);
    assert!(manifest.translation_types.contains(&"sub".to_string()));
}

#[tokio::test]
async fn describe_runs_with_no_capabilities_at_all() {
    // A manifest must not be able to depend on the network access it is about to request, or the
    // allowlist would be circular: you would need the manifest to authorise the fetch that the
    // manifest needs.
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");
    assert_eq!(plugin.describe().await.expect("describe").id, "example-rust");
}

#[tokio::test]
async fn a_guest_calling_a_disallowed_host_is_denied() {
    // The security property the whole design rests on. The plugin declares `httpbin.org`; with no
    // client wired up its fetch fails, but the *denial* path is what this checks — the guest sees
    // a host error and turns it into a provider error rather than reaching anything.
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");

    // With `http: None` the host reports a transport failure, which the guest maps to `blocked`.
    // Either way the call returns cleanly rather than trapping, which is the point: a guest
    // cannot distinguish "denied" from "unreachable" by crashing the host.
    let result = plugin.search("frieren", "sub").await.expect("no trap");
    assert!(
        matches!(result, Err(GuestError::Blocked(_)) | Err(GuestError::Other(_))),
        "expected a clean provider error, got {result:?}"
    );
}

#[tokio::test]
async fn the_error_vocabulary_survives_the_abi() {
    // `not-found` must arrive as `not-found`, because the host's failover rules treat it
    // differently from every other error — it is the one that does *not* try the next provider.
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");

    let result = plugin.list_episodes("not-an-example-id", "sub").await.expect("no trap");
    assert!(matches!(result, Err(GuestError::NotFound)), "got {result:?}");
}

#[tokio::test]
async fn a_guest_can_return_structured_results() {
    // Proves records cross the ABI intact in both directions, including `option` fields.
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");

    let episodes = plugin
        .list_episodes("example:frieren", "sub")
        .await
        .expect("no trap")
        .expect("episodes");
    assert_eq!(episodes.len(), 3);
    assert_eq!(episodes[0].number, "1");
    assert_eq!(episodes[0].duration_secs, Some(1_440));
    assert!(episodes[0].title.as_deref().unwrap().contains("frieren"));
}

#[tokio::test]
async fn the_lent_aes_works_from_inside_a_guest() {
    // The reason `aes-decrypt` is a host function: this plugin carries no crypto at all, yet
    // decrypts a real AES-128-CBC payload. A guest bundling its own would dwarf the parser.
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");

    let streams =
        plugin.resolve("example:frieren", "1", "sub").await.expect("no trap").expect("streams");
    assert_eq!(streams.len(), 1);
    assert_eq!(
        streams[0].url, "https://cdn.example.test/master.m3u8",
        "the host's AES did not produce the expected plaintext"
    );
    assert_eq!(streams[0].kind, "hls");
    assert_eq!(streams[0].quality, Some(1080));
    // Referer travels with the stream, because a referer-locked CDN 403s without it.
    assert!(streams[0].headers.iter().any(|(n, _)| n == "referer"));
    assert_eq!(streams[0].subtitles.len(), 1);
    assert!(!streams[0].subtitles[0].hard);
}

#[tokio::test]
async fn calls_do_not_leak_state_into_one_another() {
    // A fresh store per call is what stops a guest stashing something from your search and using
    // it during someone else's. Same input, same output, twice.
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");

    let first = plugin.list_episodes("example:a", "sub").await.unwrap().unwrap();
    let second = plugin.list_episodes("example:a", "sub").await.unwrap().unwrap();
    assert_eq!(first.len(), second.len());
    assert_eq!(first[0].title, second[0].title);
}

#[tokio::test]
async fn a_plugin_directory_loads_every_component_in_it() {
    let Some(path) = component() else { return };
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(&path, dir.path().join("a.wasm")).unwrap();
    std::fs::copy(&path, dir.path().join("b.wasm")).unwrap();
    std::fs::write(dir.path().join("notes.txt"), b"ignored").unwrap();

    let host = PluginHost::new(Limits::default(), None).unwrap();
    let loaded = host.load_dir(dir.path()).await;
    assert_eq!(loaded.len(), 2, "should have loaded exactly the two .wasm files");
    assert!(loaded.iter().all(|r| r.is_ok()));
}

#[tokio::test]
async fn a_tiny_memory_ceiling_is_enforced_rather_than_ignored() {
    // The ceiling has to bite. Wasm memory grows in 64 KiB pages and a component needs several
    // just to instantiate, so a one-page ceiling must fail — if this passes, the limiter is not
    // wired up and the memory bound is decorative.
    let Some(path) = component() else { return };
    let host =
        PluginHost::new(Limits { memory_bytes: 64 * 1024, ..Limits::default() }, None).unwrap();

    // Loading itself instantiates, so the failure surfaces there.
    let result = host.load(&path).await;
    assert!(result.is_err(), "a 64 KiB ceiling must stop a real component from instantiating");
}

#[tokio::test]
async fn a_generous_memory_ceiling_permits_normal_work() {
    // The other half of the previous test: the ceiling must not be so eager that it breaks
    // ordinary plugins.
    let Some(path) = component() else { return };
    let host =
        PluginHost::new(Limits { memory_bytes: 32 * 1024 * 1024, ..Limits::default() }, None)
            .unwrap();
    let plugin = host.load(&path).await.expect("load");
    assert!(plugin.list_episodes("example:a", "sub").await.unwrap().is_ok());
}

#[tokio::test]
async fn a_deadline_of_one_tick_still_completes_a_fast_call() {
    // Guards the tick arithmetic: an off-by-one that set the deadline to zero would make every
    // call fail, and a test that only checked a *long* deadline would not notice.
    //
    // A few ticks rather than exactly one. At one tick this was a coin flip whenever the machine
    // was busy — the epoch advances every 100ms regardless of scheduling, so a call that starts
    // just before a tick has almost no budget, and the adversarial tests in this same file
    // deliberately saturate the CPU. Three ticks still fails loudly if the deadline is computed as
    // zero, which is the actual bug being guarded, without depending on the scheduler.
    let Some(path) = component() else { return };
    let host = PluginHost::new(
        Limits { deadline: Duration::from_millis(300), ..Limits::default() },
        None,
    )
    .unwrap();
    let plugin = host.load(&path).await.expect("load");
    assert!(plugin.describe().await.is_ok(), "a trivial call must fit in one tick");
}

// ── the polyglot claim ───────────────────────────────────────────────────────────────────────
//
// The same assertions, against two guests that share nothing but a `.wit` file.

#[tokio::test]
async fn the_rust_component_satisfies_the_reference_behaviour() {
    let Some(path) = component() else { return };
    let host = default_host();
    let plugin = host.load(&path).await.expect("load");
    assert_reference_behaviour(&plugin, "example-rust").await;
}

#[tokio::test]
async fn the_javascript_component_satisfies_the_same_reference_behaviour() {
    // *The* polyglot test. A JavaScript component, built by `jco componentize`, indistinguishable
    // from the Rust one at this boundary. If this needed even one different expectation, the ABI
    // would be leaking the host's implementation language.
    let Some(path) = component_js() else { return };
    let host = default_host();
    let plugin = host.load(&path).await.expect("load");
    assert_reference_behaviour(&plugin, "example-ts").await;
}

#[tokio::test]
async fn both_languages_report_the_same_display_shape() {
    // Not just the same values — the same *kinds* of value. A plugin author in either language
    // should be able to read the other's manifest and recognise it.
    let (Some(rust), Some(js)) = (component(), component_js()) else { return };
    let host = default_host();
    let rust = host.load(&rust).await.expect("load rust");
    let js = host.load(&js).await.expect("load js");

    assert_eq!(rust.manifest().allowed_hosts, js.manifest().allowed_hosts);
    assert_eq!(rust.manifest().translation_types, js.manifest().translation_types);
    assert_ne!(rust.manifest().id, js.manifest().id, "they are still distinct providers");
    // Both name themselves for a human, and neither leaves it blank.
    assert!(rust.manifest().display_name.contains("Example"));
    assert!(js.manifest().display_name.contains("Example"));
}

#[tokio::test]
async fn the_javascript_component_needs_no_wasi_at_all() {
    // Built with `--disable all`, the JS component imports only `anistream:provider/host` — no WASI
    // floor whatsoever, unlike the Rust one whose standard library pulls one in. Worth asserting
    // because it proves the host's imports are sufficient on their own: a guest needs nothing but
    // the four lent functions.
    let Some(path) = component_js() else { return };
    assert!(default_host().load(&path).await.is_ok(), "a WASI-free component must load");
}

#[tokio::test]
async fn granted_settings_reach_the_guest_and_only_the_guest_they_name() {
    // The whole `config-get` contract in one test: a granted key arrives, and the reference
    // plugin's documented fallback covers everything else — the host never invents a value.
    let Some(path) = component() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap().with_plugin_settings(
        [(
            "example-rust".to_string(),
            [("cdn".to_string(), "mirror.example.net".to_string())].into_iter().collect(),
        )]
        .into_iter()
        .collect(),
    );
    let plugin = host.load(&path).await.expect("load");

    let streams =
        plugin.resolve("example:frieren", "1", "sub").await.expect("no trap").expect("streams");
    assert_eq!(
        streams[0].url, "https://mirror.example.net/master.m3u8",
        "the configured mirror should replace the baked-in default"
    );
}

// ── the adversary ────────────────────────────────────────────────────────────────────────────
//
// `plugins/test-hostile` exists to attack the host. A sandbox with no adversary in its test suite
// is a sandbox nobody has tried.

fn hostile() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../plugins/test-hostile/target/wasm32-wasip2/release/anistream_hostile_plugin.wasm",
    );
    if path.exists() {
        Some(path)
    } else {
        eprintln!(
            "skipping adversarial test: {} is not built.\n  \
             cargo build --release --target wasm32-wasip2 \
             --manifest-path plugins/test-hostile/Cargo.toml",
            path.display()
        );
        None
    }
}

#[tokio::test]
async fn a_setting_never_granted_is_none_rather_than_a_leak() {
    // `config-get` on a host with no settings attached. The hostile plugin reports any value
    // it receives as an `ESCAPED` error; a clean empty list is the host answering `none`.
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");

    let result = plugin.sources("x", "1", "sub").await.expect("no trap").expect("no leak");
    assert!(result.is_empty());
}

#[tokio::test]
async fn an_infinite_loop_is_stopped_by_the_deadline() {
    // *The* sandbox test. A guest in `loop {}` never yields, so nothing except epoch interruption
    // can stop it — a `tokio::time::timeout` around the call would hang forever, because the
    // future is never polled again. If this test hangs, the host is broken in the way that would
    // freeze the whole UI on a bad plugin.
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(
        Limits { deadline: Duration::from_millis(500), ..Limits::default() },
        None,
    )
    .unwrap();
    let plugin = host.load(&path).await.expect("load");

    let started = std::time::Instant::now();
    let error =
        plugin.search("anything", "sub").await.expect_err("the guest should be stopped");
    let elapsed = started.elapsed();

    assert!(
        matches!(error, PluginError::Deadline { .. }),
        "expected a deadline, got {error:?}"
    );
    // Generous, but it must be bounded: the point is that it returns at all.
    assert!(elapsed < Duration::from_secs(10), "took {elapsed:?} to stop a spinning guest");
}

#[tokio::test]
async fn a_fetch_to_an_undeclared_host_is_denied_before_any_connection() {
    // The plugin declares `allowed.example` and then tries `exfiltrate.example`. The denial has
    // to come from the host's own check — a plugin that could reach an arbitrary host would make
    // the manifest decorative.
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(Limits::default(), None).unwrap();
    let plugin = host.load(&path).await.expect("load");
    assert_eq!(plugin.manifest().allowed_hosts, vec!["allowed.example"]);

    let result = plugin.list_episodes("x", "sub").await.expect("no trap");
    let Err(GuestError::Other(message)) = result else {
        panic!("expected the guest to report the denial, got {result:?}");
    };
    assert!(message.starts_with("denied:"), "the host did not deny the fetch: {message}");
    assert!(!message.contains("ESCAPED"), "{message}");
    // And it names what was refused, so a user can see which plugin overreached.
    assert!(message.contains("exfiltrate.example"), "{message}");
}

#[tokio::test]
async fn unbounded_allocation_is_stopped_by_the_memory_ceiling() {
    // A guest allocating 4 MiB at a time until something stops it. Either the ceiling refuses the
    // growth — which in Rust means an allocation failure and a trap — or the call is cut off by
    // the deadline. Both are the sandbox working; escaping is not.
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(
        Limits {
            memory_bytes: 24 * 1024 * 1024,
            deadline: Duration::from_secs(5),
            ..Limits::default()
        },
        None,
    )
    .unwrap();
    let plugin = host.load(&path).await.expect("load");

    match plugin.resolve("x", "1", "sub").await {
        // Trapped on a failed allocation, or cut off by the deadline.
        Err(PluginError::Trap { .. } | PluginError::Deadline { .. }) => {}
        Err(other) => panic!("unexpected host error: {other:?}"),
        Ok(Err(GuestError::Other(message))) if message.contains("ESCAPED") => {
            panic!("the memory ceiling did not hold: {message}");
        }
        // A guest that handled its own allocation failure gracefully is also fine — it was still
        // bounded, which is all the host promises.
        Ok(other) => panic!("expected the guest to be stopped, got {other:?}"),
    }
}

#[tokio::test]
async fn one_hostile_plugin_does_not_prevent_loading_another() {
    // Plugins must be independent: a directory containing something broken should still yield the
    // working ones, or one bad file would take the whole source list down.
    let (Some(good), Some(bad)) = (component(), hostile()) else { return };
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(&good, dir.path().join("a-good.wasm")).unwrap();
    std::fs::copy(&bad, dir.path().join("b-hostile.wasm")).unwrap();
    std::fs::write(dir.path().join("c-junk.wasm"), b"not wasm at all").unwrap();

    let host = PluginHost::new(Limits::default(), None).unwrap();
    let loaded = host.load_dir(dir.path()).await;
    assert_eq!(loaded.len(), 3);

    let ids: Vec<String> =
        loaded.iter().filter_map(|r| r.as_ref().ok().map(|p| p.id().to_owned())).collect();
    assert!(ids.contains(&"example-rust".to_string()), "the good plugin should still load");
    // The hostile one loads fine — it only misbehaves when called, which is exactly why the
    // per-call limits matter more than load-time validation.
    assert!(ids.contains(&"test-hostile".to_string()));
    assert_eq!(loaded.iter().filter(|r| r.is_err()).count(), 1, "only the junk should fail");
}

#[tokio::test]
async fn a_stopped_plugin_can_be_called_again() {
    // A deadline must not poison the plugin: the next call gets a fresh store, so a guest that
    // hung once is not permanently broken. Without this, one slow search would disable a source
    // for the rest of the session.
    let Some(path) = hostile() else { return };
    let host = PluginHost::new(
        Limits { deadline: Duration::from_millis(300), ..Limits::default() },
        None,
    )
    .unwrap();
    let plugin = host.load(&path).await.expect("load");

    assert!(plugin.search("a", "sub").await.is_err());
    // `describe` is trivial and must still work afterwards.
    assert_eq!(plugin.describe().await.expect("describe after a deadline").id, "test-hostile");
}

#[tokio::test]
async fn a_load_failure_names_the_file() {
    // A plugin directory is user-writable, so "one of your plugins is broken" is useless — the
    // message has to say which.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broken.wasm");
    std::fs::write(&path, b"\0asm-but-not-really").unwrap();

    let host = PluginHost::new(Limits::default(), None).unwrap();
    let error = host.load(&path).await.unwrap_err();
    assert!(matches!(error, PluginError::Load { .. }));
    assert!(error.to_string().contains("broken.wasm"), "{error}");
}
