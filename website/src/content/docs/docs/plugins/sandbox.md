---
title: Sandbox & guarantees
description: No sockets, host-lent fetch, and limits a compromised guest cannot ignore.
---

**The sandbox has no sockets. The host lends you `fetch`.**

The host owns the browser-shaped TLS/HTTP2 handshake, the rate limiter and the cookie jar, so a
plugin is a parser: it says what to request, the host makes the request. Plugins stay kilobytes,
inherit the fingerprint, and cannot exfiltrate anything — the declared hosts are enforced
host-side. Regex, AES-CBC (128/192/256 by key length) and per-plugin settings are lent for the
same reason.

## The limits

All enforced host-side. Ceilings are configurable under `[providers.plugins]`; raising one cannot
grant a new capability.

| Limit | Default |
|---|---|
| Reachable hosts | manifest `allowed-hosts` |
| Memory | 64 MiB |
| Deadline | 20 s per call |
| Fetches | 12 per call |
| Response body | 8 MiB |
| Methods | GET, POST |

## The allowlist

- **http/https only** — `file:` and `data:` are not transport
- **Exact host or a subdomain** — `cdn.example.com` is reachable from `example.com`;
  `example.com.evil.test` is not
- **No credentials in the URL** — `https://example.com@evil.test/` points at evil.test
- **No loopback or private addresses**, even if declared — including anistream's own torrent
  stream server
- **Redirects are not followed** — the redirect comes back as a response; the next hop is
  allowlisted again

Reserved headers (`host`, `content-length`, `connection`, `transfer-encoding`, `accept-encoding`)
are refused; `referer`, `cookie` and `user-agent` are yours to set.

`config-get` is scoped the same way: a plugin sees only its own
`[providers.plugins.settings.<id>]` table, one key at a time, and `describe` runs with no settings
at all.

**WASI is linked, but grants nothing**: no `wasi:filesystem`, no `wasi:sockets`, no
`wasi:random`, no wall clock — a component importing them fails to instantiate.

## Testing yours

```sh
cargo test -p anistream-plugin       # skips itself if components are not built
```

The conformance suite runs the same assertions against both reference plugins, plus
`plugins/test-hostile`, which spins forever, allocates without bound, fetches an undeclared host
and probes undeclared settings. Copy the `assert_reference_behaviour` shape for your own plugin.
