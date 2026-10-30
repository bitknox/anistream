# anistream for Windows and Linux: design brief

This brief is for the agents who draw the desktop boards and for the agents who build the two apps
from them. Read the main brief first: everything in it holds on Windows and Linux unless this file
says otherwise (content, shared data, voice, Nagare, cover rules, the review decisions, the motion
tokens and what each motion means): `~/anistream-design/native-design-brief.md`. Then read
`android/docs/android-design-brief.md`, which shows how a platform takes the identity on without
copying the Apple screens. This brief does the same twice.

The desktop apps buy into their platforms the way the Apple and Android apps do. Windows gets
Fluent: Mica, NavigationView, the Windows 11 title bar, Windows Settings-style cards. Linux gets
GNOME: libadwaita's adaptive widgets, header bars, boxed lists and the HIG's wording. The identity
lives in the same few places as on the phones: the Obi & Silk colours, show titles in Zen Kaku
Gothic New, cover art as the hero, amber as the only accent, and Nagare.

## Johan's decisions (2026-10-23)

- Fully native on each platform. No Electron, Tauri, webview or bundled Chromium.
- Windows: WinUI 3 on the Windows App SDK, in C#. The engine comes in through UniFFI's C#
  bindings.
- Linux: GTK4 and libadwaita in Rust, calling the engine directly.
- One shared Rust crate, `anistream-mpv-host`, drives libmpv for both. On Windows the video is a
  `SwapChainPanel` fed by mpv's d3d11 composition mode; on Linux it is a `GtkGLArea` under
  `GtkGraphicsOffload`. Both put real native controls over the video.
- Feature parity with the Android and Apple apps, not pixel replication.
- Dark only at v1, as the other apps are today, whatever the system's theme. The Obi & Silk
  palette stays fixed: no accent colour taken from Windows or from GNOME. A light palette is kept
  for later in the appendix.

The brief's open questions, answered the same day:

- Light mode: dark only at v1, not following the system (see "Dark and high contrast").
- Windows backdrop: Mica tinted to the ground, on the title bar and navigation pane only.
- Linux narrow navigation: the sidebar collapses into an overlay; no bottom view switcher bar.
- Linux selection: libadwaita's neutral fill, not the Mac's amber tint.
- Linux settings: a Settings page in the sidebar, as on the Mac and Windows, not a preferences
  dialog.
- Closing the window while downloads or seeding run: the app asks first, keep going or stop (see
  "Closing the window while downloads run").
- Linux mini player: offered, though Wayland cannot keep it on top.
- Windows search: in the title bar only, with no Search item in the pane.
- The Windows jump list and a GNOME Shell search provider: after v1, with widgets.
- Subtitle style on Windows: follows the system's caption settings until the user sets one in the
  app.

## What carries over and what each platform replaces

| From the anistream design | Windows | Linux |
|---|---|---|
| Obi & Silk palette, amber as the only accent | Kept. Amber replaces the Windows accent colour in the app's resources | Kept. Amber replaces the libadwaita accent through its CSS variables |
| Glass chrome (Apple) | Replaced by Mica, tinted to the ground, on the title bar and navigation pane only | Replaced by solid surfaces: GNOME has no translucent chrome |
| Show titles in Zen Kaku Gothic New 700 | Kept, font bundled with the app | Kept, font bundled in the AppImage |
| UI type (SF on Apple, Roboto Flex on Android) | Segoe UI Variable and the Fluent type ramp | The system font (Adwaita Sans on GNOME) and libadwaita's type classes |
| Metadata in spaced caps (11/500, 0.08em) | Replaced by Fluent's Caption style in haze, no caps | Replaced by `.caption` and `.caption-heading`, no caps (the HIG avoids all caps) |
| Mac title case for controls | Replaced by sentence case everywhere ("Resume episode 7 · 10:06", "Join store…") | Sentence case for buttons, menu items, headings, page titles, row labels, check boxes and body text, as the catalogue writes them ("Resume episode 7 · 10:06", "Join store…") |
| Cover art as the hero, 2:3 covers, 16:9 banners, flat placeholders in the cover's colour | Kept | Kept |
| Nagare, static, integer cells, empty states, first launch and About | Kept | Kept, inside `AdwStatusPage` empty states |
| Voice and the legal line "anistream hosts no content." | Kept | Kept |
| Motion tokens, no bounce | Kept, mapped to Composition springs and Fluent page transitions | Kept, mapped to `AdwSpringAnimation` and `AdwTimedAnimation` |
| SF Symbols, Material Symbols | Segoe Fluent Icons | The GNOME symbolic icon set, 16 px |
| Mac menu bar and ⌘K palette | No menu bar (Fluent apps have none). The title bar's search box finds titles, settings and commands | No menu bar. A primary menu in the header bar, and search that finds titles and settings |
| Touch targets 44 (Apple), 48 (Android) | 32 epx for mouse-first controls, 40 where touch is likely (player, title page action) | libadwaita's own control sizes; never below 32 px |

What never carries over: the iPhone tab bar, the Android navigation bar as a phone pattern, sheets
that rise from the bottom on a wide window, and anything Apple draws as glass.

## Window sizes and boards

Windows measures in effective pixels (epx), which follow the display scale. Linux measures in
logical pixels, and libadwaita breakpoints in `sp`, which also follow the text scale. Every layout
below is given at 100% scale and must reflow, not only grow, as scale and text size go up.

| | Windows | Linux |
|---|---|---|
| Minimum window | 500×420 epx (fits a quarter snap on a 1920×1080 display at 125%) | 360×360, the HIG's adaptive minimum |
| Default window | 1280×800 | 1280×800 |
| Wide | Window from 1008 epx: NavigationView expanded, pane open at 240 | Window from 1000 sp: sidebar shown, 240 wide |
| Medium | 641 to 1007 epx: pane compact, a 48 epx icon rail | 600 to 999 sp: sidebar shown, 220 wide; inner splits collapse |
| Narrow | Under 641 epx: pane minimal, behind the menu button in the title bar | Under 600 sp: sidebar collapses to an overlay behind a sidebar button |
| Boards | 1280×800, 960×720, 560×800 | 1280×800, 860×680, 360×720 |
| Window chrome on boards | Draw the Windows caption buttons (minimise, maximise, close; 46×32 each, top right) in the title bar | Draw GNOME's default: a close button only, at the header bar's end |
| Remembered | Size, position and maximised state (`AppWindow`) | Size and maximised state only; Wayland does not let apps place windows |

