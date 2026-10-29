using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>What the page's player has done so far, which decides what the banner and the web view do.</summary>
public enum TrailerPhase
{
    /// <summary>No web view.</summary>
    Idle,
    /// <summary>A web view is loading the page; the banner stays as it was.</summary>
    Loading,
    /// <summary>A frame is on screen and the video shows.</summary>
    Playing,
    /// <summary>The view is held, paused, with the banner back over it.</summary>
    Paused,
    /// <summary>It ended or was refused: not again on this visit.</summary>
    Done,
}

/// <summary>What the view does after the flow has moved.</summary>
[Flags]
public enum TrailerEffect
{
    None = 0,
    /// <summary>The video fades in over the banner.</summary>
    ShowVideo = 1,
    /// <summary>The video fades out and the banner is all there is.</summary>
    HideVideo = 2,
    /// <summary>Let go of the web view.</summary>
    Release = 4,
    PausePlayer = 8,
    PlayPlayer = 16,
}

/// <summary>
/// The life of one trailer in one place (a title page, a hover preview): the banner shows at once,
/// the video shows only once the page says a frame is playing, a trailer that has ended or was
/// refused does not come back on this visit. No WinUI in it, so the rules are tested.
/// </summary>
public sealed class TrailerFlow
{
    public TrailerPhase Phase { get; private set; }

    /// <summary>A web view is held: loading, playing or paused.</summary>
    public bool Active => Phase is TrailerPhase.Loading or TrailerPhase.Playing or TrailerPhase.Paused;

    public bool Showing => Phase == TrailerPhase.Playing;

    /// <summary>A trailer may start: nothing is held and it has not already ended or failed.</summary>
    public bool CanStart => Phase == TrailerPhase.Idle;

    public bool Start()
    {
        if (!CanStart)
            return false;
        Phase = TrailerPhase.Loading;
        return true;
    }

    /// <summary>The page said something. Only a frame playing shows the video; an end or an error lets go for good.</summary>
    public TrailerEffect Heard(TrailerMessage? message)
    {
        switch (message)
        {
            case TrailerMessage.Playing when Phase is TrailerPhase.Loading or TrailerPhase.Paused:
                Phase = TrailerPhase.Playing;
                return TrailerEffect.ShowVideo;
            case TrailerMessage.Ended or TrailerMessage.Error when Active:
                return Let(TrailerPhase.Done);
            default:
                return TrailerEffect.None;
        }
    }

    /// <summary>The view could not be made, or the runtime is missing.</summary>
    public TrailerEffect Failed() => Active ? Let(TrailerPhase.Done) : TrailerEffect.None;

    /// <summary>It has not played in time: let go, and try again on a later visit.</summary>
    public TrailerEffect GiveUp() => Phase == TrailerPhase.Loading ? Let(TrailerPhase.Idle) : TrailerEffect.None;

    /// <summary>Hold the trailer: it pauses and the banner comes back over it.</summary>
    public TrailerEffect Pause()
    {
        if (Phase != TrailerPhase.Playing)
            return TrailerEffect.None;
        Phase = TrailerPhase.Paused;
        return TrailerEffect.PausePlayer | TrailerEffect.HideVideo;
    }

    /// <summary>Play it again from where it paused; the video shows when the page says it is playing.</summary>
    public TrailerEffect Resume() => Phase == TrailerPhase.Paused ? TrailerEffect.PlayPlayer : TrailerEffect.None;

    /// <summary>Let go of the view, whatever it was doing.</summary>
    public TrailerEffect Stop() => Active ? Let(TrailerPhase.Idle) : TrailerEffect.None;

    TrailerEffect Let(TrailerPhase then)
    {
        var was = Phase;
        Phase = then;
        return TrailerEffect.Release | (was is TrailerPhase.Playing ? TrailerEffect.HideVideo : TrailerEffect.None);
    }
}

/// <summary>
/// Trailers as the Windows app has them: the engine's page and the identity it is loaded under,
/// whether a title's trailer may start by itself, and the words a preview shows.
/// </summary>
public static class Trailers
{
    /// <summary>
    /// Who the app is to YouTube: the origin the page is loaded under, which the player is told as
    /// its <c>origin</c> and the web view sends as the Referer. An embed without one fails (error 153).
    /// </summary>
    public const string Origin = "https://tv.anistream.windows";

