//! The server and the client against each other, over real HTTP on loopback.

use std::sync::Arc;

use super::*;
use crate::{
    client::{ClientError, StoreClient},
    inspect::fake::{FakeInspector, plugin},
    invite::Invite,
    protocol::{InviteRequest, Role},
};

struct Running {
    _dir: tempfile::TempDir,
    server: Arc<Server>,
    base: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

async fn start() -> Running {
    start_under("").await
}

/// A store whose URL has `path` after the host, to exercise a reverse proxy that forwards with
/// or without it.
async fn start_under(path: &str) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let (listener, addr) = bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let base = format!("http://{addr}{path}");
    let store =
        open(&Storage::Dir(dir.path().into()), Some(&base), Some("Test store")).await.unwrap();
    let server = Arc::new(Server::new(
        store,
        Some(base.clone()),
        Some(Box::new(FakeInspector)),
        Some("let-me-in".into()),
        false,
    ));
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(serve(listener, server.clone(), None, async {
        let _ = stopped.await;
    }));
    Running { _dir: dir, server, base, stop: Some(stop) }
}

fn client() -> StoreClient {
    StoreClient::new(std::time::Duration::from_secs(10)).unwrap()
}

/// Join as the owner through the printed owner invite, as `anistream store serve` would.
async fn join_as_owner(running: &Running) -> (Invite, String) {
    let invite =
        running.server.store().fresh_owner_invite(&running.base, now()).await.unwrap().unwrap();
    let joined = client().redeem(&invite, "desk").await.unwrap();
    assert_eq!(joined.member.role, Role::Owner);
    (invite, joined.token)
}

async fn invite_member(running: &Running, owner: &str, name: &str) -> Invite {
    let created = client()
        .create_invite(
            &running.base,
            owner,
            &InviteRequest::Member { name: name.into(), role: Role::Member },
        )
        .await
        .unwrap();
    Invite::parse(&created.link).unwrap()
}

#[tokio::test]
async fn the_whole_journey_from_invite_to_verified_download() {
    let running = start().await;
    let client = client();
    let (owner_invite, owner) = join_as_owner(&running).await;
    let key = owner_invite.key;

    // The owner publishes; a friend joins with a link the owner made in the app.
    let wasm = plugin("example", "1.0.0", &["api.example"]);
    let published = client.publish(&running.base, &owner, wasm.clone()).await.unwrap();
    assert_eq!(published.id, "example");

    let invite = invite_member(&running, &owner, "Alex").await;
    let preview = client.preview(&invite).await.unwrap();
    assert_eq!(preview.name, "Test store");
    let alex = client.redeem(&invite, "laptop").await.unwrap();
    assert_eq!(alex.member.name, "Alex");
    assert_eq!(alex.member.role, Role::Member);

    let me = client.me(&running.base, &alex.token).await.unwrap();
    assert_eq!(me.device.label, "laptop");

    let index = client.index(&running.base, &alex.token, &key, 0).await.unwrap();
    assert_eq!(index.plugins.len(), 1);
    let bytes = client.download(&running.base, &alex.token, &index.plugins[0]).await.unwrap();
    assert_eq!(bytes, wasm);
}

#[tokio::test]
async fn an_invite_is_spent_once_and_a_preview_does_not_spend_it() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let invite = invite_member(&running, &owner, "Alex").await;

    // A chat app fetching the link for a preview.
    let page = reqwest::get(invite.link().split('#').next().unwrap()).await.unwrap();
    assert!(page.status().is_success());
    assert!(page.text().await.unwrap().contains("Test store"));

    client.preview(&invite).await.unwrap();
    client.redeem(&invite, "laptop").await.unwrap();
    let again = client.redeem(&invite, "phone").await.unwrap_err();
    assert_eq!(again.code(), Some("invite_unusable"));
}

