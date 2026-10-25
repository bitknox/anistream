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
    /// "Ep 10 · Dub · 16:30" for a dub row, "Ep 10 · 16:30" for any other, "~16:30" when the time
    /// is an estimate, with "(was 23:00)" when the schedule moved it. The engine's
    /// <c>episode_line</c> and <c>air_time</c>.
    /// </summary>
    public string Line(TimeZoneInfo zone, CultureInfo? culture = null)
    {
        var time = AnistreamFfiMethods.AirTime(Calendar.Time(Airs, zone, culture), Entry.AiringApproximate);
        var line = $"{EpisodeText} · {time}";
        return Was is { } was ? $"{line} (was {Calendar.Time(was, zone, culture)})" : line;
    }

    /// <summary>
    /// The episode with the row's mark after it, as every episode line ends its episode: "Ep 10 ·
    /// Dub" for a dub row, "Ep 10" for a sub row, which is not marked.
    /// </summary>
    public string EpisodeText => AnistreamFfiMethods.EpisodeLine($"Ep {Episode}", Mark is null ? null : Translation.Dub);

    /// <summary>"Dub" for a dub row, else null (a sub row is not marked): the engine's <c>calendar_mark</c>.</summary>
    public string? Mark => AnistreamFfiMethods.CalendarMark(Entry);
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
    /// month names of the system's locale.
    /// </summary>
    public static string WeekTitle(DateOnly monday, CultureInfo? culture = null)
    {
        culture ??= CultureInfo.CurrentCulture;
        var sunday = monday.AddDays(6);
        if (monday.Year != sunday.Year)
            return $"{monday.ToString("d MMMM yyyy", culture)} to {sunday.ToString("d MMMM yyyy", culture)}";
        if (monday.Month != sunday.Month)
            return $"{monday.ToString("d MMMM", culture)} to {sunday.ToString("d MMMM yyyy", culture)}";
        return $"{monday.Day} to {sunday.ToString("d MMMM yyyy", culture)}";
    }

    /// <summary>A clock time on the system's clock: "16:30", or "4:30 PM" where the locale says so.</summary>
    public static string Time(long unix, TimeZoneInfo zone, CultureInfo? culture = null) =>
        TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(unix), zone).ToString("t", culture ?? CultureInfo.CurrentCulture);
}
