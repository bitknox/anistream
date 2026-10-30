//! Trailers: whether one may start by itself, the page a front end's web view plays it in, and
//! what that page says back.
//!
//! The engine owns the decision and the page; a front end owns a web view and its bridge. It
//! loads [`embed_page`] with its [`AppIdentity`] as the base URL, sends the page
//! [`TrailerControl`] scripts, and hands each message the page posts to [`TrailerMessage::parse`].
//!
//! The page keeps to YouTube's embed rules: the IFrame Player API on `youtube-nocookie.com`, the
//! documented player parameters and nothing that edits the player, the app's identifier as the
//! `origin` (and, through the base URL, the Referer), and a banner player that takes no pointer
//! or touch input. What the page cannot hold to is the front end's: nothing drawn over the
//! player, a viewport of at least 200 × 200 px, autoplay only while more than half of it shows,
//! one autoplaying player on screen at a time, and the system's own web view.

use anistream_core::config::TrailerAutoplay;

use crate::catalog::{Entry, LibrarySegment, LinkedTitle, RelationKind, Trailer};

/// How the page plays the trailer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EmbedStyle {
    /// Muted, no controls, no input: the banner and the hover preview.
    Banner,
    /// Sound and YouTube's own controls: Play trailer's sheet or window.
    Full,
}

/// Who the app is to YouTube: the origin the page is loaded under, which the player is told as
/// its `origin` and the web view sends as the Referer. An embed without one fails (error 153).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AppIdentity {
    /// `https://tv.anistream.app` on Apple platforms and Android, `https://<package Name>` on
    /// Windows, `https://tv.anistream.desktop` on Linux. No path and no trailing slash.
    pub origin: String,
}

impl AppIdentity {
    /// The Mac, iPhone, iPad and Android apps: their bundle and package identifier.
    pub const APPLE_AND_ANDROID: &str = "https://tv.anistream.app";
    /// The Linux app: its application id.
    pub const LINUX: &str = "https://tv.anistream.desktop";

    pub fn new(origin: impl Into<String>) -> Self {
        Self { origin: origin.into() }
    }

    /// What the web view loads the page under (`loadHTMLString(_:baseURL:)`,
    /// `loadDataWithBaseURL`, `load_html`): the origin with a trailing slash.
    pub fn base_url(&self) -> String {
        format!("{}/", self.origin)
    }

    /// An `https` origin with a host and nothing else, so it can go into the page as it is.
    fn valid(&self) -> bool {
        let Some(host) = self.origin.strip_prefix("https://") else { return false };
        let (name, port) = host.split_once(':').unwrap_or((host, ""));
        !name.is_empty()
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            && port.bytes().all(|b| b.is_ascii_digit())
            && (port.is_empty() == !host.contains(':'))
    }
}

/// The page a front end loads into its web view to play `trailer`, or `None` for one the app's
/// player does not play ([`Trailer::embeddable`]) or an origin that is not one. One page for
/// every platform, so no front end writes its own.
///
/// It reports through whichever bridge the platform gives it: `window.webkit.messageHandlers
/// .anistream` on Apple platforms, `window.chrome.webview` in WebView2, and the object an Android
/// `WebMessageListener` named `anistream` puts on the page. The player fills the view and
/// letterboxes rather than crops.
pub fn embed_page(trailer: &Trailer, style: EmbedStyle, app: &AppIdentity) -> Option<String> {
    page(Some(trailer), style, app, "")
}

/// The page with captions in the viewer's subtitle language (`playback.subtitle_language`, as
/// [`caption_language`] reads it), and, for `None`, with no video at all: a player loaded ahead
/// of the first trailer, which [`load_script`] then plays. The banner's captions show while its
/// sound is on; Play trailer's always do. A language YouTube has no captions in shows none.
pub fn page(
    trailer: Option<&Trailer>,
    style: EmbedStyle,
    app: &AppIdentity,
    subtitle_language: &str,
) -> Option<String> {
    if trailer.is_some_and(|trailer| !trailer.embeddable()) || !app.valid() {
        return None;
    }
    let (muted, controls, input) = match style {
        EmbedStyle::Banner => (1, 0, "pointer-events:none;"),
        EmbedStyle::Full => (0, 1, ""),
    };
    // The id and the origin were checked above to hold no quote, backslash or angle bracket,
    // and the language is one of the table's, so each goes into the script as it is.
    Some(
        PAGE.replace("{INPUT}", input)
            .replace("{VIDEO}", trailer.map_or("", |trailer| &trailer.id))
            .replace("{ORIGIN}", &app.origin)
            .replace("{MUTE}", &muted.to_string())
            .replace("{CONTROLS}", &controls.to_string())
            .replace("{CAPTIONS}", caption_language(subtitle_language).unwrap_or_default()),
    )
}

