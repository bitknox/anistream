# anistream for Android

The Android app: Kotlin and Jetpack Compose on the anistream engine, which it reaches through the
Kotlin bindings UniFFI generates from `crates/anistream-ffi`. Phones, tablets and foldables,
Android 15 (API 35) and up, built to the boards in `docs/android-design-brief.md` and the motion
spec in `docs/motion-android.md`.

anistream ships no sources of its own and hosts no content. Every source is external.

## Modules

| Module | What it holds | On Apple |
|---|---|---|
| `:engine` | The generated bindings and the engine's library, per flavour | AnistreamKit |
| `:store` | `EngineClient`, `AppStore` and its feature models, `EngineLaunch`, `AppHost` and the keystore-sealed secrets | AnistreamStore |
| `:design` | The Obi & Silk colour scheme, type, shapes, motion tokens, Nagare, the cover plate | AnistreamDesign |
| `:ui` | The navigation bar and the screens | AnistreamUI |
| `:player` | The player, once the Player track brings it over from its spike | AnistreamPlayer |
| `:app` | The activity, the application, and each flavour's launch | the app target |
| `:testkit` | For tests only: the engine's scenes recorded as JSON, and the fixture titles' art | – |

Two flavours, by the engine inside:

- `app` (`tv.anistream.app`): the engine the app ships, on the real AniList and sources.
- `fixtures` (`tv.anistream.fixtures`): the engine's `testing` build, on the test kit's fake
  AniList and fixture titles, with nothing leaving the device. For screenshots and tests, never for
  real use.

## What it needs

- Android Studio, for its JDK (`/Applications/Android Studio.app/Contents/jbr/Contents/Home`), and
  the SDK with platform 37, the NDK and the emulator. The scripts find the SDK at
  `~/Library/Android/sdk` unless `ANDROID_HOME` says otherwise, and the newest NDK in it.
- The Rust toolchain the workspace pins, with `rustup target add aarch64-linux-android`, and
  `cargo install cargo-ndk`.
- Bash 4.4 or newer for the scripts (`brew install bash`).

## Building

```sh
android/scripts/engine.sh testing   # the fixtures flavour's engine
android/scripts/engine.sh release   # the app flavour's
android/scripts/fonts.sh            # Roboto Flex and Zen Kaku Gothic New, with their licences
cd android && ./gradlew assembleFixturesDebug assembleAppDebug
```

`engine.sh` builds `anistream-ffi` twice: for arm64 phones with cargo-ndk (Android 35), into the
APK's `jniLibs`, and for this Mac, for the unit tests and for the `uniffi-bindgen` that writes the
Kotlin bindings from the arm64 library. Everything goes to `android/.engine/<release|testing>/`,
which git ignores, with a stamp of the Rust sources it was built from. Cargo builds in
`~/anistream-target-android` unless `CARGO_TARGET_DIR` says otherwise; it takes about 9 GB.
Cargo runs in a clean environment, which keeps cargo's and rustup's variables (`CARGO_BUILD_JOBS`
among them), `RUSTC_WRAPPER` and sccache's `SCCACHE_*`, so a compiler cache works through it.
Without an NDK it still builds this Mac's library and the bindings, so Gradle and the unit tests
run, and the APK has no engine in it.

Each build is also kept in a cache every worktree shares, `~/.cache/anistream-engine/android/`
(`ANISTREAM_ENGINE_CACHE` names another root), by the sources' hash, the flavour, the profile,
the ABI and the machine. A new worktree on sources some worktree has built already copies that
build in, in under a second and with no cargo, which is most app work: it rarely changes
`crates/`. The script says `engine: reused <hash>` or `engine: built <hash>`. An entry is written
beside its place and renamed into it, so two builds at once never leave half of one, and entries
unused for 14 days are removed; each takes about 370 MB.

Every Gradle build checks first that its flavour's engine is current and that the fonts are there
(`engine.sh --check`, `fonts.sh --check`), and fails saying what to run when not. Rust changes
need `engine.sh` again; Gradle never builds Rust itself.

`ANISTREAM_ENGINE_PROFILE` picks the cargo profile: `app-dev` (opt-level 1, line tables) unless it
says otherwise, `release` for a build to keep.

## Running

```sh
adb install -r android/app/build/outputs/apk/fixtures/debug/app-fixtures-debug.apk
adb shell am start -n tv.anistream.fixtures/tv.anistream.app.MainActivity --es scene fresh --es section seasonal
```

