# anistream plugin store on Vercel

A private plugin store for you and the people you invite, running as a Vercel function. Members
join with an invite link from you and install the plugins you publish, all from inside anistream.

[![Deploy with Vercel](https://vercel.com/button)](https://vercel.com/new/clone?repository-url=https%3A%2F%2Fgithub.com%2Fbitknox%2Fanistream%2Ftree%2Fmain%2Fdeploy%2Fvercel&project-name=anistream-plugin-store&repository-name=anistream-plugin-store&env=ANISTREAM_STORE_SETUP_CODE&envDescription=A%20long%20code%20you%20make%20up.%20You%20enter%20it%20once%20on%20the%20store%27s%20admin%20page%20to%20become%20its%20owner.&envLink=https%3A%2F%2Fgithub.com%2Fbitknox%2Fanistream%2Ftree%2Fmain%2Fdeploy%2Fvercel%23choose-a-setup-code&stores=%5B%7B%22type%22%3A%22integration%22%2C%22integrationSlug%22%3A%22neon%22%2C%22productSlug%22%3A%22neon%22%2C%22protocol%22%3A%22storage%22%7D%5D)

## What it creates

The button copies this folder into a new repository on your GitHub, GitLab or Bitbucket account
and deploys it to your Vercel account. It sets up two things:

- a project called `anistream-plugin-store`, whose one function answers every address of
  `https://anistream-plugin-store.vercel.app` (or a similar name, if that one is taken)
- a Neon Postgres database, added through the Vercel Marketplace, which keeps everything: members,
  invites, the plugin list and the plugin files themselves

Vercel connects the database to the project and gives it `DATABASE_URL`, so there is nothing to
copy by hand. A store for a group of friends fits in Vercel's Hobby plan and Neon's free plan.
Plugin files count toward the database's storage.

If you skipped the database during the deploy, open the project, go to Storage, create a Neon
database and connect it to the project. Then open Deployments and redeploy the latest one, so the
function sees the new `DATABASE_URL`.

## Choose a setup code

During the deploy you are asked for `ANISTREAM_STORE_SETUP_CODE`. This is a code you make up, and
it is how you prove the store is yours the first time you open it. Use something long and random,
for example the output of `openssl rand -hex 16`, and keep it to yourself. It stops working once
the store has an owner.

You can also give the store a name with `ANISTREAM_STORE_NAME`, which is what members see in
anistream. Add it under Settings, Environment Variables, and redeploy.

## Claim the store

1. Open `https://<your-project>.vercel.app/admin/` in a browser. Use the address listed under the
   project's Domains. The longer address of each single deployment is kept behind a Vercel login.
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
bunx vercel login
bunx vercel link
bunx vercel integration add neon
bunx vercel env add ANISTREAM_STORE_SETUP_CODE production
bunx vercel deploy --prod
```

`vercel link` creates the project, and `vercel integration add neon` creates the database and
connects it, which sets `DATABASE_URL`. Any other Postgres works too: put its connection string in
`DATABASE_URL` instead.

## Update

Your members, invites and plugins live in the database, so updating the function keeps all of
them.

If you used the button, replace the files in your copy of the repository with the ones from this
folder in the latest anistream release, and commit. Vercel deploys every commit to your
repository's main branch.

If you deployed by hand, fetch the latest anistream, then run `bun install` and
`bunx vercel deploy --prod` in this folder again.

## Your own domain

A domain added to the project works without any change: invite links use whichever address the
store was reached at. To make every link use one address, set `ANISTREAM_STORE_URL`, for example
to `https://plugins.example.com`, under Settings, Environment Variables, and redeploy.
