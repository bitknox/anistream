// The engine driven from C# through the generated bindings: started on the test kit's fake
// AniList, sent the model's own commands, and read with NextEvents() for the model's own events
// until the job a command started has ended. The C# twin of the Apple package's EngineTests.

using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.Engine.Tests;

/// <summary>A host that opens nothing, shows nothing and keeps no secrets.</summary>
class QuietHost : FfiHost
{
    public bool OpenUrl(string url) => false;
    public void Notify(string title, string body) { }
    public string? SecretGet(string id) => null;
    public bool SecretSet(string id, string value) => false;
    public bool SecretDelete(string id) => true;
}

/// <summary>A host that writes down what the engine asks of it, from whichever thread asks.</summary>
class RecordingHost : FfiHost
{
    readonly object gate = new();
    readonly List<string> calls = [];
    readonly Dictionary<string, string> secrets = [];

    public string[] Calls
    {
        get { lock (gate) return [.. calls]; }
    }

    void Record(string call)
    {
        lock (gate) calls.Add(call);
    }

    public bool OpenUrl(string url)
    {
        Record($"open {url}");
        return false;
    }

    public void Notify(string title, string body) => Record($"notify {title}");

    public string? SecretGet(string id)
    {
        Record($"get {id}");
        lock (gate) return secrets.GetValueOrDefault(id);
    }

    public bool SecretSet(string id, string value)
    {
        Record($"set {id}");
        lock (gate) secrets[id] = value;
        return true;
    }

    public bool SecretDelete(string id)
    {
        Record($"delete {id}");
        lock (gate) return secrets.Remove(id);
    }
}

/// <summary>The engine started for a test in a new temporary directory, removed with it.</summary>
sealed class Started : IDisposable
{
    public FfiEngine Engine { get; }
    public string Root { get; }

    Started(FfiEngine engine, string root)
    {
        Engine = engine;
        Root = root;
    }

    public static FfiDirs Dirs(string root) =>
        new(
            Config: Path.Combine(root, "config"),
            Data: Path.Combine(root, "data"),
            Cache: Path.Combine(root, "cache")
        );

    public static string NewRoot()
    {
        HomeGuard.Check();
        return Path.Combine(Path.GetTempPath(), $"anistream-cs-{Guid.NewGuid():N}");
    }

    public static Started Testing(FfiHost? host = null, FfiStart? start = null)
    {
        var root = NewRoot();
        return new(FfiEngine.StartTesting(Dirs(root), host ?? new QuietHost(), start), root);
    }

    public void Dispose()
    {
        Engine.Shutdown();
        Engine.Dispose();
        try
        {
            Directory.Delete(Root, recursive: true);
        }
        catch (DirectoryNotFoundException) { }
    }

    /// <summary>Every event until the one that ends <paramref name="job"/>, that one included.</summary>
    public async Task<List<Event>> EventsUntil(ulong job)
    {
        var read = new List<Event>();
        while (true)
        {
            var batch = await Engine.NextEvents();
            Assert.True(batch.Length > 0, $"the engine stopped before the job ended: {Describe(read)}");
            read.AddRange(batch);
            if (batch.Any(e => e is Event.JobDone done && done.V1 == job))
                return read;
        }
    }

    /// <summary>Events until one matches, that one included.</summary>
    public async Task<List<Event>> EventsUntil(Func<Event, bool> match)
    {
        var read = new List<Event>();
        while (true)
        {
            var batch = await Engine.NextEvents();
            Assert.True(batch.Length > 0, $"the engine stopped first: {Describe(read)}");
            read.AddRange(batch);
            if (batch.Any(match))
                return read;
        }
    }

    public static string Describe(IEnumerable<Event> events) =>
        string.Join(", ", events.Select(e => e.GetType().Name));
}

/// <summary>
/// Tests never see the real HOME: scripts/test.sh runs them under a temporary one, and says
/// which in ANISTREAM_TEST_HOME. Run any other way, they refuse to start an engine.
/// </summary>
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

public class EngineTests
{
    const int Minute = 60_000;

    [Fact(Timeout = Minute)]
    public async Task ASearchComesBackWithTheFixtureTitles()
    {
        // Started as the Windows app will start, with the fields it names and the rest defaulted.
        using var started = Started.Testing(start: new FfiStart(Platform: FfiPlatform.MacOs, Secrets: SecretsMode.Host));

        var job = started.Engine.Send(new Command.Search("hoshikuzu", new SearchFilters()));
        Assert.NotNull(job);
        var read = await started.EventsUntil(job.Value);

        var found = read.OfType<Event.Content>()
            .Where(c => c.View is ContentFor.Search { Query: "hoshikuzu" })
            .Select(c => c.ContentValue)
            .OfType<ContentList.Entries>()
            .Select(e => e.V1)
            .FirstOrDefault();
        Assert.NotNull(found);
        Assert.Equal([900_001u, 900_004u], found.Select(e => e.Id));
        Assert.Equal(["Stardust Wayfarer", "Stardust Wayfarer Season 2"], found.Select(e => e.Title));
        Assert.Equal(
            ["Hoshikuzu no Tabibito", "Hoshikuzu no Tabibito 2nd Season"],
            found.Select(e => e.Secondary)
        );
        // What launch said came first, the settings ahead of everything.
        Assert.IsType<Event.Settings>(read[0]);
    }

    [Fact(Timeout = Minute)]
    public async Task ContinueIsEmptyOnANewDatabase()
    {
        using var started = Started.Testing();

        var job = started.Engine.Send(new Command.LoadContinue());
        Assert.NotNull(job);
        var read = await started.EventsUntil(job.Value);
        // Nothing has been watched yet, and the list says so rather than failing.
        Assert.Contains(read, e => e == new Event.Content(new ContentFor.Continue(), new ContentList.Empty()));
    }

