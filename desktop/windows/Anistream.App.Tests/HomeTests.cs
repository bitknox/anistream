using System.Globalization;
using Anistream.App;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class HomeTests
{
    static readonly CultureInfo Gb = CultureInfo.GetCultureInfo("en-GB");
    static readonly DateTimeOffset Now = new(2026, 10, 30, 12, 0, 0, TimeSpan.Zero);

    static HomeItem Item(uint id = 7, HomeAction action = HomeAction.Play, Playable playable = Playable.Unknown, uint? day = null,
        long? airs = null, uint? episode = null, UpNextReason? reason = null, string? because = null, FuzzyDate? starts = null) =>
        new(Entry: new Entry(Id: id, Title: "Lantern Street"), Action: action, Episode: episode, AirsAt: airs, Day: day,
            Reason: reason, Starts: starts, Playable: playable, Because: because);

    [Fact]
    public void ContinueIsCardsThisWeekIsDaysAndTheRestArePosters()
    {
        Assert.Equal(HomeShape.Card, HomeWording.Shape(SectionKind.Continue));
        Assert.Equal(HomeShape.Week, HomeWording.Shape(SectionKind.ThisWeek));
        foreach (var kind in new[] { SectionKind.Starting, SectionKind.AiringToday, SectionKind.UpNext, SectionKind.ThisSeason, SectionKind.Because, SectionKind.MoreGenre })
            Assert.Equal(HomeShape.Poster, HomeWording.Shape(kind));
    }

    [Fact]
    public void ANotFoundTitleAsksToAddToPlanningInsteadOfPlaying()
    {
        Assert.Equal("Resume episode 3", HomeWording.SpotlightAction(HomeAction.Resume, 3, Playable.Unknown));
        Assert.Equal("Play episode 1", HomeWording.SpotlightAction(HomeAction.Play, 1, Playable.Found));
        Assert.Equal("Add to Planning", HomeWording.SpotlightAction(HomeAction.Play, 1, Playable.NotFound));
        Assert.Equal("Add to Planning", HomeWording.SpotlightAction(HomeAction.AddToPlanning, 1, Playable.Unknown));
        Assert.Null(HomeWording.SpotlightAction(HomeAction.Nothing, 1, Playable.Unknown));
        Assert.Equal(HomeAction.AddToPlanning, HomeWording.Effective(HomeAction.Resume, Playable.NotFound));
        Assert.Equal(HomeAction.Nothing, HomeWording.Effective(HomeAction.Nothing, Playable.NotFound));
    }

    [Fact]
    public void APosterSaysItsShortReasonAndTheNameCarriesTheFullOne()
    {
        var item = Item(reason: UpNextReason.Recommended, because: "Tourou Doori");
        Assert.Equal("Recommended for you", HomeWording.Line(item, Now, TimeZoneInfo.Utc, Gb));
        Assert.Equal("Lantern Street, Because you finished Tourou Doori", HomeWording.Name(item, Now, TimeZoneInfo.Utc, Gb));
    }

    [Fact]
    public void AnUnairedTitleSaysWhenItStarts()
    {
        var item = Item(reason: UpNextReason.NewSeason, starts: new FuzzyDate(2027, 1, 9));
        Assert.Equal("Starts 9 January 2027", HomeWording.Line(item, Now, TimeZoneInfo.Utc, Gb));
    }

    [Fact]
    public void AWeekEntrySaysTheEpisodeAndWhenItAirs()
    {
        var later = Item(episode: 5, airs: Now.ToUnixTimeSeconds() + 3600);
        Assert.Equal("Episode 5 · 13:00", HomeWording.Line(later, Now, TimeZoneInfo.Utc, Gb).Replace("1:00 PM", "13:00"));
        var out_ = Item(episode: 5, airs: Now.ToUnixTimeSeconds() - 7200);
        Assert.StartsWith("Episode 5 · Out ", HomeWording.Line(out_, Now, TimeZoneInfo.Utc, Gb));
    }

    [Fact]
    public void TheDayColumnsStartWithTodayAndNameTheRest()
    {
        Assert.Equal("Today", HomeWording.Day(0, Now));
        Assert.Equal("Sunday 1", HomeWording.Day(2, Now, Gb));
    }

    [Fact]
    public void ThisWeekGroupsByDayAndOrdersByAirTime()
    {
        var section = new HomeSection(Kind: SectionKind.ThisWeek, Title: new Msg.HomeThisWeek(), Total: 3,
            Items: [Item(1, day: 3, airs: 200), Item(2, day: 0, airs: 50), Item(3, day: 3, airs: 100)]);
        var days = HomeWording.ByDay(section);
        Assert.Equal(7, days.Length);
        Assert.Single(days[0]);
        Assert.Empty(days[1]);
        Assert.Equal([3u, 1u], days[3].Select(i => i.Entry.Id));
    }

    [Fact]
    public void PagingMovesByTheWholeCardsThatFitAndLandsOnACardEdge()
    {
        // Poster 132 + 16 gap: 148 a step. A 1160 viewport fits seven whole posters.
        Assert.Equal(7, ShelfPaging.Fit(1160, 132));
        Assert.Equal(1, ShelfPaging.Fit(50, 132));
        const double extent = 20 * 148 - 16;
        Assert.Equal(7 * 148, ShelfPaging.Target(0, 1160, extent, 132, 20, forward: true));
        Assert.Equal(extent - 1160, ShelfPaging.Target(7 * 148, 1160, extent, 132, 20, forward: true));
        Assert.Equal(0, ShelfPaging.Target(3 * 148, 1160, extent, 132, 20, forward: false));
    }

    [Fact]
    public void AnArrowShowsOnlyWhereThereIsMoreToSee()
    {
        Assert.False(ShelfPaging.Can(0, 1000, 3000, forward: false));
        Assert.True(ShelfPaging.Can(0, 1000, 3000, forward: true));
        Assert.True(ShelfPaging.Can(500, 1000, 3000, forward: false));
        Assert.False(ShelfPaging.Can(2000, 1000, 3000, forward: true));
        Assert.False(ShelfPaging.Can(0, 1000, 900, forward: true));
    }

    [Fact]
    public void KnownKeepsTheHomeTheEngineSentAndTheHiddenTitles()
    {
        var known = new Known();
        var spot = new Spotlight(SpotlightCase.Popular, new Msg.HomeSpotlightPopular(), new Entry(Id: 9, Title: "Dust and Fireworks"), 1, HomeAction.Play,
            Reason: null, Resume: null, Starts: null, Playable: Playable.Unknown, Line: null, Because: null);
        var home = new HomeView(spot, [], [], null, false, null, false, null);
        known.Take(new Event.Home(home));
        Assert.Same(home, known.Home);
        Assert.Equal("Dust and Fireworks", known.Entry(9)!.Title);
        known.Take(new Event.HiddenTitles([new HiddenTitle(new Entry(Id: 4, Title: "Moon Laundry"), true)]));
        Assert.Single(known.Hidden);
    }
}
