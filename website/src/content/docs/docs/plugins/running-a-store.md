---
title: Running a store
description: Deploy a plugin store, claim it, and look after it.
---

A store is one small program, and it is the same program wherever it runs. Pick a home for it by
what you already have: an account on a hosting platform, a server, or a computer that stays on.

## Pick a host

| Where | How | What it costs |
| --- | --- | --- |
| Cloudflare Workers | [Deploy button](https://github.com/bitknox/anistream/tree/main/deploy/cloudflare) | Fits the free plan. R2 may ask for a payment method on file |
| Vercel | [Deploy button](https://github.com/bitknox/anistream/tree/main/deploy/vercel) | Fits the Hobby plan and Neon's free plan |
| Netlify | [Deploy button](https://github.com/bitknox/anistream/tree/main/deploy/netlify) | Fits the Free plan |
| Render | [Deploy button](https://github.com/bitknox/anistream/tree/main/deploy#render) | The smallest paid instance, because the store needs a disk |
| Heroku | [Deploy button](https://github.com/bitknox/anistream/tree/main/deploy#heroku) | A Basic dyno and Postgres Essential 0 |
| Fly.io | [`fly launch`](https://github.com/bitknox/anistream/tree/main/deploy#flyio) | Billed by use. The machine stops while nobody uses the store |
| Railway | [A template](https://github.com/bitknox/anistream/tree/main/deploy/railway) | Billed by use |
| A server or a computer of yours | [`anistream store serve`](#on-a-machine-of-your-own), or [Docker](https://github.com/bitknox/anistream/tree/main/deploy/docker) | Nothing more |

Each link has the details for that host: what the deploy creates, where it keeps the store's
records and plugin files, and how to update it.

## Claim it

A new store has no owner. The templates for hosting platforms ask for, or generate, a setup code.
Open `https://<your store>/admin/` in a browser, enter the code, and the page answers with an owner
invite link. On a platform that generates the code, it is listed with the service's environment
variables.

Paste the link into anistream the way anyone joins a store: **Providers**, Enter, **join a store**.
The device that joins with it owns the store. The setup code stops working once the store has an
owner.

A store also prints a one-time owner invite in its log when it starts without an owner, which is
the easier way on your own machine or wherever the log is in front of you.

## On a machine of your own

```sh
anistream store serve ./my-store --url https://plugins.example --name "Our plugins"
```

The first run creates `./my-store` and prints the owner invite. `--url` is the address people will
reach the store at, and invite links are built from it. The server listens on `127.0.0.1:7878`
unless `--listen` says otherwise.

anistream joins stores over HTTPS only, apart from stores on the same machine. Either pass
`--tls-cert` and `--tls-key`, or put the server behind something that serves HTTPS already, such as
Caddy, a Cloudflare Tunnel or Tailscale. The
[Docker templates](https://github.com/bitknox/anistream/tree/main/deploy/docker) come with Caddy
for a machine with a domain, and with Tailscale Funnel for one without.

The server also comes as a small program of its own, `anistream-store`, in the container image
`ghcr.io/bitknox/anistream-store`. It takes the same commands as `anistream store`.

## Looking after it

The store's page in anistream has what an owner needs day to day: **invite someone**,
**members**, where you can remove someone's access, and **publish a plugin** from your plugin
directory. Choose a plugin you published to stop offering it.

Choose **open the admin page** to open the store in your browser, already signed in. It has the
same controls and a few more: rename people, make someone an owner, revoke one lost device without removing
its owner, and publish by dropping `.wasm` files onto the page. A sign-in link works once, for ten
minutes, and a session lasts twelve hours.

On the store's own machine, a few things work from the shell:

```sh
anistream store invite ./my-store --name Alex            # print an invite link
anistream store publish ./my-store --plugin plugin.wasm  # publish or update a plugin
anistream store admin ./my-store                         # print a sign-in link for the admin page
```

In a container, the same commands run as `anistream-store`, for example
`docker compose exec store anistream-store invite --name Alex`.

## Publishing to Cloudflare, Vercel or Netlify

A store there cannot run a plugin to ask what it is, so the plugin has to carry the answer. Publish
from anistream and it is added for you. To publish a file through the admin page instead, add it
first:

```sh
anistream plugin stamp plugin.wasm
```

Members' copies of anistream check every plugin against what it says when they install it, so a
plugin cannot get in under someone else's name this way.

## Settings

The templates set these for you. Every option of `anistream store serve` can also come from the
environment:

| Variable | Meaning |
| --- | --- |
| `ANISTREAM_STORE_URL` | The address people reach the store at. Found on its own on Render, Railway, Fly and Heroku |
| `ANISTREAM_STORE_NAME` | The store's name, as members see it |
| `ANISTREAM_STORE_SETUP_CODE` | Claims the store from its admin page while nobody owns it |
| `ANISTREAM_STORE_DIR` | The directory holding the store's records and plugin files |
| `DATABASE_URL` | A Postgres database to keep everything in instead |
| `ANISTREAM_STORE_TRUST_PROXY` | `true` behind a proxy that is the only way in |

The full list is in the
[protocol document](https://github.com/bitknox/anistream/blob/main/docs/plugin-stores.md#settings).

## Keeping it safe

Everything a store has is in one place: its directory, its volume or its database. That includes
the key that signs its plugin list, so keep backups as private as the store itself. Someone with
that key, or with control of the server, could sign a list your members would accept.