    public static AppIdentity Identity { get; } = new(Origin);

    /// <summary>The page is served from here, at <c>/trailer.html</c>.</summary>
    public static string Host => new Uri(Origin).Host;

    /// <summary>The app's own player plays this title's trailer (YouTube only).</summary>
    public static bool Embeddable(Entry entry) =>
        entry.Trailer is { } trailer && AnistreamFfiMethods.TrailerEmbeddable(trailer);

    /// <summary>The page to load for the title's trailer, or null for one the app does not play.</summary>
    public static string? Page(Entry entry, EmbedStyle style)
    {
#if ANISTREAM_TESTING
        // The fixture scene's made-up trailer plays a gradient, with no network.
        if (entry.Trailer?.Id == FixtureId)
            return FixturePage;
        if (entry.Trailer?.Id == FixtureStuckId)
            return FixtureStuckPage;
#endif
        return entry.Trailer is { } trailer ? AnistreamFfiMethods.TrailerEmbedPage(trailer, style, Identity) : null;
    }

    /// <summary>
    /// Whether this title's trailer may start by itself now: the engine has said trailers may
    /// play on this device and network, and the title is one the spoiler rule does not hold back.
    /// </summary>
    public static bool MayAutoplay(Known known, Entry entry)
    {
        if (known.Autoplay is not { Trailers: true } || !Embeddable(entry))
            return false;
        var spoilers = SettingsCatalog.IsOn(known.Setting(SettingKey.SpoilerProtection));
        return AnistreamFfiMethods.TrailerMayAutoplay(entry, spoilers);
    }

    /// <summary>The synopsis without the parts a spoiler tag marks, for the preview's two lines.</summary>
    public static string SpoilerFree(Entry entry)
    {
        if (entry.SynopsisSpans is not { Length: > 0 } spans)
            return entry.Synopsis;
        return string.Concat(spans.Where(s => !s.Spoiler).Select(s => s.Text)).Trim();
    }

#if ANISTREAM_TESTING
    /// <summary>
    /// The fixture scene's titles carry no trailer of their own: most are given a made-up one, so
    /// the banner and the preview have a video to play, and one in five none, so that state shows.
    /// </summary>
    public const string FixtureId = "fixtureclip1";

    /// <summary>A trailer that never plays: the window's loading, then its "Couldn't open the trailer".</summary>
    public const string FixtureStuckId = "fixtureclip-stuck";

    public const string FixtureStuckPage = """
        <!doctype html><html><head><meta charset="utf-8"><style>html,body{margin:0;background:transparent}</style></head><body><script>
        function say(m){try{window.chrome.webview.postMessage(m)}catch(e){}}
        window.trailer={play:function(){},pause:function(){},mute:function(){},unmute:function(){},remaining:function(){return -1}};
        say("ready");
        </script></body></html>
        """;

    /// <summary>
    /// A page that says what the real one would, for the shots and for trying the app offline: a
    /// slow gradient in place of the video, <c>ready</c> at once and <c>playing</c> a moment later.
    /// </summary>
    public const string FixturePage = """
        <!doctype html><html><head><meta charset="utf-8"><style>
        html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}
        #clip{width:100%;height:100%;background:linear-gradient(120deg,#1d3557,#e76f51,#f4a261,#264653);
        background-size:300% 300%;animation:drift 6s linear infinite}
        @keyframes drift{0%{background-position:0% 50%}100%{background-position:100% 50%}}
        </style></head><body><div id="clip"></div><script>
        function say(m){try{window.chrome.webview.postMessage(m)}catch(e){}}
        var t0=Date.now(),len=30;
        window.trailer={play:function(){say("playing")},pause:function(){say("paused")},mute:function(){},unmute:function(){},
        remaining:function(){return Math.max(0,len-(Date.now()-t0)/1000)}};
        say("ready");setTimeout(function(){say("playing")},700);setTimeout(function(){say("ended")},len*1000);
        </script></body></html>
        """;

    public static Entry WithFixtureTrailer(Entry entry) =>
        entry.Trailer is null && entry.Id % 5 != 4
            ? entry with { Trailer = new Trailer(TrailerHost.YouTube, FixtureId), TrailerUrl = "https://video.example/watch?v=" + FixtureId }
            : entry;
#endif
}
