# Next features

Six features to build once the parity follow-up and the website are done, each specified in its own file here. Their questions were settled on 24 October 2026; each spec lists the decisions at its end. They follow `docs/native-ux.md` for states, motion and copy, and the engine owns the logic in all of them: the apps render what the engine says.

Hayase has a version of each. The specs use it as the bar to beat, not as a design to copy. Each one says what Hayase does, where it falls short, and what we do better with what we have and it doesn't: native toolkits on every platform, a shared Rust engine, embedded libmpv, the Android TV app, Play on TV and the casting spike.

## Order

| # | Feature | Spec | Size | Better than Hayase |
| --- | --- | --- | --- | --- |
| 1 | Stats for nerds | [player-stats.md](player-stats.md) | About 4 days | Reads mpv, so it names the decoder, shaders, display, HDR and network speed a browser can't, at one read a second |
| 2 | Picture in Picture when leaving the app | [auto-pip.md](auto-pip.md) | About 1.5 weeks, less if the iOS GPU test passes | The floating window shows mpv's own frames with shaders and ASS subtitles, with no second copied video, and Android gets a Skip button |
| 3 | Trailers | [trailers.md](trailers.md) | About 2 weeks | Follows YouTube's embed rules, checks the network, battery, Reduce Motion and screen readers before playing, and uses each platform's own popover for hover previews |
| 4 | Episode discussions | [episode-threads.md](episode-threads.md) | About 2 weeks | Opens the one thread for the episode just watched, held back until it is watched, in a native sheet or pane, within the engine's shared AniList budget |
| 5 | Controller on Linux and the Steam Deck | [gamepad.md](gamepad.md) | About 1.5 weeks | Real focus movement in GTK from libmanette with the Deck's own driver, not six buttons turned into key presses, reusing the TV app's focus rules |
| 6 | Cast to Chromecast | [cast.md](cast.md) | About 4 weeks after the gate | Built on the spike: a Rust Cast sender in the engine, a token-locked relay that remuxes MKV, real progress from the TV, and Chromecasts in the same Play on TV picker |

## Why this order

- Stats for nerds first. It is small, touches the hosted player protocol that two later features also extend (`Sample` here, `Presentation` and `FloatingActions` for Picture in Picture), and makes every later feature easier to test and to report bugs against.
- Picture in Picture second, because most of it exists. What it needs first is a day on real iPhones and iPads, and that test decides whether the larger fallback gets built. Better to learn that early.
- Trailers third. It adds the device state (`Command::DeviceState`) and the autoplay rule that the rest of the app can use, and is the feature people see most.
- Episode discussions fourth. It is self-contained in the engine's AniList client and each app's sheet, so it can move in parallel with trailers if two people work on it.
- The controller can run in parallel with any of the above. It touches only the Linux app and the model's input mapping, and it should follow the Linux app's own parity work.
- Cast last, because it has a gate: the spike has never run against a real TV. Johan runs the spike's `live.sh` on his TV first. Until that passes, the four-week estimate is a guess.

## The biggest risks

- YouTube's embed rules: no overlays on the player, at least 200 × 200 px, autoplay only when mostly visible, and the app's identity in the Referer. Getting these wrong gets the embed blocked (error 153 with no Referer).
- iOS and the GPU in the background. If iOS refuses mpv's Metal work while the app is in the background, the floating window freezes on its last frame, and the software path at 540p may not hold 24 fps.
- Casting: the spike is untested on a TV; iOS suspends apps in the background, so casting from an iPhone may stop when the screen locks; Windows asks for firewall access the first time; ASS subtitles need WebVTT or a $5 custom receiver.
- libmanette has no Rust bindings (generated with gtk-rs `gir`), and the Deck's driver reads hidraw, which needs udev access outside Steam and `--device=all` in Flatpak.
- AniList's episode threads exist only by convention, so some episodes have none, and the API's limit is about 30 requests a minute for the whole app.
- WebKitGTK on Linux needs GStreamer's libav plugin for YouTube's codecs, which some distributions leave out.
