# anistream native UX

Written 20 October 2026. The behaviour spec for every native client.

The apps behave like Apple's own apps and look like anistream. Navigation, controls, menus and glass are the system's; the identity sits in the Obi & Silk palette, show titles in Zen Kaku Gothic New, cover art and Nagare. Motion is continuous and depth-led, and the player fades to bare video whenever nobody is touching it. This spec covers behaviour; the design canvas covers the look. The numbers are the values to build with, tuned on device by screenshot and screen recording.

Later decisions win where they disagree with this text:

- The design review settled: Settings sits inside the Mac and iPad main window, a ⌘K palette is built, torrents run on every device, and Up Next counts down 10 s and fades through the ground colour.
- Copy, 23 October 2026: the legal line is "anistream hosts no content." Drop "ships no sources of its own" and "Every source is run by someone else" where they appear below.
- Android TV, Windows and Linux follow the same principles, states, motion tokens and copy rules through their own platform's controls. Their own notes are `android/docs/tv-design-brief.md` and `docs/desktop-*.md`.

## Principles

1. System first. A system control or pattern wins over a custom one. We build our own only where the system has none: the player's controls, the notice capsule, the starting screen.
2. One accent. Amber marks what you can do next. Vermilion appears only with an error.
3. Nothing pops. Elements fade or move continuously. Nothing scales up from zero, bounces or overshoots, and every spring has zero bounce.
4. Everything moving can be interrupted. A gesture drives its motion one to one, then hands off to a spring that keeps the gesture's velocity.
5. Waiting is silent for 600 ms. Work that answers inside that shows nothing. Past it, the wait shows where its result will land, never as a spinner over the whole screen.
6. Haptics answer the hand. They confirm what a finger did, never what the app did on its own.
7. Thin chrome, generous targets. Lines and icons stay light, text stays readable, and every target is at least 44 × 44 pt on touch and 28 pt tall on Mac.

## Interaction model

One SwiftUI app with three shapes: a split view on Mac, the same split view on iPad that collapses as its window narrows, and a tab bar on iPhone. The sections are the canvas's: Continue, Calendar, Seasonal, Search, Library, Downloads, then Providers and Plugin stores under Sources, then Accounts.

### Mac

- The main window is a `NavigationSplitView`. Continue and Library show sidebar, list and a preview of the selected title, as on the canvas. Opening the preview (Return or a double-click) widens it into the full title page (M3). Seasonal, Calendar and Search use two columns, and a title pushes onto the content column. Section changes are instant (M1), and the preview crossfades as the selection moves (M2).
- The window's minimum size is 960 × 600 pt, and it restores its frame, sidebar width, section and scroll position (`SceneStorage`). The toolbar holds the search field (`searchable(placement: .toolbar)`) and the sync status, which opens a popover of running jobs.
- The player has its own single-instance `Window` (M4), so browsing goes on during playback. It remembers its size and position, opening the first time at the video's size capped at 70% of the screen, and resizes at the video's aspect ratio. The title bar is transparent and hides with the controls, and dragging the video moves the window.
- As in the companion, a click on the video pauses after the double-click interval, a double-click toggles full screen, and a drag of more than 8 pt never pauses. ⌘W stops playback with the position saved.
- Rows highlight on press and selection only, with no hover highlight and no scale. A truncated title shows whole in a `help(_:)` tooltip.
- Every cover and row has a `contextMenu` with the same items as the iPhone's long-press menu.
- Settings is the system Settings window (⌘,). See Decisions for you.

### iPad

- The root is a `TabView` with `.tabViewStyle(.sidebarAdaptable)`: the Mac's sidebar on iPad and the tab bar on iPhone, from one definition. List and preview sit side by side when the window is at least 900 pt wide.
- Touch works as on iPhone. The pointer gets the system's button effects, rows highlight on press and selection only, and secondary click opens context menus.
- The keyboard has the same `.commands` as Mac, shown in iPadOS 26's menu bar and by holding ⌘.
- The player is a full-screen cover zoomed from the banner (T2), and it follows the window in Stage Manager.

### iPhone

- Five tabs: Continue, Calendar, Seasonal, Library and Search (`Tab(role: .search)`, with its field at the bottom). Downloads is Library's first row. Accounts, Providers, Plugin stores and Settings open as a sheet from the account button on Continue and Library.
- The tab bar minimises on scroll and the large title collapses, both the system's (T6).
- The main action sits low. When a title page's Resume button scrolls away, a Resume capsule fades into the bottom toolbar. Sheets open at the medium detent with their action at the bottom.
- Long press opens `contextMenu(menuItems:preview:)` with the banner as the preview. Pull to refresh (`refreshable`) works on Continue, Calendar, Seasonal, Library (where it also syncs) and Downloads, and its spinner lasts until the engine's job ends, at most 15 s.
- The app runs in portrait. The player rotates to landscape when it opens (T2).

Swipe actions on iPhone and iPad:

| Row | Leading | Trailing | Full swipe |
| --- | --- | --- | --- |
| Continue | Watched (the current episode) | Remove | Trailing, with Undo in a notice |
| Episode | Watched or Unwatched | Download | Leading |
| Download | none | Cancel, Delete | None. Delete asks first |
| Source on Providers | none | none | Reordered by drag in edit mode |

