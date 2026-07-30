# Packaging

Installers, as opposed to the plain archives the release workflow also builds. The point of these
is that one install gets you both ways of running anistream: `anistream` on your `PATH`, and a
launcher in the place your desktop keeps applications.

A TUI has no window of its own, so every launcher here opens a terminal and runs anistream inside
it. Each platform has its own way of saying that:

| | Launcher | How it opens a terminal |
|---|---|---|
| Linux | `.desktop` entry | `Terminal=true`, which the spec has for exactly this |
| macOS | `.app` bundle | `launcher.sh` re-launches the binary via `open -a` |
| Windows | Start Menu shortcut | targets `wt.exe` with the binary as its argument |

macOS is the awkward one. Finder launches a bundle with no controlling terminal, so
`Contents/MacOS/launcher` is a shell script and the real binary sits beside it. The script walks a
list of terminals — `$ANISTREAM_TERMINAL` first, then Ghostty, kitty, WezTerm, iTerm and finally
Terminal.app, which every Mac has.

The Linux AppImage embeds the same `.desktop` entry and icon, and is assembled inline in the
release workflow like the `.deb`.

## Files

- `linux/anistream.desktop` — desktop entry, installed to `/usr/share/applications`
- `macos/Info.plist` — bundle metadata; the version is substituted at package time
- `macos/launcher.sh` — the bundle's entry point
- `windows/anistream.nsi` — NSIS installer: per-user, no UAC prompt

Icons come from `assets/icon/`, generated from the mascot by `tools/icons/generate.ts`. Regenerate
with `bun tools/icons/generate.ts`, then `iconutil -c icns assets/icon/anistream.iconset` on macOS.

## Building one by hand

The release workflow does all of this, but each is runnable alone.

```sh
# Windows, needs makensis (brew install nsis)
makensis -DVERSION=0.1.0 -DSOURCE=<dir with anistream.exe, anistream.ico, LICENSE, README.md> \
  packaging/windows/anistream.nsi
```

macOS and Linux are assembled inline in `.github/workflows/release.yml`, since both are a handful
of `install` calls followed by `pkgbuild` or `dpkg-deb`.

## Not signed

Neither the `.pkg` nor the `setup.exe` is code-signed, so both warn on first run. Fixing that means
an Apple Developer account for notarisation and an Authenticode certificate for Windows, and it is
a CI change rather than a packaging one — nothing in this directory would move.

## mpv and ffmpeg stay out

They are runtime dependencies anistream shells out to, not libraries it links. The `.deb` declares
them in `Depends:`; the other two say so in the docs. Bundling mpv would also pull GPL obligations
onto an MIT binary, which is a good reason not to.
