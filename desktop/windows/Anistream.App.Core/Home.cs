using System.Globalization;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>How a section's items are drawn.</summary>
public enum HomeShape
{
    /// <summary>Wide 16:9 cards with a meter: Continue.</summary>
    Card,

    /// <summary>2:3 posters with a reason or an episode line.</summary>
    Poster,

    /// <summary>Seven day columns, today first: This week.</summary>
    Week,
}

/// <summary>A heading in two runs, so a title that is the viewer's own can be set bold: "Because you finished " and "Lantern Street".</summary>
public readonly record struct HomeHeading(string Lead, string Title = "")
{
    public override string ToString() => Lead + Title;
}

/// <summary>
/// Home's words and sizes. The engine decides every title, section, order and line; this words
/// what it leaves to the app (dates, times, the main action's label) and holds the numbers the
/// board draws at, so they can be tested.
/// </summary>
public static class HomeWording
{
    /// <summary>The cards fade in this many ms after a section arrives from the local store: the board's "about 90 ms".</summary>
    public const int LocalArrivalMs = 90;

    /// <summary>Discovery placeholders wait this long before showing, and stay at least <see cref="PlaceholderLeastMs"/>.</summary>
    public const int PlaceholderAfterMs = 600, PlaceholderLeastMs = 300, ActivityAfterMs = 4000;

    /// <summary>Undo in the Not interested notice holds this long.</summary>
    public const int NoticeMs = 8000;

    // Windows' numbers from the board: effective pixels in a 1280 window.
    public const double CardWidth = 280, CardHeight = 157.5, PosterWidth = 132, PosterHeight = 198, Gap = 16, ShelfGap = 40;

    /// <summary>The spotlight's art takes this share of the content width, 16:9, and never more than the board's 640.</summary>
    public const double ArtShare = 0.56, ArtMaxWidth = 640;

    public static HomeShape Shape(SectionKind kind) => kind switch
    {
        SectionKind.Continue => HomeShape.Card,
        SectionKind.ThisWeek => HomeShape.Week,
        _ => HomeShape.Poster,
    };

    /// <summary>The section's heading. The title of "Because you finished X" is the second run.</summary>
    public static HomeHeading Heading(HomeSection section)
    {
        var whole = section.Title.Text();
        if (section.Title is Msg.HomeBecause because && because.Title.Length > 0)
        {
            var at = whole.LastIndexOf(because.Title, StringComparison.Ordinal);
            if (at >= 0)
                return new(whole[..at], whole[at..]);
        }
        return new(whole);
    }

    /// <summary>The small words after a heading: "12 shows" on a cut Continue, "Autumn 2026" on This season.</summary>
    public static string? Aside(HomeSection section, SeasonYear? season) => section.Kind switch
    {
        SectionKind.Continue when section.Total > section.Items.Length => T.HomeContinueCount(section.Total),
        SectionKind.ThisSeason when season is { } s => Wording.SeasonWithYear(s),
        _ => null,
    };

    /// <summary>The link at a heading's end: View all, Full calendar, Seasonal or Search.</summary>
    public static string? ViewAll(HomeSection section) => section.ViewAll switch
    {
        null => null,
        HomeLink.Watching => section.Total > section.Items.Length ? T.HomeViewAll : null,
        HomeLink.CalendarView => T.HomeFullCalendar,
        HomeLink.SeasonalView => T.HomeSeasonal,
        HomeLink.Browse => section.Kind is SectionKind.UpNext ? T.HomeViewAll : T.HomeSearch,
        _ => null,
    };

    /// <summary>The spotlight's main action, or null when there is nothing to press.</summary>
    public static string? SpotlightAction(HomeAction action, uint episode, Playable playable)
    {
        if (playable == Playable.NotFound && action is HomeAction.Resume or HomeAction.Play)
            action = HomeAction.AddToPlanning;
        return action switch
        {
            HomeAction.Resume => T.HomeResumeEpisode(episode),
            HomeAction.Play => T.HomePlayEpisode(episode),
            HomeAction.AddToPlanning => T.UpNextAddPlanning,
            _ => null,
        };
    }

    /// <summary>What the main action does once playability is known: not found turns a play into Add to Planning.</summary>
    public static HomeAction Effective(HomeAction action, Playable playable) =>
        playable == Playable.NotFound && action is HomeAction.Resume or HomeAction.Play ? HomeAction.AddToPlanning : action;

    /// <summary>The facts under a title, in the board's order: format, episodes, season.</summary>
    public static string Facts(Entry entry)
    {
        var parts = new List<string>();
        if (entry.Format is { Length: > 0 } f)
            parts.Add(Wording.Format(f));
        if (entry.FilmLength is { } length)
            parts.Add(TitleFacts.FilmLength(length));
        else if (entry.Episodes is > 0 and var n)
            parts.Add(T.EpisodesCount(n));
        if (entry.Season is { } s && entry.Year is { } y)
            parts.Add(Wording.SeasonWithYear(s, y));
        else if (entry.Year is { } year)
            parts.Add(year.ToString(CultureInfo.InvariantCulture));
        return string.Join(" · ", parts);
    }

    /// <summary>The short reason under a poster: "New season", "On your Planning list", "Recommended for you".</summary>
    public static string Reason(UpNextReason reason) => reason switch
    {
        UpNextReason.NewSeason => T.UpNextReasonNewSeason,
        UpNextReason.Planning => T.UpNextReasonPlanning,
        UpNextReason.Recommended => T.UpNextReasonRecommendedPlain,
        _ => T.UpNextReasonAiringToday,
    };