#[tokio::test]
async fn an_invite_shows_who_runs_the_store_and_what_it_offers_before_it_is_spent() {
    let running = start().await;
    let client = client();
    let (owner_invite, owner) = join_as_owner(&running).await;
    let wasm = plugin("example", "1.0.0", &["api.example"]);
    client.publish(&running.base, &owner, wasm).await.unwrap();
    let invite = invite_member(&running, &owner, "Alex").await;

    let index = client.preview_index(&invite).await.unwrap().expect("a store with previews");
    assert_eq!(index.owners, ["Owner"]);
    assert_eq!(index.plugins.len(), 1);
    assert_eq!(index.plugins[0].allowed_hosts, ["api.example"]);

    // Not spent: it still joins, and the joined list says the same of who runs it.
    let alex = client.redeem(&invite, "laptop").await.unwrap();
    let joined = client.index(&running.base, &alex.token, &owner_invite.key, 0).await.unwrap();
    assert_eq!(joined.owners, ["Owner"]);
    // Spent now, so it shows nothing, and says why.
    let spent = client.preview_index(&invite).await.unwrap_err();
    assert_eq!(spent.code(), Some("invite_unusable"));

    // A link pairing the store with someone else's key: its list is not signed by that key.
    let forged =
        Invite { key: crate::crypto::SigningKey::generate().unwrap().public(), ..invite };
    let fresh = invite_member(&running, &owner, "Sam").await;
    let forged = Invite { code: fresh.code, ..forged };
    assert_eq!(client.preview_index(&forged).await.unwrap_err(), ClientError::BadSignature);
}

#[tokio::test]
async fn a_made_up_invite_previews_nothing_and_counts_as_a_failure() {
    let running = start().await;
    let (owner_invite, _) = join_as_owner(&running).await;
    let made_up = Invite { code: "k3TzQe9WbA1xLr7uVn2c0g".into(), ..owner_invite };
    for _ in 0..anistream_store_core::store::ALLOWED_FAILURES {
        let refused = client().preview_index(&made_up).await.unwrap_err();
        assert_eq!(refused.code(), Some("invite_unusable"));
    }
    let blocked = client().preview_index(&made_up).await.unwrap_err();
    assert_eq!(blocked.code(), Some("rate_limited"), "limited like a failed redeem");
}

#[tokio::test]
async fn a_store_from_before_previews_joins_without_one() {
    // Answers every request as a store without the endpoint does.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await;
            let body = r#"{"error":"not_found","message":"no such endpoint"}"#;
            let answer = format!(
                "HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(answer.as_bytes()).await;
        }
    });
    let key = crate::crypto::SigningKey::generate().unwrap().public();
    let invite = Invite { base, code: "k3TzQe9WbA1xLr7uVn2c0g".into(), key };
    assert_eq!(client().preview_index(&invite).await.unwrap(), None);
}

#[tokio::test]
async fn a_store_answering_with_another_key_is_not_joined() {
    let running = start().await;
    let invite =
        running.server.store().fresh_owner_invite(&running.base, now()).await.unwrap().unwrap();
    let forged =
        Invite { key: crate::crypto::SigningKey::generate().unwrap().public(), ..invite };
    assert_eq!(client().preview(&forged).await.unwrap_err(), ClientError::KeyMismatch);
}

#[tokio::test]
async fn members_cannot_do_what_owners_do() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let invite = invite_member(&running, &owner, "Alex").await;
    let alex = client.redeem(&invite, "laptop").await.unwrap().token;

    let forbidden = |e: ClientError| e.code().map(str::to_owned);
    assert_eq!(
        forbidden(
            client
                .create_invite(
                    &running.base,
                    &alex,
                    &InviteRequest::Member { name: "Sam".into(), role: Role::Owner },
                )
                .await
                .unwrap_err()
        ),
        Some("forbidden".into())
    );
    assert_eq!(
        forbidden(client.members(&running.base, &alex).await.unwrap_err()),
        Some("forbidden".into())
    );
    assert_eq!(
        forbidden(
            client.publish(&running.base, &alex, plugin("x", "1.0.0", &[])).await.unwrap_err()
        ),
        Some("forbidden".into())
    );
}

