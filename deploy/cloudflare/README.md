# anistream plugin store on Cloudflare

A private plugin store for you and the people you invite, running on Cloudflare Workers. Members
join with an invite link from you and install the plugins you publish, all from inside anistream.

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/bitknox/anistream/tree/main/deploy/cloudflare)

## What it creates

The button copies this folder into a new repository on your GitHub or GitLab account and deploys
it to your Cloudflare account. It sets up three things:

- a Worker called `anistream-plugin-store`, which answers at `https://anistream-plugin-store.<your-subdomain>.workers.dev`
- a D1 database, which keeps members, invites and the plugin list
- an R2 bucket, which keeps the plugin files

A store for a group of friends stays inside the free tier of all three. Cloudflare may ask for a
payment method before it turns on R2 for your account, even though the store's usage is free.

## Choose a setup code

During the deploy you are asked for `ANISTREAM_STORE_SETUP_CODE`. This is a code you make up, and
it is how you prove the store is yours the first time you open it. Use something long and random,
for example the output of `openssl rand -hex 16`, and keep it to yourself. It stops working once
the store has an owner.

You can also change the store's name, `ANISTREAM_STORE_NAME`, which is what members see in
anistream.

## Claim the store

1. Open `https://<your-worker>/admin/` in a browser.
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
bunx wrangler login
bunx wrangler deploy
bunx wrangler secret put ANISTREAM_STORE_SETUP_CODE
```

The first deploy creates the D1 database and the R2 bucket. Until the secret is set, nobody can
claim the store, so the order is safe.

To try it on your own machine first, copy `.dev.vars.example` to `.dev.vars`, fill in a code and
run `bun run dev`. The store then answers at `http://localhost:8787` with storage that stays on
your machine.

## Update

Your members, invites and plugins live in D1 and R2, so updating the Worker keeps all of them.

If you used the button, replace the files in your copy of the repository with the ones from this
folder in the latest anistream release, and commit. Cloudflare deploys every commit to your
repository's main branch.

If you deployed by hand, fetch the latest anistream, then run `bun install` and `bunx wrangler
deploy` in this folder again.

## Your own domain

A custom domain added to the Worker works without any change: invite links use whichever address
the store was reached at. To make every link use one address, add it to `vars` in
`wrangler.jsonc`, for example `"ANISTREAM_STORE_URL": "https://plugins.example.com"`, and deploy.
