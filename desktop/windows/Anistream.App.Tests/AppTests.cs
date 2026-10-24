// The Windows app's own rules, and the testing engine's Watching scene read the way the app
// reads it: through EngineSession into Known, then the model's own queue rules.

using Anistream.App;
using Anistream.Engine.Core;
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
    public void OfflineFollowsTheEnginesConnectivity()
    {
        var known = new Known();
        Assert.False(known.Offline);
        known.Take(new Event.Connectivity(true));
        Assert.True(known.Offline);
        known.Take(new Event.Connectivity(false));
        Assert.False(known.Offline);
    }

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
        System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.GetCultureInfo("en-GB");
        // Tuesday 20 October 2026, 19:30 UTC.
        var tuesday = new DateTimeOffset(2026, 10, 20, 19, 30, 0, TimeSpan.Zero).ToUnixTimeSeconds();
        Assert.Equal(new DateOnly(2026, 10, 19), Calendar.WeekStart(tuesday, 0, Utc));
        Assert.Equal(new DateOnly(2026, 10, 26), Calendar.WeekStart(tuesday, 1, Utc));
        Assert.Equal("19 to 25 October 2026", Calendar.WeekTitle(new DateOnly(2026, 10, 19)));
        Assert.Equal("28 September to 4 October 2026", Calendar.WeekTitle(new DateOnly(2026, 9, 28)));
        // The system's clock: a 12-hour locale reads its own way.
        var us = System.Globalization.CultureInfo.GetCultureInfo("en-US");
        Assert.Matches(@"^7:30\sPM$", Calendar.Time(tuesday, Utc, us));
    }

    [Fact]
    public void BroadcastsFallOnTheirDayAndSayWhenTheyMoved()
    {
        System.Globalization.CultureInfo.CurrentCulture = System.Globalization.CultureInfo.GetCultureInfo("en-GB");
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

public class SettingsCatalogTests
{
    static SettingRow Row(SettingKey key, string value, SettingChange? change = null) =>
        new(key, value, change ?? new SettingChange.Steps(true, true));

    [Fact]
    public void ARowsControlFollowsItsKeyNotItsValue()
    {
        // A switch is a switch whatever words the row says, and a ladder is a menu even when one
        // of its values reads "off".
        Assert.Equal(SettingControl.Switch, SettingsCatalog.ControlFor(Row(SettingKey.AutoNext, "on")));
        Assert.Equal(SettingControl.Switch, SettingsCatalog.ControlFor(Row(SettingKey.Torrents, "off")));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(Row(SettingKey.UpNextCountdown, "off")));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(Row(SettingKey.Upscaling, "off")));
        Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(Row(SettingKey.Quality, "1080p")));
        Assert.Equal(SettingControl.Text, SettingsCatalog.ControlFor(Row(SettingKey.DownloadDir, "D:/Anime", new SettingChange.Typed("D:/Anime"))));
        Assert.Equal(SettingControl.ReadOnly, SettingsCatalog.ControlFor(Row(SettingKey.VpnMode, "external", new SettingChange.Fixed())));
    }

    [Fact]
    public void TheLaddersAreTheEnginesInTheEnginesWords()
    {
        Assert.Equal(["480p", "720p", "1080p", "2160p"], SettingsCatalog.Options(SettingKey.Quality).Select(o => o.Label));
        Assert.Equal(["70%", "80%", "85%", "90%", "95%"], SettingsCatalog.Options(SettingKey.CommitThreshold).Select(o => o.Label));
        Assert.Equal(["off", "natural · ArtCNN", "sharp · Anime4K"], SettingsCatalog.Options(SettingKey.Upscaling).Select(o => o.Label));
        Assert.Equal(["off", "5 s", "10 s", "15 s", "20 s", "30 s"], SettingsCatalog.Options(SettingKey.UpNextCountdown).Select(o => o.Label));
        Assert.Equal(["off", "until ratio 0.5", "until ratio 1.0", "until ratio 1.5", "until ratio 2.0"], SettingsCatalog.Options(SettingKey.SeedRatio).Select(o => o.Label));
        Assert.Equal(["off", "1 episode", "2 episodes", "3 episodes", "5 episodes", "10 episodes"], SettingsCatalog.Options(SettingKey.KeepNextEpisodes).Select(o => o.Label));
        Assert.Equal("unlimited", SettingsCatalog.Options(SettingKey.UploadLimit)[0].Label);
        Assert.Equal("262 kB/s", SettingsCatalog.Options(SettingKey.UploadLimit)[1].Label);
        Assert.Equal(new SettingValue[] { new SettingValue.Bool(false), new SettingValue.Bool(true) }, SettingsCatalog.Options(SettingKey.Syncplay).Select(o => o.Value));
        Assert.Empty(SettingsCatalog.Options(SettingKey.DownloadDir));
        Assert.Empty(SettingsCatalog.Options(SettingKey.MpvOptions));
    }

    [Fact]
    public void ARowStandsOnTheOptionItsWordsSay()
    {
        Assert.Equal(new SettingValue.Int(1080), SettingsCatalog.Current(Row(SettingKey.Quality, "1080p"))?.Value);
        Assert.Null(SettingsCatalog.Current(Row(SettingKey.Quality, "1440p")));
        // The session's choice is beside the saved one, and the row stands on the saved one.
        Assert.Equal(new SettingValue.Str("sub"), SettingsCatalog.Current(Row(SettingKey.Translation, "sub · dub this session"))?.Value);
        Assert.True(SettingsCatalog.IsOn(Row(SettingKey.Fullscreen, "on")));
        Assert.False(SettingsCatalog.IsOn(Row(SettingKey.Fullscreen, "off")));
        Assert.False(SettingsCatalog.Editable(Row(SettingKey.VpnMode, "external", new SettingChange.Fixed())));
        Assert.Equal("D:/Anime", SettingsCatalog.TypedText(Row(SettingKey.DownloadDir, "D:/Anime", new SettingChange.Typed("D:/Anime"))));
    }
}