#[tokio::test]
async fn a_member_links_a_second_device_without_the_owner() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let invite = invite_member(&running, &owner, "Alex").await;
    let laptop = client.redeem(&invite, "laptop").await.unwrap();

    let device =
        client.create_invite(&running.base, &laptop.token, &InviteRequest::Device).await;
    let phone = client.redeem(&Invite::parse(&device.unwrap().link).unwrap(), "phone").await;
    assert_eq!(phone.unwrap().member.id, laptop.member.id);

    let list = client.members(&running.base, &owner).await.unwrap();
    let alex = list.members.iter().find(|m| m.name == "Alex").unwrap();
    assert_eq!(alex.devices, 2);
}

#[tokio::test]
async fn removing_a_member_locks_out_every_device_at_once() {
    let running = start().await;
    let client = client();
    let (owner_invite, owner) = join_as_owner(&running).await;
    let invite = invite_member(&running, &owner, "Alex").await;
    let alex = client.redeem(&invite, "laptop").await.unwrap();

    client.remove_member(&running.base, &owner, &alex.member.id).await.unwrap();
    assert_eq!(
        client.index(&running.base, &alex.token, &owner_invite.key, 0).await.unwrap_err(),
        ClientError::Unauthorized
    );
}

#[tokio::test]
async fn leaving_revokes_only_this_device() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let invite = invite_member(&running, &owner, "Alex").await;
    let laptop = client.redeem(&invite, "laptop").await.unwrap();
    let device =
        client.create_invite(&running.base, &laptop.token, &InviteRequest::Device).await;
    let phone =
        client.redeem(&Invite::parse(&device.unwrap().link).unwrap(), "phone").await.unwrap();

    client.leave(&running.base, &laptop.token).await.unwrap();
    assert_eq!(
        client.me(&running.base, &laptop.token).await.unwrap_err(),
        ClientError::Unauthorized
    );
    assert!(client.me(&running.base, &phone.token).await.is_ok());
}

#[tokio::test]
async fn no_token_or_a_made_up_one_is_unauthorized() {
    let running = start().await;
    let client = client();
    let key = running.server.store().public_key();
    assert_eq!(
        client.index(&running.base, "ast1_made_up", &key, 0).await.unwrap_err(),
        ClientError::Unauthorized
    );
    let bare = reqwest::get(format!("{}/v1/index", running.base)).await.unwrap();
    assert_eq!(bare.status(), 401);
    let body: crate::protocol::ErrorBody = bare.json().await.unwrap();
    assert_eq!(body.error, "unauthorized");
}

#[tokio::test]
async fn only_the_current_version_can_be_downloaded() {
    let running = start().await;
    let client = client();
    let (invite, owner) = join_as_owner(&running).await;
    client.publish(&running.base, &owner, plugin("example", "1.0.0", &[])).await.unwrap();
    let old =
        client.index(&running.base, &owner, &invite.key, 0).await.unwrap().plugins[0].clone();
    client.publish(&running.base, &owner, plugin("example", "1.1.0", &[])).await.unwrap();

    let gone = client.download(&running.base, &owner, &old).await.unwrap_err();
    assert_eq!(gone.code(), Some("not_found"));
}

#[tokio::test]
async fn the_index_sequence_moves_with_every_change_and_a_rollback_is_refused() {
    let running = start().await;
    let client = client();
    let (invite, owner) = join_as_owner(&running).await;
    client.publish(&running.base, &owner, plugin("one", "1.0.0", &[])).await.unwrap();
    client.publish(&running.base, &owner, plugin("two", "1.0.0", &[])).await.unwrap();
    client.unpublish(&running.base, &owner, "one").await.unwrap();

    let index = client.index(&running.base, &owner, &invite.key, 0).await.unwrap();
    assert_eq!(index.sequence, 3);
    assert_eq!(index.plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["two"]);
    // A client that has already seen a later list refuses this one.
    assert_eq!(
        client.index(&running.base, &owner, &invite.key, 4).await.unwrap_err(),
        ClientError::Rollback { seen: 4, got: 3 }
    );
}

