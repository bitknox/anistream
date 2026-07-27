---
title: Sandbox & guarantees
description: No sockets, host-lent fetch, and limits a compromised guest cannot ignore.
---

## Start here

**The sandbox has no sockets. The host lends you `fetch`.**

That is the design rather than a restriction to work around. Some hosts vary their responses by
client, so matching a browser's TLS and HTTP/2 handshake is what makes them parseable, and
anistream does that once in one place. If plugins opened their own connections, each would carry a
TLS stack and solve it again.

So your plugin is a **parser**. It says what to request, the host makes the request, and you read
bytes. That buys you three things:

- Plugins are kilobytes, not megabytes. The Rust reference is 71 KB with no HTTP client in it.
- You inherit the handshake, the rate limiter and the cookie jar.
- You cannot exfiltrate anything, because the host enforces your declared hosts on its own side.

Regex and AES-128-CBC are lent for the same reason: a TinyGo or JavaScript guest bundling its own
would dwarf the parsing logic it exists for, and real sources need both.

## The limits

Every limit is enforced **host-side**, because a limit a guest could check for itself is a limit a
compromised guest ignores.

| Limit | Default | Why |
|---|---|---|
| Declared hosts | manifest | A parser has no business reaching anything else |
| Memory | 64 MiB | An unbounded allocation would take the process down |
| Deadline | 20 s per call | A guest that loops forever must not wedge the UI |
| Fetches | 12 per call | The host's client is not a request amplifier |
| Response body | 8 MiB | — |
| Methods | GET, POST | Anything else changes state on a remote site |

Ceilings are configurable under `[providers.plugins]`; raising one cannot grant a capability, only
more of one already held.

## The host allowlist

Stricter than it first looks, and the tests spell out each case:

- **http/https only** — `file:` and `data:` are not transport
- **Exact host or a subdomain** — `cdn.example.com` is reachable from `example.com`;
  `example.com.evil.test` is not, which is the trick a naive `ends_with` falls for
- **No credentials in the URL** — `https://example.com@evil.test/` points at evil.test
- **No loopback or private addresses**, even if declared — a plugin must not reach a service on
  the user's own machine, including anistream's own torrent stream server
- **Redirects are not followed** — an allowed host must not be able to `302` you somewhere else;
  the redirect comes back as a response and the next hop is allowlisted again

Reserved headers (`host`, `content-length`, `connection`, `transfer-encoding`, `accept-encoding`)
are refused for correctness rather than suspicion: `accept-encoding` has to agree with the
handshake the host negotiated, and the rest are the client's to compute. `referer`, `cookie` and
`user-agent` are yours to set.

**WASI is linked, but grants nothing.** A `wasm32-wasip2` component in any language imports a WASI
floor for its standard library. What matters is the absence: no `wasi:filesystem`, no
`wasi:sockets`, no `wasi:random`, no wall clock — those are not in the linker at all, so a
component importing them fails to instantiate. The context handed over has no preopens, no
environment and no network.

## Testing yours

```sh
cargo test -p anistream-plugin       # skips itself if components are not built
```

`crates/anistream-plugin/tests/conformance.rs` runs against three real components: the two
references and `plugins/test-hostile`, which spins forever, allocates without bound, and tries to
fetch a host it never declared. The deadline is driven from an OS thread rather than a tokio task,
because a guest sitting in `loop {}` occupies the executor thread and the timer would never fire.

For the same confidence in your own plugin, `assert_reference_behaviour` is the shape to copy: one
function of assertions, run against every implementation.
