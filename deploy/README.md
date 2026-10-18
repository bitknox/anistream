# Deploy a plugin store

A plugin store is a small private server for sharing anistream provider plugins with people you
invite. Each folder here sets one up somewhere different. Pick one, deploy it, [claim
it](#claim-your-store), then invite people from the store's page in anistream.

| Where | How | Keeps its data in | Cost |
| --- | --- | --- | --- |
| Cloudflare | Button in [`cloudflare/`](cloudflare/) | D1 and R2 | A group of friends fits in the free tier |
| Vercel | Button in [`vercel/`](vercel/) | Neon Postgres | A group of friends fits in the Hobby plan and Neon's free plan |
| Netlify | Button in [`netlify/`](netlify/) | Netlify Database (Postgres) | A group of friends fits in the Free plan |
| Fly.io | [`fly launch`](#flyio) | A 1 GB volume | Billed by usage; the machine stops when idle |
| Render | [Button](#render) | A 1 GB disk | Disks need a paid instance |
| Railway | A template, made once from [`railway/`](railway/) | A volume | Billed by usage |
| Heroku | [Button](#heroku) | Heroku Postgres | A Basic dyno and Postgres Essential 0, about $12 a month |
| Your own machine | [`docker/`](docker/) | A Docker volume | Your machine |

Cloudflare, Vercel and Netlify run the store as a function, with nothing to keep running. The rest
run the container image below.

## Claim your store

A new store has no owner. There are two ways to become it:

- With the setup code. Templates for hosted platforms set `ANISTREAM_STORE_SETUP_CODE`, generated
  during the deploy or chosen by you. Open `https://<your-store>/admin/`, enter the code, and the
  page answers with an owner invite link.
- From the log. On its first start the store prints a one-time owner invite, and prints a new one
  on each start until somebody uses it.

In anistream, go to Providers, press Enter, choose join a store and paste the link. The device that
joins with it owns the store.

## Fly.io

In an empty directory:

```sh
fly launch --from https://github.com/bitknox/anistream --copy-config
```

This clones the repository and launches with its `fly.toml`: one machine with 512 MB of memory and
a 1 GB volume, from the published image. Fly asks for an app name and a region. The owner invite is
in `fly logs`. To update, run `fly deploy` in the same directory.

Fly bills by usage and has no free tier. A stopped machine is billed only for storage, and the
machine stops when nobody is using the store.

## Render

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/bitknox/anistream)

The button reads `render.yaml`: one web service on Render's smallest paid instance, with a 1 GB
disk, which is why it is not free. Render generates the setup code; find it on the service's
Environment tab and claim the store at `https://<service>.onrender.com/admin/`.

Render does not redeploy on its own. To update, choose Manual Deploy, then Clear build cache &
deploy, which pulls the current image. Because of the disk, each deploy stops the store for a few
seconds.

## Heroku

[![Deploy to Heroku](https://www.herokucdn.com/deploy/button.svg)](https://www.heroku.com/deploy?template=https://github.com/bitknox/anistream)

Heroku has no disk, so `app.json` adds a Heroku Postgres database and the store keeps everything in
it. After the deploy, Heroku opens the admin page; the setup code is under Settings, Config Vars.

The store builds invite links from the address each request arrives at, which is the app's
`herokuapp.com` address. Running `heroku labs:enable runtime-dyno-metadata -a <app>` and deploying
again tells it that address up front, so the owner invite also appears in `heroku logs`.

To update, push this repository to the app from a clone: `heroku git:remote -a <app>`, then
`git push heroku main`. Each build starts from the current published image.

## Railway

See [`railway/`](railway/README.md).

## The container image

`ghcr.io/bitknox/anistream-store`, for linux/amd64 and linux/arm64, built from
[`container/Dockerfile`](container/Dockerfile) by `.github/workflows/store-image.yml`. Its tags
are the release's version (`1.0.0`), its minor and major versions (`1.0`, `1`) and `latest`. The
templates use `1`, which follows every 1.x release.

It runs as a non-root user and keeps its files in `/data`. Settings come from the environment:

| Variable | Meaning |
| --- | --- |
| `ANISTREAM_STORE_URL` | The store's HTTPS address. Found by itself on Fly, Render and Railway, and on Heroku with dyno metadata. |
| `ANISTREAM_STORE_NAME` | The name members see. |
| `ANISTREAM_STORE_SETUP_CODE` | Claims the store from its admin page while nobody owns it. |
| `ANISTREAM_STORE_TRUST_PROXY` | `true` behind a proxy that is the only way in. Automatic on the platforms above. |
| `DATABASE_URL` | Keep everything in this Postgres database instead of `/data`. |
| `PORT` | The port to listen on, on every interface. `7878` unless the platform sets it. |
| `ANISTREAM_STORE_LISTEN` | An exact address to listen on instead, such as `127.0.0.1:7878`. |

To build it yourself, from the repository root:

```sh
docker build -f deploy/container/Dockerfile -t anistream-store .
```

### For maintainers

The image is published when a `v*` tag is pushed, so the templates above only work once a release
has been tagged. GHCR makes a new package private: after the first publish, open the package's
settings on GitHub (Packages, anistream-store, Package settings) and change its visibility to
public once, or the platforms cannot pull it.

The serverless templates carry the store as committed WebAssembly. After changing the store's
crates, a dependency they compile or the toolchain, run `deploy/sync.sh` and commit what it
writes; CI's `deploy/sync.sh --check` fails until then. A release changes the crates' version, so
it needs the same.