    [Fact(Timeout = Minute)]
    public async Task ATitleComesBackWholeAfterTheSourcePinnedForIt()
    {
        using var started = Started.Testing();

        var job = started.Engine.Send(new Command.LoadDetail(900_001));
        Assert.NotNull(job);
        var read = await started.EventsUntil(job.Value);

        var pinned = read.FindIndex(e => e == new Event.ProviderPreference(null));
        var page = read.FindIndex(e => e is Event.Detail);
        Assert.True(pinned >= 0, Started.Describe(read));
        Assert.True(page >= 0, Started.Describe(read));
        // Said ahead of the page, so the page never shows another title's pin.
        Assert.True(pinned < page);
        var entry = ((Event.Detail)read[page]).V1;
        Assert.Equal(900_001u, entry.Id);
        Assert.Equal("Stardust Wayfarer", entry.Title);
        Assert.NotEmpty(entry.Related);
    }

    [Fact(Timeout = Minute)]
    public async Task AFixturePlaysToItsEndInAHostedPlayer()
    {
        using var started = Started.Testing();
        var engine = started.Engine;

        // The finale, so the playback ends with the episode rather than rolling on.
        engine.Send(new Command.Play(900_001, "12"));
        var opened = false;
        ulong? loaded = null;
        Ending? ending = null;
        while (ending == null)
        {
            var batch = await engine.NextEvents();
            Assert.True(batch.Length > 0, "the engine stopped before the playback ended");
            foreach (var e in batch)
            {
                switch (e)
                {
                    case Event.PlayerOps ops:
                        foreach (var op in ops.Ops)
                        {
                            switch (op)
                            {
                                case PlayerOp.Open:
                                    opened = true;
                                    break;
                                case PlayerOp.Load load when loaded == null:
                                    Assert.StartsWith("av://lavfi:", load.Url);
                                    var playback = ops.PlaybackValue;
                                    loaded = playback;
                                    // As libmpv reports it: the file, its length, a playhead, the end.
                                    void Report(PlayerReport report) =>
                                        engine.Send(new Command.PlayerReport(playback, report));
                                    Report(new PlayerReport.FileLoaded());
                                    Report(new PlayerReport.Property("duration", "30"));
                                    for (var second = 0; second < 30; second++)
                                        Report(new PlayerReport.Property("time-pos", $"{second}"));
                                    Report(new PlayerReport.Ended(PlayerEnd.Eof));
                                    break;
                            }
                        }
                        break;
                    case Event.PlaybackEnded ended:
                        ending = ended.Ending;
                        break;
                }
            }
        }
        Assert.True(opened);
        Assert.NotNull(loaded);
        Assert.Equal(new Ending.Complete(null), ending);
    }

    [Fact(Timeout = Minute)]
    public async Task ShutdownEndsTheStream()
    {
        using var started = Started.Testing();
        var engine = started.Engine;

        engine.Shutdown();
        // What launch said is still there to read, and then the stream ends.
        var batches = 0;
        while ((await engine.NextEvents()).Length > 0)
        {
            batches++;
            Assert.True(batches < 100, "the stream ends once the engine has stopped");
        }
        Assert.Null(engine.Send(new Command.LoadContinue()));
    }

    [Fact(Timeout = Minute)]
    public async Task TheEngineAsksTheHostToOpenALinkFromItsOwnThread()
    {
        var host = new RecordingHost();
        using var started = Started.Testing(host);

        // No job: the engine asks the host at once, on one of its own threads, and says so when
        // the host would not.
        Assert.Null(started.Engine.Send(new Command.OpenExternal("https://anistream.example/help")));
        var read = await started.EventsUntil(e => e is Event.Toast);

        Assert.Contains("open https://anistream.example/help", host.Calls);
        var toast = read.OfType<Event.Toast>().Single();
        Assert.Equal(NoticeKind.Alert, toast.V1.Kind);
        Assert.Equal("could not open: the app did not open it", toast.V1.Text);
    }

    [Fact(Timeout = Minute)]
    public async Task ManyCommandsFromManyThreadsAllEnd()
    {
        using var started = Started.Testing();
        var engine = started.Engine;

        // Send is safe from any thread, as the app's UI thread and its background tasks both send.
        var jobs = await Task.WhenAll(
            Enumerable.Range(0, 8).Select(_ => Task.Run(() => engine.Send(new Command.LoadContinue())))
        );
        var pending = jobs.Select(j => j!.Value).ToHashSet();
        Assert.Equal(8, pending.Count);
        while (pending.Count > 0)
        {
            var batch = await engine.NextEvents();
            Assert.True(batch.Length > 0, "the engine stopped first");
            foreach (var done in batch.OfType<Event.JobDone>())
                pending.Remove(done.V1);
        }
    }

    [Fact(Timeout = Minute)]
    public void AConfigTheEngineRefusesIsAnErrorWithWords()
    {
        var root = Started.NewRoot();
        try
        {
            var dirs = Started.Dirs(root);
            Directory.CreateDirectory(dirs.Config);
            File.WriteAllText(Path.Combine(dirs.Config, "config.toml"), "this is = = not toml");

            var error = Assert.Throws<FfiException.Config>(
                () => FfiEngine.Start(dirs, new QuietHost(), new FfiBackground())
            );
            Assert.Contains("config.toml", error.reason);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    [Fact]
    public void TheAppsLogIsWrittenThroughTheFreeFunctions()
    {
        var root = Started.NewRoot();
        try
        {
            Directory.CreateDirectory(root);
            // The log is process-wide: the first start wins, and a later one says it did not.
            AnistreamFfiMethods.StartLog(root);
            AnistreamFfiMethods.LogLine(FfiLogLevel.Info, "tests", "a line from C#");
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }
}