public class SourcesTests
{
    static ProviderRow Plugin(string id, bool usable = true, string state = "ready") =>
        new(Id: id, Kind: "plugin", State: state, Usable: usable, Name: id, Origin: new ProviderOrigin.ByHand());

    static ProviderRow Torrent() =>
        new(Id: "torrent", Kind: "torrent", State: "ready", Usable: true, Name: "Torrents", Origin: new ProviderOrigin.BuiltIn());

    [Fact]
    public void AMoveStepsOverWhatHasNoRowAndKeepsIt()
    {
        var rows = new[] { Torrent(), Plugin("a-archive"), Plugin("b-library") };
        // "plugins" has no row of its own: it stays after the plugins it stands for.
        var order = new[] { "torrent", "plugins", "remote" };
        var down = SourceOrder.Moved(order, rows, "torrent", 1);
        Assert.Equal(["a-archive", "torrent", "b-library", "plugins", "remote"], down);
        Assert.Null(SourceOrder.Moved(order, rows, "torrent", -1));
        var up = SourceOrder.Moved(["a-archive", "b-library", "torrent", "plugins"], rows, "b-library", -1);
        Assert.Equal(["b-library", "a-archive", "torrent", "plugins"], up);
        Assert.Null(SourceOrder.Moved(order, rows, "missing", 1));
    }

    [Fact]
    public void TheRowsAreSortedByTheOrder()
    {
        var rows = new[] { Plugin("a-archive"), Torrent(), Plugin("b-library") };
        var sorted = SourceOrder.Sorted(rows, ["torrent", "plugins"]);
        Assert.Equal(["torrent", "a-archive", "b-library"], sorted.Select(r => r.Id));
        Assert.Equal(["b-library", "torrent", "a-archive"], SourceOrder.Sorted(rows, ["b-library", "torrent"]).Select(r => r.Id));
    }

