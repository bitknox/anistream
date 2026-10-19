---
title: Writing a provider plugin
description: A provider plugin is a WebAssembly component, a parser and not a networking stack.
---

A provider plugin is a WebAssembly component: search terms in, episodes and stream URLs out.
anistream loads `*.wasm` from the plugin directory (see
[where things live](/docs/getting-started/installation/#where-things-live), or run
`anistream --plugins`) and treats it like a native source, with the same ranking, health tracking
and failover. Any WASI 0.2 language works; reference implementations in Rust and JavaScript live in
`plugins/`.

:::note
The sandbox has no sockets. The host lends you `fetch`, and your plugin is a parser.
[Sandbox & guarantees](/docs/plugins/sandbox/) covers what is enforced.
:::

## The interface

`wit/anistream-provider.wit` is the contract, package `anistream:provider@1.1.0`:

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

- `fetch-many`: several requests concurrently, results in request order. Same allowlist and
  fetch budget, per request. Return *all* your streams from `resolve`: playback falls over to
  the next stream when one produces no frames, so ordering is a hint, not a verdict.
- `sources`: the selectable releases for the Sources overlay, best-first. An empty list means
  "nothing to choose between", which is an answer and not a failure.
- `resolve-source`: one candidate's id back into streams. Never fall back to the automatic pick.
- `aes-decrypt`: AES-CBC. The key length selects 128, 192 or 256.
- `config-get`: reads `[providers.plugins.settings.<your-id>]` from the user's config.toml.
  Every key is optional, so work with all of them unset.

Record fields are documented in the WIT. Easy to miss: `synonyms`/`format` on a search hit (title
matching), `description`/`thumbnail`/`air-date`/`filler` on an episode, `format` on a subtitle
whose URL has no extension, `download-source` on a stream.

Errors map one-for-one onto the registry's failover rules:

| Return | Meaning | Failover? |
|---|---|---|
| `not-found` | Works, no such title or episode | Yes, not counted as a fault |
| `blocked(msg)` | Bot protection, rate limit, geo-block, ban | Yes |
| `parse(msg)` | Response arrived, did not look like expected | Yes |
| `other(msg)` | Anything else | Yes |

Do not flatten `not-found` into `other`. A missing title is an answer, and reported as one: the
Providers screen and the failure message would otherwise show a fault where the source only lacks
the title.

## Rust

```sh
cargo build --release --target wasm32-wasip2 \
  --manifest-path plugins/example-rust/Cargo.toml
# then copy the .wasm into the plugin directory; `anistream --plugins` prints it
```

Start from `plugins/example-rust/src/lib.rs`. It exercises the whole ABI against a stable
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
- Exports run without a receiver: `this.resolve(…)` traps, so share a plain function.
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

To share a plugin, publish it to a [plugin store](/docs/plugins/stores/), or hand over the `.wasm`
for someone to put in their plugin directory. Either way, declare the narrowest `allowed-hosts`
that works, since anistream shows it to users, name yourself clearly in `display-name`, and
version `describe()`.

A store reads the id, version and hosts from the plugin itself, and checks them:

- The id is 1 to 64 characters of `a`–`z`, `0`–`9`, `-` and `_`, and does not start with `-` or
  `_`. It cannot be `torrent`, `remote` or `plugins`, the names of anistream's own sources. The id
  becomes the file name on every member's machine, so use one no other plugin uses.
- The version is 1 to 64 letters, digits and `.+-_`, and does not start with `.`.
- A published version never changes. Publish a new version for any change, however small.

A store that cannot run WebAssembly, such as one on Cloudflare Workers, Vercel or Netlify, cannot
ask the plugin what it is. It needs the plugin to carry its manifest in the file, and refuses one
without it. anistream adds the manifest when you publish from the app, and
`anistream plugin stamp plugin.wasm` adds it to a file you publish some other way.
