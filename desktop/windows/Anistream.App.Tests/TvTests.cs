// Play on TV on Windows: what it says, and the remote driving a fake TV beside the testing
// engine, from finding it through pairing with its code to the card and a command.

using System.Globalization;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class TvWordsTests
{
    [Fact]
    public void ThePickerSaysWhatChoosingATvPlays()
    {
        Assert.Equal("Pair a TV", TvWords.PickerTitle(null));
        Assert.Equal("Play on TV", TvWords.PickerTitle(new TvPlay(1, "Show", "3")));
        Assert.Equal("Show, episode 3", TvWords.PickerLine(new TvPlay(1, "Show", "3")));
        Assert.Equal("Episode 3", TvWords.PickerLine(new TvPlay(1, "", "3")));
        Assert.Equal("Open anistream on the TV. It needs to be on the same Wi-Fi as this device.", TvWords.PickerLine(null));
    }

    [Fact]
    public void PairingSaysWhatItWaitsFor()
    {
        var connecting = new TvPairingStep("", NeedsCode: true, Working: true);
        Assert.False(connecting.Typing);
        Assert.Equal("Connecting to the TV", TvWords.PairingTitle(connecting));
        Assert.Equal("This takes a moment.", TvWords.PairingLine(connecting));
        var typing = new TvPairingStep("Living room", NeedsCode: true, Working: false);
        Assert.True(typing.Typing);
        Assert.Equal("Pair with Living room", TvWords.PairingTitle(typing));
        Assert.Equal("Pairing with Living room", TvWords.PairingTitle(typing with { NeedsCode = false }));
    }

    [Fact]
    public void OnlySixDigitsAreACode()
    {
        Assert.Null(TvWords.SixDigits("12345"));
        Assert.Equal("123456", TvWords.SixDigits("123 456"));
        Assert.Null(TvWords.SixDigits("1234567"));
    }

    [Fact]
    public void TheCardsLinesAndDot()
    {
        Assert.Equal(TvWords.Dot.Water, TvWords.DotFor(FfiLinkPhase.Connected));
        Assert.Equal(TvWords.Dot.Vermilion, TvWords.DotFor(FfiLinkPhase.Unpaired));
        Assert.Equal(TvWords.Dot.Vermilion, TvWords.DotFor(FfiLinkPhase.KeyChanged));
        Assert.Equal(TvWords.Dot.Haze, TvWords.DotFor(FfiLinkPhase.Retrying));
        Assert.True(TvWords.Lost(new FfiRemoteCard("TV", FfiLinkPhase.KeyChanged, null, new FfiScreen.Home(), null, 1)));
        Assert.False(TvWords.Lost(new FfiRemoteCard("TV", FfiLinkPhase.Retrying, null, new FfiScreen.Home(), null, 1)));
        FfiTvTrack[] tracks = [new(1, "ja", null, false), new(2, "en", "Signs", true)];
        var english = CultureInfo.GetCultureInfo("en-GB");
        Assert.Equal("English (Signs)", TvWords.Track(tracks[1], english));
        Assert.Equal("Off", TvWords.Chosen(tracks, off: true));
        Assert.Equal("Track 9", TvWords.Track(new FfiTvTrack(9, null, null, false), english));
        Assert.Equal("A TV lists this device with the code AB12. Forget a TV here and it forgets this device too.", TvWords.Footnote("AB12"));
    }

    [Fact]
    public void TheTitleBarSaysHowTheLinkStands()
    {
        FfiRemoteCard Card(FfiLinkPhase phase) => new("Living room", phase, null, new FfiScreen.Home(), null, 1);
        Assert.Equal("Playing on Living room", TvWords.BarLine(Card(FfiLinkPhase.Connected), playing: true));
        Assert.Equal("Living room", TvWords.BarLine(Card(FfiLinkPhase.Connected), playing: false));
        Assert.Equal("Connecting to Living room", TvWords.BarLine(Card(FfiLinkPhase.Connecting), playing: true));
        Assert.Equal("Reconnecting to Living room", TvWords.BarLine(Card(FfiLinkPhase.Retrying), playing: true));
        Assert.Equal("Pair again with Living room", TvWords.BarLine(Card(FfiLinkPhase.Unpaired), playing: true));
    }

    static FfiCastCard Cast(FfiLinkPhase phase, FfiNowCard? now, Msg? notice = null, string? phaseLine = null) =>
        new(new FfiRemoteCard("Living room", phase, phaseLine, new FfiScreen.Home(), now, 1, Over: notice is not null, Reached: false),
            FfiVolumeControl.MuteOnly, false, notice);

    static FfiNowCard Now(bool loading) => new(900_001, "The Lantern Road", "7", "Episode 7 · The Lantern Road", loading,
        492, loading ? null : 1420, false, 1, null, [], [], false, null);

    [Fact]
    public void ACastsStageIsTheEnginesReasonOrWhetherTheTvShowsAPicture()
    {
        Assert.Equal(CastStage.Starting, TvWords.StageOf(null));
        Assert.Equal(CastStage.Starting, TvWords.StageOf(Cast(FfiLinkPhase.Connecting, null)));
        Assert.Equal(CastStage.Starting, TvWords.StageOf(Cast(FfiLinkPhase.Connected, Now(loading: true))));
        Assert.Equal(CastStage.Playing, TvWords.StageOf(Cast(FfiLinkPhase.Connected, Now(loading: false))));
        Assert.Equal(CastStage.Lost, TvWords.StageOf(Cast(FfiLinkPhase.Connected, Now(false), new Msg.NoticeCastLost("Living room"))));
        Assert.Equal(CastStage.Failed, TvWords.StageOf(Cast(FfiLinkPhase.Connected, null, new Msg.NoticeCastRefused("Living room"))));
    }

    [Fact]
    public void ACastThatEndedOffersByItsReason()
    {
        Assert.Equal([CastOffer.ChooseSource, CastOffer.PlayHere], TvWords.OffersFor(new Msg.NoticeCastRefused("Living room")));
        Assert.Equal([CastOffer.ChooseSource, CastOffer.PlayHere], TvWords.OffersFor(new Msg.NoticeCastVideoCodec("Living room", "AV1")));
        Assert.Equal([CastOffer.TryAgain, CastOffer.PlayHere], TvWords.OffersFor(new Msg.NoticeCastNeverFetches("Living room")));
        Assert.Equal([CastOffer.ChooseSource], TvWords.OffersFor(new Msg.NoticeCastDeepLink()));
        Assert.Equal([CastOffer.PlayHere, CastOffer.ChooseAnother], TvWords.OffersFor(new Msg.NoticeCastNotSameNetwork("Living room")));
        Assert.Equal([CastOffer.PlayHere, CastOffer.ChooseAnother], TvWords.OffersFor(new Msg.NoticeCastLost("Living room")));
        Assert.True(TvWords.OnCastCard(new Msg.NoticeCastTakenOver("Living room")));
        Assert.False(TvWords.OnCastCard(new Msg.NoticeCastNoDevice()));
    }

    [Fact]
    public void ACastsLineCountsPastFourSeconds()
    {
        var starting = Cast(FfiLinkPhase.Connecting, null);
        Assert.Equal("Starting on Living room", TvWords.CastStarting(starting, "Living room", TimeSpan.FromSeconds(3.9)));
        Assert.Equal("Starting on Living room · 6 s", TvWords.CastStarting(starting, "Living room", TimeSpan.FromSeconds(6.4)));
        var waiting = Cast(FfiLinkPhase.Connected, null, phaseLine: "Waiting for Living room");
        Assert.Equal("Waiting for Living room · 5 s", TvWords.CastStarting(waiting, "Living room", TimeSpan.FromSeconds(5)));
        var said = Cast(FfiLinkPhase.Connected, null) with { PhaseLine = new Msg.CastPhasePreparing(5) };
        Assert.Equal("Preparing the stream · 5 s", TvWords.CastStarting(said, "Living room", TimeSpan.FromSeconds(9)));
        Assert.Equal("Playing on Living room", TvWords.CastStatus(CastStage.Playing, null, "Living room", TimeSpan.Zero));
        Assert.Equal("Couldn't start on Living room", TvWords.CastStatus(CastStage.Failed, null, "Living room", TimeSpan.Zero));
        Assert.Equal("Casting stopped", TvWords.CastStatus(CastStage.Lost, null, "Living room", TimeSpan.Zero));
        Assert.Equal(TvWords.Dot.Water, TvWords.CastDot(CastStage.Playing));
        Assert.Equal(TvWords.Dot.Vermilion, TvWords.CastDot(CastStage.Failed));
        Assert.Equal(TvWords.Dot.Haze, TvWords.CastDot(CastStage.Starting));
        Assert.Equal("Set on the TV", TvWords.CastVolume(Cast(FfiLinkPhase.Connected, Now(false))));
        Assert.Equal("Unmute", TvWords.MuteWord(true));
    }

    [Fact]
    public void ACastsUpNextCountsAndASkipCanBeUndone()
    {
        var next = new FfiCastUpNext("8", "A Town Without Night", null, true, 6);
        Assert.Equal("Up next · plays in 6 s", TvWords.CastUpNext(next));
        Assert.Equal("Episode 8 · A Town Without Night", TvWords.CastUpNextEpisode(next));
        Assert.Equal("Up next", TvWords.CastUpNext(next with { Auto = false, SecondsLeft = null }));
        Assert.Equal("Episode 8", TvWords.CastUpNextEpisode(next with { Title = null }));
        Assert.Equal("Skipped opening", TvWords.CastSkipped(new FfiCastUndo(SkipKind.Opening, 0)));
        Assert.Equal("Skipped ending", TvWords.CastSkipped(new FfiCastUndo(SkipKind.Ending, 1300)));
        var row = new FfiCastRow("k", "Living room", "", "192.0.2.20", FfiCastRouteHint.ThroughThisDevice);
        Assert.Equal("Plays through this PC", TvWords.CastRoute(row));
        Assert.Null(TvWords.CastRoute(row with { RouteHint = FfiCastRouteHint.Direct }));
    }

    [Fact]
    public void SettingsSaysWhenATvWasPairedAndSeen()
    {
        var now = DateTimeOffset.FromUnixTimeSeconds(1_800_000_000);
        var tv = new FfiPairedTv("fp", "AB12", "Bedroom", "192.168.1.20:47811", PairedAt: now.ToUnixTimeSeconds(), LastSeen: now.ToUnixTimeSeconds() - 2);
        Assert.Equal("Connected · 192.168.1.20:47811", TvWords.Note(tv, connected: true, now));
        var note = TvWords.Note(tv, connected: false, now, CultureInfo.InvariantCulture);
        Assert.StartsWith("Paired ", note);
        Assert.EndsWith(" · seen just now", note);
        Assert.DoesNotContain("seen", TvWords.Note(tv with { LastSeen = null }, connected: false, now));
    }
}

