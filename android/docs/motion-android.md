<!-- A copy of ~/anistream-design/native-spec/motion-android.md, the motion spec the app is built
     to. The motion boards are in ~/anistream-design/design/project/AndroidMotion*.dc.html. -->

# Motion on Android

The motion spec for the anistream Compose app on Android phones, tablets and foldables. It is the
Android side of `motion.md` (`~/anistream-design/native-spec/motion.md`): every transition keeps the meaning and the id it has on
Apple, and this file says how Compose builds it and what the boards draw, with their times and
thresholds. Every value here is fixed.

Boards on the canvas:

- AndroidMotionSystem: tokens with live demos, `MotionScheme.standard()`, the system's motion and
  ours, Remove animations, haptics, the Never list, the transition index.
- AndroidMotionCoverToTitle: T1, with predictive back.
- AndroidMotionTitleToPlayer: T2 and T3.
- AndroidMotionPlayer: T4, T5 and Picture in Picture.
- AndroidMotionFeedback: T6 to T9.
- AndroidMotionTablet: AT1 to AT3, tablet and foldable only.
- AndroidFoundations: the components and their shapes, which this file animates.

Springs for anything spatial, never bounce. Opacity uses the fade tokens. Anything the platform
animates on its own (NavHost seeking on predictive back, sheets, menus, the navigation indicator,
the rail, panes, snackbars, Picture in Picture, rotation) keeps the platform's animation.

Conventions:

- Times are board milliseconds at 1×, from the start of the board's chapter. Springs are drawn to
  99 % of their travel: quick at 0.21 s, standard at 0.32 s, the shared bounds at 0.33 s. Where the
  system owns the motion the board draws an approximation and says "drawn at about".
- Positions are dp on the phone canvas, 412 × 915 portrait or 915 × 412 landscape, origin top left.
- `haptic(X)` is `LocalHapticFeedback.current.performHapticFeedback(HapticFeedbackType.X)`.

## Tokens

```kotlin
import androidx.compose.animation.core.*

object Motion {
    /** Selection, toggles, the row dim after marking watched, the pane divider's handle,
     *  the fade-through between navigation destinations. */
    fun <T> quick(): SpringSpec<T> = spring(dampingRatio = 1f, stiffness = 1000f)

    /** Content changes inside a screen: progress, Skip opening and Up Next appearing,
     *  list insert and remove, the spring back after a cancelled gesture. */
    fun <T> standard(): SpringSpec<T> = spring(dampingRatio = 1f, stiffness = 440f)

    /** Player controls appearing, artwork arriving, the first frame over the art,
     *  a page's content in a shared transition, the detail pane's content. */
    fun <T> fadeIn(): TweenSpec<T> = tween(durationMillis = 200, easing = LinearOutSlowInEasing)

    /** Player controls hiding, a page's content leaving. */
    fun <T> fadeOut(): TweenSpec<T> = tween(durationMillis = 300, easing = FastOutLinearInEasing)
}

// linear: countdowns only, written in place: tween(10_000, easing = LinearEasing).
// system: no token. Leave the platform's motion alone. Shared bounds and shared elements keep
// Compose's default boundsTransform, spring(dampingRatio = 1f, stiffness = 400f), about 0.31 s.
```

| Token | Compose | CSS on the boards | Used for |
|---|---|---|---|
| quick | `spring(dampingRatio = 1f, stiffness = 1000f)` | `240ms` with the spring `linear()` below | Selection, toggles, row dim after marking watched, the divider handle, the fade-through between destinations (T6) |
| standard | `spring(dampingRatio = 1f, stiffness = 440f)` | `360ms` with the spring `linear()` below | Content changes inside a screen, the spring back after a cancelled gesture |
| fade-in | `tween(200, easing = LinearOutSlowInEasing)` | `200ms cubic-bezier(0, 0, 0.2, 1)` | Controls appearing, artwork, the first frame, page and detail content |
| fade-out | `tween(300, easing = FastOutLinearInEasing)` | `300ms cubic-bezier(0.4, 0, 1, 1)` | Controls hiding, page content leaving |
| linear | `tween(n, easing = LinearEasing)` | `ns linear` | Countdowns only: Up Next, 10 s |
| system | the platform's own | shared bounds: `spring(1f, 400f)`, about 0.31 s, drawn as `380ms` with the spring `linear()` | Shared bounds and elements, NavHost seeking, sheets, menus, indicator, rail, panes, snackbar, PiP, rotation |

A zero-bounce spring of duration d has stiffness (2π / d)², so Apple's 0.2 s and 0.3 s springs are
stiffness 987 and 439: Compose's `spring(1f, 1000f)` and `spring(1f, 440f)`. Compose's default
bounds spring, stiffness 400 (`Spring.StiffnessMediumLow`), is a spring of d = 0.31 s; it is the
"system" timing for every shared bounds and shared element on Android. All three are critically
damped, so none passes its target. The boards draw a spring of duration d as a CSS duration of
1.2·d with this easing, the same as the Apple boards:

```css
linear(0, 0.055 5%, 0.175 10%, 0.312 15%, 0.445 20%, 0.562 25%, 0.66 30%, 0.803 40%, 0.89 50%, 0.94 60%, 0.968 70%, 0.983 80%, 1)
```

The fade easings differ slightly from Apple's (`(0, 0, 0.2, 1)` against `(0, 0, 0.58, 1)` for
fade-in, `(0.4, 0, 1, 1)` against `(0.42, 0, 1, 1)` for fade-out). They are Android's own named
easings at the same durations: parity, not replication.

### MotionScheme

```kotlin
MaterialExpressiveTheme(
    colorScheme = ObiSilkDark,               // fixed; dynamicDarkColorScheme is never called
    motionScheme = MotionScheme.standard(),  // never expressive()
    typography = AnistreamType,
    shapes = AnistreamShapes,
) { App() }
```

Components move on the motion scheme, not on our tokens: the switch handle, button shape morphs on
press, the navigation bar and rail indicator, the slider handle, connected button groups,
`ModalWideNavigationRail` and `ListDetailPaneScaffold`'s panes. `MotionScheme.standard()` damps its
spatial springs at 0.9, which passes the target by under 0.2 % and reads as no overshoot;
`expressive()` damps at 0.6 to 0.8 and its fast spatial spring overshoots by 9 %.

| Spec | `standard()` | `expressive()`, not used |
|---|---|---|
| fastSpatialSpec | `spring(0.9f, 1400f)` | `spring(0.6f, 800f)` |
| defaultSpatialSpec | `spring(0.9f, 700f)` | `spring(0.8f, 380f)` |
| slowSpatialSpec | `spring(0.9f, 300f)` | `spring(0.8f, 200f)` |
| fast, default, slow effects | `spring(1f, 3800f)`, `spring(1f, 1600f)`, `spring(1f, 800f)` | the same |

### The system's own, and ours

Leave alone (no spec of ours, no `animationSpec` override):

- NavHost composing the previous page and seeking the shared transition on predictive back (T1).
- `ModalBottomSheet`: open, drag, detents, scrim (T7).
- `DropdownMenu` opening from its anchor (T7).
- The navigation bar and rail indicator, and `ModalWideNavigationRail` expanding (T6, AT1).
- `SnackbarHost` in and out.
- The large top app bar collapsing, the search bar expanding into the search view.
- `ListDetailPaneScaffold`'s pane motion and the divider settling on an anchor (AT2, AT3).
- Picture in Picture, rotation, the fold's display switch.
- Ripples, switch and slider handles, button shape morphs.

Ours, on the tokens:

- The shared element and shared bounds keys: cover, art, page, player (T1, T2). Compose's
  default `boundsTransform` is the system spring; do not pass another.
- The fade-through between navigation destinations, on quick (T6).
- The player's drag and predictive back, which move a still of the frame (T3).
- Player chrome fades (T4), Skip opening, Up Next and its countdown (T5).
- Progress, numeric text, list insert and remove (standard).
- Placeholders to content and covers arriving (T8).
- Marking watched: the dim, the check, the count (T9).
- Selection (quick) and the detail pane's content (AT2).
- The title page banner's 0.5× parallax.

## Phone

All phone transitions run inside one `SharedTransitionLayout` around the `NavHost`. Every
destination's composable gets its `AnimatedContentScope` from `composable<Route> { }`, which the
shared modifiers take as `animatedVisibilityScope`. The navigation bar lives outside the `NavHost`
(in `NavigationSuiteScaffold`), so it never takes part in a transition.

NavHost transitions follow one rule. A switch between top-level destinations fades through on
quick (T6). A push inside a destination (T1, T2) keeps the page underneath: the source's exit and
popEnter are `ExitTransition.KeepUntilTransitionsFinished` and `EnterTransition.None`, the pushed
page's enter and popExit are `EnterTransition.None` and `ExitTransition.KeepUntilTransitionsFinished`,
and the shared bounds do all the moving.

```kotlin
val topLevel: AnimatedContentTransitionScope<NavBackStackEntry>.() -> Boolean = {
    initialState.topLevelDestination() != targetState.topLevelDestination()
}
NavHost(
    nav, startDestination = Continue,
    enterTransition = { if (topLevel()) fadeIn(Motion.quick()) else EnterTransition.None },
    exitTransition = { if (topLevel()) fadeOut(Motion.quick()) else ExitTransition.KeepUntilTransitionsFinished },
    popEnterTransition = { if (topLevel()) fadeIn(Motion.quick()) else EnterTransition.None },
    popExitTransition = { if (topLevel()) fadeOut(Motion.quick()) else ExitTransition.KeepUntilTransitionsFinished },
) { … }
// topLevelDestination(): the entry's top-level graph (Continue, Calendar, Seasonal, Library, Search)
```

