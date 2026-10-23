# Linux desktop app: phase 1

Branch `desktop/linux-app`, October 2026, built on the foundations spike
([desktop-linux-spike.md](desktop-linux-spike.md)) to the design brief on `desktop/design-brief`.
GTK 4 and libadwaita in Rust, no web views. Dark only for now.

## What is in it

- The shell: an `AdwApplicationWindow` with the sidebar of the brief (Continue, Calendar,
  Seasonal, Search, Library, Downloads; Providers and Plugin Stores under Sources; the account
  row and Settings at the foot). It collapses into an overlay below the wide size.
- Continue: the queue (In progress, New this week) beside a preview of the selected title, and a
  setup card while nothing is connected. Enter plays, Delete hides, Ctrl+Shift+M marks watched.
- Seasonal: the season in the header's centre, the format in a toggle group, a poster grid.
- Search: typing anywhere starts it. Recent searches, This Season and genres before typing;
  filters, results and matching settings after.
- A title's page: banner, cover, the one main action, list status, seasons, episode rows with
  watched checks and spoiler-blurred stills, details and related titles beside.
- The player: libmpv's GL render API in a `GtkGLArea`, native controls over the video (top bar,
  bottom bar with the timeline, volume, speed and settings), skip, up next and stall cards, and
  the brief's keys and mouse. Everything goes through the engine's hosted player protocol.
- Library: the tracker's five lists with counts, ordered from the header bar.
- Settings: a page in the sidebar with its panes, rows updated in place from the engine. Closing
  the window while downloads run asks whether to keep them going, as the brief says.

The engine runs in-process on a tokio runtime of its own. The window only queues commands, and
the engine's events reach GTK's main loop over a channel, where `hub.rs` hands them out.

## Running it

All of it builds and runs in the Linux box from the spike:

```sh
export CARGO_TARGET_DIR=~/anistream-target-linux OUT=/tmp/anistream-linux-out
desktop/linux/scripts/docker.sh cargo test --manifest-path desktop/linux/Cargo.toml --features fixtures
desktop/linux/scripts/docker.sh desktop/linux/scripts/shots.sh
```

`--features fixtures` builds in the test kit's scene: a fake AniList on loopback, made-up titles
and a watch history, under a data directory of its own. The app then takes `--fixtures`, and
`shots.sh` has it screenshot its own pages at the brief's three sizes into `OUT`. `SIZES` picks
the sizes (`wide medium narrow`) and `SHOTS` the wide run's shots.

## Not yet

- Calendar, Downloads, Providers and Plugin Stores are placeholders.
- The timeline has no segment marks or seek preview, subtitles do not rise over the bar, and
  there is no mini player.
- Settings has no preview column, Show Settings and the torrent switch are off, and recent
  searches are kept only for the session.
- The account row says "Signed in" when the tracker gives no name.
