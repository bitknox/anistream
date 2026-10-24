# Gamepad navigation on Linux

Written 24 October 2026, to build after the parity and website phase. Follows `docs/native-ux.md` and the Linux app's own notes (`docs/desktop-linux-app.md`).

The Linux app (GTK 4 and libadwaita) can be used from the couch on a Steam Deck, with the Deck's own controls or any controller: browse with the D-pad or the left stick, open with A, go back with B, and drive the player with the buttons a TV remote would have. The keyboard and mouse keep working beside it, and the last input used decides what the screen shows.

Better than Hayase because every button has a job that fits where focus is, the player gets its own map instead of arrow keys, input arrives as native GTK focus moves rather than synthetic key events, nothing polls while no controller is connected, and the map lives in the model so the Windows app can take it unchanged.

## Compared with Hayase

Hayase polls the web Gamepad API on every animation frame and turns six inputs into synthetic keyboard events on the focused element: A is Enter, B and Start are Escape, the D-pad and left stick (with 0.5 and 0.3 hysteresis) are arrows, repeating after 400 ms every 100 ms. Its spatial navigation then moves focus.

Where it falls short:

- Feel. The player gets the same six inputs, so seeking, skipping and the episode list need several presses through on-screen buttons. X, Y, the shoulders, triggers, Select and the right stick do nothing.
- Reliability. Synthetic events are untrusted, so it re-creates link and button activation by hand, and anything listening for real key events misses them. Only the first `standard`-mapped pad counts.
- Battery. The frame loop runs whenever a pad is present.
- Platform fit. In Steam's desktop mode the Deck's buttons also arrive as keyboard and mouse input, which it doesn't account for, and there is no answer for typing.

What we have that it doesn't: GTK's own focus and keyboard navigation (the app is already fully keyboard driven), the player's key handling in `desktop/linux/src/player/mod.rs`, the TV app's focus model and D-pad rules (`android/docs/tv-design-brief.md`), which this map borrows, and the engine's player commands.

## Input

Two paths reach the app, and the spec supports both.

| Path | What it is | What the app sees | Code |
| --- | --- | --- | --- |
| libmanette | GNOME's gamepad library: evdev with SDL's mapping database, and since 0.2.11 its own HID driver for the Steam Deck, reading `/dev/hidraw` | A `ManetteDevice` per controller, with `button-press-event`, `button-release-event` and `absolute-axis-event` in standard button names | The app's |
| Steam Input's keyboard templates | Steam translating the controller into keys and the mouse for a non-Steam game | Arrow keys, Return, Escape, Space and so on | None: the app is already keyboard driven |

libmanette is the main path. Its Deck driver reads every button and keeps the touchpads working, skips Steam's virtual pad (28DE:11FF) where it sees the real one, and adds a dead zone for the Deck's sticks (0.2.12). It needs read access to the Deck's hidraw node, which Steam's udev rules grant on SteamOS; under Flatpak the app needs `--device=all`.

The keyboard path is the fallback when libmanette finds nothing: a Flatpak without device access, a distribution without libmanette 0.2.12, or a viewer who prefers Steam's templates. The Settings row says which path is in use.

In Steam's game mode, Steam keeps the Deck out of its desktop "lizard mode", so libmanette's events arrive once. In desktop mode with Steam running, lizard mode sends keys and mouse movement as well, which libmanette can't switch off while Steam runs. There the app takes the libmanette event and ignores the matching key that arrives within 50 ms of it, so nothing acts twice.

There are no Rust bindings for libmanette. We generate them from its `Manette-0.2.gir` with gtk-rs's `gir` tool, as a small `manette` crate inside `desktop/linux`, which is the way the app's other GNOME libraries are bound.

## Focus model

The TV brief's focus model, on GTK's focus.

- The first gamepad input switches the window into controller mode: `gtk_window_set_focus_visible(true)`, the pointer hides until it moves, and hover previews (see `trailers.md`) stop opening. Any mouse movement leaves controller mode; a key press keeps the focus ring and leaves the rest.
- Exactly one focused widget while the browse UI shows. Each page names its first focus: the main action on a title page, the first row on Continue, the first cover in a grid, the search field on Search.
- Directions move focus with `gtk_widget_child_focus` from the focused widget's nearest focus group: the sidebar, a list, a grid, a page's action row, a dialog. GTK's list and grid views already move by item for arrow directions; between groups the move is geometric and crosses only at a group's edge.
- Left from the leftmost item of a page moves to the sidebar on its current row. Right from the sidebar returns to the item the page last focused.
- Rows and grids remember their last focused item for the page's lifetime. Going back puts focus on the item that opened the page, with the scroll position unchanged.
- Nothing wraps. Focus never jumps on its own, except to the skip offer and to Up Next in the player, as on TV.
- The focused item scrolls into view with the page's own scrolling. Held directions repeat after 400 ms and then every 80 ms; while a direction repeats, focus changes snap and the scroll follows without easing, so a long hold never falls behind.

