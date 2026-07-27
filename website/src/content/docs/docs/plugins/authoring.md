---
title: Writing a provider plugin
description: A provider plugin is a WebAssembly component — a parser, not a networking stack.
---

A provider plugin is a WebAssembly component that turns search terms into episodes and episodes
into stream URLs. anistream loads it from `~/.config/anistream/plugins/*.wasm` and treats it
exactly like a native source: same ranking, same health tracking, same failover.

You can write one in any language that targets WASI 0.2 — Rust, Go (TinyGo), JavaScript (jco),
Python (componentize-py). Two reference implementations live in `plugins/`, in **Rust** and
**JavaScript**, and the conformance suite runs the *same* assertions against both.

:::note
The sandbox has no sockets — the host lends you `fetch`. That single fact shapes everything about
plugin authoring; [Sandbox & guarantees](/docs/plugins/sandbox/) explains what it buys you and
what is enforced.
:::

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

`provider-error` mirrors anistream's internal error type one-for-one, which is what lets the
registry apply the same failover rules to a plugin as to a native source. The distinction that
matters:

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

Start from `plugins/example-rust/src/lib.rs`. It exercises every part of the ABI against a stable
public endpoint rather than a real source, so it keeps working when someone else's markup changes.

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
  error, so `throw { tag: 'not-found' }` and `throw { tag: 'blocked', val: 'reason' }`. Host
  errors arrive the same way, so calling `fetch` needs a `try`/`catch`.
- **`--disable all`** is what strips WASI. Without it the component imports a floor it never uses.

## What it costs

Measured with `cargo run -p anistream-plugin --example plugin_bench --release`:

| | size | compile (once) | per call |
|---|---|---|---|
| Rust | 0.1 MB | 19 ms | 38 µs |
| JavaScript | 12.0 MB | 1.06 s | 923 µs |

The JS component embeds a whole JS engine, so it is 170× larger and 24× slower per call. Both are
negligible beside a single HTTP request, and **both run under the same default limits** — the JS
build needs no extra memory ceiling. Pick Rust if the size of what you ship matters, JavaScript if
you would rather write JavaScript.

The per-call figure includes instantiating a fresh store, which is what stops one call leaking
state into the next: a plugin cannot stash a cookie from your search and use it during someone
else's.

## Publishing

Nothing formal. A plugin is a `.wasm` file someone drops in a directory — which also makes it a
supply-chain surface, so:

- Declare the narrowest `allowed-hosts` that works. `anistream --plugins` shows your list to
  users, and asking for more than you need is visible.
- Say what it does in `display_name`. That string is what people see in the Providers screen.
- Version it. `describe()` is the only place you can tell a user which build they are running.
