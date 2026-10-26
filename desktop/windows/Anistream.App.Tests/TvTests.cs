// Play on TV on Windows: what it says, and the remote driving a fake TV beside the testing
// engine, from finding it through pairing with its code to the card and a command.

using System.Globalization;
using Anistream.App;
using Anistream.Engine.Ffi;
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
        Assert.Equal("Open anistream on the TV. It needs to be on the same Wi-Fi as this PC.", TvWords.PickerLine(null));
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
        Assert.Equal("English · Signs", TvWords.Track(tracks[1], english));
        Assert.Equal("Off", TvWords.Chosen(tracks, off: true));
        Assert.Equal("Track 9", TvWords.Track(new FfiTvTrack(9, null, null, false), english));
        Assert.Equal("A TV lists this PC with the code AB12. Forget a TV here and it forgets this PC too.", TvWords.Footnote("AB12"));
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
}