/// The two-letter code YouTube names a caption track and its interface by, for the subtitle
/// language the viewer set: ISO 639-2 (`eng`, `ger` or `deu`), ISO 639-1 (`en`), or a tag with a
/// region (`pt-BR`). `None` for none set, `off`, or a language this table does not know, which
/// then shows no captions.
pub fn caption_language(subtitle_language: &str) -> Option<&'static str> {
    let primary = subtitle_language.trim().split(['-', '_']).next()?.to_ascii_lowercase();
    CAPTION_LANGUAGES
        .iter()
        .find(|(two, threes)| *two == primary || threes.contains(&primary.as_str()))
        .map(|(two, _)| *two)
}

/// ISO 639-1 codes and the ISO 639-2 codes (bibliographic and terminology) that name them.
const CAPTION_LANGUAGES: &[(&str, &[&str])] = &[
    ("ar", &["ara"]),
    ("bg", &["bul"]),
    ("ca", &["cat"]),
    ("cs", &["cze", "ces"]),
    ("da", &["dan"]),
    ("de", &["ger", "deu"]),
    ("el", &["gre", "ell"]),
    ("en", &["eng"]),
    ("es", &["spa"]),
    ("et", &["est"]),
    ("fa", &["per", "fas"]),
    ("fi", &["fin"]),
    ("fr", &["fre", "fra"]),
    ("he", &["heb"]),
    ("hi", &["hin"]),
    ("hr", &["hrv"]),
    ("hu", &["hun"]),
    ("id", &["ind"]),
    ("it", &["ita"]),
    ("ja", &["jpn"]),
    ("ko", &["kor"]),
    ("lt", &["lit"]),
    ("lv", &["lav"]),
    ("ms", &["may", "msa"]),
    ("nl", &["dut", "nld"]),
    ("no", &["nor", "nob", "nno"]),
    ("pl", &["pol"]),
    ("pt", &["por"]),
    ("ro", &["rum", "ron"]),
    ("ru", &["rus"]),
    ("sk", &["slo", "slk"]),
    ("sl", &["slv"]),
    ("sr", &["srp"]),
    ("sv", &["swe"]),
    ("th", &["tha"]),
    ("tl", &["tgl", "fil"]),
    ("tr", &["tur"]),
    ("uk", &["ukr"]),
    ("vi", &["vie"]),
    ("zh", &["chi", "zho"]),
];

