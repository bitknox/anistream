using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// A row of the list: a queue row says what plays, a suggestion says why it is there. The same
/// height, cover and selection; a suggestion has no progress bar.
/// </summary>
public sealed class QueueRow
{
    internal QueueRow(QueueItem item, CoverItem cover)
    {
        Item = item;
        Cover = cover;
        Lead = item.Action;
        Name = $"{item.Entry.Title}, {item.Action}";
    }

    internal QueueRow(UpNextItem suggestion, CoverItem cover, UpNextLine line)
    {
        Suggestion = suggestion;
        Cover = cover;
        Lead = line.Lead;
        Tail = line.Tail;
        Name = $"{suggestion.Entry.Title}, {line}";
    }

    /// <summary>What the queue row plays; null for a suggestion.</summary>
    public QueueItem? Item { get; }

    /// <summary>The suggestion; null for a queue row.</summary>
    public UpNextItem? Suggestion { get; }

    public CoverItem Cover { get; }
    public Entry Entry => Item?.Entry ?? Suggestion!.Entry;
    public string Lead { get; }
    public string Tail { get; } = "";
    public string Name { get; }
    public bool Playable => Item?.Playable ?? false;
    public double Fraction => Item?.Fraction ?? 0;
    public Visibility MeterShown => Item?.Fraction is not null ? Visibility.Visible : Visibility.Collapsed;
    public string MenuPlay => Item?.Resume is not null ? T.ContinueMenuResume : T.ActionPlay;
    public Visibility FromStartShown => Item?.Resume is not null ? Visibility.Visible : Visibility.Collapsed;
    public Visibility PlayShown => Suggestion?.Action == UpNextAction.PlayFirstEpisode ? Visibility.Visible : Visibility.Collapsed;
    public Visibility PlanShown => Suggestion?.Action == UpNextAction.AddToPlanning ? Visibility.Visible : Visibility.Collapsed;
}

public sealed class QueueGroup(string key, IEnumerable<QueueRow> rows, bool suggestions = false) : List<QueueRow>(rows)
{
    public string Key { get; } = key;
    public bool Suggestions { get; } = suggestions;

    /// <summary>Space over the heading, but for the first: the groups are one list with a seam.</summary>
    public Thickness Gap { get; set; }
}

/// <summary>The queue's row template or the suggestion's, by what the row is.</summary>
public sealed class QueueTemplates : DataTemplateSelector
{
    public DataTemplate? Queue { get; set; }
    public DataTemplate? Suggestion { get; set; }

    protected override DataTemplate? SelectTemplateCore(object item) =>
        item is QueueRow { Suggestion: not null } ? Suggestion : Queue;

    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container) => SelectTemplateCore(item);
}

/// <summary>A title on the welcome: a card for what airs today, a poster for this season.</summary>
public sealed class WelcomeCard : System.ComponentModel.INotifyPropertyChanged
{
    ImageSource? art;

    internal WelcomeCard(CoverItem cover, string line, bool dot)
    {
        Cover = cover;
        Line = line;
        DotShown = dot ? Visibility.Visible : Visibility.Collapsed;
        Name = line.Length > 0 ? $"{cover.Title}, {line}" : cover.Title;
        art = cover.Banner ?? cover.Cover;
        cover.PropertyChanged += (_, _) =>
        {
            art = cover.Banner ?? cover.Cover;
            PropertyChanged?.Invoke(this, new(nameof(Art)));
        };
    }

    public CoverItem Cover { get; }
    public string Line { get; }
    public string Name { get; }
    public Visibility DotShown { get; }
    public ImageSource? Art => art;

    public event System.ComponentModel.PropertyChangedEventHandler? PropertyChanged;
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
#if ANISTREAM_TESTING
    bool asked;
#endif
    bool loaded, welcomeAsked, hadSuggestions;
    DateTimeOffset lastPick;

    /// <summary>The title whose detail and episodes the preview asked for last.</summary>
    uint? detailAsked;

    /// <summary>Suggestions marked Not interested here, until Undo, so a refill cannot bring one back first.</summary>
    readonly HashSet<uint> dismissed = [];

