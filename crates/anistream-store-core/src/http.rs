//! The store over HTTP: routing, authentication, the admin page, and the protocol's error shape.
//!
//! In terms of a plain [`Request`] and [`Response`] rather than any server library's, so the same
//! routes answer behind hyper in the reference server, in a Worker and in a serverless function.
//! Each of those only translates its own request type into this one and back.

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    admin,
    blobs::Blobs,
    db::Database,
    inspect::Inspector,
    protocol::{
        AdminLink, AdminLogin, AdminOverview, ClaimRequest, Claimed, ErrorBody, InviteCreated,
        InviteRequest, Me, Member, MemberChange, MemberList, OpenJoinRequest, OpenSetting,
        PART_SIZE, PreviewRequest, PublishRequest, RedeemRequest, RedeemResponse, Role,
        ServerInfo, StoreInfo, UploadRequest, UploadStarted,
    },
    store::{NewInvite, SESSION_SECS, Store, StoreError},
};

/// The admin page's session cookie.
const SESSION_COOKIE: &str = "anistream_admin";

/// The header the admin page sets on everything it sends. A cross-site form cannot set a custom
/// header, so its presence — with the right `Origin` — is what shows the page itself is asking.
const ADMIN_HEADER: &str = "x-anistream-admin";

/// The largest JSON body any endpoint takes.
const MAX_JSON_BYTES: usize = 64 * 1024;

/// The largest body any request may carry: one upload part, and a little room.
pub const MAX_BODY_BYTES: usize = PART_SIZE as usize + MAX_JSON_BYTES;

/// Where the store is, as this request reached it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// The address invite and sign-in links are built from, with no trailing slash.
    pub base: String,
    /// `scheme://host[:port]` this request was made to: the only `Origin` the admin page's
    /// changes are accepted from.
    pub origin: String,
    /// The path the store is mounted under, with no trailing slash. Empty at the root.
    pub prefix: String,
    /// Whether the store is reached over HTTPS, which makes the session cookie `Secure`.
    pub secure: bool,
}

impl Site {
    /// Everything about a site from its base address.
    pub fn from_base(base: &str) -> Self {
        let url = url::Url::parse(base).ok();
        Self {
            base: base.trim_end_matches('/').to_owned(),
            origin: url.as_ref().map(|u| u.origin().ascii_serialization()).unwrap_or_default(),
            prefix: url
                .as_ref()
                .map(|u| u.path().trim_end_matches('/').to_owned())
                .unwrap_or_default(),
            secure: url.as_ref().is_some_and(|u| u.scheme() == "https"),
        }
    }
}

/// A request, as every adapter hands it over.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    /// The path, without the query.
    pub path: String,
    /// Header names in lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Who is asking, for counting failed attempts: the client's address as the platform reports
    /// it.
    pub client: String,
    pub site: Site,
    /// Seconds since the epoch. Passed in, because some places this runs have no clock to read.
    pub now: i64,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }

    fn headers_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.headers.iter().filter(move |(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self { status, headers: vec![("content-type".into(), content_type.into())], body }
    }

    fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_owned(), value.into()));
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
}

/// A store, answering requests.
pub struct Service<D, B> {
    pub store: Store<D, B>,
    /// The WebAssembly runtime that reads uploads, where there is one. Without it, uploads must
    /// carry their manifest.
    inspector: Option<Box<dyn Inspector>>,
    /// The code that claims a store nobody owns yet, when it was deployed with one.
    setup_code: Option<String>,
    /// Take a publisher's SHA-256 rather than computing it, for stores on a tight CPU budget.
    trust_digests: bool,
    /// What the store runs, for the admin page.
    server: ServerInfo,
}

impl<D: Database, B: Blobs> Service<D, B> {
    pub fn new(
        store: Store<D, B>,
        inspector: Option<Box<dyn Inspector>>,
        setup_code: Option<String>,
    ) -> Self {
        Self {
            store,
            inspector,
            setup_code: setup_code.filter(|c| !c.trim().is_empty()),
            trust_digests: false,
            server: ServerInfo::new(env!("CARGO_PKG_VERSION"), None),
        }
    }