Predictive back needs `android:enableOnBackInvokedCallback="true"` on the application (the default
from API 36) and Navigation Compose 2.8 or later. NavHost then composes the previous destination
under the current one while the finger is down and seeks the transition with the gesture's
progress.

Orientation. On phones (smallest width under 600 dp) the browse UI is portrait only and the player
is landscape: the app asks for portrait at start, T2 asks for landscape once the player's bounds
have settled, and T3 asks for portrait again as it closes. A phone never returns from the player
into a landscape title page. Tablets and foldables rotate freely and the app never asks them for an
orientation; for apps that target API 36, Android 16 ignores such requests on large screens anyway.
A folded foldable's outer screen is under 600 dp, so it follows the phone rule (AT3).

```kotlin
val phone = resources.configuration.smallestScreenWidthDp < 600   // read again on every change
if (phone) activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
// AndroidManifest: configChanges="orientation|screenSize|screenLayout|smallestScreenSize|density",
// so a turn or a fold does not recreate the activity, the player, its surface or the shared state
```

### T1 Cover to title

- Trigger: a tap on a cover in any grid or row, on the Continue hero or on an In progress card.
- From: the cover, or the hero's or card's banner.
- To: the title page, pushed in the current destination's back stack.

The stage is the phone at 1:1, 412 × 915 dp, matching AndroidHome and AndroidTitle. The navigation
host is the 412 × 827 area above the 88 dp navigation bar; the bar never moves and never dims.

Forward:

| Element | From → to | Token | When |
|---|---|---|---|
| Page bounds, `sharedBounds` key `page-$id` | the hero banner, x 16, y 152, 380 × 214, corners 16 → x 0, y 0, 412 × 827, corners 0 | system, `spring(1f, 400f)`, about 0.31 s | 520 → 852 (99 %) |
| Banner art, `sharedElement` key `art-$id` | 380 × 214 at x 16, y 152, corners 16 → 412 × 196 at x 0, y 0, corners 0; the art crops to its box | the same spring | 520 → 852 |
| Page content | opacity 0 → 1; scaled to the bounds' width (0.922 → 1), pinned to the top | fade-in | 520 → 720 |
| Hero banner on Continue | hidden while the page holds the banner | none | from 520 until the return lands |
| Continue | stays as it is under the page: no dim, no scrim | none | throughout |

- The tap is drawn 400 → 520 at x 206, y 259; the transition starts on release.
- Drawn order, back to front: Continue, the page's ground container, the banner art, the page's
  content. The page's 80 × 120 poster overlaps the banner by 44 dp, so it draws above the flying
  art for the whole transition: give the poster a higher `zIndexInOverlay` than the art.
- Other sources: an In progress card grows the same way from its banner (x 16, y 538, 208 × 117,
  corners 16). A poster in a grid flies into the page's cover (`sharedElement` key `cover-$id`, to
  80 × 120 at x 16, y 152, corners 8) while the page's bounds grow from the grid cell.
- On the title page the banner scrolls at 0.5× (parallax). Overscroll is the platform's stretch;
  there is no banner stretch of our own.

Predictive back. NavHost seeks the shared transition backwards with `BackEventCompat.progress` p,
0 to 1, so as the gesture progresses the title page shrinks toward its cover tile under the finger.
This is the Android counterpart of Apple's interactive zoom dismiss. Nothing of ours runs while the
finger is down: every part of the return sits where it is at fraction p of its run, the bounds on
the system spring's curve and the page's content on its fade-out.

| Progress p | 0 | 0.25 | 0.5 | 0.75 | 1 |
|---|---|---|---|---|---|
| Return played, ms (drawn run 0.33 s) | 0 | 83 | 166 | 249 | 332 |
| Page bounds toward the tile | 0 % | 50 % | 85 % | 97 % | 100 % |
| Page width, dp | 412 | 396.0 | 384.7 | 381.0 | 380 |
| Page height, dp | 827 | 521.0 | 304.5 | 233.3 | 214 |
| Corners, dp | 0 | 8.0 | 13.6 | 15.5 | 16 |
| Page content opacity | 1 | 0.88 | 0.62 | 0.26 | 0 |
| Continue | fully visible under the page, no dim, throughout | | | | |

The banner art flies back from the page's banner (412 × 196 at 0, 0) toward the hero rect on the
same curve. The board draws the seek with the bounds spring run to 99 % in 0.33 s; on the device
the seek uses Compose's own estimate of the transition's length.

Release:

| Outcome | What moves | Token | When on the board |
|---|---|---|---|
| Past the commit point | the rest of the return plays from where the seek left it: the bounds and the art land in the hero tile (x 16, y 152, 380 × 214, corners 16), the content finishes its fade-out | system | from p 0.62: 3700 → 3826, content out by 3794 |
| Before it | the seek goes from p back to 0, and the page grows back to full size | standard | from p 0.24: 2100 → 2417 |

- Commit point: the system decides at release, from distance and speed past its own trigger (a
  system value). The board draws it at p 0.30. Our code never compares p with a number.
- Edge: either edge seeks the same return. `BackEventCompat.swipeEdge` changes nothing in it; the
  board's Edge switch moves only the drawn finger.
- No dim under the page during the gesture. Continue is fully visible underneath.
- Other back paths: the top bar's back arrow and the 3-button back key play the whole return from
  p 0, on the system spring.

Compose:

```kotlin
SharedTransitionLayout {
    NavHost(nav, startDestination = Continue /* transitions as in Phone above */) {
        composable<ContinueRoute> {
            Box(Modifier.sharedBounds(rememberSharedContentState("page-$id"), this@composable,
                    enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut()))) {
                BannerArt(Modifier.sharedElement(rememberSharedContentState("art-$id"), this@composable))
            }
        }
        composable<TitleRoute> {
            TitlePage(Modifier.sharedBounds(rememberSharedContentState("page-$id"), this@composable,
                enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut()),
                resizeMode = SharedTransitionScope.ResizeMode.ScaleToBounds(
                    ContentScale.FillWidth, Alignment.TopCenter))) {
                BannerArt(Modifier
                    .sharedElement(rememberSharedContentState("art-$id"), this@composable)
                    .graphicsLayer { translationY = scroll * 0.5f })        // the parallax
                Poster(Modifier.sharedElement(rememberSharedContentState("cover-$id"), this@composable,
                    zIndexInOverlay = 1f))
            }
        }
    }
}
// boundsTransform stays Compose's default, spring(1f, 400f).
// Predictive back: no PredictiveBackHandler of ours. NavHost seeks this transition; a commit
// plays the rest, a cancel goes back to the page on standard.
```

- Timing: system for the bounds and the art; fade-in in, fade-out out for the page's content.
- What stays: the navigation bar and its selection, and Continue, which never dims. The source is
  hidden while the page is up.
- Gesture: predictive back from either edge, seeking the return; the back arrow.
- Haptic: none of ours. The system plays its own at its commit point.
- Remove animations: we never override the scale. The forward transition and a back-arrow return
  are timed, so they land at once. The back gesture still seeks with the finger, and its settle
  snaps: a commit lands on Continue, a cancel on the page. The parallax follows the scroll.
- Board: AndroidMotionCoverToTitle, one 4.8 s timeline that loops after a 1 s hold, plus an Edge
  switch (left, right), a Back progress slider (0 to 1) and Let go, which drive the stage by hand.

Board timeline:

- 0–400: Continue at rest. 400–520: tap the hero.
- 520: the page grows out of the hero banner; settled by 852. Content in on fade-in, 520–720.
- 1000: the title page at rest.
- 1500–2100: edge swipe to p 0.24; the page shrinks toward the tile with the seek.
- 2100: let go before the commit point; the seek returns to 0 on standard by 2417.
- 2900–3700: edge swipe to p 0.62, past the commit point at 3291.
- 3700: let go; the rest of the return plays, 126 ms, into the hero tile by 3826.
- 3826: Continue at rest. Let go on the board replays the outcome from the slider's p.

### T2 Title to player

- Trigger: Resume on the title page, Resume on the Continue hero, or an episode row.
- From: the banner it was opened from.
- To: the player, a full-screen destination over the navigation bar. Phones turn to landscape.

The stage is the phone at 0.8 in a 915 × 915 box, drawn turning in the hand. Positions are dp in
the screen as it is at that moment: 412 × 915 portrait or 915 × 412 landscape.

| Element | From → to | Token | When |
|---|---|---|---|
| Player bounds, `sharedBounds` key `player-$id` | the title banner, x 0, y 0, 412 × 196 → the full screen, 412 × 915; corners 0 throughout | system, `spring(1f, 400f)` | 620 → 952 |
| Player content, laid out at full size and kept centred in the bounds (`ScaleToBounds(FillWidth, Center)`) | opacity 0 → 1; its art band travels from the banner to the middle | fade-in | 620 → 820 |
| Banner content, centred in the bounds | opacity 1 → 0 | fade-out | 620 → 920 |
| System bars | hide | system | with the open |
| Display, phones only | portrait → landscape, asked for with `SCREEN_ORIENTATION_SENSOR_LANDSCAPE` once the bounds have settled | system; drawn as a 0.4 s turn | 1000 → 1423 |
| Video rect | x 0, y 341.6, 412 × 231.75 → x 92, y 0, 732 × 412 | none: a relayout under the system's turn | with the turn |
| Starting layer | holds until the first frame | none | 820 → 2400 |
| First frame | starting layer opacity 1 → 0 over mpv's surface, so the frame appears and the bars go from ground to black | fade-in curve | 2400 → 2600 |
| Controls (T4) | opacity 0 → 1, then out after 3 s idle | fade-in, fade-out | 2400 → 2600, 5600 → 5900 |
| Title page | stays under the growing player, then is not composed in landscape | none | 0 → 1000 |

