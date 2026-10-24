using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>A row of the queue: what the model says it plays, and its cover.</summary>
public sealed class QueueRow
{
    internal QueueRow(QueueItem item, CoverItem cover)
    {
        Item = item;
        Cover = cover;
    }

    public QueueItem Item { get; }
    public CoverItem Cover { get; }
    public string Name => $"{Item.Entry.Title}, {Item.Action}";
    public string MenuPlay => Item.Resume is not null ? "Resume" : "Play";
    public Visibility FromStartShown => Item.Resume is not null ? Visibility.Visible : Visibility.Collapsed;
}

public sealed class QueueGroup(string key, IEnumerable<QueueRow> rows) : List<QueueRow>(rows)
{
    public string Key { get; } = key;
}

/// <summary>
/// Continue: the queue first, at most five rows under way, then what is new this week, each row
/// saying what it plays. Beside it the selected row's preview with the one amber action. The
/// queue's rules are the model's (<c>present::Queue</c>), the same the other apps read.
/// </summary>
public sealed partial class ContinuePage : Page, IPageKeys
{
    readonly Ctx ctx = Ctx.Current;
    QueueRow? selected;
    bool loaded;
    DateTimeOffset lastPick;

    public ContinuePage()
    {
        InitializeComponent();
        State.Placeholders = () => States.Rows(5, 36, 54);
        State.ActivityText = "Loading what you're watching";
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Send(new Command.LoadContinue());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        var again = false;
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Content { View: ContentFor.Continue } c when c.ContentValue is not ContentList.Loading:
                    loaded = true;
                    again = true;
                    break;
                case Event.Progress:
                case Event.Sync:
                    again = true;
                    break;
                case Event.Detail d when d.V1.Id == selected?.Item.Entry.Id:
                case Event.Episodes ep when ep.Id == selected?.Item.Entry.Id:
                    ShowPreview(selected);
                    break;
            }
        }
        if (again)
            Show();
    }

    void Show()
    {
        var known = ctx.Known;
        var entries = known.ListEntries(new ContentFor.Continue());
        var queue = AnistreamFfiMethods.ContinueQueue(entries, [.. known.HiddenFromContinue], Casing.Sentence);
        var groups = new List<QueueGroup>();
        if (queue.InProgress.Length > 0)
            groups.Add(new("In progress", queue.InProgress.Select(Row)));
        if (queue.NewThisWeek.Length > 0)
            groups.Add(new("New this week", queue.NewThisWeek.Select(Row)));

        var empty = groups.Count == 0;
        ShowSyncLine();
        if (!loaded)
        {
            State.Wait(hasContent: !empty);
            if (empty)
                return;
            Body.Visibility = Visibility.Visible;
        }
        else if (known.Lists.GetValueOrDefault(new ContentFor.Continue()) is ContentList.Failed failed && empty)
        {
            State.Failed(failed.V1, () => ctx.Send(new Command.LoadContinue()));
            return;
        }
        else if (empty)
        {
            State.Empty(States.Empty("Nothing watched yet", "Pick something from Seasonal, or search for a title.", "Browse Seasonal", () => ctx.Window.Go("seasonal")));
            return;
        }
        else
            State.Done();

        var keep = selected?.Item.Entry.Id;
        Groups.Source = groups;
        Queue.ItemsSource = Groups.View;
        var rows = groups.SelectMany(g => g).ToList();
        var pick = rows.FirstOrDefault(r => r.Item.Entry.Id == keep) ?? rows[0];
        Queue.SelectedItem = pick;
        ShowPreview(pick);
    }

    QueueRow Row(QueueItem item)
    {
        var cover = new CoverItem(item.Entry);
        cover.PropertyChanged += (_, p) =>
        {
            if (p.PropertyName == nameof(CoverItem.Banner) && selected?.Cover == cover)
                BannerImage.Source = cover.Banner;
        };
        return new(item, cover.Load(ctx.Art, withBanner: true));
    }

    void ShowSyncLine()
    {
        var waiting = ctx.Known.Anilist?.Outbox ?? 0;
        SyncLine.Text = waiting > 0 ? $"{AnistreamFfiMethods.Plural(waiting, "change")} waiting to sync" : "";
        SyncLine.Visibility = waiting > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (Queue.SelectedItem is QueueRow row && row != selected)
        {
            // The preview crossfades in, unless the selection is still moving (M2).
            var now = DateTimeOffset.UtcNow;
            if (selected is not null && now - lastPick > TimeSpan.FromMilliseconds(150))
                Motion.FadeIn(PreviewBody);
            lastPick = now;
            ShowPreview(row);
            ctx.Send(new Command.LoadDetail(row.Item.Entry.Id));
            ctx.Send(new Command.LoadEpisodes(row.Item.Entry.Id));
        }
    }

    void ShowPreview(QueueRow? row)
    {
        selected = row;
        if (row is null)
            return;
        var item = row.Item;
        var entry = ctx.Known.Entry(item.Entry.Id) ?? item.Entry;
        BannerImage.Source = row.Cover.Banner;
        PreviewTitle.Text = entry.Title;
        var episodeTitle = ctx.Known.Episodes.TryGetValue(entry.Id, out var rows)
            ? rows.FirstOrDefault(r => r.Number == item.Episode.ToString())?.Title
            : null;
        EpisodeLine.Text = episodeTitle is { Length: > 0 } ? $"Episode {item.Episode} · {episodeTitle}" : $"Episode {item.Episode}";
        Meter.Visibility = item.Fraction is not null ? Visibility.Visible : Visibility.Collapsed;
        EpisodeProgress.Value = item.Fraction ?? 0;
        LeftLine.Text = item.Left ?? "";
        PlayButton.Content = item.Button;
        PlayButton.Visibility = item.Button is not null ? Visibility.Visible : Visibility.Collapsed;
        WaitingLine.Text = item.Waiting ?? "";
        WaitingLine.Visibility = item.Waiting is not null ? Visibility.Visible : Visibility.Collapsed;
        Synopsis.Text = entry.Synopsis;
    }

    void OnBannerSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var height = Math.Min(360, e.NewSize.Width * 9 / 16);
        if (double.IsNaN(Banner.Height) || Math.Abs(Banner.Height - height) > 0.5)
            Banner.Height = height;
    }

    const double PreviewWidth = 780;

    void OnBodySizeChanged(object sender, SizeChangedEventArgs e)
    {
        // The page's padding counts: the brief measures the content area.
        var wide = e.NewSize.Width + 72 >= PreviewWidth;
        QueueColumn.Width = wide ? new GridLength(360) : new GridLength(1, GridUnitType.Star);
        PreviewColumn.Width = wide ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
        Preview.Visibility = wide ? Visibility.Visible : Visibility.Collapsed;
        Body.ColumnSpacing = wide ? 32 : 0;
        Queue.IsItemClickEnabled = !wide;
    }

    void OnQueueItemClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is QueueRow row)
            ctx.OpenTitle(row.Item.Entry.Id);
    }

    void Play(QueueRow? row)
    {
        if (row is { Item.Playable: true })
            ctx.Play(row.Item.Entry.Id, row.Item.Episode.ToString());
    }

    void OnPlay(object sender, RoutedEventArgs e) => Play(selected);

    void OnBannerTapped(object sender, TappedRoutedEventArgs e)
    {
        if (selected is not null)
            ctx.OpenTitle(selected.Item.Entry.Id);
    }

    void OnQueueDoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        if (selected is not null)
            ctx.OpenTitle(selected.Item.Entry.Id);
    }

    void OnQueueKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == global::Windows.System.VirtualKey.Enter)
        {
            Play(selected);
            e.Handled = true;
        }
    }

    static QueueRow? Of(object sender) => (sender as FrameworkElement)?.Tag as QueueRow;

    void OnMenuPlay(object sender, RoutedEventArgs e) => Play(Of(sender));

    void OnMenuFromStart(object sender, RoutedEventArgs e)
    {
        if (Of(sender) is { Item.Playable: true } row)
            ctx.Play(row.Item.Entry.Id, row.Item.Episode.ToString(), fromStart: true);
    }

    void OnMenuWatched(object sender, RoutedEventArgs e) => MarkWatched(Of(sender));

    void MarkWatched(QueueRow? row)
    {
        if (row is not null)
            ctx.Send(new Command.SetWatched(row.Item.Entry.Id, [row.Item.Episode.ToString()], true));
    }

    void OnMenuHide(object sender, RoutedEventArgs e) => Hide(Of(sender));

    void Hide(QueueRow? row)
    {
        if (row is null)
            return;
        ctx.Known.HiddenFromContinue.Add(row.Item.Entry.Id);
        ctx.Send(new Command.HideFromContinue(row.Item.Entry.Id));
        Show();
    }

    // Ctrl+Shift+M and Delete, from the window, on the selected row.
    bool IPageKeys.MarkWatched()
    {
        MarkWatched(selected);
        return selected is not null;
    }

    bool IPageKeys.Delete()
    {
        Hide(selected);
        return selected is not null;
    }

    void OnMenuDrop(object sender, RoutedEventArgs e)
    {
        if (Of(sender) is { } row)
            ctx.Send(new Command.SetStatus(row.Item.Entry.Id, LibrarySegment.Dropped));
    }

    void OnMenuOpen(object sender, RoutedEventArgs e)
    {
        if (Of(sender) is { } row)
            ctx.OpenTitle(row.Item.Entry.Id);
    }

}

/// <summary>The window's Ctrl+Shift+M and Delete, for the page that has something selected.</summary>
interface IPageKeys
{
    bool MarkWatched();
    bool Delete() => false;
}
