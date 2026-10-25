using Anistream.Engine.Core;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// Lines the apps agreed on ahead of the model's own rule for them, so Windows reads as the others
/// do now.
/// </summary>
public static class Lines
{
    /// <summary>
    /// An episode as the player names it, its Dub mark after the label: "Episode 10 · Dub ·
    /// The Return". A sub says nothing, since that is what most episodes play.
    /// </summary>
    public static string Episode(string label, string? title, Translation translation)
    {
        var line = translation == Translation.Dub ? $"{label} · Dub" : label;
        return title is { Length: > 0 } t ? $"{line} · {t}" : line;
    }

    /// <summary>
    /// How long a code has left: "Expires in 9 min", counting whole minutes up so the last one
    /// reads "1 min", then "Expires in 45 sec" in the last minute, and "Code expired" at nought.
    /// </summary>
    public static string Expiry(long secondsLeft) => secondsLeft switch
    {
        <= 0 => "Code expired",
        < 60 => $"Expires in {secondsLeft} sec",
        _ => $"Expires in {(secondsLeft + 59) / 60} min",
    };

    /// <summary>
    /// The engine's word that a setting was written, "saved · quality". The row already shows
    /// the new value, so the apps leave it unsaid; a refusal ("not saved: …") still shows.
    /// </summary>
    public static bool SettingSaved(Notice notice) =>
        notice.Kind == NoticeKind.Info && notice.Text.StartsWith("saved · ", StringComparison.Ordinal);
}
