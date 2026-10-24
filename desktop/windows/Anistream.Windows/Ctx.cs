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

    public ulong? Send(Command command) => Session.Send(command);

    public void OpenTitle(uint id) => Window.OpenTitle(id);

    public void Play(uint id, string episode) => Send(new Command.Play(id, episode));

    public void Search(string query) => Window.ShowSearch(query);
}
