# Trailers

Written 24 October 2026, to build after the parity and website phase. Follows `docs/native-ux.md`; where this spec is silent, that one applies.

A title's official trailer plays, muted, in place of its banner. The banner shows at once, as it does today, and the trailer fades in over it only once it is playing, so the wait for the video is never seen. On Mac, Windows and Linux, resting the pointer on a cover in a browse grid opens a preview with the same behaviour.

The trailer is the one AniList lists for the title (`Media.trailer { id site }`). The app never searches a video site and never plays anything AniList does not name.

Better than Hayase because the trailer starts only when the device, the network and the viewer's settings allow it, the engine decides that once for every app, and it keeps to YouTube's embed rules, which Hayase's preview breaks.

## Compared with Hayase

Hayase opens a preview card 30 ms after the pointer touches a cover. The card shows the banner and lays a muted, autoplaying `youtube-nocookie.com` iframe on top, which fades in (0.3 s) when the player posts state 1. A second, blurred copy of the same iframe sits behind it as a glow, except on weak devices. At the end it reloads the iframe to loop.

Where it falls short:

- Feel. A 30 ms hover opens a card for every cover the pointer crosses, so moving across a grid flickers previews.
- Battery and network. Two players decode the same video for the glow, every preview loops until closed, and nothing checks metered networks, Low Data Mode or power saving.
- Accessibility. Nothing checks Reduce Motion, the system's video autoplay setting or a screen reader.
- The rules. Its preview's video area is about 280 × 144 px, under YouTube's 200 px minimum, and a gradient and a sound button are drawn over the player, which the terms forbid.
- Platform fit. It is one web page on every platform. Its touch path opens previews on a first tap.

What we have that it doesn't: native popovers and title pages per platform, one engine that knows the network path, the power state and the viewer's progress (for the sequel rule), and AniList's list status already synced.

## What exists

- `crates/anistream-meta/src/anilist/model.rs` reads `trailer { id site }` into `Trailer`, and `Media::trailer_url()` turns a YouTube or Dailymotion id into a watch URL.
- The field is in `MEDIA_DETAIL_FIELDS` only (`query.rs`), so a title's page has it and a list row does not.
- `Entry::trailer_url` carries the URL to every front end, and `Command::PlayTrailer` opens it where the host opens links. Every app has a Play trailer menu item.
- `Command::NetworkPath { cellular, constrained, offline }` already reaches the engine from every app's network monitor, and `PlayerReport::Power { on_battery, low_power }` does for the player.

## The YouTube embed rules

Trailers on AniList are almost all YouTube videos, and YouTube allows them in an app only through its embedded player. Its Required Minimum Functionality page sets rules that shape this whole feature:

1. Nothing may sit in front of any part of the player: no text, scrim, button or other view. The banner's scrim, the title drawn over the banner and the content that scrolls over the banner on iPhone must never cover a playing trailer.
2. The player's viewport is at least 200 × 200 px. A 16:9 trailer therefore needs at least 356 × 200 px, and the hover preview is sized from that.
3. Automatic playback starts only when more than half of the player is visible, and only one automatically playing YouTube player may be on screen at a time.
4. Mouseovers and touches on the player must not start an action on the viewer's behalf. The hover is on our cover, never on the player, and the player view takes no pointer or touch input at all.
5. The app identifies itself with the HTTP `Referer`, whose host is the app's identifier as its store or the system knows it: `https://tv.anistream.app` on Apple platforms and Android, the package `Name` on Windows, `https://tv.anistream.desktop` on Linux. A web view sends no Referer by default, and an embed without one fails with error 153.
6. Use the system's own web view.
7. Only changes the player API describes. We use the documented player parameters and the IFrame Player API, nothing that edits the player's page.

The spec keeps to all seven. Rule 1 decides the scroll behaviour below; rule 2 decides the preview's size.

## Behaviour

### On the title page

