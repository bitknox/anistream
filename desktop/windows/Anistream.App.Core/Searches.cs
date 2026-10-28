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

    /// <summary>
    /// What is typed with the Search page's filters. With nothing typed it is a browse, which the
    /// engine answers popular first unless a sort is picked.
    /// </summary>
    public static SearchQuery With(string text, SearchFilters filters) =>
        Of(text, filters.Sort, filters.Season, filters.Year is { } y ? new Bounds(Min: y, Max: y) : null) with
        {
            Genres = filters.Genres.Length == 0 ? [] : [.. filters.Genres],
            Formats = filters.Formats.Length == 0 ? [] : [.. filters.Formats],
        };

    /// <summary>
    /// Whether two queries ask the same thing. A query read back from the engine carries new
    /// arrays, which a record's own equality compares by reference.
    /// </summary>
    public static bool Same(SearchQuery a, SearchQuery b) =>
        a.Text == b.Text && a.Sort == b.Sort && a.Season == b.Season && a.MinTagRank == b.MinTagRank
        && a.MinScore == b.MinScore && a.MinPopularity == b.MinPopularity && a.LicensedOnly == b.LicensedOnly
        && a.OnList == b.OnList
        && Same(a.Years, b.Years) && Same(a.Episodes, b.Episodes) && Same(a.Minutes, b.Minutes)
        && Same(a.Genres, b.Genres) && Same(a.GenresExcluded, b.GenresExcluded)
        && Same(a.Tags, b.Tags) && Same(a.TagsExcluded, b.TagsExcluded)
        && Same(a.Formats, b.Formats) && Same(a.Statuses, b.Statuses) && Same(a.Sources, b.Sources)
        && Same(a.Countries, b.Countries) && Same(a.StreamingOn, b.StreamingOn);

    /// <summary>Whether two lists' views are the same, searches by what they ask.</summary>
    public static bool Same(ContentFor a, ContentFor b) =>
        a is ContentFor.Search x && b is ContentFor.Search y ? Same(x.Query, y.Query) : a == b;

    static bool Same<T>(T[]? a, T[]? b) => (a ?? []).SequenceEqual(b ?? []);

    static bool Same(Bounds? a, Bounds? b) => (a?.Min, a?.Max) == (b?.Min, b?.Max);

    /// <summary>AniList's genres, for the Genre filter until the engine's own list arrives.</summary>
    public static readonly string[] Genres =
    [
        "Action", "Adventure", "Comedy", "Drama", "Ecchi", "Fantasy", "Horror", "Mahou Shoujo", "Mecha", "Music",
        "Mystery", "Psychological", "Romance", "Sci-Fi", "Slice of Life", "Sports", "Supernatural", "Thriller",
    ];

    /// <summary>A genre as the filter shows it, in the app's language. AniList's name is what a search sends; a genre the catalogue has no word for shows as it came.</summary>
    public static string GenreName(string genre) => genre switch
    {
        "Action" => T.GenreAction,
        "Adventure" => T.GenreAdventure,
        "Comedy" => T.GenreComedy,
        "Drama" => T.GenreDrama,
        "Ecchi" => T.GenreEcchi,
        "Fantasy" => T.GenreFantasy,
        "Horror" => T.GenreHorror,
        "Mahou Shoujo" => T.GenreMahouShoujo,
        "Mecha" => T.GenreMecha,
        "Music" => T.GenreMusic,
        "Mystery" => T.GenreMystery,
        "Psychological" => T.GenrePsychological,
        "Romance" => T.GenreRomance,
        "Sci-Fi" => T.GenreSciFi,
        "Slice of Life" => T.GenreSliceOfLife,
        "Sports" => T.GenreSports,
        "Supernatural" => T.GenreSupernatural,
        "Thriller" => T.GenreThriller,
        _ => genre,
    };

    /// <summary>A format as the Format filter names it: "TV", "Movie", "OVA".</summary>
    public static string FormatName(MediaFormat format) => format switch
    {
        MediaFormat.Tv => T.FormatTv,
        MediaFormat.TvShort => T.FormatTvShort,
        MediaFormat.Movie => T.FormatMovie,
        MediaFormat.Special => T.FormatSpecial,
        MediaFormat.Ova => T.FormatOva,
        MediaFormat.Ona => T.FormatOna,
        _ => T.FormatMusic,
    };

    /// <summary>A sort as the Sort filter names it.</summary>
    public static string SortName(SearchSort sort) => sort switch
    {
        SearchSort.Relevance => T.SortRelevance,
        SearchSort.Trending => T.SearchSortTrending,
        SearchSort.Popularity => T.SortPopularity,
        SearchSort.Score => T.SortScore,
        SearchSort.Newest => T.SearchSortNewest,
        SearchSort.Oldest => T.SearchSortOldest,
        SearchSort.Title => T.SearchSortTitle,
        _ => T.SearchSortFavourites,
    };

    /// <summary>The formats the Format filter offers, by the engine's names for them.</summary>
    public static readonly MediaFormat[] Formats =
        [MediaFormat.Tv, MediaFormat.Movie, MediaFormat.Ona, MediaFormat.Ova, MediaFormat.Special, MediaFormat.TvShort];

    /// <summary>The sorts the Sort filter offers beside the default (best match, or popular when browsing).</summary>
    public static readonly SearchSort[] Sorts =
        [SearchSort.Popularity, SearchSort.Trending, SearchSort.Score, SearchSort.Newest, SearchSort.Title];

    /// <summary>The years the Year filter offers: next year back to 1970, newest first.</summary>
    public static uint[] Years(int thisYear) => [.. Enumerable.Range(1970, thisYear - 1970 + 2).Reverse().Select(y => (uint)y)];

    static SearchQuery Of(string text, SearchSort? sort = null, Season? season = null, Bounds? years = null) => new(
        Text: text,
        Sort: sort,
        Genres: [],
        GenresExcluded: [],
        Tags: [],
        TagsExcluded: [],
        Formats: [],
        Statuses: [],
        Season: season,
        Years: years ?? new Bounds(Min: null, Max: null),
        Episodes: new Bounds(Min: null, Max: null),
        Minutes: new Bounds(Min: null, Max: null),
        Sources: [],
        Countries: [],
        StreamingOn: []);
}

