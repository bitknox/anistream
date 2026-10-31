# Flatpak

The Linux desktop app (`desktop/linux`) as a Flatpak, app id `tv.anistream.desktop`, on the GNOME
49 runtime (GTK 4.20, libadwaita 1.8; the app asks for 4.16 and 1.7). anistream hosts no content,
and the Flatpak is the same app as the AppImage: the same sources, libmpv built as LGPL from the
same pinned FFmpeg and mpv, and the same permissions the app asks for in the other builds.

- `tv.anistream.desktop.yml` is the manifest. Each permission has a comment saying why.
- `desktop/linux/data/tv.anistream.desktop.metainfo.xml` is the AppStream file and
  `tv.anistream.desktop.desktop` beside it the desktop entry.
- `build.sh <out>` builds it in Docker with flatpak-builder, writes `<out>/anistream-<arch>.flatpak`,
  installs it and runs `start-check.sh`, which starts it on a virtual display.
- `check-pins.sh` fails when the manifest's FFmpeg, libass or mpv differ from `android/scripts/mpv.sh`.

Checked on 2026-10-31 on aarch64 only (Docker on an Apple silicon Mac): it builds in about 25
minutes on 12 cores, the bundle is 26 MB (130 MB installed), and `start-check.sh` opens the window
and keeps it up. x86_64 and a real GPU are untested; the NVDEC options for x86_64 are in the
manifest but were not built.

Updates: inside a Flatpak the app does not update itself and says so in Settings
(`desktop/linux/src/updates.rs`); Flatpak does it.

## What Flathub would need

Not done here, and not a promise to submit.

1. A Flathub pull request for `flathub/flathub` (new app), reviewed by the Flathub team, with the
   manifest as `tv.anistream.desktop.yml` in its own repository `flathub/tv.anistream.desktop`.
2. App id ownership: Flathub verifies `tv.anistream.desktop` through a domain, and the id must be
   the reverse of one the project owns. The project's domain is anistream.tv, so the id would
   conventionally be `tv.anistream.*` with a `.well-known/org.flathub.VerifiedApps.txt` on
   anistream.tv, or the GitHub-based `io.github.bitknox.*` form.
3. Offline builds: Flathub's builders have no network. Cargo crates must be listed as sources with
   `flatpak-cargo-generator.py desktop/linux/Cargo.lock`, and the `--share=network` build argument
   removed. The app's source becomes a `git` source at the release tag, not the `dir` source.
4. AppStream: screenshots (with URLs on a stable host), a longer description, `<releases>` kept up
   to date for every version, the OARS content rating checked, and a `developer` and `branding`
   block. Run `appstreamcli validate --pedantic` and `flatpak-builder-lint` against both.
5. The licences: the app is MIT and the bundle carries LGPL libmpv and FFmpeg. The AppImage's
   notices also name an upscaling model under CC BY-NC 4.0 (non-commercial). Flathub accepts only
   free software, so that asset would need to be left out of a Flathub build, or the question put
   to the reviewers. The metainfo's `project_license` would then list what remains.
6. Review of the permissions: `--device=input`, `--own-name` for MPRIS and the two Discord sockets
   are the ones a reviewer will ask about. Each is justified in the manifest.
7. A maintainer who can answer reviews, and a plan for the runtime: GNOME 49 goes end of life and
   the manifest must move to a newer branch about once a year.
8. A CI job (not added here). The existing Linux job builds the AppImage in `apps-release.yml` on
   `ubuntu-latest` and `ubuntu-24.04-arm`. A Flatpak job would follow the same matrix with the
   container `ghcr.io/flathub-infra/flatpak-github-actions:gnome-49` and attach the bundle to the
   release, but it adds a long build (the first one needs about an hour for libmpv) to a workflow
   that gates every release, so it is proposed in the pull request for a decision.