    /// Say what the store runs: its version, its build and where it runs.
    pub fn describing(mut self, server: ServerInfo) -> Self {
        self.server = server;
        self
    }

    /// Take publishers' digests rather than computing them. For a Worker on Cloudflare's free
    /// plan, where hashing a large plugin costs more CPU than a request is allowed.
    pub fn trusting_digests(mut self) -> Self {
        self.trust_digests = true;
        self
    }

    pub async fn handle(&self, request: Request) -> Response {
        match self.route(&request).await {
            Ok(response) => response,
            Err(failure) => failure.into_response(),
        }
    }

    async fn route(&self, request: &Request) -> Result<Response, Failure> {
        let site = &request.site;
        let full = request.path.as_str();
        let path = full
            .strip_prefix(site.prefix.as_str())
            .filter(|rest| {
                !site.prefix.is_empty() && (rest.is_empty() || rest.starts_with('/'))
            })
            .unwrap_or(full);
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let now = request.now;
        let store = &self.store;

        match (request.method.as_str(), segments.as_slice()) {
            ("GET", []) => {
                let name = escape(&store.name().await);
                let fallback = home_page(&name, self.unclaimed().await);
                Ok(admin::home(
                    &escape(&site.prefix),
                    &format!("{name} · anistream"),
                    &fallback,
                ))
            }
            // Never redeems. A chat app fetching the link for a preview must not spend it.
            ("GET", ["join", code]) => {
                let open = store.is_open_code(code).await?;
                Ok(html(&join_page(&store.name().await, open)))
            }
            // The page is served from a directory so its assets and API calls resolve relative
            // to it, wherever the store is mounted.
            ("GET", ["admin"]) if !full.ends_with('/') => {
                Ok(Response::new(308, "text/plain", Vec::new())
                    .with("location", format!("{}/admin/", site.prefix)))
            }
            ("GET", ["admin", rest @ ..]) => admin::asset(&rest.join("/"))
                .ok_or_else(|| Failure::new(404, "not_found", "no such file")),
            ("GET", ["v1", "store"]) => {
                let info =
                    StoreInfo { claimable: self.unclaimed().await, ..store.info().await };
                json(200, &info)
            }
            // For a platform's health check: answers once the database does.
            ("GET", ["healthz"]) => {
                store.has_owner().await?;
                Ok(Response::new(200, "text/plain; charset=utf-8", b"ok".to_vec()))
            }
            ("POST", ["v1", "redeem"]) => self.redeem(request).await,
            ("POST", ["v1", "preview"]) => self.preview(request).await,
            ("POST", ["v1", "open", "preview"]) => self.open_preview(request).await,
            ("POST", ["v1", "open", "join"]) => self.open_join(request).await,
            ("GET", ["v1", "open"]) => {
                self.owner(request).await?;
                json(200, &store.open_state(&site.base).await?)
            }
            ("PUT", ["v1", "open"]) => {
                let owner = self.owner(request).await?;
                let setting: OpenSetting = read_json(request)?;
                store.set_open(setting.open).await?;
                tracing::info!(by = %owner.name, open = setting.open, "who can join changed");
                json(200, &store.open_state(&site.base).await?)
            }
            ("DELETE", ["v1", "open", "devices"]) => {
                let owner = self.owner(request).await?;
                let removed = store.remove_anonymous(now).await?;
                tracing::info!(by = %owner.name, removed, "anonymous devices removed");
                json(200, &store.open_state(&site.base).await?)
            }
            ("POST", ["v1", "setup"]) => self.claim(request).await,
            ("GET", ["v1", "me"]) => {
                let (member, device) = self.member(request).await?;
                json(200, &Me { member, device })
            }
            ("DELETE", ["v1", "me"]) => {
                let (member, device) = self.member(request).await?;
                store.revoke_device(&device.id, now).await?;
                tracing::info!(member = %member.name, device = %device.label, "device left");
                Ok(empty())
            }
            ("GET", ["v1", "index"]) => {
                self.member(request).await?;
                json(200, &store.signed_index().await?)
            }
            ("GET", ["v1", "plugins", id, version]) => {
                self.member(request).await?;
                self.download(request, id, version).await
            }
            ("POST", ["v1", "invites"]) => {
                let member = self.caller(request).await?;
                let body: InviteRequest = read_json(request)?;
                self.create_invite(&member, body, request).await
            }
            ("GET", ["v1", "members"]) => {
                self.owner(request).await?;
                let list = MemberList {
                    members: store.members().await?,
                    invites: store.pending_invites(now).await?,
                };
                json(200, &list)
            }
            ("DELETE", ["v1", "members", id]) => {
                let owner = self.owner(request).await?;
                store.remove_member(id, &owner.id, now).await?;
                tracing::info!(by = %owner.name, member = %id, "member removed");
                Ok(empty())
            }
            ("PATCH", ["v1", "members", id]) => {
                let owner = self.owner(request).await?;
                let change: MemberChange = read_json(request)?;
                if let Some(name) = &change.name {
                    store.rename(id, name).await?;
                }
                if let Some(role) = change.role {
                    store.set_role(id, role, &owner.id).await?;
                }
                tracing::info!(by = %owner.name, member = %id, "member changed");
                Ok(empty())
            }
            ("DELETE", ["v1", "invites", id]) => {
                self.owner(request).await?;
                store.cancel_invite(id, now).await?;
                Ok(empty())
            }
            ("DELETE", ["v1", "devices", id]) => {
                let owner = self.owner(request).await?;
                store.revoke_any_device(id, now).await?;
                tracing::info!(by = %owner.name, device = %id, "device revoked");
                Ok(empty())
            }
            ("POST", ["v1", "uploads"]) => {
                let owner = self.owner(request).await?;
                let body: UploadRequest = read_json(request)?;
                let (id, parts) = store.begin_upload(&owner.id, body.size, now).await?;
                json(200, &UploadStarted { id, part_size: PART_SIZE, parts })
            }
            ("PUT", ["v1", "uploads", id, part]) => {
                let owner = self.owner(request).await?;
                let part: u64 = part
                    .parse()
                    .map_err(|_| Failure::new(400, "bad_request", "parts are numbered"))?;
                store.put_part(id, &owner.id, part, request.body.clone(), now).await?;
                Ok(empty())
            }
            ("POST", ["v1", "plugins"]) => {
                let owner = self.owner(request).await?;
                let body: PublishRequest = read_json(request)?;
                let digest = body.sha256.as_deref().filter(|_| self.trust_digests);
                let entry = store
                    .finish_upload(
                        &body.upload,
                        &owner.id,
                        self.inspector.as_deref(),
                        digest,
                        now,
                    )
                    .await?;
                tracing::info!(by = %owner.name, id = %entry.id, version = %entry.version, "published");
                json(200, &entry)
            }
            ("DELETE", ["v1", "plugins", id]) => {
                let owner = self.owner(request).await?;
                store.unpublish(id, now).await?;
                tracing::info!(by = %owner.name, %id, "unpublished");
                Ok(empty())
            }
            ("POST", ["v1", "admin", "links"]) => {
                // From the app, with a device token — never from a session, or one stolen cookie
                // could keep minting fresh ones past its own expiry.
                let (member, _) = self.member(request).await?;
                if member.role != Role::Owner {
                    return Err(forbidden());
                }
                let (link, expires_at) =
                    store.create_admin_link(&member.id, &site.base, now).await?;
                json(200, &AdminLink { link, expires_at })
            }
            ("POST", ["v1", "admin", "session"]) => {
                same_origin(request)?;
                self.admin_login(request).await
            }
            ("DELETE", ["v1", "admin", "session"]) => {
                if let Some(token) = session_cookie(request) {
                    same_origin(request)?;
                    store.close_session(&token).await?;
                }
                Ok(empty().with("set-cookie", clear_cookie(site)))
            }
            ("GET", ["v1", "admin", "overview"]) => {
                let me = self.owner(request).await?;
                let overview = AdminOverview {
                    store: store.info().await,
                    url: site.base.clone(),
                    fingerprint: store.public_key().fingerprint(),
                    me,
                    sequence: store.sequence().await,
                    plugins: store.plugins().await?,
                    members: store.members().await?,
                    invites: store.pending_invites(now).await?,
                    open: store.open_state(&site.base).await?,
                    server: Some(self.server.clone()),
                };
                json(200, &overview)
            }
            _ => Err(Failure::new(404, "not_found", "no such endpoint")),
        }
    }

