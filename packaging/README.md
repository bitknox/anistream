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
| Windows | Start Menu shortcut | targets `wt.exe` with the binary as its argument, or the binary directly where Windows Terminal is absent |

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
- `linux/glibc-floor.sh` — fails the release when the Linux binary needs a newer glibc than it
  promises, and names the symbols that need it

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

## glibc on Linux

A binary linked on the runner needs the runner's glibc, 2.39 on Ubuntu 24.04, and will not start
on anything older. So the Linux build links against glibc 2.28's symbols instead, with
`cargo zigbuild --target x86_64-unknown-linux-gnu.2.28`, and the `.tar.gz`, the `.deb` and the
AppImage all run on Debian 10, Ubuntu 20.04, RHEL 8 and later. The release workflow then reads the
finished binary with `linux/glibc-floor.sh` and starts it in Debian 10, AlmaLinux 8, Ubuntu 20.04,
Debian 11 and Ubuntu 22.04 containers before packaging it.

The floor is `glibc` on the Linux entry of the workflow's matrix. The `.deb`'s `libc6` dependency
follows it. To check a binary by hand:

```sh
packaging/linux/glibc-floor.sh target/x86_64-unknown-linux-gnu/release/anistream 2.28
```

## The apps

A release carries the apps beside the terminal binaries: `anistream-<version>-android-arm64.apk`,
`anistream-<version>-macos.dmg` and `anistream-<version>-ios-unsigned.ipa`, each with a `.sha256`,
and `anistream-<version>-apps-third-party.zip`. `.github/workflows/apps-release.yml` builds them,
and `release.yml` calls it on a tag and adds what it built to the draft release after the
terminal builds. A pull request that changes the release workflows, this directory,
`apple/project.yml`, `android/app/build.gradle.kts`, the version catalog or the mpv and engine
scripts builds them all without publishing anything, and so does a dispatch of apps-release.yml.
CI's own jobs build the apps for everything else.

- `apps/version.sh` — the version from the tag, with the build number Android and Apple need,
  which grows with it (`v1.2.0` is 1020099, `v1.2.0-rc.2` 1020062). Gradle takes them as the
  properties `ANISTREAM_VERSION_NAME` and `ANISTREAM_VERSION_CODE`, Xcode as `MARKETING_VERSION`
  and `CURRENT_PROJECT_VERSION`; a local build keeps the ones in its project.
- `apps/third-party.sh` — the notices' zip: every licence and notice for libmpv, FFmpeg and what
  is inside them, the fonts and the shaders, with the scripts and patches each library was
  built from and what each build recorded; and the release's notes for the apps.
- `apps/release-notes.md` — those notes' install steps, for every version.
- `apps/publish.sh` — adds the files and the notes to the draft, replacing what an earlier run
  added.

The APK is signed with the project's release key, from the repository's `ANDROID_RELEASE_*`
secrets. The Mac app is signed ad hoc and not notarized, and the `.ipa` is unsigned, for
AltStore or SideStore to sign with each person's own Apple ID: there is no paid Apple developer
account behind either.

## Not signed

Neither the `.pkg` nor the `setup.exe` is code-signed, so both warn on first run. Fixing that means
an Apple Developer account for notarisation and an Authenticode certificate for Windows, and it is
a CI change rather than a packaging one — nothing in this directory would move.

## mpv and ffmpeg stay out

They are runtime dependencies anistream shells out to, not libraries it links. The `.deb` declares
them in `Depends:`; the other two say so in the docs. Bundling mpv would also pull GPL obligations
onto an MIT binary, which is a good reason not to.
