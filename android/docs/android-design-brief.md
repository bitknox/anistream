<!-- A copy of ~/anistream-design/android-design-brief.md, the brief the Android boards were drawn
     to, with its paths pointed at ~/anistream-design. The boards are in
     ~/anistream-design/design/project/Android*.dc.html. -->

# anistream for Android: design brief for canvas agents

Read the main brief first: everything in it holds on Android unless this file says otherwise
(content, data, voice, legal lines, Nagare, cover rules, the review decisions, the motion tokens
and what each motion and haptic means):
`~/anistream-design/native-design-brief.md` (its own paths to /private/tmp are stale: the canvas
folder is `~/anistream-design/design/project/`)

The Android app buys into the platform the way the Apple apps do: Android's structure, components
and conventions, with the identity in the same few places (Obi & Silk colours, show titles in
Zen Kaku Gothic New, cover art as the hero, Nagare). The Android notes section of the UX doc
already maps behaviour to Compose; read it:
https://claude.ai/artifact/B2EqQjf8J2bDE9w4NSbXSa (Claude Docs; if you can't read it, the mapping
is repeated below where it matters).

## Johan's decisions (2026-10-20)

- Material 3 Expressive components and shapes, with calm motion: `MotionScheme.standard()`, never
  `expressive()`. No overshoot, no bounce, same as Apple.
- The Obi & Silk palette stays fixed (no dynamic colour from the wallpaper). Dark first; light
  follows the system.
- Phones plus tablets and foldables. No TV yet.

## Canvas sizes

- Phone: 412×915 (compact width). Top system bar inset 40, bottom gesture bar inset 24 (draw no
  fake status bar, no fake gesture pill; leave the space). Edge to edge: art may run under the top
  inset, text never.
- Tablet: 1280×800 landscape (expanded width), same insets.
- Orientation: on phones (smallest width under 600 dp) the browse UI is portrait only and the
  player is landscape, so a phone never returns from the player into a landscape title page.
  Tablets and foldables rotate freely.

## Colour roles (fixed ColorScheme, tonal elevation off)

| M3 role | Hex | Our token |
|---|---|---|
| primary | #F2A64B | amber |
| onPrimary | #161A2E | ground |
| primaryContainer | #4A3A24 | amber at low tone, for the selected nav indicator and filter chips |
| onPrimaryContainer | #F7C98B | |
| secondary | #7FC7D9 | water (states) |
| error | #E06B54 | vermilion |
| surface / background | #161A2E | ground |
| surfaceContainerLow | #1C2034 | |
| surfaceContainer | #222638 | raised (cards, sheets, nav bar) |
| surfaceContainerHigh | #2A2F45 | menus, dialogs |
| surfaceContainerHighest | #343A57 | text field fill, tonal button fill |
| onSurface | #E4E0D6 | silk |
| onSurfaceVariant | #9298B5 | haze |
| outline | #434653 | |
| outlineVariant | #2F3242 | hairlines |

Amber rules from the main brief hold: one filled amber action per screen, progress, selection,
switches on. Selection uses primaryContainer: the navigation indicator pill (filled #F7C98B icon,
amber label), the selected segment of a connected button group (with a check) and selected
FilterChips. Tonal buttons fill #343A57.

## Type (Roboto Flex for the UI; Zen Kaku Gothic New 700 for show titles only)

Google Fonts link: `https://fonts.googleapis.com/css2?family=Roboto+Flex:opsz,wght@8..144,400..700&family=Zen+Kaku+Gothic+New:wght@400;500;700&display=swap`
Stack: `'Roboto Flex', Roboto, system-ui, sans-serif`.

| Role | Size/line | Weight | Use |
|---|---|---|---|
| headlineMedium | 28/36 | 400 | large top app bar title ("Continue") |
| titleLarge | 22/28 | 400 | small top app bar title, sheet titles |
| titleMedium | 16/24 | 500 | list item headline, section headers |
| bodyLarge | 16/24 | 400 | body, list supporting text when it leads |
| bodyMedium | 14/20 | 400 | supporting text |
| labelLarge | 14/20 | 500 | buttons, tabs |
| labelMedium | 12/16 | 500 | nav bar labels, chips |
| labelSmall | 11/16 | 500 | metadata caps (0.06em) |