    public ContinuePage()
    {
        InitializeComponent();
        State.Placeholders = () => States.Rows(5, 36, 54);
        State.ActivityText = T.ContinueLoading;
        // Heard from the start: what arrives before the page is loaded is not missed.
        // The queue's rows let go once the frame leaves: a ListView still holding its items
        // keeps the page alive after it has gone.
        ctx.Hear(this, Heard, () =>
        {
            Queue.ItemsSource = null;
            Groups.Source = null;
        });
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
#if ANISTREAM_TESTING
        foreach (var part in (ctx.Options.Hide ?? "").Split(',', StringSplitOptions.RemoveEmptyEntries))
            if (uint.TryParse(part, out var hide))
                ctx.Send(new Command.HideFromContinue(hide));
#endif
#if ANISTREAM_TESTING
        foreach (var id in FakeIds)
            if (ctx.Options.UpNext is not null)
                ctx.Send(new Command.LoadDetail(id));
#endif
        ctx.Send(new Command.LoadContinue());
        ctx.Send(new Command.LoadUpNext());
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
                case Event.Content { View: ContentFor.Calendar or ContentFor.Seasonal }:
                case Event.CalendarWindow:
                case Event.UpNext:
                case Event.Progress:
                case Event.Sync:
                    again = true;
                    break;
                case Event.Detail d when d.V1.Id == selected?.Entry.Id:
                case Event.Episodes ep when ep.Id == selected?.Entry.Id:
                    ShowPreview(selected);
                    break;
            }
#if ANISTREAM_TESTING
            // --up-next: the made-up suggestions wait on their titles' details. Without it the
            // details are the preview's own, and rebuilding on them would select, load and
            // rebuild again for ever.
            if (e is Event.Detail fake && faked is null && ctx.Options.UpNext is not null && FakeIds.Contains(fake.V1.Id))
                again = true;
#endif
            // A row plays the first episode after the next that a source lists, once its table is
            // known; when the table moves it, the queue is worked out again.
            if (e is Event.Episodes table && Moved(table.Id))
                again = true;
        }
        if (again)
            Show();
    }

    void Show()
    {
        var known = ctx.Known;
        var entries = known.ListEntries(new ContentFor.Continue());
        var queue = Wording.Said(AnistreamFfiMethods.ContinueQueue(entries, [.. known.HiddenFromContinue], Casing.Sentence));
        var groups = new List<QueueGroup>();
        if (queue.InProgress.Length > 0)
            groups.Add(new(T.GroupInProgress, queue.InProgress.Select(Row)));
        if (queue.NewThisWeek.Length > 0)
            groups.Add(new(T.GroupNewThisWeek, queue.NewThisWeek.Select(Row)));

        var empty = groups.Count == 0;
        // Up next for you follows the queue, never leads it, and never sits with the welcome.
        var sent = known.UpNext;
#if ANISTREAM_TESTING
        sent = Fake() ?? sent;
#endif
        var suggestions = empty ? [] : sent.Where(i => !dismissed.Contains(i.Entry.Id)).ToArray();
        if (suggestions.Length > 0)
            groups.Add(new(T.UpNextHeading, suggestions.Select(Suggestion), suggestions: true));
        for (var i = 1; i < groups.Count; i++)
            groups[i].Gap = new Thickness(0, 12, 0, 0);
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
            SetWelcome(false);
            State.Failed(failed.V1, () => ctx.Send(new Command.LoadContinue()));
            return;
        }
        else if (empty)
        {
            if (!welcomeAsked)
            {
                welcomeAsked = true;
                ctx.Send(new Command.LoadCalendar());
                ctx.Send(new Command.LoadSeasonal());
            }
            var (shown, pending) = BuildWelcome();
            if (shown)
            {
                SetWelcome(true);
                return;
            }
            SetWelcome(false);
            if (pending)
                State.Wait(hasContent: false);
            else
                State.Empty(States.Empty(T.ContinueEmptyTitle, T.ContinueEmptyDetail, T.ContinueEmptyAction, () => ctx.Window.Go("seasonal")));
            return;
        }
        else
            State.Done();
        SetWelcome(false);

        var keep = selected?.Entry.Id;
        var rows = groups.SelectMany(g => g).ToList();
