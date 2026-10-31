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

    /// <summary>
    /// How the app starts the engine: its palette, its platform, and the clients the apps ship
    /// with. AniList's implicit grant and MyAnimeList's PKCE redirect to
    /// <c>anistream://auth/&lt;tracker&gt;</c>, which the installer registers for this app; Simkl
    /// and Trakt sign in with a code typed on their site. These ids are public, as every app's
    /// are. A fixture run passes <paramref name="signIn"/> false: its trackers are the fake's,
    /// and a sign-in must not open the real AniList.
    /// </summary>
    public static FfiStart AppStart(bool signIn = true) => new(
        Theme: Palette.PlayerTheme,
        Platform: FfiPlatform.Windows,
        AnilistClientId: signIn ? AnilistClient : null,
        MalClientId: signIn ? MalClient : null,
        SimklClientId: SimklClient,
        TraktClientId: TraktClient);

    /// <summary>anistream's AniList client: implicit grant, redirecting to <c>anistream://auth/anilist</c>.</summary>
    public const string AnilistClient = "54537";

    /// <summary>anistream's MyAnimeList client: PKCE, redirecting to <c>anistream://auth/mal</c>.</summary>
    public const string MalClient = "0ccc4725381aca9513f26d32107a94cd";

    /// <summary>The Simkl and Trakt clients registered for the apps, as the Apple apps ship them.</summary>
    public const string SimklClient = "016a3721d77c54d04f5aa6f5a213fe76f047d83d3ae35b2c892074859b88be0b";
    public const string TraktClient = "p2yWSSGe7aVAlnHEMiQQh0RhiL6ivZfJNaubOJi4DZE";

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
                // One event the app can't take is logged and skipped; the next still arrive.
                try
                {
                    if (OffThread?.Invoke(e) == true)
                        continue;
                }
                catch (Exception error)
                {
                    Failed("the player", e, error);
                    continue;
                }
                forUi.Add(e);
            }
            if (forUi.Count > 0)
                post(() =>
                {
                    foreach (var e in forUi)
                    {
                        try
                        {
                            Known.Take(e);
                        }
                        catch (Exception error)
                        {
                            Failed("the app", e, error);
                        }
                    }
                    Batch?.Invoke(forUi);
                });
        }
        post(() => Stopped?.Invoke());
    }

    /// <summary>Logs an event that something could not take, by the event's kind.</summary>
    public static void Failed(string who, Event e, Exception error) =>
        AnistreamFfiMethods.LogLine(FfiLogLevel.Error, "windows", $"{who} could not take {e.GetType().Name}: {error}");

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
