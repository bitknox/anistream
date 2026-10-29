// Trailers: the flow that decides when the video shows, the engine's page under the app's identity,
// and who may autoplay.

using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class TrailerTests
{
    static readonly Trailer Clip = new(TrailerHost.YouTube, "clip0001");

    static Entry Titled(uint id, Trailer? trailer = null) => new(Id: id, Title: "Lantern Road", Trailer: trailer);

    [Fact]
    public void TheVideoShowsOnlyOnceAFrameIsPlaying()
    {
        var flow = new TrailerFlow();
        Assert.True(flow.Start());
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.None, flow.Heard(new TrailerMessage.Ready()));
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.ShowVideo, flow.Heard(new TrailerMessage.Playing()));
        Assert.True(flow.Showing);
    }

    [Fact]
    public void AnEndOrAnErrorLetsGoForGoodOnThisVisit()
    {
        foreach (TrailerMessage over in new TrailerMessage[] { new TrailerMessage.Ended(), new TrailerMessage.Error(150) })
        {
            var flow = new TrailerFlow();
            flow.Start();
            flow.Heard(new TrailerMessage.Playing());
            var effect = flow.Heard(over);
            Assert.True(effect.HasFlag(TrailerEffect.Release) && effect.HasFlag(TrailerEffect.HideVideo));
            Assert.Equal(TrailerPhase.Done, flow.Phase);
            Assert.False(flow.Start());
        }
    }

    [Fact]
    public void AnErrorBeforeAnyFrameLeavesTheBannerAloneAndSaysNothing()
    {
        var flow = new TrailerFlow();
        flow.Start();
        var effect = flow.Heard(new TrailerMessage.Error(0));
        Assert.Equal(TrailerEffect.Release, effect);
        Assert.False(flow.CanStart);
    }

    [Fact]
    public void ScrollingAwayPausesAndTheVideoComesBackWhenItPlaysAgain()
    {
        var flow = new TrailerFlow();
        flow.Start();
        flow.Heard(new TrailerMessage.Playing());
        Assert.Equal(TrailerEffect.PausePlayer | TrailerEffect.HideVideo, flow.Pause());
        Assert.True(flow.Active);
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.PlayPlayer, flow.Resume());
        Assert.False(flow.Showing);
        Assert.Equal(TrailerEffect.ShowVideo, flow.Heard(new TrailerMessage.Playing()));
    }

    [Fact]
    public void GivingUpWhileLoadingMayTryAgainButStoppingAPlayingTrailerFadesTheVideoAway()
    {
        var flow = new TrailerFlow();
        flow.Start();
        Assert.Equal(TrailerEffect.Release, flow.GiveUp());
        Assert.True(flow.CanStart);
        flow.Start();
        flow.Heard(new TrailerMessage.Playing());
        Assert.Equal(TrailerEffect.None, flow.GiveUp());
        Assert.Equal(TrailerEffect.Release | TrailerEffect.HideVideo, flow.Stop());
        Assert.Equal(TrailerEffect.None, flow.Stop());
    }

    [Fact]
    public void TheEnginesPageIsLoadedUnderTheAppsOriginAndTheBannerOneIsMuted()
    {
        var entry = Titled(1, Clip);
        var banner = Trailers.Page(entry, EmbedStyle.Banner);
        Assert.NotNull(banner);
        Assert.Contains(Trailers.Origin, banner);
        Assert.Contains("clip0001", banner);
        Assert.Equal("https://tv.anistream.windows/", AnistreamFfiMethods.TrailerBaseUrl(Trailers.Identity));
        Assert.Equal("tv.anistream.windows", Trailers.Host);
        Assert.Null(Trailers.Page(Titled(2), EmbedStyle.Banner));
        Assert.Null(Trailers.Page(Titled(3, new Trailer(TrailerHost.Dailymotion, "x1")), EmbedStyle.Banner));
    }

    [Fact]
    public void MessagesFromThePageAreReadByTheEngine()
    {
        Assert.IsType<TrailerMessage.Playing>(AnistreamFfiMethods.TrailerMessage("playing"));
        Assert.Equal(new TrailerMessage.Error(101), AnistreamFfiMethods.TrailerMessage("error:101"));
        Assert.Null(AnistreamFfiMethods.TrailerMessage("hello"));
    }

    [Fact]
    public void TrailersWaitForTheEnginesWordAndFollowIt()
    {
        var known = new Known();
        var entry = Titled(1, Clip);
        Assert.False(Trailers.MayAutoplay(known, entry));
        known.Take(new Event.Autoplay(new AutoplayView(true)));
        Assert.True(Trailers.MayAutoplay(known, entry));
        Assert.False(Trailers.MayAutoplay(known, Titled(2)));
        known.Take(new Event.Autoplay(new AutoplayView(false, AutoplayHold.Network)));
        Assert.False(Trailers.MayAutoplay(known, entry));
    }

    [Fact]
    public void ThePreviewLeavesOutWhatASpoilerTagMarks()
    {
        var entry = Titled(1) with
        {
            Synopsis = "A courier crosses the lake. She dies at the end.",
            SynopsisSpans = [new SynopsisSpan("A courier crosses the lake. "), new SynopsisSpan("She dies at the end.", true)],
        };
        Assert.Equal("A courier crosses the lake.", Trailers.SpoilerFree(entry));
        Assert.Equal("Plain.", Trailers.SpoilerFree(Titled(2) with { Synopsis = "Plain." }));
    }

    [Fact]
    public void PlayTrailersOffersTheThreeChoicesTheEngineKeeps()
    {
        var options = SettingsCatalog.Options(SettingKey.Trailers);
        Assert.Equal(["always", "Wi-Fi only", "off"], options.Select(o => o.Label));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(new SettingRow(SettingKey.Trailers, "always", new SettingChange.Steps(true, true))));
    }
}