#if ANISTREAM_TESTING
        // --select=<id>: that title's row once it is there, for a screenshot.
        if (!asked && uint.TryParse(ctx.Options.Select, out var wanted) && rows.Any(r => r.Entry.Id == wanted))
            (keep, asked) = (wanted, true);
#endif
        Groups.Source = groups;
        Queue.ItemsSource = Groups.View;
        var pick = rows.FirstOrDefault(r => r.Entry.Id == keep) ?? rows[0];
        Queue.SelectedItem = pick;
        ShowPreview(pick);
        // The group arrives as one: header and rows fade in together, nothing above moves.
        var arrived = suggestions.Length > 0 && !hadSuggestions;
        hadSuggestions = suggestions.Length > 0;
        if (arrived && Motion.Enabled)
            DispatcherQueue.TryEnqueue(FadeSuggestions);
    }

#if ANISTREAM_TESTING
    static readonly uint[] FakeIds = [900004, 900003, 900002, 900006, 900001];
    UpNextItem[]? faked;

    /// <summary>The testing build's made-up suggestions, once their titles are known.</summary>
    UpNextItem[]? Fake()
    {
        if (ctx.Options.UpNext is not { } mode)
            return null;
        if (faked is not null)
            return faked;
        var known = ctx.Known;
        if (FakeIds.Any(id => known.Entry(id) is null))
            return null;
        Entry E(uint id) => known.Entry(id)!;
        UpNextItem I(uint id, UpNextReason r, UpNextAction a, string? because = null, FuzzyDate? starts = null) =>
            new(E(id), r, a, because, starts);
        var play = UpNextAction.PlayFirstEpisode;
        var shown = mode;
        mode = mode.Split('+')[0];
        faked = mode switch
        {
            "signed-out" =>
            [
                I(900004, UpNextReason.NewSeason, play),
                I(900003, UpNextReason.NewSeason, UpNextAction.Nothing, starts: new FuzzyDate((ushort)(DateTime.Now.Year + 1), 1, 9)),
                I(900002, UpNextReason.Recommended, play, because: E(900001).Title),
            ],
            _ =>
            [
                I(900004, UpNextReason.NewSeason, play),
                I(900003, UpNextReason.NewSeason, UpNextAction.AddToPlanning, starts: new FuzzyDate((ushort)(DateTime.Now.Year + 1), 1, 9)),
                I(900006, UpNextReason.Planning, play),
                I(900002, UpNextReason.Recommended, play, because: E(900001).Title),
            ],
        };
        // "+dismiss" and "+menu": the notice after Not interested and the row's menu, for a screenshot.
        if (shown.Contains("+dismiss") || shown.Contains("+menu"))
            _ = Task.Delay(2500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                var row = ((IEnumerable<QueueGroup>?)Groups.Source ?? []).SelectMany(g => g).FirstOrDefault(r => r.Entry.Id == 900003);
                if (shown.Contains("+dismiss"))
                    Dismiss(row);
                else if (Queue.ContainerFromItem(row) is DependencyObject container)
                    Open(container);
            }));
        return faked;
    }

    static bool Open(DependencyObject at)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(at); i++)
        {
            var child = VisualTreeHelper.GetChild(at, i);
            if (child is FrameworkElement { ContextFlyout: { } flyout } element)
            {
                flyout.ShowAt(element);
                return true;
            }
            if (Open(child))
                return true;
        }
        return false;
    }
