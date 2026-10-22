# Plugin stores

A plugin store is a small server that shares provider plugins with a group of people. Someone runs
one, invites the people they want, and everyone who joins can install and update the store's
plugins from the Providers screen. anistream ships no store and lists none. Every store is run by
someone you know, for the people they invite.

This page covers using a store, running one, and the protocol a store speaks, so another server
can be written against it.

## Joining a store

The owner sends you an invite link. It looks like this:

```
https://plugins.example/join/k3TzQe9WbA1xLr7uVn2c0g#Gv8s0qYH3rKj2m5WzT1nB6cXe4fLd9aP7uQiR0sMw8E
```

In anistream, open **Providers**, press Enter to open plugin stores, choose **join a store** and
paste the link. anistream checks the link against the store before using it, shows the store's name
and key, and asks you to confirm. After that the store's page lists its plugins. Enter on one shows
what it is allowed to contact, and Enter again installs it.

The link works once. If someone else used it first, joining fails, and the owner can see who joined
with it. An unused link expires after seven days.

To use the same store on a second computer, open the store and choose **link another device**. That
gives you a new one-time link for yourself.

## Running a store

```sh
anistream store serve ./my-store --url https://plugins.example --name "Our plugins"
```

The first run creates `./my-store` and prints an owner invite. Join with that link from your own
anistream and you become the store's owner. From then on the store is run from its page in the app,
or from its admin page in a browser.

The store's page in the app has what an owner needs day to day: invite someone, see who has joined,
remove someone, and publish a plugin you have installed locally.

The admin page has all of that and a little more room: rename people, make someone an owner, revoke
one lost device without removing its owner, and publish by dropping `.wasm` files onto the page. To
open it, choose **open the admin page** on the store's page in anistream, and your browser opens it
already signed in. On the machine that runs the store, `anistream store admin ./my-store` prints a
sign-in link instead. A sign-in link works once, for ten minutes, and a session lasts twelve hours.

`--url` is the address people will reach the store at, and it is what invite links are built from.
The server listens on `127.0.0.1:7878` unless `--listen` says otherwise.

### Deploying one

[`deploy/`](../deploy/README.md) has a template for each kind of host, every one of them running the
same store:

| Where                                        | Records kept in     | Plugin files kept in |
| -------------------------------------------- | ------------------- | -------------------- |
| Docker, a VPS or a NAS; Fly; Railway; Render | SQLite, on a volume | the same volume      |
| Heroku, or any container host with Postgres  | Postgres            | Postgres             |
| Cloudflare Workers                           | D1                  | R2                   |
| Vercel, Netlify                              | Postgres            | Postgres             |

The container hosts run the `anistream-store` image, `ghcr.io/bitknox/anistream-store`. Cloudflare,
Vercel and Netlify run the same code compiled to WebAssembly, with a little JavaScript that lends it
the platform's database and file storage.

A store on a host whose log is awkward to read is deployed with a setup code. While nobody owns
the store, its admin page asks for the code and answers with the owner invite. The code does nothing
once someone owns the store, and wrong guesses count toward the same limit as failed invites.

### Settings

Every option of `serve` can also be set in the environment, which is how the templates set them.

| Option                    | Environment                         | Meaning                                                        |
| ------------------------- | ----------------------------------- | -------------------------------------------------------------- |
| `DIR`                     | `ANISTREAM_STORE_DIR`               | The directory holding the database and plugin files            |
| `--database`              | `ANISTREAM_STORE_DATABASE_URL`      | A Postgres URL to keep everything in instead                   |
| `--url`                   | `ANISTREAM_STORE_URL`               | The address people reach the store at                          |
| `--name`                  | `ANISTREAM_STORE_NAME`              | The store's name, as members see it                            |
| `--listen`                | `ANISTREAM_STORE_LISTEN`            | Where to listen                                                |
| `--tls-cert`, `--tls-key` | `ANISTREAM_STORE_TLS_CERT`, `…_KEY` | Serve HTTPS here, from PEM files                               |
| `--trust-proxy`           | `ANISTREAM_STORE_TRUST_PROXY`       | Take the client's and the store's address from `X-Forwarded-*` |
| `--setup-code`            | `ANISTREAM_STORE_SETUP_CODE`        | A code that claims the store while nobody owns it              |

