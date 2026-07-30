---
title: Writing a provider plugin
description: A provider plugin is a WebAssembly component — a parser, not a networking stack.
---

A provider plugin is a WebAssembly component: search terms in, episodes and stream URLs out.
anistream loads `*.wasm` from the plugin directory (see
[where things live](/docs/getting-started/installation/#where-things-live), or run
`anistream --plugins`) and treats it like a native source — same ranking, health tracking and
failover. Any WASI 0.2 language works; reference implementations in Rust and JavaScript live in
`plugins/`.

:::note
The sandbox has no sockets — the host lends you `fetch`. Your plugin is a parser;
[Sandbox & guarantees](/docs/plugins/sandbox/) covers what is enforced.
:::

## The interface

`wit/anistream-provider.wit` is the contract — package `anistream:provider@1.1.0`:

```wit
interface host {                          // what you may call
    fetch:          func(req: http-request) -> result<http-response, host-error>;
    log:            func(level: string, msg: string);
    aes-decrypt:    func(key: list<u8>, iv: list<u8>, data: list<u8>) -> result<list<u8>, string>;
    regex-captures: func(pattern: string, haystack: string) -> list<list<string>>;
    fetch-many:     func(reqs: list<http-request>) -> list<result<http-response, host-error>>;
    config-get:     func(key: string) -> option<string>;
}

interface provider {                      // what you must export
    describe:       func() -> manifest;
    search:         func(query: string, translation: string) -> result<list<search-hit>, provider-error>;
    list-episodes:  func(id: string, translation: string) -> result<list<episode>, provider-error>;
    resolve:        func(id: string, episode: string, translation: string)
                      -> result<list<media-stream>, provider-error>;
    sources:        func(id: string, episode: string, translation: string)
                      -> result<list<source-candidate>, provider-error>;
    resolve-source: func(id: string, episode: string, translation: string, source-id: string)
                      -> result<list<media-stream>, provider-error>;
}
```

- `fetch-many` — several requests concurrently, results in request order. Same allowlist and
  fetch budget, per request. Return *all* your streams from `resolve`: playback falls over to
  the next stream when one produces no frames, so ordering is a hint, not a verdict.
- `sources` — the selectable releases for the Sources overlay, best-first. An empty list means
  "nothing to choose between": an answer, not a failure.
- `resolve-source` — one candidate's id back into streams. Never fall back to the automatic pick.
- `aes-decrypt` — AES-CBC; the key length selects 128, 192 or 256.
- `config-get` — reads `[providers.plugins.settings.<your-id>]` from the user's config.toml.
  Every key is optional; work with all of them unset.

Record fields are documented in the WIT. Easy to miss: `synonyms`/`format` on a search hit (title
matching), `description`/`thumbnail`/`air-date`/`filler` on an episode, `format` on a subtitle
whose URL has no extension, `download-source` on a stream.

Errors map one-for-one onto the registry's failover rules:

| Return | Meaning | Failover? |
|---|---|---|
| `not-found` | Works, no such title or episode | **No** — this is an answer |
| `blocked(msg)` | Bot protection, rate limit, geo-block, ban | Yes |
| `parse(msg)` | Response arrived, did not look like expected | Yes |
| `other(msg)` | Anything else | Yes |

Do not flatten `not-found` into `other` — that makes every missing episode walk the whole chain.

## Rust

```sh
cargo build --release --target wasm32-wasip2 \
  --manifest-path plugins/example-rust/Cargo.toml
# then copy the .wasm into the plugin directory — `anistream --plugins` prints it
```

Start from `plugins/example-rust/src/lib.rs` — it exercises the whole ABI against a stable
endpoint.

## JavaScript

```sh
cd plugins/example-ts && npm install && npm run build
# then copy anistream-example-plugin-ts.wasm into the plugin directory
```

Five differences from Rust:

- Imports carry the package version: `anistream:provider/host@1.1.0`.
- Names are lowerCamelCase: `list-episodes` → `listEpisodes`.
- Errors are thrown, not returned: `throw { tag: 'not-found' }`. `fetch` needs `try`/`catch`.
- Exports run without a receiver — `this.resolve(…)` traps; share a plain function.
- `--disable all` strips WASI.

## Cost

Measured with `cargo run -p anistream-plugin --example plugin_bench --release`:

| | size | compile (once) | per call |
|---|---|---|---|
| Rust | 0.1 MB | 19 ms | 38 µs |
| JavaScript | 12.2 MB | 1.06 s | 923 µs |

Both run under the same default limits; both are negligible beside one HTTP request. Every call
gets a fresh store, so no state survives between calls.

## Publishing

A plugin is a `.wasm` file someone drops in a directory, so: declare the narrowest
`allowed-hosts` that works (`anistream --plugins` shows it to users), name yourself clearly in
`display_name`, and version `describe()`.
