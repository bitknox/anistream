using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>Search queries as the apps ask them. The bindings leave a record's other fields unset, and the ones that are records of their own must be filled in.</summary>
public static class Searches
{
    /// <summary>A search for these words, with no filter.</summary>
    public static SearchQuery Text(string text) => Of(text);

    /// <summary>One season of one year, most popular first.</summary>
    public static SearchQuery InSeason(Season season, uint year) =>
        Of("", SearchSort.Popularity, season, new Bounds(Min: year, Max: year));

    static SearchQuery Of(string text, SearchSort? sort = null, Season? season = null, Bounds? years = null) => new(
        Text: text,
        Sort: sort,
        Season: season,
        Years: years ?? new Bounds(Min: null, Max: null),
        Episodes: new Bounds(Min: null, Max: null),
        Minutes: new Bounds(Min: null, Max: null));
}
