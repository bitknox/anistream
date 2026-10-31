// Fixing a source match: what the match picker shows. The question a source could not answer for
// a title, what a search of the source by hand found, the pick on its way, and what the title is
// matched to now. The same rules as the Mac's and the phones' match sheets.

using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>The titles a source offered for a show when it could not choose between them.</summary>
/// <param name="ProviderId">The source.</param>
/// <param name="Source">The source's name, as a person reads it.</param>
/// <param name="Candidates">What it offered, best first.</param>
public sealed record MatchAsked(string ProviderId, string Source, MatchCandidate[] Candidates)
{
    /// <summary>The candidates the ladder did not rule out, which the page's line counts.</summary>
    public int Possible => Candidates.Count(c => c.Rejected is null);
}

/// <summary>A search of a source by hand, from the picker's field.</summary>
public abstract record MatchSearch(string Query)
{
    public sealed record Searching(string Query) : MatchSearch(Query);

    /// <summary>What the source found; <paramref name="ProviderId"/> is the one to pin.</summary>
    public sealed record Found(string Query, string ProviderId, string Source, MatchCandidate[] Candidates) : MatchSearch(Query);

    /// <summary>The source answered with nothing.</summary>
    public sealed record Empty(string Query, string Source) : MatchSearch(Query);

    /// <summary>The source could not be searched; <paramref name="Reason"/> is the engine's, worded.</summary>
    public sealed record Failed(string Query, string Reason) : MatchSearch(Query);
}

/// <summary>A pick that has settled, for the notice that says it and the picker that closes on it.</summary>
/// <param name="Serial">Tells two of the same picks apart.</param>
public sealed record MatchSettled(uint Id, string Title, string Source, int Serial);

/// <summary>The match picker's state per title, kept from the engine's events. Only the UI thread touches it.</summary>
public sealed class MatchBook
{
    readonly Dictionary<uint, MatchAsked> asked = [];
    readonly Dictionary<uint, CurrentMatch> current = [];
    readonly HashSet<uint> known = [];
    readonly Dictionary<uint, MatchSearch> searches = [];
    readonly Dictionary<uint, string> picking = [];
    readonly Dictionary<uint, string> pickFailed = [];
    readonly Dictionary<uint, (string Title, string Source)> pickedTitle = [];
    int serial;

    /// <summary>The newest pick that settled.</summary>
    public MatchSettled? Settled { get; private set; }

    /// <summary>Whether the page for <paramref name="id"/> asks for its match in place of Play.</summary>
    public bool Asks(uint id) => asked.ContainsKey(id);

    public MatchAsked? Asked(uint id) => asked.GetValueOrDefault(id);

    /// <summary>What the title is matched to now; null when to nothing, or before the engine has said.</summary>
    public CurrentMatch? Current(uint id) => current.GetValueOrDefault(id);

    /// <summary>Whether the engine has said what the title is matched to, matched or not.</summary>
    public bool IsKnown(uint id) => known.Contains(id);

    public MatchSearch? Search(uint id) => searches.GetValueOrDefault(id);

    /// <summary>The key picked, while the source lists its episodes.</summary>
    public string? Picking(uint id) => picking.GetValueOrDefault(id);

    /// <summary>Why the last pick could not be used.</summary>
    public string? PickFailed(uint id) => pickFailed.GetValueOrDefault(id);

    /// <summary>Whether there is a source to match a title on, for Fix this match.</summary>
    public static bool CanFix(IEnumerable<ProviderRow> providers) => providers.Any(p => p.Usable);

    /// <summary>The source the picker for <paramref name="id"/> searches: the one that asked, else the one matched now, else the one a search found on.</summary>
    public (string ProviderId, string Name)? SourceFor(uint id)
    {
        if (asked.TryGetValue(id, out var a))
            return (a.ProviderId, a.Source);
        if (current.TryGetValue(id, out var c))
            return (c.ProviderId, c.Source);
        return searches.GetValueOrDefault(id) is MatchSearch.Found f ? (f.ProviderId, f.Source) : null;
    }