The fixtures flavour takes intent extras under the names of the Apple app's `-Anistream*`
arguments: `scene` (`watching`, `fresh` or `playing`), `media` (what the playing scene streams),
`section` (`continue`, `calendar`, `seasonal`, `library`, `search`), `title` (a title page to
open, by id), `query` (a search to run), `page` (a page over the destination, by its route:
`settings`, `settings/playback`, `settings/sources`, `downloads`), `watched` (`900006:8`, or
several titles joined by `/`), `downloads` (any value: three downloads seeded, see below), `notify`
(any value: two new-episode notifications posted, with the fixtures' covers), `check` (seconds: the
new-episode check run once that many seconds on, as the periodic work runs it; kill the app first
for it to start an engine of its own), and
`sheet` and `player` for the screens that take them in later phases. A scene is the engine's
own (`start_scene`): the fake AniList at the real time, with its art, and for `watching` and
`playing` a history seeded first. Each launch starts the testing engine in a new directory of its
own. A debug build of either flavour says `ANISTREAM_READY` in the log, tag
`anistream`, once its screens have what they asked for.

The engine downloads torrents only, and no fixture has one, so the `downloads` extra seeds the
queue instead (`seed_fixture_downloads`): episode 6 of 900001 finished, its file the fixtures'
clip (`app/src/fixtures/assets/fixture-download.mkv`, ffmpeg's test pattern from
`scripts/fixture-clip.sh`) copied into the app's download folder, so it plays offline; episode 7
running; episode 3 of 900002 waiting. `shots.sh phone watching:downloads=1+page=downloads` shoots
the Downloads screen, and `page=settings/downloads` the storage meter.

### Checking downloads on a phone

The real transfer, the download job and Wi-Fi only need a torrent source, which only a phone with
its own VPN and index has:

1. Build and install the app flavour: `android/scripts/engine.sh release`, then
   `cd android && ./gradlew assembleAppDebug` and
   `adb install -r app/build/outputs/apk/app/debug/app-app-debug.apk`.
2. With the phone's VPN on, open Settings › Sources, turn on Torrent source, pick VPN on this
   device, enter the torrent index, and Turn on. Wait for Last check to say Passed.
3. On a title page, long-press an episode and pick Download. Downloads lists it as Queued, then
   running, with its speed and the time left.
4. The download notification is up: on Android 16 a Live Update at the top of the shade and the
   lock screen, a segment per episode and the time left in the status bar's chip; on Android 15 the
   plain progress notification. Pause (Pause all), Resume and Cancel act on the queue; a queue
   paused keeps the notification ten minutes for Resume. `adb shell dumpsys jobscheduler | grep -A4
   tv.anistream.app` shows the job, user-initiated, running.
5. Lock the phone for ten minutes, or switch to another app: the download goes on and the
   notification moves.
6. With Wi-Fi only on (Settings › Downloads), turn Wi-Fi off: the download waits. Turn it on: it
   goes on.
7. Once done, the row says Downloaded with its size, "Episode 8 downloaded" is posted with Play,
   and the job ends and its own notification goes. The
   file is under `/sdcard/Android/data/tv.anistream.app/files/Movies`
   (`adb shell ls -l /sdcard/Android/data/tv.anistream.app/files/Movies`).
8. In flight mode, play it from Downloads.

Launcher shortcuts and notifications open the activity on an app link, on an explicit intent, in
any build: `anistream://open/search`, `…/downloads`, `…/title/<id>`, `…/continue/<id>` (the title
page, then the episode Continue would play) and `…/play/<id>/<episode>`. The frame follows each
through the app's own navigation (`AppLinks.kt`). Search and Downloads are static shortcuts,
written into each variant's resources from `app/shortcuts.xml` with its package; "Continue
<title>" is published from the Continue queue as its first playable title changes.

```sh
adb shell am start -a android.intent.action.VIEW -d anistream://open/downloads -n tv.anistream.fixtures/tv.anistream.app.MainActivity
```

