# anistream plugin store on Netlify

A private plugin store for you and the people you invite, running as a Netlify function. Members
join with an invite link from you and install the plugins you publish, all from inside anistream.

[![Deploy to Netlify](https://www.netlify.com/img/deploy/button.svg)](https://app.netlify.com/start/deploy?repository=https://github.com/bitknox/anistream&create_from_path=deploy/netlify)

## What it creates

The button copies this folder into a new repository on your Git account and deploys it to your
Netlify account. It sets up two things:

- a project whose one function answers every address of `https://<your-project>.netlify.app`
- a Netlify Database, which is Postgres and keeps everything: members, invites, the plugin list
  and the plugin files themselves

The database is created by the first deploy, because this folder's `package.json` lists
`@netlify/database`, and the function finds it through `NETLIFY_DB_URL`. There is nothing to copy
by hand. Netlify Database is part of Netlify's credit-based plans, Free included, and a store for a
group of friends fits in the Free plan's allowance. Plugin files count toward the database's
storage.

To use a database of your own instead, or if your plan does not include Netlify Database, put a
Postgres connection string in `DATABASE_URL` under Project configuration, Environment variables,
and deploy again. Any Postgres works, for example a free Neon project. `DATABASE_URL` wins when
both are there.

## Choose a setup code

During the deploy you are asked for `ANISTREAM_STORE_SETUP_CODE`. This is a code you make up, and
it is how you prove the store is yours the first time you open it. Use something long and random,
for example the output of `openssl rand -hex 16`, and keep it to yourself. It stops working once
the store has an owner.

You can also give the store a name with `ANISTREAM_STORE_NAME`, which is what members see in
anistream. Add it under Project configuration, Environment variables, and deploy again.

## Claim the store

1. Open `https://<your-project>.netlify.app/admin/` in a browser.
2. Enter the setup code. The page answers with an owner invite link.
3. In anistream, go to Providers, press Enter to open plugin stores, choose join a store and paste
   the link.

The device that joins with that link becomes the owner. From then on you invite people, publish
plugins and remove members from the store's page in anistream, or from the admin page, which you
open with "open the admin page" on the store in anistream.

## Deploy by hand

With [Bun](https://bun.sh) installed, from this folder:

```sh
bun install
bunx netlify login
bunx netlify sites:create
bunx netlify env:set ANISTREAM_STORE_SETUP_CODE "<your setup code>"
bunx netlify deploy --prod
```

`netlify sites:create` makes the project and links this folder to it. `netlify deploy` builds on
your machine, which also creates the Netlify Database, and uploads the result.

## Update

Your members, invites and plugins live in the database, so updating the function keeps all of
them.

If you used the button, replace the files in your copy of the repository with the ones from this
folder in the latest anistream release, and commit. Netlify deploys every commit to your
repository's main branch.

If you deployed by hand, fetch the latest anistream, then run `bun install` and
`bunx netlify deploy --prod` in this folder again.

## Your own domain

A domain added to the project works without any change: invite links use whichever address the
store was reached at. To make every link use one address, set `ANISTREAM_STORE_URL`, for example
to `https://plugins.example.com`, under Project configuration, Environment variables, and deploy
again.
