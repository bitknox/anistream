---
title: Plugin stores
description: Share plugins with people you know, through a store one of you runs.
---

A plugin store is a small server that one person runs for the people they invite. Everyone who
joins can install the store's plugins from inside anistream and hears about new versions when
anistream starts. anistream ships no store and lists none, so every store you see is one someone
sent you.

## Joining a store

The store's owner sends you an invite link. In anistream, open **Providers** and press Enter to
open plugin stores, choose **join a store**, and paste the link. anistream checks the link against
the store before using it, shows the store's name and key, and asks you to confirm.

The store's page lists its plugins. Enter on one shows what it may contact before anything is
installed. Installed plugins run as part of the `"plugins"` entry in `providers.order`, which is
there by default; `K` on the Providers screen moves one ahead of the others.

An invite link works once and expires after seven days. To use the same store on another computer,
choose **link another device** on the store's page for a new link of your own.

## Updates

When a store has a newer version of a plugin you installed, anistream says so once when it starts.
Nothing is installed until you choose **update** on the store's page, and an update that wants to
contact a host the old version could not reach says which before you accept it.

## Leaving a store

**leave this store** on the store's page gives up your access. Plugins you installed from it stay
on your machine and stop updating. To remove one, choose it on the store's page and pick
**remove** before you leave.

## What a store protects

Only people with an invite can list or download a store's plugins. Everyone who joins does get the
plugins themselves, and can read them like any other file on their machine.

Members are protected by the sandbox and by the store's signature. A plugin can only reach the hosts
it declares, and a plugin list or download that was not signed by the key in your invite is refused,
as is a list older than one you have already seen. The key lives with the store, so the signature
guards against tampering on the way to you and against another server answering at the store's
address. It does not guard against someone who controls the store itself.

Removing someone stops their updates at once; plugins they already installed stay on their machine.

## Running a store

Anyone can run one, on their own computer or on a hosting platform, and most platforms' free or
smallest plans are enough for a group of friends. [Running a store](/docs/plugins/running-a-store/)
covers deploying one, claiming it, and managing it from anistream or its admin page.

The protocol is described in full in
[docs/plugin-stores.md](https://github.com/bitknox/anistream/blob/main/docs/plugin-stores.md), for
anyone who wants to write a different server.
