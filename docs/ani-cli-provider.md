# Built-in ani-cli provider

This fork adds a native provider implementing the workflow used by ani-cli 5.1.6
(commit 0301d67d5768d332b049821a30082ad43d9a215a). It runs inside the shared Rust
engine, including iOS, without shell commands, iSH, a plugin store, or a server.

New configurations try `ani-cli` first. Existing configurations automatically gain
the provider at the front of the chain. To choose its position explicitly:

```toml
[providers]
order = ["ani-cli", "torrent", "plugins"]
```

Set `disabled = ["ani-cli"]` under `[providers]` to switch it off.

The provider uses HiAnime search and episode lists, selects the requested sub or dub
embed, decodes its configuration, and returns the HLS master playlist and soft
subtitles. The player retains adaptive variants and separate audio renditions.
It uses the shared browser-emulating HTTP client, retries connection failures through
the standard HTTP client, and reports challenges as blocked.
There is no CAPTCHA bypass, DRM decryption, AllDebrid integration, or torrent change.
Provider host and markup changes require updating this implementation.

The provider protocol was adapted from pystardust/ani-cli, copyright its contributors,
under GPL-3.0-or-later. See LICENSE-ani-cli. Original Anistream files retain their MIT
notices; distribution of the combined fork must comply with GPL-3.0-or-later.

Validation combines synthetic fixtures, live metadata and playlist requests, and an
iOS Simulator build. A signed physical-device build was also tested by the owner.

## Local verification, 29 October 2026

Feature branch base: `7054c13c`. Tested with Xcode 26.2 and Rust 1.99.1 on Apple silicon.

- Provider and core library tests: 318 passed, including a rerun after the lint cleanup.
- Workspace all-target check, workspace all-target clippy with warnings denied, no-default
  engine clippy, FFI model clippy, formatting and dependency-boundary checks passed.
- Engine library tests with the native-app features: 464 passed, including registration,
  switching ani-cli off and on, config persistence, and visibility after restart.
- The live probe returned 30 search results, 25 episodes, one sub stream and one subtitle.
  The stream's master playlist returned HTTP 200 and an HLS header. No video segments were fetched.
- The native engine and Swift app compiled for iOS 26.2 Simulator in Debug. Swift Release
  compilation also passed, using the same `app-dev` Rust engine to check the Swift sources.
- A temporary harness ran the real engine inside the isolated Fixtures app, with a temporary
  home and in-memory secrets. The ani-cli live search health check reported ready in 875 ms.
  Switching the native source off and back on both passed on the Simulator.
- The Simulator rendered the generated test episode with subtitles through embedded libmpv.
  This checks the player independently of the live provider.
- Three small Apple compatibility changes were needed for Xcode 26.2: explicit numeric
  conversions in the player and `duration(to:)` for the artwork retry clock.
- The iOS Store/UI test run passed 319 of 324 tests. Artwork and source-picker suites passed.
  Five tests in untouched behavior failed: two reducer tests assume title capitalization,
  one presence-settings test assumes desktop fields, one update-label test expects desktop
  capitalization, and one widget-route test expects a Settings pane instead of the iPhone sheet.
  The full Apple suite is therefore not green.
- The provider screen now uses the native provider's name, built-in origin and host list.
  The source switch accepts native web providers, and disabled ani-cli stays visible so it
  can be enabled again after restart.

A separately identified development build using the real engine was installed on an iPhone
17e over USB. The owner tested the app and reported that it works. This is a manual device
confirmation, not an automated playback assertion. The temporary Simulator and device launch
harnesses were removed from the app sources after testing; normal app startup is unchanged.

Successful live dub resolution and a distributable release IPA remain unverified. Some sampled
results did not have a resolvable sub or dub server and returned `NotFound`.

To repeat the metadata check:

```sh
cargo run -p anistream-providers --example ani_cli_probe -- QUERY
```

Add `--resolve` to check the first result's first sub episode and fetch its HLS playlist,
or `--resolve --dub` for dub. The probe fetches playlists only, never video segments.
Run under the local-checks HOME guard.

Upstream's Apple release workflow uses a custom `xcode-27` runner. Producing an IPA in this
fork still requires an equivalent runner or an adjusted build workflow.