#[tokio::test]
async fn a_store_behind_a_path_answers_with_and_without_it() {
    let running = start_under("/friends").await;
    let client = client();
    let (invite, owner) = join_as_owner(&running).await;
    assert_eq!(invite.base, running.base);
    client.index(&running.base, &owner, &invite.key, 0).await.unwrap();
    // A proxy that strips the prefix before forwarding.
    let stripped = running.base.trim_end_matches("/friends");
    client.index(stripped, &owner, &invite.key, 0).await.unwrap();
}

#[tokio::test]
async fn an_oversized_body_is_refused_before_it_is_read() {
    let running = start().await;
    let (_, owner) = join_as_owner(&running).await;
    let response = reqwest::Client::new()
        .put(format!("{}/v1/uploads/x/0", running.base))
        .bearer_auth(&owner)
        .header("content-length", (MAX_BODY_BYTES + 1).to_string())
        .body(vec![0u8; 16])
        .send()
        .await;
    // Either the server answers 413 straight away, or it closes the connection on a body that
    // does not match its declared length. Both refuse it; neither stores it.
    if let Ok(response) = response {
        assert_eq!(response.status(), 413);
    }
    assert!(running.server.store().plugins().await.unwrap().is_empty());
}

// --- the admin page -----------------------------------------------------------------------

/// A browser on the admin page: it keeps the cookie, and says where it is like a browser does.
struct Browser {
    http: reqwest::Client,
    base: String,
    cookie: Option<String>,
}

impl Browser {
    fn new(base: &str) -> Self {
        Self { http: reqwest::Client::new(), base: base.to_owned(), cookie: None }
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let origin = url::Url::parse(&self.base).unwrap().origin().ascii_serialization();
        let mut request = self
            .http
            .request(method, format!("{}/{path}", self.base))
            .header("origin", origin)
            .header("x-anistream-admin", "1");
        if let Some(cookie) = &self.cookie {
            request = request.header("cookie", cookie);
        }
        request
    }

    async fn sign_in(&mut self, link: &str) -> reqwest::Response {
        let code = link.rsplit("#login=").next().unwrap();
        let response = self
            .request(reqwest::Method::POST, "v1/admin/session")
            .json(&serde_json::json!({ "code": code }))
            .send()
            .await
            .unwrap();
        if let Some(set) = response.headers().get("set-cookie") {
            let set = set.to_str().unwrap().to_owned();
            self.cookie = Some(set.split(';').next().unwrap().to_owned());
        }
        response
    }
}

