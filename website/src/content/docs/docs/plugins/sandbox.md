---
title: The plugin sandbox
description: Plugins get no sockets and reach only the hosts they declare, through the host’s fetch and within fixed limits.
---

A plugin has no sockets. It asks the host to make each request, through `fetch`.

The host makes every request with anistream's own HTTP client, so a plugin carries no networking
code of its own: it says what to request, and the host makes the request. Requests go only to the
hosts the plugin declares, because the host checks the allowlist before each one. A plugin can
still send what it is given, such as your search terms and its own settings, to those hosts, so
`anistream --plugins` and a store's page show them before you rely on a plugin.
Regex, AES-CBC (128/192/256 by key length) and per-plugin settings are lent for the same reason.

## The limits

All enforced host-side. `memory_mb` and `deadline_secs` are configurable under
`[providers.plugins]`; the fetch budget and body ceiling are fixed. Raising a ceiling cannot grant
a new capability.

| Limit | Default |
|---|---|
| Reachable hosts | manifest `allowed-hosts` |
| Memory | 64 MiB |
| Deadline | 20 s per call |
| Fetches | 12 per call |
| Response body | 8 MiB |
| Methods | GET, POST |

## The allowlist

- Only `http` and `https` are allowed. `file:` and `data:` are not transport.
- A declared host covers itself and its subdomains. `cdn.site.example` is reachable from
  `site.example`, and `site.example.evil.example` is not.
- No credentials in the URL. `https://site.example@evil.example/` points at evil.example.
- No loopback or private addresses, even if declared. That includes anistream's own torrent
  stream server.
- Redirects are not followed. The redirect comes back as a response, and the next hop is
  allowlisted again.

Reserved headers (`host`, `content-length`, `connection`, `transfer-encoding`, `accept-encoding`)
are refused; `referer`, `cookie` and `user-agent` are yours to set.

`config-get` is scoped the same way: a plugin sees only its own
`[providers.plugins.settings.<id>]` table, one key at a time, and the `describe` that reads a
plugin's manifest at load time runs with no capabilities at all: no settings, no allowlist, no
client.

WASI is linked, but the context is empty. A `wasm32-wasip2` component in any language
imports a WASI floor for its standard library, so the whole p2 surface is linked and containment
comes from an empty context instead: no preopened directories, so nothing on disk is openable;
every socket address is refused; no environment, no stdio. A guest can read the wall clock and take
random bytes. Neither reaches outside the sandbox.

## Plugins from a store

A plugin installed from a [plugin store](/docs/plugins/stores/) shows the hosts it may reach before
you install it, and again when an update adds new ones. Its bytes are checked against the store's
signed plugin list before anything is written to the plugin directory, and anistream loads it with
no network access to confirm that its id, version and hosts match that list.

## Testing yours

```sh
cargo test -p anistream-plugin       # skips itself if components are not built
```

The conformance suite runs the same assertions against both reference plugins, plus
`plugins/test-hostile`, which spins forever, allocates without bound, fetches an undeclared host
and probes undeclared settings. Copy the `assert_reference_behaviour` shape for your own plugin.