/// The page, with its blanks. The player's parameters are YouTube's documented ones; the page
/// says `playing` only once the player's state is playing and its clock has moved, two frames
/// on, so the front end fades the banner away over a picture rather than over a black player.
const PAGE: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="referrer" content="strict-origin-when-cross-origin">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<style>
html, body { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: transparent; -webkit-user-select: none; user-select: none; -webkit-touch-callout: none; }
#player { position: absolute; top: 0; left: 0; width: 100%; height: 100%; border: 0; {INPUT} }
</style>
</head>
<body>
<div id="player"></div>
<script>
(function () {
  "use strict";
  function say(message) {
    try {
      if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.anistream) {
        window.webkit.messageHandlers.anistream.postMessage(message);
        return;
      }
    } catch (e) {}
    try {
      if (window.chrome && window.chrome.webview) {
        window.chrome.webview.postMessage(message);
        return;
      }
    } catch (e) {}
    try {
      if (window.anistream && window.anistream.postMessage) {
        window.anistream.postMessage(message);
      }
    } catch (e) {}
  }
  document.addEventListener("contextmenu", function (event) { event.preventDefault(); });

  var player = null;
  var showing = false;
  var watch = 0;
  var tail = 0;
  var told = false;
  var sound = false;
  var ramp = 0;
  var pending = "";
  // The viewer's caption language, or none; and whether captions show now.
  var captions = "{CAPTIONS}";
  var captioned = false;
  // YouTube shows its title, a pause button, More videos and its logo over the first seconds of
  // a video, even with no controls, and hides them about four seconds in. The banner's video
  // comes forward only after that; Play trailer has YouTube's controls anyway.
  var settle = {CONTROLS} ? 0 : 4500;
  var began = 0;
  function stopTail() {
    if (tail) { clearInterval(tail); tail = 0; }
  }
  // A second before the end, so a front end can fade its art back and let go before the
  // player's end screen.
  function watchTail() {
    stopTail();
    tail = setInterval(function () {
      if (!player || !player.getDuration || !player.getCurrentTime) { return; }
      var length = player.getDuration();
      if (length > 1 && length - player.getCurrentTime() <= 1 && !told) {
        told = true;
        say("ending");
      }
    }, 100);
  }
  function stopRamp() {
    if (ramp) { clearInterval(ramp); ramp = 0; }
  }
  // Sound on at a start: from silence to half volume over 300 ms, once the player plays.
  function rampUp() {
    stopRamp();
    if (!player || !player.setVolume || !player.unMute) { return; }
    var step = 0;
    player.setVolume(0);
    player.unMute();
    ramp = setInterval(function () {
      step += 1;
      player.setVolume(Math.round(50 * step / 6));
      if (step >= 6) { stopRamp(); }
    }, 50);
  }
  function tellCaptions(on) {
    if (on !== captioned) {
      captioned = on;
      say(on ? "captions:on" : "captions:off");
    }
  }
  // The banner's captions while its sound is on; Play trailer's always. A track in the language
  // itself first, YouTube's own speech recognition next, and one translated into it last.
  function applyCaptions() {
    if (!captions || !player || !player.getOption || !player.setOption) { return; }
    try {
      if ({MUTE} && !sound) {
        if (captioned && player.unloadModule) { player.unloadModule("captions"); }
        tellCaptions(false);
        return;
      }
      if (player.loadModule) { player.loadModule("captions"); }
      var tracks = player.getOption("captions", "tracklist") || [];
      var own = null;
      var spoken = null;
      for (var i = 0; i < tracks.length; i++) {
        var code = String(tracks[i].languageCode || "").split("-")[0];
        if (code !== captions) { continue; }
        if (tracks[i].kind === "asr") { spoken = spoken || tracks[i]; } else { own = own || tracks[i]; }
      }
      var track = own || spoken;
      if (track) {
        player.setOption("captions", "track", { languageCode: track.languageCode });
      } else if (tracks.length) {
        player.setOption("captions", "track", {
          languageCode: tracks[0].languageCode,
          translationLanguage: { languageCode: captions }
        });
      } else {
        return;
      }
      tellCaptions(true);
    } catch (e) {}
  }
  function frames(then) {
    requestAnimationFrame(function () { requestAnimationFrame(then); });
  }
  function stopWatching() {
    if (watch) { clearInterval(watch); watch = 0; }
  }
  function whenMoving() {
    stopWatching();
    var from = player.getCurrentTime ? player.getCurrentTime() : 0;
    if (!began) { began = Date.now(); }
    watch = setInterval(function () {
      if (!player || !player.getCurrentTime || player.getCurrentTime() <= from) { return; }
      if (Date.now() - began < settle) { return; }
      stopWatching();
      frames(function () {
        if (!showing) { showing = true; say("playing"); }
      });
    }, 50);
  }
  var vars = {
    autoplay: 1,
    mute: {MUTE},
    controls: {CONTROLS},
    playsinline: 1,
    rel: 0,
    fs: 0,
    disablekb: 1,
    iv_load_policy: 3,
    enablejsapi: 1,
    origin: "{ORIGIN}"
  };
  if (captions) {
    vars.hl = captions;
    vars.cc_lang_pref = captions;
    if (!{MUTE}) { vars.cc_load_policy = 1; }
  }
  window.onYouTubeIframeAPIReady = function () {
    player = new YT.Player("player", {
      host: "https://www.youtube-nocookie.com",
      videoId: "{VIDEO}",
      width: "100%",
      height: "100%",
      playerVars: vars,
      events: {
        onReady: function () {
          say("ready");
          applyCaptions();
          if (pending) {
            var id = pending;
            pending = "";
            window.trailer.load(id);
          }
        },
        onStateChange: function (event) {
          if (event.data === 1) {
            if (sound && player.isMuted && player.isMuted()) { rampUp(); }
            applyCaptions();
            whenMoving();
            watchTail();
          } else if (event.data === 2) {
            stopWatching();
            stopTail();
            showing = false;
            say("paused");
          } else if (event.data === 0) {
            stopWatching();
            stopTail();
            showing = false;
            began = 0;
            say("ended");
          }
        },
        // The captions module's track list arrives after the player is ready.
        onApiChange: function () { applyCaptions(); },
        onError: function (event) {
          stopWatching();
          stopTail();
          say("error:" + event.data);
        }
      }
    });
  };
  window.trailer = {
    play: function () { if (player && player.playVideo) { player.playVideo(); } },
    pause: function () { if (player && player.pauseVideo) { player.pauseVideo(); } },
    mute: function () {
      sound = false;
      stopRamp();
      if (player && player.mute) { player.mute(); }
      applyCaptions();
    },
    // While it plays, half volume at once; before, the start ramps it up.
    unmute: function () {
      sound = true;
      if (!player || !player.unMute || !player.getPlayerState) { return; }
      if (player.getPlayerState() === 1 && !ramp) {
        player.setVolume(50);
        player.unMute();
      }
      applyCaptions();
    },
    // Another trailer in the same player, muted at its start as the first was; the sound comes
    // on as it plays when it is on. Before the player is up, it waits for it.
    load: function (id) {
      stopWatching();
      stopTail();
      stopRamp();
      showing = false;
      told = false;
      began = 0;
      if (!player || !player.loadVideoById) { pending = id; return; }
      if ({MUTE}) { player.mute(); }
      player.loadVideoById(id);
    },
    remaining: function () {
      if (!player || !player.getDuration || !player.getCurrentTime) { return -1; }
      var left = player.getDuration() - player.getCurrentTime();
      return left >= 0 && left < 86400 ? left : -1;
    }
  };
  var api = document.createElement("script");
  api.src = "https://www.youtube.com/iframe_api";
  api.onerror = function () { say("error:0"); };
  document.head.appendChild(api);
})();
</script>
</body>
</html>
"#;

/// What the page posts a second before the trailer ends: the banner comes back now. It is
/// not a [`TrailerMessage`], so [`TrailerMessage::parse`] ignores it and a front end that does not
/// know it loses nothing; the `ended` that follows still comes.
pub const ENDING_MESSAGE: &str = "ending";

