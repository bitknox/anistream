## Apps

anistream for Android, the Mac, iPhone and iPad, on the same engine as the terminal app. Like
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

### Checking a download

Each file has a `.sha256` beside it. With both in one folder, run
`shasum -a 256 -c anistream-@VERSION@-macos.dmg.sha256` (or `sha256sum -c` on Linux).