New-episode notifications come from a `WorkManager` job, `new-episodes`, about every six hours on
a network with the battery not low, while Settings › Notifications' New episodes switch (the
engine's `EpisodeNotifications`) is on; turning it on is the one time the app asks to post
notifications, and each show can turn its own off in its settings sheet. The job asks the engine
for what aired (`CheckNewEpisodes`), on the app's engine when it runs and otherwise on one started
for the job with every background switch off (`BackgroundEngine`), fetches each episode's cover, and
posts one notification per episode on the "New episodes" channel, grouped under a summary when
there are several, with Play (an app link) and Mark watched (the app's own receiver). Then it asks
for the widgets' record (`WidgetSnapshot`) and keeps it in `files/widgets/record.json`
(`WidgetRecord`), where the widgets read it. The fixtures' `notify` extra posts two, for a look at
the shade:

```sh
adb shell pm grant tv.anistream.fixtures android.permission.POST_NOTIFICATIONS
adb shell am start -n tv.anistream.fixtures/tv.anistream.app.MainActivity --es notify 1
```

## Tests

```sh
android/scripts/test.sh
```

The JVM unit tests: the store's reducers fed scripted event lists, the host and the sealed secrets
on a plain key, and, in the fixtures flavour, the store on the real testing engine through JNA. The
script runs them with HOME in a new temporary directory and tokens in files, and refuses to run
with HOME at the user's own; the Gradle test task sets its own HOME under `build/` too.

## Screenshots

The screens are drawn on the JVM, with no emulator, at each board's size:

```sh
cd android && ./gradlew :ui:recordRoborazziFixturesDebug
android/scripts/pairs.sh <boards> a2
```

`BoardShots` in `:ui` draws each board's screen with Robolectric's native graphics and captures it
with Roborazzi, into `android/.shots/roborazzi/`, which git ignores: the phone at 412 × 915 dp
and 420 dpi, the tablet at 1280 × 800 dp, each with a 24 dp status bar and gesture bar. The store
is the app's own, fed a scene the testing engine recorded, with no engine behind it: the clock
(`AppClock`) is held at the moment the scene was recorded, and art comes from the Rust test kit's
files as the fake AniList serves it, so a run draws the same pixels every time.
`verifyRoborazziFixturesDebug` compares a run with the last one recorded, and
`compareRoborazziFixturesDebug` writes the differences. CI records them on every run and keeps
them as the `android-screenshots` artifact.

`pairs.sh <boards> [phase]` sets each capture beside its board, in
`android/.shots/<phase>/pair-<board>.png`. `<boards>` is a directory of the boards rendered from
the design canvas as PNGs, named as the canvas names them (`AndroidHome.png`).

The scenes are in `android/testkit/src/main/resources/tv/anistream/testkit/scenes/`, as JSON:

```sh
android/scripts/scenes.sh
```

`scenes.sh` records them again: the store's `SceneRecorder` starts the testing engine on its
`fresh` and `watching` scenes, asks for every screen's lists, the search the shots run and each
fixture title's page and episodes, and writes every event it gets but art and the player's. Run it
when the engine's events or the fixtures change; a scene recorded before a field the bindings now
require fails to read, and says to.

The emulator is for what needs a device: the engine on a real phone image, sign-in, the player,
picture-in-picture and rotation.

```sh
android/scripts/shots.sh phone fresh fresh:seasonal app:seasonal
android/scripts/shots.sh tablet fresh
```

`shots.sh <phone|phone-37|tablet|fold> [shot…]` builds the APKs, starts the device's AVD headless
on a port of its own (5560 to 5566) unless it is running, installs, and for each shot clears the
app, starts it with the shot's extras, waits for `ANISTREAM_READY` and takes a screencap, with the
status bar in demo mode at 19:30. `ANISTREAM_EMULATOR_PORT` puts the emulator on another port, for a
second worktree's beside the first. A shot is `watching`, `fresh` or `app` (the app flavour on the
real AniList), then optionally `:` and a section or `key=value`s joined by `+`
(`fresh:title=900001`, `fresh:query=hoshi`, `fresh:watched=900001:1,2,3`). Shots go to
`android/.shots/a0/` (`ANISTREAM_SHOTS_PHASE` names another), which git ignores. Shots of the app
flavour show AniList's real catalogue, so they stay there and are never committed.

The phone is `anistream-phone-35-16k`, Android 15 with 16 KB pages; `ANISTREAM_KEEP_EMULATOR=1`
leaves an emulator the script started running for the next run.

## Versions

Every version is pinned in `gradle/libs.versions.toml`. Material3 is 1.5.0-beta01 rather than the
stable 1.4.0 the Compose BOM picks, because 1.4.0 keeps the Expressive APIs the design is built on
(`MaterialExpressiveTheme`, `MotionScheme`, the short navigation bar, the flexible top app bar,
connected button groups) internal. Move to 1.5.0 once it is stable.

The app is dark only, on the fixed Obi & Silk scheme; light follows the system once a light
palette is drawn. It has no backup (`allowBackup="false"`): the tokens are sealed with a key that
never leaves the device's keystore, so a restored copy could not open them.

## Third-party

Roboto Flex and Zen Kaku Gothic New are under the SIL Open Font License 1.1; `fonts.sh` fetches
them from github.com/google/fonts at a pinned commit by SHA-256, and their licences ship in the APK
under `assets/licences/`. JNA is used under the Apache License 2.0. The tests alone also use
Robolectric (MIT), Roborazzi, kotlinx.serialization and kotlin-reflect (Apache License 2.0), none
of which ships in the APK.