/// What the page posts when captions start showing over the video, and when they stop, for a
/// front end that keeps the foot of the video clear while they show. Not [`TrailerMessage`]s
/// either, for the same reason as [`ENDING_MESSAGE`].
pub const CAPTIONS_ON_MESSAGE: &str = "captions:on";
pub const CAPTIONS_OFF_MESSAGE: &str = "captions:off";

/// What the page says, as the front end hands it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TrailerMessage {
    /// The player is up. Nothing shows yet.
    Ready,
    /// A frame is on screen: the banner fades away over it now, and not before.
    Playing,
    /// Paused, by the front end or by the player.
    Paused,
    /// It played to the end: the banner comes back, and it does not loop.
    Ended,
    /// The player refused it, with YouTube's code: `2` a bad id, `5` an HTML5 error, `100` gone or
    /// private, `101` and `150` embedding disabled by its owner, and `150` too when YouTube asks
    /// the viewer to sign in to prove they are not a bot (a network it has flagged, such as a VPN
    /// exit); `152` and `153` the embed's identity refused or missing. `0` is the page's own: the
    /// player API did not load. The banner stays; Play trailer opens [`Trailer::watch_url`]
    /// instead.
    Error { code: u32 },
}

impl TrailerMessage {
    /// A message the page posted: `ready`, `playing`, `paused`, `ended` or `error:<code>`.
    /// `None` for anything else, which a front end ignores.
    pub fn parse(message: &str) -> Option<Self> {
        Some(match message.trim() {
            "ready" => Self::Ready,
            "playing" => Self::Playing,
            "paused" => Self::Paused,
            "ended" => Self::Ended,
            other => Self::Error { code: other.strip_prefix("error:")?.parse().ok()? },
        })
    }
}

/// What the front end tells the page, through `evaluateJavaScript` and its equivalents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TrailerControl {
    Play,
    Pause,
    Mute,
    Unmute,
}

impl TrailerControl {
    /// The script that does it, through the player API.
    pub const fn script(self) -> &'static str {
        match self {
            Self::Play => "window.trailer && window.trailer.play();",
            Self::Pause => "window.trailer && window.trailer.pause();",
            Self::Mute => "window.trailer && window.trailer.mute();",
            Self::Unmute => "window.trailer && window.trailer.unmute();",
        }
    }
}

/// The script that plays `trailer` in a page that is already up, in place of the one it was
/// built for: a front end keeps one web view and its player, and switches trailers with this
/// rather than loading a new page. `None` for a trailer the app's player does not play.
pub fn load_script(trailer: &Trailer) -> Option<String> {
    // The id was checked to hold no quote, backslash or angle bracket.
    trailer
        .embeddable()
        .then(|| format!("window.trailer && window.trailer.load(\"{}\");", trailer.id))
}

/// What the device says about itself with
/// [`Command::DeviceState`](crate::command::Command::DeviceState), as the engine keeps it: the
/// inputs to autoplay besides the network, and what other features (Picture in Picture, the
/// gamepad) read of the device.
///
/// The default is a device that holds nothing back, which is what the engine assumes until the
/// front end says otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    /// The system's Reduce Motion (Remove animations on Android and Windows).
    pub reduce_motion: bool,
    /// The system lets videos preview by themselves: iOS and iPadOS's Auto-Play Video Previews.
    /// `true` where the system has no such setting.
    pub autoplay_previews: bool,
    /// Low Power Mode, Battery Saver, Energy Saver.
    pub low_power: bool,
    /// The thermal state is serious or critical.
    pub thermal_serious: bool,
    /// A screen reader runs: VoiceOver, TalkBack, Narrator, Orca.
    pub screen_reader: bool,
    /// A low-memory device: Android's `isLowRamDevice()`, or under 4 GB.
    pub low_memory: bool,
}

impl Default for Device {
    fn default() -> Self {
        Self {
            reduce_motion: false,
            autoplay_previews: true,
            low_power: false,
            thermal_serious: false,
            screen_reader: false,
            low_memory: false,
        }
    }
}

/// The network the device uses, as [`Command::NetworkPath`] last said it.
///
/// [`Command::NetworkPath`]: crate::command::Command::NetworkPath
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Network {
    pub cellular: bool,
    /// Low Data Mode, Data Saver, a metered connection.
    pub constrained: bool,
    pub offline: bool,
}

/// Whether trailers may start by themselves now, as
/// [`Event::Autoplay`](crate::event::Event::Autoplay) says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct AutoplayView {
    pub trailers: bool,
    /// Why not, for the log and the tests. Never shown on screen: the setting's footnote says
    /// when trailers play.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub held_by: Option<AutoplayHold>,
}

/// What holds trailers back. When several do, the first in this order is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AutoplayHold {
    /// Play trailers is off.
    Setting,
    /// No network at all.
    Offline,
    /// The network: constrained, or cellular while the setting is Wi-Fi only.
    Network,
    /// The system's Reduce Motion.
    ReduceMotion,
    /// The system's Auto-Play Video Previews is off.
    VideoPreviews,
    /// Low Power Mode or a battery saver.
    PowerSaving,
    /// The device is running hot.
    Thermal,
    /// A screen reader runs.
    ScreenReader,
    /// A low-memory device.
    LowMemory,
}

