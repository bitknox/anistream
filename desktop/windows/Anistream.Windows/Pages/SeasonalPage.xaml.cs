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
    bool answered;

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
        State.Placeholders = () => States.Covers();
        State.ActivityText = "Loading this season";
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        Pick(picked);
    }

    static SearchQuery Query(SeasonYear season) => Anistream.App.Searches.InSeason(season.Season, (uint)season.Year);

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
        ContentFor view = season == now ? new ContentFor.Seasonal() : new ContentFor.Search(Query(season));
        entries = ctx.Known.ListEntries(view);
        answered = ctx.Known.List(view) is ContentList.Entries or ContentList.Empty;
        State.Wait(hasContent: entries.Length > 0);
        ctx.Send(season == now ? new Command.LoadSeasonal() : new Command.Search(Query(season)));
        if (entries.Length > 0 || answered)
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
                ContentFor.Search s => picked != now && Anistream.App.Searches.Same(s.Query, Query(picked)),
                _ => false,
            };
            if (!ours || c.ContentValue is ContentList.Loading)
                continue;
            if (c.ContentValue is ContentList.Failed f)
            {
                if (entries.Length == 0)
                    State.Failed(f.V1, () => Pick(picked));
                else
                    State.Done();
                continue;
            }
            entries = ctx.Known.ListEntries(c.View);
            answered = true;
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
        if (shown.Count > 0)
            State.Done();
        else if (answered)
            State.Empty(States.Unavailable(entries.Length == 0 ? $"Nothing listed for {AnistreamFfiMethods.SeasonName(picked)} yet" : "No titles in this format", entries.Length == 0 ? "AniList adds a season's titles as they're announced." : "Try All."));
    }

    void OnFormatChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs e)
    {
        keep = sender.SelectedItem?.Tag is string tags && tags.Length > 0 ? tags.Split(',') : [];
        Show();
    }

    void OnEarlier(object sender, RoutedEventArgs e) => Pick(AnistreamFfiMethods.SeasonStep(picked, -1));

    void OnLater(object sender, RoutedEventArgs e) => Pick(AnistreamFfiMethods.SeasonStep(picked, 1));
}