    [Fact]
    public void AProviderStandsAsItsRowSaysItDoes()
    {
        Assert.Equal(ProviderStanding.Working, Plugin("a").Standing());
        Assert.Equal(ProviderStanding.Failing, Plugin("a", usable: false).Standing());
        Assert.Equal(ProviderStanding.Failing, Plugin("a", state: "down").Standing());
        Assert.Equal(ProviderStanding.Slow, Plugin("a", state: "degraded").Standing());
        Assert.Equal(ProviderStanding.Off, (Plugin("a") with { Disabled = true }).Standing());
        Assert.Equal(ProviderStanding.HeldBack, (Plugin("a") with { HeldBack = true }).Standing());
        Assert.Null(ProviderStanding.Working.StandingLine());
        Assert.Equal("Not answering", ProviderStanding.Failing.StandingLine());
    }
}

public class StoresTests
{
    static StorePluginRow Listed(string id, StorePluginStatus status, string version = "1.0.0", params string[] hosts) =>
        new(Id: id, Name: id, Version: version, Status: status, Hosts: hosts);

    static StoreRow Store(string id, params StorePluginRow[] plugins) =>
        new(Id: id, Name: id + " plugins", Url: $"https://{id}.example", Host: $"{id}.example", Fingerprint: "9f3c 1a2b",
            Member: "Mika", State: new StoreState.Ready(), Plugins: plugins);

    [Fact]
    public void APluginComesFromTheStoreThatKeepsItUpdated()
    {
        var stores = new[]
        {
            Store("one", Listed("films", new StorePluginStatus.OtherStore("two plugins")), Listed("hand", new StorePluginStatus.AddedByHand())),
            Store("two", Listed("films", new StorePluginStatus.Installed()), Listed("hand", new StorePluginStatus.Available())),
        };
        Assert.Equal("two", Stores.OriginOf(stores, "films")?.Store.Id);
        Assert.Equal("one", Stores.OriginOf(stores, "hand")?.Store.Id);
        Assert.Null(Stores.OriginOf(stores, "nothing"));
    }

    [Fact]
    public void UpdatesAndContactsAreCounted()
    {
        var store = Store("one",
            Listed("a", new StorePluginStatus.Update("1.0.0", ["new.example"]), "1.1.0", "x.example", "new.example"),
            Listed("b", new StorePluginStatus.Installed()));
        Assert.Equal(1, Stores.Updates(store));
        Assert.Equal("Contacts x.example and 1 more", Stores.ContactsLine(store.Plugins[0]));
        Assert.Equal("Contacts no hosts", Stores.ContactsLine(store.Plugins[1]));
        Assert.Equal("1.0.0", Stores.InstalledVersion(store.Plugins[0]));
        Assert.Equal("1.0.0 installed", Stores.Standing(store.Plugins[0]));
        Assert.Equal("Run by Ren, Aki · one.example", Stores.RunBy(["Ren", "Aki"], "one.example"));
        Assert.Equal("one.example", Stores.RunBy([], "one.example"));
        Assert.Equal("joined without a name", Stores.JoinedAs(store with { Anonymous = true }));
    }

    [Fact]
    public void AnInviteIsFoundInWhatWasPasted()
    {
        Assert.Equal("https://plugins.example/join/abc#key", Stores.InviteIn("  https://plugins.example/join/abc#key \n"));
        Assert.Equal("https://plugins.example/join/abc#key",
            Stores.InviteIn("anistream://join?link=https%3A%2F%2Fplugins.example%2Fjoin%2Fabc%23key"));
        Assert.Equal("https://plugins.example/join/abc#key", Stores.InviteIn("anistream://plugins.example/join/abc#key"));
        Assert.Null(Stores.InviteIn("https://plugins.example/about"));
        Assert.Null(Stores.InviteIn("not a link"));
    }