    /// <summary>The full reason, for the accessibility label: "Because you finished Lantern Street" when the engine says which.</summary>
    public static string FullReason(UpNextReason reason, string? because) =>
        reason == UpNextReason.Recommended && because is { Length: > 0 } ? T.UpNextReasonRecommended(because) : Reason(reason);

    /// <summary>The line under a card or a poster. The engine's own words win; otherwise the date, the air time or the reason.</summary>
    public static string Line(HomeItem item, DateTimeOffset now, TimeZoneInfo zone, CultureInfo? culture = null)
    {
        if (item.Line is { } line)
            return line.Text();
        if (item.Starts is { } starts)
            return T.UpNextStarts(UpNextWording.Date(starts, now, culture));
        if (item.AirsAt is { } at)
            return item.Episode is { } ep ? T.UpNextBroadcast(ep, When(at, now, zone, culture)) : When(at, now, zone, culture);
        return item.Reason is { } reason ? Reason(reason) : "";
    }

    /// <summary>"Out 2 hours ago" for an episode that is out, or its clock time.</summary>
    public static string When(long at, DateTimeOffset now, TimeZoneInfo zone, CultureInfo? culture = null) =>
        at <= now.ToUnixTimeSeconds() ? T.UpNextOut(Wording.Ago(now.ToUnixTimeSeconds() - at)) : Calendar.Time(at, zone, culture);

    /// <summary>The day column's heading: "Today", then "Sunday 1".</summary>
    public static string Day(uint offset, DateTimeOffset today, CultureInfo? culture = null)
    {
        if (offset == 0)
            return T.HomeToday;
        var day = today.Date.AddDays(offset);
        return day.ToString("dddd d", culture ?? CultureInfo.CurrentCulture);
    }

    /// <summary>What a screen reader says for a card: title, line, and the full reason when there is one.</summary>
    public static string Name(HomeItem item, DateTimeOffset now, TimeZoneInfo zone, CultureInfo? culture = null)
    {
        var parts = new List<string> { item.Entry.Title };
        var line = item.Reason is { } reason && item.Line is null && item.Starts is null && item.AirsAt is null
            ? FullReason(reason, item.Because)
            : Line(item, now, zone, culture);
        if (line.Length > 0)
            parts.Add(line);
        if (item.Starts is not null && item.Reason is { } why)
            parts.Add(FullReason(why, item.Because));
        if (item.Playable == Playable.NotFound)
            parts.Add(T.HomeNotFound);
        return string.Join(", ", parts);
    }

    /// <summary>The spotlight's label for a screen reader: the label first, then title, line and facts.</summary>
    public static string Name(Spotlight spot)
    {
        var parts = new List<string> { spot.Label.Text(), spot.Entry.Title };
        if (spot.Line is { } line)
            parts.Add(line.Text());
        var facts = Facts(spot.Entry);
        if (facts.Length > 0)
            parts.Add(facts);
        if (spot.Playable == Playable.NotFound)
            parts.Add(T.HomeNotFound);
        return string.Join(", ", parts);
    }

    /// <summary>The offline line over a kept Home: "Offline. Showing Home from 14:20."</summary>
    public static string OfflineLine(long since, TimeZoneInfo zone, CultureInfo? culture = null) =>
        T.HomeOffline(Calendar.Time(since, zone, culture));

    /// <summary>The days of This week grouped by column, in column order; a day with nothing airing has an empty list.</summary>
    public static List<HomeItem>[] ByDay(HomeSection week)
    {
        var days = new List<HomeItem>[7];
        for (var i = 0; i < days.Length; i++)
            days[i] = [];
        foreach (var item in week.Items)
            if (item.Day is { } d && d < days.Length)
                days[d].Add(item);
        foreach (var day in days)
            day.Sort((a, b) => Nullable.Compare(a.AirsAt, b.AirsAt));
        return days;
    }
}

/// <summary>Paging a shelf: by the whole cards that fit, landing on a card edge.</summary>
public static class ShelfPaging
{
    /// <summary>How many whole cards fit in <paramref name="viewport"/>, at least one.</summary>
    public static int Fit(double viewport, double card, double gap = HomeWording.Gap) =>
        Math.Max(1, (int)Math.Floor((viewport + gap) / (card + gap)));

    /// <summary>
    /// The offset a page lands on from <paramref name="offset"/>, forward or back, always the
    /// left edge of a card and never past the last page.
    /// </summary>
    public static double Target(double offset, double viewport, double extent, double card, int count, bool forward, double gap = HomeWording.Gap)
    {
        var step = card + gap;
        var per = Fit(viewport, card, gap);
        var first = (int)Math.Round(offset / step);
        var last = Math.Max(0, count - per);
        var at = Math.Clamp(first + (forward ? per : -per), 0, last);
        // The last page may sit short of a card edge when the row ends sooner: stop at the end then.
        return Math.Min(at * step, Math.Max(0, extent - viewport));
    }

    /// <summary>Whether the arrow in that direction has anywhere to go.</summary>
    public static bool Can(double offset, double viewport, double extent, bool forward) =>
        forward ? offset + viewport < extent - 1 : offset > 1;
}
