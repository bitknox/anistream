# Picture in Picture when leaving the app

Written 24 October 2026, to build after the parity and website phase. Follows `docs/native-ux.md`; its Picture in Picture section is out of date where this spec and the code disagree.

Leaving the app while an episode plays, by the home gesture, the app switcher or a notification that opens another app, moves the video into the system's floating window on iPhone, iPad and Android. Coming back puts it into the player where it was. Nothing reloads either way.

Most of this exists. iOS and Android both ask the system to start Picture in Picture by itself, and the Mac starts it on minimise or hide. What is missing is proof on devices, a fallback for iOS's background GPU rule, the engine knowing that the picture floats, a setting, and the behaviour of skips, Up Next and stalls while it floats.

Better than Hayase because the floating window shows mpv's own frames, with the shaders and ASS subtitles, from the one player that keeps playing, where Hayase copies a canvas into a second, muted video that only browsers allow, and our Android floating window gets a Skip button when there is something to skip.

## Compared with Hayase

Hayase listens for `visibilitychange` and, with its auto setting on, calls `requestPictureInPicture()` on the HTML video. With subtitles on, it draws each frame and the JASSUB subtitle canvas into a 2D canvas at the video's full size, turns that into a stream (`captureStream`), plays it in a second, muted video element and puts that in Picture in Picture.

Where it falls short:

- Browsers grant `requestPictureInPicture()` only with a user gesture, and a page being hidden is not one, so the automatic start depends on the shell letting it through. Android's System WebView doesn't offer the Picture in Picture API, so the Capacitor app has none.
- The floating window plays a copy. Its play and pause drive the copy, the real video and its audio go on separately, and the copy can drift.
- Every frame is drawn twice at the source's full size, whatever the floating window's size.
- There are no system playback controls, no skip actions and no restore into the player's own state.

What we have that it doesn't: libmpv embedded in a native player on each platform, `PictureFeed` on Apple platforms that copies mpv's finished frames into the system's display layer at the floating window's size, Android's `SurfaceView` that the system's window takes as it is, and the engine's session, which keeps resume, history and auto-next going whatever the window.

## What exists

- Apple, `apple/Packages/AnistreamPlayer/Sources/AnistreamPlayerUI/PictureInPicture.swift`: `AVPictureInPictureController` with a sample-buffer content source. On iPhone and iPad the feed runs whenever the player is up, at most 960 × 540 into a display layer beneath the picture, so the system can start from it, and `canStartPictureInPictureAutomaticallyFromInline` is on. While the window floats, frames render at its size. Restore, close and the window's buttons go through the player's model. The video track goes off in the background (`vid=no`) unless the window is up (`PlayerModel.followVideoTrack`).
- Mac: the button, ⌥⌘P, the Window menu, and minimising or hiding while it plays. Frames at one size, 1920 × 1080, scaled by the window.
- Android, `android/player-ui/src/main/kotlin/tv/anistream/player/ui/PictureInPicture.kt`: `setAutoEnterEnabled(playing)`, the video's aspect, the source-rect hint, seamless resize, and three `RemoteAction`s (back 10 s, play or pause, forward 10 s). Closing the window closes the player and keeps the place. The engine is told `AppPhase` on the way out and back.
- What the Simulator could not show, from `apple/README.md`: the home gesture starting it, the window's own buttons, and whether iOS lets mpv render on the GPU while the app is in the background. If it refuses, the floating window keeps its last frame while the audio plays on.

## Behaviour

### Starting

1. The player is up, the episode plays (not paused, not on the starting screen) and Picture in Picture when leaving is on.
2. The viewer leaves the app. The system moves the video into its floating window with its own animation.
3. Locking the phone is not leaving: the system doesn't start Picture in Picture on lock, and the background rules apply as now (audio on, video track off).
4. Paused, or on the starting screen: nothing floats. The app goes to the background as today.
5. iPad in Stage Manager or Split View: switching to another app's window while ours stays on screen is not leaving, and nothing floats.

### While it floats