    [Fact]
    public void TheEventsFillWhatThePagesRead()
    {
        var known = new Known();
        known.Take(new Event.SourcesChanged());
        Assert.True(known.Rebuilding);
        known.Take(new Event.Providers([new ProviderRow(Id: "torrent", Kind: "torrent", State: "ready", Usable: true, Name: "Torrents", Origin: new ProviderOrigin.BuiltIn())]));
        Assert.False(known.Rebuilding);
        Assert.True(known.ProvidersLoaded);
        known.Take(new Event.Stores([Store("one")]));
        known.Take(new Event.StorePreview(new InvitePreview.Failed("The link is not an invite.")));
        Assert.IsType<InvitePreview.Failed>(known.Preview);
        known.Take(new Event.StoreJoined("one"));
        Assert.Equal("one", known.Joined);
        Assert.Null(known.Preview);
    }
}

public class DownloadsTests
{
    static DownloadRow Row(long id, uint title, DownloadState state, ulong downloaded, ulong total) =>
        new(Id: id, AnilistId: title, Title: $"Show {title}", Episode: id.ToString(), State: state,
            Fraction: total == 0 ? 0 : (double)downloaded / total, Downloaded: downloaded, Total: total);

    [Fact]
    public void TheQueueGroupsByTitleWhereTheFirstRowIs()
    {
        var queue = new DownloadQueue();
        var now = DateTimeOffset.UnixEpoch;
        queue.Apply([
            Row(1, 7, DownloadState.Done, 1000, 1000), Row(2, 9, DownloadState.Queued, 0, 0),
            Row(3, 7, DownloadState.Active, 300, 1000), Row(4, 9, DownloadState.Paused, 50, 100)], now);
        var groups = queue.Groups();
        Assert.Equal([7u, 9u], groups.Select(g => g.Id));
        Assert.Equal([1L, 3L], groups[0].Rows.Select(r => r.Id));
        // A finished download counts its size, the others what they have so far.
        Assert.Equal(1300UL, groups[0].BytesOnDisk);
        Assert.Equal(1350UL, queue.BytesOnDisk);
    }

    [Fact]
    public void ASpeedIsReadFromOneEventToTheNextAndSmoothed()
    {
        var queue = new DownloadQueue();
        var t = DateTimeOffset.UnixEpoch;
        queue.Apply([Row(1, 7, DownloadState.Active, 0, 10_000)], t);
        Assert.Null(queue.SecondsLeft(queue.Rows[0]));
        queue.Apply([Row(1, 7, DownloadState.Active, 1_000, 10_000)], t.AddSeconds(1));
        Assert.Equal(1_000, queue.Rates[1], 3);
        Assert.Equal(9, queue.SecondsLeft(queue.Rows[0]));
        queue.Apply([Row(1, 7, DownloadState.Active, 3_000, 10_000)], t.AddSeconds(2));
        Assert.Equal(1_500, queue.Rates[1], 3);
        // A paused download has no speed.
        queue.Apply([Row(1, 7, DownloadState.Paused, 3_000, 10_000)], t.AddSeconds(3));
        Assert.False(queue.Rates.ContainsKey(1));
    }

    [Fact]
    public void ASeedingFilesSizeIsWhatWentUpOverTheRatio()
    {
        Assert.Equal(400UL, new SeedRow(Name: "x", UploadedBytes: 100, Ratio: 0.25, Goal: 1, LeftSecs: 60).Size());
        Assert.Null(new SeedRow(Name: "x", UploadedBytes: 0, Ratio: 0, Goal: 1, LeftSecs: 60).Size());
    }
}

public class PlayerRulesTests
{
    [Fact]
    public void ItAsksAfterTheEpisodesThatRolledOnWithNobodyThere()
    {
        var still = new StillWatching { After = 3 };
        for (var i = 0; i < 2; i++)
        {
            still.RollingOn();
            still.Started();
            Assert.False(still.FirstPicture());
        }
        still.RollingOn();
        still.Started();
        Assert.True(still.FirstPicture());
        Assert.True(still.Asking);
        // Asked once until it is answered.
        Assert.False(still.FirstPicture());
        still.Answered();
        still.Input();
        Assert.Equal(0, still.RolledOn);
    }

