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
    public required Updater Updater { get; init; }
    /// <summary>Play on TV: the TVs, pairing, and the card of the one driven.</summary>
    public required TvRemote Tv { get; init; }

    public Known Known => Session.Known;

    /// <summary>Every batch of the engine's events, on the UI thread, after Known has them.</summary>
    public event Action<IReadOnlyList<Event>>? Batch;

    /// <summary>Protections checked before they are saved: the proxy dialog's checks.</summary>
    public ProtectionTrials Trials => trials ??= new(command => Send(command));
    ProtectionTrials? trials;

    public void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
            if (e is Event.ProtectionChecked checkedNow)
                Trials.Heard(checkedNow.V1);
        Batch?.Invoke(batch);
    }

    public ulong? Send(Command command)
    {
#if ANISTREAM_TESTING
        // The source picker's shots: the listing is the one the showcase invented, or none at all.
        if (command is Command.LoadSourceOptions listing && Options.Show?.StartsWith("source-picker", StringComparison.Ordinal) == true)
        {
            if (Options.Show != "source-picker-loading")
                Window.DispatcherQueue.TryEnqueue(() =>
                {
                    var events = new Event[] { new Event.SourceOptions(listing.V1, Showcase.PickerOptions(Options.Show)) };
                    Session.Known.Take(events[0]);
                    Heard(events);
                });
            return null;
        }
        // A discussion's shots: the fixture engine has no AniList forum, so the pane is answered with the invented thread.
        if (command is Command.OpenDiscussion opening && Discussion.DiscussionShowcase.Is(Options.Show))
        {
            Window.DispatcherQueue.TryEnqueue(() =>
            {
                var events = new Event[] { new Event.Discussion(Discussion.DiscussionShowcase.View(Options.Show!, opening.Media, opening.Episode)) };
                Session.Known.Take(events[0]);
                Heard(events);
            });
            return null;
        }
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
        // A proxy dialog's check held while it checks, or answered as failed, for a screenshot:
        // the fixture's own check always passes.
        if (command is Command.CheckTorrentProtection && Options.Show is "torrent-edit-checking" or "torrent-edit-failed")
        {
            if (Options.Show == "torrent-edit-failed")
                _ = Task.Delay(1200).ContinueWith(_ => Window.DispatcherQueue.TryEnqueue(() => Heard([new Event.ProtectionChecked(new GuardView.Leaking())])));
            return null;
        }
#endif
        return Session.Send(command);
    }

    public void OpenTitle(uint id) => Window.OpenTitle(id);

    /// <summary>One of the window's own pages, by its tag: "downloads".</summary>
    public void Go(string tag) => Window.Go(tag);

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
