<!-- The brief the TV boards and the :tv screens are drawn and built to. Written 2026-10-23 on
     branch tv/design-brief, from tv/foundations. It sits beside android-design-brief.md and
     motion-android.md, which it builds on. -->

# anistream for Android TV and Google TV: design brief

Read the main brief first (`~/anistream-design/native-design-brief.md`), then the Android brief
(`android-design-brief.md`) and the Android motion spec (`motion-android.md`). Everything in them
holds on TV unless this file says otherwise: content, data, voice, the legal line, Nagare, the cover
and hero art rules, the review decisions, the motion tokens and what each motion means.

The TV app buys into the TV platform the way the phone app buys into Android: Compose for TV
structure and components, focus as the platform draws it, and the identity in the same few places
(Obi & Silk colours, show titles in Zen Kaku Gothic New, cover art as the hero, Nagare). It is a
whole app, not a casting target. The person sits three metres away with a five-button remote, and
often with a phone in their hand; both are first-class ways in.

## Johan's decisions (2026-10-23)

- A separate APK, the `:tv` module, `tv.anistream.tv` (fixtures: `tv.anistream.tv.fixtures`).
  Android 11 (API 30) and up, `armeabi-v7a` and `arm64-v8a`.
- It shares `:engine`, `:store`, `:design` and `:player` with the phone app. It does not use `:ui`
  or `:player-ui`; its screens and the player's screens live in `:tv`.
- The first release is the full app: Continue, Search, Seasonal, Calendar, Library, title pages,
  Settings and the player, driven by the D-pad, with tracker sync and plugins.
- Phones and the other anistream apps are remotes through the engine: Play on TV, remote controls,
  and typing search text from the phone.
- TV boxes like the test device get no upscaling.

## The test device

Chromecast with Google TV (4K), measured 2026-10-23. Every budget in this file is set against it.

| What | Measured |
|---|---|
| CPU and userspace | 32-bit ARM (`armeabi-v7a` only), 2 GB RAM, Mali-G31 |
| UI | renders at 1080p (960 × 540 dp at density 2) on the 4K panel |
| Zero-copy MediaCodec | 1080p H.264 and 10-bit HEVC with no dropped frames |
| Heavy styled subtitles | 40 blurred, moving signs drop about 13 % of frames, for that stretch only |
| Copy-back decoding | about 2 fps, unusable |
| High-quality scalers | drop about 21 fps |

So the UI is light: no blur, no glow, no shadows, no gradients, motion on `graphicsLayer` only, and
memory watched on every screen. Stronger boxes (Nvidia Shield, Google TV Streamer) get the same
design; nothing is added for them in the first release.

## Canvas and safe area

- Board size 960 × 540 (dp; the UI at 1080p is exactly this at density 2). Draw art for
  1920 × 1080.
- The safe area is Google's 5 %: 48 left and right, 27 top and bottom. Text, focusable items and
  their focus rings stay inside x 48 to 912 and y 27 to 513. Art and backgrounds bleed to the
  edges. Most panels no longer overscan, but some still do, and a cut-off focus ring is a lost
  remote press.
- Content keyline x 124: every screen's rows, grids and headings start there. The navigation rail
  sits in x 48 to 96.
- Grid: 6 poster columns of 118 with 16 gaps (124 to 912), or 4 columns of 16:9 cards of 185 with
  16 gaps. A row whose cards run past x 912 bleeds them to the screen edge; focus never lands on a
  card that is partly outside the safe area, because the row scrolls first.
- Draw no system UI. The TV has no status bar or gesture bar; the system's own overlays (volume,
  the Google TV panel) are not ours.

## Ten-foot rules

