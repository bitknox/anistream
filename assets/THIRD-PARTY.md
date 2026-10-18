# Third-party assets

Everything under `assets/mpv/` is embedded in the anistream binary and written to
`<cache>/mpv/<hash>/` at launch (`crates/anistream-player/src/bundle.rs`), licence texts
included, so a copy of each licence travels with every binary. This file records where each
piece came from and under what terms. Update it in the same commit as the files.

## uosc

| | |
|---|---|
| What | mpv on-screen controller: `assets/mpv/scripts/uosc/**`, `assets/mpv/fonts/uosc_icons.otf`, `assets/mpv/fonts/uosc_textures.ttf` |
| Source | https://github.com/tomasklaen/uosc, release `5.13.0` (tag object `6a0fc25184429852daf755c79e82f9cda650f7bc`, commit `d124c2c930d69446448022851373e00ae592390d`) |
| Taken from | release asset `uosc.zip`, sha256 `4be9da3289285300fa374496c3f1bfd7bb20ac08e890d25bd5a06b28eebe4882` |
| Licence | LGPL-2.1, `assets/mpv/licenses/LICENSE-uosc` (`LICENSE.LGPL` at the tag) |
| Changes | None. The zip's `scripts/uosc/bin/` (the prebuilt `ziggy` helpers, 17 MB) is left out. |

Without `ziggy`, uosc loses the features that call it: searching and downloading subtitles
from OpenSubtitles, the in-player update check and the "open changelog" link, and the
clipboard on mpv older than 0.40 (0.40 and later use mpv's own `clipboard/text` property).
Each fails inside its own menu or dialog with an error line and a console message; nothing
else depends on the helper. uosc only offers its self-updater after a successful check, so
without the helper it can never install a second uosc into the user's mpv config directory.

`uosc_icons.otf` is Google's Material Icons Round as shipped by uosc, under the Apache
License 2.0: `assets/mpv/licenses/LICENSE-material-icons`, from
https://github.com/google/material-design-icons (`LICENSE` at `49d4db35df873165d6bd6ba09b063c7dafbac2f4`).

## thumbfast

| | |
|---|---|
| What | seekbar thumbnailer: `assets/mpv/scripts/thumbfast.lua` |
| Source | https://github.com/po5/thumbfast, `master` at `0f711de3138c9bd6718209d819ac54022c23ded2` (2026-06-28) |
| Upstream file sha256 | `a3d08e71eae8b892f6cd39f9593ea219768e709312d176bca883841b156448bf` |
| Licence | MPL-2.0, `assets/mpv/licenses/LICENSE-thumbfast` |
| Changes | Yes, each marked `-- anistream:` in the file; summarised below. |

The patch makes the thumbnailer open `user-data/anistream/thumbnail-url` instead of the
playing file's `path` when the host sets it (a torrent stream gives the thumbnailer its own
URL so it does not disturb piece priority), retires a running thumbnailer when that URL
changes mid-file, and passes the parent's `http-header-fields` (one
`--http-header-fields-append` per entry, so commas survive), `user-agent` and `referrer` to
the child process. It also hides a thumbnail whose frame has not arrived within 350 ms, so a
seek into a part of a torrent not yet downloaded shows nothing rather than the last frame. The modified file is shipped in source form, as MPL-2.0 requires.
`assets/mpv/thumbfast-opts.conf` is anistream's own and not part of thumbfast.

## Zen Kaku Gothic New

| | |
|---|---|
| What | UI font: `assets/mpv/fonts/ZenKakuGothicNew-Regular.ttf`, `assets/mpv/fonts/ZenKakuGothicNew-Bold.ttf` |
| Source | https://github.com/google/fonts, `ofl/zenkakugothicnew` at `ba225958c40ae6268a256861d80355a84b26e535`; upstream project https://github.com/googlefonts/zen-kakugothic |
| sha256 | Regular `b840cd07a67d89cacca44249ae49aa99ee7640eb5ce623be8d8983d6aabac801`, Bold `0081cedabc4921982fcd061f845a005664ac7fb642af2dd34b4007bc63ccd235` |
| Licence | SIL Open Font License 1.1, `assets/mpv/licenses/OFL-ZenKakuGothicNew.txt` |
| Changes | None. Light, Medium and Black (about 2.2–2.3 MB each) are not shipped. |

## Anime4K

| | |
|---|---|
| What | upscaling shaders: `assets/shaders/Anime4K_*.glsl` |
| Source | https://github.com/bloc97/Anime4K; vendored before this file existed, and the commit was not recorded |
| Licence | MIT, `assets/shaders/LICENSE-Anime4K` |
