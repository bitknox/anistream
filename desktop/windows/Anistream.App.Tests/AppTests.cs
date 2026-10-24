// The Windows app's own rules, and the testing engine's Watching scene read the way the app
// reads it: through EngineSession into Known, then the model's own queue rules.

using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

static class HomeGuard
{
    public static void Check()
    {
        var guarded = Environment.GetEnvironmentVariable("ANISTREAM_TEST_HOME");
        var home = Environment.GetEnvironmentVariable(OperatingSystem.IsWindows() ? "USERPROFILE" : "HOME");
        Assert.False(string.IsNullOrEmpty(guarded), "run the tests with desktop/windows/scripts/test.sh, which gives them a temporary HOME");
        Assert.Equal(guarded, home);
    }
}

class QuietHost : FfiHost
{
    public bool OpenUrl(string url) => false;
    public void Notify(string title, string body) { }
    public string? SecretGet(string id) => null;
    public bool SecretSet(string id, string value) => false;
    public bool SecretDelete(string id) => true;
}

public class KnownTests
{
    static Entry Title(uint id, string title) => new(Id: id, Title: title);

    [Fact]
    public void ProgressMovesTheTitleWhereverItWasSeen()
    {
        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Continue(), new ContentList.Entries([Title(7, "Stardust Wayfarer")])));
        known.Take(new Event.Progress(7, new TitleProgress(Done: 3, Next: 4), new ResumePoint(Position: 60), []));
        var entry = known.ListEntries(new ContentFor.Continue()).Single();
        Assert.Equal(3u, entry.Progress?.Done);
        Assert.Equal(60.0, entry.Resume?.Position);
    }

    [Fact]
    public void AListRowKeepsWhatTheTitlePageKnew()
    {
        var known = new Known();
        known.Take(new Event.Detail(Title(7, "Stardust Wayfarer") with { Synopsis = "A courier crosses the sky.", BannerUrl = "http://127.0.0.1/b.jpg" }));
        known.Take(new Event.Content(new ContentFor.Seasonal(), new ContentList.Entries([Title(7, "Stardust Wayfarer")])));
        var entry = known.Entry(7)!;
        Assert.Equal("A courier crosses the sky.", entry.Synopsis);
        Assert.Equal("http://127.0.0.1/b.jpg", entry.BannerUrl);
    }

    [Fact]
    public void CalendarRowsKeepTheirOwnBroadcasts()
    {
        var known = new Known();
        var rows = new[]
        {
            Title(7, "Kumo no Ue no Toshokan") with { AiringEpisode = 5, AiringIn = -420_000 },
            Title(7, "Kumo no Ue no Toshokan") with { AiringEpisode = 6, AiringIn = 184_500 },
        };
        known.Take(new Event.Content(new ContentFor.Calendar(), new ContentList.Entries(rows)));
        Assert.Equal([5u, 6u], known.ListRows(new ContentFor.Calendar()).Select(r => r.AiringEpisode!.Value));
    }

    [Fact]
    public void ClosingCountsWhatDownloadsAndWhatSeeds()
    {
        static DownloadRow Row(DownloadState state) => new(
            Id: 1, AnilistId: 1, Title: "Atelier Hotaru", Episode: "1", State: state, Fraction: 0,
            Downloaded: 0, Total: 0);
        var known = new Known();
        known.Take(new Event.Downloads([Row(DownloadState.Active), Row(DownloadState.Queued), Row(DownloadState.Done), Row(DownloadState.Paused)]));
        Assert.Equal((2, 0), known.Running());
    }
}

public class CalendarTests
{
    static readonly TimeZoneInfo Utc = TimeZoneInfo.Utc;