- Trigger drawn: Resume E7 · 10:06, 500 → 620 at x 206, y 316 (the 56 dp button at y 288). Resume
  on the Continue hero and an episode row open the same way from their banner.
- Landscape is asked for after the shared bounds settle, so the system's turn never cuts into the
  grow. Tablets keep their orientation.
- Starting layer, in the player at full size: ground #161A2E everywhere, the banner art cropped to
  the video rect, a 35 % ground dim over it, and centred on the video rect an indeterminate
  `CircularProgressIndicator`, 32 dp (silk, 3 dp stroke, no track; its motion is its own), with
  "Starting episode 7" 16 dp below it (16/24, 500, silk, a 1 dp shadow at 70 %). This is the one
  place a spinner appears outside a button or pull to refresh.
- The video rect is where mpv will draw, so the first frame lands on the art without a jump.

Compose:

```kotlin
Banner(Modifier.sharedBounds(rememberSharedContentState("player-$id"), this@composable,
    enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())))

PlayerScreen(Modifier.fillMaxSize().sharedBounds(rememberSharedContentState("player-$id"), this@composable,
    enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut()))) {
    AndroidView(factory = { SurfaceView(it) })                  // libmpv, black until the first frame
    StartingLayer(Modifier.graphicsLayer { alpha = 1f - firstFrame.value }) {
        BannerArt(title); GroundDim(0.35f)
        CircularProgressIndicator(Modifier.size(32.dp), color = Silk, strokeWidth = 3.dp,
            trackColor = Color.Transparent)
        Text("Starting episode 7")
    }
    PlayerChrome()                                               // T4
}
LaunchedEffect(hasFirstFrame) { if (hasFirstFrame) firstFrame.animateTo(1f, Motion.fadeIn()) }
LaunchedEffect(transition.isRunning) {                           // once the bounds have settled
    if (!transition.isRunning && phone)
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
}
WindowCompat.getInsetsController(window, view).run {
    systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    hide(WindowInsetsCompat.Type.systemBars())
}
```

- Fade the layer above the SurfaceView, not the SurfaceView: a SurfaceView does not take Compose's
  alpha, clip or shape reliably.
- Timing: system for the grow and the turn; fade-in for the first frame. The controls show once the
  frame is up and hide after 3 s idle (T4).
- What stays: the banner art, the poster until the first frame; the page underneath, for the way
  back, until the phone turns.
- Gesture: none while opening. The player closes with T3.
- Haptic: none.
- Remove animations: we never override the scale. The grow, the starting layer, the first frame
  and the controls are timed, so they land at once. The turn is the system's.
- Board: AndroidMotionTitleToPlayer, 0 → 6.2 s of a 9.6 s timeline that loops after a 1 s hold.

### T3 Player dismiss

- Trigger: a drag down on the video, the back gesture, or the back arrow in the player's top bar or
  the 3-button back key, which run the same close.
- From: the player.
- To: the page it was opened from, the player closing into the banner.

The whole player moves as one layer, the bars included, as a still of the frame over a ground
cover, so the ground #161A2E shows behind it. On a phone the title page is portrait only and is not
drawn behind the player in landscape. A marker at 6.2 s on the board reads "about 3 min later": the
position is 12:58 (54 %).

Drag down:

| Element | From → to | Token | When |
|---|---|---|---|
| Player offset | 0 → dy, 1:1 with the finger, down only | follows the finger | 6400 → 6900 (to 80 dp), 7400 → 8200 (to 170 dp) |
| Player scale, about its centre | 1 → 0.9, that is 1 − 0.1k with k = min(dy, 120) / 120 | follows the finger | as above |
| Player corners | 0 → 16 dp, that is 16k | follows the finger | as above |
| Let go short of the threshold | dy to 0, so scale and corners return | standard | 6900 → 7217 |

| dy, dp | 0 | 40 | 80 | 120 | 170 |
|---|---|---|---|---|---|
| Scale | 1 | 0.967 | 0.933 | 0.9 | 0.9 |
| Corners, dp | 0 | 5.3 | 10.7 | 16 | 16 |
| Let go | back | back | back | closes | closes |

- Threshold: let go past 120 dp, or fling faster than 800 dp/s downward, and the player closes.
  Short of both it springs back. The drag starts after touch slop, on a vertical drag only.
- At 170 dp the player is 823.5 × 370.8 with its top at y 190.6.
- Haptic: `GestureThresholdActivate` each time dy crosses 120 dp going down (drawn at 7913);
  nothing when it comes back up.

Predictive back. The title page is not composed behind the landscape player, so there is nothing
for NavHost to seek. The player's own `PredictiveBackHandler` shrinks it under the finger, and the
shared return runs at commit.

