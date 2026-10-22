---
title: The player
description: anistream’s controls, skip button, Up Next, menus and upscaling inside mpv, and the settings for each.
---

anistream plays everything in mpv and gives it the app's look: its own controls, seekbar
thumbnails, a skip button, an Up Next card and menus, in the same colours and font as the
terminal. One mpv window carries a whole binge. The next episode loads into the window the last
one played in, so fullscreen and window position stay put.

The controls appear when you move the mouse, pause or seek, and leave about a second after the
mouse stops, wherever it rests, or as soon as it leaves the window.
While they are up, the bottom of the picture is shaded so they read over a bright scene, and
subtitles move up out of their way and settle back when they go. When they are hidden the screen
shows the video and nothing else.

## Two interfaces

```toml
[playback]
player_ui = "anistream"   # or "mpv"
```

`anistream` is the default. It brings its own controls (uosc), thumbnails (thumbfast) and the
companion script that draws the skip button, Up Next and the menus. It still reads your
`mpv.conf`, but scripts in your mpv folder are not loaded, so another controller or track picker
cannot compete with these.

`mpv` keeps your own setup as it is and adds only the companion script. The skip button, Up Next
and the menus work with mpv's built-in controller and with your own uosc. In this mode the
anistream menu also appears in mpv's right-click menu.

Both need mpv 0.36 or newer for the skip button, Up Next, the menus and thumbnails. Older
versions, such as Debian 12's 0.35, play with mpv's own controls. `anistream --doctor` reports
your mpv version, which interface is in use and where its files were written.

## Keys inside the player

Keys are written the way the player’s `?` list writes them: `S` is the S key on its own, and
`Shift+N` is N with Shift held.

| Key | Action |
|---|---|
| `Shift+N` / `Shift+P` | Next · previous episode |
| `Shift+M`, right click | The anistream menu |
| `?` | Every key on this list, in the player window |
| `←` / `→` | Back · forward 5 seconds |
| `S` or `Enter` | Skip the opening or ending, while the button is shown |
| `U` | Undo an automatic skip, while the note is shown |
| `Enter` / `Esc` | Play now · cancel, while Up Next is shown |
| `Enter` / `Esc` | Switch source · keep waiting, when a stream stops mid-episode |
| Click the picture | Pause and resume. A double click is fullscreen, as in mpv |
| Mouse wheel | Volume |
| `Alt+M` | Mini player |
| `Alt+Z` | Sync subtitles |

Keys that belong to an element only exist while it is on screen. The rest of the time `S` takes
a screenshot and `Enter` and `Esc` do what mpv does. Every binding has a name, so a line like
`F2 script-binding anistream/menu` in your `input.conf` moves it, and the `?` list shows the key
you chose. mpv's own full list of bindings is `Shift+I`, then `4`.

## Skipping and Up Next

Opening and ending times come from the file's own chapters when it has them, and from aniskip
otherwise. Either way they are marked on the seekbar. While the playhead is inside one, a skip
button sits in the corner. With `skip_opening = true` the skip happens on its own and a short note
offers to undo it.

Near the end of an episode, from the start of its ending when that is known, an Up Next card shows
the next episode's still and title. With `auto_next = true` the next episode starts in the same
window when this one ends. Its streams are found while the ending plays, so it starts without a
search, and a torrent has its first pieces by then. Cancel keeps the player where it is.

## Menus

Press `Shift+M` or right click for the anistream menu:

- **Episodes** jumps to any episode of the season.
- **Sources** switches to another release of the same episode and keeps your place.
- **Sub / Dub** restarts the episode in the other version, where you were.
- **Upscaling**, **HDR tone mapping** (only while HDR video is playing) and **Night mode**.
- **Subtitles** and **Audio** list the file's tracks, by title, or by language when a track has none.
- **Sync subtitles** and **Mini player**.

## Upscaling

```toml
[playback]
upscaling = "natural"   # "natural", "sharp" or "off"
```

`natural` uses ArtCNN, small networks trained to reconstruct line work and texture without
restyling them. Standard-definition sources get the AniSD model. `sharp` uses Anime4K's presets,
which give crisper lines and heavier cleanup. The old `anime4k-fast` and `anime4k-quality`
settings read as `sharp`.

Which networks run is decided per file:

- Nothing heavy runs when the source already fills your display.
- On battery the chain starts one step lighter.
- If frames keep dropping, the chain steps down and stays there for the rest of the binge. A
  short note on the video says so.
- HDR video only gets chroma work, and Dolby Vision gets none, because the networks were trained
  on SDR frames.

If you pass your own `--glsl-shaders` in `playback.mpv_args`, anistream leaves shaders alone.

The shaders ship inside anistream. ArtCNN and Anime4K are MIT, KrigBilateral and SSimDownscaler
are LGPL-3.0, and the AniSD model is CC BY-NC 4.0, which anistream can ship because it is free
and non-commercial. Release packages carry every licence in a `third-party` folder.

## HDR

On a display that mpv drives in SDR, HDR video is tone mapped. `auto` is mpv's own choice and
right for most screens. The menu lists the alternatives while HDR is playing, and your pick is
kept as `tone_mapping`.

## Tracks

On releases with several subtitle tracks, anistream picks the full subtitles over signs and songs
when you watch subbed, and the signs track when you watch dubbed. Commentary is never chosen for
you.

When you change the audio track, the subtitle track or the subtitle delay, that choice is kept for
the series, separately for sub and dub, and matched on later episodes even when their track
numbers differ. This follows `persist_speed`, which also carries playback speed.

Sync subtitles (`Alt+Z`) fixes subtitles that drift: mark a line where it shows and where it should
show, twice, and anistream works out the delay and speed.

## Thumbnails

Hovering the seekbar shows a thumbnail of that moment. Torrent streams get thumbnails too. They
are read only from parts of the file already downloaded, so hovering never takes bandwidth from
playback, and parts not downloaded yet show no thumbnail.

## Night mode

Night mode evens out quiet dialogue and loud music, so a low volume stays usable without the
opening arriving at full blast. Turn it on in the menu or with `night_mode = true`.

## Trying it without a source

The `player_preview` example plays any local video as a two-episode binge through the real
playback code, with your config's player settings:

```sh
cargo run -p anistream --example player_preview -- path/to/video.mp4
```
