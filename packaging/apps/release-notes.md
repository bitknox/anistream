## Apps

anistream for Android phones and TVs, the Mac, iPhone, iPad, Windows and Linux, on the same engine as the terminal app. Like
it, the apps host no content. Plugins and sources are yours to add, and what you point them at is
your own decision, at your own risk.

### Android

`anistream-@VERSION@-android-arm64.apk`, for Android 15 or later on a 64-bit ARM phone or tablet.

Open the APK on the device. Android asks once whether the app you opened it from, your browser or
file manager, may install apps: allow it, then choose Install. The APK is signed with anistream's
release key, so each release installs over the one before and keeps your data. It is not on
Google Play, and Play Protect may offer to scan it. Once it is installed, the app updates itself
when you choose Update in its notice or in Settings › About. The first time, Android asks you to
allow it for anistream too.

### Android TV

`anistream-@VERSION@-android-tv-arm64.apk` or `anistream-@VERSION@-android-tv-armv7.apk`, for
Android TV and Google TV on Android 11 or later. Most TV boxes, including the Chromecast with
Google TV, run a 32-bit system and take the armv7 file; the Nvidia Shield and newer boxes take the
arm64 file. If the install says the app is not compatible, try the other file.

Install it with a sideloading app on the TV, or with `adb install` and the file's name
from a computer on the same network. It is signed with the same release key as the phone app and
installs beside it. It is not on Google Play. Pair it with your phone from the TV's first screen.

### Mac

`anistream-@VERSION@-macos.dmg`, for macOS 26 or later on a Mac with Apple silicon.

Open the disk image and drag Anistream to Applications. The app is signed ad hoc rather than with
an Apple developer account, and it is not notarized, so macOS stops it the first time it opens:

1. Open Anistream, and choose Done when macOS says it cannot verify it.
2. In System Settings › Privacy & Security, under Security, choose Open Anyway beside the message
   about Anistream, and confirm with your password or Touch ID.

That is once for each version you download. Right-click › Open no longer skips the check since
macOS 15. From a terminal, `xattr -dr com.apple.quarantine /Applications/Anistream.app` does the
same. Once it is installed, the app updates itself when you choose Update in its notice or in
Settings › Integrations, and macOS does not ask again.

After an update macOS may ask again whether Anistream can use what it keeps in your keychain.
Choose Always Allow.

### iPhone and iPad

`anistream-@VERSION@-ios-unsigned.ipa`, for iOS or iPadOS 26 or later.

The .ipa is not signed for any device. Install it with [AltStore](https://altstore.io) or
[SideStore](https://sidestore.io), which sign it with your own Apple ID as they install it. Add
anistream's source to either, and they offer each new release:
`https://github.com/bitknox/anistream/releases/latest/download/sidestore.json`. They
use two of your Apple ID's App IDs, one for the app and one for its widgets, and an App Group the
two share. With a free Apple ID that signature lasts 7 days: AltStore or SideStore has to refresh
the app within that time, or it will not open until they do. Your data stays either way.

### Windows

`anistream-@VERSION@-windows-x64-setup.exe`, or `anistream-@VERSION@-windows-arm64-setup.exe` for
Windows on Arm, for Windows 10 version 2004 or later.

Run the installer. It installs for your account only, with no administrator prompt, adds anistream
to the Start menu and to Settings › Apps, where you can uninstall it, and lets the app open
anistream:// links. The installer is not code-signed, so SmartScreen stops it the first time:
choose More info, then Run anyway. A new release installs over the one before and keeps your
library and settings.

### Linux

`anistream-@VERSION@-linux-x86_64.AppImage`, or `anistream-@VERSION@-linux-aarch64.AppImage` on
64-bit ARM, for systems with glibc 2.41 or newer: Debian 13, Ubuntu 25.04, Fedora 42 and later. It
carries GTK 4, libadwaita and the player.

Make it executable with `chmod +x anistream-@VERSION@-linux-x86_64.AppImage` and open it. The first
run registers it for anistream:// links. The app updates itself: it downloads each new version,
checks it, and puts it in place of the AppImage when you choose Restart to Update in its notice or
in Settings. AppImageUpdate, AppImageLauncher and Gear Lever can update it in place too, from the
release's `.zsync` file. Without FUSE, run it with `--appimage-extract-and-run`.

### Checking a download

Each file has a `.sha256` beside it. With both in one folder, run
`shasum -a 256 -c anistream-@VERSION@-macos.dmg.sha256` (or `sha256sum -c` on Linux).