With neither a directory nor a database given, the store uses `DATABASE_URL` if it is set, and
then `ANISTREAM_STORE_DEFAULT_DIR`, which the container image sets to `/data`. So the image keeps
its records on its volume, unless the platform attaches a database. Without `--listen`, the store
listens on every interface at `PORT` when a platform sets one.

On Render, Railway, Fly and Heroku the store reads its own address from the platform and trusts the
platform's proxy without being told. A store that is given no address and trusts a proxy takes its
address from each request.

A Postgres URL's `sslmode` means what it means to libpq: `disable` for no TLS, `require` (and
`prefer`, the default) to encrypt without checking whose certificate it is, which is what Heroku's
databases need, and `verify-full` to check it as a browser would.

anistream only talks to a store over HTTPS, apart from stores on the same machine, and checks the
certificate the way a browser would. The server can terminate TLS itself with `--tls-cert` and
`--tls-key` (a certificate other machines trust, such as one from Let's Encrypt), or sit behind
something that already does:
Caddy, a Cloudflare Tunnel, or `tailscale serve`. Tailscale keeps the store off the public internet
entirely, which suits a store shared by a handful of people.

A few things also work without the app:

```sh
anistream store invite ./my-store --name Alex            # print an invite link
anistream store publish ./my-store --plugin plugin.wasm  # publish or update a plugin
anistream store admin ./my-store                         # print a sign-in link for the admin page
```

The server also comes on its own, as `anistream-store`, without the player and the rest of the app.
It is what the container image runs, and it takes the same commands: `anistream-store serve`,
`anistream-store invite`, and so on.

## What a store protects

Invites and tokens decide who can download from a store. Nobody without a token or an unspent
invite can list its plugins, and nobody without a token can fetch them.

Everyone who joins does get the plugins themselves. A plugin is a `.wasm` file on each member's
machine, and a member can read it or watch what it contacts. A store keeps its plugins away from
people who were not invited, not from the people who were.

What keeps members safe is the sandbox and the signature. A plugin can only reach the hosts it
declares, and anistream shows those hosts before installing and again if an update adds new ones.
Each store has a signing key, and the invite link carries its public half. anistream refuses a
plugin list that was not signed with that key, a plugin whose bytes do not match the signed list,
and a list older than one it has already seen. In the reference server the key lives with the
server, so the signature protects against tampering between the server and you, and against a
different server answering at the same address. It does not protect against someone who controls
the store's machine, or its database on a platform that keeps the key there.

Removing a member stops their updates and new installs straight away. Plugins they already
downloaded stay on their machine.

## Protocol

Version 1. Everything is JSON over HTTPS except plugin downloads. A store lives at a base URL,
which may include a path (`https://example.com/anistream`), and every path below is relative to it.

### Invite links

```
{base}/join/{code}#{key}
```

`code` is the invite code, 16 random bytes in unpadded base64url. `key` is the store's Ed25519
public key, 32 bytes in unpadded base64url. The key travels in the fragment so it never reaches
the server or its logs.

A `GET` of the join path must never use up the invite. Chat apps fetch links to build previews, and
an invite that redeemed on `GET` would be spent before the person it was sent to saw it. The
reference server answers it with a page explaining how to paste the link into anistream.

A client accepts plain `http` only for loopback hosts.

### Authentication

`POST /v1/redeem` trades an invite code for a token. Every other authenticated request carries that
token as `Authorization: Bearer {token}`. A token belongs to one device of one member. Servers store
a hash of each token and each invite code, never the value.

A request with a missing, unknown or revoked token gets `401`. A member asking for an owner action
gets `403`.

### The admin page

The page is served at `{base}/admin/`. A sign-in link is `{base}/admin/#login={code}`, with the code
in the fragment so it stays out of logs, and the page trades it for a session the moment it loads
and removes it from the address bar. Only `POST /v1/admin/links` makes one, and only with a device
token: a session cannot extend itself by making new links.

The session is an `HttpOnly`, `SameSite=Strict` cookie, `Secure` when the store's address is HTTPS.
It counts as an owner's credential on the owner endpoints and on `POST /v1/invites`, never on the
member endpoints, which take device tokens only. Anything sent with the cookie that changes
something must also carry an `X-Anistream-Admin` header and an `Origin` equal to the store's own,
which a page on another site cannot arrange. A session ends at sign-out, after twelve hours, or the
moment its owner is removed or stops being an owner.

The page runs under a content security policy that allows nothing but its own files: no inline
script, no outside resources, no framing.

### Errors

Every error body has the same shape:

```json
{ "error": "invite_unusable", "message": "this invite was used, cancelled or has expired" }
```

| Status | `error`           | When                                           |
| ------ | ----------------- | ---------------------------------------------- |
| 400    | `bad_request`     | The body or path could not be understood       |
| 400    | `invalid_plugin`  | An upload is not a loadable plugin             |
| 401    | `unauthorized`    | No token, or one the store does not accept     |
| 403    | `forbidden`       | The action needs the owner role                |
| 404    | `not_found`       | No such plugin, version, member or invite      |
| 404    | `invite_unusable` | The invite was used, cancelled or has expired  |
| 404    | `link_unusable`   | A sign-in link was used or has expired         |
| 409    | `conflict`        | That version is already published, differently |
| 409    | `conflict`        | A store with an owner was sent a setup code    |
| 413    | `too_large`       | The body is over the server's limit            |
| 429    | `rate_limited`    | Too many failed attempts from this address     |

A used, cancelled and unknown invite all get the same answer, so a guess learns nothing.

### Endpoints

| Method   | Path                         | Who    | Purpose                                   |
| -------- | ---------------------------- | ------ | ----------------------------------------- |
| `GET`    | `/v1/store`                  | anyone | Name, protocol version and public key     |
| `POST`   | `/v1/redeem`                 | anyone | Trade an invite code for a token          |
| `POST`   | `/v1/preview`                | anyone | What an invite leads to, without spending |
| `GET`    | `/v1/me`                     | member | Who this token belongs to                 |
| `DELETE` | `/v1/me`                     | member | Revoke this device's token                |
| `GET`    | `/v1/index`                  | member | The signed plugin list                    |
| `GET`    | `/v1/plugins/{id}/{version}` | member | One plugin's `.wasm`                      |
| `POST`   | `/v1/invites`                | member | A device invite for yourself (owner: any) |
| `GET`    | `/v1/members`                | owner  | Members and pending invites               |
| `DELETE` | `/v1/members/{id}`           | owner  | Remove a member and all their devices     |
| `DELETE` | `/v1/invites/{id}`           | owner  | Cancel a pending invite                   |
| `POST`   | `/v1/uploads`                | owner  | Start sending a plugin in parts           |
| `PUT`    | `/v1/uploads/{id}/{part}`    | owner  | One part of it                            |
| `POST`   | `/v1/plugins`                | owner  | Publish what an upload carried            |
| `DELETE` | `/v1/plugins/{id}`           | owner  | Stop offering a plugin                    |

The reference server adds these for its admin page. Another server does not need them, and the app
says so when one is missing.

| Method   | Path                 | Who     | Purpose                                    |
| -------- | -------------------- | ------- | ------------------------------------------ |
| `POST`   | `/v1/admin/links`    | owner   | A one-time sign-in link, device token only |
| `POST`   | `/v1/admin/session`  | anyone  | Trade a sign-in code for a session cookie  |
| `DELETE` | `/v1/admin/session`  | session | Sign out                                   |
| `GET`    | `/v1/admin/overview` | owner   | Everything the page shows, in one answer   |
| `PATCH`  | `/v1/members/{id}`   | owner   | Rename someone, or change their role       |
| `DELETE` | `/v1/devices/{id}`   | owner   | Revoke one device, keeping its member      |
| `POST`   | `/v1/setup`          | anyone  | Claim a store nobody owns, with its code   |
| `GET`    | `/healthz`           | anyone  | `ok` once the database answers             |

`GET /v1/store`:

```json
{ "protocol": 1, "name": "Our plugins", "key": "Gv8s0qYH3rKj2m5WzT1nB6cXe4fLd9aP7uQiR0sMw8E" }
```

A client compares `key` with the one in the invite and stops if they differ. A store that nobody
owns yet and that was deployed with a setup code adds `"claimable": true`, which is how its admin
page knows to ask for the code.

`POST /v1/setup` takes `{ "code": "…" }` and answers with an owner invite,
`{ "link": "https://plugins.example/join/…#…", "expires_at": 1793000000 }`. A wrong code is `403`
and counts toward the same limit as a failed invite. A store deployed without a setup code answers
`404`, and one that already has an owner answers `409`.

`POST /v1/redeem` takes the code and a label for the device, and answers with the token:

```json
{ "invite": "k3TzQe9WbA1xLr7uVn2c0g", "device": "laptop" }
```

```json
{
  "token": "ast1_9q8C…",
  "member": { "id": "6f1c2a9e0b3d4e57", "name": "Alex", "role": "member" }
}
```

`GET /v1/me` answers with the same `member` object plus `"device": { "id": "…", "label": "laptop" }`.

`POST /v1/preview` takes `{ "invite": "…" }` and answers with the signed index the invite would give
access to, without spending it, so the confirmation can say who runs the store and what it offers.
The answer is the same envelope as `GET /v1/index`, and anistream checks it against the key in the
link.
- A code that would not join answers `404` with `invite_unusable`, and counts toward the same limit
  as a failed redeem.
- A store from before the preview answers `404` with `not_found`, and anistream joins it without
  one.

`POST /v1/invites` takes `{ "kind": "device" }` from anyone, for themselves, or
`{ "kind": "member", "name": "Alex", "role": "member" }` from an owner. `role` may be `owner`.
Both answer with the link:

```json
{ "id": "a41f0c2d9e7b3851", "link": "https://plugins.example/join/…#…", "expires_at": 1793000000 }
```

`GET /v1/members`:

```json
{
  "members": [
    { "id": "6f1c2a9e0b3d4e57", "name": "Alex", "role": "member",
      "joined_at": 1792300000, "last_seen": 1792390000, "devices": 2 }
  ],
  "invites": [
    { "id": "a41f0c2d9e7b3851", "name": "Sam", "role": "member", "kind": "member",
      "expires_at": 1793000000 }
  ]
}
```

An owner cannot remove themselves, so a store always keeps one. Removing a member revokes all of
their devices and cancels every unused invite they made or asked for, and an invite whose maker has
been removed is refused even if it was missed.

The reference server limits failed redemptions, sign-ins and setup codes per address. When a
request comes from loopback, which is what a proxy on the same machine looks like, or the server
was told to trust a proxy, it uses the last entry of `X-Forwarded-For` instead: that entry is the
one the proxy added, so a client cannot choose it. On Cloudflare the address is
`CF-Connecting-IP`, and on Vercel and Netlify it is the one the platform reports.

### Publishing

A plugin goes up in parts, so no request carries more than 4 MiB and a little JSON. Serverless
platforms cap a request at about 4.5 MB, and a plugin can be up to 32 MiB.

1. `POST /v1/uploads` with `{ "size": 9437184 }` answers `{ "id": "…", "part_size": 4194304,
   "parts": 3 }`.
2. `PUT /v1/uploads/{id}/{n}` for each `n` from `0`, with that part's bytes as the body. Every part
   is `part_size` bytes except the last, which is what is left.
3. `POST /v1/plugins` with `{ "upload": "…", "sha256": "…" }` puts the parts together, works out
   what the plugin is, and publishes it. It answers with the plugin's index entry.

An upload belongs to the owner who started it, and one left unfinished for an hour is thrown away.
Its parts are deleted when it finishes, whether or not the plugin was published.

`sha256` is the whole file's digest in lowercase hex, and may be left out. A store with CPU to spare
computes its own and ignores it. One that runs under a tight CPU budget, as a Worker on Cloudflare's
free plan does, takes it as given. Members check every download against the digest in the signed
index either way, so a wrong one leaves the plugin impossible to install, not unsafe.

Publishing a version that is already published succeeds when the bytes are the same, which is what a
retried upload looks like, and is `409` when they differ. A published version never changes.

To work out what a plugin is, the reference server loads it with no capabilities and asks it to
describe itself. A store that cannot run WebAssembly components, which is any store on Workers,
Vercel or Netlify, reads the plugin's manifest section instead, and refuses a plugin without one.

### The manifest section

A plugin can carry what it says about itself as a custom section named `anistream:manifest` at the
top level of the component, holding JSON:

```json
{ "id": "example", "name": "Example", "version": "1.4.0",
  "allowed_hosts": ["api.example"], "translations": ["sub", "dub"] }
```

`anistream plugin stamp plugin.wasm` writes it by loading the plugin and asking, and the app stamps
every plugin it publishes. A store that can run the plugin checks the section against the plugin's
own answer and refuses one that disagrees. Nothing trusts the section further than that: every
member's app loads a plugin before installing it, and refuses one whose answer differs from the
index.

### The signed index

`GET /v1/index` answers with an envelope:

```json
{ "index": "{\"protocol\":1,…}", "signature": "Q2x0…" }
```

`index` is the index document as a JSON string, so the bytes that were signed are exactly the bytes
the client reads. `signature` is an Ed25519 signature, unpadded base64url, over the UTF-8 bytes of
`anistream-store-index-v1\n` followed by the bytes of `index`. The prefix keeps a store's key from
being usable to sign anything else.

The document inside:

```json
{
  "protocol": 1,
  "store": "Our plugins",
  "key": "Gv8s0qYH3rKj2m5WzT1nB6cXe4fLd9aP7uQiR0sMw8E",
  "sequence": 7,
  "issued_at": 1792300000,
  "plugins": [
    {
      "id": "example",
      "name": "Example",
      "version": "1.4.0",
      "allowed_hosts": ["api.example"],
      "translations": ["sub", "dub"],
      "sha256": "9b1d…",
      "size": 104233,
      "published_at": 1792300000
    }
  ],
  "owners": ["Alex"]
}
```

`sequence` goes up by at least one every time the plugin list changes. A client remembers the highest it
has accepted, including after leaving a store, and refuses a lower one, so a server cannot quietly
hand back an older list to keep someone on an old version.

`owners` names the store's owners, the longest-standing first, for "Run by …". Like `store`, it
describes the store as it is now, so neither moves `sequence`. A store from before it leaves it
out.

A plugin's `.wasm` is at `/v1/plugins/{id}/{version}`. A published version never changes, so a
client can verify the download against `sha256` and `size` and then trust it.

A download without a `Range` header answers with the whole file. With `Range: bytes={start}-{end}`
it answers `206` with at most 4 MiB of it and a `Content-Range`, and `416` for a range outside the
file. anistream always downloads in ranges, so a plugin larger than a serverless platform's response
limit still arrives.

### What the client checks

Before joining, anistream checks that the store's `GET /v1/store` key is the one in the invite, and
that the preview's index is signed with it.
After redeeming, and before keeping the token, it fetches the index and checks its signature. Anyone
can repeat a store's public key; only the store can sign with it, so a server that does the first
and not the second is not joined. A link to a store already joined must name the address it was
joined at. A store is known by its key, and following a link that pairs a joined store's key with
another address would send the token being replaced to that address. A store that has moved is left
and joined again.

Links with a user name in the address (`https://plugins.example@elsewhere/`) are refused, and
responses are read up to a size limit, since the first request goes to whatever a pasted link
names.

Before installing, anistream verifies the index signature against the key from the invite, checks
the download's size and SHA-256 against the index, then loads the plugin with no network access
and compares what it declares about itself with the index: the same id, the same version and the
same allowed hosts. Any difference stops the install, as does a plugin named after one of anistream's own sources
(`torrent`, `remote`, `plugins`) or another file in the plugin directory that already declares the
same id. The file is written to the plugin directory as
`{id}.wasm`, so `[providers.plugins.settings.{id}]` applies to it as it would to one added by hand.
