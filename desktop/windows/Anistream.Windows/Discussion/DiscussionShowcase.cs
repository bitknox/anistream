#if ANISTREAM_TESTING
using Anistream.App;
using Anistream.Engine.Model;

namespace Anistream.Windows.Discussion;

/// <summary>
/// The discussions the pane's shots show, made up the way the board makes them up: people and
/// words that belong to no show. <c>--show=discussion</c> and its variants on the testing build.
/// </summary>
static class DiscussionShowcase
{
    public static bool Is(string? show) => show is { } s && s.StartsWith("discussion", StringComparison.Ordinal);

    static CommentSpan Say(string text) => new(text);
    static CommentBlock Block(params CommentSpan[] spans) => new(spans);

    static CommentView Comment(uint id, string who, Msg age, string text, uint likes = 0, bool liked = false, bool own = false,
        PostState pending = PostState.Posted, CommentView[]? replies = null, uint hidden = 0, CommentBlock[]? body = null) =>
        new(id, who, 0, age, likes, pending, null, body ?? [Block(Say(text))], liked, own, replies ?? [], hidden, text);

    static Msg Hours(uint n) => new Msg.DiscussionAgeHours(n);
    static Msg Minutes(uint n) => new Msg.DiscussionAgeMinutes(n);

    /// <summary>The discussion a variant of <c>--show=discussion</c> is.</summary>
    public static DiscussionView View(string show, uint media, uint episode)
    {
        var url = "https://anilist.co/forum/thread/60000";
        DiscussionView Make(DiscussionState state, CommentView[] comments, bool canPost = true, bool signedIn = true, uint count = 214) =>
            new(media, episode, "Episode " + episode + " Discussion", count, url, state, comments, canPost, signedIn, false);

        var deep = Comment(14, "marlowe", Minutes(50), "It reads even better the second time.");
        var thread = new[]
        {
            Comment(11, "lampwick", Hours(2), "Nobody spoke for almost a minute, and it was the best part of the episode.", 41, replies:
            [
                Comment(12, "tamsin", Hours(1), "And the music came back on the exact frame her lantern caught.", 13, liked: true),
            ]),
            Comment(13, "ferrylog", Hours(1), "The lantern light on the faces is drawn frame by frame, not a filter.", 8),
            Comment(15, "wrenfield", Minutes(40), "Calmest episode so far and somehow the tensest.", 3),
        };
        return show switch
        {
            "discussion-loading" => Make(new DiscussionState.Loading(), []),
            "discussion-limited" => Make(new DiscussionState.RateLimited(20), []),
            "discussion-nothread" => Make(new DiscussionState.NoThread(), [], canPost: false, count: 0),
            "discussion-failed" => Make(new DiscussionState.Failed(), [], canPost: false),
            "discussion-empty" => Make(new DiscussionState.Ready(false), [], count: 0),
            "discussion-signedout" => Make(new DiscussionState.Ready(false), thread, canPost: false, signedIn: false),
            "discussion-offline" => Make(new DiscussionState.Offline(120), thread, canPost: false),
            "discussion-offline-empty" => Make(new DiscussionState.Offline(0), [], canPost: false),
            "discussion-more" => Make(new DiscussionState.Ready(true), thread),
            "discussion-sending" => Make(new DiscussionState.Ready(false),
            [
                .. thread,
                Comment(21, "sorrel", new Msg.DiscussionAgeNow(), "The brush texture on the glass is the best detail.", own: true, pending: PostState.Sending),
                Comment(22, "sorrel", new Msg.DiscussionAgeNow(), "Posting this one again, the first never arrived.", own: true, pending: PostState.Failed),
            ]),
            "discussion-didnt" => Make(new DiscussionState.Ready(false),
            [
                thread[0],
                Comment(22, "sorrel", new Msg.DiscussionAgeNow(), "Posting this one again, the first never arrived.", own: true, pending: PostState.Failed),
            ]),
            "discussion-folds" => Make(new DiscussionState.Ready(false),
            [
                Comment(31, "juniper", Hours(1), "", 27, body:
                [
                    Block(Say("If you read ahead you know why the ferryman would not look at her: "), new CommentSpan("the ferryman is her brother", Spoiler: true)),
                    Block(new CommentSpan("", Link: "https://pics.example/frame.png", Media: MediaKind.Image)),
                ]),
                Comment(32, "ferrylog", Hours(2), "", 9, body:
                [
                    Block(Say("A long comment keeps going past ten lines. ")),
                    .. Enumerable.Range(1, 12).Select(i => Block(Say("Line " + i + " of what someone had to say about the lantern road and how it was staged."))),
                ]),
                Comment(33, "lampwick", Hours(3), "", 4, body:
                [
                    new CommentBlock([Say("the best part of the episode")], Quote: true),
                    Block(Say("Agreed, and "), new CommentSpan("bold", Bold: true), Say(", "), new CommentSpan("italic", Italic: true), Say(", "), new CommentSpan("struck", Strike: true), Say(" and "), new CommentSpan("a link", Link: "https://shows.example/a")),
                ], replies:
                [
                    Comment(34, "tamsin", Hours(2), "First answer.", 2, hidden: 1, replies: [deep]),
                    Comment(35, "wrenfield", Hours(1), "Second answer."),
                ]),
            ]),
            _ => Make(new DiscussionState.Ready(false),
            [
                .. thread,
                Comment(21, "sorrel", new Msg.DiscussionAgeNow(), "The brush texture on the glass is the best detail.", 1, own: true),
            ]),
        };
    }

    /// <summary>
    /// What to do to the pane once it has drawn, for the shots that need a click: the spoilers
    /// opened, a menu, the delete question, an answer or an edit under way.
    /// </summary>
    public static void Then(string show, DiscussionPane pane)
    {
        switch (show)
        {
            case "discussion-revealed":
                pane.Reveal("all");
                break;
            case "discussion-menu":
                pane.Row(21)?.OpenMenu();
                break;
            case "discussion-delete":
                _ = pane.ConfirmDelete(21);
                break;
            case "discussion-reply":
                pane.Compose(new ComposeMode.Reply(11, "lampwick"), "The brush texture on the glass is the best detail.");
                break;
            case "discussion-edit":
                pane.Compose(new ComposeMode.Edit(21), "The brush texture on the glass is the best detail.");
                break;
            case "discussion-typing":
                pane.Compose(new ComposeMode.New(), "The brush texture on the glass is the best detail.");
                break;
        }
    }
}
#endif