/// Whether trailers may start by themselves under `setting`, on `network`, on `device`. Play
/// trailer plays whatever this says, because the viewer asked for it.
pub fn autoplay(setting: TrailerAutoplay, network: Network, device: Device) -> AutoplayView {
    use AutoplayHold as H;
    let network_holds = match setting {
        TrailerAutoplay::Always => network.constrained,
        TrailerAutoplay::WifiOnly => network.cellular || network.constrained,
        TrailerAutoplay::Off => false,
    };
    let held_by = [
        (setting == TrailerAutoplay::Off, H::Setting),
        (network.offline, H::Offline),
        (network_holds, H::Network),
        (device.reduce_motion, H::ReduceMotion),
        (!device.autoplay_previews, H::VideoPreviews),
        (device.low_power, H::PowerSaving),
        (device.thermal_serious, H::Thermal),
        (device.screen_reader, H::ScreenReader),
        (device.low_memory, H::LowMemory),
    ]
    .into_iter()
    .find_map(|(holds, hold)| holds.then_some(hold));
    AutoplayView { trailers: held_by.is_none(), held_by }
}

/// Whether this title's trailer may start by itself, as far as the title goes: it has one the
/// app's player plays, and, while Spoiler protection is on, it is not a later part of a story
/// the viewer has on their list and has not finished, whose trailer would show how the earlier
/// part ends. [`autoplay`] says whether any trailer may start now; both must hold.
///
/// Decided from what the entry carries: the title page's relations and seasons, each with where
/// it is on the viewer's list, or, on a list row that has no page's detail, its prequels.
pub fn trailer_may_autoplay(entry: &Entry, spoiler_protection: bool) -> bool {
    entry.trailer.as_ref().is_some_and(Trailer::embeddable)
        && !(spoiler_protection && follows_unfinished(entry))
}

/// Whether an earlier part of the title's story is on the viewer's list, unfinished: a prequel or
/// the parent story, or a season before it in the franchise's watch order.
fn follows_unfinished(entry: &Entry) -> bool {
    let unfinished = |title: &LinkedTitle| is_unfinished(title.list_status);
    let Some(detail) = &entry.detail else {
        return entry.prequels.iter().any(|prequel| is_unfinished(prequel.list_status));
    };
    let earlier_related = detail.relations.iter().any(|title| {
        matches!(title.relation, Some(RelationKind::Prequel | RelationKind::Parent))
            && unfinished(title)
    });
    let seasons = detail.seasons.as_deref().unwrap_or_default();
    let earlier_season = seasons.iter().any(|season| season.id == entry.id)
        && seasons.iter().take_while(|season| season.id != entry.id).any(unfinished);
    earlier_related || earlier_season
}

