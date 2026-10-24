using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Seasonal: a season's titles as a poster grid, popular first. The current season is the
/// engine's own Seasonal list; any other is a search by season and year, as on Linux.
/// </summary>
public sealed partial class SeasonalPage : Page
{
    const int Ahead = 1, Back = 6;

    readonly Ctx ctx = Ctx.Current;
    readonly SeasonYear now = AnistreamFfiMethods.SeasonAt(DateTimeOffset.UtcNow.ToUnixTimeSeconds());
    SeasonYear picked;
    Entry[] entries = [];
    string[] keep = [];

    public SeasonalPage()
    {
        InitializeComponent();
        picked = now;
        for (var step = Ahead; step >= -Back; step--)
        {
            var season = AnistreamFfiMethods.SeasonStep(now, step);
            var item = new RadioMenuFlyoutItem { Text = AnistreamFfiMethods.SeasonName(season), GroupName = "season", Tag = season };
            item.Click += (_, _) => Pick(season);
            SeasonMenu.Items.Add(item);
        }
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        Pick(picked);
    }

    static SearchQuery Query(SeasonYear season) => new(
        Season: season.Season, Years: new Bounds(Min: (uint)season.Year, Max: (uint)season.Year), Sort: SearchSort.Popularity);

    void Pick(SeasonYear season)
    {
        picked = season;
        SeasonPicker.Content = AnistreamFfiMethods.SeasonName(season);
        foreach (var item in SeasonMenu.Items.OfType<RadioMenuFlyoutItem>())
            item.IsChecked = item.Tag is SeasonYear s && s == season;
        var first = SeasonMenu.Items.OfType<RadioMenuFlyoutItem>().First().Tag as SeasonYear;
        var last = SeasonMenu.Items.OfType<RadioMenuFlyoutItem>().Last().Tag as SeasonYear;
        Later.IsEnabled = season != first;
        Earlier.IsEnabled = season != last;
        if (season == now)
        {
            entries = ctx.Known.ListEntries(new ContentFor.Seasonal());
            ctx.Send(new Command.LoadSeasonal());
        }
        else
        {
            entries = ctx.Known.ListEntries(new ContentFor.Search(Query(season)));
            ctx.Send(new Command.Search(Query(season)));
        }
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is not Event.Content c)
                continue;
            var ours = c.View switch
            {
                ContentFor.Seasonal => picked == now,
                ContentFor.Search s => picked != now && s.Query == Query(picked),
                _ => false,
            };
            if (!ours || c.ContentValue is ContentList.Loading)
                continue;
            entries = ctx.Known.ListEntries(c.View);
            Empty.Text = c.ContentValue is ContentList.Failed f ? f.V1 : $"Nothing listed for {AnistreamFfiMethods.SeasonName(picked)} yet.";
            Show();
        }
    }

    void Show()
    {
        var shown = entries
            .Where(e => keep.Length == 0 || e.Format is { } f && keep.Contains(f.ToUpperInvariant()))
            .Select(e => new CoverItem(e).Load(ctx.Art))
            .ToList();
        Grid.Covers = shown;
        Empty.Visibility = shown.Count == 0 && Empty.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnFormatChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs e)
    {
        keep = sender.SelectedItem?.Tag is string tags && tags.Length > 0 ? tags.Split(',') : [];
        Show();
    }

    void OnEarlier(object sender, RoutedEventArgs e) => Pick(AnistreamFfiMethods.SeasonStep(picked, -1));

    void OnLater(object sender, RoutedEventArgs e) => Pick(AnistreamFfiMethods.SeasonStep(picked, 1));
}
