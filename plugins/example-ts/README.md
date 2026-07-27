# example-ts

The same reference provider, in JavaScript. It exists to make one claim honest: **the ABI is
language-agnostic.** The Rust plugin could be mistaken for a special case — same language as the
host, same toolchain, maybe a shared assumption leaking through. This one shares nothing with the
host but `wit/anistream-provider.wit`, and `tests/conformance.rs` runs the *same* assertions
against both.

```sh
npm install
npm run build      # → anistream-example-plugin-ts.wasm
```

Copy the `.wasm` into `~/.config/anistream/plugins/` (or wherever `--plugins` reports) and
anistream picks it up.

## What it costs

Measured with `cargo run -p anistream-plugin --example plugin_bench --release`:

| | size | compile (once) | per call |
|---|---|---|---|
| example-rust | 0.1 MB | 19 ms | 38 µs |
| example-ts | 12.0 MB | 1.06 s | 923 µs |

The JavaScript component is 170× larger and 24× slower per call, because it embeds
StarlingMonkey — a whole JS engine — alongside a hundred lines of parsing. Compilation happens
once at load; the per-call figure includes instantiating a fresh store, which the host does
deliberately so one call cannot leak state into the next.

Both numbers are small next to a single HTTP request, so **either language is a reasonable choice.**
Pick Rust if you care about the download size of what you ship; pick this if you would rather write
JavaScript. What you should not do is conclude from the size that JS plugins need special
treatment — they run under the same default limits, which the conformance suite asserts.

## Notes for plugin authors

- **No dependencies, and no `fetch` polyfill.** The runtime has no sockets to offer; the host does
  the networking. That is why this file is a parser and nothing else.
- **Errors are thrown, not returned.** jco maps a WIT `result` to a return value plus a thrown
  error, so the error arm of `provider-error` becomes `throw { tag: 'not-found' }`. The host
  receives the same typed error a Rust plugin would return.
- **Imports carry the package version**: `anistream:provider/host@0.1.0`, not
  `anistream:provider/host`. Getting this wrong fails at componentize time with a module-resolution
  error rather than at runtime.
- **Names are lowerCamelCase on this side.** WIT's `list-episodes` is `listEpisodes`, and
  `duration-secs` is `durationSecs`.
- **`--disable all` is what removes WASI.** Without it the component imports a WASI floor it never
  uses; with it, the only import is `anistream:provider/host`.