public class TvRemoteTests
{
    /// <summary>A network with no TV running the app on it.</summary>
    sealed class NoTvs : FfiTvBrowser
    {
        public void Start(FfiDiscoverySink sink)
        {
        }

        public void Stop()
        {
        }
    }

    /// <summary>A <c>_googlecast._tcp</c> browse that finds the fake TVs where they listen.</summary>
    sealed class FakeCastBrowse(params FfiFakeCast[] tvs) : FfiCastBrowser
    {
        public void Start(FfiCastSink sink)
        {
            foreach (var tv in tvs)
            {
                var advert = tv.Advert();
                sink.Found(advert.Instance, [tv.Host()], advert.Port, advert.Txt);
            }
        }

        public void Stop()
        {
        }
    }

    /// <summary>A browse that finds the fake TV where it listens, as DNS-SD would.</summary>
    sealed class FakeBrowse(FfiFakeTv tv) : FfiTvBrowser
    {
        public void Start(FfiDiscoverySink sink)
        {
            var advert = tv.Advert();
            sink.Found(advert.Instance, [tv.Host()], tv.Port(), advert.Txt);
        }

        public void Stop()
        {
        }
    }

    static async Task Until(Func<bool> there, string what)
    {
        var deadline = DateTime.UtcNow.AddSeconds(30);
        while (!there())
        {
            Assert.True(DateTime.UtcNow < deadline, $"waited 30 s for {what}");
            await Task.Delay(50, TestContext.Current.CancellationToken);
        }
    }

