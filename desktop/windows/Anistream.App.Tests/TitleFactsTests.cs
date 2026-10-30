// The title page's facts: the run's fact, a film's length, where it was made and the metadata line.

using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class TitleFactsTests
{
    [Fact]
    public void ADateIsAsFarAsItIsKnown()
    {
        Assert.Equal("2027", TitleFacts.Date(new FuzzyDate(2027, null, null)));
        Assert.Equal("January 2027", TitleFacts.Date(new FuzzyDate(2027, 1, null)));
        Assert.Equal("9 January 2027", TitleFacts.Date(new FuzzyDate(2027, 1, 9)));
    }

    [Fact]
    public void TheRunReadsByWhatTheEngineDecided()
    {
        Assert.Null(TitleFacts.Airing(null));
        Assert.Equal("Now", TitleFacts.Airing(new AiringFact.Airing(null))!.Value);
        Assert.Equal("Since March 2026", TitleFacts.Airing(new AiringFact.Hiatus(new FuzzyDate(2026, 3, null)))!.Value);
        Assert.Equal("On hiatus", TitleFacts.Airing(new AiringFact.Hiatus(null))!.Label);
        var range = TitleFacts.Airing(new AiringFact.Finished(new FuzzyDate(2023, 4, null), new FuzzyDate(2023, 6, null)))!;
        Assert.Equal(("Aired", "April 2023 to June 2023"), (range.Label, range.Value));
        var film = TitleFacts.Airing(new AiringFact.Finished(new FuzzyDate(2025, 5, 2), null))!;
        Assert.Equal(("Released", "2 May 2025"), (film.Label, film.Value));
        Assert.Equal("After episode 6", TitleFacts.Airing(new AiringFact.Cancelled(6))!.Value);
    }

    [Fact]
    public void AFilmsLengthIsHoursAndMinutes()
    {
        Assert.Equal("1 h 52 min", TitleFacts.FilmLength(new RunLength(1, 52)));
        Assert.Equal("2 hr", TitleFacts.FilmLength(new RunLength(2, 0)));
        Assert.Equal("46 min", TitleFacts.FilmLength(new RunLength(0, 46)));
    }

    [Fact]
    public void TheCountryShowsOnlyOutsideJapan()
    {
        Assert.Null(TitleFacts.Country(new Entry(Id: 1, Title: "Lantern Road", Country: "JP")));
        Assert.Equal("China", TitleFacts.Country(new Entry(Id: 1, Title: "Lantern Road", Country: "CN", MadeOutsideJapan: true)));
        Assert.Equal("Made outside Japan", TitleFacts.Country(new Entry(Id: 1, Title: "Lantern Road", MadeOutsideJapan: true)));
    }

    [Fact]
    public void TheMetadataLineCarriesTheStudioBeforeTheScore()
    {
        var film = new Entry(Id: 1, Title: "Lantern Road", Format: "MOVIE", FilmLength: new RunLength(1, 52), Year: 2025, Studio: "Ginrei Studio", Score: 82);
        Assert.Equal("Movie · 1 h 52 min · 2025 · Ginrei Studio · 82%", Wording.MetaLine(film));
    }
}
