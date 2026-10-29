using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>What the compose field is for.</summary>
public abstract record ComposeMode
{
    public sealed record New : ComposeMode;
    public sealed record Reply(uint Parent, string Name) : ComposeMode;
    public sealed record Edit(uint Id) : ComposeMode;
}

/// <summary>A paragraph cut into what draws as one run of text, a spoiler, a picture or a media line.</summary>
public abstract record CommentPart
{
    /// <summary>Spans that are shown as they are.</summary>
    public sealed record Text(CommentSpan[] Spans) : CommentPart;

    /// <summary>Neighbouring spoiler spans; <paramref name="At"/> is the place that stays the same when the comment is drawn again.</summary>
    public sealed record Spoiler(int At, CommentSpan[] Spans) : CommentPart;

    /// <summary>An image that loads inline, because Settings says so.</summary>
    public sealed record Picture(string Url) : CommentPart;

    /// <summary>An image or video as a line that opens it in the browser.</summary>
    public sealed record Media(MediaKind Kind, string? Url) : CommentPart;
}

/// <summary>The discussion pane's rules that need no window: how a comment's body is cut, what may open, what the field does.</summary>
public static class Discussions
{
    /// <summary>A comment folds past this many lines, with More to open it.</summary>
    public const int FoldLines = 10;

    /// <summary>
    /// A paragraph as its parts. <paramref name="spoiler"/> counts the spoilers so far in the comment,
    /// so each has a place of its own, and neighbouring spoiler spans are one.
    /// </summary>
    public static IReadOnlyList<CommentPart> Parts(CommentBlock block, bool loadImages, ref int spoiler)
    {
        var parts = new List<CommentPart>();
        var text = new List<CommentSpan>();
        List<CommentSpan>? hidden = null;
        var at = 0;
        void Flush()
        {
            if (text.Count > 0)
                parts.Add(new CommentPart.Text([.. text]));
            text.Clear();
        }
        void Close()
        {
            if (hidden is null)
                return;
            Flush();
            parts.Add(new CommentPart.Spoiler(at, [.. hidden]));
            hidden = null;
        }
        foreach (var span in block.Spans ?? [])
        {
            if (span.Spoiler)
            {
                if (hidden is null)
                {
                    hidden = [];
                    at = spoiler++;
                }
                hidden.Add(span);
                continue;
            }
            Close();
            if (span.Media is { } kind)
            {
                Flush();
                parts.Add(kind == MediaKind.Image && loadImages && Web(span.Link) is { } url
                    ? new CommentPart.Picture(url)
                    : new CommentPart.Media(kind, Web(span.Link)));
            }
            else
                text.Add(span);
        }
        Close();
        Flush();
        return parts;
    }

    /// <summary>The link when it is a web address the browser may open; anything else (a script, a file) never opens.</summary>
    public static string? Web(string? link) =>
        link is { Length: > 0 } && Uri.TryCreate(link, UriKind.Absolute, out var uri) && uri.Scheme is "http" or "https" ? uri.AbsoluteUri : null;

    /// <summary>The host of a link, for the media line: "Image · pics.example".</summary>
    public static string Host(string? link) =>
        Web(link) is { } url ? new Uri(url).Host : "";

    /// <summary>The comment with this id, searching replies.</summary>
    public static CommentView? Find(IEnumerable<CommentView> comments, uint id)
    {
        foreach (var c in comments)
        {
            if (c.Id == id)
                return c;
            if (Find(c.Replies ?? [], id) is { } inner)
                return inner;
        }
        return null;
    }

    /// <summary>The words a comment's age shows: its own, or Sending while it waits.</summary>
    public static string Age(CommentView comment) =>
        comment.Pending == PostState.Sending ? T.DiscussionSending : comment.Age.Text();

    /// <summary>The command a field's text sends in a mode.</summary>
    public static Command Send(ComposeMode mode, string text) => mode switch
    {
        ComposeMode.Reply r => new Command.PostComment(r.Parent, text),
        ComposeMode.Edit e => new Command.EditComment(e.Id, text),
        _ => new Command.PostComment(null, text),
    };

    /// <summary>Whether an episode's discussion is worth offering at <paramref name="position"/> of <paramref name="duration"/>: from 85% on.</summary>
    public static bool Counted(double position, double duration) => duration > 0 && position / duration >= 0.85;
}
