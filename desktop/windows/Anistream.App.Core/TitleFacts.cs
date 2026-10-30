using System.Globalization;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>One row of the title page's facts: a label and its value.</summary>
public sealed record TitleFact(string Label, string Value);

/// <summary>
/// The title page's facts, worded from the engine's decisions. The engine decides which fact
/// a title's run is (<see cref="AiringFact"/>); the catalogue words it.
/// </summary>
public static class TitleFacts
{
    static readonly CultureInfo Culture = CultureInfo.CurrentCulture;

    /// <summary>A fuzzy date as far as it is known: "9 January 2027", "January 2027" or "2027".</summary>
    public static string Date(FuzzyDate date)
    {
        if (date.Month is not (>= 1 and <= 12 and var month))
            return date.Year.ToString(CultureInfo.InvariantCulture);
        var first = new DateTime(date.Year, month, 1);
        return date.Day is { } day and >= 1
            ? first.AddDays(day - 1).ToString("d MMMM yyyy", Culture)
            : first.ToString("MMMM yyyy", Culture);
    }

    /// <summary>The run's fact: "Aired" and its dates, "On hiatus" and since when, and the rest.</summary>
    public static TitleFact? Airing(AiringFact? fact) => fact switch
    {
        AiringFact.Airing a => new(T.MediaStatusAiring, a.Since is { } s ? T.MediaAiringSince(Date(s)) : T.TitleFactNow),
        AiringFact.Finished { From: { } from, To: { } to } => new(T.MediaStatusAired, T.TitleFactRange(Date(from), Date(to))),
        AiringFact.Finished { From: { } on } => new(T.TitleFactReleased, Date(on)),
        AiringFact.Finished { To: { } on } => new(T.TitleFactReleased, Date(on)),
        AiringFact.Finished => new(T.MediaStatusAired, ""),
        AiringFact.NotYetReleased n => new(T.TitleFactStarts, n.From is { } f ? Date(f) : T.TitleFactTba),
        AiringFact.Hiatus h => new(T.MediaStatusHiatus, h.Since is { } s ? T.MediaAiringSince(Date(s)) : ""),
        AiringFact.Cancelled c => new(T.MediaStatusCancelled, c.AfterEpisode is { } e ? T.TitleFactAfter(e) : ""),
        _ => null,
    };

    /// <summary>A film's length: "1 hr 52 min", "2 hr" or "46 min".</summary>
    public static string FilmLength(RunLength length) => (length.Hours, length.Minutes) switch
    {
        (0, var m) => T.FilmLengthMinutes(m),
        (var h, 0) => T.FilmLengthHours(h),
        var (h, m) => T.FilmLengthHoursMinutes(h, m),
    };

    /// <summary>Where it was made, only when that is not Japan.</summary>
    public static string? Country(Entry entry)
    {
        if (!entry.MadeOutsideJapan)
            return null;
        return entry.Country?.ToUpperInvariant() switch
        {
            "CN" => T.CountryChina,
            "KR" => T.CountrySouthKorea,
            "TW" => T.CountryTaiwan,
            _ => T.MediaMadeElsewhere,
        };
    }

    /// <summary>A ranking's line: "#4 Highest rated, Spring 2023".</summary>
    public static string Ranking(Ranking ranking)
    {
        var scope = ranking.AllTime ? T.RankingAllTime
            : ranking.Season is { } season && ranking.Year is { } year ? Wording.SeasonWithYear(season, year)
            : ranking.Year is { } y ? y.ToString(CultureInfo.InvariantCulture)
            : ranking.Context;
        var label = ranking.Kind == RankingKind.Rated ? T.RankingHighestRated(scope) : T.RankingMostPopular(scope);
        return T.RankingLine(ranking.Rank, label);
    }

    /// <summary>A person's status and score on the show: "Completed · 9/10", or the status alone.</summary>
    public static string Follow(FollowedEntry follow, Func<LibrarySegment, string> status)
    {
        var name = follow.Status is { } s ? status(s) : "";
        return follow.Score is { } score and > 0 && name.Length > 0 ? T.MoreFollowingLine(name, (long)Math.Round(score / 10.0)) : name;
    }
}
