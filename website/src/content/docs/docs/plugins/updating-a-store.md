---
title: Updating a store
description: Find out which version a store runs, and move it to a newer release on any host.
---

A store keeps its members, invites and plugins wherever it was deployed: a volume, a disk or a
database. Updating replaces the program and leaves all of that in place.

## Which version it runs

The admin page shows it under **This store**: the version, the database's schema version and the
host it runs on. A store on Cloudflare, Vercel or Netlify also shows a build, a short code for the
exact package it was built from.

Choose **Check for updates** to compare it with the newest release. The check runs in your browser
and asks GitHub directly, without cookies or a referrer. The store is not involved and nobody else
is told. Turn on **Check once a day** to have the page check by itself when you open it in that
browser. It is off until you turn it on.

When a newer release is out, the page says so at the top and shows the steps for your host.

## What happens to the data

A new version brings the store's database up to date by itself the first time it starts, or on
its first request on Cloudflare, Vercel and Netlify. Each change to the database is applied
whole or not at all, and the database records which changes it has had. This works the same on
SQLite, Postgres and Cloudflare D1.

A store refuses to start on a database that a newer version has already used, rather than guess at
data it does not understand. So going back to an older version means restoring a backup taken
before the update. Take one before a new major version.

## Docker, a VPS or a NAS

In the folder with `compose.yaml`:

```sh
docker compose pull
docker compose up -d
```

The compose file names the image `ghcr.io/bitknox/anistream-store:2`, which follows every 2.x
release. A new major version needs the tag changed by hand, after reading its release notes. To
stay on one release, name it in full, such as `:2.0.8`.

## Fly.io

Run `fly deploy` in the directory you launched from. It pulls the current image for the tag in
`fly.toml`.

## Render

Render does not redeploy on its own. Open the service, choose **Manual Deploy**, then
**Clear build cache & deploy**. Because of the disk, the store stops for a few seconds.

## Railway

Open the service's **Settings**, choose **Configure Auto Updates** under **Source**, and pick a
maintenance window. Railway then redeploys whenever the image's tag gets a new release. To update
straight away, choose **Redeploy**.

## Heroku

From a clone of the anistream repository:

```sh
git pull
heroku git:remote -a <app>
git push heroku main
```

Each build starts from the current published image.

## Cloudflare, Vercel and Netlify

The deploy buttons put a copy of the template in a repository of yours. It is a copy rather than
a fork, so GitHub's **Sync fork** button does not apply. The template carries a workflow,
`update-store.yml`, that does the same job:

1. Once, move `update-store.yml` into `.github/workflows/` and commit it.
2. To update, open the repository's **Actions** tab, choose **Update the store** and
   **Run workflow**.

The workflow copies the template from the newest release over your repository, keeps your
settings file (`wrangler.jsonc`, `vercel.json` or `netlify.toml`), commits and pushes. The host
deploys the push as it deploys any commit. If the release changed the settings file, the run shows
the difference as a warning, for you to bring over by hand.

The workflow runs only when you run it. To have it run every week, remove the `#` from its
`schedule` lines.

## `anistream store serve`

A store run by anistream itself is as new as anistream. Update anistream and start the store
again. The same goes for the `anistream-store` program: replace it with the new release's and
start it again with the same directory.