## Button map

Names are the Deck's and an Xbox controller's (the same layout). A Nintendo-layout controller can swap A and B in Settings.

### Browsing

| Input | Does |
| --- | --- |
| D-pad, left stick | Move focus |
| A | Activate: open a title, press a button, toggle a switch |
| B | Back: close a popover, menu or dialog first, then go back a page. On a section's root page, nothing |
| X | The focused item's menu: the same items as its context menu (Mark as watched, Remove from Continue, Download) |
| Y | Search |
| Menu (≡) | The window's main menu |
| View (⧉) | Show or hide the sidebar |
| L1, R1 | Previous and next section, in sidebar order |
| L2, R2 | Page up and page down in the focused list or grid |
| Right stick | Scroll the page without moving focus |
| Steam, Quick Access | The system's. The app never sees them |

### In the player

As the TV's D-pad rules, with the extra buttons doing what a remote's media keys do.

| Input | Controls hidden | Controls shown |
| --- | --- | --- |
| A | Play or pause, and show the controls. While the skip offer is in its first 8 s, Skip | Activate the focused control |
| B | Close the player, the position kept | Hide the controls. With a popover open, close it |
| D-pad left, right | Back or forward 10 s, with a running count; one exact seek 500 ms after the last press | Move focus along the bar; on the seek line, scrub as on TV |
| D-pad up | Show the controls, focus on the seek line | Move focus |
| D-pad down | Show the controls, focus on Episodes | Move focus |
| L1, R1 | Back or forward 30 s | The same |
| L2 held | Nothing | Nothing |
| R2 held | 2× speed while held, with the "2×" label | The same |
| X | Subtitles and audio popover | The same |
| Y | Episodes popover | The same |
| Menu | The More menu | The same |
| View | Playback details (see `player-stats.md`) | The same |
| Up Next showing | A plays now, B cancels | The same |
| Stall prompt showing | A switches source, B keeps waiting | The same |

The Deck's volume buttons are the system's and work as they do everywhere.

## Typing

Search and the plugin store's invite field need text.

- In game mode, focusing a text field with A asks Steam for its on-screen keyboard by opening `steam://open/keyboard`, which Steam answers while it runs (to confirm on a Deck before building; Steam+X opens the same keyboard by hand). Return on that keyboard submits.
- In desktop mode, the desktop's own on-screen keyboard is used where there is one; otherwise the field stays focused for a real keyboard.
- Search starts with recent searches and genres, which A opens without typing, so most searches need no keyboard at all.

## Help

The Keyboard Shortcuts window (Ctrl+? today) shows the controller map instead when the last input came from a controller, opened with View held for 1 s. Buttons appear nowhere else in the interface: no glyphs on buttons, no hint bars.

## States