1. The page opens with its banner, as today.
2. When the page has settled (the T1 or M3 transition has ended and 0.5 s has passed) and the engine allows autoplay (see When trailers play), the app creates a hidden web view the size of the banner, behind the banner image, and loads the engine's embed page into it.
3. The page reports `playing` once the player's state is 1 (playing) and a frame has been presented. Only then does the banner image fade out over the video (fade-in on the video, which is the same as the banner fading away). Until then the viewer sees the banner and nothing else.
4. While it plays, nothing is drawn over the trailer's rectangle. The banner's scrim and any title text over the banner fade out with the banner image (fade-out), and the title sits below the video, where it already sits on Mac and in the iPhone layout once the page scrolls.
5. Below the banner, at the trailing end of the row that holds the list and trailer buttons, a plain icon button toggles sound (`speaker.slash`, then `speaker.wave.2`). Its label is "Trailer sound". Sound is off at every start and the choice lasts for this trailer only.
6. When the trailer ends, it fades out (fade-out) and the banner returns. It does not loop.
7. Scrolling the page by more than 8 pt pauses the trailer and fades the banner back over it (fade-out on the video), because the page would otherwise slide over a playing player. Scrolling back to the top within 30 s resumes it with the same fade-in. After 30 s scrolled away the web view is released, and the trailer does not restart on this visit.
8. Leaving the page, starting playback, opening a sheet over the page, the window losing key status on Mac, or the app going to the background stops the trailer and releases the web view at once.
9. If the trailer has not reported `playing` within 8 s, or reports an error, the web view is released and the banner stays. Nothing tells the viewer: the trailer is an extra, and its absence is not an error.

Play trailer, from the menu or the title's context menu, stays the way to watch a trailer properly. It opens the trailer in a sheet (a window on Mac, Windows and Linux) holding the embed with YouTube's own controls and sound on. If the embed refuses the video (embedding disabled by the owner, region-locked), the sheet closes and the watch URL opens in the browser, which is today's behaviour.

### Hover previews on Mac, Windows and Linux

1. The pointer rests on a cover in Seasonal, Search results, Library, Calendar, related titles or recommendations. When it has moved less than 4 pt for 800 ms, a preview opens anchored to the cover: an `NSPopover` on Mac, a `Flyout` on Windows, a `GtkPopover` on Linux.
2. The preview holds the banner at 384 × 216 pt (at least 356 × 200 px on any display), then the title, one facts line ("TV · 12 episodes · Fall 2026") and two lines of synopsis with spoilers removed. Its one amber action is the title's main action ("Play episode 1", "Resume episode 7"). There is no second card, frame or glow around the video.
3. The trailer behaves as on the title page: banner first, fade-in only when playing, muted, no loop, the sound button below the video.
4. The preview closes 150 ms after the pointer leaves both the cover and the preview, and at once on a scroll, a key press, a click elsewhere or the window losing key status. Closing releases its web view.
5. One preview at a time. Opening one closes any other, and a title page with its own trailer playing gets no previews over it.
6. A title with no trailer, or where trailers may not play, gets the same preview without the video. The preview is useful for the synopsis alone, and a preview that appears for some covers and not others would read as broken.
7. Keyboard focus never opens a preview. Space on a focused cover does what it does today.

### When trailers play

The engine decides, from what the device says, and tells the front ends one answer. A trailer starts automatically only when all of these hold:

- the Trailers setting allows it on this network: Off never, Wi-Fi only when the path is neither cellular nor constrained, Always unless the path is constrained (Low Data Mode or Data Saver);
- the device is online;
- the system's Reduce Motion is off, and on iPhone and iPad the system's Auto-Play Video Previews is on;
- Low Power Mode or battery saver is off, and the thermal state is below serious;
- no screen reader runs (VoiceOver, TalkBack, Narrator, Orca), because a moving picture with no description only adds noise, and the web view's own accessibility tree would compete with ours;
- the device is not a low-memory one: Android's `isLowRamDevice()` is false and the device has at least 4 GB;
- the title is not a sequel of one the viewer has on their list and has not finished, while Spoiler protection is on. Later seasons' trailers show earlier seasons' endings.

Play trailer works whatever the answer, because the viewer asked for it.

## States

