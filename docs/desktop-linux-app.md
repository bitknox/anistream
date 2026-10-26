# Linux desktop app: phases 1 and 2

Branch `desktop/linux-app`, October 2026, built on the foundations spike
to the design brief on `desktop/design-brief`.
GTK 4 and libadwaita in Rust, no web views. Dark only for now.

## What is in it

- The shell: an `AdwApplicationWindow` with the sidebar of the brief (Continue, Calendar,
  Seasonal, Search, Library, Downloads; Providers and Plugin Stores under Sources; the account
  row and Settings at the foot). It collapses into an overlay below the wide size.
- Continue: the queue (In progress, New this week) beside a preview of the selected title, and a
  setup card while nothing is connected. Enter plays, Delete hides, Ctrl+Shift+M marks watched.
- Seasonal: the season in the header's centre, the format in a toggle group, a poster grid.
- Search: typing anywhere starts it. The filters stand under the search bar from the start, so
  the titles can be browsed with nothing typed: genres wanted or left out, year, season, formats,
  statuses, and the sort at the end, with what is set as chips to take off one at a time or all.
  Recent searches, This Season and genres before anything is typed or chosen; results and
  matching settings after.
- A title's page: banner, cover, the one main action, list status, seasons, episode rows with
  watched checks and spoiler-blurred stills, details and related titles beside.
- The player: libmpv's GL render API in a `GtkGLArea`, native controls over the video (top bar,
  bottom bar with the timeline, volume, speed and settings), skip, up next and stall cards, and
  the brief's keys and mouse. Everything goes through the engine's hosted player protocol.
- Library: the tracker's five lists with counts, ordered from the header bar.
- Settings: a page in the sidebar with its panes, rows updated in place from the engine. Closing
  the window while downloads run asks whether to keep them going, as the brief says.

Phase 2 added:

- Calendar: the week as seven columns, an agenda on medium and narrow windows.
- Downloads: the engine's queue by title, with pause, resume and remove, Pause All, and the
  storage meter of the download folder's disk.
- Providers: the source order, dragged or moved from a row's menu, each source's standing and
  its page (hosts, version, update, disable, remove). Plugin Stores: the stores joined, each
  store's page with Install, Update and Replace, Leave, and Join Store with the invite checked
  as it is pasted. `--showcase` shows invented sources and stores in place of the fixture
  engine's own, for the shots.
- The player: openings and endings marked on the timeline from mpv's chapters, the buffered
  part, a seek preview from a second mpv while the pointer is over the timeline, subtitles
  raised above the controls while they show, and a mini player (P, or the top bar's button):
  the window itself at 480×270, undecorated, dragged to move, double-clicked to restore.
- Settings: at 1280 and wider a preview column on the pane's end, the Mac's: upscaling as the
  480p source beside the mode, a caption sample for subtitles, a note for the other panes.
  Below that, Upscaling opens a page of its own with the frame and the modes. A title's More
  menu opens its Show Settings dialog: audio, the subtitles remembered, skips, spoilers,
  new-episode notifications, the source it keeps and Reset to Defaults.
- Engine details: calendar rows show an estimated time as "~16:30" and a "Sub" or "Dub" mark;
  the timeline draws the engine's opening and ending marks, with the file's chapters standing in
  until it has said, and the buffered ranges it reports, ahead of the played part; the device
  code dialog counts down ("Expires in 14:59") and asks for a new code at nought; a queued
  download has a Start Now action.
- Recent searches kept between launches, the tracker's name on the account row when it gives
  no user name, and the app icon at every hicolor size from `assets/icon`.

Later added:

- A store owner's tools in the store page's menu, as the Mac has them: Invite Someone, Show
  Members (removing members, cancelling invites), Who Can Join (the store's open link and the
  devices that joined with it), Publish a Plugin and Open Admin Page; Link Another Device for a
  named member.
- An invite link pasted anywhere outside a text field opens Join Store with it, and so does an
  `anistream://join` link or a store invite opened with the app.
- Titles change language on every open page when the setting changes, without a reload.
- An AppImage in a folder the app cannot write to saves its update to Downloads and says so,
  with Show in Files.

The engine runs in-process on a tokio runtime of its own. The window only queues commands, and
the engine's events reach GTK's main loop over a channel, where `hub.rs` hands them out.

## Running it

All of it builds and runs in the Linux box from the spike:

```sh
export CARGO_TARGET_DIR=~/anistream-target-linux OUT=/tmp/anistream-linux-out
desktop/linux/scripts/docker.sh cargo test --manifest-path desktop/linux/Cargo.toml --features fixtures
desktop/linux/scripts/docker.sh desktop/linux/scripts/shots.sh
```

The build goes to a Docker volume named after `CARGO_TARGET_DIR`, not to that folder, so nothing
lands there. `desktop/linux/scripts/docker.sh --clean` deletes it, and `docker volume ls` lists
them. Only what a command writes to `OUT` comes back to the host.

`--features fixtures` builds in the test kit's scene: a fake AniList on loopback, made-up titles
and a watch history, under a data directory of its own. The app then takes `--fixtures`, and
`shots.sh` has it screenshot its own pages at the brief's three sizes into `OUT`. `SIZES` picks
the sizes (`wide medium narrow`) and `SHOTS` the wide run's shots.

## Not yet

- Night mode and the subtitle style have no picture of their own yet.
