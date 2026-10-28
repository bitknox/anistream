---
paths:
  - "crates/anistream-store-core/**"
  - "crates/anistream-store-server/**"
  - "crates/anistream-store-wasm/**"
  - "crates/anistream-plugin-store/**"
  - "deploy/**"
  - "Cargo.toml"
  - "Cargo.lock"
  - "rust-toolchain.toml"
---

# Plugin store checks

- The admin page's built assets are committed. After changing its source, run
  `(cd crates/anistream-store-core/admin && bun install --frozen-lockfile && bun run build)`
  and include the resulting `crates/anistream-store-core/assets/admin/` changes.
  Edit source, then rebuild; CI checks that source and output agree.
- `deploy/sync.sh --check` detects stale store WASM, template copies and the shared
  Node handler. Run it after changing store code, embedded assets, dependencies or
  the toolchain. If stale, run `deploy/sync.sh` and include its generated changes.
  This needs modern Bash, wasm-pack and the `wasm32-unknown-unknown` Rust target.
- The Node handler is authored in `deploy/vercel/lib/handler.js`; sync copies it to
  Netlify. `deploy/sync.sh --copy` copies existing output but does not rebuild WASM.
- Template integration checks use local services: install with
  `bun install --frozen-lockfile` in `deploy/{cloudflare,vercel,netlify,test}`, then
  run `deploy/test/journeys.sh` (or pass `cloudflare`, `vercel` or `netlify`).
  Use Node 24 to match CI. The journey claims a fresh store and publishes test data;
  use its temporary local services for routine verification.
