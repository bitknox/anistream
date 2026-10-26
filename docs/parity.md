# Platform parity

What each front end does, read from its code on 2026-10-24. The features page on the website
(`website/src/pages/features.astro`) shows 16 of these rows, as they stand once the parity PRs
(#163, #165, #168 to #173) are in, and names the rows every app has in one line above its table;
keep the two in step.

Where the code was read:

- Mac, iPhone, iPad, Android, Windows, Linux and Terminal: `train/2026-10-24-7` at `e835caa9`
  (PR #147), which is main at `767ef78a` (the Apple, Android and Windows audits and Search polish,
  PRs #136 to #142) plus the Mac fixes, the engine's dub extent and the Linux audit.
- Android TV: `fix/android-tv-audit` at `9b761d52` (PR #148), which adds joining plugin stores
  with a QR code and the TV's show settings. Marks that come only from that branch say so.
- Windows, read again on 2026-10-25: main at `c79ee13f` (train #207) with
  `fix/windows-parity-leftovers`, which adds the torrent dialog and live title language.

Marks:

- **yes**: built and reachable from the app's interface.
- **partial**: some of it, as the note says.
- **dev**: the platform could do it and the app does not yet. The site says "In development".
- **n/a**: the platform has no place for it. The site shows a dash.
- **no**: not built, and nothing planned. The site says what the app does instead.

Columns: Mac, iPhone, iPad, Android (phones, tablets and foldables, which have the same features),
Android TV, Windows, Linux, Terminal.

## Finding a show

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Search by name | yes | yes | yes | yes | yes | yes | yes | yes |
| Browse with filters, no query | yes | yes | yes | yes | yes (1) | yes | yes (2) | yes |
| Seasonal charts | yes | yes | yes | yes | yes | yes | yes | yes |
| Airing calendar | yes | yes | yes | yes | yes | yes | yes | yes |
| Seasons on the title page | yes | yes | yes | yes | yes | yes | yes | yes (3) |
| English or romaji titles | yes | yes | yes | yes | yes (4) | yes | yes | yes |
| How far the dub goes | dev (5) | dev (5) | dev (5) | dev (5) | dev (5) | yes | yes | yes |
| Continue | yes | yes | yes | yes | yes | yes | yes | yes |
| Library by status | yes | yes | yes | yes | yes | yes | yes | yes |

1. Sort, genre, year, season and format chips over the results, each a panel on the right; no
   status filter or All filters. `android/tv/.../screens/SearchScreen.kt`.
2. The Mac's row: genres wanted or left out, year, season once a year is chosen, formats,
   statuses and the sort, shown before typing, with chips and Clear All.
   `desktop/linux/src/pages/search_filters.rs`.
3. The franchise is a watch-order list (`Action::WatchOrder`).
4. Settings > Playback > Title language.
5. The engine answers `Command::LoadDub` with `Event::TitleDub`, and `present::dub_line` (also
   `words::dub_line` over FFI) words it as "Dub to episode 6" or "Dubbed". Only Windows
   (`Pages/TitlePage.xaml.cs`), Linux (`pages/title.rs`, under the meta line) and the terminal
   (the Episodes header, `screens/episodes.rs`) send it and draw it.

The calendar marks the native UX spec describes, "~" for an estimated air time and Sub or Dub on
a row, come from the engine's calendar `Entry` (`airing_approximate`, `airing_translation`,
`airing_dubbed`), worded by `present::air_time` and `present::calendar_mark`. Mac, iPhone and iPad
show them (`CalendarScreen.swift`, `PhoneScreens.swift`), and so does Windows, with Dub after the
episode, "Ep 10 · Dub · 16:30" (`Pages/CalendarPage.xaml.cs`). Only dubs are marked.

## Watching

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mpv | built in | built in | built in | built in | built in | built in | built in | external (1) |
| Upscaling, ArtCNN or Anime4K | yes | yes | yes | yes | n/a (2) | yes | yes | yes |
| Skip opening and ending | yes | yes | yes | yes | yes | yes | yes | yes |
| Opening and ending on the timeline | yes | yes | yes | yes | yes | yes | yes (3) | mpv's (4) |
| Up Next | yes | yes | yes | yes | yes | yes | yes | yes (5) |
| Resume | yes | yes | yes | yes | yes | yes | yes | yes |
| Pick a source by hand | yes | yes | yes | yes | yes | yes | yes | yes |
| Sub or dub | yes | yes | yes | yes | yes (6) | yes | yes (6) | yes |
| Subtitle and audio tracks | yes | yes | yes | yes | yes | yes | yes (7) | yes (8) |
| Media keys and now playing | yes | yes | yes | yes | yes | yes | yes (9) | mpv's |
| Picture in Picture | yes | yes | yes | yes | n/a | mini player | mini player | n/a |
| Watch parties | dev (10) | dev (10) | dev (10) | n/a | n/a | yes | dev (10) | yes |
| Discord presence | yes | n/a | n/a | n/a | n/a | yes | yes | yes |

1. The terminal app drives the mpv you install, over JSON IPC. The apps embed libmpv: Metal
   through MoltenVK on Apple, GLES into a SurfaceView on Android and TV, a d3d11 swap chain in a
   `SwapChainPanel` on Windows, GL in a `GtkGLArea` on Linux.
2. `android/tv/.../screens/SettingsScreen.kt`: the Playback rows leave out upscaling as not
   applying on a TV, which scales the picture itself.
3. Linux also draws the buffered range, from its own `demuxer-cache-time`
   (`desktop/linux/src/player/mod.rs`); no other app does.
4. mpv's own chapter marks, from the engine's `chapter-list`.
5. Next episode and auto-next; the countdown card is the apps' own.
6. The default Translation setting and the show settings' Audio row, with a Dub tag on releases;
   no switch during playback (Android phones have none either, only the tag). The TV's Audio row
   is on `fix/android-tv-audit`.
7. A Tracks menu (T) with the file's subtitle and audio tracks (`desktop/linux/src/player/mod.rs`).
8. Through anistream's menu inside mpv; the TUI has no track action of its own.
9. Apple has `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` (`PlayerSystem.swift`);
   Android and TV a media3 `MediaSession` (`player/PlayerService.kt`), and Windows the
   SystemMediaTransportControls and taskbar thumbnail buttons (`Player/MediaControls.cs`). Linux
   has MPRIS (`mpris.rs`).
10. The engine holds the party and drives the app's own player (`party` feature, on in these
    apps' builds, every Apple slice included), and the commands and events are in the bindings;
    no screen starts a party yet. The settings rows (on, server, room) are the terminal's: in an
    app the on switch does nothing, as apps start parties when asked. See
    `docs/party-protocol.md`.

The Mac's Subtitles and Audio menus, in the player and in the menu bar, also load a subtitle or
audio file for the episode playing (Load Subtitle File…, Load Audio File…), as Linux's do: mpv's
alone, not remembered for the show. iPhone and iPad have no such items.

On Mac, iPhone and iPad the seek line draws the engine's own opening and ending marks
(`Event::PlayerTimeline`, the chapters standing in until it has said) and the buffered ranges it
reads from mpv's `demuxer-cache-state` (`Event::PlayerBuffered`), in `PlayerParts.swift`.

Undo after an automatic skip: the Mac binds U and ⌘Z (`MacPlayerView.swift`), Windows U and
Ctrl+Z (`Player/PlayerView.xaml`), and the mpv companion binds u. Linux binds U and Ctrl+Z, with
Undo on its skip card. T opens the Tracks menu on Windows and Linux.

## Tracking and syncing

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| History with no account | yes | yes | yes | yes | yes | yes | yes | yes |
| AniList | yes | yes | yes | yes | yes | yes | yes | yes |
| MyAnimeList | yes | yes | yes | yes | no (1) | yes | yes | yes |
| Kitsu | yes | yes | yes | yes | yes | yes | yes | yes |
| Simkl, Trakt | yes | yes | yes | yes | yes | yes | yes | yes |
| Mark episodes watched | yes | yes | yes | yes | yes | yes | yes | yes |
| Per-show settings | yes | yes | yes | yes | yes (2) | yes | yes | partial (3) |

1. The TV says MyAnimeList "signs in from a paired anistream phone", but no phone sends it, and
   the remote protocol keeps sign-ins off the wire (`docs/remote-protocol.md`). AniList and Kitsu
   sign in on a page the TV serves on the network, opened from a QR code; Simkl and Trakt show a
   device code with "Get a new code" and no countdown. `android/tv/.../signin/SignInScreen.kt`.
2. More > Show settings (`ui/ShowSettings.kt`) has Audio, Subtitles (shown, chosen in the player),
   Skip opening, Skip filler, Spoilers, Source and Reset; not New episodes, since the TV posts no
   notifications.
3. A preferred source per title (`Action::PickProvider`); the TUI drops `Event::ShowSettings`.

The engine's `ShowSettingKey` also has Quality, DownloadQuality and KeepNext
(`anistream-model/src/show.rs`); no app's show settings offers them yet.

On Apple, Library, Sync now and conflict review are AniList's only; the other four trackers get a
sign-in and sign-out row. Simkl's and Trakt's device code counts down under the code ("Expires in
14:59", `present::device_code_expiry`) and, once it has run out, the sheet stays with Get a New
Code (`AccountsSheets.swift`).

## Downloads

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Download an episode | yes | yes | yes | yes | n/a (1) | yes | yes | yes |
| Keep the next episodes | yes | yes | yes | yes | n/a | yes | yes | config (2) |
| Downloads page | yes | yes | yes | yes | n/a | yes | yes | yes |

1. No download code in the TV app. Treated as not for the platform, since TV storage is small;
   revisit if that changes.
2. `[downloads] keep_next` in config.toml; the TUI does not draw the row.

On iPhone, downloads show as a Live Activity (`DownloadsActivityFeed.swift`, the widget bundle's
`DownloadsLiveActivity`), and so do the episode playing (`PlaybackActivityFeed.swift`, with Play
and Pause) and an episode of a followed show airing within the hour (`AiringActivityFeed.swift`,
a countdown, then "Out now"). All three are local, with no push; playback ranks first in the
Dynamic Island, then airing, then downloads.
On Mac and iPad a queued download has Start Now beside it, and on iPhone a Start now swipe
(`Command::DownloadStartNow`).

## On the TV

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Play on TV (send) | yes | yes | yes | yes | receives | dev | dev | dev (1) |
| Remote for the TV | yes | yes | yes | yes | pairs (2) | dev | dev | dev (1) |
| Arrow-key TV interface | n/a | n/a | n/a | n/a | yes | n/a | n/a | n/a |

1. `anistream-remote` exists but neither the binary nor the TUI depends on it.
2. QR code and a six-digit code under Settings > Remotes, a phone can ask to pair, and the TV
   advertises itself over mDNS when allowed. Play on TV arrives as the engine's `Command::Play`
   (docs/remote-protocol.md), not as a `FrontIntent`.

The paired-TV list (`TVsGroup`) is in the Mac's Integrations pane, the iPad's Playback pane and
the iPhone's settings sheet, so every Apple device can see and forget any TV it paired.

## Plugins and stores

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Add and order sources | yes | yes | yes | yes | partial (1) | yes | yes | yes |
| Join plugin stores | yes | yes | yes | yes | yes (2) | yes | yes | yes |
| Torrents with the VPN guard | yes | yes | yes | yes | dev | yes | yes (3) | yes (4) |

1. Settings > Sources lists them read-only; sources arrive only by joining a store.
2. On `fix/android-tv-audit`: `signin/JoinStoreScreen.kt` takes the invite through a QR code to
   a page the TV serves, or typed on the TV, previews the store and joins; Settings > Plugin
   stores lists them with Leave. A phone cannot send a store link to a paired TV: the remote
   protocol keeps stores off the wire, and its `text` command always lands in Search.
3. Turning Torrents on opens the setup dialog with "Protect with" and the connection check
   (`pages/torrents.rs`). A new or edited proxy is checked first and saved only once it passes;
   Edit Proxy sits beside "Protect with".
4. VPN mode is set in config.toml, not in the TUI.

## On the device

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Home screen widgets | yes | yes | yes | yes | n/a | dev | n/a | n/a |
| New-episode notifications | yes | yes | yes | yes | n/a (1) | yes | yes | config (2) |
| Handoff, Spotlight, Shortcuts | yes | yes | yes | n/a (3) | n/a | n/a | n/a | n/a |
| Updates from the app | yes | via SideStore | via SideStore | yes | yes (4) | yes | yes (5) | yes |
| Export lists and history | dev (6) | n/a | n/a | n/a | n/a | dev (6) | dev (6) | yes (7) |
| Command line with JSON | n/a | n/a | n/a | n/a | n/a | n/a | n/a | yes |

1. The TV app keeps the launcher's Watch Next row instead.
2. A digest of new episodes at launch (`airing_digest`), sent to the desktop with
   `[notifications] desktop`; there is no background check and the TUI does not draw the row.
3. Android has launcher shortcuts (Search, Downloads, Continue a title) and deep links, which
   are not counted here.
4. Settings > About checks the GitHub releases, downloads the TV's APK, checks its size and
   SHA-256 and hands it to Android's installer, which asks to confirm.
5. The AppImage downloads and checks the new version, puts it in its own place and restarts
   into it; in a folder it cannot write to, it saves the new version to Downloads and says so
   (`desktop/linux/src/updater.rs`).
6. The engine has no export command, and no app shows the item until it has one.
7. `anistream --export` (`crates/anistream/src/main.rs`).

## Other gaps worth knowing

- Windows is dark only; the Account item and Continue show AniList state only.
- Linux keeps tracker tokens in `0600` files under the data directory until the Secret portal is
  wired in, though a comment in `pages/settings.rs` says the secret store. There is no Flatpak
  manifest or AppStream metainfo yet, only `.desktop` files, one of them named
  `tv.anistream.desktop.desktop`. Linux has no subtitle sync.
- On TV, the reason an episode could not play is on screen only with `fix/android-tv-audit`,
  which carries its own copy of that change and will overlap with #147 when it merges.
- The terminal's Settings screen leaves out several shared rows (Up Next countdown, episode
  notifications, mpv options, VPN on failure, keep next); they work from config.toml.
