# Player screenshots

How the player screenshots in `assets/screenshots/` and `website/public/player/` are made, so
they can be made again when the player changes.

They are mpv's own captures (`screenshot-to-file … window`), with everything the player draws
over the picture, taken from a real playback through anistream's playback loop. A script poses
each one over mpv's IPC socket: it seeks, moves the pointer, opens menus with the keys a viewer
would press, and picks upscaling looks through the companion script.

The footage is こうしす！ by OPAP-JP contributors, licensed CC BY 4.0 and credited in
`assets/screenshots/NOTICE.md`: #3.1 for the player, and #3.2 for the upscaling comparison,
because it was released at 1080p and so has an original to hold the upscales against. Use
openly licensed footage only.

The comparison is a 480p stream at 1080p. That is where upscaling matters most, and where
anistream runs its strongest networks (AniSD and Anime4K's mode C). It needs drawn line art over
flat colour, which is where the looks differ: a soft, painterly frame shows almost nothing, even
from a 4K master, and a 720p source at 1440p shows much less than this.

## Making them

1. `tools/player-screens/footage.sh footage` downloads and prepares both episodes.
2. Serve the Up Next still: `python3 -m http.server 8765 --bind 127.0.0.1 --directory footage/www`.
3. Start the scenario, the `anistream` crate's `player_screens` example: the episode presented as
   part of the series, with the episode list from `scenario.json`, three sources and Up Next.

   ```sh
   cargo run -p anistream --example player_screens -- footage/kosys-3.1.mkv \
     --scenario footage/scenario.json --still http://127.0.0.1:8765/still-3.2.jpg
   ```

4. In another shell, `python3 tools/player-screens/shots.py hero controls skip menu episodes
   sources upscaling subtitles up-next loading`. The shots land in `out/` (`PLAYER_SCREENS_OUT`).
   `loading` plays to the end of the episode, so take it last. Quit mpv over its socket before
   stopping the example, or its window outlives it.
5. For the comparison, plug the computer in (on battery the chain starts one step lighter),
   run the scenario on `footage/kosys-3.2-480p-loop.mkv --start 1`, and then
   `python3 tools/player-screens/compare.py 4.25`. It sizes the window to the original's
   1920×1080 and refuses to keep a capture that is the same picture as the one before, which is
   what a paused player gives when it was not redrawn.
6. Encode the shots to WebP (`cwebp -q 86`). For the slider, crop the three captures and
   `footage/reference-1080.png` to her face (`crop=720:480:600:50`) and encode them with
   `cwebp -lossless`: lossy WebP halves the colour resolution, which hides part of what the
   looks change. For the README's strip, crop the same four to her left eye
   (`crop=440:330:555:110`) as `out/strip-{off,natural,sharp,original}.png`, render
   `strip.html` with a headless Chromium
   (`--headless --hide-scrollbars --allow-file-access-from-files --force-device-scale-factor=1
   --window-size=1888,420 --screenshot=out/strip.png`) and encode that losslessly too.

The player's window is 2560×1440 pixels: a 1280×720 window on a 2× display, the size the
controls are drawn for. Keep the subtitle files out of the video's folder, or mpv loads them a second time
beside the muxed tracks.