    public void Take(Event e)
    {
        switch (e)
        {
            case Event.MatchChoices m when m.Answers is MatchQuestion.Ladder:
                asked[m.Id] = new MatchAsked(m.ProviderId, m.Source, m.Candidates);
                break;
            case Event.MatchChoices { Answers: MatchQuestion.Search s } m:
                // An answer to a search the picker has moved on from is not shown.
                if (searches.GetValueOrDefault(m.Id)?.Query != s.Query)
                    break;
                searches[m.Id] = m.Candidates.Length == 0
                    ? new MatchSearch.Empty(s.Query, m.Source)
                    : new MatchSearch.Found(s.Query, m.ProviderId, m.Source, m.Candidates);
                break;
            case Event.MatchSearchFailed f:
                if (searches.GetValueOrDefault(f.Id)?.Query == f.Query)
                    searches[f.Id] = new MatchSearch.Failed(f.Query, f.Reason.Text());
                break;
            case Event.MatchFixFailed f:
                picking.Remove(f.Id);
                pickedTitle.Remove(f.Id);
                pickFailed[f.Id] = f.Reason.Text();
                break;
            case Event.TitleMatch t:
                Matched(t.Id, t.Current);
                break;
        }
    }

    void Matched(uint id, CurrentMatch? now)
    {
        known.Add(id);
        if (now is null)
        {
            current.Remove(id);
            return;
        }
        current[id] = now;
        // Matched: the question is answered.
        asked.Remove(id);
        if (picking.Remove(id))
        {
            pickedTitle.Remove(id, out var said);
            Settled = new MatchSettled(id, now.Title ?? said.Title ?? "", now.Source, ++serial);
            searches.Remove(id);
        }
    }

    /// <summary>The picker's field searched <paramref name="query"/>.</summary>
    public void Searching(uint id, string query)
    {
        searches[id] = new MatchSearch.Searching(query);
        pickFailed.Remove(id);
    }

    /// <summary><paramref name="candidate"/> picked on <paramref name="source"/>, until the engine says the title is matched.</summary>
    public void Pick(uint id, MatchCandidate candidate, string source)
    {
        picking[id] = candidate.Key;
        pickFailed.Remove(id);
        pickedTitle[id] = (candidate.Title, source);
    }

    /// <summary>The person let the engine match the title again: what they chose is forgotten at once.</summary>
    public void Cleared(uint id)
    {
        current.Remove(id);
        searches.Remove(id);
        picking.Remove(id);
        pickFailed.Remove(id);
    }

    /// <summary>The picker for the title closed: a search or failure it showed is not kept for the next one.</summary>
    public void Closed(uint id)
    {
        searches.Remove(id);
        pickFailed.Remove(id);
    }
}

/// <summary>What the picker's rows and the page's line say.</summary>
public static class MatchWords
{
    /// <summary>"TV · 2024 · 12 episodes", from what the source lists.</summary>
    public static string Facts(MatchCandidate c) => string.Join(" · ", new[]
    {
        c.Format is { } f ? Searches.FormatName(f) : null,
        c.Year?.ToString(),
        c.Episodes is { } n ? T.EpisodesCount(n) : null,
    }.Where(p => p is not null));

    /// <summary>How close it is, "71% match", or why it was ruled out.</summary>
    public static string Closeness(MatchCandidate c) =>
        c.Rejected is { } why ? why.Text() : T.MatchSimilarity(Wording.Percent(c.Similarity * 100));

    /// <summary>What AniList says of the show: "On AniList: TV · 2024 · 12 episodes"; empty with no facts.</summary>
    public static string AniList(Entry entry)
    {
        var facts = string.Join(" · ", new[]
        {
            entry.Format is { } f ? Wording.Format(f) : null,
            entry.Year?.ToString(),
            entry.Episodes is > 0 and var n ? T.EpisodesCount(n) : null,
        }.Where(p => p is not null));
        return facts.Length == 0 ? "" : T.MatchAnilistFacts(facts);
    }

    /// <summary>The page's line while the match is open: how many titles could be it.</summary>
    public static string Uncertain(MatchAsked asked) =>
        asked.Possible == 0 ? T.MatchUncertainNone(asked.Source) : T.MatchUncertain(asked.Possible, asked.Source);
}