| State | Title page | Hover preview |
| --- | --- | --- |
| Loading | The banner is the loading state. No spinner, no placeholder, no activity line: the trailer is decoration, and a wait for it is never shown. If the banner itself has not arrived, the Raised plate stands in as for any art | The preview opens after the dwell with its banner, or the Raised plate until the banner arrives (fade-in) |
| Empty | No trailer on record: the banner stays and nothing says so. Play trailer is disabled, as today | The preview without the video |
| Failure | Silent, back to the banner (step 9). A failure of Play trailer, which the viewer asked for, shows "Couldn't open the trailer" with Try again, as native-ux's copy table has it | Silent |
| Offline | No web view is created | No web view is created |
| Not allowed | The banner stays; nothing says why. The setting's footnote explains when trailers play | The preview without the video |

## Motion

| Moment | Token |
| --- | --- |
| Banner fades away as the first frame plays | fade-in (0.2 s) |
| Banner returns at the end, on a scroll or on a stop | fade-out (0.3 s) |
| Hover preview opens and closes | system (the popover's or flyout's own) |
| Sound button's symbol change | quick |

Nothing scales, slides or glows. With Reduce Motion on, trailers do not start automatically at all, so there is no substitution to make. The hover preview still opens, with the system's own reduced-motion popover.

## The embed page

One HTML page for every platform, written by the engine (`anistream_model::trailer::embed_page`) so no front end writes its own. It:

- loads the IFrame Player API from `https://www.youtube.com/iframe_api` and creates a player on `youtube-nocookie.com` (privacy-enhanced mode) with `autoplay=1`, `mute=1`, `controls=0` for the banner and preview or `controls=1` for Play trailer, `playsinline=1`, `rel=0`, `fs=0`, `disablekb=1`, `iv_load_policy=3`, `origin` set to the app's identifier URL, and `enablejsapi=1`;
- sets `<meta name="referrer" content="strict-origin-when-cross-origin">`, YouTube's recommended policy;
- reports `ready`, `playing`, `ended`, `paused` and `error { code }` to the app through whichever bridge the platform gives it, `window.webkit.messageHandlers.anistream`, `window.chrome.webview` or an Android `WebMessageListener` named `anistream`;
- takes `play`, `pause`, `mute` and `unmute` from the app (`evaluateJavaScript` and its equivalents), which call the player API;
- sizes the player to fill the view and crops nothing.

### What each web view needs

| Platform | Web view | Referer | Autoplay and inline | Storage | Bridge |
| --- | --- | --- | --- | --- | --- |
| Mac, iPhone, iPad | `WKWebView` | `loadHTMLString(_:baseURL:)` with `https://tv.anistream.app/`; the iframe's requests then carry that origin | `allowsInlineMediaPlayback = true`, `mediaTypesRequiringUserActionForPlayback = []`; `allowsPictureInPictureMediaPlayback` and `allowsAirPlayForMediaPlayback` off | `WKWebsiteDataStore.nonPersistent()` | `WKScriptMessageHandler` |
| Android | System WebView | `loadDataWithBaseURL("https://tv.anistream.app/", …)` | `settings.mediaPlaybackRequiresUserGesture = false`, JavaScript on | Third-party cookies off (`CookieManager.setAcceptThirdPartyCookies(view, false)`), data cleared when the view is released | `WebViewCompat.addWebMessageListener` limited to the base URL's origin |
| Windows | WebView2 | `SetVirtualHostNameToFolderMapping` serving the page from the app's folder at `https://<package Name>/trailer.html`, so the origin is the app's identifier | Muted autoplay is allowed by Chromium's policy; nothing to set | `CoreWebView2ControllerOptions.IsInPrivateModeEnabled = true` | `chrome.webview.postMessage` and `WebMessageReceived` |
| Linux | WebKitGTK 6 (`webkit6` crate) | `WebView::load_html(page, Some("https://tv.anistream.desktop/"))` | `WebsitePolicies` autoplay `Allow`; the default `AllowWithoutSound` is enough while muted | `NetworkSession::new_ephemeral()` | `UserContentManager::register_script_message_handler("anistream")` |

Every web view is transparent until `playing` (`isOpaque = false` on Apple, a transparent background elsewhere), ignores pointer and touch input, has no context menu and no text selection, opens no new windows, and is released rather than kept in a pool. A released view frees its decoder, which matters on phones.

On Linux, YouTube's player needs Media Source Extensions with H.264 or VP9 through GStreamer. The Flatpak's GNOME runtime has the plugins; a distribution package must depend on GStreamer's libav plugin, or YouTube serves nothing playable and the trailer fails silently.