Inner layouts switch on the content width, not the window width. A list with a preview column
needs 780 of content (the queue's 360 and at least 380 for the preview, as on the Mac); below that
the list stands alone and a row opens its title. On Linux the breakpoint conditions are on the
window, so they are those content widths plus the sidebar.

## Windows: Fluent mapping

### Backdrop and colour

The window uses a Mica backdrop through `MicaController`, tinted towards the ground: tint colour
#161A2E, tint opacity 0.8, fallback colour #161A2E. Mica shows only in the title bar and the
navigation pane. The content area is a solid layer in the ground colour, so covers and banners
always sit on the same blue, whatever the wallpaper. When the window is inactive, when
transparency effects are off, or on battery saver, Mica falls back to the solid ground by itself.
No Acrylic in the app's own surfaces; menus and flyouts keep the system's in-app Acrylic, tinted
the same way, as the main brief keeps system menus on Apple.

The app sets these theme resources in its dark `ThemeDictionary`:

| Fluent resource | Hex | Our token |
|---|---|---|
| `SolidBackgroundFillColorBase`, `LayerFillColorDefault` | #161A2E | ground |
| `CardBackgroundFillColorDefault` | #222638 | raised (setting cards, the Continue setup card) |
| `ControlFillColorDefault` | #2A2F45 | buttons, text boxes at rest |
| `ControlFillColorSecondary` | #343A57 | raised selected (hover) |
| `ControlFillColorTertiary` | #222638 | pressed |
| `ControlStrokeColorDefault`, `CardStrokeColorDefault`, `DividerStrokeColorDefault` | #2F3242 | faint rule |
| `TextFillColorPrimary` | #E4E0D6 | silk |
| `TextFillColorSecondary` | #9298B5 | haze |
| `TextFillColorDisabled` | #5C6073 | |
| `SystemAccentColorLight2` (accent fill in dark) | #F2A64B | amber |
| `SystemAccentColorLight1` (accent pressed) | #DE9340 | |
| `SystemAccentColorLight3` (accent text) | #F7C98B | |
| `TextOnAccentFillColorPrimary` | #161A2E | ground on amber |
| `SystemFillColorCritical` | #E06B54 | vermilion |
| Progress track (`ProgressBar` background) | #434653 | rule |

Water has no Fluent role. It is the app's own brush for states (new episode, airing, synced,
downloading) and never stands in for Fluent's green success colour, which the app does not use.

Amber rules from the main brief hold. Fluent puts the accent on the selected navigation item's
pill, on toggles that are on, on the filled primary button, on progress and on the focus of a
`SelectorBar`; that set matches the brief, so the override is all it takes. One `AccentButtonStyle`
button per screen.

### Type

Segoe UI Variable, the Fluent type ramp. Boards draw it with the stack
`'Segoe UI Variable Text', 'Segoe UI', 'Noto Sans', sans-serif` and load Noto Sans from Google
Fonts as the stand-in on machines without Segoe.

| Fluent style | Size/line | Weight | Use |
|---|---|---|---|
| Title | 28/36 | 600 | page titles ("Continue", "Calendar") |
| Subtitle | 20/28 | 600 | section headings on a page ("New this week") |
| Body Large | 18/24 | 400 | the preview's episode line |
| Body Strong | 14/20 | 600 | list item titles that are not show titles, settings group headers |
| Body | 14/20 | 400 | body, list supporting text |
| Caption | 12/16 | 400 | metadata, in haze |

Show titles: Zen Kaku Gothic New 700 at 28/36 on the title page and in the Continue preview, 15/20
in rows and under covers. It scales with Windows text size as the ramp does.

### Shape and size

- Fluent radii: 4 for controls (`ControlCornerRadius`), 8 for cards, flyouts, dialogs and the
  banner (`OverlayCornerRadius`). Covers keep 6 up to 40 wide and 8 above.
- Buttons 32 high, the title page's primary action 40. Icon buttons 40×36 in the title bar.
- List rows: 40 for one line, 62 for a cover row with a 36×54 cover (as on the Mac).
- Setting cards 68 high for a title and a description, 48 for a title alone.
- Spacing on Fluent's 4 epx grid; page padding 36 left and right on wide, 16 on narrow.

### Components (use these, never invent a lookalike)