- Dark only. The fixed Obi & Silk scheme, no light theme, no dynamic colour. TVs crush near-black,
  so separation comes from surface steps (ground #161A2E, raised #222638, high #2A2F45), never from
  hairlines: #2F3242 on the ground is 1.4:1 and vanishes on most panels. No hairlines on TV.
- Silk #E4E0D6 is the brightest thing on screen. Never pure white, which blooms on TV panels.
- Contrast floor 4.5:1 for all text. Measured: silk on ground 13.0, silk on #222638 11.4, silk on
  #343A57 8.4, haze on ground 6.0, haze on #222638 5.3, haze on #2A2F45 4.6, amber on ground 8.5,
  water 9.1, vermilion 5.2. Haze is the quietest text there is: it never sits on #343A57 (3.9),
  and on panels (#2A2F45) it is 14 sp or larger. Anything a person must read to act is silk.
- Text over art always has a solid ground or the player's 35 % dim behind it. Most screens put text
  beside or below art, not on it.
- Nothing moves on its own: no autoplaying trailers, no carousel that advances, no idle animation.
- No text under 12 sp. Lines no longer than 60 characters.

## Type (TV roles)

Roboto Flex for the UI and Zen Kaku Gothic New 700 for show titles only, as on the phone, set larger
for distance. Sizes in sp on the 960 × 540 canvas.

| Role | Size/line | Weight | Use |
|---|---|---|---|
| displaySmall | 36/44 | 400 | device codes (pairing, sign-in) |
| headlineSmall | 24/32 | 400 | panel and dialog titles, empty-state lines |
| titleLarge | 22/28 | 500 | page headings (Settings sections), Up Next title |
| titleMedium | 18/24 | 500 | row headers, list item headlines |
| bodyLarge | 16/24 | 400 | body, synopsis, supporting text that leads |
| bodyMedium | 14/20 | 400 | supporting text, card second lines |
| labelLarge | 16/20 | 500 | buttons, tabs, rail labels, chips |
| labelMedium | 14/18 | 500 | metadata, times in the player |
| labelSmall | 12/16 | 500 | metadata caps, tracked 0.06 em (the smallest text anywhere) |

Show titles: Zen Kaku 700 at 36/44 on the title page and the Continue hero, 22 on the Up Next card
and the player header, 16/20 in lists, 14/18 under posters and cards (two lines, then an ellipsis).
No marquee: the focused card's full title is in the row's header line or the title page.

## Colour roles on TV

The phone's table holds. TV adds the focus roles that tv-material asks for:

| tv-material role | Hex | Use |
|---|---|---|
| border (focus ring) | #E4E0D6 | silk, 3 dp, the focus ring on art |
| inverseSurface (focused container) | #E4E0D6 | a focused button, chip, list row or rail item fills silk |
| inverseOnSurface | #161A2E | text and icons on a focused container |
| primary | #F2A64B | the one amber action, progress, switches on |
| primaryContainer / onPrimaryContainer | #4A3A24 / #F7C98B | selection: the current destination, the selected tab, the chosen option |
| scrim | #000000 at 32 % | under the expanded rail, panels over browse screens and dialogs |

Focus is silk and selection is amber. A focused selected item shows focus (silk fill) with a small
amber dot before its label, so neither meaning is lost. Amber rules from the main brief hold: one
amber action per screen, progress, selection, switches on; status text is never amber.

## Shape and size

- Covers 2:3, radius 8. 16:9 cards and episode stills radius 8. Banners bleed full width with no
  radius (they meet the screen edge). Buttons and chips full (pill), panels 16 on the inner edge,
  dialogs 28.
- Focus targets at least 48 × 48. Buttons 48 high (the amber hero action 56), icon buttons 48,
  list rows 56 (one line) and 72 (two lines), chips 40.
- Spacing on the 4 grid: 16 between cards, 28 between rows (header to header 28 plus the row), 12
  from a row header to its cards.
- Elevation 0 everywhere. No shadows: Mali-G31 pays for every one, and on a dark TV they are not
  seen anyway.

## Components (Compose for TV, checked 2026-10-23)

Google's current TV stack is `androidx.tv:tv-material` 1.1.0 (stable since May 2026) on Compose
foundation. tv-foundation's own lazy layouts were removed in 2025: rows and grids are Compose's
`LazyRow`, `LazyColumn` and `LazyVerticalGrid`, with focus pivots through `BringIntoViewSpec`.
Google's TV guide makes the navigation drawer, collapsed to a rail, the backbone of an app's
structure, with five or six destinations, every item an icon and a short label, and up to three
actions at the bottom.

The `:tv` theme wraps `androidx.tv.material3.MaterialTheme` with the values from `:design`
(colours, type, shapes are one source). Use these, never a lookalike:

| Need | TV component | Replaces the phone's |
|---|---|---|
| Top-level navigation | `ModalNavigationDrawer`: a 96-wide rail of icons, expanding to a 240 drawer over the content with a scrim | Navigation bar and rail |
| Cards | tv-material `Card` / `ClassicCard` with our focus values (below), `Glow.None` | Cards, rows |
| Buttons | tv-material `Button`, `OutlinedButton`, `IconButton`, `WideButton` (settings) | M3 buttons |
| Tabs | tv-material `TabRow` and `Tab` (Library, Calendar days, Seasonal seasons) | Connected button group |
| Filters | tv-material `FilterChip` | FilterChip row |
| Lists | tv-material `ListItem`, `DenseListItem` | List items |
| Switches, choices | tv-material `Switch`, `RadioButton`, `Checkbox` | M3 switch |
| Panels | our side panel: a solid #2A2F45 column from the right, 400 wide, full height | Bottom sheets |
| Dialogs | Compose `Dialog` holding our surface, 28 corners, scrim | M3 dialogs |
| Menus | a side panel (long press on a card); TV has no anchored dropdowns | Dropdown menus |
| Notices | a toast-like notice top right (below), never at the bottom where the player's bar lives | Snackbar |
| Progress | linear, amber on #434653, 4 high (6 in the player) | Linear progress |
| Loading | static placeholders; a 32 dp circular indicator only on the player's starting screen and while buffering | The same |
| Text entry | our search keyboard, the system keyboard for everything else | Search bar |

Not used: tv-material `Carousel` (a promotional hero that advances on its own; Continue has
nothing promotional above the queue), `ImmersiveList`-style backgrounds that change with focus (a
decode per focus move), glow, and any blurred background.

## Focus

### The look

| Element | Unfocused | Focused | Pressed |
|---|---|---|---|
| Poster, 16:9 card, episode still | art, title below in silk | scale 1.05, silk ring 3 dp at 3 dp outside the art; title below stays silk | scale 1.0 |
| Small card (Calendar entry, chip-sized) | as above | scale 1.1, the same ring | scale 1.0 |
| Button (tonal) | #343A57 fill, silk label | silk fill, ground label, scale 1.05 | scale 1.0 |
| The amber action | amber fill, ground label | amber fill, silk ring 2 dp at 2 dp outside, scale 1.05 | scale 1.0 |
| List row, rail item, tab, chip | no fill, silk label (haze supporting) | silk fill, ground label and icon; no scale | the same |
| Selected (current option, destination, tab) | #4A3A24 fill, #F7C98B label | silk fill, ground label, amber dot | |
| Disabled | 38 % opacity, still focusable where it explains itself | silk ring only | |

- Google's default scales are 1.025, 1.05 and 1.1; we use 1.05 for anything 100 dp or wider and 1.1
  for smaller cards. The hero banner and full-width rows never scale.
- No glow, no shadow, no lift, no colour wash on focus. The ring and the scale carry it.
- Scale and the ring animate on quick (`spring(1f, 1000f)`), on `graphicsLayer`, so a focus move
  never re-lays out a row. When a key is held (repeat count above 0), the scale snaps instead of
  animating, so a run of twenty presses a second never falls behind.
- Rows leave room for the scale: 6 dp of padding above and below the art inside the row, so the
  ring is never clipped by the row's bounds.

### The model

- Exactly one focused item on screen at all times while the browse UI shows. The player with its
  chrome hidden has no visible focus.
- Every screen names its first focus (listed per screen below). It is the thing most people want:
  the amber action on a title page, the hero's Resume on Continue, the first result in a grid.
- Focus groups: the rail, each row, each grid, each panel, the player's chrome. Directional moves
  stay geometric inside a group and cross between groups only at its edges.
- Rows remember their last focused item (`focusRestorer`): moving up or down into a row returns
  to the card it last had, its first card if it never had one. Grids do not remember; up and down
  stay in the same column.
- Returning to a screen (Back from a title page, closing a panel) puts focus back on the item that
  opened it, with the scroll position unchanged. A list that changed while away (an episode
  marked watched) keeps focus on the same id, not the same index.
- Nothing wraps. Right at the end of a row, Up at the top of a page, Down at the end do nothing,
  and play no sound.
- Focus never jumps on its own, except in three places: the skip offer while the chrome is hidden,
  the Up Next card, and a phone's Search submit (which moves focus to the first result). Content
  arriving late (art, a list) never moves focus.
- A pointer (a mouse, or a pointer remote) hovers with the focus look and clicks what it hovers.
  The manifest declares `android.software.leanback.supports_touch` as false for the first release.

### D-pad rules

| Key | Browse screens |
|---|---|
| Up, Down, Left, Right | Move focus. Left from the leftmost item of any row or grid opens the rail. Right from the rail returns to the content's last focused item. |
| Center (select) | Activate. On a card: open its title page. On the rail: go to that destination. |
| Center, long press | On a card or episode: its actions panel (Mark watched, Hide from Continue, Drop, Open the title page). On the rail: nothing. |
| Back | See Back, below. |
| Play/pause, rewind, fast forward (where the remote has them) | On a card or title page: play what the amber action plays. Otherwise nothing. |
| Search key, mic or Assistant key | Belong to the system (Gemini or Assistant on Google TV). The app does not receive them. |

The rail is reached only with Left, never with Up. Moving through the rail does not switch screens:
Center does. Switching on focus would load a screen for every item passed, which a 2 GB box should
not pay for.

### Back at every level

| Where | Back does |
|---|---|
| A panel, dialog, menu or the expanded rail | Closes it, focus returns to what opened it |
| The system keyboard | Closes the keyboard (the keyboard's own), focus stays in the field |
| Deep in a browse screen (any row but the first, or scrolled) | Scrolls to the top and focuses the screen's first focus |
| At the top of a browse screen | Opens the rail with focus on the current destination |
| The rail, on any destination but Continue | Goes to Continue, rail closed, Continue's first focus |
| The rail, on Continue | Leaves the app to the TV's home screen. No "Exit?" dialog. |
| A title page, a store page, a settings page | Returns to where it was opened from, focus on the opener |
| The player | See the player's Back rules |
| Pairing and sign-in screens | Cancel and return; a code in progress is abandoned |

Back never needs a long press, and nothing intercepts the Home key.

## Navigation

A collapsed rail on the left, expanding over the content as a modal drawer.

- Collapsed: 96 wide, no background (icons on the ground or the hero art's ground strip), icons 24
  in 48 × 48 pills centred at x 72. The current destination's icon is #F7C98B; the others haze.
  With nothing focused in it, the rail shows icons only and never a label.
- Expanded (Left from content, or Back at the top of a screen): a 240-wide #222638 drawer over the
  content, the scrim black at 32 % over the rest, items with icon and label (labelLarge), the
  current destination in primaryContainer, the focused one in silk. Focus opens it on the current
  destination. It closes on Right, Back or Center.
- Items, top to bottom: Search, then Continue, Calendar, Seasonal, Library (the phone's order),
  then at the bottom Settings. Six in all, inside Google's five-or-six. No logo, no profile, no
  badges.
- Continue is the start destination. Search sits first because Google's TV anatomy puts it in the
  drawer's top section, not because it is the start.
- A paired phone that is connected shows nowhere in the rail. Its presence is in notices (below)
  and in Settings › Remotes.
- Switching destinations fades through on quick (T6 parity): the outgoing screen fades out and is
  disposed, the incoming one fades in. Only the current destination is composed.

## Notices

The phone's snackbar moves to the top right on TV, since the bottom belongs to the player's bar and
to row content.

- Position: top right, its right edge at x 912, top at y 27, max 360 wide, #2A2F45 with 12
  corners, bodyMedium silk, an optional 20 dp icon on the left.
- Never focusable and never an action. Anything that needs an answer is a dialog or lives in the
  screen. The phone's NoticeHint actions become a line in the relevant screen.
- Stays 4 s, plus 1 s per 10 words past 12. One at a time; a newer one replaces the shown one.
- Used for: "2 changes synced to AniList", "Mika's phone connected", "Paused from Mika's phone",
  "Subtitles: English · Full", "Signed in to AniList as Mika".

## Screens

Every browse screen has the rail on the left and its content from x 124. The loading rule is T8's:
nothing for 600 ms, then static placeholders in the real layout (covers as flat rects in their muted
colour, text as silk bars at 12 %), held at least 0.3 s, then one crossfade on standard. Art fades in
on fade-in as it arrives. No shimmer, no pulse, no stagger. Errors name what failed and offer one
way forward. Nagare appears only in empty states, the first-run card, pairing and About.

### Continue (TVContinue, TVWelcome)

The queue first, nothing promotional above it, at most five rows.

- Hero, y 0 to 320. The next episode's show: its banner (the hero art rule: AniList banner, else a
  spoiler-safe still, else a compact header with the poster beside the title) bleeds full width
  from y 0 to 200 with no text on it. Below on the ground: the show title (Zen Kaku 36) at x 124,
  y 212; the line "Episode 7 · The Lantern Road · 14 min left" (bodyLarge, haze); the amber
  "Resume episode 7" (56 high) with a tonal "Title page" beside it.
- Rows below, each a header (titleMedium) and cards:
  - Continue watching: 16:9 cards (185 × 104) of each show's banner, never an episode still, with
    the show title and "Resume E7 · 10:06" or "Start E8" below, and a 4 dp amber progress bar
    along the card's bottom edge.
  - New this week: 16:9 cards, "Start E9 · out 2h ago", a water dot for new.
  - Airing today: 16:9 cards with the air time ("16:30", "~23:00", "(was 23:00)").
  - This season: posters (118 × 177).
- The page scrolls by row: once focus leaves the hero, the hero scrolls away and the focused row's
  header sits at y 48, with the next row's header showing below. Inside a row the focused card
  stays at the content keyline once the row has scrolled.
- First focus: the hero's Resume. Down goes to the first row. Up from the first row returns to
  Resume.
- Long press on a card: actions panel with Mark episode 7 watched, Hide from Continue, Drop
  (keeps progress), Open the title page.
- Offline: a quiet line under the hero, "Offline. 2 changes waiting to sync." (haze, a dot), and
  the rows from the cache.
- Loading: the hero's text and the rows as placeholders after 600 ms. The banner region stays the
  ground until art arrives.
- Empty queue (signed in, nothing watching): no hero. Airing today and This season lead, from
  AniList's public data, with the setup card if it is not dismissed.
- Empty everything (no network, nothing cached): Nagare (cell 6) on the ground at the centre,
  "anistream can't reach AniList right now." and a tonal "Try again", focused.

First run (TVWelcome). No modal, no carousel. Continue opens filled with Airing today and This
season, with the setup card where the hero would be:

- A #222638 card, x 124 to 912, 200 high, 16 corners. Nagare (cell 5) on the left on the plain
  ground of the card's inset, then three steps as tonal buttons in a row, each with a one-line
  note below:
  1. "Pair your phone": "Then the phone can sign in and add plugins for this TV."
  2. "Connect a tracker": "Your lists and progress, from AniList and others."
  3. "Add a plugin store": "How anistream plays episodes. anistream ships no sources of its own and
     hosts no content. Every source is external."
- A done step shows a water check and its result ("Signed in as Mika"). First focus: the first
  step not done. A "Hide" text button at the card's right end; the card also goes once a tracker
  and a store are done. Settings has all three again.

### Search (TVSearch, TVSearchPhone)

- Layout: the query field across the top (x 124 to 912, y 27, 56 high, #222638, full shape),
  our keyboard below it on the left (x 124 to 412), results on the right (x 444 to 912).
- Before typing: the right side shows Recent searches (chips), Genres (chips) and This season
  (posters, 3 columns of 118).
- As text arrives, results replace it, 300 ms after the last change: a poster grid, 3 columns,
  titles under posters. Titles match romaji, English, native and synonyms (engine). No results:
  "Nothing for "hoshikuzu tabi"." with the query repeated, and This season below it.
- Our keyboard: a 6-column grid of 48 × 48 keys: a to z, then 1 to 0, then a row of wide keys
  (Space, Delete, Clear). Keys are tonal; focus fills silk. Holding Center on Delete deletes
  repeatedly. Right from the keyboard's right column moves to the results; Left from the results'
  left column returns to the keyboard on the same row.
- Below the keyboard, a tonal "System keyboard" opens Android's IME into the field, for kana and
  kanji, accents, and the keyboard's own voice key where it has one (Gboard on Google TV does).
- Voice: a mic key on our keyboard appears only when `SpeechRecognizer.isRecognitionAvailable()`
  is true. The first use asks for the microphone (`RECORD_AUDIO`); declined, the key goes away and
  stays gone. Listening shows "Listening…" in the field and fills it with what was heard. The
  remote's mic or Assistant button opens the system assistant, never us, so the design never tells
  people to press it.
- Typing from a phone (TVSearchPhone): when a paired phone types, the TV opens Search if it is
  elsewhere in the browse UI (never during playback), and the field fills as the letters arrive,
  with a line under it: "Typing on Mika's phone" (phone icon, haze). Focus stays where it was. Both
  the TV's keyboard and the phone edit the same text; the newest change wins. The phone's Search
  (enter) moves focus to the first result and the line goes. Results follow the same 300 ms rule.
- First focus: the keyboard's "a". With recent searches, it is still "a": a search screen opens to
  type.
- Back: from results to the keyboard, from the keyboard to the rail (the Back table holds; the
  field's text stays until the app leaves Search).
- Loading: result placeholders after 600 ms. Error: "Search isn't reachable right now." in the
  results area, keyboard still usable.

The system's own search (Google TV's search bar, the global searchable provider) is not wired in
the first release: Google TV answers from its own catalogue, not from apps' providers.

### Seasonal (TVSeasonal)

- Top: a TabRow of seasons, "Spring 2026", "Summer 2026", "Fall 2026", "Winter 2027", the current
  one selected. Tabs select on focus after focus rests 250 ms (so passing through does not load
  each). Below it, a row of FilterChips: Format (TV, Movie, ONA, OVA), Sort (Popular, Score, Air
  day), "On my list" (when signed in).
- Grid: posters, 6 columns of 118, the title under each (two lines), a haze line "24 eps · 79 %".
  A water dot on titles airing now. Titles already on a list carry their status as a small chip
  on the poster's bottom left ("Watching").
- First focus: the first poster. Up from the first grid row to the chips, then to the tabs.
- Center opens the title page; long press offers Add to Planning, Open the title page.
- Loading: tabs and chips at once (local), the grid as placeholders after 600 ms. Empty season (a
  future season not announced yet): Nagare and "Nothing announced for Winter 2027 yet."

### Calendar (TVCalendar)

A week as day tabs, and one day's schedule as a list, so text stays large.

- Top: a TabRow of days, "Today", "Wed 21", "Thu 22" and so on for seven days, today selected;
  select on focus after 250 ms. On the right of the tab row, a FilterChip pair "My list" and "All
  airing" (My list when signed in, All airing otherwise).
- The day: a list of 72-high rows, one per entry in air order. Each row: the time at x 124 (labelLarge,
  "16:30", "~23:00", or "16:30 (was 23:00)" in haze for delays), a 16:9 still or banner crop
  (96 × 54, radius 8) at x 220, the title (Zen Kaku 16), "Episode 10" as a second line, "Episode 10 · Dub"
  for a dub (a sub is not marked), a water "Airing" or "Aired" state on the right, and a progress chip
  ("E7 of 24") for shows on a list. Sub and dub air times are separate rows when they differ.
- The row for the next airing on today is the first focus; past rows sit above it, dimmed to haze.
- Left and Right on a row move to the previous or next day at the nearest time, without going
  back up to the tabs. Left on the first day's list opens the rail.
- Center opens the title page. Long press: Open the title page, Add to Planning. There are no
  reminders on TV, since it posts no notifications.
- Empty day: "Nothing airs on Wednesday." (no Nagare, it is a normal state). Not signed in with My
  list: the chip shows All airing and is not changeable until sign-in, with a line "Sign in to see
  only your shows".

### Library (TVLibrary, TVLibraryEmpty)

- Top: a TabRow of the tracker's lists, Watching, Planning, Completed, Paused, Dropped, each with
  a count ("Watching 12"). Select on focus after 250 ms. On the right, a tonal "Sort" opening a
  panel (Recently updated, Title, Score, Air day).
- Grid: posters, 6 columns, title under each, the progress line "7 / 24" and a 4 dp amber bar on
  the poster's bottom edge for Watching.
- First focus: the first poster of Watching.
- Long press: Change status (panel of the five lists), Mark next episode watched, Open the title
  page.
- Empty, not signed in: Nagare (cell 6), "Your lists live in a tracker. Sign in to see them here."
  and a tonal "Connect a tracker", focused, which opens Settings › Accounts.
- Empty list: "Nothing in Paused." in haze, no Nagare, focus stays on the tab.
- Sync state: a quiet line under the tabs only when something waits, "2 changes waiting to
  sync". A good sync shows nothing.
- No Downloads on TV (decided), so the Library has no Downloads tab.

### Title page (TVTitle, TVShowSettings)

- Header, y 0 to 300: the banner bleeds full width from y 0 to 200 (the hero art rule; compact
  header with the poster at x 124 when there is no banner or safe still). Below on the ground: the
  title (Zen Kaku 36) at x 124, a metadata line in labelSmall caps ("TV · SUMMER 2026 · 24 EPS ·
  79 % · ATELIER HOTARU"), and the action row:
  - the amber action, labelled with what it plays: "Resume episode 7", "Play episode 1", "Play
    episode 10 (out Thursday)" disabled until it is out;
  - tonal "Watching" (the list status, opening a panel of the five lists, or "Add to list");
  - tonal icon button "More" (panel: Show settings, Mark all watched up to here, Hide from
    Continue, Drop).
- Seasons: when AniList relations give more than one entry, a row of chips ("Cour 1", "Cour 2",
  "Movie" under Related) above the episodes, each with its progress. Language belongs to the
  source: a note like "Dub to episode 6" sits on the episode row, never a season.
- Episodes: a row of 16:9 episode cards (185 × 104): the still, "E7 · The Lantern Road", "24 min"
  or "14 min left", the amber progress bar, a water check on watched ones (dimmed to haze text).
  Stills of unwatched episodes after the next one are blurred by decoding them at 24 × 14 px and
  drawing that scaled up with filtering (Android 11 has no RenderEffect, and this costs nothing).
  Titles stay. Long shows get range chips above the row ("1 to 12", "13 to 24").
- Related (movies, OVAs, sequels): a row of posters. Details: synopsis (bodyLarge, at most 6
  lines; Center on it opens the full text in a panel), studio, genres, source.
- First focus: the amber action. Down: seasons, then episodes (focus on the next episode to play,
  not episode 1), then Related, then Details.
- Long press on an episode: Play from the start, Mark watched, Mark watched up to here, Choose
  source (panel, as in the player).
- Show settings (TVShowSettings): a panel with Audio, Subtitles, Skip opening automatically, Skip
  ending automatically, Spoilers, Source, the per-show choices of the review decisions.
- Loading: header text and rows as placeholders after 600 ms; the banner area stays ground until
  art arrives. Missing sources: the amber action says "Add a plugin store to play" and opens the
  store flow.

### Settings (TVSettings and pages)

Two panes, following Android TV's own settings: sections on the left (x 124 to 380), the section's
rows on the right (x 412 to 912). Moving through sections shows their rows; Right goes into them.

- Sections: Playback, Sources, Plugin stores, Accounts, Remotes, This TV, About.
- Rows: 56 or 72 high ListItems, the label silk, the current value haze on the right, switches on
  the trailing side. A row with choices opens a panel of radio rows from the right; the choice
  applies at once and the panel closes on Center.
- Playback: translation, subtitles, quality, auto-play next, skip opening automatically, skip
  filler automatically, mark opening and ending, counts as watched at, night mode. Rows overridden
  by shows say "Overridden for 3 shows". Upscaling is not shown on TV (decision). Start fullscreen
  does not apply and is left out.
- Sources: source order (Center on a source picks it up, Up and Down move it, Center drops it,
  Back puts it back where it was), each provider with one quiet line (its store) and a word only
  for a problem, an update or off. A provider's page holds its version, hosts and response time.
  Then the phone's Torrents group: "Torrent source" (a switch; turning it on opens TVTorrents
  first), and while on "Torrent index", "Protect with" (a radio panel; another choice opens
  TVTorrents on it), "Torrent index" opens a screen of that name for the index alone, "Edit proxy…" while a proxy is set (its address and user name), "Last check" (its status with a vermilion dot for a failed check or a water
  dot for a pass, where traffic leaves from, Center checks now) and "If a check fails" (Warn only
  is confirmed first). Under it the check's note, or "Off by default. Turning this on asks how to
  protect your connection first." While on, a Seeding group: seed after watching, seed for at
  most, upload limit, and "Best effort on this TV: seeding runs only while anistream is open."
- Plugin stores: one row per store, "Add a plugin store", and each store's page (plugins, Update
  with the amber, "This update also contacts …").
- Accounts: AniList, Kitsu, Simkl, Trakt, each with its state ("Signed in as Mika",
  "Last synced 2 min ago" or "Not signed in"), Sign in or Sign out, and Sync now.
- Remotes: the TV's name for phones ("Living room TV", from the system's device name, editable),
  "Pair a phone", paired phones with last seen ("Mika's phone · connected", "Ren's tablet · last
  seen yesterday") and Remove on each, and "Let phones on this network find this TV" (on).
- This TV: "Show anistream on the home screen" (Watch Next, on), "Clear the image cache (84 MB)".
- About: version, check for updates (the update installs with the system's own confirmation),
  the legal line, licences, Nagare (cell 4).
- Rows that only exist elsewhere (downloads, download folder, discord presence, syncplay, torrents
  on cellular) are left out, not greyed.
- First focus: Playback. Loading: the section list is local and shows at once.

## Signing in and adding plugins without a browser

The TV has no browser, so sign-in comes from a phone or a code. Each tracker's Sign in opens
TVSignIn, which offers the ways that tracker allows, best first:

| Tracker | From a paired anistream phone | A code on any phone | Typed on the TV |
|---|---|---|---|
| AniList | the phone's account, confirmed on the phone | AniList's PIN page, pasted into the TV's sign-in page | no |
| Kitsu | the phone's account | the TV's sign-in page (username and password form) | yes, with the system keyboard |
| Simkl, Trakt | the phone's account | the service's device code (engine `DeviceFlow`) | no |

MyAnimeList is left out of the TV's Accounts: its redirect must match the one address it has
registered, so a TV has no way to finish its sign-in.

- From a phone (first, when a paired phone is signed in to that tracker): "Use Mika's phone", with
  "AniList as Mika" below. Center asks the phone; the phone shows "Sign in Living room TV to AniList
  as Mika?" with Allow; on Allow the engine hands the token over the paired, encrypted channel. The
  TV shows a 32 dp indicator and "Waiting for Mika's phone", then the notice "Signed in to AniList
  as Mika" and returns.
- Device code (Simkl, Trakt): the code in displaySmall (36) silk, spaced in two groups ("8F3K
  2QPL"), the service's activation address in labelLarge, a QR code (180 × 180, silk on ground,
  quiet zone 4 modules) of the address with the code filled in where the service supports it,
  "Expires in 9 min" counting down, and a tonal "Get a new code". The TV polls; approval returns to
  Accounts with the notice.
- The TV's sign-in page (AniList PIN, Kitsu): while TVSignIn is open, the engine serves a one-page
  form on the local network. The TV shows a QR code of its address (`http://192.168.0.24:47110/…`
  with a one-time path) and the steps: "Scan with your phone. Open AniList there, allow anistream,
  then paste the code on this page." The page takes the code (or Kitsu's username and password) and
  the TV signs in. The page stops when TVSignIn closes or after 10 minutes.
- Every such page (sign-in, Join store, torrents) is sealed: the address ends in `#k=` and a
  one-time key, which a browser never sends. The page encrypts what is typed with that key
  (AES-256-GCM, in the page itself, as plain HTTP has no `crypto.subtle`) and the TV decrypts it,
  so nothing typed crosses the network readable. The address under the QR code is shown whole,
  key included. Opened without its key, the page says "Scan the code again"; a form not sealed
  with the key is turned away.
- Two ways side by side at most: the left column the phone, the right the code. First focus: the
  phone's button when there is one, else nothing to press (the code waits), with focus on "Get a
  new code".

Adding a plugin store (TVJoinStore):

- From a paired phone: the phone lists its stores with "Add to Living room TV"; it asks each store
  for a "link another device" link and sends it to the TV. Open stores send their open link. The
  TV shows the store preview (name, owner "Ren", key fingerprint "9f3c 1a2b 77de 0c41", plugins
  with their hosts, whether anyone with the link can join) and an amber Join. Several stores queue,
  one preview at a time ("1 of 3").
- On the TV: "Add a plugin store" offers "Send a link from your phone" (the pairing flow if no
  phone is paired), the sign-in page's QR for pasting a link from any phone, and "Type the link"
  with the system keyboard as the last resort.
- The legal line sits under every Join: "anistream ships no sources of its own and hosts no
  content. Every source is external."

Turning torrents on (TVTorrents), from Settings › Sources:

- The phone's turn-on sheet as a screen: "Turn on torrents" (or "Protect torrent traffic" when
  changing), the line on why, then "Protect with" as D-pad radio rows on the left (SOCKS5 proxy,
  VPN on this device, No protection, each with its note; the choice in the amber container, VPN on
  this device first). No protection is confirmed in a dialog before it is chosen.
- On the right, while the details are not whole, the sign-in page's QR: the page takes the torrent
  index, and for a proxy its host, port, user name and password. It takes one answer and closes,
  or closes after 10 minutes, and is reachable only on the local network while it is open. Once
  the details are whole they show as rows, with "Use a phone" for a new page. Focus then moves
  to "Type it", which takes the place of the button pressed, and comes back to it after Done or
  Back from the typing.
- Then one status place, held at one height whatever it says so the buttons never move: the
  check's note, "Checking the proxy" with its spinner, or the vermilion dot and why. TalkBack says
  each as it comes. Under it the amber "Turn on" (or "Save"), "Type it" (the same fields on the
  system keyboard, as pills, each named for TalkBack whatever it holds) and "Cancel", all 56 high.
  Back leaves the screen and changes nothing.
- A new or changed proxy is checked before anything is saved, turning torrents on included: the
  amber button keeps focus, the others are locked, and a failed check saves nothing ("Torrents
  are still off. …" or "Not saved. …, so the saved protection stays."). A verdict counts only for
  the exact details it checked. A saved change says so in a notice: "Torrents are on", "Torrent
  protection saved", "Torrent index saved", "Proxy saved".
- "Edit proxy…" opens the same screen on the saved proxy: "Edit proxy", the line that torrents keep
  the saved proxy until the new one passes the connection check, the proxy as a row, and the QR
  page for the proxy alone, filled in but for the password (left empty, the saved one is kept).
  "Save" checks first: a spinner and "Checking the proxy", then either the screen closes or a
  vermilion dot says why nothing was saved.
- The password never shows: not on the TV, not in the page's address, not in the logs.
- Downloads stay off on the TV; torrents stream only. Seeding is best effort, while anistream is
  open: the engine is told when the app goes to the background.

## The player (TVPlayer, TVPlayerSeek, TVPlayerEpisodes, TVPlayerSources, TVPlayerTracks, TVPlayerUpNext, TVPlayerInfo)

Full screen, libmpv through `:player`, zero-copy MediaCodec, no upscaling, the UI at 1080p over the
video plane. The video is never scaled by mpv's shaders on TV boxes; the hardware compositor scales
it to the panel.

### Starting

- The starting screen: the ground colour, the banner at its real aspect at the top (never a
  stretched cover, never blurred), the show title (Zen Kaku 22), "Starting episode 7", the step
  note from the engine ("Finding a source", "Hotaru Open Archive") in haze, and the 32 dp circular
  indicator. From a phone it adds "from Mika's phone".
- After 20 s without a first frame, tonal "Choose another source" and "Back" appear, focus on the
  first. The first frame fades in on fade-in.
- The player asks the display for the stream's frame rate (`Surface.setFrameRate`, without
  a mode switch), so a TV set to match content frame rate does so.

### The chrome

Over a 35 % dim of the ground colour, fading in on fade-in and out on fade-out, never sliding.

- Top left (x 48, y 27): show title (Zen Kaku 22), the episode line "E7 · The Lantern Road" (bodyLarge
  haze). Top right: the clock "19:42" and "Ends at 19:56" (labelMedium haze).
- Bottom: elapsed "10:29" and remaining "−13:31" (labelMedium) above the seek bar, the bar from x 48
  to 912 at y 437, 6 high, amber on #434653, with the opening and ending as water segments on top
  of the buffered and played parts. Below it at y 465, the button row (48 high): play/pause, then Episodes, Subtitles and
  audio, Source, Info as tonal pill buttons with icon and label, and "Next episode" at the right end.
- The chrome hides after 5 s with no key (the phone's 3 s is short across a room), never while the
  seek bar is scrubbing, while a panel is open, or while paused. Controls stay up while paused.

### D-pad in the player

| Key | Chrome hidden | Chrome shown |
|---|---|---|
| Center | Pause or play, and show the chrome (focus on play/pause). While a skip offer is fresh (its first 8 s), Center presses Skip instead. | Activate the focused control |
| Left, Right | Seek 10 s back or forward with a running count ("+30 s") beside a slim timeline at the bottom; one seek 500 ms after the last press | Move focus in the button row; on the seek bar, scrub |
| Up | Show the chrome, focus on the seek bar | Move focus (button row to seek bar) |
| Down | Show the chrome, focus on Episodes | Move focus (seek bar to button row) |
| Back | Close the player | See Back below |
| Play/pause, rewind, fast forward | Pause or play; 10 s back; 10 s forward (held: 30 s steps) | The same |
| Next, previous track | Next episode; restart, then the previous episode | The same |

Scrubbing (TVPlayerSeek): with the seek bar focused, Left and Right move a scrub head without
seeking. Steps are 10 s, 30 s after holding 1 s, 60 s after 3 s. The head stops once on the
opening's and the ending's edges, so a few presses land right after the opening. The time under the
head shows in labelLarge above it. On boxes with 64-bit userspace and at least 3 GB, a 192 × 108 frame
preview sits above the head; elsewhere only the time. Center jumps there; Back, Up or Down leave the
head and playback carries on where it was. Playback never pauses for a scrub.

Back in the player, top to bottom: an open panel closes; a scrub cancels; the Up Next card cancels;
"Still watching?" stops; otherwise the player closes, the position is saved, and the title page (or
the screen it came from) returns with focus on the episode and its progress updated (standard).

### Panels

Every panel is the 400-wide #2A2F45 column on the right, full height, fading in on fade-in. The
video keeps playing beside it (the chrome's dim stays; no extra scrim in the player). Focus starts
on the current item. Back closes.

- Episodes (TVPlayerEpisodes): the season chips at the top when there are several, then 72-high
  rows: still 96 × 54 (the spoiler rule), "E8 · A Town Without Night", "24 min" or "Watched". The
  playing one says "Playing" with an amber dot. Center plays that episode.
- Subtitles and audio (TVPlayerTracks): two sections, Audio ("Japanese", "English") and Subtitles
  ("Off", "English · Signs and songs", "English · Full"), radio rows. The choice applies at once
  and is remembered for the show. Subtitle size and colours follow the system's caption settings,
  noted in a haze line at the bottom ("Style follows the system's caption settings").
- Source (TVPlayerSources): the sources in order, each row "Hotaru Open Archive" with "1080p · Sub"
  below, the current one checked in amber. Center switches and resumes at the same position. A
  failed source says so in vermilion dot plus silk text and moves down.
- Info (TVPlayerInfo): the episode title, synopsis (spoiler rule), "Aired 13 October", then
  Playback details in labelMedium: "1080p · HEVC 10-bit · hardware decoding", "Hotaru Open
  Archive", "Dropped frames: 12", "Upscaling: off on this TV". This is where heavy subtitle
  stretches show their cost.

### Skip, Up Next and Still watching

- One skip offer per segment (opening, recap, ending, preview), bottom right above the seek bar's
  line (x 912 right edge, y 400), a tonal pill "Skip opening", shown while the chrome is hidden.
  For its first 8 s it holds focus (Center skips, with the "Skipped opening · Undo" offer after);
  then it dims and lets go, and is reachable by opening the chrome and moving Right from Next
  episode. Per-show auto-skip skips with only the Undo offer.
- Up Next (TVPlayerUpNext) when the ending starts: a #2A2F45 card bottom right (360 × 120): the next
  episode's still (184 × 104), "Next: E8 · A Town Without Night" (Zen Kaku 22 for the title),
  "Plays in 8 s" with the linear bar, and Play now (focused) and Cancel. It takes focus, so Center
  plays and Back cancels. At zero it fades through the ground colour into the next episode.
- Still watching? after three episodes in a row with no remote key and no phone command: playback
  pauses, a dialog "Still watching?" with Continue (focused) and Stop.

### What the TV does while playing

- Keeps the screen on while playing. Paused for 10 minutes, it lets the system's screensaver
  start; returning resumes on the paused frame with the chrome up.
- A MediaSession publishes what plays, so the system's controls and Google's own phone remote can
  pause and seek.
- Leaving the app (Home) pauses and saves the position. There is no Picture in Picture on TV in the
  first release.

## Phones as remotes (TVPair, TVPairRequest, TVRemoteActive)

The engine's remote protocol (`tv/remote`, `crates/anistream-remote`) carries this; the brief
fixes what people see on the TV and what the protocol must give it.

### Pairing

- The TV is findable on the local network while the app runs and for 10 minutes after it goes to
  the background (Settings › Remotes can turn that off).
- From the phone: the phone's Play on TV lists TVs on the network by name ("Living room TV"); the
  person picks one. The TV, wherever it is in the browse UI, shows a dialog (TVPairRequest): "Pair
  with Mika's phone?" with the 6-digit code in displaySmall and "Enter this code on the phone",
  and Cancel. During playback the request waits as a notice "Mika's phone wants to pair" and the
  dialog shows when the player closes.
- From the TV (TVPair, Settings › Remotes › Pair a phone, or the first-run card): a full screen with
  a QR code (180 × 180) carrying the TV's address and a one-time key, the same 6-digit code beside
  it for typing, and the steps: "Open anistream on your phone and choose Play on TV, then scan this
  code." Nagare (cell 5) on the left. The code changes every 5 minutes, and the screen stays until
  a phone pairs or Back.
- Paired: the notice "Mika's phone is paired" and the TV returns where it was. Pairing is
  remembered per device until removed in Settings › Remotes. A paired phone reconnects with no
  prompt.
- Several phones can pair. The newest command wins, and notices name who sent it.

### While a phone drives the TV

- Connect and disconnect show a notice ("Mika's phone connected"), nothing persistent.
- Play on TV from the phone: the TV opens the player from wherever it is (saving the position of
  anything playing), and the starting screen says "from Mika's phone".
- Remote controls (play, pause, seek, skip, next, subtitles, audio, source) act as if pressed on
  the TV. The TV shows what changed the way the D-pad would: a seek from the phone shows the slim
  timeline with its running count; a track change shows the notice "Subtitles: English · Full". It
  never opens a panel on the TV for a choice made on the phone.
- Search typing: as in Search above.
- The phone never mirrors or drives the TV's browse focus. The TV's own remote keeps working at all
  times, and the two never fight: the TV acts on each command in arrival order.
- The phone side (a Now playing bar with these controls on the phone's own screens) belongs to the
  phone briefs; it reads the TV's state from the engine.

### What the design needs from the protocol

- Discovery with the TV's name, pairing with a 6-digit code or a QR one-time key, per-device trust,
  removal from either side.
- Commands: play a title and episode (with source and position), pause, play, seek to and seek by,
  skip the current segment, next and previous, choose audio, subtitles and source, cancel Up Next,
  type text into search, submit search.
- State from the TV: what plays, position and duration, paused, the current tracks and source, the
  segment offers.
- The sender's name on every command, for the notices.
- Hand-overs: a tracker sign-in (the phone confirms) and plugin store links.
- Waking the TV app when it is closed is not in the first release. The TV listens while the app
  runs and for 10 minutes after it goes to the background.

## Home screen integration

- The launcher banner (320 × 180 at xhdpi): the ground #161A2E, Nagare from the app icon on the
  left, and "anistream" in Roboto Flex 500 silk. The icon is the phone app's.
- Watch Next (TVHomeRow is drawn as a mock of the system row, labelled as the system's): the app
  writes Android TV's Watch Next programs from the engine's Continue queue and new episodes, under
  Google's rules:
  - `WATCH_NEXT_TYPE_CONTINUE` for an episode stopped after more than 2 minutes and before its
    ending (or 3 minutes left), written within 5 s of leaving the app or 5 minutes paused;
  - `WATCH_NEXT_TYPE_NEXT` once an episode is finished and the next is out;
  - `WATCH_NEXT_TYPE_NEW` when a caught-up show gets a new episode;
  - at most one entry per show; removed on Drop, Hide from Continue, or nothing left to watch;
  - only the entry that changed is updated;
  - art is the show's banner (16:9 crop) or a spoiler-safe still, the title the show's, the
    description "Episode 8 · A Town Without Night";
  - its intent opens the player on that episode; Back from that player goes to the title page.
- This TV › "Show anistream on the home screen" turns it off and removes our entries.
- No app channels (preview rows). Google TV's home does not show them for apps outside its
  partner programme, and Google plans to retire Watch Next itself in the second half of 2027 in
  favour of the Engage SDK, which needs enrolment. See Decided.
- On Fire OS the launcher reads none of this; the writes are skipped there.

## Motion on TV

The tokens are motion-android.md's: quick `spring(1f, 1000f)`, standard `spring(1f, 440f)`,
fade-in `tween(200, LinearOutSlowInEasing)`, fade-out `tween(300, FastOutLinearInEasing)`, linear
for countdowns, `MotionScheme.standard()`. Everything moves on `graphicsLayer` (alpha, scale,
translation); nothing animates size or layout.

| Id | Motion | TV |
|---|---|---|
| TV1 | Focus | Scale and ring on quick; snap when the key is held |
| TV2 | Row and grid scroll | `BringIntoViewSpec` with quick, the focused card pinned at the keyline; a held key scrolls card by card on the same spring, retargeted each press |
| TV3 | Rail | tv-material's drawer expansion (system), scrim fades on fade-in |
| T6 | Destinations | Fade through on quick, as on the phone |
| TV5 | Card to title page | A crossfade on fade-in, the incoming page composed first. No shared element on TV: it keeps two pages and their art alive through the move, on a 2 GB box, for a transition seen from three metres |
| TV6 | Title to player | The starting screen fades in on fade-in; the first frame fades in over it |
| T4 | Player chrome | Fade in and out, never sliding; 5 s idle on TV |
| T5 | Up Next | As on the phone: fade in on standard, linear countdown, fade through the ground |
| TV7 | Panels and dialogs | Fade in on fade-in, out on fade-out, no slide; scrim on fade-in |
| T8 | Loading to content | As on the phone, unchanged |
| T9 | Marking watched | The check, the dim and the count as on the phone; no haptic |

Remove animations is honoured as on the phone: timed motion lands at once; the 600 ms wait, the
0.3 s hold, the 5 s chrome timer and the Up Next countdown still run. No haptics and no sounds on TV.

## Performance budgets (2 GB, 32-bit boxes)

Measured on the Chromecast with Macrobenchmark and `dumpsys meminfo`; a screen that misses a budget
is not done.

- Memory. The whole app under 300 MB PSS while browsing and under 420 MB while playing 1080p
  (engine, plugins, libmpv and decoder buffers included); never over 500 MB. `onTrimMemory`
  clears the image memory cache at `RUNNING_LOW` and above.
- Image memory cache 40 MB (about 230 posters), disk cache 200 MB. Entering the player trims the
  memory cache to 16 MB and disposes the browse screens; leaving refills from disk.
- Image sizes, decoded at the size drawn, never larger:
  - posters 240 × 360 px (AniList's `large` cover; `extraLarge` only for the compact header);
  - 16:9 cards 370 × 208 px; episode panel stills 192 × 108 px; the Up Next still 368 × 208 px;
  - the hero and title banners 1920 px wide, at most two decoded at once (the current one and the
    one fading in);
  - spoiler stills 24 × 14 px, drawn scaled;
  - covers, cards and stills in RGB_565 (opaque art, half the memory), banners in ARGB_8888 (sky
    gradients band in 565).
- Lists. Compose `LazyColumn` of `LazyRow`s and `LazyVerticalGrid`, with stable keys and
  `contentType` on every item. At most the visible rows plus one composed; prefetch the next row's
  first five cards. No nested scrolling on the same axis. Cards are one `Card` with an image and two
  `Text`s, no extra layers.
- Images while moving: a card focused for less than 150 ms during a held key does not start its
  request; requests for cards that have scrolled off are cancelled.
- Frames. 60 Hz UI. No frame over 33 ms during a focus move, a row scroll or the rail; under 1 %
  janky frames on Continue with a held Right across 20 cards. Overdraw at most 2× outside the
  player. Ship baseline profiles for `:tv` (32-bit ARM runs interpreted code slowly before the
  profile is compiled).
- Start. Cold start to first frame under 1.5 s, to Continue's content under 3 s with a warm engine
  cache.
- Never on these boxes: blur of any kind, shadows and elevation, glow, gradients, animated
  backgrounds, video previews in browse, a second decoder for seek frames, upscaling shaders.
- Subtitles. The 13 % drop under heavy signs is accepted: it lasts only that stretch, and stripping
  the typesetting would lose the signs. Info shows the dropped frames.
- Plugins run on Pulley on 32-bit ARM, so sources resolve slower than on the phone; the starting
  screen's step notes carry that wait.

## Never on TV

- A light theme, dynamic colour, pure white, hairlines.
- Glow, shadows, blur, gradients, or a scrim over the video for panels.
- Focus that jumps by itself, except the skip offer, Up Next and the phone's Search submit.
- Switching screens on rail focus. Wrapping focus at an edge.
- An exit confirmation. Intercepting Home.
- Autoplaying video, trailers or carousels in browse.
- Telling people to press the remote's mic or Assistant key.
- A text field as the only way to enter a link, a token or a password.
- Nagare in navigation, the player, or beside a show's art; animating Nagare.
- Real show, site, fansub or release-group names, and any claim that a VPN or a check protects
  anyone. The legal line is the main brief's, word for word.

## Board names (prefix TV)

TVFoundations (canvas, safe area, type, colour and focus states, drawn), TVMotion, TVContinue,
TVWelcome, TVRailExpanded, TVSearch, TVSearchPhone, TVSeasonal, TVCalendar, TVLibrary,
TVLibraryEmpty, TVTitle, TVTitleActions (long press panel), TVShowSettings, TVPlayer, TVPlayerSeek,
TVPlayerEpisodes, TVPlayerTracks, TVPlayerSources, TVPlayerInfo, TVPlayerSkip, TVPlayerUpNext,
TVPlayerStarting, TVPair, TVPairRequest, TVRemoteActive, TVSettings, TVSettingsAccounts, TVSignIn,
TVSignInCode, TVJoinStore, TVTorrents, TVHomeRow. All 960 × 540.

Shared data is the main brief's: now is Tuesday 20 October 2026, 19:30; Mika on AniList; Hoshikuzu
no Tabibito with episode 7 in progress (14 min left of 24); the four stores and Hotaru Open
Archive; Ren's fingerprint. The TV is "Living room TV"; the phone is "Mika's phone".

## Decided (2026-10-23)

Johan took the recommendation on all twelve open questions.

1. Navigation is the left rail, expanding over the content.
2. Search uses our own keyboard grid, with a key for the system keyboard.
3. The mic key shows only where the box has speech recognition (check it on the Chromecast).
4. The TV listens for phones while the app runs and for 10 minutes after it goes to the
   background. Launching a closed app through Android TV's own remote service comes later.
5. Pairing shows a QR code and a 6-digit code; a pairing started on the phone shows the code.
6. The TV serves a one-time sign-in page on the local network (AniList's PIN code, Kitsu's form).
7. Home screen integration is Watch Next, behind its setting; no Engage SDK.
8. Seek preview frames are off on 32-bit boxes or under 3 GB.
9. Upscaling is hidden on every TV.
10. Torrents run on the TV; downloads stay off. (Changed 2026-10-25: Johan chose TV torrents.)
11. Back in the player closes it, after any panel, scrub or Up Next.
12. The phone is not a D-pad; it sends commands and search text.
