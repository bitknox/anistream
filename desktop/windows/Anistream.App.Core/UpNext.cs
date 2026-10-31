using System.Globalization;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>A suggestion's second line: the reason, with the part that is the viewer's own title last so it alone is cut short.</summary>
public readonly record struct UpNextLine(string Lead, string Tail)
{
    public override string ToString() => Lead + Tail;
}

/// <summary>
/// "Up next for you": the words around what the engine sends. The engine decides the titles, their
/// order and their count; this only words a row and its notice.
/// </summary>
public static class UpNextWording
{
    /// <summary>"9 January", "January 2027" or "2027": as much of the date as AniList knows.</summary>
    public static string Date(FuzzyDate date, DateTimeOffset now, CultureInfo? culture = null)
    {
        culture ??= CultureInfo.CurrentCulture;
        if (date.Month is not { } month)
            return date.Year.ToString(culture);
        var first = new DateTime((int)date.Year, month, date.Day ?? 1);
        if (date.Day is not { } day)
            return first.ToString("MMMM yyyy", culture);
        return first.ToString(date.Year == now.Year ? "d MMMM" : "d MMMM yyyy", culture);
    }

    /// <summary>The reason alone: "New season", "Because you finished Lantern Street".</summary>
    static UpNextLine Reason(UpNextItem item)
    {
        switch (item.Reason)
        {
            case UpNextReason.NewSeason:
                return new(T.UpNextReasonNewSeason, "");
            case UpNextReason.Planning:
                return new(T.UpNextReasonPlanning, "");
            case UpNextReason.Recommended when item.Because is { Length: > 0 } because:
                var whole = T.UpNextReasonRecommended(because);
                var at = whole.LastIndexOf(because, StringComparison.Ordinal);
                return at >= 0 ? new(whole[..at], whole[at..]) : new(whole, "");
            default:
                return new(T.UpNextReasonRecommendedPlain, "");
        }
    }

    /// <summary>
    /// A row's second line. A title not yet airing says when it starts after the reason.
    /// </summary>
    public static UpNextLine Line(UpNextItem item, DateTimeOffset now, CultureInfo? culture = null)
    {
        var reason = Reason(item);
        if (item.Starts is not { } starts)
            return reason;
        var when = T.UpNextStarts(Date(starts, now, culture));
        // "Because you finished Title · Starts 9 January": the title stays the one cut short.
        return reason.Tail.Length > 0
            ? new(reason.Lead, reason.Tail + " · " + when)
            : new(T.UpNextRowStarts(reason.Lead, when), "");
    }

    /// <summary>What Not interested says it did: a title that has not aired is hidden until it does.</summary>
    public static string HiddenNotice(UpNextItem item) =>
        item.Starts is not null ? T.UpNextHiddenUntilAirs(item.Entry.Title) : T.UpNextHidden(item.Entry.Title);

    /// <summary>The main button's words, or null when the row has none.</summary>
    public static string? Button(UpNextItem item) => item.Action switch
    {
        UpNextAction.PlayFirstEpisode => T.TitleButtonPlay(1),
        UpNextAction.AddToPlanning => T.UpNextAddPlanning,
        _ => null,
    };
}