| Need | Windows | Replaces Apple's |
|---|---|---|
| Window frame | `TitleBar` control with the app icon, "anistream", a back button, the pane toggle and the search box, content extended into it | Mac toolbar |
| Top-level navigation | `NavigationView`, Left, LeftCompact or LeftMinimal by width | Mac sidebar |
| Back | The title bar's back button, Alt+Left and the mouse's back button | ⌘[ and swipe back |
| Sub-pages | `Frame` navigation; `BreadcrumbBar` on Settings sub-pages, as Windows Settings does | Pushed pages |
| Search | `AutoSuggestBox` in the title bar, centred, 320 to 480 wide; a search button on narrow | Sidebar search, ⌘K palette |
| Poster grids | `ItemsView` with `UniformGridLayout`, covers 140 epx wide at least | Poster grid |
| Lists | `ListView` with the Fluent selection pill | Mac lists |
| Segmented choice | `SelectorBar` | Segmented capsule |
| Choice of one from many | `DropDownButton` with a radio `MenuFlyout`, or `ComboBox` in forms | Pop-up menu |
| Context menus | `MenuFlyout` on right-click, Shift+F10 and the Menu key; `CommandBarFlyout` where there are 3 to 5 frequent actions | Context menus |
| Dialogs | `ContentDialog` (one primary button, amber) | Sheets |
| Settings rows | `SettingsCard` and `SettingsExpander` from the Windows Community Toolkit | Settings forms |
| Switches | `ToggleSwitch`, label on the left, no On/Off text | Switch |
| Notices in the window | `InfoBar` for lasting states (offline, an update ready), an in-app toast above the content's foot for passing ones | Notice capsule |
| Teaching | `TeachingTip` once, for the first torrent or the mini player | none |
| Progress | `ProgressBar`, amber on #434653 | Progress bar |
| Loading | Static placeholders in the cover's colour; `ProgressRing` only in the player's starting screen and inside a button | Spinner |
| Empty states | Nagare, a line and one action, centred on the ground | Same |

### Navigation

`NavigationView` holds the destinations, with a `NavigationViewItemHeader` "Sources" above the
last two:

- Continue, Calendar, Seasonal, Library, Downloads
- Sources: Providers, Plugin stores
- Footer: the account item ("Mika · AniList synced" with a water dot; it opens Settings ›
  Accounts) and Settings, the built-in gear.

Search is not a pane item. It lives in the title bar, where Windows 11 apps put it. Pane item
selection uses Fluent's amber pill. Changing destination is instant on wide windows (no page
transition between top-level destinations, as the Mac's M1); a title page or a sub-page opens with
Fluent's drill-in transition.

## Linux: libadwaita mapping

### Colour

The app loads its own stylesheet on top of libadwaita and sets the CSS variables libadwaita 1.6
and later read. It sets the colour scheme through `AdwStyleManager` and never ships a GTK theme.

| libadwaita variable | Hex | Our token |
|---|---|---|
| `--window-bg-color`, `--view-bg-color` | #161A2E | ground |
| `--window-fg-color`, `--view-fg-color` | #E4E0D6 | silk |
| `--headerbar-bg-color` | #161A2E | ground (header bars match the window, as GNOME does) |
| `--sidebar-bg-color` | #1C2034 | the sidebar, one step off the ground |
| `--card-bg-color` | #222638 | raised (boxed lists, cards) |
| `--popover-bg-color`, `--dialog-bg-color` | #2A2F45 | menus, popovers, dialogs |
| `--border-color` | #2F3242 | faint rule |
| `--accent-bg-color` | #F2A64B | amber |
| `--accent-fg-color` | #161A2E | ground on amber |
| `--accent-color` | #F2A64B | amber as a standalone colour (links are not amber; see below) |
| `--destructive-bg-color`, `--error-color` | #E06B54 | vermilion |

Secondary text uses an app class `.haze` (#9298B5) rather than `.dim-label`'s opacity, so it reads
the same over every surface. Water is an app class `.state` for the same states as elsewhere; it
does not replace `--success-color`. Links in body text are silk and underlined, never amber.

Selection follows libadwaita: the sidebar and lists draw the selected row with libadwaita's own
neutral fill. Amber stays on the suggested action (`.suggested-action`, one per screen), switches
that are on, check marks, progress and the timeline.

### Type

The system font: Adwaita Sans on GNOME, whatever the desktop sets elsewhere. Boards draw it with
Inter from Google Fonts, which Adwaita Sans is built from. Use libadwaita's classes, not fixed
sizes, so the text follows GNOME's Large Text setting.

| Class | Use |
|---|---|
| `.title-1` | page titles in empty states and the title page's show title (in Zen Kaku) |
| `.title-2` | the Continue preview's show title (in Zen Kaku) |
| `.title-4` | section headings on a page ("New this week") |
| `.heading` | list item titles that are not show titles, `AdwPreferencesGroup` titles |
| `.body` | body and row subtitles |
| `.caption`, `.caption-heading` | metadata, in `.haze` |
| `.numeric` | times, counts, sizes (tabular figures) |

Show titles: Zen Kaku Gothic New 700. In CSS they take relative sizes (`1.07em` in rows, `2em` on
the title page) so they follow the text scale. The font file ships in the AppImage under
`usr/share/anistream/fonts`, which `AppRun` hands to the app as `ANISTREAM_FONTS`.

### Shape and size

- libadwaita's radii: buttons 6, cards and boxed lists 12, the banner 12, dialogs and windows as
  the system draws them. Covers keep 6 up to 40 wide and 8 above.
- Buttons and header bar buttons are libadwaita's (34 high). The title page's primary action is a
  `.pill` button, 48 high.
- Rows: `AdwActionRow` and `AdwSwitchRow` at their own heights (50 and up); cover rows 62 with a
  36×54 cover.
- Content in `AdwClamp` where it reads as text (Settings, a plugin's page): 640 maximum.

### Components (use these, never invent a lookalike)

| Need | Linux | Replaces Apple's |
|---|---|---|
| Window | `AdwApplicationWindow` with an `AdwToolbarView` and `AdwHeaderBar` per pane | Mac window and toolbar |
| Top-level navigation | `AdwOverlaySplitView` with a sidebar list (`GtkListBox`, `.navigation-sidebar`) | Mac sidebar |
| List and preview | `AdwNavigationSplitView` inside the content (the libadwaita triple-pane pattern) | Mac split view |
| Pushed pages | `AdwNavigationView` and `AdwNavigationPage`; back button in the header bar, Alt+Left, the mouse's back button, swipe on touchpads and touchscreens | Push and swipe back |
| Search | A search button in the header bar (Ctrl+F), a `GtkSearchBar` with key capture so typing anywhere searches | Sidebar search, ⌘K palette |
| Poster grids | `GtkGridView`, covers 140 wide at least | Poster grid |
| Segmented choice | `AdwToggleGroup`; `AdwInlineViewSwitcher` where the choice swaps a view | Segmented capsule |
| Choice of one from many | `GtkDropDown`, or `AdwComboRow` in forms | Pop-up menu |
| Context menus | `GtkPopoverMenu` on right-click, long press, Shift+F10 and the Menu key | Context menus |
| Primary menu | A `GtkMenuButton` with `open-menu-symbolic` at the sidebar header's end: Sync Now, Join Store…, Export Lists and History…, Keyboard Shortcuts, About anistream | Mac menu bar |
| Dialogs | `AdwDialog` and `AdwAlertDialog`, which turn into bottom sheets on a narrow window | Sheets |
| Settings | A sidebar destination: `AdwPreferencesPage`s in the main window, with groups, `AdwSwitchRow`, `AdwComboRow`, `AdwSpinRow`, `AdwEntryRow`, `AdwExpanderRow` | Settings forms |
| Notices in the window | `AdwToast` for passing ones; `AdwBanner` under the header bar for lasting states (offline, an update) | Notice capsule |
| Progress | `GtkProgressBar` and `GtkLevelBar` (the storage meter) | Progress bar |
| Loading | Static placeholders in the cover's colour; `AdwSpinner` only in the player's starting screen and inside a button | Spinner |
| Empty states | `AdwStatusPage` with Nagare as its paintable | Same |
| About | `AdwAboutDialog` | About window |
| Shortcuts | `AdwShortcutsDialog`, Ctrl+? | Keyboard Shortcuts window |

### Navigation

The sidebar lists Continue, Calendar, Seasonal, Search, Library, Downloads, then a "Sources" header
over Providers and Plugin Stores. Its foot holds the account row ("Mika", "AniList synced" with a
water dot), which opens Settings on the Accounts pane, and under it Settings alone, as the Mac's
sidebar ends. Settings is a page of the main window, not a preferences dialog, so the primary menu
has no Preferences item; Ctrl+, selects Settings in the sidebar.

Under 600 sp the sidebar collapses into an overlay that slides in from the start edge, behind a
sidebar button at the start of every content header bar. Picking a destination closes it. Inner
split views collapse first (at the medium size), so a narrow window shows one column with pushed
pages.

## Dark and high contrast

Dark only at v1, whatever the system's theme. Windows sets `Application.RequestedTheme` to Dark
and the app's dark `ThemeDictionary` applies everywhere; the title bar's caption buttons follow it.
Linux sets `AdwStyleManager`'s colour scheme to `ADW_COLOR_SCHEME_FORCE_DARK`. The player is dark
on both, as it would be in any case. Boards draw dark only. Colours are still defined as theme
resources and CSS variables, never hard-coded in screens, so the light palette in the appendix can
come later without touching them.

High contrast replaces the palette, it does not adjust it:

- Windows contrast themes (Aquatic, Desert, Dusk, Night sky and the user's own) load a
  `HighContrast` theme dictionary that uses only the system colours: `SystemColorWindowColor`,
  `SystemColorWindowTextColor`, `SystemColorHighlightColor` and `SystemColorHighlightTextColor`
  for selection and the primary action, `SystemColorHotlightColor` for links,
  `SystemColorGrayTextColor` for disabled text. Mica turns off. Cover art and video stay as they
  are; every cover gets a 1 epx `SystemColorWindowTextColor` edge. The player's dim becomes solid
  bars in the window colour.
- GNOME's High Contrast (`AdwStyleManager:high-contrast`) keeps libadwaita's high-contrast style
  and adds the app's values under `@media (prefers-contrast: more)`: haze lifts to #B8BDD6, the
  rule to #5C6073 and the faint rule to #434653 (the Mac's Increase Contrast values), cards and
  covers get a 1 px silk edge at 0.5, and the player's dim goes to 60%.

## Screens

Each screen keeps the rules the main brief gives it. What follows is how each platform lays it
out at its three widths. Shared data (Mika, Tuesday 20 October 2026 at 19:30, Hoshikuzu no
Tabibito at episode 7 with 14 minutes left) is the same on every board.

### First launch

No modal, no carousel, no empty first screen. Continue opens filled with Airing today and This
season, under a setup card with Nagare (cell 4) and two rows: Connect AniList (optional) and Add a
plugin store, which says how anistream plays episodes.

- Windows: the card is a `CardBackgroundFillColorDefault` card with two `SettingsCard` rows inside
  it, each with a chevron. Join store opens a `ContentDialog`: the invite link box, the store's
  preview (name, who runs it, the key fingerprint, its plugins) and Join as the amber primary
  button. On narrow windows Nagare moves above the rows.
- Linux: the card is a boxed list (`.boxed-list`) under a heading, Nagare beside it on wide and
  above it on narrow. Join Store opens an `AdwDialog` with the same content, which becomes a
  bottom sheet on a narrow window.
- Both: pasting an invite link anywhere in the window (Ctrl+V) or opening an `anistream://join`
  link opens the same dialog, filled in.

### Continue

The queue first, at most five rows of In progress, then New this week. Each row says what plays:
"Resume E7 · 12:31" or "Start E8". Hide from Continue and Drop live in the row's context menu.
Offline shows a quiet line such as "2 changes waiting to sync".

- Wide, both platforms: the Mac's layout. The queue down the start edge, 360 wide, rows 62 with
  36×54 covers, grouped under In progress and New this week. Beside it the preview of the selected
  row: the banner at 16:9 of the column's width up to 360 high, the show title, the episode line,
  the progress bar with "14 min left", the one amber action ("Resume episode 7 · 10:06" on
  Windows, "Resume episode 7 · 10:06" on Linux) and the synopsis. Arrow keys move the selection;
  Enter plays; double-click or the banner opens the title page.
- Windows medium: the same two columns while the content is 780 or wider (it is at 960, with the
  48 epx rail). Linux medium: the inner split collapses; the queue fills the content and a row
  opens its title page.
- Narrow, both: the queue alone, full width, then New this week as rows, then Airing today and
  This season as horizontal rows of 16:9 banner cards (each show's banner, never an episode
  still). A row opens its title page.
- Windows: the queue is a `ListView`; the preview sits on the ground with no card around it. The
  context menu is a `MenuFlyout`: Resume, Mark watched, Hide from Continue, Drop, Open title.
- Linux: the queue is the sidebar of an inner `AdwNavigationSplitView` (its background stays the
  ground, not the sidebar colour, so it reads as content). The preview is its content page with a
  flat header bar. The same menu as a `GtkPopoverMenu`, in sentence case.

### Search

Before typing: recent searches, This season and the genres, all as covers or genre tiles. Results
are a poster grid with filters for format, season, genre and status. No results repeats the query
("Nothing found for “lantern road”"). Titles match romaji, English, native and synonyms.

- Windows: the title bar's `AutoSuggestBox` is the search. Typing shows up to six suggestions in
  its list (cover, show title, year), then a "Settings" group for matching setting names and a
  "Commands" group (Sync now, Join store…, Pause all downloads). Enter, or "See all results",
  opens the Search page with the full grid and the filter row (`DropDownButton`s, amber text when
  set). Focusing the empty box and pressing Enter opens the Search page in its before-typing
  state. Ctrl+F and Ctrl+E focus the box. On narrow windows the box becomes a search button that
  opens the box across the title bar.
- Linux: Search is a sidebar destination and also the header bar's search button. Typing anywhere
  in the window opens the `GtkSearchBar` and searches. The Search page shows the before-typing
  state, then the grid with the filters as a row of `GtkDropDown`s under the search bar. Matching
  Settings rows show at the end under "Settings".
- Grids: covers 140 to 180 wide, columns as many as fit with 16 between; on narrow, three columns
  at 360.

### Seasonal

This season's titles as a poster grid, popular first, with the season as a picker.

- Windows: the page title "Seasonal", then a `DropDownButton` with the season ("Fall 2026") and
  previous and next buttons beside it, and a `SelectorBar` for format (TV, Movie, OVA, ONA). The
  grid under it. Covers carry the title in Zen Kaku 15 and one caption line in haze ("Atelier
  Hotaru · 12 episodes"). Right-click: Add to Planning, Open title.
- Linux: the season is a `GtkDropDown` in the header bar's centre; format is an `AdwToggleGroup`
  under the header bar. Same grid and captions.
- Narrow, both: the format choice moves into the season picker's menu; the grid keeps three
  columns at 360.

### Calendar

The week, Monday first, with Mine and All airing as a choice. Delays read "(was 23:00)",
approximate times carry "~", and sub and dub air times are separate entries when they differ.
Narrow day columns use the shows' wide art across the column, not tiny posters.

- Wide, both: the Mac's week. Seven columns, today's on raised from heading to foot, each
  broadcast an entry with its banner across the column, the show title, "Ep 10 · 16:30" and a
  water dot when it has aired and is not watched. Watched episodes are one quiet line. The week
  ("19 to 25 October 2026") with previous and next buttons and the Mine and All airing choice.
- Medium, both: an agenda. Days as sections down the page, today first and marked, each broadcast
  a row with a 96×54 banner thumbnail, title, episode and time.
- Narrow, both: the agenda at full width, banner thumbnails 80×45.
- Windows: the week and its buttons in the page header; Mine and All airing as a `SelectorBar`.
- Linux: the week in the header bar's title ("19 to 25 October" with the year as subtitle),
  previous and next as header bar buttons, Mine and All airing as an `AdwToggleGroup` under it.

### Library

The tracker's lists: Watching, Planning, Completed, Paused, Dropped, each a poster grid with its
count. Order by last updated, title or score. Signed out, the Library is Nagare (cell 5), a line
("Your lists come from your tracker.") and one amber action, Connect AniList.

- Windows: a `SelectorBar` with the five lists and their counts ("Watching 12"), the order as a
  `DropDownButton` at the end, then the grid. Export lists and history sits in the page's "More"
  button.
- Linux: an `AdwInlineViewSwitcher` with the lists under the header bar, the order in a header bar
  menu button. Signed out, an `AdwStatusPage` with Nagare.
- Narrow, both: the list choice becomes a `DropDownButton` (Windows) or `GtkDropDown` (Linux) in
  place of the bar; grid at three columns.

### Title page

One amber action, labelled with what it plays. The banner under the title bar or header bar with
the ground over its lower part, where the cover and title sit. Seasons from AniList relations as a
season picker; movies and OVAs under Related. Language belongs to the episode and source ("Dub to
episode 6"). Spoiler protection blurs the stills of unwatched episodes after the next one.

- Wide, both: the banner across the content, up to 360 high, then two columns. The main column has
  the cover (120×180) beside the show title in Zen Kaku, the metadata line in haze ("TV · Summer
  2026 · 24 episodes · 79%"), the primary action, the watching status menu and More. Under them
  the season picker and the episodes: rows 72 high with a 128×72 still, "7 · The Lantern Road",
  duration or "14 min left" with progress, and a watched check at the end. The side column, 260
  wide, holds the details (studio Atelier Hotaru, genres, source in a haze line), trackers, and
  Related as small covers.
- Medium, both: one column. The details move under the episodes; Related becomes a row of covers.
- Narrow, both: the banner at full width with no cover beside the title; the primary action at full
  width; episode stills 96×54.
- Windows: the status is a `DropDownButton` ("Watching"), More is a `MenuFlyout` (Show settings…,
  Mark all previous watched, Download…, Play trailer, Open in AniList). Episode rows show a play
  button and a More button on hover and on keyboard focus, and act on Enter. The season picker is
  a `ComboBox` ("Season 1 · Cour 2"). Show settings is a `ContentDialog` with audio and subtitles,
  auto-skip, spoilers and source.
- Linux: the status and More are header bar menu buttons on narrow and buttons beside the action
  on wide. Episode rows activate to play and carry a menu button at the end; nothing shows only on
  hover, since GNOME runs on touchscreens too. Show Settings is an `AdwDialog` with an
  `AdwPreferencesGroup` per part.
- Marking an episode watched: the check fills, the row's text dims to haze (quick token), the count
  updates.

### Downloads

Grouped by title, each episode with its state (Queued, Downloading, Paused, Done, Failed), its
size and its actions. A storage meter. Seeding episodes show their ratio and time left. Keep the
next N episodes of a show; quality Standard or Higher.

- Windows: the storage meter under the page title ("41 GB of 220 GB free · downloads take 18 GB"),
  Pause all and Open folder as buttons at the end. Titles as `SettingsExpander`-like groups: the
  cover, the name, "3 episodes · 2.1 GB", and the episodes inside. The bar is water while it runs,
  haze while it waits. The taskbar button shows overall progress while anything downloads.
- Linux: the meter is a `GtkLevelBar` in a boxed list at the top; Pause All and Open Folder in the
  header bar. Titles as `AdwExpanderRow`s with the episodes as rows inside.
- Narrow, both: one column, actions move into each row's menu.

### Providers and Plugin Stores

A provider row has one quiet line (the store) and speaks only for a problem, an update or off.
Each store has its own page; a plugin's page shows its version, response time and hosts. The
update confirmation names new hosts ("This update also contacts open-archive.hotaru.example and 1
more"). Amber only on Update. The legal line, "anistream hosts no content.", sits at the foot of
both lists in haze.

- Windows: lists of `SettingsCard`s with the store's name as the description; the order is changed
  by drag or by Move up and Move down in each card's menu. A store's page uses a `BreadcrumbBar`
  ("Plugin stores › Hotaru Plugins").
- Linux: boxed lists of `AdwActionRow`s pushed into `AdwNavigationPage`s; drag to reorder with a
  handle, and Move Up and Move Down in each row's menu.

### Settings

The panes: Playback, Downloads, Notifications, Sources, Accounts, Integrations and Advanced, then
About. Integrations (watch together and its server and room, Discord presence and whether it shows
the title) and the Mac-only rows (start fullscreen, after each download, check for updates) exist
on both desktops. Advanced holds the mpv options the terminal's config.toml allows. Rows that shows
override say "Overridden for 3 shows". Tracker tokens live in the system's store (Windows
Credential Manager, the Secret Service or its portal), so token storage is not a setting, as on
the Mac.

- Windows: Settings fills the content, styled as Windows 11 Settings. Wide: the panes as a list
  down the start of the page (a second, inner `NavigationView` is not used; it is a `ListView`
  with icons), the pane's `SettingsCard`s in the middle, and, at 1280 and wider, the Mac's preview
  column on the end showing the selected row's effect where it has one (upscaling shows the 480p
  stream against its 1080p original; subtitles show a caption sample). Medium: the pane list and
  cards, the preview drops under the selected card. Narrow: the pane list is a page; a pane is a
  sub-page with a `BreadcrumbBar`. Ctrl+, opens Settings.
- Linux: Settings is the sidebar's last destination (Ctrl+, selects it), laid out as Windows
  lays it out. Wide: an inner `AdwNavigationSplitView` with the panes as a `.navigation-sidebar`
  list (220 wide, icons and names in sentence case), the pane as an `AdwPreferencesPage`
  in an `AdwClamp` of 640, and, at 1280 and wider, the Mac's preview column on the end. Medium:
  the pane list and the pane; rows whose effect needs a picture (upscaling, subtitle style, night
  mode) open a subpage that holds it. Narrow: the pane list is a page and a pane is pushed over
  it, with the header bar's back button. Settings search is the window's own search, which lists
  matching rows under "Settings".
- Both: turning on the torrent source opens a confirmation dialog first, with the main brief's
  connection-check copy; No protection and Warn only sit behind the same kind of acknowledgement.
- The download folder: Windows `Videos\anistream` by default, picked with the system folder
  picker. Linux the XDG Videos folder's `anistream` by default, picked through the file chooser
  portal.

### About

- Windows: Settings › About, with Nagare (cell 6) beside the version, Check for updates, the legal
  line, the licence notices (the app, Zen Kaku Gothic New's OFL, the CC BY-NC assets, libmpv's
  LGPL) and links.
- Linux: `AdwAboutDialog` with the app icon, version, the legal line in its comments, and the same
  notices in its legal sections. Nagare is not in the dialog (its art slot is the app icon); he
  stays in the empty states.

## The player

### Where it plays

The player plays inside the main window, over the page it came from, as the Mac's does. The
navigation pane and sidebar hide; the title bar or header bar becomes the player's top bar. Closing
it (Esc, the back button, Alt+Left) returns to the page with the progress updated (standard token).
The player is dark in every system theme.

Video is the bottom layer: the `SwapChainPanel` on Windows, the `GtkGLArea` under
`GtkGraphicsOffload` on Linux. The controls are native controls above it. When the controls have
faded out they are collapsed (Windows `Visibility.Collapsed`, GTK unmapped), not left at zero
opacity, so the compositor can scan the video out directly with nothing over it.

### Chrome over video

The main brief's player rules hold: controls on a 35% dim of the ground over the whole frame, no
blur behind the seek bar or the subtitles, the middle of the picture clear, controls stay up while
paused.

- Top bar: back, the show title in Zen Kaku with "Episode 7 · The Lantern Road" under it, then at
  the end subtitles and audio, episodes, the mini player, fullscreen. On Windows the caption
  buttons stay at the top right while the controls show, and hide with them. On Linux the header
  bar uses the `.osd` style class over the dim and keeps the window's close button.
- Bottom bar: play and pause, back 10 s, forward 10 s, the timeline with the opening and ending
  marked as segments on the track, the elapsed and remaining time, volume, speed and a settings
  menu (source, quality, upscaling, night mode). The timeline's played part is amber; the buffered
  part is silk at 0.3; the track is the rule colour. Hovering the timeline shows a seek preview
  (the second mpv handle's frame) with the time under it.
- Subtitles are never under the bottom bar: while it shows, the host raises them by the bar's
  height through mpv, with no animation.
- Windows: the bars are XAML `Grid`s in the `SwapChainPanel`, menus are `MenuFlyout`s, the
  timeline is a `Slider` with a custom track. Linux: an `AdwToolbarView` with
  `extend-content-to-top-edge` and `extend-content-to-bottom-edge`, revealing its bars; menus are
  popovers; the timeline is a `GtkScale` with marks.
- Corner cards, at the bottom end over the dim or alone when the controls are hidden: Skip opening
  (one per segment), Up next (Play now in amber, Cancel, the 10 s countdown bar, "Plays in 8 s"),
  the stall prompt (try another source or keep waiting). "Still watching?" after three episodes in
  a row with no input is a centred card that pauses playback.
- The starting screen: the banner art full frame, "Starting episode 7" and a 32 px `ProgressRing`
  (Windows) or `AdwSpinner` (Linux) until the first frame fades in.

### Mouse, touch and pen

- Moving the pointer shows the controls (fade-in); they fade out after 2 s idle and the pointer
  hides with them, never while the pointer is over a bar, while scrubbing or while a menu is open.
- Click on the video plays or pauses; double-click toggles fullscreen.
- Vertical wheel steps the volume; horizontal scrolling (a tilt wheel or a touchpad) seeks.
- Right-click on the video opens the playback menu: play or pause, skip opening, next episode,
  subtitles and audio, speed, mini player, fullscreen.
- Mouse back button closes the player, as Alt+Left does.
- Touch (Windows tablets, Linux touchscreens): a tap toggles the controls instead of pausing;
  double-tap on either third seeks 10 s with a running count, as on the phones.

### Keyboard

The companion's keys, the same on both platforms. Arrow keys and S act without showing the
controls; every other key shows them.

| Key | Action |
|---|---|
| Space, K | Play or pause |
| Left, Right | Back or forward 5 s; with Shift, 30 s |
| J, L | Back or forward 10 s, as the buttons |
| Up, Down | Volume up or down 5 |
| M | Mute |
| F, F11 | Fullscreen |
| Esc | Close a menu, then cancel Up next or the stall prompt, then leave fullscreen, then close the player |
| Enter | Take the corner card's action (skip, play now, try another source) |
| S | Skip the current segment; U undoes it |
| N, Shift+N | Next or previous episode |
| E | Episodes |
| T | Subtitles and audio |
| I | Stats for nerds |
| [ and ], Backspace | Slower, faster, normal speed |
| P | Mini player |
| Alt+Left | Close the player |

Windows keys are `KeyboardAccelerator`s on the player page; Linux keys are a
`GtkShortcutController` on the player. Neither app grabs keys globally; media keys come through the
media session (below).

### Fullscreen and the mini player

- Windows: fullscreen is `AppWindow`'s full-screen presenter, never the swapchain's (composition
  swapchains cannot go full screen). The taskbar hides; leaving restores the window's size and
  position. The mini player is the compact overlay presenter: an always-on-top window, 16:9, with
  play and pause, back and forward 10 s and a button back to the full window. The first time,
  a `TeachingTip` points at the mini player button.
- Linux: fullscreen is the window's own (`gtk_window_fullscreen`), on the monitor the window is on.
  The mini player is a small undecorated window, 480×270, with the same three controls. Wayland
  lets no app keep itself on top, so the mini player says nothing about it; GNOME and KDE both let
  the user pick Always on Top from the window menu.
- Both: "Start fullscreen" in Settings › Playback opens every episode fullscreen.

## Keyboard outside the player

| Action | Windows | Linux |
|---|---|---|
| Destinations | Ctrl+1 to Ctrl+7 | Ctrl+1 to Ctrl+7 |
| Search | Ctrl+F, Ctrl+E | Ctrl+F, or start typing |
| Back | Alt+Left, mouse back | Alt+Left, mouse back |
| Settings | Ctrl+, | Ctrl+, |
| Sync now | Ctrl+R, F5 | Ctrl+R |
| Mark watched | Ctrl+Shift+M | Ctrl+Shift+M |
| Hide from Continue | Delete | Delete |
| Context menu | Shift+F10, Menu key | Shift+F10, Menu key |
| Keyboard shortcuts | Ctrl+/ opens a `ContentDialog` listing them | Ctrl+? opens `AdwShortcutsDialog` |
| Close and quit | Alt+F4 closes the window | Ctrl+W closes the window, Ctrl+Q quits |

Windows also shows access keys when Alt is held, on the pane items and the title page's buttons.
Every control is reachable with Tab and arrow keys on both; focus uses the platform's focus
visual, never a custom ring.

## Motion

Same tokens as the main brief, mapped to each platform. Springs are critically damped: no bounce,
no overshoot.

| Token | Windows | Linux |
|---|---|---|
| quick | `SpringScalarNaturalMotionAnimation`, damping ratio 1, period 200 ms | `AdwSpringAnimation`, damping ratio 1, mass 1, stiffness 1000 |
| standard | Same, period 300 ms | Same, stiffness 440 |
| fade-in | 200 ms, decelerate easing | `AdwTimedAnimation`, 200 ms, `ease-out-cubic` |
| fade-out | 300 ms, accelerate easing | 300 ms, `ease-in-cubic` |
| linear | Linear key frames (Up next's 10 s countdown only) | `linear` easing |
| system | Fluent page transitions, flyouts, dialogs, window presenters | libadwaita's navigation, dialogs, popovers, split views |

The periods give the same stiffness as Android's `spring(1f, 1000f)` and `spring(1f, 440f)`.

- Cover or row to title page: Windows uses a connected animation (`ConnectedAnimationService`)
  with the direct configuration, no gravity, from the cover to the title page's cover, and
  drill-in for the rest. Linux uses `AdwNavigationView`'s own push; no shared element.
- Continue selection to preview: the preview crossfades (fade-in), skipped when the selection
  moves again within 0.15 s (M2).
- Player chrome: fade only, never a slide; corner cards fade with the standard token.
- Up next at zero: the last frame fades through the ground colour over 0.25 s, never black.
- Reduce motion: Windows' "Animation effects" off (`UISettings.AnimationsEnabled`) turns the
  connected animation and drill-in into crossfades and keeps fades. GNOME's "Reduce Animation"
  (`gtk-enable-animations` off) makes libadwaita's animations jump to their end, fades included;
  the app does not fight it.

## Accessibility

- Screen readers: Narrator and NVDA through UI Automation on Windows (`AutomationProperties.Name`
  on every icon button, headings marked with `HeadingLevel`, notices announced with
  `LiveSetting`), Orca through AT-SPI on Linux (`GtkAccessible` labels and roles, notices through
  `gtk_accessible_announce`). Covers are named by their show; decorative art is hidden from both.
- The player names its state: "Playing, 10:06 of 24:00". Skip and Up next cards stay for the whole
  segment and Up next waits for input while a screen reader is running, as on Apple.
- Scaling: Windows display scale (100% to 300% and up) and text size (100% to 225%) separately.
  Text scaling grows rows and wraps labels; nothing truncates a show title to one line at 225%
  except in grids, which take two lines and a tooltip. Linux fractional scaling and Large Text
  (1.25): the breakpoints are in `sp`, so a bigger text size moves a window into its narrower
  layout sooner, as it should.
- Nagare stays crisp at any scale: the cell is chosen in device pixels (the sprite is drawn as
  rectangles at whole device pixels, nearest-neighbour), so at 125% a cell of 4 epx is 5 px.
- Subtitles: on Windows the player follows the system's caption settings (Settings ›
  Accessibility › Captions: font, size, colour, background), mapped to mpv's subtitle options,
  unless the user sets a style in the app. Linux has no system caption style, so the app's own
  subtitle settings apply.
- Contrast: see "Dark and high contrast". Text over art always has the dim or solid ground
  behind it.
- Pointer targets: 32 epx (Windows) and 32 px (Linux) at least; 40 for player buttons and the
  title page's action.

## Platform integrations

| Concern | Windows | Linux |
|---|---|---|
| Media session | System Media Transport Controls through the window interop (works unpackaged): title "Episode 7 · The Lantern Road", artist the show, the banner as thumbnail, timeline, play and pause, next and previous | MPRIS (`org.mpris.MediaPlayer2.anistream`): the same metadata, `mpris:artUrl` pointing at the cached banner, seek and position, Raise brings the window forward |
| Media keys | Through the media session, so they work with the window in the background | GNOME and KDE route media keys to MPRIS; no grab |
| Taskbar or dock | A thumbnail toolbar on the taskbar preview (previous episode, play and pause, next episode); taskbar progress for downloads (paused and failed states too) | Nothing beyond MPRIS; GNOME has no dock badges |
| Notifications | App notifications: "Episode 10 is out" with the show as body, the banner as hero image, buttons Play and Open | `GNotification` through the portal: the same text, the cover as icon, a Play button; the default action opens the title |
| Keeping awake | A display request while playing | `gtk_application_inhibit` for idle and suspend while playing |
| Running with the window closed | Only after the user chooses Keep going (below): a notification area icon (Open anistream, Pause all downloads, Quit) | Only after Keep going: the Background portal, so GNOME lists anistream under Background Apps; on KDE a StatusNotifierItem with the same menu |
| Single instance | `AppInstance` key, later launches redirect their arguments | `GApplication` over D-Bus |
| Deep links | `anistream://` registered under `HKCU\Software\Classes` by the installer | `x-scheme-handler/anistream` in the `.desktop` file |
| Updates | Velopack: an `InfoBar` "Version 1.4 is ready. Restart to update." and a row in About | The AppImage downloads and checks the new version, puts it in place and restarts into it (`updater.rs`); in a folder it cannot write to, it saves the file to Downloads and says so |

Deep links are the routes the other apps answer: `anistream://auth/anilist` and the other
trackers' sign-in returns, `anistream://join/…` for store invites, `anistream://title/…`,
`anistream://play/…` and `anistream://settings/accounts`. Notification actions go through the same
activation path.

Tracker sign-in, both platforms:

1. Settings › Accounts › AniList › "Sign in…" opens the default browser.
2. The row reads "Waiting for the browser…" with a Cancel button.
3. The tracker redirects to `anistream://auth/anilist`; the running instance takes it, comes
   forward, and the row turns into "Mika · synced 19:30", with a toast "Signed in to AniList".
4. If Windows refuses to bring the window forward and flashes its taskbar button instead, the toast
   is still there when the user switches back. On Linux the portal passes an activation token, so
   the window comes forward.
5. A tracker that cannot take a custom scheme uses the engine's `127.0.0.1` listener instead; the
   browser page it serves says the sign-in is done and the tab can be closed.

### Closing the window while downloads run

Closing the window with nothing running quits. Closing it while episodes download or seed asks
first; the app never keeps running without saying so.

- Windows: `AppWindow.Closing` is cancelled and a `ContentDialog` opens. Title "Downloads are still
  running". Body "2 episodes are downloading and 1 is seeding. Keep them going after the window
  closes, or stop them?" Buttons: Keep going (primary, amber), Stop and close, Cancel. A check box
  under the body, "Don't ask again", off by default. Keep going closes the window and adds the
  notification area icon; the first time, a notification says "anistream is still downloading.
  It's in the notification area." Stop and close pauses everything and quits; the downloads
  resume at the next launch.
- Linux: the window's `close-request` is stopped and an `AdwAlertDialog` opens, which is a bottom
  sheet on a narrow window. Heading "Keep downloads going?". Body as on Windows. Responses: Cancel,
  Stop and close, Keep going (suggested, amber). A "Don't ask again" check button as the dialog's
  extra child. Keep Going asks the Background portal; if the user or the desktop refuses, a toast
  in the still-open window says "anistream can't keep running in the background here" and the
  dialog's choice falls back to Stop and Close. On KDE the StatusNotifierItem appears.
- Both: the choice can be changed later in Settings › Downloads, "When the window closes with
  downloads running": Ask, Keep going, Stop. Ask is the default. Quitting (the notification area
  icon's Quit, or Ctrl+Q on Linux) stops everything without asking, since quitting is the
  answer.

### After v1

Where the Apple apps have Spotlight, the Linux app gets a GNOME Shell search provider for titles in
the user's lists, and the Windows app a jump list with the Continue queue. Both come after v1,
with widgets.

## Board names

Windows (1280×800 unless noted): WindowsFoundations (the mappings above, drawn), WindowsHome,
WindowsHomeMedium (960×720), WindowsHomeNarrow (560×800), WindowsWelcome, WindowsJoinStore,
WindowsTitle, WindowsTitleNarrow (560×800), WindowsShowSettings, WindowsSearch,
WindowsSearchSuggestions, WindowsSeasonal, WindowsCalendar, WindowsCalendarMedium (960×720),
WindowsLibrary, WindowsDownloads, WindowsProviders, WindowsStore, WindowsSettings,
WindowsSettingsPlayback, WindowsSettingsSources, WindowsTorrentDialog, WindowsAbout, WindowsPlayer
(1280×720), WindowsPlayerUpNext (1280×720), WindowsMiniPlayer (480×270), WindowsCloseDialog,
WindowsHighContrast, WindowsMotion.

Linux (1280×800 unless noted): LinuxFoundations, LinuxHome, LinuxHomeMedium (860×680),
LinuxHomeNarrow (360×720), LinuxWelcome, LinuxJoinStore, LinuxTitle, LinuxTitleNarrow (360×720),
LinuxShowSettings, LinuxSearch, LinuxSeasonal, LinuxCalendar, LinuxCalendarNarrow (360×720),
LinuxLibrary, LinuxDownloads, LinuxProviders, LinuxStore, LinuxSettings, LinuxSettingsPlayback,
LinuxSettingsSources, LinuxSettingsNarrow (360×720), LinuxTorrentDialog, LinuxAbout, LinuxPlayer
(1280×720), LinuxPlayerUpNext (1280×720), LinuxMiniPlayer (480×270), LinuxCloseDialog,
LinuxHighContrast, LinuxMotion.

## Appendix: the light palette (later)

Not in v1. Kept so a light appearance can follow the system later without new design work. The
light tokens, checked for contrast against the light ground (body text 4.5:1, graphics 3:1):

| Token | Dark | Light | Light contrast on ground |
|---|---|---|---|
| ground | #161A2E | #F4F1EA | |
| raised | #222638 | #FFFFFF | |
| raised selected | #343A57 | #E6E1D6 | |
| faint rule | #2F3242 | #DCD7CB | |
| rule (progress track) | #434653 | #8F8A7D | 3.1 |
| text (silk, then ink) | #E4E0D6 | #161A2E | 15.2 |
| haze | #9298B5 | #575D7C | 5.7 |
| amber fill, with ground-ink text on it | #F2A64B | #F2A64B | ink on amber 8.5 |
| amber as a line, check or timeline | #F2A64B | #A35C0C | 4.6 |
| water | #7FC7D9 | #2B7488 | 4.7 |
| vermilion | #E06B54 | #B0412C | 5.1 |

On Windows, the light `ThemeDictionary` takes these values, with `SystemAccentColorDark1` at
#F2A64B (accent fill in light) and `SystemAccentColorDark2` at #A35C0C (accent text). Mica's tint
moves to #F4F1EA. On Linux, the app's stylesheet sets the light values under
`@media (prefers-color-scheme: light)`, with `--accent-color` at #A35C0C. The player stays dark
in a light appearance.
