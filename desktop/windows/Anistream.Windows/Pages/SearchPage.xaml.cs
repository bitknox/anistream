using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Search, opened from the title bar's box: the results as a poster grid under a row of filters.
/// With nothing typed and no filter set it shows this season's titles; a filter with nothing typed
/// browses, as the engine's query does with no text. The filters stay set for the session, so a
/// new search from the title bar keeps them.
/// </summary>
public sealed partial class SearchPage : Page
{
    static SearchFilters remembered = SearchFilters.None;

    readonly Ctx ctx = Ctx.Current;
    readonly DispatcherTimer settle = new() { Interval = TimeSpan.FromMilliseconds(250) };
    string query = "";
    SearchFilters filters = remembered;
    /// <summary>What was last asked, or null while the page shows this season.</summary>
    SearchQuery? asked;
    Entry[] entries = [];

    public SearchPage()
    {
        InitializeComponent();
        State.Placeholders = () => States.Covers();
        State.ActivityText = "Searching";
        settle.Tick += (_, _) =>
        {
            settle.Stop();
            Run();
        };
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) =>
        {
            ctx.Batch -= Heard;
            settle.Stop();
        };
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        query = e.Parameter as string ?? "";
#if ANISTREAM_TESTING
        // --show=browse: a genre and a sort set with nothing typed, for a screenshot.
        if (ctx.Options.Show == "browse")
            filters = SearchFilters.None with { Genres = ["Adventure"], Sort = SearchSort.Popularity };
        else if (ctx.Options.Show == "browse-none")
            filters = SearchFilters.None with { Genres = ["Horror"], Year = 1971 };
#endif
        if (ctx.Known.Facets is null)
            ctx.Send(new Command.LoadSearchFacets());
        BuildFilters();
        Run();
    }

    /// <summary>Asks the engine for what the box and the filters now say, or this season when they say nothing.</summary>
    void Run()
    {
        remembered = filters;
        ShowFilters();
        if (query.Length == 0 && !filters.Narrows && filters.Sort is null)
        {
            asked = null;
            Heading.Text = "Search";
            Section.Text = "This season";
            Section.Visibility = Visibility.Visible;
            entries = ctx.Known.ListEntries(new ContentFor.Seasonal());
            State.Wait(hasContent: entries.Length > 0);
            if (ctx.Known.List(new ContentFor.Seasonal()) is null or ContentList.Failed)
                ctx.Send(new Command.LoadSeasonal());
            Show();
            return;
        }
        asked = Searches.With(query, filters);
        Heading.Text = query.Length > 0 ? $"Results for “{query}”" : "Browse";
        Section.Visibility = Visibility.Collapsed;
        entries = ctx.Known.ListEntries(new ContentFor.Search(asked));
        State.Wait(hasContent: entries.Length > 0);
        ctx.Send(new Command.Search(asked));
        Show();
    }

    ContentFor View => asked is { } q ? new ContentFor.Search(q) : new ContentFor.Seasonal();

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is Event.Facets)
                BuildGenres();
            if (e is not Event.Content c || !Searches.Same(c.View, View) || c.ContentValue is ContentList.Loading)
                continue;
            entries = ctx.Known.ListEntries(View);
            Show();
        }
    }

    void Show()
    {
        Grid.Covers = [.. entries.Select(e => new CoverItem(e).Load(ctx.Art))];
        var list = ctx.Known.List(View);
        if (entries.Length > 0)
            State.Done();
        else if (list is ContentList.Failed f)
            State.Failed(f.V1, Run);
        else if (list is ContentList.Entries or ContentList.Empty)
            State.Empty(Nothing());
    }

    /// <summary>No results: the query repeated, and a way out when the filters narrowed it.</summary>
    UIElement Nothing()
    {
        if (asked is null)
            return States.Unavailable("Nothing listed for this season yet", "AniList adds a season's titles as they're announced.");
        if (!filters.Narrows)
            return States.Unavailable($"Nothing found for “{query}”", "Check the spelling, or try the English or Japanese title.");
        var title = query.Length > 0 ? $"Nothing found for “{query}” with these filters" : "Nothing found with these filters";
        return States.Unavailable(title, "Try fewer filters.", "Clear filters", Clear);
    }

    // The filter row.

    void BuildFilters()
    {
        BuildGenres();
        FormatList.Children.Clear();
        foreach (var format in Searches.Formats)
        {
            var box = new CheckBox { Content = AnistreamModelMethods.MediaFormatLabel(format), Tag = format, IsChecked = filters.Formats.Contains(format) };
            box.Click += (_, _) => Change(filters.Toggle(format));
            FormatList.Children.Add(box);
        }

        SeasonMenu.Items.Clear();
        SeasonMenu.Items.Add(Radio("Any season", "season", filters.Season is null, () => filters with { Season = null }));
        foreach (var season in Enum.GetValues<Season>())
            SeasonMenu.Items.Add(Radio(AnistreamModelMethods.SearchSeasonLabel(season), "season", filters.Season == season, () => filters with { Season = season }));

        YearMenu.Items.Clear();
        YearMenu.Items.Add(Radio("Any year", "year", filters.Year is null, () => filters with { Year = null }));
        foreach (var year in Searches.Years(DateTime.Now.Year))
            YearMenu.Items.Add(Radio(year.ToString(), "year", filters.Year == year, () => filters with { Year = year }));

        SortMenu.Items.Clear();
        SortMenu.Items.Add(Radio("Best match", "sort", filters.Sort is null, () => filters with { Sort = null }));
        foreach (var sort in Searches.Sorts)
            SortMenu.Items.Add(Radio(AnistreamModelMethods.SearchSortLabel(sort), "sort", filters.Sort == sort, () => filters with { Sort = sort }));
    }

    /// <summary>The genres: the engine's list once it has one, AniList's usual ones until then.</summary>
    void BuildGenres()
    {
        var genres = ctx.Known.Facets?.Genres is { Length: > 0 } g ? g : Searches.Genres;
        GenreList.Children.Clear();
        foreach (var genre in genres)
        {
            var box = new CheckBox { Content = genre, IsChecked = filters.Genres.Contains(genre) };
            box.Click += (_, _) => Change(filters.Toggle(genre, genres));
            GenreList.Children.Add(box);
        }
    }

    RadioMenuFlyoutItem Radio(string text, string group, bool on, Func<SearchFilters> then)
    {
        var item = new RadioMenuFlyoutItem { Text = text, GroupName = group, IsChecked = on };
        item.Click += (_, _) => Change(then());
        return item;
    }

    /// <summary>A filter changed: show it at once, search once the clicks stop.</summary>
    void Change(SearchFilters to)
    {
        filters = to;
        ShowFilters();
        settle.Stop();
        settle.Start();
    }

    /// <summary>Each drop-down says its name, or in amber what it has.</summary>
    void ShowFilters()
    {
        Say(GenreButton, SearchFilters.Say("Genre", filters.Genres), filters.Genres.Length > 0);
        Say(FormatButton, SearchFilters.Say("Format", [.. filters.Formats.Select(AnistreamModelMethods.MediaFormatLabel)]), filters.Formats.Length > 0);
        Say(SeasonButton, filters.Season is { } s ? AnistreamModelMethods.SearchSeasonLabel(s) : "Season", filters.Season is not null);
        Say(YearButton, filters.Year?.ToString() ?? "Year", filters.Year is not null);
        Say(SortButton, filters.Sort is { } o ? AnistreamModelMethods.SearchSortLabel(o) : "Sort", filters.Sort is not null);
        if (filters.IsEmpty)
            Motion.Hide(ClearButton);
        else
            Motion.Show(ClearButton);
    }

    static void Say(DropDownButton button, string text, bool set)
    {
        button.Content = text;
        if (set)
            button.Foreground = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["AccentTextFillColorTertiaryBrush"];
        else
            button.ClearValue(Control.ForegroundProperty);
    }

    void Clear()
    {
        filters = SearchFilters.None;
        BuildFilters();
        Run();
    }

    void OnClear(object sender, RoutedEventArgs e) => Clear();
}
