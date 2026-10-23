## Terminal

The installers are the `.pkg` on macOS, `setup.exe` on Windows and the `.deb` on Debian and
Ubuntu. Each one puts `anistream` on your `PATH` and adds a launcher, so you can type it or click
it. The launcher opens your terminal, because anistream runs in one.

On any other Linux distribution, use the AppImage: `chmod +x` it and run it from a terminal.
Nothing is installed. Every Linux build needs glibc 2.28 or newer.

The `.tar.gz` and `.zip` archives hold only the binary, for putting on your `PATH` yourself.
Once it is installed, `anistream --update` takes each new release.

These builds are not code-signed. macOS stops the `.pkg` the first time it opens: choose Done,
then Open Anyway in System Settings › Privacy & Security. Windows SmartScreen warns about the
installer: choose More info, then Run anyway.

mpv and ffmpeg are not bundled. anistream drives mpv for playback and runs ffmpeg to mux
subtitles into a finished download, so install both first; see the
[requirements](https://anistream.tv/docs/getting-started/requirements/).

Check a download against its `.sha256` before running it.

Every package carries the licences of what it is built from in a `third-party` folder, the Rust
crates' among them, and `anistream-@VERSION@-rust-licences.txt` has those for all four builds.

anistream hosts no content. What you point any source at is your own decision, at your own risk.
