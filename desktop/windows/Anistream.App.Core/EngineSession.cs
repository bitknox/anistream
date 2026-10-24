using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// The engine started, and the loop that reads its events. <see cref="FfiEngine.NextEvents"/>
/// waits on a pool thread; each batch goes to <see cref="Batch"/> on the UI thread through the
/// <c>post</c> the app gives, except what must not wait on the UI: the player's ops, which go to
/// <see cref="OffThread"/> at once, in the engine's order.
/// </summary>
public sealed class EngineSession : IDisposable
{
    readonly Action<Action> post;
    Task? loop;
    bool disposed;

    public FfiEngine Engine { get; }
    public Known Known { get; } = new();

    /// <summary>A batch of events, in order, on the UI thread, after <see cref="Known"/> has them.</summary>
    public event Action<IReadOnlyList<Event>>? Batch;

    /// <summary>The engine stopped: it has nothing more to say.</summary>
    public event Action? Stopped;

    /// <summary>
    /// Called on the loop's thread for every event before the UI sees it; true keeps it from the
    /// UI. The player's ops are taken here.
    /// </summary>
    public Func<Event, bool>? OffThread { get; set; }

    public EngineSession(FfiEngine engine, Action<Action> post)
    {
        Engine = engine;
        this.post = post;
    }

    /// <summary>The engine as the app ships it, on the user's own folders.</summary>
    public static FfiEngine StartUser(FfiHost host, FfiStart start) =>
        FfiEngine.Start(AppDirs.User(), host, new FfiBackground(), start);

    /// <summary>What the app tells the engine about itself: its palette and that it is Windows.</summary>
    public static FfiStart AppStart() => new(Theme: Palette.PlayerTheme, Platform: FfiPlatform.Windows);

    public void Start()
    {
        loop ??= Task.Run(Pump);
    }

    async Task Pump()
    {
        while (true)
        {
            Event[] events;
            try
            {
                events = await Engine.NextEvents();
            }
            catch (Exception e)
            {
                AnistreamFfiMethods.LogLine(FfiLogLevel.Error, "windows", $"reading the engine's events failed: {e.Message}");
                break;
            }
            if (events.Length == 0)
                break;
            var forUi = new List<Event>(events.Length);
            foreach (var e in events)
            {
                if (OffThread?.Invoke(e) == true)
                    continue;
                forUi.Add(e);
            }
            if (forUi.Count > 0)
                post(() =>
                {
                    foreach (var e in forUi)
                        Known.Take(e);
                    Batch?.Invoke(forUi);
                });
        }
        post(() => Stopped?.Invoke());
    }

    /// <summary>Ask the engine for <paramref name="command"/>. Never waits.</summary>
    public ulong? Send(Command command) => disposed ? null : Engine.Send(command);

    public void Dispose()
    {
        if (disposed)
            return;
        disposed = true;
        Engine.Shutdown();
        try
        {
            loop?.Wait(TimeSpan.FromSeconds(5));
        }
        catch (AggregateException) { }
        Engine.Dispose();
    }
}