| State | Shown as |
| --- | --- |
| Controller connected | A notice, "Controller connected · Steam Deck", 4 s, the first time in a session only |
| Controller disconnected while in use | A notice, "Controller disconnected", 4 s. Focus stays where it was, for the keyboard |
| No device access (Flatpak without `--device=all`, no udev rule) | No notice. Settings > Controller reads "No controller found. Steam Input's keyboard layouts still work." with a link to the help page |
| Loading | Focus never waits for content. A page whose content is loading focuses its first placeholder row and moves to the real row in place when it arrives (the placeholders follow native-ux's 600 ms and 4 s rules) |
| Offline, failure, empty | As each page already shows them; the empty states' one button takes first focus |

## Motion

| Moment | Token |
| --- | --- |
| Focus ring moving between items | quick, GTK's own focus ring |
| Scrolling a focused item into view | standard; snap while a direction repeats |
| Sidebar showing or hiding | system (`AdwOverlaySplitView`'s own) |
| Player controls on a button press | fade-in and fade-out, as for the keyboard |

No scale on focus, unlike TV. At the Deck's distance a 7-inch screen reads the ring well, and scaling covers in a GTK grid would re-lay out rows.

## The Steam Deck screen

- The display is 1280 × 800 at 7 inches. The app's medium layout fits it, with the sidebar collapsed by default below 1360 pt wide.
- In game mode (gamescope, `SteamGamepadUI` set) the app starts full screen and stays there; the window controls hide.
- Text follows the system's scale, which SteamOS's desktop sets to 1.0 at that size; see the open questions for a larger scale in game mode.
- On battery, the engine's power check already starts upscaling one step lighter. The Deck's sysfs battery reads like any laptop's.

## Platforms

| Platform | Gets it | Why |
| --- | --- | --- |
| Linux | Yes | The Steam Deck is the target, and any Linux desktop with a controller gets the same |
| Windows | Later, with the same map | `Windows.Gaming.Input.Gamepad` gives the same buttons. The map is in the model so the WinUI app can follow without a second design |
| Mac | No | Few people drive a Mac from a couch; GameController could add it later with the same map |
| iPhone, iPad | No | Touch first. Controllers on iPad are rare for video |
| Android | No | Touch first |
| Android TV | Has it | The system turns a controller's D-pad and A and B into D-pad, Center and Back, which the TV app handles already |
| Terminal | Through Steam Input | Its keys work with a keyboard template; the TUI needs nothing |

## Engine and model changes

The map is data in the model, so front ends share one table and the tests pin it. The focus moves stay in each toolkit.

```rust
// anistream-model::input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadButton {
    South, East, West, North,
    DpadUp, DpadDown, DpadLeft, DpadRight,
    LeftShoulder, RightShoulder, LeftTrigger, RightTrigger,
    Start, Select,
    LeftStick, RightStick,
}

/// Where focus is, as far as the map cares.
pub enum PadContext {
    Browse,
    Player { controls_shown: bool, card: Option<PlayerCard> },
}

pub enum PlayerCard {
    SkipFresh,
    UpNext,
    Stall,
}

/// What a press means. Browse actions are the front end's; player ones become commands.
pub enum PadAction {
    Focus(Direction),
    Activate,
    Back,
    ItemMenu,
    Search,
    MainMenu,
    ToggleSidebar,
    Section(i32),
    Page(i32),
    Player(PlayerCommand),
    PlayerPanel(PlayerPanel),
    HoldSpeed(bool),
}

pub enum PlayerPanel {
    Tracks,
    Episodes,
    More,
    Details,
}

pub fn pad_action(button: PadButton, pressed: bool, context: &PadContext, swap_face: bool)
    -> Option<PadAction>;

/// The left stick as directions, with hysteresis: pressed past 0.5, released under 0.3.
pub struct StickDirections { /* state */ }
impl StickDirections {
    pub fn update(&mut self, x: f64, y: f64) -> Vec<(Direction, bool)>;
}

/// The help window's rows, worded once.
pub fn pad_help(context: &PadContext) -> Vec<(PadButton, String)>;
```

`SettingKey` gains `ControllerInput` and `SwapFaceButtons`. No engine change beyond the settings.

## Settings

| Row | Values | Default | Where |
| --- | --- | --- | --- |
| Use a controller | On, Off | On | Settings > General > Controller |
| Swap A and B | On, Off | Off | The same, shown once a controller has been seen |
| Controller | Read-only: the device's name and path ("Steam Deck · libmanette", or "Steam Input keyboard layout") | | The same |

## Privacy and legal

Nothing leaves the device. The app reads controller input only while its window has focus, and opens no hidraw device other than a controller's. `steam://open/keyboard` goes to the local Steam client.

## Size

Medium, about a week and a half.

- The `manette` bindings and packaging (Flatpak device access, the Debian dependency on libmanette 0.2.12 or later): two days.
- The model's map and its tests: one day.
- Controller mode, focus groups, first focus per page and the sidebar crossing: three days, most of it walking every page with a controller.
- The player map, typing and the help window: two days.
- A day on a Steam Deck in game mode and desktop mode, then the polish pass.

## Open questions for Johan

1. Input path. Options: libmanette through generated bindings, with Steam Input's keyboard layouts as the fallback; gilrs, a pure-Rust library on evdev with SDL's mappings, which sees Steam's virtual pad but has no Deck driver; or Steam Input's keyboard layouts only, with no controller code. Recommended: libmanette. It is GNOME's own, reads the Deck's every button with or without Steam, and the keyboard fallback costs nothing.
2. B with the player's controls hidden. Options: close the player, as Back does on TV; or do nothing, with Menu then Close as the only way out. Recommended: close, keeping the position, the same as the TV. A press of B to leave a video is what every couch app does.
3. A larger scale in game mode. Options: the system's 1.0; 1.25 for text and targets while in game mode only. Recommended: 1.25 in game mode. The Deck sits at arm's length, and the app's 28 pt rows are small there.
4. Windows in the same release. Options: build the WinUI app's controller support right after, from the same map; leave Windows without it. Recommended: right after, as its own small task, since a Windows handheld is the same case.

## Sources

- libmanette documentation: <https://gnome.pages.gitlab.gnome.org/libmanette/doc/0.2/>, `ManetteDevice` signals: <https://gnome.pages.gitlab.gnome.org/libmanette/doc/master/ManetteDevice.html>
- libmanette's Steam Deck driver, lizard mode and permissions: <https://blogs.gnome.org/alicem/2024/10/>
- libmanette 0.2.11 to 0.2.13 release notes: <https://sources.debian.org/src/libmanette/0.2.13-1/NEWS>
- gtk-rs `gir` for bindings: <https://github.com/gtk-rs/gir>
- GTK 4 focus: `gtk_widget_child_focus`, `gtk_window_set_focus_visible`: <https://docs.gtk.org/gtk4/>
- gilrs: <https://docs.rs/gilrs>
- Hayase's gamepad handling, read for this spec: `interface/src/lib/modules/gamepad.ts`.
