using System.Globalization;
using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>One broadcast: a title's episode and when it airs, in Unix seconds.</summary>
public sealed record Broadcast(Entry Entry, uint Episode, long Airs, long? Was)
{
    public bool Watched => Entry.Progress is { } p && p.Done >= Episode;

    /// <summary>Aired and not watched: the water dot.</summary>
    public bool IsNew(long now) => Airs <= now && !Watched;

    /// <summary>
    /// "E10 dubbed · 16:30" for a dub row, "E10 · 16:30" for any other, "~16:30" when the time
    /// is an estimate, with "· Moved from 23:00" when the schedule moved it.
    /// </summary>
    public string Line(TimeZoneInfo zone, CultureInfo? culture = null)
    {
        var shown = Calendar.Time(Airs, zone, culture);
        return Said(Entry.AiringApproximate ? T.CalendarTimeEstimate(shown) : shown, zone, culture);
    }

    /// <summary>
    /// The line as Narrator reads it: "about 16:30" for an estimate, which the shown line marks
    /// with a tilde that would be read out as one.
    /// </summary>
    public string Spoken(TimeZoneInfo zone, CultureInfo? culture = null)
    {
        var shown = Calendar.Time(Airs, zone, culture);
        return Said(Entry.AiringApproximate ? T.CalendarTimeAbout(shown) : shown, zone, culture);
    }

    string Said(string time, TimeZoneInfo zone, CultureInfo? culture)
    {
        var line = Dubbed ? T.CalendarRowTimeDubbed(Episode, time) : T.CalendarRowTime(Episode, time);
        return Was is { } was ? $"{line} · {T.CalendarMovedFrom(Calendar.Time(was, zone, culture))}" : line;
    }

    /// <summary>
    /// The episode with the row's mark after it, as every episode line ends its episode: "E10
    /// dubbed" for a dub row, "E10" for a sub row, which is not marked.
    /// </summary>
    public string EpisodeText => Dubbed ? T.CalendarEpisodeDubbed(Episode) : T.EpisodeNumberShort(Episode);

    /// <summary>
    /// A row of the dub's own time, or of an episode out in the dub: the model's
    /// <c>calendar_mark</c>. Only dubs are marked, so a sub row says nothing.
    /// </summary>
    public bool Dubbed => Entry.AiringTranslation == Translation.Dub || Entry.AiringDubbed;
}

/// <summary>The calendar's week: Monday first, in the viewer's time zone.</summary>
public static class Calendar
{
    /// <summary>A title is yours when it is under way here or on your Watching or Planning list.</summary>
    public static bool IsMine(Entry entry) =>
        entry.Progress is not null || entry.ListStatus is LibrarySegment.Watching or LibrarySegment.Planning;

    /// <summary>The broadcasts in <paramref name="rows"/>, oldest first, each title as it now stands.</summary>
    public static List<Broadcast> Broadcasts(IEnumerable<Entry> rows, long anchor, Known known, bool mine)
    {
        var list = new List<Broadcast>();
        foreach (var row in rows)
        {
            if (row.AiringIn is not { } airingIn || row.AiringEpisode is not { } episode)
                continue;
            var entry = row;
            if (known.Entry(row.Id) is { } now)
                entry = row with { Progress = now.Progress, ListStatus = now.ListStatus ?? row.ListStatus };
            if (mine && !IsMine(entry))
                continue;
            list.Add(new Broadcast(entry, episode, anchor + airingIn, row.DelayedFrom));
        }
        list.Sort((a, b) => a.Airs.CompareTo(b.Airs));
        return list;
    }

    /// <summary>The Monday that starts the week <paramref name="offset"/> weeks from the one holding <paramref name="now"/>.</summary>
    public static DateOnly WeekStart(long now, int offset, TimeZoneInfo zone)
    {
        var today = DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(now), zone).DateTime);
        var sinceMonday = ((int)today.DayOfWeek + 6) % 7;
        return today.AddDays(-sinceMonday + 7 * offset);
    }

    /// <summary>The day a Unix time falls on, in <paramref name="zone"/>.</summary>
    public static DateOnly Day(long unix, TimeZoneInfo zone) =>
        DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(unix), zone).DateTime);

    /// <summary>The seven days from <paramref name="monday"/>, each with its broadcasts in order.</summary>
    public static List<(DateOnly Day, List<Broadcast> Broadcasts)> Week(DateOnly monday, IEnumerable<Broadcast> broadcasts, TimeZoneInfo zone)
    {
        var days = Enumerable.Range(0, 7).Select(i => (Day: monday.AddDays(i), Broadcasts: new List<Broadcast>())).ToList();
        foreach (var b in broadcasts)
        {
            var at = Day(b.Airs, zone).DayNumber - monday.DayNumber;
            if (at is >= 0 and < 7)
                days[at].Broadcasts.Add(b);
        }
        return days;
    }

    /// <summary>
    /// "19 to 25 October 2026", or across months "28 September to 4 October 2026", with the
    /// month names of the system's locale and the catalogue's word between them.
    /// </summary>
    public static string WeekTitle(DateOnly monday, CultureInfo? culture = null)
    {
        culture ??= CultureInfo.CurrentCulture;
        var sunday = monday.AddDays(6);
        if (monday.Year != sunday.Year)
            return T.CalendarWeekRange(monday.ToString("d MMMM yyyy", culture), sunday.ToString("d MMMM yyyy", culture));
        if (monday.Month != sunday.Month)
            return T.CalendarWeekRange(monday.ToString("d MMMM", culture), sunday.ToString("d MMMM yyyy", culture));
        return T.CalendarWeekRange(monday.Day.ToString(culture), sunday.ToString("d MMMM yyyy", culture));
    }

    /// <summary>A clock time on the system's clock: "16:30", or "4:30 PM" where the locale says so.</summary>
    public static string Time(long unix, TimeZoneInfo zone, CultureInfo? culture = null) =>
        TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(unix), zone).ToString("t", culture ?? CultureInfo.CurrentCulture);
}
