using Anistream.App;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class CalendarReachTests
{
    static readonly TimeZoneInfo Utc = TimeZoneInfo.Utc;
    // Thursday 29 October 2026, 16:42 UTC.
    static readonly long Anchor = new DateTimeOffset(2026, 10, 29, 16, 42, 0, TimeSpan.Zero).ToUnixTimeSeconds();
    const long Day = 86_400;

    static CalendarReach Reach(bool morePast = true, bool moreAhead = true)
    {
        var reach = new CalendarReach();
        reach.Window(new Event.CalendarWindow(Anchor, 1, morePast, Anchor - 90 * Day, Anchor + 28 * Day, 1, moreAhead));
        return reach;
    }

    [Fact]
    public void WeeksStopAtTheFloorAndTheCeiling()
    {
        var reach = Reach();
        // The floor is Friday 31 July, so the week of Monday 27 July is the first, the ceiling Thursday 26 November.
        Assert.True(reach.CanGoEarlier(new DateOnly(2026, 8, 3), Utc));
        Assert.False(reach.CanGoEarlier(new DateOnly(2026, 7, 27), Utc));
        Assert.True(reach.CanGoLater(new DateOnly(2026, 11, 16), Utc));
        Assert.False(reach.CanGoLater(new DateOnly(2026, 11, 23), Utc));
        Assert.True(reach.InRange(new DateOnly(2026, 7, 31), Utc));
        Assert.False(reach.InRange(new DateOnly(2026, 7, 30), Utc));
        Assert.False(reach.InRange(new DateOnly(2026, 11, 27), Utc));
    }

    [Fact]
    public void ALaterWeekAsksForTheNextPageThroughItsEnd()
    {
        var reach = Reach();
        var ask = Assert.IsType<Command.LoadCalendarAhead>(reach.Later(Anchor + 12 * Day, Anchor + 8 * Day));
        Assert.Equal((Anchor, 2u), (ask.Anchor, ask.Page));
        Assert.Equal(Anchor + 12 * Day, ask.Through);
        // One ask at a time.
        Assert.Null(reach.Later(Anchor + 13 * Day, Anchor + 8 * Day));
        reach.Ahead(3, true);
        Assert.Equal(3u, reach.AheadPages);
        Assert.IsType<Command.LoadCalendarAhead>(reach.Later(Anchor + 20 * Day, Anchor + 12 * Day));
    }

    [Fact]
    public void NothingIsAskedForWhatIsLoadedOrBeyondTheEnds()
    {
        var reach = Reach();
        Assert.Null(reach.Later(Anchor + 5 * Day, Anchor + 8 * Day));
        Assert.Null(reach.Past(Anchor - 3 * Day, Anchor - 8 * Day));
        // Past the ceiling the ask is cut to it, and once it is loaded nothing more is asked.
        Assert.Null(reach.Later(Anchor + 60 * Day, Anchor + 28 * Day));
        Assert.Null(Reach(moreAhead: false).Later(Anchor + 20 * Day, Anchor + 8 * Day));
        Assert.Null(Reach(morePast: false).Past(Anchor - 20 * Day, Anchor - 8 * Day));
    }

    [Fact]
    public void AFailedAskWaitsForARetry()
    {
        var reach = Reach();
        Assert.IsType<Command.LoadCalendarPast>(reach.Past(Anchor - 20 * Day, Anchor - 8 * Day));
        reach.Backfill(null, true);
        Assert.True(reach.PastFailed);
        Assert.Null(reach.Past(Anchor - 20 * Day, Anchor - 8 * Day));
        var again = Assert.IsType<Command.LoadCalendarPast>(reach.Past(Anchor - 20 * Day, Anchor - 8 * Day, retry: true));
        Assert.Equal(2u, again.Page);
        Assert.False(reach.PastFailed);
    }

    [Fact]
    public void ADayBeyondTheLoadedEdgeIsPending()
    {
        var reach = Reach();
        var monday = new DateOnly(2026, 11, 9);
        var start = CalendarReach.Start(monday, Utc);
        Assert.True(reach.Pending(start, start + Day, Anchor - 8 * Day, Anchor + 8 * Day));
        Assert.False(reach.Pending(start, start + Day, Anchor - 8 * Day, Anchor + 12 * Day));
        // Beyond the ceiling is not loading, it is the edge.
        var beyond = CalendarReach.Start(new DateOnly(2026, 11, 30), Utc);
        Assert.False(reach.Pending(beyond, beyond + Day, Anchor - 8 * Day, Anchor + 8 * Day));
    }
}