Show titles: Zen Kaku 700 at 22 on title pages, 16 in rows.

## Shape and size

- Expressive shape scale: 4, 8, 12, 16, 28 and full. Cards 16, sheets 28 top corners, dialogs 28,
  buttons full (pill), chips 8, text fields 4 top (filled) or full (search bar). Covers keep 6/8.
- Touch targets 48 dp minimum (Android), not 44.
- Buttons: 40 high (filled, tonal, outlined, text); hero action 56 (Expressive large button) on
  title pages; icon buttons 48. Switch 52×32. FilterChip 32 visual in a 48 target.
- Lists: one-line 56, two-line 72, three-line 88; covers 40×60 in rows.

## Components (use these, never invent a lookalike)

| Need | Android | Replaces Apple's |
|---|---|---|
| Top-level navigation, phone | Navigation bar (Expressive short bar, 64 high, icon above label, indicator pill primaryContainer) with Continue, Calendar, Seasonal, Library, Search | iPhone glass tab bar + search circle |
| Top-level navigation, tablet | Navigation rail (Expressive, 96 wide, expanded to a 220 modal drawer on demand) | Mac/iPad sidebar |
| List and detail | List-detail pane (ListDetailPaneScaffold) on tablets; single pane on phones | Mac split view |
| Screen title | Large flexible top app bar that collapses to small on scroll | Large title |
| Back | Predictive back (system gesture; title page shrinks toward its cover as you swipe) and a back arrow in the top bar | Swipe back, zoom |
| Sheets | Modal bottom sheet with drag handle, 28 corners, scrim | iOS sheets |
| Menus | Dropdown menu on surfaceContainerHigh, 4 corners | System menus |
| Segmented choice | Connected button group (Expressive), selected = primaryContainer | Segmented capsule |
| Filters | FilterChip row | Filter capsules |
| Search | Search bar at the top of the Search destination (full shape), expanding to a search view | Bottom search field |
| Notices | Snackbar above the nav bar, action = NoticeHint | Notice capsule |
| Progress | Linear progress indicator, amber on #434653 (plain, not wavy) | Progress bar |
| Loading | Static placeholders as on Apple; the Expressive loading indicator only inside a button or a pull-to-refresh. The one exception: the player's starting screen shows an indeterminate CircularProgressIndicator, 32 dp, until the first frame | Spinner |
| Settings | Full-screen settings page, grouped list items with switches on the trailing side; tablets get list-detail with the preview column like the Mac | iPhone Settings sheet |
| Player controls | Over a 35% dim: top bar (back arrow, title, subtitles, cast is not offered, episodes), centre clear, bottom bar with play/pause, ±10 s and the seek slider (Expressive slider, amber active track) | AVKit-style bar |
| Picture in Picture | System PiP with play/pause, ±10 s actions | Mac PiP |
| Media notification | Media3 session notification (not drawn) | Now Playing |

No glass anywhere: bars and sheets are solid surface containers.

## Motion on Android

Same tokens, mapped (the UX doc's table): quick `spring(1f, 1000f)`, standard `spring(1f, 440f)`,
fade-in/out tweens, linear for countdowns. Cover to title uses a shared element
(`SharedTransitionLayout`); back is predictive back; the player opens as a shared-bounds transition
from the banner and closes with predictive back or a drag down.

## Board names (prefix Android)

Phone: AndroidHome, AndroidTitle, AndroidShowSettings, AndroidPlayer, AndroidPlayerUpNext,
AndroidCalendar, AndroidSearch, AndroidLibrary, AndroidDownloads, AndroidWelcome,
AndroidJoinStore, AndroidSettings, AndroidSettingsPlayback, AndroidSettingsSources,
AndroidTorrentSheet, AndroidProviders, AndroidStore.
Tablet: AndroidTabletHome, AndroidTabletTitle, AndroidTabletSettings.
System: AndroidFoundations (the mapping above, drawn), AndroidMotion.