## Keyboard and menus

Every action is a menu bar item, so the Help menu's search (⇧⌘/) is the command palette and no custom palette is built. Frequent actions get a shortcut; the rest are menu items without one, as in Apple's apps. Mac and iPad share one `.commands` definition. The menus are anistream, File (Join Plugin Store…), Edit, View (sidebar, sections, Refresh), Title (actions on the selected title), Playback, Window (Mini Player, Messages) and Help (Keyboard Shortcuts).

Bare-letter keys work only in the player window, and in lists only while no text field has focus. This is the TUI's rule that `d` in a search box must not start a download.

### Main window

| Action | Mac and iPad | TUI key |
| --- | --- | --- |
| Keyboard shortcuts | ⌘/ (also ? in a list) | ? |
| Find any command | ⇧⌘/ (system Help search) | : or Ctrl-K |
| Go to section | ⌘1 to ⌘9 in sidebar order | 1 to 9 |
| Toggle sidebar | ⌃⌘S | Tab |
| Search | ⌘F (also / in a list) | / |
| Search filters | ⌥⌘F | Ctrl-F |
| Back | ⌘\[ | Esc, q |
| Refresh | ⌘R | r |
| Force a full resync | ⇧⌘R | R |
| Open the selection | Return, ⌘↓ | Enter |
| Play next unwatched | Space while nothing plays, ⌘Return | Space |
| Mark as Watched or Unwatched | ⇧⌘U | u |
| Remove from Continue | ⌘⌫ | x |
| Move source up or down | ⌥⌘↑, ⌥⌘↓ | K, J |
| Delete download and its file | ⌘⌫ on Downloads, then confirm | X |
| Sub or Dub | ⌥⌘T | t |
| Messages (recent notices) | ⌥⌘L | L |
| Settings | ⌘, | 9 |
| Quit | ⌘Q | Q, Ctrl-C |

Set Status, Rate, Episodes, Sources, Source for This Title, Fix Match, Download, Download Range, Watch Order, Recommendations, Play Trailer, Open on AniList, Play in a Watch Party, Mark All Previous as Watched and Clear Finished Downloads are menu items without shortcuts.

### Player window

The keys follow the mpv companion, so the same hands work in both players.

| Action | Mac and iPad | Companion or TUI |
| --- | --- | --- |
| Play or pause | Space, click on the video | Space |
| Back or forward 5 s, exact | ←, → (repeat while held) | ←, → |
| Back or forward 30 s | ⇧←, ⇧→ | H, L |
| Volume up or down 5% | ↑, ↓; scroll 2% per notch | 0, 9, wheel |
| Mute | M | none |
| Next or previous episode | N, ⇧N | n, N (TUI) |
| Skip opening or ending | S, or Return while the button shows | S |
| Undo an automatic skip | ⌘Z, or U while the note shows | u |
| Up Next: Play now or Cancel | Return, Esc | Enter, Esc |
| Stall: Switch source or Keep waiting | Return, Esc | Enter, Esc |
| Faster, slower, normal speed | \], \[, ⌫ | \], \[ |
| Full screen | F, ⌃⌘F, double-click | F |
| Mini Player | ⌥⌘M | Alt-M |
| Picture in Picture | ⌥⌘P | none |
| Sync Subtitles | ⌥Z | Alt-Z |
| Episodes, Subtitles, Audio | E, ⌥S, ⌥A (open their popovers) | menu |
| Stats for nerds | I | I, ⇧I (mpv's own) |
| Show the main window, keep playing | ⌘0 | q |
| Stop | ⌘. or ⌘W | x |

Esc answers the top thing first: an open menu or popover closes, then a card is answered (Up Next is cancelled, the stall prompt keeps waiting), then full screen ends. Any key or pointer movement shows the controls; arrow keys and S act without showing them, and their result shows in a small time or label flash for 1.2 s, as the companion's volume pill does.

## Haptics

Haptics confirm what the hand did while the eye is elsewhere: a control pressed on the video, a threshold crossed in a drag, a state flipped in a list. Most actions get none. iPhone plays them. iPad has no Taptic Engine, so the calls do nothing there and no iPad behaviour depends on them. On Mac, SwiftUI's `.alignment` and `.levelChange` reach a Force Touch trackpad through `NSHapticFeedbackManager`, and nothing plays on a mouse. The System Haptics setting turns them all off.

1. Only the user's own touch, drag or press triggers one. Engine events never do: an automatic skip, Up Next rolling on, a finished download, a notice.
2. A button that navigates gets none, because the new screen confirms it. A control that changes playback in place (Skip, Undo, Play now) and a watched mark get a light impact (`.impact(weight: .light)`).
3. One per threshold, never a continuous buzz, at most one per 80 ms.
4. Where a system control already plays one (a full swipe, a context menu lifting, a wheel picker, `Toggle`), we add nothing.
5. Each is a `sensoryFeedback(_:trigger:)` keyed to the value that changed, so a redraw cannot fire it.

| Action | iPhone | Mac trackpad | Why |
| --- | --- | --- | --- |
| Scrub crosses an opening or ending mark (T4) | `.selection` | `.alignment` | A boundary the eye can miss |
| Scrub precision step, reorder slot passed | `.selection` | none | A step inside a drag |
| Player dismiss drag crosses its threshold (T3) | light impact | none | The point of no return |
| Skip opening or ending, Undo, Play now (T4, T5) | light impact | none | Acts in place on the video |
| Marking watched by check, swipe or menu (T9) | light impact | none | A small change in a dense list. A full swipe plays the system's instead |
| Long press reaches 2× speed | light impact | none | A hold shows no press under the finger |
| Pinch reaches Fill or Fit | `.alignment` | none | A snap |
| Mark all previous as watched | `.success` | none | A bulk change the screen may only partly show |
| Own input rejected in place: invite link, sign-in, episode range | `.error` | none | The failure belongs to the action |
| Volume slider reaches 0 or 100% | none | `.levelChange` | The end of a range |
| Navigation, starting playback (T2), Cancel, double-tap seek | none | none | The screen shows it |
| Engine events and notices | none | none | Rule 1 |
| Pull to refresh, pickers, toggles, context menus | none added | none | Rule 4 |

## Motion

Motion is fixed in the design brief and drawn on the canvas boards MotionSystem, MotionCoverToTitle, MotionTitleToPlayer, MotionPlayer, MotionMac and MotionFeedback. The motion spec (native-spec/motion.md) describes each transition by id: T1 to T9 on iPhone and iPad, M1 to M5 on Mac. This spec cites those ids and does not repeat them.

- Springs for anything spatial, always with zero bounce. Fades for anything that only appears or leaves.
- Depth comes from planes moving at different speeds, such as the banner scrolling at half speed behind the title page (T1). Never from shadows, glow or scale pops.
- The player's controls fade and never slide (T4, M5).
- Every spring is interruptible and input is never blocked during a transition. A gesture moves its view one to one and hands its velocity to the spring on release.
- No staggered list entrances, no shimmer, no bounce or overshoot.

| Token | SwiftUI | Used for |
| --- | --- | --- |
| quick | `.spring(duration: 0.2, bounce: 0)` | Selection tint, segmented control, toggles, row dim after marking watched |
| standard | `.spring(duration: 0.3, bounce: 0)` | Content changes inside a screen: progress bars, cards appearing (Skip opening, Up Next), list insert and remove, Mac banner morph |
| fade-in | `.easeOut(duration: 0.2)` | Player controls appearing, artwork arriving, first video frame over the poster |
| fade-out | `.easeIn(duration: 0.3)` | Player controls hiding |
| linear | `.linear(duration: n)` | Countdowns only (Up Next 8 s) |
| system | The platform's own | Navigation push and zoom, sheets, menus, tab bar, window open, full screen |

Reduce Motion: zoom and matched geometry become crossfades (standard), parallax and the banner stretch are off, the player dismiss drag fades instead of scaling, and draw-on becomes an instant change. Numeric text and fades stay.

Three motions belong to surfaces only this spec defines, and use the same tokens:

- The notice capsule fades in (fade-in) and out (fade-out). A newer notice crossfades over the current one.
- The double-tap seek label (“+10 s”) fades in (fade-in), counts up with each tap and fades out (fade-out) 0.6 s after the last one.
- The Mac mini player moves the window frame to its corner and back with the system window animation (system); with Reduce Motion it jumps.

## Player

libmpv draws into the app's own view and SwiftUI controls sit on top, which keeps MKV, ASS subtitles, upscaling shaders and HDR. The companion's drawn interface becomes native controls driven by the same engine events: `SkipAvailable`, `SkipCleared`, `PrefetchNext`, `PlaybackStaged`, `PlaybackEnded { rolling_on }` and `Resumed`.

### Controls

- At rest the screen is bare video. Controls fade in on a tap, pointer movement or key press, and fade out after 3 s idle on touch (T4) or 2 s on Mac with the pointer hidden (M5). They stay while scrubbing, while a menu is open, while the pointer is over the bar and while VoiceOver runs.
- Bottom: the seek line at full width, with elapsed and remaining time at its ends in 15 pt monospaced digits, in silk. Below it, play/pause and volume on the leading side, always there. On the trailing side: Subtitles, Audio, Episodes, AirPlay, Picture in Picture and Full Screen (Mac), and a More menu with Speed, Upscaling, Night mode, Sources and Sync subtitles. iPhone merges Subtitles and Audio. Top: the show and episode, and on iPhone a close button that runs the same close as the swipe down (T3).
- A ground-coloured scrim rises 110 pt from the bottom edge to 72% at the bar, as the companion draws it. Labels over bright video get a 1 px dark outline.
- The bottom trailing corner holds one card at a time: the stall prompt, then the Undo note, then Up Next, then the Skip button. Skip dims after 8 s and returns to full strength under a finger or pointer.

### Gestures on iPhone and iPad

| Gesture | Result |
| --- | --- |
| Tap on the video | Show or hide the controls. A tap never pauses |
| Double tap on the left or right third | Back or forward 10 s. Each further tap within 0.6 s adds 10 s |
| Long press on the right half | 2× speed while held, with a “2×” label at the top |
| Swipe down | The video follows the finger and closes into the banner past 120 pt or on a fast flick, otherwise springs back (T3). Playback stops with its position saved |
| Pinch out or in | Fill (mpv `panscan=1`) or Fit (`panscan=0`), snapping at each end |
| Drag on the seek line | Scrub |

### Pointer on Mac

| Input | Result |
| --- | --- |
| Click | Play or pause, after the double-click interval |
| Double-click | Full screen |
| Move | Show the controls; they and the cursor hide 2 s after the pointer stops (`NSCursor.setHiddenUntilMouseMoves`) |
| Vertical scroll | Volume, 2% per notch |
| Horizontal two-finger scroll | Seek, 0.25 s per point, momentum ignored |
| Secondary click | Play/Pause, Skip, Subtitles, Audio, Episodes, Mini Player, Picture in Picture |

### Scrubbing

- The hit band is 44 pt tall on touch and 36 pt on Mac, around a 3 pt line that grows to 6 pt when touched (quick). The track is silk at 25%, buffered stretches at 45%, openings and endings water, played time amber.
- While dragging, a time label with the chapter's name sits above the thumb with a 160 × 90 pt frame preview from a second, hidden libmpv instance, off in Low Power Mode. The video follows with keyframe seeks and lands with an exact seek on release.
- Crossing an opening or ending mark gives a selection haptic (T4).
- Moving the finger up off the line slows scrubbing to half speed past 48 pt and quarter speed past 96 pt, labelled “Half-speed scrubbing” and “Quarter-speed scrubbing”. On Mac, ⌥-drag scrubs at quarter speed.
- Arrow keys and double taps seek exactly, because mpv's keyframe seeks turned 5 s into 7 or 8 s on HLS streams.

### Up Next and auto-next

- The card fades in when the ending starts (T5), or 30 s before the end of anything over 2 minutes when there is no ending data. The engine is already finding the next episode's streams by then (`PrefetchNext`), so Play now usually starts within a second.
- It shows Up Next, Episode 8, “A Town Without Night”, the episode's still at 160 × 90 pt, Play now and Cancel.
- With auto-play next on, the last 8 s count down (“Plays in 8 s”) on a linear bar. At zero the last frame fades to black over 0.25 s and the next episode fades in, in the same player (T5). The countdown pauses with the video.
- Cancel fades the card out (quick) for the rest of the episode. Up Next outranks the ending's Skip button, and S still skips. After the last available episode the player closes to the title page.

### Skip and Undo

- “Skip opening” or “Skip ending” fades in at the segment's start and out at its end (T4). Pressing it seeks past the segment, and the button leaves at once.
- With skip opening automatically on, the corner shows “Skipped opening · Undo” for 6 s of playback, at most 15 s while paused. Undo seeks back to where the skip started. On Mac it is also Edit > Undo (⌘Z).

### Subtitles and audio

- Subtitles lists Off, then each track by language with its title below (“English · Signs & Songs”). Audio lists its tracks the same way. A pick applies at once and is remembered per series and translation, as the engine does today.
- Sub and Dub is a segmented picker at the top. Switching restarts the episode where it was, behind the starting screen.
- On Mac these are popovers on their buttons and submenus of the Playback menu. On iPhone they share one Audio and subtitles sheet that morphs out of its button (T7). Sync subtitles is the companion's two-point sync: four steps at the top of the video, with Mark (Return) and Cancel (Esc).

### Stall prompt

- Buffering for under 1 s shows nothing. From 1 s a system `ProgressView` sits in the middle of the video.
- After 8 s, the engine's threshold, the corner card reads “Source is buffering · 9 s” and counts up, with Switch source as the primary button and Keep waiting. Switching resumes at the same position behind the starting screen. Keep waiting hides the card until the next stall. With no first frame after 20 s, the starting screen offers Choose source and Stop.

### Resume

- A stored position plays from there, and the top leading corner shows “Resumed at 12:04” with Start over for 6 s of playback.
- The title page's button reads “Resume episode 7”, with “14 min left” beside the progress bar; its context menu has Play from the beginning. An episode counts as watched at 85%.

### Starting an episode

- From `PlaybackStaged` to the first frame, the banner art holds the screen behind a small spinner and “Starting episode 7”, and the first frame fades in over it (T2, M4). After 3 s the line admits the wait, “Finding a source · 4 s”, using the engine's job note when there is one.
- This screen does the terminal eyecatch's job of covering stream resolution. Nagare never appears in the player.

### Picture in Picture

libmpv draws with the GPU into its own layer, and Picture in Picture accepts only an `AVPlayerLayer` or an `AVSampleBufferDisplayLayer`, so frames have to be handed over one at a time.

- Mac, in v1: `AVPictureInPictureController` with `ContentSource(sampleBufferDisplayLayer:playbackDelegate:)`. While PiP is open, mpv renders each frame into an IOSurface-backed `CVPixelBuffer` at the PiP window's size, which is wrapped in a `CMSampleBuffer` and enqueued. Subtitles are burned in and HDR is tone-mapped to SDR. It costs one extra render per frame, only while PiP is open.
- iPhone and iPad: the same path works while the app is in front. In the background, iOS refuses GPU work from the app, which stops libmpv's renderer. Frames would have to come from mpv's software renderer (`MPV_RENDER_API_TYPE_SW`) at 960 × 540 or less with upscaling off, which is CPU-heavy and untested on iPhone. iOS v1 ships without PiP and a spike decides it.
- The Mac mini player is separate from PiP. The player window shrinks to a quarter of the screen's width in the bottom right corner, floats above other windows and keeps play/pause, volume and the seek line. Leaving restores the exact frame, as in the companion.

### AirPlay

- Audio: an AirPlay button (`AVRoutePickerView`) sends mpv's audio to AirPlay speakers and headphones.
- Video: AirPlay video needs `AVPlayer` and a stream the receiver fetches by itself, which rules out MKV, ASS subtitles and the local stream proxy, so v1 has none. Screen mirroring works from Control Center, and a later external display scene will put the video full screen on a mirrored Apple TV while the phone shows the controls.

### Now Playing and remote commands

`MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` are set while a playback exists and cleared on Stop.

| Item | Value |
| --- | --- |
| Title and artist | Episode 7 · The Lantern Road; Hoshikuzu no Tabibito |
| Artwork | The episode's still, otherwise the banner |
| Timing | Media type `.video`. Duration, elapsed time and rate are written on play, pause, seek, speed and episode change, never on a timer |
| Everywhere | Play, pause, toggle, and change playback position, so the lock screen scrubs |
| iPhone and iPad | Skip forward and back 10 s |
| Mac | Next and previous track as next and previous episode (F7, F9), and `playbackState` set |

### Background

- iPhone and iPad use the `.playback` category, the `.moviePlayback` mode and the audio background mode, which free signing allows. Locking or going home keeps the audio and turns the video track off (`vid=no`); coming back re-syncs in under 0.5 s.
- Calls and Siri pause, and playback resumes when the interruption allows it. Unplugging headphones pauses.
- While playing, Mac keeps the display awake (`ProcessInfo.beginActivity` with `.idleDisplaySleepDisabled`) and iPhone disables the idle timer. Pausing releases both.

## States

Every screen has loading, content, empty, offline and failed states. All of them follow the TUI's rule: nothing for the first 600 ms, then the state shows where it applies.

### Loading

- Under 600 ms the previous content stays, or the ground stays empty on a first visit. Most loads end here.
- Past 600 ms, static placeholder rows appear in the real layout (`.redacted(reason: .placeholder)`, T8). They stay at least 0.3 s and are replaced in one crossfade (standard). Past 4 s the job's activity line appears below them: “Loading this season”.
- Covers fade in when they arrive (fade-in). Until then, or if they fail, a Raised plate stands in, with no broken-image glyph, and failed covers retry later.

### Empty

Nagare (cell 5, 100 × 160 pt) stands above a title, one line and at most one button. Small, frequent empties such as a filtered list or a search use `ContentUnavailableView` without Nagare. Nagare appears only here, on first launch and in the About window.

| Screen | Title | Line | Button |
| --- | --- | --- | --- |
| Continue | Nothing watched yet | Pick something from Seasonal, or search for a title. | Browse Seasonal |
| Library, signed out | Your AniList list shows here | Sign in to sync what you watch and where you stopped. | Sign in to AniList |
| Downloads | No downloads | Downloaded episodes play without a connection. | none |
| Providers, no sources | No sources yet | anistream ships no sources of its own. Sources come as plugins from a plugin store. | Join a plugin store |
| Search, no results | `ContentUnavailableView.search(text:)` |  |  |

### Offline

- `NWPathMonitor` drives one app-wide state. Cached screens stay usable, with “Offline. Downloaded episodes still play.” under the title in haze beside a `wifi.slash` glyph, not a coloured banner.
- A screen with nothing cached shows `ContentUnavailableView` (“You're offline”) with Show downloads. Nothing is disabled ahead of time. When the connection returns, the line fades and the screen refreshes once.

### Errors

- Errors show where the action happened. A screen that failed to load shows `ContentUnavailableView` with the reason and Try again. A failed row action restores the row and a notice says why. A failed playback shows “Couldn't play episode 8” in the player with Choose source and Close.
- Vermilion colours the error glyph (`exclamationmark.triangle.fill`) and nothing else. The full error chain goes to Messages, and the screen shows its first clause in plain words.
- System alerts only confirm destructive actions: deleting a download and its file, leaving a store, removing a member, turning torrents on without protection.

### Notices

Engine notices appear in a notice capsule: glass (`glassEffect(.regular, in: .capsule)`) at the bottom centre, above the tab bar on iPhone and 16 pt above the window's bottom edge on Mac. It holds one or two lines, with the hint as a button on its trailing side. The system uses this shape for passing confirmations, such as AirPods connecting or “Added to Library” in Music. It stays clear of the toolbar and the Dynamic Island, puts the button within thumb reach and never takes focus. An alert would interrupt for something that needs no answer.

- Info notices stay 4 s, notices with a button 8 s and alerts 8 s, plus 1 s for every 10 words past 12. Timers pause under the pointer or VoiceOver focus and while the app is inactive. A swipe down or a click dismisses. A newer notice crossfades over the current one.
- `NoticeHint` becomes the button: Stop for `StopPlayback`, Open stores for `OpenStores` (it goes to Plugin stores), Sign in for `KitsuCredentials` (it opens the Kitsu sheet).
- While the player is up, playback notices show in its top leading corner. Notices that arrive in the background wait up to 60 s for the app to become active. All of them go to Messages, the TUI's recent messages.
- Anything that needs an answer is a sheet, not a notice: a device code (up to 15 minutes), a match to choose, credentials.

### Running jobs

A job (`JobStarted`, `JobNote`, `JobDone`) shows where its result will land, after 600 ms, and goes the moment `JobDone` arrives.

| Job | Shown as |
| --- | --- |
| Loading a screen, a title or episodes | Placeholder rows, then the activity line after 4 s |
| Search | A row under the field: Searching for “lantern” |
| Sync | Mac: “Syncing” with a small `ProgressView` in the toolbar status. iPhone: `navigationSubtitle` on Continue and Library |
| Starting an episode | The starting screen (T2) |
| Checking providers, installing a plugin, signing in | A `ProgressView` in the affected row or button |
| Queueing downloads | The episode rows show Queued at once; the job shows nothing |
| A `JobNote` with a wait | Added where the job shows, counting down: “Waiting for AniList · 12 s” |

On Mac the toolbar status opens a popover listing every running job with its elapsed time and note: the TUI's activity row, whole.

## First launch

First launch is three short pages and then the app. It says plainly that anistream hosts nothing and that sources belong to other people. It never mentions torrents. On iPhone it is a full-screen cover; on Mac and iPad it is a sheet, 560 × 480 pt on Mac. Every page can be skipped, and each step is in Settings afterwards.

1. Welcome. Nagare, the name, and two lines: “Keep track of anime and play it from sources you choose.” and “anistream hosts no content. Every source is run by someone else.” Button: Continue.
2. AniList. “Sign in to AniList to sync your list and where you stopped.” Buttons: Sign in to AniList, Not now. A smaller line says other trackers are in Accounts. Sign-in uses the system web authentication sheet, or the device code sheet when the tracker needs one.
3. Sources. “anistream ships no sources of its own. Sources are plugins, shared through plugin stores by invite.” A field takes a pasted invite link, with Join. Below it: “Each plugin is someone else's code connecting to someone else's service. Use them at your own risk.” Button: Skip for now. An invite link opened from Messages or Mail (`anistream://` URL scheme) jumps straight to this step, filled in.

The app then opens on Continue with its empty state.

### Torrents

Torrents are off by default and live in Settings > Sources as “Torrent source”, on Mac, iPad and iPhone. Turning the switch on opens a sheet before anything starts:

- “Torrents download from other people and share what you download with them while you watch. They can see your IP address.”
- A “Protect with” picker: SOCKS5 proxy (host, port, user name, password), VPN on this device, or No protection.
- “anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.”
- No protection keeps Turn on disabled until a checkbox is ticked: “I understand other people can see my IP address.”
- The seeding settings (seed after watching, seed for at most, upload limit) appear only once the source is on.

On iPhone and iPad, torrents run while the app is open:

- Streaming and downloading run in the foreground. When the app goes to the background, the torrent session pauses. When it comes back, the check runs again and then the session resumes, so the check never runs in the background.
- Seeding happens only while the app is open, so seed after watching is best effort. Its row says so and names the device: “On this iPhone, seeding happens only while anistream is open.”
- A “Torrents on cellular” row, off by default, appears on devices with cellular. While it is off, torrents pause on cellular and resume on Wi-Fi.
- A torrent episode playing with the phone locked is open; see Decisions for you.

## Accessibility

### VoiceOver

- A cover or row is one element. Its label is the show title, then the English title when it differs (“Hoshikuzu no Tabibito, Stardust Wayfarer”). Its value is the progress in words (“Episode 7, 14 minutes left”). The context menu's items are its custom actions (Mark as watched, Remove from Continue, Download), and the default action opens the title. The Japanese title is read in Japanese (`accessibilitySpeechLanguage` set to `ja`).
- A title page reads title, metadata, main button, progress, episodes, then synopsis (`accessibilitySortPriority`). Section headings are headers, for the headings rotor. Custom rotors (`accessibilityRotor`) give Unwatched episodes on a title page, Days on Calendar and In progress on Downloads.
- In the player the controls stay up while VoiceOver runs. The seek line adjusts in 10 s steps, with the value “12 minutes 4 seconds of 23 minutes 40 seconds”. The magic tap plays or pauses, and the escape gesture closes the player.
- Up Next is announced when it appears and once at “Plays in 8 seconds”, never every second. A skip offer is announced once. Every notice is announced (`AccessibilityNotification.Announcement`), alerts at high priority.
- For Voice Control, icon-only controls get `accessibilityInputLabels` (“Skip”, “Subtitles”).

### Dynamic Type

App text uses the system text styles. Show titles use Zen Kaku Gothic New through `Font.custom(_:size:relativeTo:)`, so they scale with the style beside them. Mac has no Dynamic Type and follows the system's sizes.

| Surface | Largest size | Past it |
| --- | --- | --- |
| Title page show title | `.accessibility3` | Wraps to at most 4 lines |
| Show title in a list row | `.accessibility2` | 2 lines, 3 at accessibility sizes |
| Grid captions | `.xxxLarge` | Grids become one-column lists with 60 pt covers |
| Metadata lines | `.accessibility1` | Letter spacing drops to 0 |
| Player controls, cards and labels | `.xxxLarge` | Capped, so they never cover the picture |

### Contrast and colour

On the ground (`#161A2E`): silk 13.0:1, water 9.1:1, amber 8.5:1 (and ground on amber 8.5:1), haze 6.0:1, vermilion 5.2:1. Haze on Raised is 5.3:1. All pass 4.5:1 at body size.

- Increase Contrast (`colorSchemeContrast == .increased`) lifts haze to `#B8BDD6` and hairlines from `#434653` to `#5C6073`.
- Colour is never the only signal. A watched episode has a checkmark, the water dot comes with “out today”, and an error has a glyph and words.

### Reduce Motion, Reduce Transparency, captions

- Reduce Motion uses the substitutions in Motion.
- Reduce Transparency turns system glass opaque by itself. The notice capsule and the player's controls switch to solid Raised, and the scrim becomes the ground at 88%, the companion's panel opacity.
- Plain-text subtitles (SRT, WebVTT) follow the system caption size from Subtitles & Captioning, read with `MACaptionAppearanceGetRelativeCharacterSize` and applied as mpv's `sub-scale`. ASS subtitles keep their own styling.

### Keyboard only on Mac

- With Full Keyboard Access every control is reachable, custom cover cells included (`focusable()`). The focus ring is the system's in the accent colour, never hidden.
- Tab moves between sidebar, list and title page; arrows move inside them; Return opens; Space plays. In the player any key shows the controls, Tab walks them, and they do not hide while one has focus.
- Every sheet has a default button on Return and a cancel button on Esc (`keyboardShortcut(.defaultAction)`, `.cancelAction`).

## Platform integration

v1 takes what a video app is expected to have and what the TUI already does. The rest waits, for the reason beside it.

| Feature | When | Reason |
| --- | --- | --- |
| Now Playing and remote commands | v1 | The lock screen, Control Center, AirPods and media keys are how people pause a video |
| Background audio on iPhone and iPad | v1 | Locking the phone must not stop an episode |
| Menu bar commands and shortcuts | v1 | Keyboard parity with the TUI |
| Mini player and Picture in Picture on Mac | v1 | Parity with the companion's mini player |
| `anistream://` invite links (`onOpenURL`) | v1 | Plugin stores run on invites. Universal links need an associated domain, which needs a paid team |
| Keychain tokens, state restoration, `ShareLink` | v1 | Expected and small |
| Picture in Picture on iPhone and iPad | After a spike | No GPU work in the background (see Player) |
| Mini player on iPhone | After v1 | T3 closes the player; a live mini player needs its own design |
| External display scene for mirroring | v1.1 | A second render target; mirroring covers it meanwhile |
| HTTP downloads that finish in the background on iPhone | v1.1 | `BGContinuedProcessingTask` (iOS 26) lets user-started work finish, with its own system progress UI |
| Live Activities | v1 | Downloads, the episode playing, and an episode of a show you follow airing within the hour. Local updates only, no push: the views run on dates and a stale date |
| Handoff of the playback position | Later | Handoff needs both apps signed by one team; ad-hoc Mac builds have none |
| Widgets | Later | Needs an app group shared with an extension; check that free signing allows one |
| Shortcuts, App Intents, Spotlight | Later | Once the engine's API settles. App Intents bring Spotlight and Siri with them |
| New-episode notifications | Later | Background refresh is unreliable without a push server |
| SharePlay | Not planned | Syncplay parties exist, and GroupActivities needs an entitlement |

## UI copy

Labels are short and concrete, in the words the platform uses for the same thing.

1. Use the system's words: Sign in, Download, Remove, Up Next, Resume, Done, Cancel, Settings, Picture in Picture.
2. Sentence case for every string the app writes: buttons, labels, settings rows, notices. The Mac menu bar is the one exception, in title case like the system menus around it (see Decisions for you).
3. Name the thing and its number: “Episode 7 · 14 min left”, “Plays in 8 s”. Numbers, durations and dates go through the system formatters (`Duration.formatted`, `Date.FormatStyle`), so they follow the locale and its clock.
4. A middle dot joins facts. No dashes, exclamation marks, emoji, “please”, “oops” or “simply”, and no marketing words such as free, unlimited, powerful or seamless.
5. An error says what failed and what to do next. Raw error chains never reach the screen; they go to Messages.
6. Legal-first: “sources you choose”, “ships no sources of its own”, “hosts no content”. Never “pirate”, nothing that suggests paid content for nothing, and torrents never in a headline.
7. Keys appear only where keys exist. Mac menus show their shortcuts; nothing on iPhone says “press S”. A `NoticeHint` is a button, never a sentence about a key.
8. Engine text arrives in the terminal's lower case. The app capitalises the first letter and changes nothing else (see Decisions for you).

| Surface | Use | Avoid |
| --- | --- | --- |
| Title page main button | Resume episode 7 | Watch now! |
| Continue row | Episode 7 · 14 min left | 42% complete |
| Episode menu | Mark as watched | Toggle watched (the TUI's palette label) |
| Skip button | Skip opening | Skip intro, Skip OP |
| After an automatic skip | Skipped opening · Undo | Opening skipped automatically for you |
| Up Next | Up Next · Episode 8 · Play now · Cancel | Next episode starting soon… |
| Stall prompt | Source is buffering · 9 s, Switch source, Keep waiting | The stream stalled. Try another? |
| Notice with a hint | Plugin update ready: alpha, with an Open stores button | plugin update ready: alpha — Providers, Enter |
| First frame timeout | Couldn't start episode 8, with Choose source and Stop | mpv has not started playing after 20s — press x to stop; the source may have no data |
| Failed action | Couldn't open the trailer, with Try again | could not open the trailer: error sending request for url (…) |
| Empty Continue | Nothing watched yet | Your journey begins here |
| Settings row | Counts as watched at · 85% | commit\_threshold = 0.85 |

## Android notes

The Compose app keeps every rule above and changes only the means: Material 3 with dynamic colour off, the Obi & Silk palette as a fixed dark `ColorScheme`, edge to edge.

| Rule | Compose and Material 3 |
| --- | --- |
| Sidebar, tab bar, list and preview | `NavigationSuiteScaffold` (bottom bar on phones, rail or drawer on tablets) and `ListDetailPaneScaffold` |
| Cover to title (T1) | `SharedTransitionLayout`, with `Modifier.sharedElement` on the cover and banner and `sharedBounds` for the page |
| Back and player dismiss (T3) | Predictive back: the title page shrinks toward its cover, and the player toward its banner, as the gesture progresses (`PredictiveBackHandler`) |
| Motion tokens | quick and standard as `spring(dampingRatio = 1f, stiffness = 1000f)` and `spring(1f, 440f)`, the stiffness of zero-bounce springs of 0.2 s and 0.3 s. fade-in, fade-out and linear as `tween` with `LinearOutSlowInEasing`, `FastOutLinearInEasing` and `LinearEasing`. `MotionScheme.standard()`, never `expressive()`, which overshoots |
| Reduce Motion | Remove animations (`ANIMATOR_DURATION_SCALE` 0) applies the same substitutions, as explicit crossfades |
| Haptics | `.selection` is `SegmentTick` (`SegmentFrequentTick` while reordering). The light impact is `ContextClick` on Skip, Undo and Play now, `ToggleOn` and `ToggleOff` on watched, and `GestureThresholdActivate` at the dismiss threshold and the 2× hold. `.success` and `.error` are `Confirm` and `Reject` |
| Notice capsule | `SnackbarHost`, the hint as `actionLabel`, timed as above |
| Swipe, long press, pull to refresh | `SwipeToDismissBox`, `combinedClickable(onLongClick)` with a `DropdownMenu`, `PullToRefreshBox` |
| Player and Picture in Picture | libmpv in a `SurfaceView` under Compose controls. Android's PiP (`setAutoEnterEnabled(true)`) takes the `SurfaceView` as it is, so Android gets PiP without the iOS frame copy |
| Now Playing and background audio | Media3 `MediaSession` around a `SimpleBasePlayer` that wraps libmpv, in a `mediaPlayback` foreground service |
| Torrents | The same rules as iPhone: foreground only, a re-check on resume, Torrents on cellular off by default |
| Text size, TalkBack, keyboard | Font scale in `sp` with the same caps; `semantics` with `contentDescription`, `stateDescription`, `customActions` and `heading()`; `onPreviewKeyEvent` with the same keys, and `onProvideKeyboardShortcuts` for Meta + / |
| Glass chrome | No glass. Bars and capsules are solid Raised, with tonal elevation off |

## Decisions for you

Each has a recommendation. The spec is written as if you take it.

1. Mac Settings: the system Settings window (⌘,) instead of the sidebar row on the canvas. Recommended, because every Mac app opens settings that way; Accounts stays in the sidebar because sync status lives there.
2. Mac menu bar case: title case in the menu bar only, sentence case everywhere else. Recommended, so our items match Edit, View and Window beside them.
3. Engine notice casing: the engine writes the terminal's lower case. Recommended: write sentence case in the engine's wording and let the TUI lower-case it if it wants. Until then the app capitalises the first letter.
4. Picture in Picture on Mac: the public sample-buffer route, or the private PIP.framework that IINA uses. Recommended: the public route. It survives macOS updates, and it is the same frame path the iOS spike needs.
5. Picture in Picture on iPhone and iPad: a spike of mpv's software renderer at 540p in the background. Recommended: run it once v1's player works, and ship v1 without PiP on iOS.
6. Engine work for torrents on iPhone and iPad: pause the torrent session when the app goes to the background, and on return re-run the VPN check before resuming; plus a cellular flag the front end passes, so torrents pause on cellular while Torrents on cellular is off. Recommended as engine tasks ahead of the iOS torrent screens.
7. A torrent episode playing in the background on iPhone. Background audio keeps the app running, so iOS does not suspend it, and pausing the session would stop the episode once mpv's buffer runs out. Recommended: keep only the playing stream's session running while its audio plays, with the minute check still running, and pause everything else. The alternative is to pause playback with the session and resume on return.
8. Double-tap seek: 10 s on touch, against 5 s for the arrow keys. Recommended: 10 s. A tap costs more than a key press, and 10 s matches the lock screen's skip buttons.
9. Long press for 2× speed on touch. Recommended: keep it. It is the touch form of the TUI's speed keys.
10. Lock screen buttons on iPhone: skip 10 s, or next and previous episode. Recommended: skip 10 s. The next episode is one tap away in the player and in Up Next.
11. iPhone mini player: T3 closes the player, so browsing on iPhone stops playback. Recommended: ship T3 as drawn and add a mini player in `tabViewBottomAccessory` later if you miss it.
