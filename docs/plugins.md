# Writing a provider plugin

A provider plugin is a WebAssembly component that turns search terms into episodes and episodes into
stream URLs. anistream loads it from `~/.config/anistream/plugins/*.wasm` and treats it exactly like
a native source: same ranking, same health tracking, same failover.

You can write one in any language that targets WASI 0.2 — Rust, Go (TinyGo), JavaScript (jco),
Python (componentize-py). Two reference implementations live in `plugins/`, in **Rust** and
**JavaScript**, and the conformance suite runs the *same* assertions against both.

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

## The interface

`wit/anistream-provider.wit` is the contract. Four functions in, four out:

```wit
interface host {                          // what you may call
    fetch:          func(req: http-request) -> result<http-response, host-error>;
    log:            func(level: string, msg: string);
    aes-decrypt:    func(key: list<u8>, iv: list<u8>, data: list<u8>) -> result<list<u8>, string>;
    regex-captures: func(pattern: string, haystack: string) -> list<list<string>>;
}

interface provider {                      // what you must export
    describe:      func() -> manifest;
    search:        func(query: string, translation: string) -> result<list<search-hit>, provider-error>;
    list-episodes: func(id: string, translation: string) -> result<list<episode>, provider-error>;
    resolve:       func(id: string, episode: string, translation: string)
                     -> result<list<media-stream>, provider-error>;
}
```

`provider-error` mirrors anistream's internal error type one-for-one, which is what lets the registry
apply the same failover rules to a plugin as to a native source. The distinction that matters:

| Return | Meaning | Failover? |
|---|---|---|
| `not-found` | Works, no such title or episode | **No** — this is an answer |
| `blocked(msg)` | Bot protection, rate limit, geo-block, ban | Yes |
| `parse(msg)` | Response arrived, did not look like expected | Yes |
| `other(msg)` | Anything else | Yes |

Getting `not-found` wrong is the common mistake: flattening it into `other` makes every missing
episode walk the entire provider chain.

## Rust

```sh
cargo build --release --target wasm32-wasip2 \
  --manifest-path plugins/example-rust/Cargo.toml
cp plugins/example-rust/target/wasm32-wasip2/release/anistream_example_plugin.wasm \
  ~/.config/anistream/plugins/
```

Start from `plugins/example-rust/src/lib.rs` — it exercises every part of the ABI against a stable
public endpoint, so it keeps working when someone else's markup
changes.

```rust
wit_bindgen::generate!({ path: "../../wit/anistream-provider.wit", world: "plugin" });

impl Guest for Component {
    fn describe() -> Manifest {
        Manifest {
            id: "my-source".into(),
            display_name: "My Source".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            allowed_hosts: vec!["example.com".into()],   // enforced host-side
            translation_types: vec!["sub".into()],
        }
    }
    // search, list_episodes, resolve …
}
```

## JavaScript

```sh
cd plugins/example-ts && npm install && npm run build
cp anistream-example-plugin-ts.wasm ~/.config/anistream/plugins/
```

Four things differ from Rust, and each is easy to get wrong:

- **Imports carry the package version**: `anistream:provider/host@0.1.0`, not
  `anistream:provider/host`. Getting it wrong fails at build time with a module-resolution error.
- **Names are lowerCamelCase**: `list-episodes` → `listEpisodes`, `duration-secs` → `durationSecs`.
- **Errors are thrown, not returned.** jco maps a WIT `result` to a return value plus a thrown
  error, so `throw { tag: 'not-found' }` and `throw { tag: 'blocked', val: 'reason' }`. Host errors
  arrive the same way, so calling `fetch` needs a `try`/`catch`.
- **`--disable all`** is what strips WASI. Without it the component imports a floor it never uses.

## What it costs

Measured with `cargo run -p anistream-plugin --example plugin_bench --release`:

| | size | compile (once) | per call |
|---|---|---|---|
| Rust | 0.1 MB | 19 ms | 38 µs |
| JavaScript | 12.0 MB | 1.06 s | 923 µs |

The JS component embeds a whole JS engine, so it is 170× larger and 24× slower per call. Both are
negligible beside a single HTTP request, and **both run under the same default limits** — a bigger
memory ceiling looked obviously necessary and measurably is not. Pick Rust if the download size of
what you ship matters; pick JavaScript if you would rather write JavaScript.

Note the per-call figure includes instantiating a fresh store. That is deliberate: it is what stops
one call leaking state into the next, so a plugin cannot stash a cookie from your search and use it
during someone else's.

## The sandbox

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

The host allowlist is stricter than it first looks, and the tests spell out why:

- **http/https only** — `file:` and `data:` are not transport
- **Exact host or a subdomain** — `cdn.example.com` is reachable from `example.com`;
  `example.com.evil.test` is not, which is the trick a naive `ends_with` falls for
- **No credentials in the URL** — `https://example.com@evil.test/` points at evil.test
- **No loopback or private addresses**, even if declared — a plugin must not reach a service on the
  user's own machine, including anistream's own torrent stream server
- **Redirects are not followed** — an allowed host must not be able to `302` you somewhere else;
  the redirect comes back as a response and the next hop is allowlisted again

Reserved headers (`host`, `content-length`, `connection`, `transfer-encoding`, `accept-encoding`)
are refused for correctness rather than suspicion: `accept-encoding` has to agree with the
handshake the host negotiated, and the rest are the client's to compute. `referer`, `cookie` and
`user-agent` are yours to set.

**WASI is linked, but grants nothing.** A `wasm32-wasip2` component in any language imports a WASI
floor for its standard library. What matters is the absence: no `wasi:filesystem`, no `wasi:sockets`,
no `wasi:random`, no wall clock — those are not in the linker at all, so a component importing them
fails to instantiate. The context handed over has no preopens, no environment and no network.

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

## Publishing

Nothing formal. A plugin is a `.wasm` file someone drops in a directory — which also makes it a
supply-chain surface, so:

- Declare the narrowest `allowed-hosts` that works. `anistream --plugins` shows your list to users,
  and asking for more than you need is visible.
- Say what it does in `display_name`. That string is what people see in the Providers screen.
- Version it. `describe()` is the only place you can tell a user which build they are running.