/// <summary>The Search page's filter row: what each drop-down has set.</summary>
public sealed record SearchFilters(string[] Genres, MediaFormat[] Formats, Season? Season, uint? Year, SearchSort? Sort)
{
    public static readonly SearchFilters None = new([], [], null, null, null);

    /// <summary>Nothing narrows the search, so an empty box shows the before-typing page.</summary>
    public bool Narrows => Genres.Length > 0 || Formats.Length > 0 || Season is not null || Year is not null;

    public bool IsEmpty => !Narrows && Sort is null;

    /// <summary>The genres with <paramref name="genre"/> added or taken out, kept in the list's order.</summary>
    public SearchFilters Toggle(string genre, IReadOnlyList<string> order) => this with
    {
        Genres = Genres.Contains(genre)
            ? [.. Genres.Where(g => g != genre)]
            : [.. order.Where(g => g == genre || Genres.Contains(g))],
    };

    public SearchFilters Toggle(MediaFormat format) => this with
    {
        Formats = Formats.Contains(format)
            ? [.. Formats.Where(f => f != format)]
            : [.. Searches.Formats.Where(f => f == format || Formats.Contains(f))],
    };

    /// <summary>What a drop-down says: its name when unset, else what it has, "Action, Drama" or "Action +2".</summary>
    public static string Say(string name, IReadOnlyList<string> picked) => picked.Count switch
    {
        0 => name,
        1 => picked[0],
        2 => T.FilterPickedTwo(picked[0], picked[1]),
        _ => T.FilterPickedMore(picked[0], picked.Count - 1),
    };
}
