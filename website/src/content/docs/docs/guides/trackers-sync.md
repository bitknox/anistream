---
title: Trackers & sync
description: Local history needs no account. Sync is opt-in, durable, and honest about conflicts.
---

Local history needs no account — SQLite is the source of truth and sync is a projection of it.
Syncing to AniList, MyAnimeList, Simkl or Trakt is opt-in, and each service needs an app you
register — there is no public client to borrow, and shipping credentials in an open-source binary
would make them public.

```toml
[trackers]
enabled = ["anilist", "mal", "simkl", "trakt"]
token_storage = "keychain"    # or "file" — see below

[trackers.anilist]
client_id = "…"
client_secret = "…"           # required: AniList has no PKCE and no public client

[trackers.mal]
client_id = "…"               # that is all: MAL is a public client, PKCE covers it

[trackers.simkl]
client_id = "…"

[trackers.trakt]
client_id = "…"
client_secret = "…"           # Trakt wants one on the token exchange
```

| | AniList | MyAnimeList | Simkl | Trakt |
|---|---|---|---|---|
| Register at | [anilist.co](https://anilist.co/settings/developer) | [myanimelist.net](https://myanimelist.net/apiconfig) — App Type **other** | [simkl.com](https://simkl.com/settings/developer) | [trakt.tv](https://trakt.tv/oauth/applications) |
| Sign-in flow | browser redirect | browser redirect | device code | device code |
| Redirect URL | `http://127.0.0.1:45617/callback` | `http://127.0.0.1:45617/callback` | none needed | none needed |
| Needs a secret | **yes** | no | no | **yes** |
| Token life | ~1 year | ~31 days, refreshed automatically | long-lived | ~3 months, then --login again |

Then `anistream --login`, or `anistream --login --tracker mal`. Simkl and Trakt use the OAuth
device flow, so `--login --tracker simkl` prints a code and a URL to enter it at. Check any of them
with `anistream --sync`.

## How sync behaves

Updates go through a **durable outbox**: watch offline for a week and everything is pushed, in
order, when a connection returns. The merge logic is pure and heavily tested, and when local and
remote genuinely disagree the conflict gets its own screen instead of a silent overwrite.

An episode is pushed when it crosses `commit_threshold`, 85% of runtime by default. It is set high
so that opening an episode to check the subtitles does not mark it seen.

## Token storage

Use `keychain` for an installed build. Use `file`, which is `0600`, if you are running from
`cargo`: macOS keys keychain access on the *binary*, and every rebuild produces a new one, so
"Always Allow" never sticks and every run prompts you. `anistream --token-to-file` moves an
existing token across, and `ANISTREAM_TOKEN_STORAGE=file` overrides for a single run.
