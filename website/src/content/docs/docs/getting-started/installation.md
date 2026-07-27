---
title: Installation
description: Building anistream from source. There are no published binaries yet.
---

There are no published binaries, no Homebrew formula and no `cargo install` yet — anistream is
installed from source:

```sh
git clone https://github.com/bitknox/anistream
cd anistream
cargo run --release
```

The first build is slow (BoringSSL, librqbit and a WASM runtime are in the tree); after that it is
incremental. The binary ends up at `target/release/anistream` if you want it on your `PATH`.

## Where things live

| | Linux | macOS |
|---|---|---|
| Config | `~/.config/anistream/config.toml` | `~/Library/Application Support/anistream/config.toml` |
| History database | `<data_dir>/anistream.db` | `<data_dir>/anistream.db` |
| Plugins | `<config_dir>/plugins/*.wasm` | `<config_dir>/plugins/*.wasm` |

No config file is required — one is only written when you change something in the Settings screen,
and those writes are format-preserving, so comments you add by hand survive.

:::note
Running from `cargo` rather than an installed binary has one consequence for tracker tokens: macOS
keys keychain access on the binary, and every rebuild produces a new one. See
[token storage](/docs/guides/trackers-sync/#token-storage).
:::