    [Fact(Timeout = 90_000)]
    public async Task PairingWithTheCodeThenPlayingDrivesTheTv()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-tv");
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Fresh(), EngineSession.AppStart());
        var fake = engine.StartFakeTv("Living room");
        try
        {
            var remote = new TvRemote();
            remote.Attach(engine.TvSender("PC", "anistream-windows/test"), new FakeBrowse(fake));
            Assert.True(remote.Available);
            Assert.NotEmpty(remote.ShortFingerprint);

            remote.Offer(new TvPlay(900_001, "", "3", 312));
            Assert.Equal(TvSheet.Picker, remote.Sheet);
            remote.StartBrowsing();
            await Until(() => remote.Rows.Any(r => r.Name == "Living room" && r.Kind == FfiTvRowKind.New), "the TV's row");
            Assert.Equal("Not paired yet", remote.Rows.Single().Detail);

            remote.Choose(remote.Rows.Single());
            await Until(() => remote.Pairing is { Typing: true, Working: false }, "the code to type");
            var shown = fake.ShownCode();
            Assert.NotNull(shown);

            // A wrong code says so in the engine's words, and the pairing waits for another.
            await remote.Submit(shown == "111111" ? "222222" : "111111");
            await Until(() => remote.Pairing is { Working: false, Problem: not null }, "the wrong code's problem");
            Assert.Equal("That code didn't match. Check the code on the TV and try again.", remote.Pairing!.Problem);
            Assert.False(remote.Pairing.Expired);

            // Wrong codes until the TV gives up on its code: then only a new code goes on.
            for (var i = 0; i < 5 && remote.Pairing is { Expired: false }; i++)
            {
                await remote.Submit(shown == "111111" ? "222222" : "111111");
                await Until(() => remote.Pairing is { Working: false, Problem: not null }, "another wrong code's problem");
            }
            Assert.True(remote.Pairing!.Expired);
            await remote.RequestCodeAgain();
            await Until(() => remote.Pairing is { Typing: true, Working: false, Problem: null, Expired: false }, "a new code");
            shown = fake.ShownCode();
            Assert.NotNull(shown);

            await remote.Submit(shown!);
            await Until(() => remote.Connected is not null && remote.Sheet == TvSheet.Remote, "the link");
            Assert.Null(remote.Pairing);
            Assert.Single(remote.Paired);
            // What was asked shows as starting until the TV's card names it.
            await Until(() => remote.Now is { Loading: false, Id: 900_001, Episode: "3" }, "the TV playing the episode");
            Assert.True(remote.Live);
            Assert.Contains("play 900001 3", fake.Commands());

            await remote.Send(new FfiTvCommand.TogglePause());
            await Until(() => remote.Now is { Paused: true }, "the TV paused");
            Assert.Null(remote.Problem);

            // Forgotten here, the TV is let go of and the card goes.
            remote.SheetClosed();
            await remote.Forget(remote.Connected!);
            Assert.Null(remote.Connected);
            Assert.Empty(remote.Paired);
            remote.StopBrowsing();
        }
        finally
        {
            fake.Stop();
            engine.Shutdown();
            engine.Dispose();
        }
        SceneTests.DeleteRoot(root);
    }

    [Fact(Timeout = 90_000)]
    public async Task CastingPlaysOnTheTvThenPlayHereHandsTheEpisodeBack()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-cast");
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Fresh(), EngineSession.AppStart());
        var living = engine.StartFakeCast("Living room", new FfiFakeCastOptions(FixedVolume: true, Fetches: true, Refuses: false, LaunchMs: 0));
        try
        {
            var remote = new TvRemote();
            var served = 0;
            remote.Served = () => served++;
            remote.Attach(engine.TvSender("PC", "anistream-windows/test"), new NoTvs(), new FakeCastBrowse(living));
            remote.Offer(new TvPlay(900_001, "", "3", null, FromPlayer: true));
            var handedOff = 0;
            remote.HandedOff = () => handedOff++;
            remote.StartBrowsing();
            await Until(() => remote.CastRows.Any(r => r.Name == "Living room"), "the Chromecast row");

            remote.Cast(remote.CastRows.Single(r => r.Name == "Living room"));
            Assert.Equal(TvSheet.Cast, remote.Sheet);
            Assert.True(remote.Casting);
            Assert.Null(remote.Request);
            await Until(() => remote.Stage == CastStage.Playing, "the cast playing");
            Assert.Equal(1, served);
            Assert.True(remote.CastServed);
            Assert.Equal(1, handedOff);
            Assert.Equal(FfiVolumeControl.MuteOnly, remote.CastCard!.VolumeControl);

            remote.SendCast(new FfiTvCommand.Mute(true));
            await Until(() => remote.CastCard?.Muted == true, "the TV muted");

            // Play here ends the cast and hands back what it played.
            var here = remote.PlayHere();
            Assert.Equal(900_001u, here?.Id);
            Assert.Equal("3", here?.Episode);
            Assert.False(remote.Casting);
            Assert.Null(remote.Sheet);
            remote.StopBrowsing();
        }
        finally
        {
            living.Stop();
            engine.Shutdown();
            engine.Dispose();
        }
        SceneTests.DeleteRoot(root);
    }
}
