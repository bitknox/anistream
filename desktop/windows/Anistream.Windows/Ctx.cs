using Anistream.App;
using Anistream.Engine.Model;

namespace Anistream.Windows;

/// <summary>
/// What every page reaches: the engine, what it has said, the art, and the window's ways to move
/// between pages and to play. One per window; pages find it as <see cref="Current"/>.
/// </summary>
sealed class Ctx
{
    public static Ctx Current { get; set; } = null!;

    public required EngineSession Session { get; init; }
    public required Art Art { get; init; }
    public required MainWindow Window { get; init; }
    public required Options Options { get; init; }

    public Known Known => Session.Known;

    /// <summary>Every batch of the engine's events, on the UI thread, after Known has them.</summary>
    public event Action<IReadOnlyList<Event>>? Batch;

    public void Heard(IReadOnlyList<Event> batch) => Batch?.Invoke(batch);

    public ulong? Send(Command command)
    {
#if ANISTREAM_TESTING
        // A showcase has no store to ask: the invite preview is the one it invented.
        if (Options.Showcase && command is Command.PreviewStore preview)
        {
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                var events = new[] { Showcase.Preview(preview.Link) };
                Session.Known.Take(events[0]);
                Heard(events);
            });
            return null;
        }
        if (Options.Showcase && command is Command.JoinStore)
        {
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                var events = new Event[] { new Event.StoreJoined("hotaru") };
                Session.Known.Take(events[0]);
                Heard(events);
            });
            return null;
        }
#endif
        return Session.Send(command);
    }

    public void OpenTitle(uint id) => Window.OpenTitle(id);

    /// <summary>Plays an episode from where it was left, or from its start with <paramref name="fromStart"/>.</summary>
    public void Play(uint id, string episode, bool fromStart = false) => Send(new Command.Play(id, episode, fromStart));

    public void Search(string query) => Window.ShowSearch(query);

    /// <summary>Pauses every download that runs or waits; each resumes from its own row.</summary>
    public void PauseAllDownloads()
    {
        foreach (var row in Known.Downloads.Where(r => r.State is DownloadState.Active or DownloadState.Queued))
            Send(new Command.DownloadPause(row.Id));
    }
}