#endif

    void FadeSuggestions()
    {
        Queue.UpdateLayout();
        if (Queue.ItemsPanelRoot is not Panel panel)
            return;
        foreach (var child in panel.Children)
        {
            var content = (child as ContentControl)?.Content;
            if (content is QueueGroup { Suggestions: true } or QueueRow { Suggestion: not null })
                Motion.FadeIn(child);
        }
    }

    // The welcome: what airs today and this season, when nothing is watched yet.

    void SetWelcome(bool on)
    {
        if (on == (Welcome.Visibility == Visibility.Visible))
            return;
        State.Visibility = on ? Visibility.Collapsed : Visibility.Visible;
        if (on)
            Motion.FadeIn(Welcome);
        else
            Welcome.Visibility = Visibility.Collapsed;
    }

    /// <summary>Fills the welcome from the calendar and the season. Whether any of it shows, and whether either list is still on its way.</summary>
    (bool Shown, bool Pending) BuildWelcome()
    {
        var known = ctx.Known;
        var calendar = known.List(new ContentFor.Calendar());
        var season = known.List(new ContentFor.Seasonal());
        var pending = calendar is null or ContentList.Loading || season is null or ContentList.Loading;

        var cards = new List<WelcomeCard>();
        if (known.CalendarAnchor is { } anchor && calendar is ContentList.Entries)
        {
            var zone = TimeZoneInfo.Local;
            var now = DateTimeOffset.Now;
            DateOnly Day(long unix) => DateOnly.FromDateTime(TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeSeconds(unix), zone).DateTime);
            var today = Day(now.ToUnixTimeSeconds());
            foreach (var b in Calendar.Broadcasts(known.ListRows(new ContentFor.Calendar()), anchor, known, mine: false)
                         .Where(b => Day(b.Airs) == today).Take(8))
            {
                var out_ = b.Airs <= now.ToUnixTimeSeconds();
                var when = out_ ? T.UpNextOut(Wording.Ago(now.ToUnixTimeSeconds() - b.Airs)) : Calendar.Time(b.Airs, zone);
                cards.Add(new(new CoverItem(b.Entry).Load(ctx.Art, withBanner: true), T.UpNextBroadcast(b.Episode, when), out_ && !b.Watched));
            }
        }
        var posters = known.ListEntries(new ContentFor.Seasonal()).Take(12)
            .Select(e => new WelcomeCard(new CoverItem(e).Load(ctx.Art), "", false)).ToList();

        AiringCards.ItemsSource = cards;
        SeasonPosters.ItemsSource = posters;
        AiringSection.Visibility = cards.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        SeasonSection.Visibility = posters.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        return (cards.Count > 0 || posters.Count > 0, pending);
    }

    void OnWelcomeOpen(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is WelcomeCard card)
            ctx.OpenTitle(card.Cover.Id);
    }

    void OnWelcomeCalendar(object sender, RoutedEventArgs e) => ctx.Go("calendar");

    void OnWelcomeSeasonal(object sender, RoutedEventArgs e) => ctx.Go("seasonal");

    /// <summary>Whether the row for title <paramref name="id"/> now plays a different episode than it says.</summary>
    bool Moved(uint id)
    {
        foreach (var group in (IEnumerable<QueueGroup>?)Groups.Source ?? [])
            foreach (var row in group.Where(r => r.Item is not null && r.Entry.Id == id))
            {
                var now = PlayableNext.Item(row.Item!, ctx.Known.Episodes.GetValueOrDefault(id), Casing.Sentence);
                if (now.Episode != row.Item!.Episode || now.Playable != row.Item.Playable)
                    return true;
            }
        return false;
    }

    QueueRow Row(QueueItem item)
    {
        item = PlayableNext.Item(item, ctx.Known.Episodes.GetValueOrDefault(item.Entry.Id), Casing.Sentence);
        return new(item, Cover(item.Entry));
    }

    QueueRow Suggestion(UpNextItem item) =>
        new(item, Cover(item.Entry), UpNextWording.Line(item, DateTimeOffset.Now));

    CoverItem Cover(Entry entry)
    {
        var cover = new CoverItem(entry);
        cover.PropertyChanged += (_, p) =>
        {
            if (p.PropertyName == nameof(CoverItem.Banner) && selected?.Cover == cover)
                BannerImage.Source = cover.Banner;
        };
        return cover.Load(ctx.Art, withBanner: true);
    }

    void ShowSyncLine()
    {
        var waiting = ctx.Known.Anilist?.Outbox ?? 0;
        SyncLine.Text = waiting > 0 ? T.ContinueSyncWaiting(waiting) : "";
        SyncLine.Visibility = waiting > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (Queue.SelectedItem is not QueueRow row || row == selected)
            return;
        // A rebuilt list selects its new row for the same title: no fade, nothing to load again.
        if (row.Entry.Id == selected?.Entry.Id && row.Entry.Id == detailAsked)
        {
            selected = row;
            return;
        }
        // The preview crossfades in, unless the selection is still moving (M2).
        var now = DateTimeOffset.UtcNow;
        if (selected is not null && now - lastPick > TimeSpan.FromMilliseconds(150))
            Motion.FadeIn(PreviewBody);
        lastPick = now;
        ShowPreview(row);
        detailAsked = row.Entry.Id;
        ctx.Send(new Command.LoadDetail(row.Entry.Id));
        if (row.Item is not null)
            ctx.Send(new Command.LoadEpisodes(row.Entry.Id));
    }

    void ShowPreview(QueueRow? row)
    {
        selected = row;
        if (row is null)
            return;
        var entry = ctx.Known.Entry(row.Entry.Id) ?? row.Entry;
        BannerImage.Source = row.Cover.Banner;
        PreviewTitle.Text = entry.Title;
        if (row.Suggestion is { } suggestion)
        {
            ShowSuggestionPreview(suggestion, entry);
            return;
        }
        var item = row.Item!;
        var episodeTitle = ctx.Known.Episodes.TryGetValue(entry.Id, out var rows)
            ? rows.FirstOrDefault(r => r.Number == item.Episode.ToString())?.Title
            : null;
        // "Episode 7 · Dubbed · The Lantern Road": a show watched dubbed is marked, as its row is.
        var dubbed = item.Entry.WatchedIn == Anistream.Engine.Core.Translation.Dub;
        EpisodeLine.Text = episodeTitle is { Length: > 0 } named
            ? dubbed ? T.EpisodeWithTitleDubbed(item.Episode, named) : T.EpisodeWithTitle(item.Episode, named)
            : Wording.EpisodeLine(T.EpisodeNumber(item.Episode), item.Entry.WatchedIn);
        Meter.Visibility = item.Fraction is not null ? Visibility.Visible : Visibility.Collapsed;
        EpisodeProgress.Value = item.Fraction ?? 0;
        LeftLine.Text = item.Left ?? "";
        PlayButton.Content = item.Button;
        PlayButton.Visibility = item.Button is not null ? Visibility.Visible : Visibility.Collapsed;
        OpenButton.Visibility = Visibility.Collapsed;
        WaitingLine.Text = item.Waiting ?? "";
        WaitingLine.Visibility = item.Waiting is not null ? Visibility.Visible : Visibility.Collapsed;
        Synopsis.Text = entry.Synopsis;
    }

    /// <summary>
    /// A suggestion's preview: what it is, why it is here, and one amber button saying what it
    /// does. A title that has not started airing and is on Planning already has none.
    /// </summary>
    void ShowSuggestionPreview(UpNextItem suggestion, Entry entry)
    {
        EpisodeLine.Text = Wording.MetaLine(entry);
        Meter.Visibility = Visibility.Collapsed;
        var button = UpNextWording.Button(suggestion);
        PlayButton.Content = button;
        PlayButton.Visibility = button is not null ? Visibility.Visible : Visibility.Collapsed;
        OpenButton.Visibility = Visibility.Visible;
        var line = UpNextWording.Line(suggestion, DateTimeOffset.Now);
        WaitingLine.Text = line.ToString();
        WaitingLine.Visibility = Visibility.Visible;
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
            ctx.OpenTitle(row.Entry.Id);
    }

    /// <summary>Enter and the amber button: play the queue row, or do what the suggestion's button says.</summary>
    void Play(QueueRow? row)
    {
        if (row is { Suggestion: { } suggestion })
        {
            switch (suggestion.Action)
            {
                case UpNextAction.PlayFirstEpisode:
                    ctx.Play(suggestion.Entry.Id, "1");
                    break;
                case UpNextAction.AddToPlanning:
                    AddToPlanning(row);
                    break;
            }
        }
        else if (row is { Item.Playable: true })
            ctx.Play(row.Entry.Id, row.Item.Episode.ToString());
    }

    void OnPlay(object sender, RoutedEventArgs e) => Play(selected);

    void OnOpenPreview(object sender, RoutedEventArgs e)
    {
        if (selected is not null)
            ctx.OpenTitle(selected.Entry.Id);
    }

    void OnBannerTapped(object sender, TappedRoutedEventArgs e)
    {
        if (selected is not null)
            ctx.OpenTitle(selected.Entry.Id);
    }

    void OnQueueDoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        if (selected is not null)
            ctx.OpenTitle(selected.Entry.Id);
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
            ctx.Play(row.Entry.Id, row.Item.Episode.ToString(), fromStart: true);
    }

    void OnMenuWatched(object sender, RoutedEventArgs e) => MarkWatched(Of(sender));

    void MarkWatched(QueueRow? row)
    {
        if (row?.Item is not null)
            ctx.Send(new Command.SetWatched(row.Entry.Id, [row.Item.Episode.ToString()], true));
    }

    void OnMenuHide(object sender, RoutedEventArgs e) => Hide(Of(sender));

    void Hide(QueueRow? row)
    {
        if (row?.Item is null)
            return;
        ctx.Known.HiddenFromContinue.Add(row.Entry.Id);
        ctx.Send(new Command.HideFromContinue(row.Entry.Id));
        Show();
    }

    // Ctrl+Shift+M and Delete, from the window, on the selected row. Delete on a suggestion is Not interested.
    bool IPageKeys.MarkWatched()
    {
        MarkWatched(selected);
        return selected?.Item is not null;
    }

    bool IPageKeys.Delete()
    {
        if (selected?.Suggestion is not null)
        {
            Dismiss(selected);
            return true;
        }
        Hide(selected);
        return selected is not null;
    }

    void OnMenuDrop(object sender, RoutedEventArgs e)
    {
        if (Of(sender) is { } row)
            ctx.Send(new Command.SetStatus(row.Entry.Id, LibrarySegment.Dropped));
    }

    void OnMenuOpen(object sender, RoutedEventArgs e)
    {
        if (Of(sender) is { } row)
            ctx.OpenTitle(row.Entry.Id);
    }

    void OnSuggestionPlay(object sender, RoutedEventArgs e) => Play(Of(sender));

    void OnSuggestionPlan(object sender, RoutedEventArgs e) => AddToPlanning(Of(sender));

    void OnSuggestionDismiss(object sender, RoutedEventArgs e) => Dismiss(Of(sender));

    void AddToPlanning(QueueRow? row)
    {
        if (row?.Suggestion is not { Action: UpNextAction.AddToPlanning } suggestion)
            return;
        ctx.Send(new Command.SetStatus(suggestion.Entry.Id, LibrarySegment.Planning));
        ctx.Known.PlannedFromUpNext(suggestion.Entry.Id);
        Show();
    }

    /// <summary>
    /// Not interested: the row leaves, the next takes the selection, and a notice says what
    /// happened with Undo. The engine sends one more row to fill the place.
    /// </summary>
    void Dismiss(QueueRow? row)
    {
        if (row?.Suggestion is not { } suggestion)
            return;
        var id = suggestion.Entry.Id;
        if (selected?.Entry.Id == id)
        {
            var rows = ((IEnumerable<QueueGroup>?)Groups.Source ?? []).SelectMany(g => g).ToList();
            var at = rows.FindIndex(r => r.Entry.Id == id);
            selected = rows.Skip(at + 1).FirstOrDefault(r => r.Suggestion is not null)
                ?? rows.Take(Math.Max(at, 0)).LastOrDefault();
        }
        dismissed.Add(id);
        ctx.Send(new Command.DismissUpNext(id));
        Show();
        ctx.Window.Say(UpNextWording.HiddenNotice(suggestion), T.ActionUndo, () =>
        {
            dismissed.Remove(id);
            ctx.Send(new Command.RestoreUpNext(id));
            Show();
        });
    }
}

/// <summary>The window's Ctrl+Shift+M and Delete, for the page that has something selected.</summary>
interface IPageKeys
{
    bool MarkWatched();
    bool Delete() => false;
}
