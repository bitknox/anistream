using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Search, opened from the title bar's box: the results as a poster grid, or before typing, this
/// season's titles. Kept simple for now: the shared search query (feat/search) brings the filters.
/// </summary>
public sealed partial class SearchPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    string query = "";

    public SearchPage()
    {
        InitializeComponent();
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    static ContentFor View(string query) => new ContentFor.Search(new SearchQuery(Text: query));

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        query = e.Parameter as string ?? "";
        if (query.Length > 0)
            ctx.Send(new Command.Search(new SearchQuery(Text: query)));
        else if (!ctx.Known.Lists.ContainsKey(new ContentFor.Seasonal()))
            ctx.Send(new Command.LoadSeasonal());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Content c && (c.View == View(query) || query.Length == 0 && c.View is ContentFor.Seasonal)))
            Show();
    }

    void Show()
    {
        if (query.Length == 0)
        {
            Heading.Text = "Search";
            Section.Text = "This season";
            Section.Visibility = Visibility.Visible;
            Grid.Covers = [.. ctx.Known.ListEntries(new ContentFor.Seasonal()).Select(e => new CoverItem(e).Load(ctx.Art))];
            Empty.Visibility = Visibility.Collapsed;
            return;
        }
        Heading.Text = $"Results for “{query}”";
        Section.Visibility = Visibility.Collapsed;
        var found = ctx.Known.Lists.GetValueOrDefault(View(query));
        var entries = ctx.Known.ListEntries(View(query));
        Grid.Covers = [.. entries.Select(e => new CoverItem(e).Load(ctx.Art))];
        Empty.Text = found is ContentList.Failed f ? f.V1 : $"Nothing found for “{query}”";
        Empty.Visibility = found is ContentList.Entries or ContentList.Empty or ContentList.Failed && entries.Length == 0
            ? Visibility.Visible
            : Visibility.Collapsed;
    }
}