## Platforms

| Platform | Title page | Hover preview | Notes |
| --- | --- | --- | --- |
| Mac | Yes | Yes | The banner on the full title page (M3) and in the Continue and Library preview column, which counts as a title page with one player at a time |
| iPhone | Yes | No | Touch has no hover. The banner scrolls at half speed behind the page (T1), so rule 7 of the behaviour (pause on scroll) matters most here. In landscape the banner area is short; below 200 px tall the trailer does not start |
| iPad | Yes | No | Pointer hover exists on iPad, but the layouts are touch-first and a popover over a grid on hover is not an iPad pattern. The split view's preview column behaves as on Mac |
| Android | Yes | No | Same rules as iPhone. On tablets the list-detail pane's banner behaves as on Mac |
| Android TV | No autoplay | No | The TV brief forbids autoplaying video and trailers in browse, and a WebView on a 2 GB box costs memory the player needs. Play trailer opens a full-screen WebView with YouTube's controls; Center plays and pauses through the player API and Back closes |
| Windows | Yes | Yes | WebView2 is part of Windows 11. A missing runtime (an old Windows 10 install) means no trailers, silently, and Play trailer opens the browser |
| Linux | Yes | Yes | Adds WebKitGTK, the app's first web view. See Decided |
| Terminal | No | No | No web view. Play trailer keeps opening mpv or the browser |

## Engine and model changes

The engine owns the decision and the page; front ends own a web view and its bridge.

```rust
// anistream-meta: trailer { id site } joins MEDIA_FIELDS, so list rows carry it. AniList
// counts requests, not fields, so the cost is the bytes alone.
pub const MEDIA_FIELDS: &str; // + "trailer { id site }"

// anistream-model::catalog
pub struct Trailer {
    pub host: TrailerHost,
    pub id: String,
}

pub enum TrailerHost {
    YouTube,
    Dailymotion,
}

impl Trailer {
    /// Where Play trailer goes when the embed refuses: today's watch URL.
    pub fn watch_url(&self) -> String;
    /// Whether the in-app player can play it. YouTube only, as decided below.
    pub fn embeddable(&self) -> bool;
}

pub struct Entry {
    // … as now, plus:
    pub trailer: Option<Trailer>,
    // trailer_url stays until every front end reads `trailer`.
}

// anistream-model::trailer
/// The page each front end loads into its web view.
pub fn embed_page(trailer: &Trailer, style: EmbedStyle, app: &AppIdentity) -> Option<String>;

pub enum EmbedStyle {
    /// Muted, no controls: the banner and the hover preview.
    Banner,
    /// Sound and YouTube's controls: Play trailer.
    Full,
}

pub struct AppIdentity {
    /// `https://tv.anistream.app`, `https://tv.anistream.desktop`, …: the Referer and `origin`.
    pub origin: String,
}

/// What the embed page says, as the front end hands it back.
pub enum TrailerMessage {
    Ready,
    Playing,
    Paused,
    Ended,
    Error { code: u32 },
}

// anistream-model::command
/// What the device says about itself, once at launch and on every change: the inputs to
/// autoplay besides the network, which `NetworkPath` already brings.
Command::DeviceState {
    reduce_motion: bool,
    autoplay_previews: bool,
    low_power: bool,
    thermal_serious: bool,
    screen_reader: bool,
    low_memory: bool,
}

// anistream-model::event
/// Whether trailers may start by themselves now. Said on every change of the setting, the
/// network or the device state, and once at launch.
Event::Autoplay(Autoplay)

pub struct Autoplay {
    pub trailers: bool,
    /// Why not, for the Messages log and the tests. Never shown on screen.
    pub held_by: Option<AutoplayHold>,
}

pub enum AutoplayHold {
    Setting,
    Network,
    Offline,
    ReduceMotion,
    PowerSaving,
    ScreenReader,
    LowMemory,
}

