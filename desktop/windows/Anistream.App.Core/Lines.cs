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
    /// An episode as the player and Continue's preview name it, its Dub mark after the label:
    /// "Episode 10 · Dub · The Return". A sub, or a translation not known, says nothing, since
    /// that is what most episodes play. The mark is the engine's <c>episode_line</c>.
    /// </summary>
    public static string Episode(string label, string? title, Translation? translation)
    {
        var line = AnistreamFfiMethods.EpisodeLine(label, translation);
        return title is { Length: > 0 } t ? $"{line} · {t}" : line;
    }

    /// <summary>
    /// The engine's word that a setting was written, "saved · quality". The row already shows
    /// the new value, so the apps leave it unsaid; a refusal ("not saved: …") still shows.
    /// </summary>
    public static bool SettingSaved(Notice notice) =>
        notice.Kind == NoticeKind.Info && notice.Msg is Msg.NoticeSaved;
}