- The window's buttons: on iPhone and iPad, the system's play and pause and its skip back and forward, which our delegate answers with 10 s seeks. On Android, three actions: back 10 s, play or pause, forward 10 s.
- When the engine offers a skip (`SkipAvailable`) on Android, the first action becomes Skip opening or Skip ending for the length of the offer, and goes back to back 10 s when it clears. iOS has no custom buttons in its window, so there the skip waits; with skip opening automatically on, the engine skips by itself as it always does.
- Up Next counts down as in the player, unseen. At zero the next episode plays in the same window, and the system's window keeps its place. With auto-play next off, the episode ends on its last frame, paused, and the window stays until closed or restored.
- A stall shows the system's own buffering state in the window. The stall prompt (Switch source, Keep waiting) waits for the viewer to return; the engine doesn't switch sources by itself while nobody can see the choice.
- The engine is told the picture floats, so it picks the upscaling chain for the window's size (usually none at that size), keeps the playing stream's torrent session running as native-ux's decision 7 has it, and holds thumbnails and the seek preview's second mpv.
- Now Playing and the lock screen stay as they are.

### Coming back

- Restore (the window's button, or tapping into the app) brings the player back full screen at the same position, controls shown for 3 s, with the floating window's system animation. Any card the engine still offers (Skip, Up Next, the stall prompt) appears in the player as usual.
- Opening the app from its icon while the window floats leaves the window floating over the app, as the system does for every video app.
- Closing the window stops playback with the position saved, the same as the player's own close.

### iOS and the GPU in the background

iOS rejects GPU command buffers from an app in the background (`kIOGPUCommandBufferCallbackErrorBackgroundExecutionNotPermitted`), and Apple documents no exception for a sample-buffer Picture in Picture. mpv renders with Metal through MoltenVK. The first device test decides which of these ships:

1. mpv renders on the GPU in the background while the window floats, and frames arrive. Nothing more to build.
2. It doesn't. While the window floats and the app is in the background, the player switches mpv to its software renderer (`MPV_RENDER_API_TYPE_SW`) at the window's size, at most 960 × 540, with hardware decoding kept and its frames copied back (`hwdec=videotoolbox-copy`), upscaling off and libass drawing on the CPU. Each frame goes into a `CVPixelBuffer` and the same display layer. Back in front, the GPU renderer takes over again.
3. The software renderer can't hold 24 fps on the oldest supported iPhone. Then iPhone and iPad start Picture in Picture only from the button while the app is in front, the setting is hidden there, and leaving the app plays audio only, as now.

The test plan: an iPhone 15 and an iPad on iOS 26, a 1080p HEVC 10-bit episode with heavy ASS typesetting, the home gesture while playing, five minutes floating, with frame timing logged from the feed and thermal state read every 10 s. Then the same with path 2 forced.

## States

| State | Shown as |
| --- | --- |
| Starting | The system's animation. Nothing of ours shows |
| Loading in the window | The system's buffering state, from the delegate's `isPlaybackPaused` and the stall the engine reports |
| Failed to start | Nothing on screen; the app went to the background, and audio plays on. "Picture in Picture didn't start" goes to Messages |
| Frames stopped (the GPU refused) | The window keeps its last frame and audio plays on; the player logs it and switches to path 2 if that ships |
| Offline | As for a stall: the window shows the system's buffering state |
| Empty | Not applicable |

## Motion

The system's own, entering and leaving (system). On restore, the player's controls fade in (fade-in) once the system's animation ends, and fade out after 3 s idle (fade-out). Reduce Motion is the system's to apply to its own animation.

## Platforms

| Platform | Leaving the app floats the video | Notes |
| --- | --- | --- |
| iPhone | Yes, after the GPU test | Needs `AVAudioSession` `.playback` with `.moviePlayback`, already set. The system's own switch (Settings > General > Picture in Picture > Start PiP Automatically) wins over ours |
| iPad | Yes, as iPhone | Not when our window stays on screen in Stage Manager |
| Android | Yes | Android 12's auto-enter (`setAutoEnterEnabled`), so the gesture animates smoothly. The per-app Picture in Picture permission in system settings wins over ours; when it is off, the Settings row says "Off in Android settings" with a button to that page |
| Android TV | No | The TV brief: leaving the app pauses and saves the position, and there is no Picture in Picture on TV in its first release |
| Mac | Already, on minimise or hide | Unchanged. The setting covers it too, so one switch means the same on every Apple device |
| Windows | No | No system Picture in Picture. Minimising a window is how a desktop viewer stops watching, so floating on minimise would surprise. The mini player stays a choice by hand (compact overlay) |
| Linux | No | The same as Windows; the app's mini player (P) is by hand |
| Terminal | No | mpv's own window; the window manager decides |

## Engine and model changes

```rust
// anistream-model::hosted
pub enum PlayerReport {
    // … as now, plus:
    /// Where the picture is: in the player, floating in the system's window, or nowhere
    /// (the app in the background with the video track off). Said on every change.
    Presentation { place: PicturePlace },
}

pub enum PicturePlace {
    Inline,
    /// The system's floating window, `width` × `height` pixels as the system last said.
    Floating { width: u32, height: u32 },
    Hidden,
}

pub enum PlayerUi {
    // … as now, plus:
    /// The floating window's actions, in order, for platforms that draw their own (Android).
    FloatingActions { actions: Vec<FloatingAction> },
}

pub enum FloatingAction {
    Back10,
    PlayPause { playing: bool },
    Forward10,
    Skip { kind: SkipKind },
}

// anistream-engine::playback
// While Floating: shaders::choose takes the window's size as the display; the thumbnailer and
// the seek preview stop; a stall waits for Inline before offering a switch; the playing
// torrent session stays up under AppState::Background { playing: true }.

// anistream-model::settings
SettingKey::PictureInPictureOnLeave
```

`FloatingActions` is sent while the player is up and again whenever a skip offer starts or clears, so Android's `setPictureInPictureParams` always holds the actions it would need on leaving. iOS ignores it.

## Settings

| Row | Values | Default | Where |
| --- | --- | --- | --- |
| Picture in Picture when leaving the app | On, Off | On | Settings > Playback |

Footnote on iPhone and iPad: "Also needs Start PiP Automatically in iOS Settings > General > Picture in Picture." On Android, when the system permission is off, the row's note says so with Open settings.

## Privacy and legal

The floating window shows what plays over other apps and on screen recordings and shares, as any video app's does. Nothing else changes: no data leaves the device.

## Size

Small to medium.

- The device test on iPhone and iPad: one day.
- Path 2, if needed: four days for the software renderer switch in `PictureFeed`, with its frame timing check.
- Engine and model (`Presentation`, `FloatingActions`, the floating rules): two days.
- Android's skip action and the permission note: one day. The setting on every app: one day.
- Then the polish pass, with screen recordings of leaving and restoring on each device.

## Open questions for Johan

1. The default. Options: on, as the system's own switch and most video apps behave; off, so leaving the app keeps today's audio-only behaviour unless asked. Recommended: on.
2. If iOS refuses the GPU in the background. Options: build the software renderer path at 540p; keep Picture in Picture on iPhone and iPad to the button while in front, with leaving staying audio-only. Recommended: build path 2 if the test shows it holds 24 fps on an iPhone 15; otherwise the button only.
3. The skip action in Android's floating window. Options: swap back 10 s for Skip opening during the offer; keep the three actions fixed. Recommended: swap it. Skipping is the one thing worth doing from a floating window that the system's buttons can't.
4. Up Next while floating with auto-play next on. Options: play the next episode in the window, as the player does; stop at the end and wait for the viewer. Recommended: follow the setting, as in the player, so the window never behaves differently from the full screen.

## Sources

- `AVPictureInPictureController.canStartPictureInPictureAutomaticallyFromInline`: <https://developer.apple.com/documentation/avkit/avpictureinpicturecontroller/canstartpictureinpictureautomaticallyfrominline>
- Picture in Picture in a custom player, including the sample-buffer content source: <https://developer.apple.com/documentation/avkit/adopting-picture-in-picture-in-a-custom-player>
- Metal in the background: <https://developer.apple.com/documentation/metal/preparing-your-metal-app-to-run-in-the-background>, and the Apple engineer's answer on discarded background command buffers in a Picture in Picture app: <https://developer.apple.com/forums/thread/691805>
- Android Picture in Picture and auto-enter: <https://developer.android.com/develop/ui/views/picture-in-picture>, `PictureInPictureParams.Builder.setAutoEnterEnabled`: <https://developer.android.com/reference/android/app/PictureInPictureParams.Builder>
- mpv's render API, software rendering: <https://github.com/mpv-player/mpv/blob/master/include/mpv/render.h>
- Hayase's Picture in Picture, read for this spec: `interface/src/lib/components/ui/player/pip.ts`.