    [Fact]
    public void AnythingTheViewerDoesStartsTheCountOver()
    {
        var still = new StillWatching { After = 2 };
        still.RollingOn();
        still.Started();
        still.Input();
        still.RollingOn();
        still.Started();
        Assert.False(still.FirstPicture());
        // An episode the viewer started is not a roll.
        still.Started();
        Assert.Equal(1, still.RolledOn);
    }

    [Fact]
    public void ZeroNeverAsks()
    {
        var still = new StillWatching { After = 0 };
        for (var i = 0; i < 5; i++)
        {
            still.RollingOn();
            still.Started();
        }
        Assert.False(still.FirstPicture());
    }

    [Fact]
    public void APreviewReadsTheStreamWithOnlyTheOptionsThatReachIt()
    {
        MpvOption[] options =
        [
            new("http-header-fields", "Referer: https://cdn.example/"), new("user-agent", "x"), new("start", "612"),
            new("sid", "no"), new("force-media-title", "Show"), new("tls-verify", "yes"),
        ];
        Assert.Equal(["http-header-fields", "user-agent", "tls-verify"], PreviewSource.Reach(options).Select(o => o.Name));
        Assert.Equal(300, PreviewSource.TimeAt(50, 100, 600), 6);
        Assert.Equal(600, PreviewSource.TimeAt(500, 100, 600), 6);
        Assert.Equal(0, PreviewSource.TimeAt(-4, 100, 600), 6);
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

    /// <summary>
    /// The catalog's words are the engine's: choosing each option of a stepped setting makes the
    /// engine's row say exactly the option's label, so a menu built from the catalog never
    /// drifts from what the rows read.
    /// </summary>
    [Fact(Timeout = 120_000)]
    public async Task EveryOptionInTheCatalogIsWhatTheEngineThenSays()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-catalog");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Fresh(), EngineSession.AppStart());
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            session.Start();
            async Task<SettingRow?> Row(SettingKey key, Func<SettingRow, bool> until)
            {
                var deadline = DateTime.UtcNow.AddSeconds(15);
                SettingRow? row = null;
                while (DateTime.UtcNow < deadline)
                {
                    lock (gate) row = session.Known.Setting(key);
                    if (row is not null && until(row))
                        return row;
                    await Task.Delay(50, TestContext.Current.CancellationToken);
                }
                return row;
            }

            SettingKey[] keys =
            [
                SettingKey.Quality, SettingKey.CommitThreshold, SettingKey.Upscaling, SettingKey.UpNextCountdown,
                SettingKey.DownloadQuality, SettingKey.KeepNextEpisodes, SettingKey.Titles, SettingKey.AutoNext,
                SettingKey.NightMode, SettingKey.SeedHours, SettingKey.UploadLimit, SettingKey.SeedRatio,
            ];
            foreach (var key in keys)
            {
                Assert.NotNull(await Row(key, _ => true));
                foreach (var option in SettingsCatalog.Options(key))
                {
                    session.Send(new Command.SetSetting(key, option.Value));
                    var row = await Row(key, r => r.Value == option.Label);
                    Assert.True(row?.Value == option.Label, $"{key} set to {option.Label}: the engine's row says \"{row?.Value}\"");
                    Assert.Equal(option, SettingsCatalog.Current(row));
                }
            }
            lock (gate)
            {
                Assert.Equal(SettingControl.Choice, SettingsCatalog.ControlFor(session.Known.Setting(SettingKey.Quality)!));
                Assert.Equal(SettingControl.Switch, SettingsCatalog.ControlFor(session.Known.Setting(SettingKey.AutoNext)!));
                Assert.Equal(SettingControl.Text, SettingsCatalog.ControlFor(session.Known.Setting(SettingKey.DownloadDir)!));
            }
        }
        DeleteRoot(root);
    }

    /// <summary>The app's own Simkl and Trakt clients turn those trackers on, so Accounts can sign in to them.</summary>
    [Fact(Timeout = 60_000)]
    public async Task TheAppsClientsTurnOnSimklAndTrakt()
    {
        HomeGuard.Check();
        var root = AppDirs.NewScratchRoot("cs-trackers");
        var gate = new object();
        var engine = FfiEngine.StartScene(AppDirs.Under(root), new QuietHost(), new FfiScene.Fresh(), EngineSession.AppStart());
        using (var session = new EngineSession(engine, a => { lock (gate) a(); }))
        {
            session.Start();
            string[] trackers = [];
            var deadline = DateTime.UtcNow.AddSeconds(20);
            while (!(trackers.Contains("simkl") && trackers.Contains("trakt")) && DateTime.UtcNow < deadline)
            {
                await Task.Delay(100, TestContext.Current.CancellationToken);
                lock (gate) trackers = [.. session.Known.Sync.Keys];
            }
            Assert.Contains("simkl", trackers);
            Assert.Contains("trakt", trackers);
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

public class ShowSettingsTests
{
    [Fact]
    public void EveryRowTheEngineHasOffersWhatItsSettingTakes()
    {
        foreach (var key in Enum.GetValues<ShowSettingKey>())
            Assert.NotEmpty(ShowSettingsCatalog.Choices(key));
        Assert.Equal(["Japanese", "English"], ShowSettingsCatalog.Choices(ShowSettingKey.Translation).Select(c => c.Label));
        Assert.Equal(["480p", "720p", "1080p", "2160p"], ShowSettingsCatalog.Choices(ShowSettingKey.Quality).Select(c => c.Label));
        Assert.Equal(["Standard", "Higher"], ShowSettingsCatalog.Choices(ShowSettingKey.DownloadQuality).Select(c => c.Label));
        Assert.Equal("Off", ShowSettingsCatalog.Choices(ShowSettingKey.KeepNext)[0].Label);
    }

    [Fact]
    public void AShowsOwnValueStandsOnItsChoice()
    {
        Assert.Equal(-1, ShowSettingsCatalog.IndexOf(ShowSettingKey.Quality, null));
        Assert.Equal(2, ShowSettingsCatalog.IndexOf(ShowSettingKey.Quality, new SettingValue.Int(1080)));
        Assert.Equal(1, ShowSettingsCatalog.IndexOf(ShowSettingKey.SkipFiller, new SettingValue.Bool(false)));
        Assert.Equal(1, ShowSettingsCatalog.IndexOf(ShowSettingKey.Translation, new SettingValue.Str("dub")));
        Assert.Equal(-1, ShowSettingsCatalog.IndexOf(ShowSettingKey.Quality, new SettingValue.Int(360)));
    }

    [Fact]
    public void ARowSaysItsValueInTheSheetsWords()
    {
        Assert.Equal("English", ShowSettingsCatalog.Shown(ShowSettingKey.Translation, "dub"));
        Assert.Equal("On", ShowSettingsCatalog.Shown(ShowSettingKey.SkipOpening, "on"));
        Assert.Equal("720p", ShowSettingsCatalog.Shown(ShowSettingKey.Quality, "720p"));
        Assert.Equal("Standard", ShowSettingsCatalog.Shown(ShowSettingKey.DownloadQuality, "standard"));
        Assert.Equal(SettingKey.KeepNextEpisodes, ShowSettingsCatalog.Setting(ShowSettingKey.KeepNext));
    }
}

public class SearchTests
{
    [Fact]
    public void AFilterWithNothingTypedBrowses()
    {
        var filters = SearchFilters.None with { Genres = ["Drama"], Formats = [MediaFormat.Movie], Year = 2019 };
        var query = Searches.With("", filters);
        Assert.Equal("", query.Text);
        Assert.Equal(["Drama"], query.Genres);
        Assert.Equal([MediaFormat.Movie], query.Formats);
        Assert.Equal(new Bounds(2019, 2019), query.Years);
        Assert.True(filters.Narrows);
        Assert.False(SearchFilters.None.Narrows);
        Assert.True((SearchFilters.None with { Sort = SearchSort.Score }) is { Narrows: false, IsEmpty: false });
    }

    [Fact]
    public void AQueryIsTheSameQueryWhateverArraysCarryIt()
    {
        var filters = SearchFilters.None with { Genres = ["Drama", "Mystery"] };
        var a = Searches.With("lamp", filters);
        var b = Searches.With("lamp", filters with { Genres = ["Drama", "Mystery"] });
        Assert.NotEqual(a, b);
        Assert.True(Searches.Same(a, b));
        Assert.False(Searches.Same(a, Searches.With("lamp", SearchFilters.None)));
        Assert.True(Searches.Same(Searches.Text("x"), new SearchQuery(Text: "x", Years: new Bounds(), Episodes: new Bounds(), Minutes: new Bounds())));

        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Search(a), new ContentList.Empty()));
        Assert.IsType<ContentList.Empty>(known.List(new ContentFor.Search(b)));
        known.Take(new Event.Content(new ContentFor.Search(b), new ContentList.Failed("down")));
        Assert.Single(known.Lists);
        Assert.IsType<ContentList.Failed>(known.List(new ContentFor.Search(a)));
    }

    [Fact]
    public void GenresKeepTheListsOrderAndADropDownSaysWhatItHas()
    {
        var order = Searches.Genres;
        var filters = SearchFilters.None.Toggle("Mystery", order).Toggle("Action", order);
        Assert.Equal(["Action", "Mystery"], filters.Genres);
        Assert.Equal(["Mystery"], filters.Toggle("Action", order).Genres);
        Assert.Equal([MediaFormat.Tv, MediaFormat.Movie], SearchFilters.None.Toggle(MediaFormat.Movie).Toggle(MediaFormat.Tv).Formats);
        Assert.Equal("Genre", SearchFilters.Say("Genre", []));
        Assert.Equal("Action, Mystery", SearchFilters.Say("Genre", ["Action", "Mystery"]));
        Assert.Equal("Action +2", SearchFilters.Say("Genre", ["Action", "Drama", "Mystery"]));
        Assert.Equal(2027u, Searches.Years(2026)[0]);
        Assert.Equal(1970u, Searches.Years(2026)[^1]);
    }
}

