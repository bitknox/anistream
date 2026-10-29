using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// How far the calendar reaches and what is loaded of it: the window the engine reported, the
/// pages asked for either side and whether an ask is out or failed. The engine fixes the floor
/// and the ceiling; nothing here hard-codes them.
/// </summary>
public sealed class CalendarReach
{
    public bool Known { get; private set; }
    public long Anchor { get; private set; }
    public long Floor { get; private set; }
    public long Ceiling { get; private set; }
    public uint PastPages { get; private set; }
    public uint AheadPages { get; private set; }
    public bool MorePast { get; private set; } = true;
    public bool MoreAhead { get; private set; } = true;
    public bool AskingPast { get; private set; }
    public bool AskingAhead { get; private set; }
    public bool PastFailed { get; private set; }
    public bool AheadFailed { get; private set; }

    /// <summary>A fresh window from the engine: what was asked is forgotten, as the rows were replaced.</summary>
    public void Window(Event.CalendarWindow w)
    {
        (Known, Anchor, Floor, Ceiling) = (true, w.Anchor, w.Floor, w.Ceiling);
        (PastPages, MorePast, AheadPages, MoreAhead) = (w.Pages, w.MorePast, w.AheadPages, w.MoreAhead);
        AskingPast = AskingAhead = PastFailed = AheadFailed = false;
    }

    /// <summary>Earlier broadcasts arrived, or, with no page, could not be had.</summary>
    public void Backfill(uint? page, bool morePast)
    {
        AskingPast = false;
        if (page is { } p)
            (PastPages, MorePast, PastFailed) = (Math.Max(PastPages, p), morePast, false);
        else
            PastFailed = true;
    }

    /// <summary>Later broadcasts arrived, or, with no page, could not be had.</summary>
    public void Ahead(uint? page, bool moreAhead)
    {
        AskingAhead = false;
        if (page is { } p)
            (AheadPages, MoreAhead, AheadFailed) = (Math.Max(AheadPages, p), moreAhead, false);
        else
            AheadFailed = true;
    }

    /// <summary>
    /// The ask that brings in what lies before <paramref name="from"/> when the loaded part starts
    /// later, or null: nothing is missing, an ask is out, the last one failed (unless
    /// <paramref name="retry"/>) or the floor has been reached.
    /// </summary>
    public Command? Past(long from, long? oldest, bool retry = false)
    {
        from = Math.Max(from, Floor);
        if (!Known || !MorePast || AskingPast || (PastFailed && !retry) || from >= (oldest ?? Anchor))
            return null;
        (AskingPast, PastFailed) = (true, false);
        return new Command.LoadCalendarPast(Anchor, PastPages + 1, from);
    }

    /// <summary>The ask for what lies after <paramref name="to"/>, mirroring <see cref="Past"/>.</summary>
    public Command? Later(long to, long? latest, bool retry = false)
    {
        to = Math.Min(to, Ceiling);
        if (!Known || !MoreAhead || AskingAhead || (AheadFailed && !retry) || to <= (latest ?? Anchor))
            return null;
        (AskingAhead, AheadFailed) = (true, false);
        return new Command.LoadCalendarAhead(Anchor, AheadPages + 1, to);
    }

    /// <summary>Whether a day, from <paramref name="start"/> to <paramref name="end"/>, still waits for its broadcasts.</summary>
    public bool Pending(long start, long end, long? oldest, long? latest) =>
        Known && start >= Floor && start < Ceiling
        && ((MorePast && end <= (oldest ?? Anchor)) || (MoreAhead && start >= (latest ?? Anchor)));

    /// <summary>Whether a day falls inside the calendar's reach at all.</summary>
    public bool InRange(DateOnly day, TimeZoneInfo zone) =>
        !Known || (Calendar.Day(Floor, zone) <= day && day <= Calendar.Day(Ceiling, zone));

    /// <summary>Whether the week starting <paramref name="monday"/> has an earlier one inside the reach.</summary>
    public bool CanGoEarlier(DateOnly monday, TimeZoneInfo zone) =>
        !Known || monday.AddDays(-1) >= Calendar.Day(Floor, zone);

    /// <summary>Whether the week starting <paramref name="monday"/> has a later one inside the reach.</summary>
    public bool CanGoLater(DateOnly monday, TimeZoneInfo zone) =>
        !Known || monday.AddDays(7) <= Calendar.Day(Ceiling, zone);

    /// <summary>The Unix second a day begins at, in <paramref name="zone"/>.</summary>
    public static long Start(DateOnly day, TimeZoneInfo zone) =>
        new DateTimeOffset(TimeZoneInfo.ConvertTimeToUtc(day.ToDateTime(TimeOnly.MinValue), zone), TimeSpan.Zero).ToUnixTimeSeconds();
}
