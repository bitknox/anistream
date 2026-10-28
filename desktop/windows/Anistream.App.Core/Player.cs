using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// "Still watching?": episodes that roll on by themselves with nobody there. The engine says how
/// many in a row to allow (<c>StillWatchingAfter</c>, with Up next); the player counts the ones
/// that rolled on and starts over at any input from the viewer, and asks once the next episode
/// has its first picture. Zero never asks.
/// </summary>
public sealed class StillWatching
{
    bool rollPending;

    /// <summary>Episodes in a row that rolled on without the viewer.</summary>
    public int RolledOn { get; private set; }

    /// <summary>After how many the player asks, as the engine last said; zero is never.</summary>
    public int After { get; set; } = 3;

    public bool Asking { get; private set; }

    /// <summary>The next episode is being started by the countdown, not by the viewer.</summary>
    public void RollingOn() => rollPending = true;

    /// <summary>A new file is starting: if it was a roll, it counts.</summary>
    public void Started()
    {
        if (rollPending)
        {
            rollPending = false;
            RolledOn++;
        }
    }

    /// <summary>The viewer did something: no longer nobody there.</summary>
    public void Input()
    {
        RolledOn = 0;
    }

    /// <summary>
    /// The new episode has its first picture. True when the player should pause and ask now.
    /// </summary>
    public bool FirstPicture()
    {
        if (After > 0 && RolledOn >= After && !Asking)
        {
            Asking = true;
            return true;
        }
        return false;
    }

    /// <summary>The viewer answered, or the player closed.</summary>
    public void Answered() => Asking = false;

    public void Reset()
    {
        rollPending = false;
        RolledOn = 0;
        Asking = false;
    }
}

/// <summary>
/// What the seek bar's previews read: the stream, or the engine's <c>Thumbnails</c> address when it
/// differs, with only the options that reach the stream (headers and the like), never the
/// playback's own.
/// </summary>
public sealed record PreviewSource(string Url, IReadOnlyList<MpvOption> Options)
{
    static readonly string[] Reaching = ["header", "user-agent", "referrer", "cookie", "-lavf-o", "tls-"];

    public static IReadOnlyList<MpvOption> Reach(IEnumerable<MpvOption> options) =>
        [.. options.Where(o => Reaching.Any(r => o.Name.Contains(r, StringComparison.Ordinal)))];

    /// <summary>Where along a bar of <paramref name="width"/> a pointer at <paramref name="x"/> falls, in seconds.</summary>
    public static double TimeAt(double x, double width, double duration) =>
        width <= 0 ? 0 : Math.Clamp(x / width, 0, 1) * duration;
}