| Element | From → to | Token | When |
|---|---|---|---|
| Player scale, about its centre | 1 − 0.1p | follows p | 6400 → 6900 (to 0.24), 7400 → 8200 (to 0.62) |
| Player corners | 28p dp (the display's radius at p 1, a system value; read it from `RoundedCorner` on API 31 and later, else 28) | follows p | as above |
| Player shift | 37.75p dp away from the swipe edge and 12.6p dp up, toward the banner it closes into | follows p | as above |
| Let go before the commit point | p to 0 | standard | 6900 → 7217 |

| Progress p | 0 | 0.25 | 0.5 | 0.75 | 1 |
|---|---|---|---|---|---|
| Scale | 1 | 0.975 | 0.95 | 0.925 | 0.9 |
| Corners, dp | 0 | 7 | 14 | 21 | 28 |
| From the edge, dp | 0 | 9.4 | 18.9 | 28.3 | 37.8 |
| Up, dp | 0 | 3.2 | 6.3 | 9.5 | 12.6 |

- At p 1 the player is 823.5 × 370.8 at x 83.5 (left edge) or 8 (right edge), y 8: 8 dp is left at
  the far side and at the top.
- Commit point: the system's (drawn at p 0.30, crossed at 7791). Haptic: none of ours.

Back arrow: the arrow at the left of the player's top bar (48 × 48 at x 48, y 16) runs the close
from rest, and so does the 3-button back key. Drawn: tap the video 7400 → 7500, controls in
7500 → 7700 (fade-in), tap the arrow 8100 → 8200.

The close, all three. From 8200, in this order and in one frame: controls that are up fade out, a
phone asks for portrait again, and we navigate up.

| Element | From → to | Token | When |
|---|---|---|---|
| Controls, if up | 1 → 0 | quick | 8200 → 8410 |
| Display, phones only | landscape → portrait | system; drawn as a 0.4 s turn | 8200 → 8623 |
| Player bounds | from where the drag or the back gesture left the player (or the full screen, for the arrow) into the title banner, x 0, y 0, 412 × 196, corners → 0 | system, `spring(1f, 400f)` | 8200 → 8532 |
| Player content | 1 → 0 | fade-out | 8200 → 8500 |
| Banner content | 0 → 1 | fade-in | 8200 → 8400 |
| Title page | 0 → 1 under the player | fade-in | 8200 → 8400 |
| The banner's own spot | stays empty until the player is gone | none | to 8700 |

Because the browse UI is portrait only on phones, the close always turns the display back to
portrait and the player returns into the portrait banner, whatever the phone's auto-rotate setting.

Back on the page. The page holds the old progress while the player is up and lets it go once the
return has settled:

| Element | From → to | Token | When |
|---|---|---|---|
| Episode 7 bar, 64 dp track | 42 % → 54 % | standard | 8800 → 9117 |
| "E7 · 14 min left" | only the changed digit rolls: the old 4 moves down 20 dp and fades, the new 1 comes from 20 dp above | standard | 8800 → 9117 |
| The Resume button's time | "Resume E7 · 10:06" becomes "Resume E7 · 12:58": the time crossfades | standard | 8800 → 9117 |

Compose:

```kotlin
val dy = remember { Animatable(0f) }                       // dp, down only
val haptics = LocalHapticFeedback.current
Modifier.draggable(
    rememberDraggableState { d ->
        val before = dy.value
        scope.launch { dy.snapTo((before + d.toDp().value).coerceAtLeast(0f)) }
        if (before < 120f && before + d.toDp().value >= 120f)
            haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
    },
    Orientation.Vertical,
    onDragStarted = { freeze() },
    onDragStopped = { v -> if (dy.value > 120f || v.toDp().value > 800f) close()
                           else { dy.animateTo(0f, Motion.standard()); unfreeze() } },
)
val back = remember { Animatable(0f) }
PredictiveBackHandler { events ->
    freeze()
    try {
        events.collect { e -> edge = e.swipeEdge; back.snapTo(e.progress) }
        close()
    } catch (e: CancellationException) {
        scope.launch { back.animateTo(0f, Motion.standard()); unfreeze() }
        throw e
    }
}
Still(Modifier.graphicsLayer {                             // drag: k = min(dy, 120) / 120
    scaleX = 1f - 0.1f * k; scaleY = scaleX; translationY = dy.value.dp.toPx()
    shape = RoundedCornerShape((16 * k).dp); clip = true
})                                                         // back: 1 − 0.1p, 28p, the shift

fun close() {
    chromeVisible = false                                   // quick
    if (phone) activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
    nav.navigateUp()                                        // the shared return
}

// back on the page
val shown = held ?: progress                                // held is set when the player opens
val fraction by animateFloatAsState(shown.fraction, Motion.standard())
ProgressBar(fraction)
// "14 min left": split the number into digits, one AnimatedContent per digit, so only the digit
// that changes moves, as .numericText does on Apple
AnimatedContent(digit, transitionSpec = {
    (slideInVertically(Motion.standard()) { -it } + fadeIn(Motion.standard())) togetherWith
        (slideOutVertically(Motion.standard()) { it } + fadeOut(Motion.standard()))
}) { Text("$it") }
```

- The still: `freeze()` copies the current frame with `PixelCopy.request(surfaceView, bitmap, …)`
  when a drag or a back gesture starts, covers the SurfaceView with ground, and moves the still
  (scale, offset, corners) in its place, the bars included. The video keeps playing under the
  cover. `unfreeze()` on a cancel removes the still and the cover. A SurfaceView does not take
  Compose's alpha, clip or shape reliably, so the still is what moves and what the return carries
  into the banner.
- `GestureThresholdActivate` needs API 34; below it use `CONTEXT_CLICK`.
- Timing: the drag and the back gesture follow the finger; released short, they spring back on
  standard; the close is the system's shared bounds return; progress and the digits run standard
  once the return has settled.
- What stays: the page under the player, drawn again once the phone is portrait.
- Remove animations: we never override the scale. The drag and predictive back still follow the
  finger with the values above, and their settle snaps: a spring back or a close lands at once. The
  progress bar and the digits land at once. The thresholds and the haptic stay.
- Board: AndroidMotionTitleToPlayer, 6.2 → 9.6 s. "T3 closes by" switches between Drag down,
  Predictive back and Back arrow; the slider sets the drag distance (0 to 200 dp) or the back
  progress (0 to 1) by hand, and Let go replays the outcome from it.

### T4 Player chrome

- Trigger: a tap on the video shows the controls; a second tap hides them. 3 s without input hides
  them.
- From: controls hidden.
- To: controls shown, and back.

The stage is the phone at 1:1 in landscape, 915 × 412 dp, matching AndroidPlayer: the video is
732 × 412 at x 92, black either side.

| Element | From → to | Token | When |
|---|---|---|---|
| Controls: top bar, bottom bar and the 35 % ground dim, one layer | opacity 0 → 1, 1 → 0 | fade-in in, fade-out out | in on a tap; out 3 s after the last input |
| Subtitles | 32 → 88 dp above the bottom edge, and back | standard | starts with the controls' fade, either way |
| Seek track | 16 → 24 dp high about its centre line, outer corners 8 → 12 | quick | on touch down, back on release |
| Slider handle | 4 → 2 dp wide | system (the Slider's own pressed state) | while pressed |
| Preview frame and time label | opacity 0 → 1, 1 → 0 | fade-in, fade-out | on touch down, on release |
| Skip opening (and recap, ending, preview) | opacity 0 → 1, 1 → 0 | standard | in at the segment's start while the controls are hidden; out when the controls come up; back when they go; out after a tap |
| Skip strength | 1 → 0.4, 0.4 → 1 | fade-out, fade-in | dims 8 s after the segment starts; full while a finger is on it |

Nothing slides or scales. The video never moves.

Geometry, from AndroidPlayer and AndroidPlayerUpNext:

- Top bar: left 48, right 48, top 16, 48 high: back, show and episode, Subtitles and audio,
  Episodes. Bottom bar: left 48, right 48, bottom 28, 48 high: back 10 s, pause, forward 10 s
  (48 each, 4 apart), elapsed (44 wide), the slider (539 wide, a 48 dp target), remaining (52).
- Slider: a 16 dp track with 2 dp inner corners and a 6 dp gap either side of a 4 × 44 amber
  handle. Played time amber, the rest #434653, opening and ending sections in haze #9298B5 with a
  2 dp gap at each edge. No stop dot. Sections show only after the handle; before it the track is
  all amber.
- Preview: 160 × 90 at corners 12 with a 1 dp silk outline at 24 %, and under it, 4 apart, a 24 dp
  time label (surfaceContainer pill, 12/500 tabular: "21:31 · Ending", the section's name when the
  handle is in one). The pair's bottom sits 12 dp above the handle's top, centred on the handle and
  clamped to x 48 to 707.
- Skip: right 48, bottom 88, a 40 dp surfaceContainer pill in a 48 dp target, "Skip opening" in
  14/500 and a skip-next icon. It shares the corner with Up Next; one card at a time.

Thresholds and holds:

- Idle: 3 s after the last tap, drag end, key or media button.
- The controls never hide while scrubbing, while paused, while a menu or sheet is open, or while
  TalkBack or Switch Access is on. The count starts again when the hold ends.
- Skip dims 8 s after the segment starts (the engine's SkipAvailable), counted even while it is out
  for the controls. With TalkBack or Switch Access on it stays at full strength.
- Taps: in the centre third the controls toggle on release. On the side thirds the tap waits for
  `viewConfiguration.doubleTapTimeoutMillis` (300 ms), because a double tap there seeks 10 s.
- The slider target is 48 dp high. While dragging, a second libmpv instance renders the preview
  with keyframe seeks; release seeks the main player exactly.

Compose:

```kotlin
Box(Modifier.fillMaxSize()) {
    AndroidView(factory = { SurfaceView(it) }, modifier = videoModifier)   // libmpv
    Box(Modifier.fillMaxSize().graphicsLayer { alpha = ground.value }.background(Ground))
    AnimatedVisibility(chromeVisible, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
        PlayerChrome()                       // the 35 % dim, the top bar, the bottom bar; never slideIn
    }
    SkipOffer()
    UpNextCard()
    StillWatching()
}

LaunchedEffect(chromeVisible, lastInput, held) {   // held: scrubbing, paused, a menu or sheet, TalkBack
    if (chromeVisible && !held) { delay(3_000); chromeVisible = false }
}

val subtitleLift by animateFloatAsState(if (chromeVisible) 88f else 32f, Motion.standard())
LaunchedEffect(subtitleLift) { player.setSubtitleMargin(subtitleLift) }    // mpv's subtitle position

val touch = remember { MutableInteractionSource() }
val pressed by touch.collectIsPressedAsState()
val dragged by touch.collectIsDraggedAsState()
val trackHeight by animateDpAsState(if (pressed || dragged) 24.dp else 16.dp, Motion.quick())
Slider(
    state = seek,
    interactionSource = touch,
    thumb = { SliderDefaults.Thumb(touch, thumbSize = DpSize(4.dp, 44.dp)) },
    track = { SeekTrack(it, sections, trackHeight) },   // draws the sections and their 2 dp gaps
)
LaunchedEffect(seek) {
    snapshotFlow { sections.at(seek.value) }.drop(1).collect {
        if (dragged) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
    }
}

// Skip: the 8 s count belongs to the segment, not to the button's visibility
var dimmed by remember { mutableStateOf(false) }
LaunchedEffect(segment?.id) { dimmed = false; delay(8_000); if (!accessibilityHolds) dimmed = true }
AnimatedVisibility(segment != null && !chromeVisible,
    enter = fadeIn(Motion.standard()), exit = fadeOut(Motion.standard())) {
    val down by skipTouch.collectIsPressedAsState()
    val strength by animateFloatAsState(if (dimmed && !down) 0.4f else 1f,
        if (dimmed && !down) Motion.fadeOut() else Motion.fadeIn())
    SkipButton(segment, Modifier.graphicsLayer { alpha = strength }, interactionSource = skipTouch,
        onClick = { haptics.performHapticFeedback(HapticFeedbackType.ContextClick); engine.skip(segment) })
}
```

- Haptic: `SegmentTick` each time the section under the handle changes during a drag, in either
  direction (API 34; below it, `CLOCK_TICK`). `ContextClick` on Skip.
- Remove animations: we never override the scale. Timed motion lands at once: the fades, the
  subtitle lift, the track height. Scrubbing follows the finger. The 3 s and 8 s timers still run.
- Board: AndroidMotionPlayer, chapter 1 "Controls and Skip" (11.6 s) and the first 6.2 s of
  chapter 2 "Scrub and Up Next".

Board timeline, chapter 1 (ms at 1×). Position 2:08.5 at 0, playing.

- 1500: the opening starts (2:10). Skip in on standard, settled by 1816.
- 3000–3100: tap on the video at (458, 170). 3100: controls in, fade-in to 3300. Skip out on
  standard. Subtitles 32 → 88 on standard.
- 3100–6100: idle timer. 6100: controls out, fade-out to 6400. Subtitles 88 → 32. Skip back.
- 9500: 8 s after the opening started, Skip dims to 40 %, fade-out to 9800.
- 10500: finger down on Skip at (793, 300): back toward 100 %, fade-in.
- 10650: release. ContextClick, an instant seek to 3:40, Skip out on standard.
- Ends at 11600; each chapter holds 1 s, then the next plays.

Board timeline, chapter 2 up to the ending. Position 20:58 at 0, controls hidden.

- 600–700: tap at (458, 170). 700: controls in to 900, subtitles up.
- 1300: finger down on the handle (20:59.3, x 735). Track 16 → 24 on quick, handle 4 → 2,
  preview and time in, fade-in to 1500.
- 1300–2300: the drag runs to 21:40 (smoothstep), crossing the ending mark at 1996: SegmentTick.
- 2300–2900: back to 21:27, crossing the mark again at 2679: SegmentTick.
- 2900: release at 21:27. Exact seek, track back to 16, handle to 4, preview out on fade-out to 3200.
- 2900–5900: idle timer. 5900: controls out to 6200, subtitles down.

### T5 Up Next

- Trigger: the ending starts (the engine's ending mark), or a seek lands inside it.
- From: the episode playing.
- To: the next episode.

| Element | From → to | Token | When |
|---|---|---|---|
| Up Next card | opacity 0 → 1 | standard | the ending starts, or a seek lands inside it |
| Countdown bar and "Plays in n s" | 0 → 100 % over 10 s of playback | linear, on the countdown's clock | from the card's appearance |
| #161A2E layer over the video | 0 → 1 | `tween(250, easing = FastOutLinearInEasing)` | at zero |
| Up Next card | 1 → 0 | the same 250 ms tween | at zero, with the layer |
| #161A2E layer | 1 → 0 | fade-in | on the next episode's first frame |
| Episode line in the top bar | crossfade | standard | the new episode's first frame |
| Up Next card, Cancel | 1 → 0 | quick | on Cancel |
| Still watching? dialog and its scrim, black at 32 % | 0 → 1, 1 → 0 | standard in, quick out | zero on the third episode in a row with no input |

The card never rises: it sits at right 48, bottom 88, 336 wide (surfaceContainer, corners 16,
padding 16 16 8): a 96 × 54 still of episode 8 at corners 8, "UP NEXT", "Episode 8",
"A Town Without Night", the 4 dp bar with its 4 dp gap, then "Plays in 8 s", Cancel and Play now
(40 dp in 48 dp targets). Never black between episodes: the last frame fades through the ground.

Rules:

- The countdown is drawn from a clock, not animated: played seconds since the card appeared, so it
  pauses with the video, starts again at 10 s after a seek into the ending, and still counts with
  Remove animations. "Plays in n s" rounds up. The card appears at 21:31; at 21:33 it reads
  "Plays in 8 s" with the bar at 20 %.
- No ending data: the card appears 30 s before the end of anything over 2 minutes.
- At zero the next episode loads in the same player and the same SurfaceView. If its first frame
  is not up 1 s after the layer is full, the ground shows "Starting episode 8" with the 32 dp
  indicator, as in T2.
- Play now: `ContextClick`, then the zero sequence at once.
- Cancel: the card leaves on quick, the countdown stops, and nothing auto-plays for the rest of
  episode 7. At its end the player closes to the title page, as after the last available episode.
  No haptic.
- Still watching?: the count is episodes played back to back with no touch, key or media button
  input; any input resets it. When zero comes on the third, playback pauses, the card leaves on
  quick and the dialog fades in. It is drawn in the player, styled as a dialog (surfaceContainerHigh,
  corners 28, 312 wide, centred, over the black 32 % scrim: "Still watching?" in titleLarge,
  "Paused after 3 episodes in a row.", Stop as a text button, Continue watching as a tonal button),
  not a `Dialog` window, so the immersive player keeps its system bars hidden. Continue watching:
  `ContextClick`, the dialog out on quick, the zero sequence. Stop closes the player.
- Auto-play next off: the card shows with no bar and no countdown and stays to the end of the
  episode. TalkBack or Switch Access on (`AccessibilityManager.isTouchExplorationEnabled`, or
  Switch Access among `getEnabledAccessibilityServiceList`): the card waits; nothing counts down,
  and skip offers stay for the whole segment.
- With the controls up, the card stays and keeps counting; the subtitles lift under it as in T4.

Compose:

```kotlin
AnimatedVisibility(showUpNext, enter = fadeIn(Motion.standard()), exit = fadeOut(Motion.quick())) {
    UpNextCard(next, secondsLeft = ceil(10 - countdown.played).toInt())    // no slide, no rise
}
// the bar is drawn from the countdown's clock, not animated, so it still counts at scale 0
LinearProgressIndicator(progress = { countdown.played / 10f }, gapSize = 4.dp, drawStopIndicator = {})

suspend fun playNext() {
    ground.animateTo(1f, tween(250, easing = FastOutLinearInEasing))   // the card fades with it
    engine.playNext()                                                   // same player
    firstFrame.await()
    ground.animateTo(0f, Motion.fadeIn())
}
AnimatedContent(episode, transitionSpec = {
    fadeIn(Motion.standard()) togetherWith fadeOut(Motion.standard())
}) { EpisodeLine(it) }

AnimatedVisibility(stillWatching, enter = fadeIn(Motion.standard()), exit = fadeOut(Motion.quick())) {
    StillWatchingDialog(onContinue = { haptics.performHapticFeedback(HapticFeedbackType.ContextClick); resume() },
        onStop = ::close)
}
```

- Haptic: `ContextClick` on Play now and Continue watching.
- Remove animations: we never override the scale. Timed motion lands at once: the card, the ground
  layer, the dialog. The countdown runs on the playback clock, so it still counts.
- Board: AndroidMotionPlayer, chapter 2 from 6.9 s, chapter 3 "Cancel" (4.2 s) and chapter 4
  "Still watching?" (5.4 s).

Board timeline, chapter 2 from the ending:

- 6900: the ending starts (21:31). Card in on standard, settled by 7216. Countdown from 6900.
- 8900: 21:33, "Plays in 8 s", bar 20 %.
- 16900: zero. The layer 0 → 1 and the card 1 → 0 over 250 ms (FastOutLinearIn).
- 17150: episode 8 under the layer; the layer 1 → 0 on fade-in to 17350. The episode line
  crossfades on standard. The slider reads 0:00 and −24:02.

Chapter 3, Cancel. Position 21:30 at 0.

- 1000: the ending starts. Card in on standard, countdown from 1000.
- 2600–2700: tap Cancel at (695, 292). 2700: card out on quick, gone by 2910. The bar stops at
  17 %, "Plays in 9 s". The ending plays on.

Chapter 4, Still watching?. Position 21:39, the card up with 2 s left, 80 %.

- 2000: zero. Playback pauses at 21:41. Card out on quick. Dialog and scrim in on standard.
- 3600–3700: tap Continue watching at (506, 252). 3700: ContextClick, dialog out on quick, the
  layer to full by 3950, episode 8, the layer clear by 4150.

### T6 Navigation bar destinations

- Trigger: a tap on a destination in the navigation bar (or the rail, AT1).
- From: one destination's back stack.
- To: another's, as it was left.

- The page under the bar, its top app bar included, fades through on quick: Material's
  fade-through, the old page out and the new one in together, NavHost's enter `fadeIn(quick)` and
  exit `fadeOut(quick)`. No slide and no scale. The bar itself is outside the NavHost and takes no
  part.
- The indicator pill is the system's: it grows from its centre to 56 × 32 on the new destination
  and shrinks to its centre on the old one; the icon changes to its filled form and the label to
  amber. That is ShortNavigationBar's own animation, drawn on the board at about 0.2 s. Don't
  restyle it.
- Each destination keeps its back stack and scroll position and comes back as it was left.
  Tapping the current destination scrolls it to the top and pops to its start. The large top app
  bar collapses with the scroll (system), and the navigation bar stays visible while scrolling.

Compose:

```kotlin
NavigationSuiteScaffold(navigationSuiteItems = { /* ShortNavigationBarItem × 5: Continue, Calendar,
                                                     Seasonal, Library, Search */ }) {
    NavHost(nav, startDestination = Continue /* the transitions in Phone above:
        fadeIn(Motion.quick()) and fadeOut(Motion.quick()) between top-level destinations */) { … }
}
nav.navigate(dest) {
    popUpTo(nav.graph.findStartDestination().id) { saveState = true }
    launchSingleTop = true
    restoreState = true
}
```

- Haptic: none.
- Remove animations: we never override the scale; the fade-through is timed, so the page swaps at
  once. The bar does what the system does.
- Board: AndroidMotionFeedback, T6 (3.6 s), on the AndroidHome and AndroidLibrary screens.

Board timeline:

- 500–600: tap Library at (288, 849). 600: Library's pill grows from its centre, Continue's
  shrinks, icons and labels swap (system, drawn 600–800). The page fades through on quick,
  600–810.
- 2100–2200: tap Continue at (41, 849). 2200: the same back.

### T7 Sheets and menus

- Trigger: Watching on the title page, the season picker, More › Settings for this show, Source,
  subtitles and audio.
- From: the button.
- To: a dropdown menu (Watching, More), a connected button group's new selection (seasons), or a
  modal bottom sheet (Settings for this show, Source, subtitles and audio).

DropdownMenu (Watching):

- Opens from its anchor, the Watching button at (16, 348, 186 × 48): it grows from 0.8 at the
  anchor's corner with a fade, and closes with a fade. That is the component's own (system), drawn
  at about 0.12 s in and 0.075 s out. The menu is surfaceContainerHigh at corners 4, items 48 high
  in labelLarge, at (16, 396), 200 wide.
- The current choice has a leading silk check; every item keeps the 24 dp leading slot so the
  labels line up. Nothing in a menu is amber.
- After a pick the button's label crossfades from Watching to Paused on quick. The button does not
  change size.

Connected button group (the season picker):

- Season 2 takes the selection. Its corners go from 8 to fully round; Season 1's inner corners go
  from fully round back to 8; the containers swap between surfaceContainer and primaryContainer and
  the labels between silk and onPrimaryContainer. That is ToggleButton's shape and colour morph on
  `MotionScheme.standard()` (system), which settles without a visible overshoot; drawn at about
  0.3 s.
- The check is ours: it fades in on fade-in while its 24 dp slot (18 icon and a 6 gap) opens on
  standard, so the label makes room; the old one leaves on quick and its slot closes on standard.
- The widths stay 1.4 : 1 : 1. The episode list below crossfades on standard; for Season 2 it is
  one line, "Airs from spring 2027".

ModalBottomSheet (More › Settings for this show):

- Rises from the bottom to its top at 77 over the scrim, black at 32 % (system, drawn at about
  0.35 s). Top corners 28, surfaceContainer, the 32 × 4 drag handle in onSurfaceVariant at 40 %,
  centred in a 48 dp row.
- It follows a drag on the handle or the content 1:1. Released less than 56 dp down and slower than
  125 dp/s, it settles back; past either, it hides. These are BottomSheetDefaults; keep them. Back,
  predictive back and a tap on the scrim also close it.
- This sheet is taller than half the screen and has one detent: `skipPartiallyExpanded = true`.
  Source, subtitles and audio keep partial and expanded, which stand in for Apple's medium and
  large.
- Android has no morph out of the button, and we do not imitate one. A menu does not dim the page;
  only sheets and dialogs sit over a scrim.

Compose:

```kotlin
Box {
    FilledTonalButton(onClick = { open = true }) {
        AnimatedContent(list, transitionSpec = { fadeIn(Motion.quick()) togetherWith fadeOut(Motion.quick()) }) {
            Text(it.title)
        }
        Icon(Icons.Rounded.ExpandMore, null)
    }
    DropdownMenu(expanded = open, onDismissRequest = { open = false },
        containerColor = SurfaceContainerHigh, shape = RoundedCornerShape(4.dp)) {
        LibraryList.entries.forEach {
            DropdownMenuItem(
                text = { Text(it.title) },
                onClick = { pick(it) },
                leadingIcon = {
                    if (it == list) Icon(Icons.Rounded.Check, null, tint = Silk)
                    else Spacer(Modifier.size(24.dp))
                },
            )
        }
    }
}

Row(horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
    seasons.forEachIndexed { i, season ->
        ToggleButton(
            checked = season == selected, onCheckedChange = { selected = season },
            shapes = when (i) {
                0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                seasons.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
            },
            modifier = Modifier.weight(if (i == 0) 1.4f else 1f),
        ) {
            AnimatedVisibility(season == selected,
                enter = fadeIn(Motion.fadeIn()) + expandHorizontally(Motion.standard()),
                exit = fadeOut(Motion.quick()) + shrinkHorizontally(Motion.standard())) {
                Icon(Icons.Rounded.Check, null, Modifier.padding(end = 6.dp).size(18.dp))
            }
            Text(season.label)
        }
    }
}
Crossfade(selected, animationSpec = Motion.standard()) { EpisodeList(it) }

if (showSettings) ModalBottomSheet(
    onDismissRequest = { showSettings = false },
    sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),   // Source: false
    containerColor = SurfaceContainer,
    scrimColor = Color.Black.copy(alpha = 0.32f),                               // the scrim role
    dragHandle = { BottomSheetDefaults.DragHandle(color = Haze.copy(alpha = 0.4f)) },
) { ShowSettings() }
```

- Haptic: none of ours.
- Remove animations: we never override the scale. Timed motion lands at once: the menus, the
  group, the label, the check and the list. The sheet follows a drag, and its settle snaps.
- Board: AndroidMotionFeedback, T7 (8.2 s), on AndroidTitle and AndroidShowSettings.

Board timeline:

- 300–400: tap Watching at (109, 372). 400: menu opens, drawn to 520.
- 1300–1400: pick Paused at (100, 572). 1400: menu closes, drawn to 1475; label crossfade on quick.
- 2200–2300: tap Season 2 at (228, 472). 2300: shape and colour morph, drawn to 2600; check in;
  list crossfade on standard.
- 3300–3400: tap Season 1 at (93, 472). 3400: the same back.
- 4400–4500: tap More at (384, 72). 4500: More menu opens from its top-right corner, drawn to 4620.
- 5000–5100: pick Settings for this show at (270, 128). 5100: menu closes; the sheet rises from
  915 to 77 and the scrim comes up, drawn to 5450.
- 5900–6300: drag the handle down 40 dp, the sheet following. 6300: release under 56 dp; it
  settles back, drawn to 6550.
- 6900–7500: drag down 160 dp, passing 56 dp at 7139. 7500: release; the sheet hides and the
  scrim clears, drawn to 7800. A dashed line on the stage marks 56 dp below the sheet's top.

### T8 Loading to content

- Trigger: a screen opens before the engine's first list for it has arrived.
- From: nothing in the list's place, then static placeholders.
- To: the real rows, then their art.

- For the first 600 ms after the screen opens, nothing is drawn where the list goes: only the
  chrome (top app bar, navigation bar) and rows from local data (Downloads, "3.4 GB on this
  device").
- At 600 ms, if the list is still missing, static placeholders in the real layout fade in on
  fade-in: covers as surfaceContainer rects at their real size, text as silk bars at 12 % the size
  of the real text, chips as outlines. No shimmer, no pulse, no stagger.
- Once shown, placeholders stay at least 0.3 s, so a list that lands at 650 ms waits until 900.
- The list replaces them in one crossfade on standard. Each cover then shows a flat rect in its
  muted colour, AniList's `coverImage.color` at 40 % over #161A2E, until its art arrives and fades
  in on fade-in, in arrival order. Art from the memory cache shows at once, without a fade.
- If the list arrives within 600 ms, there are no placeholders: the content fades in on standard.
- Nothing shifts: placeholders and covers have the real sizes.
- Snackbar: enters 12 dp above the navigation bar, 16 from the sides, and leaves; both the
  component's own (system, a short fade with a scale up from 0.8, drawn at 0.15 s in and 0.075 s
  out). It stays 4 s, 8 s with an action, plus 1 s for every 10 words past 12.

Compose:

```kotlin
var phase by remember { mutableStateOf(Phase.Waiting) }
LaunchedEffect(Unit) {
    delay(600)
    if (items == null) { phase = Phase.Placeholders; placeholdersShownAt = now() }
}
LaunchedEffect(items) {
    if (items != null) {
        if (phase == Phase.Placeholders) delay((300 - (now() - placeholdersShownAt)).coerceAtLeast(0))
        phase = Phase.Content
    }
}
AnimatedVisibility(phase == Phase.Placeholders, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.standard())) {
    LibraryPlaceholders()                                 // flat shapes, never shimmer
}
AnimatedVisibility(phase == Phase.Content, enter = fadeIn(Motion.standard())) { LibraryGrid(items) }

// each cover: a flat muted rect, the art fading in over it
val art = remember { Animatable(if (cached) 1f else 0f) }
Box(Modifier.background(muted(coverColor))) {
    AsyncImage(url, null, onSuccess = { scope.launch { art.animateTo(1f, Motion.fadeIn()) } },
        modifier = Modifier.graphicsLayer { alpha = art.value })
}

// the snackbar, with our own timing
withTimeoutOrNull(if (action != null) 8_000 else 4_000) {
    snackbars.showSnackbar(message, actionLabel = action, duration = SnackbarDuration.Indefinite)
}
```

- Haptic: none.
- Remove animations: we never override the scale. Timed motion lands at once; the 600 ms wait and
  the 0.3 s hold still apply.
- Board: AndroidMotionFeedback, T8 (7.9 s), on AndroidLibrary.

Board timeline:

- 0–600: nothing in the list's place.
- 600: placeholders in on fade-in, to 800; held to at least 900.
- 1500: the list arrives. Placeholders out and content in, one crossfade on standard to 1816.
- Cover art arrives at 1700 (Hoshikuzu no Tabibito), 1820 (Kumo no Ue no Toshokan), 1900
  (Natsukaze Relay), 2000 (Hagane no Niwashi), 2150, 2300, 2420, 2600 and 2950; each fades in over
  200 ms.
- 3500: snackbar "2 changes synced to AniList" in, drawn to 3650. 7500: out, drawn to 7575.

### T9 Marking watched

- Trigger: Mark as watched in the row's menu (a long press), or playback passing counts as watched
  at (the ending's start when it is marked, otherwise 85 %).
- From: the episode unwatched or in progress.
- To: the episode watched, the next one current.

- The long press opens a DropdownMenu at the finger (system): Mark as watched, Play from the
  beginning. `combinedClickable` plays its own `LongPress` haptic.
- In order:
  1. The row's progress bar fills from 42 % to 100 % on standard.
  2. Then, on quick: the title dims from silk to haze, the still from 100 % to 70 %, and the second
     line crossfades from the bar and "E7 · 14 min left" to "E7 · 24 min". At the same moment the
     check at the row's end draws itself on: an AnimatedVectorDrawable with `trimPathEnd` 0 → 1
     over 300 ms, `fast_out_slow_in`, in haze.
  3. With step 2, on standard: the season button's "6/24" rolls to "7/24" (only the changed digit
     moves, 10 dp, the old one up and out, the new one up from below); the main button's label
     crossfades from "Resume E7 · 10:06" to "Start E8"; episode 8's still crossfades from blurred
     to clear, since it is now the next episode (spoiler protection).
- Snackbar: "Episode 7 marked as watched" with Undo, 8 s (system in and out).
- Undo, or Mark as unwatched: all of it in reverse. The check fades out and the row brightens on
  quick; then the bar returns from 100 % to 42 % on standard. The count rolls back the other way
  (the 7 down and out, the 6 down from above), the label and the still go back.
- What stays: row positions and heights. The check takes the row's trailing slot, as on episode 6.
- When playback marks it, it happens in the player: back on the title page the row shows the
  result at once, without replaying the motion, and no haptic plays.

Compose:

```kotlin
Row(Modifier.combinedClickable(onClick = { play(episode) }, onLongClick = { menu = true })) { … }

LaunchedEffect(episode.watched) {
    if (!changedByUser) {                       // playback marked it: show the result, no motion
        progress.snapTo(if (episode.watched) 1f else episode.fraction)
        dim.snapTo(if (episode.watched) 1f else 0f)
        return@LaunchedEffect
    }
    haptics.performHapticFeedback(if (episode.watched) HapticFeedbackType.ToggleOn else HapticFeedbackType.ToggleOff)
    if (episode.watched) {
        progress.animateTo(1f, Motion.standard())
        dim.animateTo(1f, Motion.quick())
    } else {
        dim.animateTo(0f, Motion.quick())
        progress.animateTo(episode.fraction, Motion.standard())
    }
}
val titleColor = lerp(Silk, Haze, dim.value)       // the still: alpha = 1f - 0.3f * dim.value

val check = AnimatedImageVector.animatedVectorResource(R.drawable.avd_check_draw_on)
Icon(
    painter = rememberAnimatedVectorPainter(check, atEnd = episode.watched),
    contentDescription = if (episode.watched) "Watched" else null,
    tint = Haze,
)

// "6/24": per-digit AnimatedContent on standard, as in T3
AnimatedContent(digit, transitionSpec = {
    val up = targetState > initialState
    (slideInVertically(Motion.standard()) { if (up) it / 2 else -it / 2 } + fadeIn(Motion.standard())) togetherWith
        (slideOutVertically(Motion.standard()) { if (up) -it / 2 else it / 2 } + fadeOut(Motion.standard()))
}) { Text("$it") }

withTimeoutOrNull(8_000) {
    if (snackbars.showSnackbar("Episode 7 marked as watched", actionLabel = "Undo",
            duration = SnackbarDuration.Indefinite) == SnackbarResult.ActionPerformed) markUnwatched()
}
```

- Haptic: `LongPress` as the menu opens (the component's own), `ToggleOn` when watched turns true,
  `ToggleOff` when it turns false (both API 34; below it, `CONTEXT_CLICK`). Marking watched plays
  ToggleOn and ToggleOff, not `Confirm`. None when playback marks it.
- Remove animations: we never override the scale. Timed motion lands at once, the check included:
  the fill, the dim, the drawn check and the count.
- Board: AndroidMotionFeedback, T9 (5 s), on AndroidTitle, episode 7's row.

Board timeline:

- 400: finger down on the row at (260, 680); the pressed state layer shows.
- 800: the long-press timeout. LongPress haptic; the menu opens at (140, 690), drawn to 920.
- 1500–1600: tap Mark as watched at (250, 722). 1600: the menu closes; ToggleOn; the bar fills on
  standard to 1916; the snackbar comes in, drawn to 1750.
- 1920: the dim on quick (to 2130), the check drawn on (to 2220), and on standard the count roll,
  the button label and episode 8's still (to 2236).
- 3600–3700: tap Undo at (363, 791). 3700: ToggleOff; the snackbar leaves; the check out and the
  row back on quick (to 3910); the count, label and still back on standard.
- 3910: the bar returns to 42 % on standard, settled by 4226.

## Picture in Picture

- Trigger: going home while a video plays. Auto-enter is on only while playing; when paused, going
  home leaves the app.
- From: the full-screen player.
- To: the system's window, and back.

The system's: the move from the video's bounds into the window, the window's size, corners, shadow
and position, dragging it, pinch resize, stashing it at the edge, the menu a tap opens (settings,
close, expand), the expand back to full screen and the close. We don't animate or restyle any of
it. If the launcher is portrait, the rotation is the system's too, in the same move.

Ours:

- `PictureInPictureParams`, set again whenever an input changes: `setAspectRatio` from the video
  (16:9 here); `setSourceRectHint` with the SurfaceView's bounds in the window (92, 0, 824, 412 dp
  here), updated on every layout; `setAutoEnterEnabled(isPlaying)`; `setSeamlessResizeEnabled(true)`,
  since this is video; and three `RemoteAction`s: back 10 s, play or pause (its icon and title
  follow the state), forward 10 s.
- Our overlays (controls, dim, preview, Skip, Up Next, the Still watching? dialog) go at once when
  the transition starts, with no fade, so the window shows the video alone. On API 35 hide them on
  `onPictureInPictureUiStateChanged` when `isTransitioningToPip()`; below, on
  `onPictureInPictureModeChanged`. Subtitles are drawn in the video and scale with it; their lift
  drops to 32 at once.
- The SurfaceView is the same one, so playback does not restart.
- Expanded again, the controls fade in (fade-in) and hide after 3 s.
- In the window, auto-skip, auto-play next and the fade through #161A2E still run; skip offers and
  the Up Next card are not shown. Still watching? pauses and its dialog waits for expand. Closing
  the window stops playback and saves the position.

Compose and the manifest:

```kotlin
// <activity android:supportsPictureInPicture="true"
//     android:configChanges="orientation|screenSize|smallestScreenSize|screenLayout|density" />
LaunchedEffect(videoBounds, isPlaying, aspect) {
    activity.setPictureInPictureParams(
        PictureInPictureParams.Builder()
            .setAspectRatio(aspect)                       // Rational(16, 9) here
            .setSourceRectHint(videoBounds)
            .setAutoEnterEnabled(isPlaying)               // API 31; earlier, enter in onUserLeaveHint
            .setSeamlessResizeEnabled(true)
            .setActions(listOf(back10, playPause, forward10))
            .build())
}
Modifier.onGloballyPositioned { videoBounds = it.boundsInWindow().toAndroidRectF().toRect() }
DisposableEffect(activity) {
    val listener = Consumer<PictureInPictureModeChangedInfo> { inPip = it.isInPictureInPictureMode }
    activity.addOnPictureInPictureModeChangedListener(listener)
    onDispose { activity.removeOnPictureInPictureModeChangedListener(listener) }
}
// RemoteAction(icon, title, contentDescription, PendingIntent.getBroadcast(…)) into the Media3 session
```

- Haptic: none.
- Remove animations: the window's motion is the system's; our overlays already go at once.
- Board: AndroidMotionPlayer, chapter 5 (6.2 s).

Board timeline, chapter 5. Position 12:30 at 0, playing.

- 400–500: tap. 500: controls in on fade-in, subtitles up on standard.
- 1500–1900: the home gesture, a swipe up from the bottom edge, (458, 404) to (458, 300).
- 1700: auto-enter. Our overlays go at once; subtitles down at once. The video moves into the
  window, drawn at about 0.45 s (system): from 732 × 412 at (92, 0) to 256 × 144 at (643, 236),
  corners 0 → 12; the home screen comes up behind it.
- 3000–3100: tap on the window. 3100: the system's menu, drawn as a 0.15 s fade: our three actions
  along the bottom, settings and close at the top, expand in the middle.
- 4000–4100: tap expand. 4100: the window returns to full screen, drawn at about 0.45 s.
- 4550: controls in on fade-in, subtitles up.

## Tablet and foldable

These exist only where the window is medium or expanded: tablets, and foldables unfolded. Phones
never see them. Tablets and unfolded foldables rotate freely; the app never asks them for an
orientation.

### AT1 Rail to modal drawer

- Trigger: the menu button at the top of the rail. Back, the scrim, the collapse button or picking
  a destination close it.
- From: the collapsed rail, 96 wide on surface: the five destinations, and the account at the foot.
- To: the modal drawer, 220 wide on surfaceContainerLow with corners 16 on the end side, over the
  scrim, black at 32 %. It adds Downloads, a "Sources" header with Providers and Plugin stores, and
  "Accounts and settings" at the foot.

Compose:

```kotlin
val rail = rememberWideNavigationRailState()
ModalWideNavigationRail(
    state = rail,
    hideOnCollapse = false,
    header = { IconButton(onClick = { scope.launch { rail.toggle() } }) {
        Icon(if (rail.targetValue == WideNavigationRailValue.Expanded) MenuOpen else Menu) } },
) {
    Destinations.forEach { WideNavigationRailItem(selected = it == current, onClick = { go(it) }, icon = { … }, label = { Text(it.label) }) }
    // expanded only: Downloads, then a "Sources" header with Providers and Plugin stores
}
```

- Timing: system. MotionScheme's default spatial spring (0.9, 700) moves the width and the labels;
  the default effects spring (1, 1600) fades the scrim. About 0.25 s.
- What moves: the rail widens from 96 to 220 and takes the drawer's surface; each label moves from
  under its icon to beside it; Downloads and Sources appear; the scrim rises over the panes.
- What stays: the list and detail panes. The drawer lies over them and nothing reflows.
- Gesture: tap. A swipe toward the start edge closes it (system).
- Haptic: none.
- Remove animations: timed motion lands at once, so the drawer appears and leaves at once (system).
- Board: AndroidMotionTablet, 0 → 2.4 s of a 10.2 s timeline.

Board timeline: 400–520 tap the menu button at (48, 72); 520 the rail widens (drawn 0.24 s) and
the scrim comes up (drawn 0.16 s); 1600–1720 tap the scrim at (820, 420); 1720 the drawer
collapses back into the rail.

### AT2 List to detail, and the divider

- Trigger: a tap on a row in the list pane; a drag on the pane divider's handle.
- From: one title selected, its detail on the right; the list pane 380 wide.
- To: another title selected and in the detail pane; the divider at an anchor.

Compose:

```kotlin
val nav = rememberListDetailPaneScaffoldNavigator<TitleId>()
val split = rememberPaneExpansionState(
    anchors = listOf(
        PaneExpansionAnchor.Offset.fromStart(380.dp),     // the list pane's default
        PaneExpansionAnchor.Proportion(0.5f),             // half the content
    ),
)
ListDetailPaneScaffold(
    directive = nav.scaffoldDirective,
    value = nav.scaffoldValue,
    paneExpansionState = split,
    paneExpansionDragHandle = { state ->
        val interaction = remember { MutableInteractionSource() }
        VerticalDragHandle(
            modifier = Modifier.paneExpansionDraggable(
                state, LocalMinimumInteractiveComponentSize.current, interaction),
            interactionSource = interaction,
        )
    },
    listPane = { AnimatedPane { Queue(selected, onPick = { scope.launch { nav.navigateTo(ListDetailPaneScaffoldRole.Detail, it) } }) } },
    detailPane = {
        AnimatedPane {
            AnimatedContent(
                targetState = selected,
                transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeIn()) },
                contentKey = { it },
            ) { Detail(it) }
        }
    },
)
```

When the selection changes again within 0.15 s of the last change (a held arrow key, a fast run of
taps), set it without the fade, as M2 does on the Mac.

- Timing: the selection's primaryContainer fill on quick. The detail's content on fade-in, keyed by
  title id. The handle widens on press (quick). The divider follows the finger and settles on the
  nearer anchor (system, the default spatial spring).
- What moves: the fill leaves the old row and arrives on the new one; the detail crossfades.
  Dragged, the list pane widens and the detail reflows, its banner keeping its shape; released, the
  divider settles on 380 or on half the content (568 on a 1280 window with the rail).
- What stays: the rail, the list's scroll position, the selection while dragging.
- Gesture: tap a row. Drag the handle: 4 × 48, 12 × 52 while held, in a 48 dp target.
- Haptic: none.
- Remove animations: timed motion lands at once: the selection and the content. The divider
  follows the finger, and its settle snaps to the anchor.
- Board: AndroidMotionTablet, 2.4 → 6.4 s.

Board timeline: 2800–2920 tap Hagane no Niwashi at (260, 252); 2920 the selection on quick and the
detail on fade-in, to 3120; 3800 press the divider, the handle widens on quick; 3900–4700 drag, the
list 380 → 532; 4700 release, it settles at 568 (drawn 0.24 s); 5200 press again; 5300–5900 drag
back; 5900 it settles at 380.

### AT3 Fold and unfold

- Trigger: folding or unfolding a book-style foldable, or any resize that crosses a width class
  (a split screen, a freeform window).
- From: unfolded, 840 × 700: rail, list and detail; the list scrolled 56 down, a title selected.
- To: folded, the 412 × 915 outer screen: one pane, the selected title's page. Then unfolded again.

Compose:

```kotlin
// AndroidManifest: the activity keeps running through the change, so nothing is recreated
android:configChanges="screenSize|smallestScreenSize|screenLayout|orientation|density"
android:resizeableActivity="true"

// the directive follows the window; the state lives above the scaffold
val nav = rememberListDetailPaneScaffoldNavigator<TitleId>(
    scaffoldDirective = calculatePaneScaffoldDirective(currentWindowAdaptiveInfo()),
)
val queue = rememberLazyListState()                      // the list's scroll, kept across the change
// the player's SurfaceView is resized, never recreated; libmpv keeps playing
```

- Timing: the fold is the person's and the display switch is the system's. On the unfolded screen
  the scaffold's pane motion (system, the default spatial spring) brings the list back.
- What moves: folding, the inner screen goes dark and the outer one lights with one pane, the
  detail, because a title was selected. Unfolding, the inner screen lights with that pane at full
  width; the list slides in from the start edge and the detail settles into its column.
- What stays: the list's scroll position, the selection, the detail's own scroll, and playback. An
  episode playing keeps playing through the change, with no restart and no rebuffer, while the
  player resizes. The outer screen is under 600 dp, so it follows the phone rule: the browse UI is
  portrait and a playing episode asks for landscape as in T2.
- Gesture: the posture. Back on the folded detail returns to the list at the same scroll position.
- Haptic: none.
- Remove animations: timed motion lands at once, the pane motion included. Nothing else differs.
- Board: AndroidMotionTablet, 6.4 → 10.2 s.

Board timeline: 7000–7800 fold, the inner screen off 7000–7200, the outer on 7800–8050; 8800–9600
unfold, the outer off 8800–9000, the inner on 9600–9850; 9650 the panes go from one to two (drawn
0.24 s). Scroll, selection and playback hold from 7000 to 9850.

## Remove animations

Android's Remove animations sets `ANIMATOR_DURATION_SCALE` to 0. We never override the scale.

- Timed motion lands at once. Compose and the platform scale every timed animation by the setting,
  ours and the system's alike, so no code of ours reads it or swaps one animation for another.
- Gesture-driven motion (predictive back, the T3 drag, the sheet drag) follows the finger, and its
  settle snaps: a commit, a close or a spring back lands at once at release.
- The Up Next countdown runs on the playback clock, so it still counts.

| Motion | Normally | With Remove animations |
|---|---|---|
| Timed motion: shared transitions (T1, T2), fades, springs, the check (T9), menus, sheets opening, the rail, panes (T6, T7, AT1 to AT3) | As specified | Lands at once |
| Predictive back, the T3 drag, the sheet drag (T1, T3, T7) | Follows the finger | Follows the finger; the settle snaps |
| The pane divider (AT2), the banner parallax (T1) | Follows the finger, or the scroll | The same; the divider snaps to its anchor |
| The Up Next countdown (T5) | 10 s, linear | Still counts, on the playback clock |
| Timers: 3 s idle, the 8 s Skip dim, the 600 ms wait and 0.3 s hold (T4, T8) | As specified | Unchanged |

## Never

- Bounce or overshoot: `MotionScheme.expressive()`, or a spring damped below 0.9.
- Sliding player controls. They fade.
- Shimmer, pulsing or moving placeholders.
- Staggered list entrances: no per-item delays in `animateItem`.
- Covers that pop in. They fade in when their art arrives.
- A rise on the Up Next card. Opacity only.
- A fade to black between episodes. It fades through the ground colour. "Never black" is about
  fades and empty frames; the scrim under sheets and dialogs is black at 32 %.
- A scrim anywhere but under a sheet or a dialog (the modal drawer of AT1 is a side sheet), or a
  scrim in any colour but black at 32 %. Nothing dims the page under T1 or its back gesture.
- A slide or scale between navigation bar destinations. They fade through on quick.
- The wavy progress indicator, or the Expressive loading indicator outside a button or pull to
  refresh. The player's starting screen uses a 32 dp CircularProgressIndicator.
- Glow, or motion used as decoration.
- Hiding the player controls while scrubbing or while a menu or sheet is open.
- Blocking input until an animation ends.
- Overriding the person's animator duration scale.
- Restarting playback, or losing scroll position or selection, on a fold, rotation or resize.
- A landscape title page on a phone.
- Animating Nagare.

## Implementation rules

Every spring is interruptible. `animate*AsState` and `Animatable.animateTo` start from the current
value and velocity, so a second tap during a transition retargets it instead of queueing. Use
springs for anything that moves so this holds.

Input is never blocked while something animates. Do not consume pointer input for the length of a
transition, and do not wait for an animation's end before accepting the next tap. The one exception
is hidden player chrome, which ignores taps because it is not there.

Animations are keyed by stable ids: the title id, the episode number, the list item's key. Lazy
lists use `items(key = { it.id })` with `Modifier.animateItem(fadeInSpec = Motion.standard(),
placementSpec = Motion.standard(), fadeOutSpec = Motion.standard())`; `AnimatedContent` and
`Crossfade` key by the same id. Shared element keys are `"cover-$id"`, `"art-$id"`, `"page-$id"`
and `"player-$id"`, never a position.

NavHost transitions follow the rule in Phone: top-level switches fade through on quick, pushes keep
the page underneath and let the shared bounds move.

State the view owns (selection, the route, a menu or sheet opening, the player chrome's visibility)
is changed where it happens and animates there. Engine events animate on the views that show them:
collect them with `collectAsStateWithLifecycle()` and animate the shown value with
`animate*AsState` or `AnimatedContent`. The engine never decides how something animates, and an
update that arrives while the view is not composed does not play later.

Orientation follows the rule in Phone: on phones the browse UI asks for portrait and the player for
landscape; tablets and foldables never ask.

## Haptics

Haptics go through `LocalHapticFeedback.current.performHapticFeedback(HapticFeedbackType.X)`, which
maps to the View's `HapticFeedbackConstants`. The toggle, segment and gesture-threshold constants
need API 34, and `Confirm` and `Reject` API 30; below that, use `CONTEXT_CLICK`, or `CLOCK_TICK`
for the ticks. A device without a vibrator plays nothing and nothing else changes. Nothing plays on
scroll.

| Apple | Android | Where |
|---|---|---|
| Light impact | `ContextClick` | Skip opening, Play now, Continue watching, Undo (T4, T5) |
| Light impact | `ToggleOn`, `ToggleOff` | Marked watched, and unmarked (T9) |
| Light impact | `GestureThresholdActivate` | The 120 dp dismiss threshold (T3), the 2× hold |
| Selection | `SegmentTick` | Scrubbing across an opening or ending mark (T4) |
| Selection while reordering | `SegmentFrequentTick` | Source order |
| Success, error | `Confirm`, `Reject` | Wherever Apple plays `.success` and `.error`, except marking watched, which is T9's toggles |
| The system's | the platform's | The back gesture's commit point, long press through `combinedClickable` |
| None | nothing | Scrolling, T1, T2, T6, T8, AT1 to AT3 |
