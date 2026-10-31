using System.Globalization;
using Anistream.App;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class UpNextTests
{
    static readonly CultureInfo Gb = CultureInfo.GetCultureInfo("en-GB");
    static readonly DateTimeOffset Now = new(2026, 10, 30, 12, 0, 0, TimeSpan.Zero);

    static UpNextItem Item(UpNextReason reason, FuzzyDate? starts = null, string? because = null, UpNextAction action = UpNextAction.PlayFirstEpisode) =>
        new(new Entry(Id: 7, Title: "Lantern Street"), reason, action, because, starts);

    [Fact]
    public void DatesSayAsMuchAsAniListKnows()
    {
        Assert.Equal("9 January", UpNextWording.Date(new(2026, 1, 9), Now, Gb));
        Assert.Equal("9 January 2027", UpNextWording.Date(new(2027, 1, 9), Now, Gb));
        Assert.Equal("January 2027", UpNextWording.Date(new(2027, 1), Now, Gb));
        Assert.Equal("2027", UpNextWording.Date(new(2027), Now, Gb));
    }

    [Fact]
    public void ARowSaysWhyAndTheTitleIsTheOnlyPartCutShort()
    {
        var line = UpNextWording.Line(Item(UpNextReason.Recommended, because: "Tourou Doori"), Now, Gb);
        Assert.Equal("Because you finished ", line.Lead);
        Assert.Equal("Tourou Doori", line.Tail);
        Assert.Equal("New season", UpNextWording.Line(Item(UpNextReason.NewSeason), Now, Gb).ToString());
    }

    [Fact]
    public void AnUnairedTitleSaysWhenItStarts()
    {
        var line = UpNextWording.Line(Item(UpNextReason.NewSeason, new FuzzyDate(2027, 1, 9)), Now, Gb);
        Assert.Equal("New season · Starts 9 January 2027", line.ToString());
    }

    [Fact]
    public void TheButtonAndTheNoticeFollowTheAction()
    {
        Assert.Equal("Add to Planning", UpNextWording.Button(Item(UpNextReason.Planning, action: UpNextAction.AddToPlanning)));
        Assert.Null(UpNextWording.Button(Item(UpNextReason.Planning, action: UpNextAction.Nothing)));
        Assert.Equal("Lantern Street is hidden from Up next for you", UpNextWording.HiddenNotice(Item(UpNextReason.NewSeason)));
        Assert.Equal("Lantern Street is hidden until it airs", UpNextWording.HiddenNotice(Item(UpNextReason.NewSeason, new FuzzyDate(2027))));
    }
}
