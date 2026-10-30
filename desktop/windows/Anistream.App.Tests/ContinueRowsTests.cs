// Continue's rows carry this device's watch history; another view's rows must not replace it.

using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class ContinueRowsTests
{
    static readonly Entry Local = new(
        Id: 900_001, Title: "Stardust Wayfarer", Episodes: 12,
        Progress: new TitleProgress(Done: 6, Next: 7),
        Resume: new ResumePoint(Position: 606, Fraction: 606.0 / 1440));

    [Fact]
    public void ContinueKeepsLocalProgressAfterTheLibraryLoads()
    {
        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Continue(), new ContentList.Entries([Local])));
        var tracker = Local with { Progress = new TitleProgress(Done: 12, Next: 1), ListStatus = LibrarySegment.Completed };
        known.Take(new Event.Content(new ContentFor.Library(LibrarySegment.Completed), new ContentList.Entries([tracker])));
        var queue = Wording.Said(AnistreamFfiMethods.ContinueQueue(known.ListEntries(new ContentFor.Continue()), [], Casing.Sentence));
        var row = queue.InProgress.Concat(queue.NewThisWeek).Single(r => r.Entry.Id == 900_001);
        Assert.Equal("Resume E7 · 10:06", row.Action);
        Assert.Equal(7u, row.Episode);
        // The Library's own fields still land.
        Assert.Equal(LibrarySegment.Completed, known.Entry(900_001)!.ListStatus);
    }

    [Fact]
    public void AViewWithoutAirDatesKeepsWhatTheTitleAlreadyAired()
    {
        var known = new Known();
        var aired = Local with { LastAired = new Aired(Episode: 3, Ago: 600) };
        known.Take(new Event.Content(new ContentFor.Continue(), new ContentList.Entries([aired])));
        known.Take(new Event.Content(new ContentFor.Calendar(), new ContentList.Entries([Local with { Progress = null, Resume = null }])));
        Assert.Equal(3u, known.Entry(900_001)!.LastAired?.Episode);
        Assert.Equal(7u, known.Entry(900_001)!.Progress?.Next);
    }

    [Fact]
    public void ATitleFirstSeenInAnotherViewIsKeptAsItCame()
    {
        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Seasonal(), new ContentList.Entries([Local with { Progress = new TitleProgress(Done: 2, Next: 3) }])));
        Assert.Equal(3u, known.Entry(900_001)!.Progress?.Next);
    }

    [Fact]
    public void ContinueStillTakesItsOwnNewerProgress()
    {
        var known = new Known();
        known.Take(new Event.Content(new ContentFor.Continue(), new ContentList.Entries([Local])));
        var later = Local with { Progress = new TitleProgress(Done: 7, Next: 8), Resume = null };
        known.Take(new Event.Content(new ContentFor.Continue(), new ContentList.Entries([later])));
        Assert.Equal(8u, known.Entry(900_001)!.Progress?.Next);
        Assert.Null(known.Entry(900_001)!.Resume);
    }
}
