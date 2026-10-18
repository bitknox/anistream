# Player screenshots

How the player screenshots in `assets/screenshots/` and `website/public/player/` are made, so
they can be made again when the player changes.

They are mpv's own captures (`screenshot-to-file … window`), with everything the player draws
over the picture, taken from a real playback through anistream's playback loop. A script poses
each one over mpv's IPC socket: it seeks, moves the pointer, opens menus with the keys a viewer
would press, and picks upscaling looks through the companion script.

The footage is openly licensed, CC BY 4.0, and credited in `assets/screenshots/NOTICE.md`:
こうしす！ #3.1 by OPAP-JP contributors for the player, and Sol Levante by Netflix and
Production I.G for the upscaling comparison, whose 4K master gives the comparison a true
original. Use openly licensed footage only.

## Making them

1. `tools/player-screens/footage.sh footage` downloads and prepares both. Sol Levante's master
   is read over HTTP ranges; only eight seconds of it are fetched.
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
5. For the comparison, plug the computer in (on battery the chain starts lighter), run the
   scenario on `footage/sol-720p-loop.mkv --start 1`, and then
   `python3 tools/player-screens/compare.py 4.0`. It refuses to keep a capture that is the same
   picture as the one before, which is what a paused player gives when it was not redrawn.
6. Crop all three and `footage/reference-1440.png` to the eye (`crop=1200:800:1040:320`), and
   encode everything to WebP (`cwebp -q 86`; `-q 92` for the comparison).

The window is 2560×1440 pixels: a 1280×720 window on a 2× display, the size the controls are
drawn for. Keep the subtitle files out of the video's folder, or mpv loads them a second time
beside the muxed tracks.
