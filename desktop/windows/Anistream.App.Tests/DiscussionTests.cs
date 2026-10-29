// The discussion pane's rules that need no window: how a comment's body is cut into parts,
// which links may open, and what the field sends.

using Anistream.App;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

public class DiscussionTests
{
    static CommentSpan Plain(string text) => new(text);
    static CommentSpan Hidden(string text) => new(text, Spoiler: true);

    [Fact]
    public void NeighbouringSpoilerSpansAreOneSpoilerWithOnePlace()
    {
        var block = new CommentBlock([Plain("a "), Hidden("b"), Hidden("c"), Plain(" d"), Hidden("e")]);
        var spoiler = 0;
        var parts = Discussions.Parts(block, false, ref spoiler);
        Assert.Equal(2, spoiler);
        Assert.Equal([0, 1], parts.OfType<CommentPart.Spoiler>().Select(p => p.At));
        Assert.Equal(2, parts.OfType<CommentPart.Spoiler>().First().Spans.Length);
    }

    [Fact]
    public void SpoilersCountOnAcrossParagraphsOfOneComment()
    {
        var spoiler = 0;
        Discussions.Parts(new CommentBlock([Hidden("one")]), false, ref spoiler);
        var second = Discussions.Parts(new CommentBlock([Hidden("two")]), false, ref spoiler);
        Assert.Equal(1, second.OfType<CommentPart.Spoiler>().Single().At);
    }

    [Fact]
    public void AnImageIsAPictureOnlyWhenImagesLoad()
    {
        var image = new CommentSpan("", Link: "https://pics.example/a.png", Media: MediaKind.Image);
        var block = new CommentBlock([image]);
        var spoiler = 0;
        Assert.IsType<CommentPart.Picture>(Discussions.Parts(block, true, ref spoiler)[0]);
        var line = Assert.IsType<CommentPart.Media>(Discussions.Parts(block, false, ref spoiler)[0]);
        Assert.Equal("pics.example", Discussions.Host(line.Url));
    }

    [Fact]
    public void AVideoIsAlwaysALine()
    {
        var video = new CommentSpan("", Link: "https://clips.example/v", Media: MediaKind.Video);
        var spoiler = 0;
        Assert.IsType<CommentPart.Media>(Discussions.Parts(new CommentBlock([video]), true, ref spoiler)[0]);
    }

    [Theory]
    [InlineData("https://shows.example/a", true)]
    [InlineData("http://shows.example/a", true)]
    [InlineData("file:///c:/secret", false)]
    [InlineData("javascript:alert(1)", false)]
    [InlineData("ms-settings:network", false)]
    [InlineData("not a link", false)]
    [InlineData(null, false)]
    public void OnlyWebLinksOpen(string? link, bool opens) => Assert.Equal(opens, Discussions.Web(link) is not null);

    [Fact]
    public void TheFieldSendsWhatItsModeSays()
    {
        Assert.Equal(new Command.PostComment(null, "hi"), Discussions.Send(new ComposeMode.New(), "hi"));
        Assert.Equal(new Command.PostComment(7, "hi"), Discussions.Send(new ComposeMode.Reply(7, "tamsin"), "hi"));
        Assert.Equal(new Command.EditComment(9, "hi"), Discussions.Send(new ComposeMode.Edit(9), "hi"));
    }

    [Fact]
    public void ADiscussionIsOfferedFromEightyFivePercent()
    {
        Assert.False(Discussions.Counted(1000, 1440));
        Assert.True(Discussions.Counted(1224, 1440));
        Assert.False(Discussions.Counted(10, 0));
    }

    [Fact]
    public void KnownKeepsTheCountOfAFoundDiscussion()
    {
        var known = new Known();
        known.Take(new Event.DiscussionFound(5, 3, 214));
        Assert.Equal(214u, known.Discussions[(5, 3)]);
        Assert.False(known.Discussions.ContainsKey((5, 4)));
    }

    [Fact]
    public void ARepliesAreFoundInsideTheirParents()
    {
        var deep = Comment(3, []);
        var reply = Comment(2, [deep]);
        var top = Comment(1, [reply]);
        Assert.Equal(3u, Discussions.Find([top], 3)?.Id);
        Assert.Null(Discussions.Find([top], 4));
    }

    static CommentView Comment(uint id, CommentView[] replies) =>
        new(id, "author", 0, new Msg.DiscussionAgeNow(), 0, PostState.Posted, Replies: replies);
}
