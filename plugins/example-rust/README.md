# example-rust

A reference provider plugin. Build it with:

```sh
cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml
```

The component lands at `target/wasm32-wasip2/release/anistream_example_plugin.wasm`. Copy it into
the plugin directory (`anistream --plugins` prints it) and anistream will pick it up.

It exists to be *read*: it exercises every part of the ABI — the manifest, the lent `fetch`, the
lent regex, the error vocabulary — against a stable public endpoint, so the shape of a real
provider is visible without needing a live source to be up.
