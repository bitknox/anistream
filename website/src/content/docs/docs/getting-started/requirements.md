---
title: Requirements
description: What anistream needs installed, and which terminals show it at its best.
---

```sh
brew install mpv ffmpeg cmake      # cmake is for wreq's BoringSSL
rustup toolchain install 1.97.1    # pinned by rust-toolchain.toml
```

- **mpv** does all playback, over JSON IPC.
- **ffmpeg** is used for remuxing and subtitle handling on downloads.
- **cmake** builds BoringSSL for the browser-fingerprint HTTP stack.
- **Rust 1.97.1** — the toolchain is pinned by `rust-toolchain.toml`, so `rustup` will pick it up
  automatically inside the repo.

Optional:

- `rustup target add wasm32-wasip2` — only if you want to build
  [plugins](/docs/plugins/authoring/).

## Terminal

Use a terminal with a real graphics protocol if you can: **Ghostty, Kitty, iTerm2, WezTerm**. Cover
art renders as actual images over the Kitty, Sixel or iTerm2 protocols. Elsewhere it falls back to
Unicode halfblocks.

```sh
anistream --doctor
```

reports what your terminal supports, whether mpv runs and which config it reads, and the state of
the VPN guard and your VPN's kill switch.