    [Fact]
    public void TheWeekStartsOnMonday()
    {
        // Tuesday 20 October 2026, 19:30 UTC.
        var tuesday = new DateTimeOffset(2026, 10, 20, 19, 30, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        Assert.Equal(new DateOnly(2026, 10, 19), Calendar.WeekStart(tuesday, 0, Utc));
        Assert.Equal(new DateOnly(2026, 10, 26), Calendar.WeekStart(tuesday, 1, Utc));
        Assert.Equal("19 to 25 October 2026", Calendar.WeekTitle(new DateOnly(2026, 10, 19)));
        Assert.Equal("28 September to 4 October 2026", Calendar.WeekTitle(new DateOnly(2026, 9, 28)));
    }

    [Fact]
    public void BroadcastsFallOnTheirDayAndSayWhenTheyMoved()
    {
        var anchor = new DateTimeOffset(2026, 10, 20, 12, 0, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        var rows = new[]
        {
            new Entry(Id: 1, Title: "Atelier Hotaru", AiringIn: 4 * 3600 + 1800, AiringEpisode: 10, DelayedFrom: anchor + 11 * 3600),
            new Entry(Id: 2, Title: "Natsukaze Relay", AiringIn: -86_400, AiringEpisode: 8),
            new Entry(Id: 3, Title: "No schedule"),
        };
        var broadcasts = Calendar.Broadcasts(rows, anchor, new Known(), mine: false);
        Assert.Equal([2u, 1u], broadcasts.Select(b => b.Entry.Id));
        Assert.Equal("Ep 10 · 16:30 (was 23:00)", broadcasts[1].Line(Utc));
        var week = Calendar.Week(new DateOnly(2026, 10, 19), broadcasts, Utc);
        Assert.Equal(2u, week[0].Broadcasts.Single().Entry.Id);
        Assert.Equal(1u, week[1].Broadcasts.Single().Entry.Id);
        Assert.True(broadcasts[0].IsNew(anchor));
        Assert.Empty(Calendar.Broadcasts(rows, anchor, new Known(), mine: true));
    }
}

public class SharedWordsTests
{
    [Fact]
    public void TheQueueReadsAsTheOtherAppsReadIt()
    {
        var entry = new Entry(
            Id: 900_001, Title: "Stardust Wayfarer", Episodes: 12,
            Progress: new TitleProgress(Done: 6, Next: 7),
            Resume: new ResumePoint(Position: 606, Fraction: 606.0 / 1440));
        var queue = AnistreamFfiMethods.ContinueQueue([entry], [], Casing.Sentence);
        Assert.Equal("Resume episode 7 · 10:06", queue.InProgress[0].Button);
        Assert.Equal("Resume E7 · 10:06", queue.InProgress[0].Action);
        Assert.Equal("4:05", AnistreamFfiMethods.PlayClock(245.4));
        Assert.Equal("auto-play next", AnistreamFfiMethods.SettingLabel(SettingKey.AutoNext));
    }
}

public class SceneTests
{
    [Fact(Timeout = 90_000)]
    public async Task TheWatchingSceneFillsContinue()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-app");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Watching(), EngineSession.AppStart());
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            session.Start();
            session.Send(new Command.LoadContinue());
            Entry[] entries = [];
            var deadline = DateTime.UtcNow.AddSeconds(60);
            while (entries.Length < 4 && DateTime.UtcNow < deadline)
            {
                await Task.Delay(100, TestContext.Current.CancellationToken);
                lock (gate) entries = session.Known.ListEntries(new ContentFor.Continue());
            }
            Assert.Equal(4, entries.Length);
            var queue = AnistreamFfiMethods.ContinueQueue([.. entries], [], Casing.Sentence);
            var first = queue.InProgress[0];
            Assert.Equal(900_001u, first.Entry.Id);
            Assert.Equal("Resume episode 7 · 10:06", first.Button);
        }
        DeleteRoot(root);
    }

    static void DeleteRoot(string root)
    {
        var deadline = DateTime.UtcNow.AddSeconds(10);
        while (true)
        {
            try
            {
                Directory.Delete(root, recursive: true);
                return;
            }
            catch (DirectoryNotFoundException)
            {
                return;
            }
            catch (Exception) when (DateTime.UtcNow < deadline)
            {
                Thread.Sleep(200);
            }
        }
    }
}
