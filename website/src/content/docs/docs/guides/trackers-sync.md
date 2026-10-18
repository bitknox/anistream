---
title: Trackers & sync
description: Local history needs no account. Sync is opt-in, durable, and honest about conflicts.
---

Local history needs no account. SQLite is the source of truth and sync is a projection of it.
Syncing to AniList, MyAnimeList, Kitsu, Simkl or Trakt is opt-in, and every service except Kitsu
needs an app you register. There is no public client to borrow, and credentials shipped in an
open-source binary would be public.

Kitsu takes your account credentials directly, so there is nothing to register. anistream exchanges
your username and password for a token once and drops them, and the password is never stored.
Kitsu is enabled by default and waits on the Accounts screen, one sign-in away. Enabled only means
visible: nothing is queued or pulled for any tracker until it is signed in.

```toml
[trackers]
enabled = ["anilist", "mal", "kitsu", "simkl", "trakt"]   # default: ["kitsu"]
token_storage = "keychain"    # or "file", see below

[trackers.anilist]
client_id = "…"
client_secret = "…"           # required: AniList has no PKCE and no public client

[trackers.mal]
client_id = "…"               # that is all: MAL is a public client, PKCE covers it

# kitsu needs no section at all

[trackers.simkl]
client_id = "…"

[trackers.trakt]
client_id = "…"
client_secret = "…"           # Trakt wants one on the token exchange
```

| | AniList | MyAnimeList | Kitsu | Simkl | Trakt |
|---|---|---|---|---|---|
| Register at | [anilist.co](https://anilist.co/settings/developer) | [myanimelist.net](https://myanimelist.net/apiconfig), App Type **other** | nothing to register | [simkl.com](https://simkl.com/settings/developer) | [trakt.tv](https://trakt.tv/oauth/applications) |
| Sign-in flow | browser redirect | browser redirect | username and password, once | device code | device code |
| Redirect URL | `http://127.0.0.1:45617/callback` | `http://127.0.0.1:45617/callback` | none needed | none needed | none needed |
| Needs a secret | **yes** | no | no | no | **yes** |
| Token life | ~1 year | ~31 days, refreshed automatically | ~30 days, refreshed automatically | long-lived | ~3 months, then --login again |

Then `anistream --login`, or `anistream --login --tracker mal`. Simkl and Trakt use the OAuth
device flow, so `--login --tracker simkl` prints a code and a URL to enter it at. For Kitsu, press
Enter on its row on the Accounts screen, which asks for your username and a masked password, or
run `anistream --login --tracker kitsu` on a machine without the interface. Check any of them with
`anistream --sync`.

## How sync behaves

Updates go through a **durable outbox**: watch offline for a week and everything is pushed, in
order, when a connection returns. The merge logic is pure and heavily tested, and when local and
remote genuinely disagree the conflict gets its own screen instead of a silent overwrite.

An episode is pushed when it crosses `commit_threshold`, 85% of runtime by default. It is set high
so that opening an episode to check the subtitles does not mark it seen.

`a` sets a title's list status and `*` rates it out of ten. Both are queued in the same outbox for
every tracker you are signed in to, and each tracker converts the rating to its own scale.

## Token storage

Use `keychain` for an installed build. Use `file`, which is `0600`, if you are running from
`cargo`: macOS keys keychain access on the *binary*, and every rebuild produces a new one, so
"Always Allow" never sticks and every run prompts you. `anistream --token-to-file` moves an
existing token across, and `ANISTREAM_TOKEN_STORAGE=file` overrides for a single run.

The tokens for [plugin stores](/docs/plugins/stores/) you have joined are kept the same way, by
the same `token_storage` setting and `ANISTREAM_TOKEN_STORAGE` override, so the keychain notes
above apply to them too.