fn is_unfinished(status: Option<LibrarySegment>) -> bool {
    matches!(
        status,
        Some(
            LibrarySegment::Watching
                | LibrarySegment::Planning
                | LibrarySegment::Paused
                | LibrarySegment::Dropped
        )
    )
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;

    use super::*;
    use crate::catalog::{EarlierTitle, TitleDetail, TrailerHost};

    fn youtube(id: &str) -> Trailer {
        Trailer { host: TrailerHost::YouTube, id: id.into() }
    }

    fn app() -> AppIdentity {
        AppIdentity::new(AppIdentity::APPLE_AND_ANDROID)
    }

    #[test]
    fn a_trailer_is_read_from_anilists_site_and_id() {
        let trailer = Trailer::from_site("youtube", "abc_12-3").unwrap();
        assert_eq!(trailer.watch_url(), "https://www.youtube.com/watch?v=abc_12-3");
        assert!(trailer.embeddable());

        let other = Trailer::from_site("dailymotion", "x7abc").unwrap();
        assert_eq!(other.watch_url(), "https://www.dailymotion.com/video/x7abc");
        assert!(!other.embeddable(), "YouTube only plays in the app");

        assert_eq!(Trailer::from_site("video.example", "abc"), None);
        assert_eq!(Trailer::from_site("youtube", ""), None);
        assert!(!youtube("abc\"onerror").embeddable(), "an id YouTube never gives");
    }

    #[test]
    fn the_page_keeps_to_youtubes_embed_rules() {
        let page = embed_page(&youtube("abc123"), EmbedStyle::Banner, &app()).unwrap();
        for required in [
            r#"<meta name="referrer" content="strict-origin-when-cross-origin">"#,
            r#"api.src = "https://www.youtube.com/iframe_api""#,
            r#"host: "https://www.youtube-nocookie.com""#,
            r#"videoId: "abc123""#,
            "autoplay: 1,",
            "mute: 1,",
            "controls: 0,",
            "playsinline: 1,",
            "rel: 0,",
            "fs: 0,",
            "disablekb: 1,",
            "iv_load_policy: 3,",
            "enablejsapi: 1,",
            r#"origin: "https://tv.anistream.app""#,
            "pointer-events:none;",
            "window.webkit.messageHandlers.anistream.postMessage(message)",
            "window.chrome.webview.postMessage(message)",
            "window.anistream.postMessage(message)",
            // The seconds left, for the banner that returns half a second before the end.
            "remaining: function ()",
        ] {
            assert!(page.contains(required), "{required}");
        }
        for blank in ["{INPUT}", "{VIDEO}", "{ORIGIN}", "{MUTE}", "{CONTROLS}"] {
            assert!(!page.contains(blank), "{blank}");
        }
        // Nothing of the player's own page is touched: no style or script reaches into it.
        assert!(!page.contains("contentDocument") && !page.contains("contentWindow"));
    }

    #[test]
    fn play_trailer_has_sound_controls_and_input() {
        let page = embed_page(&youtube("abc123"), EmbedStyle::Full, &app()).unwrap();
        assert!(page.contains("mute: 0,") && page.contains("controls: 1,"));
        assert!(!page.contains("pointer-events:none;"));
    }

    #[test]
    fn no_page_for_a_trailer_the_app_does_not_play_or_an_origin_that_is_not_one() {
        let dailymotion = Trailer { host: TrailerHost::Dailymotion, id: "x7abc".into() };
        assert_eq!(embed_page(&dailymotion, EmbedStyle::Banner, &app()), None);
        for origin in
            ["http://tv.anistream.app", "https://", "https://a\"b", "tv.anistream.app"]
        {
            let page =
                embed_page(&youtube("abc"), EmbedStyle::Banner, &AppIdentity::new(origin));
            assert_eq!(page, None, "{origin}");
        }
        for origin in
            [AppIdentity::LINUX, "https://Publisher.Anistream", "https://localhost:8080"]
        {
            let page =
                embed_page(&youtube("abc"), EmbedStyle::Banner, &AppIdentity::new(origin));
            assert!(page.is_some(), "{origin}");
        }
        assert_eq!(app().base_url(), "https://tv.anistream.app/");
    }

    #[test]
    fn what_the_page_says_is_read_back() {
        assert_eq!(TrailerMessage::parse("ready"), Some(TrailerMessage::Ready));
        assert_eq!(TrailerMessage::parse("playing"), Some(TrailerMessage::Playing));
        assert_eq!(TrailerMessage::parse("paused"), Some(TrailerMessage::Paused));
        assert_eq!(TrailerMessage::parse("ended\n"), Some(TrailerMessage::Ended));
        assert_eq!(
            TrailerMessage::parse(ENDING_MESSAGE),
            None,
            "a front end that has no use for it"
        );
        let page = embed_page(&youtube("abc"), EmbedStyle::Banner, &app()).unwrap();
        assert!(page.contains(&format!("say(\"{ENDING_MESSAGE}\")")), "the page says it");
        assert_eq!(
            TrailerMessage::parse("error:150"),
            Some(TrailerMessage::Error { code: 150 })
        );
        assert_eq!(TrailerMessage::parse("error:0"), Some(TrailerMessage::Error { code: 0 }));
        assert_eq!(TrailerMessage::parse("error:"), None);
        assert_eq!(TrailerMessage::parse("buffering"), None);
        // Each control calls what the page defines.
        let page = embed_page(&youtube("abc"), EmbedStyle::Banner, &app()).unwrap();
        for (control, name) in [
            (TrailerControl::Play, "play"),
            (TrailerControl::Pause, "pause"),
            (TrailerControl::Mute, "mute"),
            (TrailerControl::Unmute, "unmute"),
        ] {
            assert!(control.script().contains(&format!("window.trailer.{name}()")));
            assert!(page.contains(&format!("    {name}: function ()")), "{name}");
        }
    }

    #[test]
    fn a_page_that_is_up_switches_to_another_trailer() {
        assert_eq!(
            load_script(&youtube("def_45-6")).as_deref(),
            Some(r#"window.trailer && window.trailer.load("def_45-6");"#)
        );
        assert_eq!(load_script(&youtube("abc\"onerror")), None, "an id YouTube never gives");
        let dailymotion = Trailer { host: TrailerHost::Dailymotion, id: "x7abc".into() };
        assert_eq!(load_script(&dailymotion), None);

        let banner = embed_page(&youtube("abc"), EmbedStyle::Banner, &app()).unwrap();
        assert!(banner.contains("    load: function (id) {"));
        assert!(banner.contains("player.loadVideoById(id);"), "the player the page has");
        assert!(banner.contains("if (1) { player.mute(); }"), "a banner starts muted again");
        assert!(banner.contains("pending = id;"), "a load before the player is up waits");
        let full = embed_page(&youtube("abc"), EmbedStyle::Full, &app()).unwrap();
        assert!(full.contains("if (0) { player.mute(); }"), "Play trailer keeps its sound");
    }

    #[test]
    fn captions_follow_the_subtitle_language_as_youtube_names_it() {
        for (setting, code) in [
            ("eng", "en"),
            ("en", "en"),
            ("jpn", "ja"),
            ("ger", "de"),
            ("deu", "de"),
            ("fre", "fr"),
            ("chi", "zh"),
            ("zho", "zh"),
            ("pt-BR", "pt"),
            ("  POR ", "pt"),
            ("fil", "tl"),
            ("nob", "no"),
        ] {
            assert_eq!(caption_language(setting), Some(code), "{setting}");
        }
        for none in ["", "off", "none", "xx", "qaa", "klingon", "e"] {
            assert_eq!(caption_language(none), None, "{none}");
        }
    }

    #[test]
    fn the_page_asks_for_captions_in_the_viewers_language_and_none_without_one() {
        let banner =
            super::page(Some(&youtube("abc")), EmbedStyle::Banner, &app(), "jpn").unwrap();
        assert!(banner.contains(r#"var captions = "ja";"#));
        assert!(
            banner.contains("vars.hl = captions;")
                && banner.contains("vars.cc_lang_pref = captions;")
        );
        // The banner's show only with its sound; Play trailer's from the start.
        assert!(banner.contains("if (!1) { vars.cc_load_policy = 1; }"));
        assert!(banner.contains("if (1 && !sound) {"));
        let full = super::page(Some(&youtube("abc")), EmbedStyle::Full, &app(), "eng").unwrap();
        assert!(full.contains(r#"var captions = "en";"#));
        assert!(full.contains("if (!0) { vars.cc_load_policy = 1; }"));
        // The track: the language's own, then speech recognition, then a translation.
        assert!(full.contains(
            r#"player.setOption("captions", "track", { languageCode: track.languageCode });"#
        ));
        assert!(full.contains("translationLanguage: { languageCode: captions }"));
        assert!(full.contains(&format!(
            "say(on ? \"{CAPTIONS_ON_MESSAGE}\" : \"{CAPTIONS_OFF_MESSAGE}\")"
        )));

        for unknown in ["", "qaa"] {
            let html =
                super::page(Some(&youtube("abc")), EmbedStyle::Full, &app(), unknown).unwrap();
            assert!(html.contains(r#"var captions = "";"#), "{unknown}");
            assert!(html.contains("if (!captions || !player"), "nothing is asked for");
        }
        assert_eq!(
            embed_page(&youtube("abc"), EmbedStyle::Full, &app()),
            super::page(Some(&youtube("abc")), EmbedStyle::Full, &app(), ""),
            "the page without a language has no captions"
        );
    }

    #[test]
    fn a_page_with_no_video_is_a_player_waiting_for_one() {
        let waiting = super::page(None, EmbedStyle::Banner, &app(), "eng").unwrap();
        assert!(waiting.contains(r#"videoId: "","#));
        assert!(waiting.contains("    load: function (id) {"));
        assert_eq!(
            super::page(None, EmbedStyle::Banner, &AppIdentity::new("tv.anistream.app"), ""),
            None
        );
        let dailymotion = Trailer { host: TrailerHost::Dailymotion, id: "x7abc".into() };
        assert_eq!(super::page(Some(&dailymotion), EmbedStyle::Banner, &app(), ""), None);
    }

    #[test]
    fn the_page_says_ending_a_second_before_the_end() {
        let page = embed_page(&youtube("abc"), EmbedStyle::Banner, &app()).unwrap();
        assert!(page.contains("length - player.getCurrentTime() <= 1 && !told"));
    }

    #[test]
    fn sound_on_ramps_to_half_volume_as_the_trailer_starts() {
        let page = embed_page(&youtube("abc"), EmbedStyle::Banner, &app()).unwrap();
        // Unmuted before it plays, it waits; at the start it ramps from silence.
        assert!(
            page.contains("if (sound && player.isMuted && player.isMuted()) { rampUp(); }")
        );
        assert!(page.contains("player.setVolume(0);\n    player.unMute();"));
        assert!(page.contains("player.setVolume(Math.round(50 * step / 6));"));
        assert!(page.contains("}, 50);"), "six steps of 50 ms, 300 ms in all");
        // While it plays, half volume and then the sound.
        assert!(page.contains("player.setVolume(50);\n        player.unMute();"));
        assert!(page.contains("sound = false;\n      stopRamp();"), "muting stops a ramp");
    }

    #[test]
    fn trailers_play_always_by_default_and_wifi_only_keeps_off_cellular() {
        use TrailerAutoplay::*;
        let device = Device::default();
        let on = |setting, network| autoplay(setting, network, device);
        let wifi = Network::default();
        let cellular = Network { cellular: true, ..wifi };
        let constrained = Network { constrained: true, ..wifi };
        let offline = Network { offline: true, ..wifi };

        assert_eq!(on(Always, wifi), AutoplayView { trailers: true, held_by: None });
        assert!(on(Always, cellular).trailers, "Always is any network");
        assert_eq!(on(Always, constrained).held_by, Some(AutoplayHold::Network));
        assert!(on(WifiOnly, wifi).trailers);
        assert_eq!(on(WifiOnly, cellular).held_by, Some(AutoplayHold::Network));
        assert_eq!(on(WifiOnly, constrained).held_by, Some(AutoplayHold::Network));
        assert_eq!(on(Off, wifi).held_by, Some(AutoplayHold::Setting));
        assert_eq!(on(Off, offline).held_by, Some(AutoplayHold::Setting), "the first named");
        assert_eq!(on(Always, offline).held_by, Some(AutoplayHold::Offline));
    }

    #[test]
    fn the_device_holds_trailers_back_whatever_the_setting() {
        let held = |device| autoplay(TrailerAutoplay::Always, Network::default(), device);
        let base = Device::default();
        for (device, hold) in [
            (Device { reduce_motion: true, ..base }, AutoplayHold::ReduceMotion),
            (Device { autoplay_previews: false, ..base }, AutoplayHold::VideoPreviews),
            (Device { low_power: true, ..base }, AutoplayHold::PowerSaving),
            (Device { thermal_serious: true, ..base }, AutoplayHold::Thermal),
            (Device { screen_reader: true, ..base }, AutoplayHold::ScreenReader),
            (Device { low_memory: true, ..base }, AutoplayHold::LowMemory),
        ] {
            assert_eq!(held(device), AutoplayView { trailers: false, held_by: Some(hold) });
        }
    }

    fn linked(
        id: u32,
        relation: Option<RelationKind>,
        status: Option<LibrarySegment>,
    ) -> LinkedTitle {
        LinkedTitle {
            id: AnilistId::new(id),
            title: format!("Starlit Harbour {id}"),
            title_romaji: None,
            title_english: None,
            title_native: None,
            relation,
            format: Some("TV".into()),
            cover_url: None,
            cover_color: None,
            season: None,
            year: None,
            list_status: status,
        }
    }

    fn page(id: u32, relations: Vec<LinkedTitle>, seasons: Option<Vec<LinkedTitle>>) -> Entry {
        let mut entry = Entry::new(AnilistId::new(id), "Starlit Harbour");
        entry.trailer = Some(youtube("abc123"));
        entry.detail = Some(TitleDetail {
            tags: Vec::new(),
            schedule: Vec::new(),
            relations,
            seasons,
            recommendations: Vec::new(),
            rankings: Vec::new(),
            characters: Vec::new(),
            staff: Vec::new(),
            ..TitleDetail::default()
        });
        entry
    }

    #[test]
    fn a_sequel_of_an_unfinished_story_on_the_list_is_held_while_spoilers_are_protected() {
        use LibrarySegment::*;
        let prequel =
            |status| page(2, vec![linked(1, Some(RelationKind::Prequel), status)], None);
        for status in [Watching, Planning, Paused, Dropped] {
            assert!(!trailer_may_autoplay(&prequel(Some(status)), true), "{status:?}");
            assert!(trailer_may_autoplay(&prequel(Some(status)), false), "protection off");
        }
        assert!(trailer_may_autoplay(&prequel(Some(Completed)), true), "finished");
        assert!(trailer_may_autoplay(&prequel(None), true), "not on the list");

        let parent = page(2, vec![linked(1, Some(RelationKind::Parent), Some(Watching))], None);
        assert!(!trailer_may_autoplay(&parent, true), "a side story of an unfinished parent");
        let sequel = page(1, vec![linked(2, Some(RelationKind::Sequel), Some(Watching))], None);
        assert!(trailer_may_autoplay(&sequel, true), "what comes after spoils nothing here");
    }

    #[test]
    fn a_later_season_is_held_by_any_earlier_one_unfinished_however_far_back() {
        let seasons = |first| {
            Some(vec![
                linked(1, None, Some(first)),
                linked(2, None, Some(LibrarySegment::Completed)),
                linked(3, None, None),
            ])
        };
        let third = |first| page(3, Vec::new(), seasons(first));
        assert!(!trailer_may_autoplay(&third(LibrarySegment::Paused), true));
        assert!(trailer_may_autoplay(&third(LibrarySegment::Completed), true));
        // The first season itself has nothing before it.
        let first = page(1, Vec::new(), seasons(LibrarySegment::Watching));
        assert!(trailer_may_autoplay(&first, true));
    }

    #[test]
    fn only_a_trailer_the_app_plays_may_start() {
        let mut row = Entry::new(AnilistId::new(7), "Starlit Harbour");
        assert!(!trailer_may_autoplay(&row, false), "none on record");
        row.trailer = Some(Trailer { host: TrailerHost::Dailymotion, id: "x7abc".into() });
        assert!(!trailer_may_autoplay(&row, false), "a browser link only");
        row.trailer = Some(youtube("abc123"));
        assert!(trailer_may_autoplay(&row, true), "no prequel to hold it");
        let earlier = |status| EarlierTitle { id: AnilistId::new(6), list_status: status };
        row.prequels = vec![earlier(Some(LibrarySegment::Watching))];
        assert!(!trailer_may_autoplay(&row, true), "a list row's prequel, unfinished");
        assert!(trailer_may_autoplay(&row, false), "protection off");
        row.prequels = vec![earlier(Some(LibrarySegment::Completed)), earlier(None)];
        assert!(trailer_may_autoplay(&row, true), "its prequels watched or off the list");
    }
}