/// Per title, on the detail and on list rows: false where the spoiler rule holds it back.
pub fn trailer_may_autoplay(entry: &Entry, library: &LibraryView, settings: &SettingsView) -> bool;
```

`Command::DeviceState` is shared with the other specs here: Picture in Picture and the gamepad use `screen_reader` and `low_power` too. The player's `PlayerReport::Power` stays as it is.

## Settings

| Row | Values | Default | Where |
| --- | --- | --- | --- |
| Play trailers | Always, Wi-Fi only, Off | Always | Settings > General, under the titles row |

Footnote: "Trailers play muted from YouTube, which sees your IP address. They don't play in Low Data Mode, with Reduce Motion on or in Low Power Mode." On Android the footnote names Data Saver and Battery Saver instead.

The terminal shows the row too, since the setting lives in the engine's config, but it changes nothing there.

## Privacy and legal

- The trailer is the one AniList lists. AniList's trailers are official promotional videos uploaded by their owners, and the app adds no search or fallback that could find anything else. Copy says "trailer", never "preview video" or "clip".
- YouTube receives the viewer's IP address, the app's identifier as Referer and the video id. Privacy-enhanced mode (`youtube-nocookie.com`) and storage that is never written to disk keep it from building a profile across sessions. No YouTube sign-in is possible in these views.
- Nothing is sent to YouTube for a title the viewer does not open or hover.
- The seven embed rules above are part of the acceptance checks: a screenshot of every platform's title page with a trailer playing shows nothing over the video.
- anistream hosts no content. Trailers stream from YouTube to the viewer directly.

## Size

About two weeks in all.

- Engine and model: two days. The field move, `Trailer`, the embed page, `DeviceState`, `Autoplay` and the spoiler rule, with tests.
- Apple: three days for the banner, the Mac preview popover and the sheet.
- Android: two days. TV: one day for the full-screen trailer.
- Windows: two days. Linux: three days, most of it the WebKitGTK dependency and packaging.
- Then the polish pass on every platform, with screen recordings of the fade.

## Decided

Settled on 24 October 2026. Where Johan answered, it says so; every other question took the recommended default.

1. Dailymotion trailers: YouTube only, the default chosen. Dailymotion trailers keep the browser link. YouTube covers nearly every title, and a second player means a second set of terms to keep to.
2. Hover preview without a trailer: the preview opens for every title, the default chosen, with the video only where one plays, so the pointer behaves the same on every cover.
3. The end of a trailer: fade back to the banner and stay, the default chosen. No loop.
4. The default of Play trailers: Always, Johan's choice, on any connection. The setting keeps Wi-Fi only and Off. Low Data Mode (Data Saver on Android), Reduce Motion, Low Power Mode, a screen reader and the other checks under "When trailers play" still hold a trailer back whatever the setting.
5. WebKitGTK on Linux: added for trailers, the default chosen, loaded only when a trailer is about to play.
6. Sequel trailers: held while Spoiler protection is on and an earlier season is unfinished on the viewer's list, the default chosen.

## Sources

- YouTube API Services, Required Minimum Functionality: <https://developers.google.com/youtube/terms/required-minimum-functionality>
- YouTube IFrame Player API and player parameters: <https://developers.google.com/youtube/iframe_api_reference>, <https://developers.google.com/youtube/player_parameters>
- Error 153 and the Referer: <https://simonwillison.net/2025/Dec/1/youtube-embed-153-error/>
- AniList GraphQL `Media.trailer`: <https://docs.anilist.co/reference/object/mediatrailer>
- WKWebViewConfiguration: <https://developer.apple.com/documentation/webkit/wkwebviewconfiguration>
- Android WebView `loadDataWithBaseURL` and `WebViewCompat.addWebMessageListener`: <https://developer.android.com/reference/android/webkit/WebView>, <https://developer.android.com/reference/androidx/webkit/WebViewCompat>
- WebView2 virtual host mapping: <https://learn.microsoft.com/microsoft-edge/webview2/concepts/working-with-local-content>
- WebKitGTK 6 `WebView.load_html` and `NetworkSession`: <https://webkitgtk.org/reference/webkitgtk/stable/>
- `UIAccessibility.isVideoAutoplayEnabled`: <https://developer.apple.com/documentation/uikit/uiaccessibility/isvideoautoplayenabled>
- Hayase's hover preview, read for this spec: `interface/src/lib/components/ui/cards/preview.svelte`, `YoutubeIframe.svelte` and `HOVER_TIME` in `interface/src/lib/modules/navigate.ts`.