    /// Whether the store can still be claimed with a setup code.
    async fn unclaimed(&self) -> bool {
        self.setup_code.is_some() && !self.store.has_owner().await.unwrap_or(true)
    }

    async fn limited(&self, request: &Request, what: &str) -> Result<(), Failure> {
        if self.store.blocked(&request.client, request.now).await? {
            return Err(Failure::new(
                429,
                "rate_limited",
                format!("too many {what} that did not work — wait a few minutes"),
            ));
        }
        Ok(())
    }

    async fn redeem(&self, request: &Request) -> Result<Response, Failure> {
        self.limited(request, "invites").await?;
        let body: RedeemRequest = read_json(request)?;
        self.not_the_open_link(&body.invite).await?;
        match self.store.redeem(&body.invite, &body.device, request.now).await {
            Ok((token, member)) => {
                // Never the code or the token: logs outlive the moment, and both are credentials.
                tracing::info!(member = %member.name, role = member.role.as_str(), "joined");
                json(200, &RedeemResponse { token, member })
            }
            Err(StoreError::InviteUnusable) => {
                self.store.fail(&request.client, request.now).await?;
                Err(StoreError::InviteUnusable.into())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// What an invite leads to, for its confirmation: the signed index, to whoever holds an
    /// invite that would join. Spends nothing, and a code that would not join counts toward the
    /// same limit as a failed redeem.
    async fn preview(&self, request: &Request) -> Result<Response, Failure> {
        self.limited(request, "invites").await?;
        let body: PreviewRequest = read_json(request)?;
        self.not_the_open_link(&body.invite).await?;
        match self.store.check_invite(&body.invite, request.now).await {
            Ok(()) => json(200, &self.store.signed_index().await?),
            Err(StoreError::InviteUnusable) => {
                self.store.fail(&request.client, request.now).await?;
                Err(StoreError::InviteUnusable.into())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Refuse the open link's code where an invite's is expected, in words.
    ///
    /// A client from before open links reads an open link as an invite, since the two differ
    /// only by `?open`, and sends its code here. It is not a guess, so it counts toward nothing,
    /// and not a 404, which such a client takes for a store without previews.
    async fn not_the_open_link(&self, code: &str) -> Result<(), Failure> {
        if self.store.is_open_code(code).await? {
            return Err(Failure::new(
                400,
                "open_link",
                format!(
                    "Anyone with this link can join {}. This anistream is too old to join that \
                     way, so update it and paste the link again.",
                    self.store.name().await
                ),
            ));
        }
        Ok(())
    }

    /// What the open link leads to, for its confirmation: the signed index, as an invite's
    /// preview answers it. A code that would not join counts toward the same limit as a failed
    /// invite.
    async fn open_preview(&self, request: &Request) -> Result<Response, Failure> {
        self.limited(request, "links").await?;
        let body: OpenJoinRequest = read_json(request)?;
        match self.store.check_open(&body.code).await {
            Ok(()) => json(200, &self.store.signed_index().await?),
            Err(StoreError::OpenUnusable) => {
                self.store.fail(&request.client, request.now).await?;
                Err(StoreError::OpenUnusable.into())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Join with the open link: a token for a member with no name. No device label is asked
    /// for, so nothing about who joined is kept.
    async fn open_join(&self, request: &Request) -> Result<Response, Failure> {
        self.limited(request, "links").await?;
        let body: OpenJoinRequest = read_json(request)?;
        match self.store.join_open(&body.code, &request.client, request.now).await {
            Ok((token, member)) => {
                tracing::info!("a device joined with the open link");
                json(200, &RedeemResponse { token, member })
            }
            Err(StoreError::OpenUnusable) => {
                self.store.fail(&request.client, request.now).await?;
                Err(StoreError::OpenUnusable.into())
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn claim(&self, request: &Request) -> Result<Response, Failure> {
        let Some(configured) = &self.setup_code else {
            return Err(Failure::new(
                404,
                "not_found",
                "this store was not deployed with a setup code — its owner invite is in its log",
            ));
        };
        self.limited(request, "setup codes").await?;
        let body: ClaimRequest = read_json(request)?;
        match self.store.claim(&body.code, configured, &request.site.base, request.now).await {
            Ok((invite, expires_at)) => {
                tracing::info!("claimed with the setup code");
                json(200, &Claimed { link: invite.link(), expires_at })
            }
            Err(StoreError::Forbidden(message)) => {
                self.store.fail(&request.client, request.now).await?;
                Err(Failure::new(403, "forbidden", message))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Trade a sign-in link's code for a session cookie.
    async fn admin_login(&self, request: &Request) -> Result<Response, Failure> {
        self.limited(request, "sign-in links").await?;
        let body: AdminLogin = read_json(request)?;
        match self.store.open_session(&body.code, request.now).await {
            Ok((token, member)) => {
                tracing::info!(member = %member.name, "signed in to the admin page");
                Ok(json(200, &member)?
                    .with("set-cookie", session_cookie_header(&request.site, &token)))
            }
            Err(StoreError::LinkUnusable) => {
                self.store.fail(&request.client, request.now).await?;
                Err(StoreError::LinkUnusable.into())
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn create_invite(
        &self,
        member: &Member,
        request_body: InviteRequest,
        request: &Request,
    ) -> Result<Response, Failure> {
        let now = request.now;
        let expires_at = Store::<D, B>::default_expiry(now);
        let new = match &request_body {
            // A device that joined with the open link has no name to make an invite for, and
            // another device of theirs joins the same way.
            InviteRequest::Device if member.name.is_empty() => {
                return Err(Failure::new(
                    403,
                    "forbidden",
                    "this device joined with the store's link. Join another one the same way",
                ));
            }
            // Anyone may add a device for themselves: it grants nothing they do not already have.
            InviteRequest::Device => NewInvite::device(member, expires_at),
            InviteRequest::Member { name, role } => {
                if member.role != Role::Owner {
                    return Err(forbidden());
                }
                NewInvite::member(name, *role, Some(&member.id), expires_at)
            }
        };
        let (id, invite) = self.store.create_invite(new, &request.site.base, now).await?;
        json(200, &InviteCreated { id, link: invite.link(), expires_at })
    }

    /// A plugin's bytes, whole or one range of at most [`PART_SIZE`]. A range is read from
    /// storage on its own, never cut from the whole plugin.
    async fn download(
        &self,
        request: &Request,
        id: &str,
        version: &str,
    ) -> Result<Response, Failure> {
        let not_found = || Failure::new(404, "not_found", "no such plugin version");
        let entry = self.store.published(id, version).await?.ok_or_else(not_found)?;
        let total = entry.size;
        let headers = |response: Response| {
            response
                .with("accept-ranges", "bytes")
                .with("cache-control", "private, max-age=31536000, immutable")
        };
        let Some(range) = request.header("range") else {
            let (_, bytes) =
                self.store.plugin_bytes(id, version).await?.ok_or_else(not_found)?;
            return Ok(headers(Response::new(200, "application/wasm", bytes)));
        };
        match parse_range(range, total) {
            Some((start, end)) => {
                let body = self
                    .store
                    .plugin_range(&entry, start, end - start + 1)
                    .await?
                    .ok_or_else(not_found)?;
                Ok(headers(
                    Response::new(206, "application/wasm", body)
                        .with("content-range", format!("bytes {start}-{end}/{total}")),
                ))
            }
            None => Ok(Response::new(416, "text/plain", Vec::new())
                .with("content-range", format!("bytes */{total}"))),
        }
    }

    /// The member a request's device token belongs to.
    async fn member(
        &self,
        request: &Request,
    ) -> Result<(Member, crate::protocol::Device), Failure> {
        let token = bearer(request).ok_or_else(unauthorized)?;
        self.store.authenticate(token, request.now).await?.ok_or_else(unauthorized)
    }

    /// Who is asking, by device token or admin page session. A token wins when both are present.
    async fn caller(&self, request: &Request) -> Result<Member, Failure> {
        if bearer(request).is_some() {
            return self.member(request).await.map(|(member, _)| member);
        }
        let token = session_cookie(request).ok_or_else(unauthorized)?;
        // A browser sends the cookie whoever asked it to make the request. For anything that
        // changes something, the page has to show it was the one asking.
        if !matches!(request.method.as_str(), "GET" | "HEAD") {
            same_origin(request)?;
        }
        self.store.session(&token, request.now).await?.ok_or_else(unauthorized)
    }

    /// The owner making a request, by device token or admin page session.
    async fn owner(&self, request: &Request) -> Result<Member, Failure> {
        let member = self.caller(request).await?;
        if member.role != Role::Owner {
            return Err(forbidden());
        }
        Ok(member)
    }
}

/// `bytes=start-end` or `bytes=start-`, within the file, at most [`PART_SIZE`] long.
fn parse_range(header: &str, total: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || total == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = match end.trim() {
        "" => total - 1,
        end => end.parse().ok()?,
    };
    let end = end.min(total - 1).min(start.saturating_add(PART_SIZE - 1));
    (start <= end).then_some((start, end))
}

fn bearer(request: &Request) -> Option<&str> {
    request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn session_cookie(request: &Request) -> Option<String> {
    request
        .headers_named("cookie")
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Refuse a request that did not come from the admin page on this store's own address.
fn same_origin(request: &Request) -> Result<(), Failure> {
    let marked = request.header(ADMIN_HEADER).is_some();
    let origin = request.header("origin");
    if marked && origin == Some(request.site.origin.as_str()) {
        Ok(())
    } else {
        Err(Failure::new(
            403,
            "forbidden",
            format!("open the admin page at {} to do that", request.site.base),
        ))
    }
}

fn session_cookie_header(site: &Site, token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path={}; Max-Age={SESSION_SECS}; HttpOnly; SameSite=Strict{}",
        cookie_path(site),
        if site.secure { "; Secure" } else { "" }
    )
}

fn clear_cookie(site: &Site) -> String {
    format!(
        "{SESSION_COOKIE}=; Path={}; Max-Age=0; HttpOnly; SameSite=Strict{}",
        cookie_path(site),
        if site.secure { "; Secure" } else { "" }
    )
}

fn cookie_path(site: &Site) -> &str {
    if site.prefix.is_empty() { "/" } else { &site.prefix }
}

fn read_json<T: DeserializeOwned>(request: &Request) -> Result<T, Failure> {
    if request.body.len() > MAX_JSON_BYTES {
        return Err(Failure::new(413, "too_large", "the body is over the 64 KiB limit"));
    }
    serde_json::from_slice(&request.body)
        .map_err(|e| Failure::new(400, "bad_request", format!("the body is not valid: {e}")))
}

fn json<T: Serialize>(status: u16, value: &T) -> Result<Response, Failure> {
    let body =
        serde_json::to_vec(value).map_err(|e| Failure::new(500, "internal", e.to_string()))?;
    Ok(Response::new(status, "application/json", body).with("cache-control", "no-store"))
}

fn empty() -> Response {
    Response {
        status: 204,
        headers: vec![("cache-control".into(), "no-store".into())],
        body: Vec::new(),
    }
}

fn html(page: &str) -> Response {
    Response::new(200, "text/html; charset=utf-8", page.as_bytes().to_vec())
        .with("cache-control", "no-store")
        // The join page's URL holds an invite code. No script, no outside resource, and no
        // referrer means nothing on the page can carry it anywhere.
        .with("content-security-policy", "default-src 'none'; style-src 'unsafe-inline'")
        .with("referrer-policy", "no-referrer")
        .with("x-content-type-options", "nosniff")
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{title}</title><style>\
         body{{font:16px/1.6 system-ui,sans-serif;max-width:34rem;margin:4rem auto;padding:0 1rem;\
         color:#1d1b26;background:#f0ede4}}\
         @media (prefers-color-scheme:dark){{body{{color:#e4e0d6;background:#161a2e}}}}\
         h1{{font-size:1.4rem;margin-bottom:.25rem}}p.dim{{opacity:.7}}\
         ol{{padding-left:1.25rem}}code{{font-size:.95em}}a{{color:inherit}}\
         </style></head><body>{body}</body></html>"
    )
}

/// What the store's root says without script: its name, what it is, and how to join. `name` is
/// escaped already. Links resolve against the admin page's directory, so `./` is that page.
fn home_page(name: &str, unclaimed: bool) -> String {
    let claim = if unclaimed {
        "<p>Nobody owns this store yet. If you deployed it, \
         <a href=\"./\">open the admin page</a> and claim it with its setup code.</p>"
    } else {
        ""
    };
    format!(
        "<main class=\"wrap sect\"><h1>{name}</h1><p class=\"dim\">An anistream plugin store.</p>\
         <p>People join this store with an invite link from its owners.</p>{claim}</main>"
    )
}

fn join_page(name: &str, open: bool) -> String {
    let name = escape(name);
    let (heading, note) = if open {
        (
            format!("Anyone with this link can join {name}"),
            "Joining asks for no name. Opening this page joined nothing.",
        )
    } else {
        (
            format!("You are invited to {name}"),
            "The link works once. Opening this page did not use it.",
        )
    };
    page(
        &format!("Join {name}"),
        &format!(
            "<h1>{heading}</h1>\
             <p class=\"dim\">An anistream plugin store.</p>\
             <ol><li>Open anistream and go to <b>Providers</b>.</li>\
             <li>Press Enter to open plugin stores, then choose <b>join a store</b>.</li>\
             <li>Paste the link you were sent. Copy all of it, including the part after \
             <code>#</code>.</li></ol>\
             <p class=\"dim\">{note}</p>"
        ),
    )
}

fn escape(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&#39;".into(),
            c => c.to_string(),
        })
        .collect()
}

/// An error on its way to becoming a protocol error body.
#[derive(Debug)]
struct Failure {
    status: u16,
    code: &'static str,
    message: String,
}

impl Failure {
    fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }

    fn into_response(self) -> Response {
        let body = ErrorBody { error: self.code.to_owned(), message: self.message };
        json(self.status, &body).unwrap_or_else(|_| empty())
    }
}

fn unauthorized() -> Failure {
    Failure::new(401, "unauthorized", "this store does not accept that token")
}

fn forbidden() -> Failure {
    Failure::new(403, "forbidden", "only the store's owner can do that")
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        let message = error.to_string();
        match error {
            StoreError::InviteUnusable => Self::new(404, "invite_unusable", message),
            StoreError::LinkUnusable => Self::new(404, "link_unusable", message),
            StoreError::NotFound(_) => Self::new(404, "not_found", message),
            StoreError::Forbidden(_) => Self::new(403, "forbidden", message),
            StoreError::Conflict(_) => Self::new(409, "conflict", message),
            StoreError::InvalidPlugin(_) => Self::new(400, "invalid_plugin", message),
            StoreError::BadRequest(_) => Self::new(400, "bad_request", message),
            StoreError::OpenUnusable => Self::new(404, "open_unusable", message),
            StoreError::TooManyJoins => Self::new(429, "rate_limited", message),
            StoreError::Setup(_)
            | StoreError::Db(_)
            | StoreError::Blob(_)
            | StoreError::Crypto(_) => {
                // The detail is for the server's log, not for whoever sent the request.
                tracing::error!(error = %message, "request failed");
                Self::new(500, "internal", "the store hit an error")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_bounded_by_the_file_and_the_part_size() {
        assert_eq!(parse_range("bytes=0-9", 100), Some((0, 9)));
        assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        let big = PART_SIZE * 3;
        assert_eq!(parse_range("bytes=0-", big), Some((0, PART_SIZE - 1)), "one part at most");
        assert_eq!(parse_range("bytes=100-50", 100), None);
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 100), None);
        assert_eq!(parse_range("items=0-1", 100), None);
    }

    #[test]
    fn a_site_is_read_from_its_base() {
        let site = Site::from_base("https://example.com/friends/");
        assert_eq!(site.base, "https://example.com/friends");
        assert_eq!(site.origin, "https://example.com");
        assert_eq!(site.prefix, "/friends");
        assert!(site.secure);
        let local = Site::from_base("http://127.0.0.1:7878");
        assert_eq!(local.prefix, "");
        assert!(!local.secure);
    }

    #[test]
    fn store_names_are_escaped_into_pages() {
        assert_eq!(escape("<script>&\"'"), "&lt;script&gt;&amp;&quot;&#39;");
    }
}
