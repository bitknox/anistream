# Platform parity

What each front end does, read from its code on 2026-10-24. The features page on the website
(`website/src/pages/features.astro`) shows the same table in short form; keep the two in step.

Where the code was read:

- Mac, iPhone, iPad, Android, Android TV: `train/2026-10-24-2` at `b9879d31` (main plus Play on
  TV, PR #125).
- Windows and Linux: `train/2026-10-24-3` at `fd3e311b` (PR #126).
- Terminal: main at `5763f428`.

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
| English or romaji titles | yes | yes | yes | yes | dev | yes | yes | yes (6) |
| Continue | yes | yes | yes | yes | yes | yes | yes | yes |
| Library by status | yes | yes | yes | yes | yes | yes | yes | yes |

1. Search is by name; Seasonal has format and sort chips. `android/tv/.../screens/SearchScreen.kt`.
2. `SearchPage.xaml.cs` sends `SearchQuery(Text:)` only; its comment says the filters come with the
   shared search query.
3. Genre, format, season and sort; no year filter. `desktop/linux/src/pages/search.rs`.
4. A Related row or list, with no season picker. TV `screens/TitleScreen.kt`, Windows
   `Pages/TitlePage.xaml.cs`.
5. The franchise is a watch-order list (`Action::WatchOrder`).
6. The `Titles` setting exists, but the TUI ignores `Event::ViewerTitleLanguage`
   (`anistream-tui/src/app/mod.rs`), so "AniList's choice" may not follow the account.

## Watching

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mpv | built in | built in | built in | built in | built in | built in | built in | external (1) |
| Upscaling, ArtCNN or Anime4K | yes | yes | yes | yes | n/a (2) | yes | yes | yes |
| Skip opening and ending | yes | yes | yes | yes | yes | yes | yes | yes |
| Up Next | yes | yes | yes | yes | yes | yes | yes | yes (3) |
| Resume | yes | yes | yes | yes | yes | yes | yes | yes |
| Pick a source by hand | yes | yes | yes | yes | yes | yes | yes | yes |
| Sub or dub | yes | yes | yes | yes | yes (4) | yes | yes | yes |
| Subtitle and audio tracks | yes | yes | yes | yes | yes | dev (5) | dev (5) | yes (6) |
| Picture in Picture | yes | yes | yes | yes | n/a | mini player | mini player | n/a |
| Syncplay watch parties | dev (7) | n/a | n/a | n/a | n/a | dev (8) | dev (8) | yes |
| Discord presence | yes | n/a | n/a | n/a | n/a | dev (9) | dev (9) | yes |

1. The terminal app drives the mpv you install, over JSON IPC. The apps embed libmpv: Metal
   through MoltenVK on Apple, GLES into a SurfaceView on Android and TV, a d3d11 swap chain in a
   `SwapChainPanel` on Windows, GL in a `GtkGLArea` on Linux.
2. `android/tv/.../screens/SettingsScreen.kt`: the Playback rows leave out upscaling as not
   applying on a TV, which scales the picture itself.
3. Next episode and auto-next; the countdown card is the apps' own.
4. A default Translation setting and a Dub tag on releases; no per-play switch.
5. The Tracks menu has sub or dub only, not the file's own subtitle and audio tracks.
6. Through anistream's menu inside mpv; the TUI has no track action of its own.
7. Settings > Integrations has the Syncplay rows, but "Play in Syncplay Party"
   (`Commands.swift`) is a disabled stub.
8. Settings rows only (on, server, room), with no way to start a party.
9. Settings rows exist, but the engine is built without the `presence` feature for these apps.

## Tracking and syncing

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| History with no account | yes | yes | yes | yes | yes | yes | yes | yes |
| AniList | yes | yes | yes | yes | yes | yes | yes | yes |
| MyAnimeList | yes | yes | yes | yes | yes (1) | yes | dev | yes |
| Kitsu | yes | yes | yes | yes | yes | yes | dev | yes |
| Simkl, Trakt | yes | yes | yes | yes | yes | dev | dev | yes |
| Mark episodes watched | yes | yes | yes | yes | yes | yes | yes | yes |
| Per-show settings | yes | yes | yes | yes | dev | dev (2) | yes | partial (3) |

1. From a paired phone only. AniList and Kitsu sign in on a page the TV serves on the network,
   opened from a QR code; Simkl and Trakt show a device code. `android/tv/.../signin/SignInScreen.kt`.
2. Settings shows "Overridden for N shows" but there is no editor.
3. A preferred source per title (`Action::PickProvider`); the TUI drops `Event::ShowSettings`.

On Apple, Library, Sync now and conflict review are AniList's only; the other four trackers get a
sign-in and sign-out row.

## Downloads

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Download an episode | dev (1) | dev (1) | dev (1) | yes | n/a (2) | dev (3) | dev (3) | yes |
| Keep the next episodes | yes | yes | yes | yes | n/a | yes | yes | config (4) |
| Downloads page | yes | yes | yes | yes | n/a | yes | yes | yes |

1. The episode menu's Download is `.disabled(true)` (`AnistreamUI/Menus.swift`), as is the Mac
   menu command. Downloads come from Keep next episodes, and play offline.
2. No download code in the TV app. Treated as not for the platform, since TV storage is small;
   revisit if that changes.
3. `Command::DownloadEpisode` is sent only by Retry on the Downloads page; there is no download
   button on a title page.
4. `[downloads] keep_next` in config.toml; the TUI does not draw the row.

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

On iPad the sender and the remote work, but the paired-TV list is in the Mac's Integrations pane
and the iPhone's settings sheet only, so an iPad may not be able to forget a TV from Settings.

## Plugins and stores

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Add and order sources | yes | yes | yes | yes | partial (1) | yes | yes | yes |
| Join plugin stores | yes | yes | yes | yes | dev (2) | yes | yes | yes |
| Torrents with the VPN guard | yes (3) | yes | yes | yes | dev | yes | yes | yes (4) |

1. Settings > Sources lists them read-only.
2. "Add a plugin store" is a disabled row that says "From a paired phone".
3. The guard's "Check Now" is a disabled stub; the guard checks every minute on its own.
4. VPN mode is set in config.toml, not in the TUI.

## On the device

| Feature | Mac | iPhone | iPad | Android | TV | Windows | Linux | Terminal |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Home screen widgets | yes | yes | yes | yes | n/a | dev | n/a | n/a |
| New-episode notifications | yes | yes | yes | yes | n/a (1) | dev (2) | yes | config (3) |
| Handoff, Spotlight, Shortcuts | yes | yes | yes | n/a (4) | n/a | n/a | n/a | n/a |
| Updates from the app | yes | via SideStore | via SideStore | yes | dev | dev (5) | dev (5) | yes |
| Command line with JSON | n/a | n/a | n/a | n/a | n/a | n/a | n/a | yes |

1. The TV app keeps the launcher's Watch Next row instead.
2. `AppHost.Notify` raises an in-app InfoBar only, not a Windows notification.
3. The engine notifies with `osascript` or `notify-send`; the TUI does not draw the row.
4. Android has launcher shortcuts (Search, Downloads, Continue a title) and deep links, which
   are not counted here.
5. A Check for updates button asks the engine; there is no installer or self-update in the app.

## Other gaps worth knowing

- Apple stubs: the episode Download, Downloads' "Start Now", the guard's "Check Now", "Play in
  Syncplay Party", and the Help menu items are disabled or do nothing.
- Windows is dark only; the Account item and Continue show AniList state only.
- Linux keeps tracker tokens in a file under the data directory until the Secret portal is wired
  in. There is no Flatpak manifest or AppStream metainfo yet, only a `.desktop` file.
- The terminal's Settings screen leaves out several shared rows (Up Next countdown, episode
  notifications, mpv options, VPN on failure, keep next); they work from config.toml.
