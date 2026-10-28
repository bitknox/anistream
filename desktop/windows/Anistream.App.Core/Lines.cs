using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// Lines the apps agreed on ahead of the model's own rule for them, so Windows reads as the others
/// do now.
/// </summary>
public static class Lines
{
    /// <summary>
    /// An episode as the player and Continue's preview name it, its Dubbed mark after the label:
    /// "Episode 10 · Dubbed · The Return". A sub, or a translation not known, says nothing, since
    /// that is what most episodes play.
    /// </summary>
    public static string Episode(string label, string? title, Translation? translation)
    {
        var line = Wording.EpisodeLine(label, translation);
        return title is { Length: > 0 } t ? T.PlayerEpisodeLineTitled(line, t) : line;
    }

    /// <summary>
    /// An episode by its number as the engine gives it, a string that is usually whole ("10") and
    /// now and then not ("12.5"): "Episode 10", or "Episode 10 · The Return" with a title.
    /// </summary>
    public static string EpisodeNamed(string number, string? title)
    {
        if (Whole(number) is { } n)
            return title is { Length: > 0 } t ? T.EpisodeWithTitle(n, t) : T.EpisodeNumber(n);
        var line = T.PlayerEpisodeFraction(number);
        return title is { Length: > 0 } named ? T.PlayerEpisodeLineTitled(line, named) : line;
    }

    /// <summary>The starting screen's line for <paramref name="number"/>: "Starting episode 10".</summary>
    public static string Starting(string number) =>
        Whole(number) is { } n ? T.PlayerStarting(n) : T.PlayerStartingFraction(number);

    /// <summary>The starting screen's line when the episode did not open: "Couldn't play episode 10".</summary>
    public static string StartFailed(string number) =>
        Whole(number) is { } n ? T.PlayerFailedTitle(n) : T.PlayerFailedFraction(number);

    static long? Whole(string number) =>
        long.TryParse(number, System.Globalization.NumberStyles.None, System.Globalization.CultureInfo.InvariantCulture, out var n) ? n : null;

    /// <summary>
    /// The engine's word that a setting was written, "saved · quality". The row already shows
    /// the new value, so the apps leave it unsaid; a refusal ("not saved: …") still shows.
    /// </summary>
    public static bool SettingSaved(Notice notice) =>
        notice.Kind == NoticeKind.Info && notice.Msg is Msg.NoticeSaved;
}
