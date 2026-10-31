// The match picker's rules: when a page asks, what a search answers, how a pick settles, and how
// a row and the page's line are worded.

using Anistream.App;
using Anistream.Engine.Model;
using Xunit;

namespace Anistream.App.Tests;

[Collection("Strings")]
public class MatchPickerTests
{
    const uint Show = 900001;

    public MatchPickerTests()
    {
        T.Resolver = null;
        T.Language = "en";
    }

    static MatchCandidate Candidate(string key, string title, bool ruledOut = false) =>
        new(title, key, 0.71, ruledOut ? new Msg.MatchRuledOutYear() : null, 2024, 12, MediaFormat.Tv);

    static Event Ladder(params MatchCandidate[] candidates) =>
        new Event.MatchChoices(Show, "a", candidates, new MatchQuestion.Ladder(), "Source A");

    static Event Found(string query, params MatchCandidate[] candidates) =>
        new Event.MatchChoices(Show, "a", candidates, new MatchQuestion.Search(query), "Source A");

    static Event Matched(bool byYou) =>
        new Event.TitleMatch(Show, new CurrentMatch("a", "Source A", "Made Up Show", byYou));

    [Fact]
    public void The_ladder_asks_and_counts_only_the_titles_not_ruled_out()
    {
        var book = new MatchBook();
        book.Take(Ladder(Candidate("a:1", "One"), Candidate("a:2", "Two"), Candidate("a:3", "Three", ruledOut: true)));
        Assert.True(book.Asks(Show));
        Assert.Equal(2, book.Asked(Show)!.Possible);
        Assert.Equal(("a", "Source A"), book.SourceFor(Show));
    }

    [Fact]
    public void A_match_answers_the_question_and_says_nothing_unless_a_pick_was_made()
    {
        var book = new MatchBook();
        book.Take(Ladder(Candidate("a:1", "One")));
        book.Take(Matched(false));
        Assert.False(book.Asks(Show));
        Assert.True(book.IsKnown(Show));
        Assert.Null(book.Settled);
    }

    [Fact]
    public void No_match_keeps_the_question_and_is_known()
    {
        var book = new MatchBook();
        book.Take(Ladder(Candidate("a:1", "One")));
        book.Take(new Event.TitleMatch(Show, null));
        Assert.True(book.Asks(Show));
        Assert.True(book.IsKnown(Show));
        Assert.Null(book.Current(Show));
    }

    [Fact]
    public void A_search_answer_for_an_old_query_is_not_shown()
    {
        var book = new MatchBook();
        book.Searching(Show, "second");
        book.Take(Found("first", Candidate("a:1", "One")));
        Assert.IsType<MatchSearch.Searching>(book.Search(Show));
        book.Take(Found("second", Candidate("a:1", "One")));
        Assert.IsType<MatchSearch.Found>(book.Search(Show));
        book.Searching(Show, "third");
        book.Take(Found("third"));
        Assert.IsType<MatchSearch.Empty>(book.Search(Show));
    }

    [Fact]
    public void A_pick_settles_when_the_title_is_matched()
    {
        var book = new MatchBook();
        var pick = Candidate("a:1", "One");
        book.Searching(Show, "one");
        book.Pick(Show, pick, "Source A");
        Assert.Equal("a:1", book.Picking(Show));
        book.Take(Matched(true));
        Assert.Null(book.Picking(Show));
        Assert.Equal(("Made Up Show", "Source A"), (book.Settled!.Title, book.Settled.Source));
        Assert.Null(book.Search(Show));
        var first = book.Settled.Serial;
        book.Pick(Show, pick, "Source A");
        book.Take(Matched(true));
        Assert.NotEqual(first, book.Settled!.Serial);
    }

    [Fact]
    public void A_pick_that_failed_says_why_and_a_new_search_forgets_it()
    {
        var book = new MatchBook();
        book.Pick(Show, Candidate("a:1", "One"), "Source A");
        book.Take(new Event.MatchFixFailed(Show, new Msg.NoticeMatchDidNotStick()));
        Assert.Null(book.Picking(Show));
        Assert.NotNull(book.PickFailed(Show));
        book.Searching(Show, "again");
        Assert.Null(book.PickFailed(Show));
    }

    [Fact]
    public void Matching_automatically_forgets_the_choice_at_once()
    {
        var book = new MatchBook();
        book.Take(Matched(true));
        book.Cleared(Show);
        Assert.Null(book.Current(Show));
        Assert.True(book.IsKnown(Show));
    }

    [Fact]
    public void Closing_the_picker_forgets_its_search()
    {
        var book = new MatchBook();
        book.Searching(Show, "x");
        book.Closed(Show);
        Assert.Null(book.Search(Show));
    }

    [Fact]
    public void A_row_says_its_facts_and_how_close_it_is_or_why_not()
    {
        var near = Candidate("a:1", "One");
        Assert.Equal("TV · 2024 · 12 episodes", MatchWords.Facts(near));
        Assert.Contains("71", MatchWords.Closeness(near));
        Assert.Equal(T.MatchRuledOutYear, MatchWords.Closeness(Candidate("a:2", "Two", ruledOut: true)));
        Assert.Equal("", MatchWords.Facts(near with { Year = null, Episodes = null, Format = null }));
    }

    [Fact]
    public void The_pages_line_counts_what_could_be_it()
    {
        Assert.Equal(T.MatchUncertainNone("Source A"), MatchWords.Uncertain(new MatchAsked("a", "Source A", [])));
        Assert.Equal(T.MatchUncertain(1, "Source A"), MatchWords.Uncertain(new MatchAsked("a", "Source A", [Candidate("a:1", "One")])));
    }

    [Fact]
    public void Fixing_needs_a_source_in_use()
    {
        Assert.False(MatchBook.CanFix([]));
    }
}
