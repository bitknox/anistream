---
paths:
  - "crates/anistream-plugin/**"
  - "plugins/**"
  - "wit/**"
---

# Provider plugin validation

`crates/anistream-plugin/tests/conformance.rs` skips components that are not built,
so a green test result alone does not prove the sandbox or ABI works. For host,
sandbox or WIT changes, build all three fixtures before running conformance tests:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml
cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-hostile/Cargo.toml
(cd plugins/example-ts && npm ci && npm run build)
cargo test -p anistream-plugin --test conformance -- --nocapture
```

Check the output for `skipping` messages and disclose any missing fixture coverage.
Use `docs/plugins.md` for the ABI and JS component pitfalls. Built plugin `.wasm`
files are ignored artifacts; the deploy store's committed WASM is a separate workflow.
