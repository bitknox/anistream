# Platform parity

What each front end does, read from its code on 2026-10-24. The features page on the website
(`website/src/pages/features.astro`) shows 18 of these rows, and names the rows every app has in
one line above its table; keep the two in step.

Where the code was read:

- Mac, iPhone, iPad, Android, Windows, Linux and Terminal: `train/2026-10-24-7` at `e835caa9`
  (PR #147), which is main at `767ef78a` (the Apple, Android and Windows audits and Search polish,
  PRs #136 to #142) plus the Mac fixes, the engine's dub extent and the Linux audit.
- Android TV: `fix/android-tv-audit` at `9b761d52` (PR #148), which adds joining plugin stores
  with a QR code and the TV's show settings. Marks that come only from that branch say so.

Marks:

- **yes**: built and reachable from the app's interface.
- **partial**: some of it, as the note says.
- **dev**: the platform could do it and the app does not yet. The site says "In development".
- **n/a**: the platform has no place for it. The site shows a dash.

Columns: Mac, iPhone, iPad, Android (phones, tablets and foldables, which have the same features),
Android TV, Windows, Linux, Terminal.

## Finding a show

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Search by name | yes | yes | yes | yes | yes | yes | yes | yes |
| Browse with filters, no query | yes | yes | yes | yes | dev (1) | dev (2) | yes (3) | yes |
| Seasonal charts | yes | yes | yes | yes | yes | yes | yes | yes |
| Airing calendar | yes | yes | yes | yes | yes | yes | yes | yes |
| Seasons on the title page | yes | yes | yes | yes | partial (4) | partial (4) | yes | yes (5) |
| English or romaji titles | yes | yes | yes | yes | dev (6) | yes | yes (7) | yes |
| How far the dub goes | dev (8) | dev (8) | dev (8) | dev (8) | dev (8) | dev (8) | yes | yes |
| Continue | yes | yes | yes | yes | yes | yes | yes | yes |
| Library by status | yes | yes | yes | yes | yes | yes | yes | yes |

1. Search is by name; Seasonal has season tabs and format and sort chips.
   `android/tv/.../screens/SearchScreen.kt`, `screens/ListScreens.kt`.
2. `SearchPage.xaml.cs` sends the query text only; its comment says the filters come later.
3. Genre, format, season, status and sort; no year filter. `desktop/linux/src/pages/search.rs`.
4. A Related row or list, with no season picker. TV `screens/TitleScreen.kt`, Windows
   `Pages/TitlePage.xaml.cs`.
5. The franchise is a watch-order list (`Action::WatchOrder`).
6. No Titles row; the TV's Settings leaves it out.
7. Linux does not handle `Event::TitlesChanged`, so pages already on screen keep the old names
   until they load again.
8. The engine answers `Command::LoadDub` with `Event::TitleDub`, and `present::dub_line` (also
   `words::dub_line` over FFI) words it as "Dub to episode 6" or "Dubbed". Only Linux
   (`pages/title.rs`, under the meta line) and the terminal (the Episodes header,
   `screens/episodes.rs`) send it and draw it.

The calendar marks the native UX spec describes, "~" for an estimated air time and Sub or Dub on
a row, are on no platform: the engine's calendar `Entry` (`anistream-model/src/catalog.rs`) has
`airing_in`, `airing_episode` and `delayed_from`, with no estimate flag and no translation.

## Watching

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mpv | built in | built in | built in | built in | built in | built in | built in | external (1) |
| Upscaling, ArtCNN or Anime4K | yes | yes | yes | yes | n/a (2) | yes | yes | yes |
| Skip opening and ending | yes | yes | yes | yes | yes | yes | yes | yes |
| Opening and ending on the timeline | yes | yes | yes | yes | yes | dev (3) | yes (4) | mpv's (5) |
| Up Next | yes | yes | yes | yes | yes | yes | yes | yes (6) |
| Resume | yes | yes | yes | yes | yes | yes | yes | yes |
| Pick a source by hand | yes | yes | yes | yes | yes | yes | yes | yes |
| Sub or dub | yes | yes | yes | yes | yes (7) | yes | yes (7) | yes |
| Subtitle and audio tracks | yes | yes | yes | yes | yes | dev (8) | dev (9) | yes (10) |
| Media keys and now playing | yes | yes | yes | yes | yes | dev (11) | dev (11) | mpv's |
| Picture in Picture | yes | yes | yes | yes | n/a | mini player | mini player | n/a |
| Syncplay watch parties | dev (12) | n/a | n/a | n/a | n/a | dev (12) | dev (12) | yes |
| Discord presence | yes | n/a | n/a | n/a | n/a | dev (13) | dev (13) | yes |

1. The terminal app drives the mpv you install, over JSON IPC. The apps embed libmpv: Metal
   through MoltenVK on Apple, GLES into a SurfaceView on Android and TV, a d3d11 swap chain in a
   `SwapChainPanel` on Windows, GL in a `GtkGLArea` on Linux.
2. `android/tv/.../screens/SettingsScreen.kt`: the Playback rows leave out upscaling as not
   applying on a TV, which scales the picture itself.
3. The timeline is a plain `Slider`. The engine writes the opening and ending into mpv's
   `chapter-list` (`anistream-playback/src/tracker.rs`) and publishes no event for them, and
   `FfiPlayerState` (`anistream-ffi/src/player.rs`) has no chapters or cache field.
4. Linux also draws the buffered range, from its own `demuxer-cache-time`
   (`desktop/linux/src/player/mod.rs`); no other app does.
5. mpv's own chapter marks, from the engine's `chapter-list`.
6. Next episode and auto-next; the countdown card is the apps' own.
7. The default Translation setting and the show settings' Audio row, with a Dub tag on releases;
   no switch during playback (Android phones have none either, only the tag). The TV's Audio row
   is on `fix/android-tv-audit`.
8. The Tracks menu has sub or dub only, not the file's own subtitle and audio tracks
   (`Player/PlayerView.xaml.cs`).
9. The player's menus are Source and Upscaling only (`desktop/linux/src/player/mod.rs`).
10. Through anistream's menu inside mpv; the TUI has no track action of its own.
11. Apple has `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` (`PlayerSystem.swift`);
    Android and TV a media3 `MediaSession` (`player/PlayerService.kt`). Windows has no
    SystemMediaTransportControls or taskbar thumbnail buttons; Linux has no MPRIS.
12. Settings rows only (on, server, room), with no way to start a party. Apple's "Play in
    Syncplay Party" stub is gone, and nothing sends `.playParty`.
13. Settings rows exist, but the engine is built without the `presence` feature for these apps.

Undo after an automatic skip: the Mac binds U and ⌘Z (`MacPlayerView.swift`) and the mpv
companion binds u. Windows and Linux bind neither, and Linux's skip toast has no Undo. No player
binds T for the tracks the desktop brief gives it.

## Tracking and syncing

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| History with no account | yes | yes | yes | yes | yes | yes | yes | yes |
| AniList | yes | yes | yes | yes | yes | yes | yes | yes |
| MyAnimeList | yes | yes | yes | yes | dev (1) | yes | dev | yes |
| Kitsu | yes | yes | yes | yes | yes | yes | dev | yes |
| Simkl, Trakt | yes | yes | yes | yes | yes | dev | dev | yes |
| Mark episodes watched | yes | yes | yes | yes | yes | yes | yes | yes |
| Per-show settings | yes | yes | yes | yes | partial (2) | dev (3) | yes | partial (4) |

1. The TV says MyAnimeList "signs in from a paired anistream phone", but no phone sends it, and
   the remote protocol keeps sign-ins off the wire (`docs/remote-protocol.md`). AniList and Kitsu
   sign in on a page the TV serves on the network, opened from a QR code; Simkl and Trakt show a
   device code with "Get a new code" and no countdown. `android/tv/.../signin/SignInScreen.kt`.
2. On `fix/android-tv-audit`: More > Show settings (`ui/ShowSettings.kt`) has Audio, Skip opening,
   Skip filler and Spoilers; not Subtitles, Source, New episodes or Reset, which phones have.
3. Settings shows "Overridden for N shows" but there is no editor.
4. A preferred source per title (`Action::PickProvider`); the TUI drops `Event::ShowSettings`.

The engine's `ShowSettingKey` also has Quality, DownloadQuality and KeepNext
(`anistream-model/src/show.rs`); no app's show settings offers them yet.

On Apple, Library, Sync now and conflict review are AniList's only; the other four trackers get a
sign-in and sign-out row.

## Downloads

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Download an episode | yes | yes | yes | yes | n/a (1) | dev (2) | yes | yes |
| Keep the next episodes | yes | yes | yes | yes | n/a | yes | yes | config (3) |
| Downloads page | yes | yes | yes | yes | n/a | yes | yes | yes |

1. No download code in the TV app. Treated as not for the platform, since TV storage is small;
   revisit if that changes.
2. `Command::DownloadEpisode` is sent only by Retry on the Downloads page; there is no download
   button on a title page.
3. `[downloads] keep_next` in config.toml; the TUI does not draw the row.

On iPhone, downloads show as a Live Activity (`DownloadsActivityFeed.swift`, the widget bundle's
`DownloadsLiveActivity`), and so do the episode playing (`PlaybackActivityFeed.swift`, with Play
and Pause) and an episode of a followed show airing within the hour (`AiringActivityFeed.swift`,
a countdown, then "Out now"). All three are local, with no push; playback ranks first in the
Dynamic Island, then airing, then downloads. A queued download has no "start now" on any app,
since the engine has no command for it.

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

On iPad the sender and the remote work, but the paired-TV list (`TVsGroup`) is in the Mac's
Integrations pane only and the iPhone's settings sheet, so an iPad can forget only the connected
TV, through the remote's "Pair again".

## Plugins and stores

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Add and order sources | yes | yes | yes | yes | partial (1) | yes | yes | yes |
| Join plugin stores | yes | yes | yes | yes | yes (2) | yes | yes | yes |
| Torrents with the VPN guard | yes | yes | yes | yes | dev | yes (3) | partial (4) | yes (5) |

1. Settings > Sources lists them read-only; sources arrive only by joining a store.
2. On `fix/android-tv-audit`: `signin/JoinStoreScreen.kt` takes the invite through a QR code to
   a page the TV serves, or typed on the TV, previews the store and joins; Settings > Plugin
   stores lists them with Leave. A phone cannot send a store link to a paired TV: the remote
   protocol keeps stores off the wire, and its `text` command always lands in Search.
3. A plain switch, without the confirm sheet Apple and Android show ("Protect with" and the IP
   check, `TorrentSheet.swift`, `TorrentSheet.kt`).
4. The Torrents switch is insensitive ("Set up from Providers in a later build",
   `pages/settings.rs`) and the plugin page offers only Disable, so torrents work only when
   config.toml already has them on.
5. VPN mode is set in config.toml, not in the TUI.

## On the device

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Home screen widgets | yes | yes | yes | yes | n/a | dev | n/a | n/a |
| New-episode notifications | yes | yes | yes | yes | n/a (1) | dev (2) | yes | config (3) |
| Handoff, Spotlight, Shortcuts | yes | yes | yes | n/a (4) | n/a | n/a | n/a | n/a |
| Updates from the app | yes | via SideStore | via SideStore | yes | dev (5) | dev (6) | dev (7) | yes |
| Export lists and history | dev (8) | n/a | n/a | n/a | n/a | dev (8) | dev (8) | yes (9) |
| Command line with JSON | n/a | n/a | n/a | n/a | n/a | n/a | n/a | yes |

1. The TV app keeps the launcher's Watch Next row instead.
2. `AppHost.Notify` raises an in-app InfoBar only, not a Windows notification.
3. A digest of new episodes at launch (`airing_digest`), sent to the desktop with
   `[notifications] desktop`; there is no background check and the TUI does not draw the row.
4. Android has launcher shortcuts (Search, Downloads, Continue a title) and deep links, which
   are not counted here.
5. About shows the version and licences only.
6. A Check for updates button asks the engine; there is no installer or self-update.
7. An Updates settings row only; nothing sends `Command::CheckForUpdates`.
8. The engine has no export command. Linux shows a disabled "Export Lists and History…"
   (`window.rs`); the Mac's More menu holds Sync Now only.
9. `anistream --export` (`crates/anistream/src/main.rs`).

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