public class TitleSeasonsTests
{
    static LinkedTitle Linked(uint id, Season? season, ushort? year) => new(Id: id, Title: $"Lantern Road {id}", Season: season, Year: year);

    [Fact]
    public void ThePickerNamesEachSeasonInWatchOrderWithThisOnePicked()
    {
        var detail = new TitleDetail(Seasons: [Linked(910001, Season.Spring, 2021), Linked(910002, null, 2022), Linked(910003, null, null)]);
        var entry = new Entry(Id: 910002, Title: "Lantern Road", Detail: detail,
            Related: [new RelatedTitle(910001, "Lantern Road", "prequel", "TV"), new RelatedTitle(910006, "Lantern Road: the film", "side_story", "MOVIE")]);
        var seasons = TitleSeasons.Of(entry);
        Assert.Equal(["Season 1 · Spring 2021", "Season 2 · 2022", "Season 3"], seasons.Select(s => s.Label));
        Assert.Equal([false, true, false], seasons.Select(s => s.IsThis));
        Assert.Equal([910006u], TitleSeasons.Related(entry).Select(r => r.Id));
    }

    [Fact]
    public void OneSeasonOrAWalkUnderWayIsNoPicker()
    {
        Assert.Empty(TitleSeasons.Of(new Entry(Id: 1, Title: "A")));
        Assert.Empty(TitleSeasons.Of(new Entry(Id: 1, Title: "A", Detail: new TitleDetail(Seasons: [Linked(1, null, 2020)]))));
    }
}
