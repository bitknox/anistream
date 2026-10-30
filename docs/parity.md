# Platform parity

What each front end does, read from its code, last on 2026-10-30. The features page on the website
(`website/src/pages/[...locale]/features.astro`) shows some of these rows and names the rows every
app has in one line above its table; keep the two in step.

Where the code was read:

- Mac, iPhone, iPad, Android, Windows, Linux and Terminal: `train/2026-10-24-7` at `e835caa9`
  (PR #147), which is main at `767ef78a` (the Apple, Android and Windows audits and Search polish,
  PRs #136 to #142) plus the Mac fixes, the engine's dub extent and the Linux audit.
- Android TV: `fix/android-tv-audit` at `9b761d52` (PR #148, since merged), which adds joining
  plugin stores with a QR code and the TV's show settings.
- Windows, read again on 2026-10-25: main at `c79ee13f` (train #207) with
  `fix/windows-parity-leftovers`, which adds the torrent dialog and live title language.
- Every app, read again on 2026-10-26: main at `5454bd52` (train #224), for the dub line, watch
  parties, Play on TV from Windows and Linux (#211, #214), torrents on the TV (#216) and the
  proxy check before saving (#212, #216, #220 to #222).
- Every app, read again on 2026-10-30: main at `0e93f3d6` (train #280) once the train #292 (Apple
  trailers and discussions, Android trailers, Watch again, calendar navigation and discussions,
  the Cast engine) and the pull requests for Up next for you (#289, #290) and the title page facts
  and More about (#291, #293) have merged. A code-read audit at `45c333fb` checked the cells.

Marks:

- **yes**: built and reachable from the app's interface.
- **partial**: some of it, as the note says.
- **dev**: the platform could do it and the app does not yet. The site says "In development".
- **n/a**: the platform has no place for it. The site shows a dash.
- **no**: not built, and nothing planned. The site says what the app does instead.

Columns: Mac, iPhone, iPad, Android (phones, tablets and foldables; the tablet layout differs where
a note says so), Android TV, Windows, Linux, Terminal.

On Android tablets (the phone app at 600 dp and wider) the title page has no dub line and no
trailer band (`ui/TitleScreen.kt`, `TabletTitle`), Continue has no hero and no pull to refresh
(`ui/ContinueTablet.kt`) and the player has no keyboard shortcuts.

## Finding a show

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Search by name | yes | yes | yes | yes | yes | yes | yes | yes |
| Browse with filters, no query | yes | yes | yes | yes | yes (1) | partial (6) | yes (2) | partial (7) |
| Seasonal charts | yes | yes | yes | yes | yes | yes | yes | partial (8) |
| Airing calendar | yes | yes | yes | yes | yes | yes | yes | yes |
| Calendar, earlier and later weeks | partial (9) | partial (9) | partial (9) | yes | yes | yes | yes | yes |
| Seasons on the title page | yes | yes | yes | yes | yes | yes | yes | yes (3) |
| English or romaji titles | yes | yes | yes | yes | yes (4) | yes | yes | yes |
| How far the dub goes | yes (5) | yes (5) | yes (5) | yes (5) | yes (5) | yes (5) | yes (5) | yes (5) |
| Continue | yes | yes | yes | yes | yes | yes | yes | yes |
| Up next for you | dev | dev | dev | dev | dev | yes (10) | yes (10) | partial (11) |
| Library by status | yes | yes | yes | yes | yes | partial (12) | yes | yes |

1. Sort, genre, year, season and format chips over the results, each a panel on the right; no
   status filter or All filters. `android/tv/.../screens/SearchScreen.kt`.
2. The Mac's row: genres wanted or left out, year, season once a year is chosen, formats,
   statuses and the sort, shown before typing, with chips and Clear All.
   `desktop/linux/src/pages/search_filters.rs`.
3. The franchise is a watch-order list (`Action::WatchOrder`).
4. Settings > Playback > Title language.
5. The engine answers `Command::LoadDub` with `Event::TitleDub`, and `present::dub_line` (also
   `words::dub_line` over FFI) words it as "Dub to episode 6" or "Dubbed". Every app sends it
   and draws it on the title page: Apple in `TitlePage.swift` and `PhoneTitle.swift`, Android in
   `ui/TitleScreen.kt`, the TV in `tv/screens/TitleScreen.kt`, Windows in
   `Pages/TitlePage.xaml.cs`, Linux in `pages/title.rs` (under the meta line) and the terminal in
   the Episodes header (`screens/episodes.rs`). Android tablets do not draw it.
6. Windows shows the first page of results only (`SearchMore` is never sent), and has no genre
   exclusion, no status filter, no live search and no recent searches (`App.Core/Searches.cs`).
7. Six single-value rows, no exclusion and no Clear all.
8. The current season only.
9. Mac, iPhone and iPad load the first window and no more (`LoadCalendarAhead` is not sent), the
   iPhone cannot page by week, and the Mac and iPad have no refresh (`CalendarScreen.swift`,
   `PhoneScreens.swift`). The other apps page earlier and later broadcasts in as the end comes
   near (`LoadCalendarPast`, `LoadCalendarAhead`): Android and the TV in `ui/CalendarScreen.kt` and
   `tv/screens/CalendarScreen.kt`, Windows in `App.Core/CalendarReach.cs`, Linux in
   `pages/calendar.rs` and the terminal in `app/lists.rs`.
10. A group under the queue (above it on Linux) of new seasons, titles on the Planning list and
    recommendations, each row saying why it is there, with Not interested and an Undo notice
    (`Command::LoadUpNext`, `DismissUpNext`, `RestoreUpNext`). A new install with nothing to
    continue shows what airs today and this season. Windows in `Pages/ContinuePage.xaml.cs` and
    `App.Core/UpNext.cs`, Linux in `pages/continue_page.rs`.
11. The terminal loads and dismisses picks (`app/lists.rs`) and cannot restore one.
12. The library needs AniList, which cannot sign in on Windows yet; see the note under Tracking.

The calendar marks the native UX spec describes, "~" for an estimated air time and Sub or Dub on
a row, come from the engine's calendar `Entry` (`airing_approximate`, `airing_translation`,
`airing_dubbed`), worded by `present::air_time` and `present::calendar_mark`. Every app shows
them: Mac, iPhone and iPad (`CalendarScreen.swift`, `PhoneScreens.swift`), Android and the TV
(`ui/CalendarScreen.kt`, `tv/screens/CalendarScreen.kt`), Windows, with Dub after the episode,
"Ep 10 · Dub · 16:30" (`Pages/CalendarPage.xaml.cs`), Linux (`pages/calendar.rs`) and the
terminal (`screens/lists.rs`). Only dubs are marked.

## On the title page

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Trailers | yes (1) | yes (1) | yes (1) | yes (1) | yes (1) | yes (1) | yes (1) | yes (1) |
| Title facts | yes | yes | yes | yes | partial (2) | yes (3) | yes (3) | partial (4) |
| More about | yes | yes | yes | yes | dev | yes (3) | yes (3) | partial (5) |
| Episode discussions | yes (6) | yes (6) | yes (6) | yes (6) | partial (7) | yes (6) | yes (6) | no |
| Watch again | dev | dev | dev | yes (8) | yes (8) | yes (8) | yes (8) | no |
| Score | dev | dev | dev | dev | dev | dev | dev | yes |
| Fix a wrong match | dev | dev | dev | dev | dev | dev | dev | yes (9) |

1. The trailer plays muted behind the banner where the engine allows it, with a sound button, and
   the Trailer button opens it in its own window or sheet. Settings has Play trailers (Always,
   Wi-Fi only, Off) on Apple and Android. Resting the pointer on a cover opens a preview with the
   trailer on Mac, Windows and Linux. The iPhone and iPad have the band and the sheet; Android
   tablets keep the title head without a band. The TV has a Trailer button that opens YouTube's
   player full screen, and nothing autoplays there. The terminal opens it with `v`.
2. The TV's title page has no dates or airing status.
3. How the run reads (aired with its dates, airing since, starts, on hiatus, cancelled), the
   studio in the metadata line, a film's length, what it is based on, the country when made
   outside Japan and the genres, plus a More about this show dialog or sheet with the people you
   follow, where it streams, rankings, cast, staff, more like this and tags behind a spoiler
   count. Windows in `Pages/TitlePage.xaml.cs`, Linux in `pages/title.rs`.
4. Format, episodes, year, score and studio.
5. Recommendations and the watch order only.
6. Read and post, edit, delete, like and show more, from the episode's menu and from the player
   once an episode is 85% watched. Apple draws it as a sheet, an inspector or a window, Windows
   as a trailing pane, Linux in `pages/discussion.rs`. Android has no player entry, since its
   player has no More menu. Settings has switches for Discussions and Comment images.
7. The TV reads a thread in a panel and points to the phone for replying.
8. The main button says Watch again and plays episode 1 when every episode is watched.
9. Press `m` on a title: manual search, pick and clear.

## Watching

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mpv | built in | built in | built in | built in | built in | built in | built in | external (1) |
| Upscaling, ArtCNN or Anime4K | yes | yes | yes | yes | no (2) | yes | yes | yes |
| Skip opening and ending | yes | yes | yes | yes | yes | yes | yes | yes |
| Opening and ending on the timeline | yes | yes | yes | yes | yes | yes | yes | mpv's (4) |
| Buffered range on the timeline | yes (3) | yes (3) | yes (3) | yes (3) | yes (3) | yes (3) | yes (3) | yes (3) |
| Up next | yes | yes | yes | yes | yes | yes | yes | yes (5) |
| Resume | yes | yes | yes | yes | yes | yes | yes | yes |
| Pick a source by hand | yes | yes | yes | yes | yes | yes | yes | yes |
| Sub or dub | yes (6) | yes (6) | yes (6) | yes (6) | yes (6) | yes (6) | yes (6) | yes (6) |
| Switch sub or dub while playing | dev | dev | dev | dev | dev | yes (11) | yes (11) | yes (11) |
| Subtitle and audio tracks | yes | yes | yes | yes | yes | yes | yes (7) | yes (8) |
| Media keys and now playing | yes | yes | yes | yes | yes | yes | yes (9) | mpv's |
| Picture in Picture | yes | yes | yes | yes | no (12) | mini player | mini player | n/a |
| Watch parties | yes (10) | yes (10) | yes (10) | dev (10) | no (10) | yes (10) | yes (10) | partial (13) |
| Discord presence | yes | n/a | n/a | n/a | n/a | yes | yes | yes |
| Stats overlay, Copy stats | yes | yes | yes | dev (14) | yes | yes | yes | partial (15) |
| Still watching? | yes | yes | yes | yes | yes | yes | dev (16) | no |

1. The terminal app drives the mpv you install, over JSON IPC. The apps embed libmpv: Metal
   through MoltenVK on Apple, GLES into a SurfaceView on Android and TV, a d3d11 swap chain in a
   `SwapChainPanel` on Windows, GL in a `GtkGLArea` on Linux.
2. `android/tv/.../screens/SettingsScreen.kt`: the Playback rows leave out upscaling by
   decision, since a TV scales the picture itself.
3. The seek line draws the ranges the engine reads from mpv's `demuxer-cache-state`
   (`Event::PlayerBuffered`): in `PlayerParts.swift` on Apple, `player/PlayerModel.kt` on Android
   and the TV, `Player/PlayerView.xaml.cs` on Windows, `player/mod.rs` on Linux and
   `app/mod.rs` in the terminal.
4. mpv's own chapter marks, from the engine's `chapter-list`.
5. Next episode and auto-next; the countdown card is the apps' own.
6. The default Translation setting and the show settings' Audio row, with a Dub tag on releases.
7. A Tracks menu (T) with the file's subtitle and audio tracks (`desktop/linux/src/player/mod.rs`).
8. Through anistream's menu inside mpv; the TUI has no track action of its own.
9. Apple has `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` (`PlayerSystem.swift`);
   Android and TV a media3 `MediaSession` (`player/PlayerService.kt`), and Windows the
   SystemMediaTransportControls and taskbar thumbnail buttons (`Player/MediaControls.cs`). Linux
   has MPRIS (`mpris.rs`).
10. The engine holds the party and drives the app's own player (`party` feature, on in these
    apps' builds, every Apple slice included). Each app has its own screens to start one, join
    from an invite link and pick a source for the party's episode, and a party panel in the
    player: Apple in `AnistreamUI/Party.swift` and `AnistreamPlayerUI/PartyPanel.swift` (the
    Watch party… item in the Mac player), Windows in `Pages/PartyDialog.cs` and
    `Player/PartyPanel.cs`, Linux in `desktop/linux/src/party/`. The Android engine is built
    without the `party` feature (`android/scripts/engine.sh`), so Android has none yet; the TV
    leaves parties out by decision. The settings rows (on, server, room) are the terminal's:
    in an app the on switch does nothing, as apps start parties when asked. See
    `docs/party-protocol.md`.
11. The engine sends the Translation choice in the player's menu (`PlayerChoice::Translation`);
    Windows, Linux and the terminal show it (`desktop/linux/src/player/mod.rs`). Apple's track
    menu (`PlayerParts.swift`), Android's sheet (`pui/PlayerSheets.kt`) and the TV's
    (`tv/player/TvPlayerScreen.kt`) do not.
12. Android TV supports Picture in Picture; the app leaves it out by decision.
13. The terminal starts a party from its settings rows and `y`; it cannot join from a link or show
    a panel.
14. The model keeps the stats summary (`statsSummary`) and no screen draws it.
15. mpv's own stats page.
16. `still_watching_after` is ignored (`desktop/linux/src/player/mod.rs`). The terminal does
    not ask, since the engine asks only in a hosted session.

The Mac's Subtitles and Audio menus, in the player and in the menu bar, also load a subtitle or
audio file for the episode playing (Load Subtitle File…, Load Audio File…), as Linux's and
Windows' do (`Player/PlayerView.xaml.cs`): mpv's alone, not remembered for the show. iPhone, iPad,
Android and the TV have no such items.

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
| AniList | yes | yes | yes | yes | yes | partial (4) | yes | partial (5) |
| MyAnimeList | yes | yes | yes | yes | no (1) | partial (4) | yes | partial (5) |
| Kitsu | yes | yes | yes | yes | yes | partial (4) | yes | partial (5) |
| Simkl, Trakt | yes | yes | yes | yes | yes | yes | yes | partial (5) |
| Mark episodes watched | yes | yes | yes | yes | yes | yes | yes | yes |
| Per-show settings | partial (6) | partial (6) | partial (6) | partial (6) | partial (2) | partial (6) | partial (6) | partial (3) |
| Conflict review | yes | yes | yes | yes | dev | dev | dev | partial (7) |

1. The TV's Accounts leaves MyAnimeList out: its redirect must be the one address it has
   registered, which a TV cannot finish (`signin/SignInScreen.kt`). AniList and Kitsu
   sign in on a page the TV serves on the network, opened from a QR code; Simkl and Trakt show a
   device code with a countdown and "Get a new code". `android/tv/.../signin/SignInScreen.kt`.
2. More > Show settings (`ui/ShowSettings.kt`) has Audio, Subtitles (shown, chosen in the player),
   Skip opening, Skip filler, Spoilers, Source and Reset; not New episodes, since the TV posts no
   notifications.
3. A preferred source per title (`Action::PickProvider`); the TUI drops `Event::ShowSettings`.
4. Sign-in can't finish in the Windows release build yet; a fix is in progress. The engine there
   is built without the `desktop` feature, so AniList and MyAnimeList are refused, and Kitsu has no
   credential prompt. The library, the conflicts and the Account item all depend on AniList.
5. The terminal lists a tracker only once it is enabled in config.toml, and MyAnimeList also needs
   a client id there.
6. Audio, Skip opening, Skip filler, Spoilers, New episodes, Source and Reset on every app, with
   Subtitles shown and chosen in the player (no app sends `SetShowSubtitles`). Windows and Linux
   add Quality, Download quality and Keep next.
7. The terminal can resolve a conflict by keeping the local entry only. Windows, Linux and the TV
   have no review; the TV also has no Sync now.

The engine's `ShowSettingKey` also has Quality, DownloadQuality and KeepNext
(`anistream-model/src/show.rs`). Windows (`Pages/ShowSettingsDialog.cs`) and Linux
(`pages/show_settings.rs`) offer them; Apple, Android and the TV do not.

On Apple, Library, Sync now and conflict review are AniList's only; the other four trackers get a
sign-in and sign-out row. Simkl's and Trakt's device code counts down under the code ("Expires in
14:59", `present::device_code_expiry`) and, once it has run out, the sheet stays with Get a New
Code (`AccountsSheets.swift`).

## Downloads

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Download an episode | yes | yes | yes | yes | no (1) | yes | yes | yes |
| Keep the next episodes | yes | yes | yes | yes | no | yes | yes | config (2) |
| Downloads page | yes | partial (3) | yes | yes | no | yes | yes | yes |

1. No download code in the TV app, by decision, since TV storage is small; revisit if that
   changes.
2. `[downloads] keep_next` in config.toml; the TUI does not draw the row.
3. The iPhone's page has no Clear finished and no seeding list (`PhoneSettings.swift`).

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
| Play on TV (send) | yes | yes | yes | yes | receives | yes | yes | dev (1) |
| Remote for the TV | yes | yes | yes | yes | pairs (2) | yes | yes | dev (1) |
| Chromecast (send) | dev (3) | dev (3) | dev (3) | dev (3) | n/a | dev (3) | dev (3) | no |
| Arrow-key TV interface | n/a | n/a | n/a | n/a | yes | n/a | n/a | n/a |

1. `anistream-remote` exists but neither the binary nor the TUI depends on it.
2. QR code and a six-digit code under Settings > Remotes, a phone can ask to pair, and the TV
   advertises itself over mDNS when allowed. Play on TV arrives as the engine's `Command::Play`
   (docs/remote-protocol.md), not as a `FrontIntent`.
3. The engine can play on a TV with Chromecast built in (`anistream-cast`, behind the `cast`
   feature, `Command::CastTo` and `StopCasting`). No app sends it yet, so no app has the feature.

The paired-TV list (`TVsGroup`) is in the Mac's Integrations pane, the iPad's Playback pane and
the iPhone's settings sheet, so every Apple device can see and forget any TV it paired. On Linux
it is in Settings > Integrations (`src/tv/paired.rs`), and Avahi finds TVs over D-Bus
(`src/tv/avahi.rs`); with no Avahi running, a typed address still reaches a TV. Windows lists them
in Settings > Integrations, finds TVs with Windows' own DNS-SD (a device watcher on
`_anistream._tcp`), and hands the player's episode over from the player's top bar.

## Plugins and stores

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Add and order sources | yes | yes | yes | yes | partial (1) | yes | yes | yes |
| Join plugin stores | yes | yes | yes | yes | yes (2) | yes | yes | yes |
| Torrents with the VPN guard | yes (3) | yes (3) | yes (3) | yes (3) | yes (5) | yes (3) | yes (3) | partial (4) |
| Check the guard again | dev (6) | dev (6) | dev (6) | partial (6) | partial (6) | yes (6) | yes (6) | no |

1. Settings > Sources lists them read-only; sources arrive only by joining a store.
2. `signin/JoinStoreScreen.kt` takes the invite through a QR code to
   a page the TV serves, or typed on the TV, previews the store and joins; Settings > Plugin
   stores lists them with Leave. A phone cannot send a store link to a paired TV: the remote
   protocol keeps stores off the wire, and its `text` command always lands in Search.
3. Settings > Sources: turning Torrents on asks what protects them ("Protect with") and runs
   the connection check. A new or changed proxy goes through `Command::CheckTorrentProtection`
   first and is saved only once it passes; a failed check saves nothing and says why. Once a
   proxy is set, Edit proxy sits beside "Protect with" and opens its form filled in, where an
   empty password keeps the saved one. Apple in `TorrentProtection.swift` and `TorrentSheet.swift`,
   Android in `ui/TorrentSheet.kt`, Windows in `Pages/TorrentDialog.cs` and
   `App.Core/Torrents.cs`, Linux in `pages/torrents.rs`.
4. VPN mode is set in config.toml, not in the TUI.
5. Settings > Sources has the phone's Torrents group (`tv/screens/SettingsScreen.kt`). The index
   and a proxy's details come from a page the TV serves, opened on a phone from a QR code, or are
   typed on the TV (`signin/TorrentScreen.kt`); the proxy is checked before it is saved, as in
   note 3. Downloads stay off on the TV.
6. Check now on Android and the TV sends `Command::CheckProviders`, which re-checks plugins and
   leaves the guard alone (`ui/SettingsPanes.kt`, `tv/screens/SettingsScreen.kt`). Windows and
   Linux send `Command::CheckConnection`. Apple has no Check now.

## On the device

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Home screen widgets | yes | yes | yes | yes | n/a | dev | n/a | n/a |
| New-episode notifications | yes | yes | yes | yes | n/a (1) | yes | partial (12) | config (2) |
| Coming up | yes (8) | yes (8) | yes (8) | yes (8) | n/a | dev | dev | no |
| Handoff, Spotlight, Shortcuts | yes | yes | yes | partial (3) | n/a | dev (9) | dev (9) | n/a |
| Updates from the app | yes | via SideStore | via SideStore | yes | yes (4) | yes | yes (5) | partial (10) |
| Export lists and history | dev (6) | n/a | n/a | n/a | n/a | dev (6) | dev (6) | yes (7) |
| Command line with JSON | n/a | n/a | n/a | n/a | n/a | n/a | n/a | yes |
| Bug report | yes (11) | yes (11) | yes (11) | yes (11) | dev | dev | dev | yes |
| High contrast | yes | yes | yes | dev | dev | dev | yes | no |
| System caption style | yes | yes | yes | yes | yes | dev | dev | mpv's |

1. The TV app keeps the launcher's Watch Next row instead.
2. A digest of new episodes at launch (`airing_digest`), sent to the desktop with
   `[notifications] desktop`; there is no background check and the TUI does not draw the row.
3. Android has launcher shortcuts (Search, Downloads, Continue a title; `app/Shortcuts.kt`) and
   deep links, but no Handoff or system search.
4. Settings > About checks the GitHub releases, downloads the TV's APK, checks its size and
   SHA-256 and hands it to Android's installer, which asks to confirm.
5. The AppImage downloads and checks the new version, puts it in its own place and restarts
   into it; in a folder it cannot write to, it saves the new version to Downloads and says so
   (`desktop/linux/src/updater.rs`).
6. The engine has no export command, and no app shows the item until it has one.
7. `anistream --export` (`crates/anistream/src/main.rs`).
8. Notices for episodes of followed shows about to air (`Command::UpcomingNotices`), with a row
   in Settings > Notifications on Apple and Android.
9. Windows could have a jump list, and Linux `.desktop` actions and a GNOME search provider;
   neither has them yet.
10. `anistream --update` checks and installs; the TUI has no row.
11. Report a problem from About and from error notices. On Android it also appears after the
    player closes.
12. Only the engine's digest at launch, worded in English by the engine
    (`crates/anistream-engine/src/background/digest.rs`); no timer checks while the app runs.
    Windows checks on a timer.

## Other gaps worth knowing

- Windows is dark only; the Account item and Continue show AniList state only, and AniList,
  MyAnimeList and Kitsu sign-in cannot finish in the release build yet.
- Linux keeps tracker tokens in `0600` files under the data directory until the Secret portal is
  wired in, though a comment in `pages/settings.rs` says the secret store. There is no Flatpak
  manifest or AppStream metainfo yet, only `.desktop` files, one of them named
  `tv.anistream.desktop.desktop`. Linux has no subtitle sync.
- The terminal's Settings screen leaves out the rows the engine lists as the native apps' own
  (`SettingKey::NATIVE_ROWS`): the Up next countdown, spoiler protection, download quality, keep
  next episodes, downloads and torrents on mobile data, VPN on failure, episode notifications, mpv
  options, adult titles, the discussions and comment images rows, the controller rows and
  Calendar, mine only. Most work from config.toml.