#[tokio::test]
async fn an_owner_signs_a_browser_in_and_manages_the_store_from_it() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let invite = invite_member(&running, &owner, "Alex").await;
    let alex = client.redeem(&invite, "laptop").await.unwrap();

    let link = client.admin_link(&running.base, &owner).await.unwrap();
    assert!(link.link.starts_with(&format!("{}/admin/#login=", running.base)), "{}", link.link);

    let mut browser = Browser::new(&running.base);
    let signed_in = browser.sign_in(&link.link).await;
    assert_eq!(signed_in.status(), 200);
    let cookie = signed_in.headers()["set-cookie"].to_str().unwrap().to_owned();
    for flag in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(cookie.contains(flag), "cookie lacks {flag}: {cookie}");
    }
    // Spent: the same link does not sign anyone else in.
    assert_eq!(Browser::new(&running.base).sign_in(&link.link).await.status(), 404);

    let overview: crate::protocol::AdminOverview = browser
        .request(reqwest::Method::GET, "v1/admin/overview")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(overview.store.name, "Test store");
    assert_eq!(overview.members.len(), 2);
    let alex_row = overview.members.iter().find(|m| m.name == "Alex").unwrap();
    assert_eq!(alex_row.device_list[0].label, "laptop");

    // Publish from the browser, in parts like the page does, then promote and remove someone.
    let wasm = plugin("example", "1.0.0", &[]);
    let started: crate::protocol::UploadStarted = browser
        .request(reqwest::Method::POST, "v1/uploads")
        .json(&serde_json::json!({ "size": wasm.len() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let part = browser
        .request(reqwest::Method::PUT, &format!("v1/uploads/{}/0", started.id))
        .body(wasm)
        .send()
        .await
        .unwrap();
    assert_eq!(part.status(), 204);
    let published = browser
        .request(reqwest::Method::POST, "v1/plugins")
        .json(&serde_json::json!({ "upload": started.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(published.status(), 200);
    let promoted = browser
        .request(reqwest::Method::PATCH, &format!("v1/members/{}", alex.member.id))
        .json(&serde_json::json!({ "role": "owner" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promoted.status(), 204);
    let renamed = browser
        .request(reqwest::Method::PATCH, &format!("v1/members/{}", overview.me.id))
        .json(&serde_json::json!({ "name": "Johan" }))
        .send()
        .await
        .unwrap();
    assert_eq!(renamed.status(), 204, "an owner renames themselves");
    let device = &alex_row.device_list[0].id;
    let revoked =
        browser.request(reqwest::Method::DELETE, &format!("v1/devices/{device}")).send().await;
    assert_eq!(revoked.unwrap().status(), 204);
    assert_eq!(
        client.me(&running.base, &alex.token).await.unwrap_err(),
        ClientError::Unauthorized
    );

    // Signed out: the cookie stops working.
    let out =
        browser.request(reqwest::Method::DELETE, "v1/admin/session").send().await.unwrap();
    assert!(out.headers()["set-cookie"].to_str().unwrap().contains("Max-Age=0"));
    let after =
        browser.request(reqwest::Method::GET, "v1/admin/overview").send().await.unwrap();
    assert_eq!(after.status(), 401);
}

#[tokio::test]
async fn the_session_cookie_alone_cannot_change_anything() {
    // What another site could make a browser send: the cookie, but not the header, or not this
    // store's origin.
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let link = client.admin_link(&running.base, &owner).await.unwrap();
    let mut browser = Browser::new(&running.base);
    browser.sign_in(&link.link).await;
    let cookie = browser.cookie.clone().unwrap();
    let http = reqwest::Client::new();
    let attempt = |origin: Option<&str>, marked: bool| {
        let mut request = http
            .post(format!("{}/v1/invites", running.base))
            .header("cookie", &cookie)
            .json(&serde_json::json!({ "kind": "member", "name": "Mallory", "role": "owner" }));
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        if marked {
            request = request.header("x-anistream-admin", "1");
        }
        request.send()
    };
    let here = url::Url::parse(&running.base).unwrap().origin().ascii_serialization();
    assert_eq!(attempt(Some("https://evil.example"), true).await.unwrap().status(), 403);
    assert_eq!(attempt(Some(&here), false).await.unwrap().status(), 403);
    assert_eq!(attempt(None, true).await.unwrap().status(), 403);
    assert_eq!(attempt(Some(&here), true).await.unwrap().status(), 200);

    // Reading is allowed without the header: SameSite keeps other sites from sending the cookie,
    // and a read changes nothing.
    let read = http
        .get(format!("{}/v1/admin/overview", running.base))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), 200);
}

#[tokio::test]
async fn only_an_owner_with_a_device_token_gets_a_sign_in_link() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let invite = invite_member(&running, &owner, "Alex").await;
    let alex = client.redeem(&invite, "laptop").await.unwrap();
    assert_eq!(
        client.admin_link(&running.base, &alex.token).await.unwrap_err().code(),
        Some("forbidden")
    );

    // A session cannot mint links for itself, or one stolen cookie could outlive its session.
    let link = client.admin_link(&running.base, &owner).await.unwrap();
    let mut browser = Browser::new(&running.base);
    browser.sign_in(&link.link).await;
    let minted = browser.request(reqwest::Method::POST, "v1/admin/links").send().await.unwrap();
    assert_eq!(minted.status(), 401);
}

#[tokio::test]
async fn the_page_is_served_with_a_strict_policy() {
    let running = start().await;
    let http =
        reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
    let bare = http.get(format!("{}/admin", running.base)).send().await.unwrap();
    assert_eq!(bare.status(), 308);
    assert_eq!(bare.headers()["location"], "/admin/");

    let page = http.get(format!("{}/admin/", running.base)).send().await.unwrap();
    assert_eq!(page.status(), 200);
    let policy = page.headers()["content-security-policy"].to_str().unwrap().to_owned();
    assert!(policy.contains("script-src 'self'") && policy.contains("frame-ancestors 'none'"));
    assert_eq!(page.headers()["x-frame-options"], "DENY");
    assert!(page.text().await.unwrap().contains("./admin.js"));

    for path in ["admin/admin.js", "admin/admin.css", "admin/fonts/OFL.txt"] {
        let file = http.get(format!("{}/{path}", running.base)).send().await.unwrap();
        assert_eq!(file.status(), 200, "{path}");
    }
    let missing =
        http.get(format!("{}/admin/../signing.key", running.base)).send().await.unwrap();
    assert_ne!(missing.status(), 200);
}

// --- parts, claiming, and stores that cannot run plugins ----------------------------------

/// A "plugin" several parts long: the fake inspector reads the JSON and ignores the padding.
fn big_plugin(id: &str, version: &str) -> Vec<u8> {
    let mut wasm = plugin(id, version, &["api.example"]);
    wasm.resize(crate::protocol::PART_SIZE as usize * 2 + 12_345, b' ');
    wasm
}

#[tokio::test]
async fn a_plugin_larger_than_a_part_goes_up_and_comes_down_in_parts() {
    let running = start().await;
    let client = client();
    let (invite, owner) = join_as_owner(&running).await;
    let wasm = big_plugin("example", "1.0.0");
    let entry = client.publish(&running.base, &owner, wasm.clone()).await.unwrap();
    assert_eq!(entry.size, wasm.len() as u64);

    let index = client.index(&running.base, &owner, &invite.key, 0).await.unwrap();
    let bytes = client.download(&running.base, &owner, &index.plugins[0]).await.unwrap();
    assert_eq!(bytes, wasm);
}

#[tokio::test]
async fn a_range_is_one_part_at_most_and_a_bad_one_is_refused() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let wasm = big_plugin("example", "1.0.0");
    client.publish(&running.base, &owner, wasm.clone()).await.unwrap();
    let http = reqwest::Client::new();
    let get = |range: &str| {
        http.get(format!("{}/v1/plugins/example/1.0.0", running.base))
            .bearer_auth(&owner)
            .header("range", range)
            .send()
    };
    let all = get("bytes=0-").await.unwrap();
    assert_eq!(all.status(), 206);
    assert_eq!(all.bytes().await.unwrap().len() as u64, crate::protocol::PART_SIZE);
    let tail = get("bytes=8388608-").await.unwrap();
    assert_eq!(tail.status(), 206);
    assert_eq!(tail.bytes().await.unwrap().len(), 12_345);
    assert_eq!(get(&format!("bytes={}-", wasm.len())).await.unwrap().status(), 416);
}

#[tokio::test]
async fn a_part_of_the_wrong_size_or_from_someone_else_is_refused() {
    let running = start().await;
    let client = client();
    let (_, owner) = join_as_owner(&running).await;
    let http = reqwest::Client::new();
    let started: crate::protocol::UploadStarted = http
        .post(format!("{}/v1/uploads", running.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "size": 10 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(started.parts, 1);
    let short = http
        .put(format!("{}/v1/uploads/{}/0", running.base, started.id))
        .bearer_auth(&owner)
        .body(vec![0u8; 9])
        .send()
        .await
        .unwrap();
    assert_eq!(short.status(), 400);
    // Another owner's upload is not yours to add to.
    let invite = client
        .create_invite(
            &running.base,
            &owner,
            &InviteRequest::Member { name: "Sam".into(), role: Role::Owner },
        )
        .await
        .unwrap();
    let sam = client.redeem(&Invite::parse(&invite.link).unwrap(), "desk").await.unwrap();
    let theirs = http
        .put(format!("{}/v1/uploads/{}/0", running.base, started.id))
        .bearer_auth(&sam.token)
        .body(vec![0u8; 10])
        .send()
        .await
        .unwrap();
    assert_eq!(theirs.status(), 404);
}

#[tokio::test]
async fn a_store_is_claimed_once_with_its_setup_code() {
    let running = start().await;
    let http = reqwest::Client::new();
    let claim = |code: &str| {
        http.post(format!("{}/v1/setup", running.base))
            .json(&serde_json::json!({ "code": code }))
            .send()
    };
    // The admin page asks before it offers the setup code form.
    assert!(client().info(&running.base).await.unwrap().claimable);
    assert_eq!(claim("wrong").await.unwrap().status(), 403);
    let claimed: crate::protocol::Claimed =
        claim("let-me-in").await.unwrap().json().await.unwrap();
    let invite = Invite::parse(&claimed.link).unwrap();
    let owner = client().redeem(&invite, "desk").await.unwrap();
    assert_eq!(owner.member.role, Role::Owner);
    // Claimed: the code does nothing more.
    assert_eq!(claim("let-me-in").await.unwrap().status(), 409);
    assert!(!client().info(&running.base).await.unwrap().claimable);
}

#[tokio::test]
async fn failed_claims_are_limited_like_failed_invites() {
    let running = start().await;
    let http = reqwest::Client::new();
    for _ in 0..anistream_store_core::store::ALLOWED_FAILURES {
        let response = http
            .post(format!("{}/v1/setup", running.base))
            .json(&serde_json::json!({ "code": "guess" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
    }
    let blocked = http
        .post(format!("{}/v1/setup", running.base))
        .json(&serde_json::json!({ "code": "let-me-in" }))
        .send()
        .await
        .unwrap();
    assert_eq!(blocked.status(), 429, "even the right code waits once the limit is reached");
}

/// The smallest WebAssembly component, for a store that reads manifests rather than plugins.
fn component() -> Vec<u8> {
    b"\0asm\x0d\x00\x01\x00".to_vec()
}

#[tokio::test]
async fn a_store_that_cannot_run_plugins_publishes_from_the_manifest_they_carry() {
    let dir = tempfile::tempdir().unwrap();
    let (listener, addr) = bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let base = format!("http://{addr}");
    let store = open(&Storage::Dir(dir.path().into()), Some(&base), None).await.unwrap();
    // No inspector: what a Worker or a serverless function is.
    let server = Arc::new(Server::new(store, Some(base.clone()), None, None, false));
    tokio::spawn(serve(listener, server.clone(), None, std::future::pending()));
    let invite = server.store().fresh_owner_invite(&base, now()).await.unwrap().unwrap();
    let owner = client().redeem(&invite, "desk").await.unwrap().token;

    let unstamped = client().publish(&base, &owner, component()).await.unwrap_err();
    assert_eq!(unstamped.code(), Some("invalid_plugin"));
    assert!(unstamped.to_string().contains("plugin stamp"), "{unstamped}");

    let info = crate::PluginInfo {
        id: "example".into(),
        name: "Example".into(),
        version: "1.0.0".into(),
        allowed_hosts: vec!["api.example".into()],
        translations: vec![],
    };
    let stamped = crate::manifest::stamp(&component(), &info).unwrap();
    let entry = client().publish(&base, &owner, stamped).await.unwrap();
    assert_eq!(entry.id, "example");
    assert_eq!(entry.allowed_hosts, vec!["api.example"]);
}

#[tokio::test]
async fn behind_a_platforms_proxy_the_store_learns_its_address_from_the_request() {
    let dir = tempfile::tempdir().unwrap();
    let (listener, addr) = bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let store = open(&Storage::Dir(dir.path().into()), None, None).await.unwrap();
    let server = Arc::new(Server::new(store, None, None, Some("code".into()), true));
    tokio::spawn(serve(listener, server, None, std::future::pending()));
    let claimed: crate::protocol::Claimed = reqwest::Client::new()
        .post(format!("http://{addr}/v1/setup"))
        .header("x-forwarded-host", "my-store.example")
        .header("x-forwarded-proto", "https")
        .json(&serde_json::json!({ "code": "code" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(claimed.link.starts_with("https://my-store.example/join/"), "{}", claimed.link);
}
