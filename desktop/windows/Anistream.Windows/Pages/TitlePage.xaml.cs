using System.ComponentModel;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>An episode row: "7 · The Lantern Road", its runtime or what is left, and its still.</summary>
public sealed partial class EpisodeItem : INotifyPropertyChanged
{
    ImageSource? still;

    internal EpisodeItem(EpisodeRow row, bool spoiler)
    {
        Row = row;
        Spoiler = spoiler;
    }

    public EpisodeRow Row { get; }
    public bool Spoiler { get; }
    public string Heading => Row.Title is { Length: > 0 } t ? $"{Row.Number} · {t}" : $"Episode {Row.Number}";
    public string Name => $"{Heading}, {Line}";
    public double Watched => Row.Watched;
    public bool Completed => Row.Completed;
    /// <summary>An episode no source lists is dimmed whole; a watched one is only a little.</summary>
    public double Dim => Row.Unsourced ? 0.55 : Row.Completed ? 0.6 : 1;
    /// <summary>Play on TV in the episode's menu, once the engine gives a sender.</summary>
    public Visibility OnTv => Ctx.Current.Tv.Available ? Visibility.Visible : Visibility.Collapsed;
    public Visibility PartWay => Row.Watched is > 0 and < 1 && !Row.Completed ? Visibility.Visible : Visibility.Collapsed;
    public bool Playable => EpisodeSources.Playable(Row);
    public string WatchedAction => Row.Completed ? "Mark as unwatched" : "Mark as watched";
    public Visibility SpoilerShown => Spoiler ? Visibility.Visible : Visibility.Collapsed;

    public string Line
    {
        get
        {
            if (Row.Unsourced)
                return T.EpisodeUnsourced;
            if (Row.DurationSecs is not { } secs)
                return Row.MetadataOnly ? "Listed by AniList" : "";
            if (Row.Watched is > 0 and < 1 && !Row.Completed)
                return $"{Math.Max(1, (int)Math.Round(secs * (1 - Row.Watched) / 60))} min left";
            return AnistreamFfiMethods.RuntimeMinutes(secs);
        }
    }

    EpisodeDownload download = EpisodeDownload.Of([], 0, "");
    bool reveal;

    /// <summary>Where the episode stands in the download queue.</summary>
    public EpisodeDownload Download
    {
        get => download;
        set
        {
            if (download == value)
                return;
            download = value;
            Changed(nameof(DownloadOpacity), nameof(DownloadSay), nameof(DownloadGlyph), nameof(DownloadTint), nameof(GlyphShown),
                nameof(RingShown), nameof(RingWaiting), nameof(RingActive), nameof(RingValue), nameof(CanDownload));
        }
    }

    /// <summary>The pointer over the row, or focus in it: Download shows.</summary>
    public bool Reveal
    {
        get => reveal;
        set
        {
            if (reveal == value)
                return;
            reveal = value;
            Changed(nameof(DownloadOpacity));
        }
    }

    public bool CanDownload => Playable && Download.Asks;
    public string DownloadSay => Download.Say;
    /// <summary>Nothing yet, and no pointer or focus here: the cell is empty, though it keeps its place.</summary>
    public double DownloadOpacity => !Playable || (Download.Kind == EpisodeDownloadKind.None && !Reveal) ? 0 : 1;
    public string DownloadGlyph => Download.Kind == EpisodeDownloadKind.Done ? "\uE73E" : "\uE896";
    public Brush DownloadTint => Download.Kind switch
    {
        EpisodeDownloadKind.Done => (Brush)Application.Current.Resources["WaterBrush"],
        EpisodeDownloadKind.Failed => new SolidColorBrush(Ui.Vermilion),
        _ => (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
    };
    public Visibility GlyphShown => Download.Kind is EpisodeDownloadKind.None or EpisodeDownloadKind.Done or EpisodeDownloadKind.Failed ? Visibility.Visible : Visibility.Collapsed;
    public Visibility RingShown => GlyphShown == Visibility.Visible ? Visibility.Collapsed : Visibility.Visible;
    public bool RingWaiting => Download.Kind == EpisodeDownloadKind.Queued;
    /// <summary>A paused download's ring stands still at how far it got.</summary>
    public bool RingActive => Download.Kind != EpisodeDownloadKind.Paused;
    public double RingValue => Download.Fraction * 100;

    void Changed(params string[] names)
    {
        foreach (var name in names)
            PropertyChanged?.Invoke(this, new(name));
    }

    public ImageSource? Still
    {
        get => still;
        set { still = value; PropertyChanged?.Invoke(this, new(nameof(Still))); }
    }

    public event PropertyChangedEventHandler? PropertyChanged;
}

/// <summary>
/// A title's page: the banner with the ground over its lower part, the cover beside the show
/// title, the metadata line, the one amber action labelled with what it plays, the list status
/// and More, then the episodes. Details and related titles sit in a side column when the content
/// is wide enough, and under the episodes when it is not.
/// </summary>
public sealed partial class TitlePage : Page, IPageKeys
{
    const double SideWidth = 900;

    /// <summary>The connected animation's key: a cover grid prepares it, the title page's cover takes it.</summary>
    internal const string CoverAnimation = "cover";

    readonly Ctx ctx = Ctx.Current;
    uint id;
    QueueItem? action;

    public TitlePage()
    {
        InitializeComponent();
        foreach (var (segment, name) in new[]
        {
            (LibrarySegment.Watching, "Watching"), (LibrarySegment.Planning, "Planning"),
            (LibrarySegment.Completed, "Completed"), (LibrarySegment.Paused, "Paused"),
            (LibrarySegment.Dropped, "Dropped"),
        })
        {
            var item = new RadioMenuFlyoutItem { Text = name, GroupName = "status", Tag = segment };
            item.Click += (_, _) => ctx.Send(new Command.SetStatus(id, segment));
            StatusMenu.Items.Add(item);
        }
        SourceItem.Text = T.SourcePickerTitle;
        State.Placeholders = Placeholders;
        State.ActivityText = "Loading this title";
        var settings = new Microsoft.UI.Xaml.Input.KeyboardAccelerator { Key = global::Windows.System.VirtualKey.I, Modifiers = global::Windows.System.VirtualKeyModifiers.Control };
        settings.Invoked += (_, a) =>
        {
            a.Handled = true;
            OpenShowSettings();
        };
        KeyboardAccelerators.Add(settings);
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    /// <summary>The page's own layout as raised plates: banner, cover and title, then episode rows.</summary>
    static UIElement Placeholders()
    {
        var page = new StackPanel { Spacing = 20 };
        page.Children.Add(new Border { Height = 220, Background = Ui.Res<Brush>("RaisedBrush") });
        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 20, Margin = new Thickness(36, 0, 36, 0) };
        head.Children.Add(States.Plate(120, 180, 8));
        var bars = new StackPanel { Spacing = 10, VerticalAlignment = VerticalAlignment.Bottom };
        bars.Children.Add(States.Plate(320, 28, 4));
        bars.Children.Add(States.Plate(200, 14, 3));
        bars.Children.Add(States.Plate(150, 40, 6));
        head.Children.Add(bars);
        page.Children.Add(head);
        var rows = States.Rows(4, 128, 72);
        if (rows is FrameworkElement f)
            f.Margin = new Thickness(36, 8, 36, 0);
        page.Children.Add(rows);
        return page;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        id = (uint)e.Parameter;
        var known = ctx.Known.Entry(id) is not null;
        if (!known)
            State.Wait();
        ctx.Send(new Command.LoadDetail(id));
        ctx.Send(new Command.LoadEpisodes(id));
        // The queue, for each row's download cell.
        ctx.Send(new Command.LoadDownloads());
        ctx.Send(new Command.LoadDub(id));
        DubLine.Opacity = 0;
        Show();
        ShowEpisodes();
        // The cover a grid sent flies in to this one; anything else crossfades.
        var flight = ConnectedAnimationService.GetForCurrentView().GetAnimation(CoverAnimation);
        if (flight is not null)
        {
            if (known && Motion.Enabled)
            {
                flight.Configuration = new BasicConnectedAnimationConfiguration();
                flight.TryStart(CoverFrame);
            }
            else
                flight.Cancel();
        }
        if (ctx.Options.Show is "settings" or "settings-loading" or "settings-failed" && ctx.Options.Title == id)
            DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, OpenShowSettings);
#if ANISTREAM_TESTING
        if (ctx.Options.Show is "source-picker" or "source-picker-pinned" or "source-picker-releases" or "source-picker-loading" && ctx.Options.Title == id)
            _ = Task.Delay(1500).ContinueWith(_ => DispatcherQueue.TryEnqueue(OpenSourcePicker));
        // --show=source-unsourced-end: the table scrolled to its end, where the last unsourced rows are.
        if (ctx.Options.Show == "source-unsourced-end")
            _ = Task.Delay(2500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Scroll.ChangeView(null, Scroll.ScrollableHeight, null, true)));
        // --show=source-menu: the More menu open, for a screenshot.
        if (ctx.Options.Show == "source-menu" && ctx.Options.Title == id)
            _ = Task.Delay(1500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => MoreButton.Flyout.ShowAt(MoreButton)));
#endif
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Detail d when d.V1.Id == id:
                    Show();
                    break;
                case Event.Progress p when p.Id == id:
                    Show();
                    ctx.Send(new Command.LoadEpisodes(id));
                    break;
                case Event.TitleDub td when td.Id == id:
                    var line = AnistreamFfiMethods.DubLine(td.Dub);
                    if (line is not null)
                        DubLine.Text = line;
                    DubLine.Opacity = line is null ? 0 : 1;
                    break;
                case Event.Episodes ep when ep.Id == id:
                    ShowEpisodes();
                    // The button plays the first episode after the next that a source lists.
                    if (ctx.Known.Entry(id) is { } known)
                        ShowAction(known);
                    break;
                case Event.Downloads d:
                    ShowDownloads(d.V1);
                    break;
                case Event.TitlesChanged:
                    // Named another way now, with its relations and recommendations.
                    ctx.Send(new Command.LoadDetail(id));
                    break;
            }
        }
    }

    void Show()
    {
        if (ctx.Known.Entry(id) is not { } entry)
            return;
        State.Done();
        var cover = new CoverItem(entry);
        CoverFrame.Background = cover.Placeholder;
        Banner.Background = cover.Placeholder;
        ctx.Art.Bind(entry.CoverUrl, s => CoverImage.Source = s);
        ctx.Art.Bind(entry.BannerUrl ?? entry.CoverUrl, s => BannerImage.Source = s);
        ShowTitle.Text = entry.Title;
        Secondary.Text = entry.Secondary ?? "";
        Secondary.Visibility = entry.Secondary is { Length: > 0 } ? Visibility.Visible : Visibility.Collapsed;
        Meta.Text = AnistreamFfiMethods.MetaLine(entry);
        Synopsis.Text = entry.Synopsis;

        ShowAction(entry);

        StatusButton.Content = entry.ListStatus is { } status ? status.ToString() : "Add to list";
        foreach (var item in StatusMenu.Items.OfType<RadioMenuFlyoutItem>())
            item.IsChecked = item.Tag is LibrarySegment s && s == entry.ListStatus;
        TrailerItem.IsEnabled = entry.TrailerUrl is not null;

        ShowSeasons(entry);
        ShowDetails(entry);
    }

    /// <summary>
    /// The one amber action and what depends on it: the engine's next episode, or once the table
    /// is loaded the first after it that some source lists. With none, a line says so in its place.
    /// </summary>
    void ShowAction(Entry entry)
    {
        action = PlayableNext.Item(entry, ctx.Known.Episodes.GetValueOrDefault(id), Casing.Sentence);
        PrimaryAction.Content = action.Button;
        PrimaryAction.Visibility = action.Button is not null ? Visibility.Visible : Visibility.Collapsed;
        WaitingLine.Text = action.Waiting ?? "";
        WaitingLine.Visibility = action.Waiting is not null ? Visibility.Visible : Visibility.Collapsed;
        // A party plays the episode the primary action would.
        PartyItem.IsEnabled = action is { Playable: true };
        TvAction.Visibility = action is { Playable: true } && ctx.Tv.Available ? Visibility.Visible : Visibility.Collapsed;
    }

    IReadOnlyList<Anistream.App.TitleSeasons.Choice> seasons = [];

    /// <summary>The franchise's seasons, once the engine has walked them, with this one picked.</summary>
    void ShowSeasons(Entry entry)
    {
        var now = Anistream.App.TitleSeasons.Of(entry);
        if (now.SequenceEqual(seasons))
            return;
        seasons = now;
        SeasonPicker.SelectionChanged -= OnSeasonPicked;
        SeasonPicker.Items.Clear();
        foreach (var choice in seasons)
            SeasonPicker.Items.Add(choice.Label);
        SeasonPicker.SelectedIndex = seasons.ToList().FindIndex(c => c.IsThis);
        SeasonPicker.SelectionChanged += OnSeasonPicked;
        if (seasons.Count > 0)
            Motion.Show(SeasonPicker);
        else
            Motion.Set(SeasonPicker, false);
    }

    void OnSeasonPicked(object sender, SelectionChangedEventArgs e)
    {
        if (SeasonPicker.SelectedIndex is not (var at and >= 0) || at >= seasons.Count || seasons[at].IsThis)
            return;
        ctx.OpenTitle(seasons[at].Id);
        // Back here, the picker says this page's season again.
        SeasonPicker.SelectionChanged -= OnSeasonPicked;
        SeasonPicker.SelectedIndex = seasons.ToList().FindIndex(c => c.IsThis);
        SeasonPicker.SelectionChanged += OnSeasonPicked;
    }

    void ShowDetails(Entry entry)
    {
        var wide = Scroll.ActualWidth == 0 || Scroll.ActualWidth >= SideWidth;
        SideColumn.Width = wide ? new GridLength(260) : new GridLength(0);
        Side.Visibility = wide ? Visibility.Visible : Visibility.Collapsed;
        var host = wide ? Side : DetailsBelow;
        Side.Children.Clear();
        DetailsBelow.Children.Clear();

        var details = new StackPanel { Spacing = 6 };
        details.Children.Add(new TextBlock { Text = "Details", Style = Res<Style>("BodyStrongTextBlockStyle") });
        void Line(string? text)
        {
            if (!string.IsNullOrEmpty(text))
                details.Children.Add(new TextBlock { Text = text, Style = Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.WrapWholeWords });
        }
        Line(entry.Studio is { } studio ? $"Studio {studio}" : null);
        Line(entry.Genres is { Length: > 0 } genres ? string.Join(", ", genres) : null);
        Line(entry.Source is { } source ? $"Source: {Words.Source(source)}" : null);
        Line(AnistreamFfiMethods.BroadcastLine(entry) is { Length: > 0 } airing ? airing : null);
        host.Children.Add(details);

        if (Anistream.App.TitleSeasons.Related(entry) is { Length: > 0 } related)
        {
            var list = new StackPanel { Spacing = 6 };
            list.Children.Add(new TextBlock { Text = "Related", Style = Res<Style>("BodyStrongTextBlockStyle") });
            foreach (var r in related.Take(8))
            {
                var link = new HyperlinkButton { Padding = new Thickness(0), HorizontalAlignment = HorizontalAlignment.Left };
                var text = new StackPanel();
                text.Children.Add(new TextBlock { Text = r.Title, Style = Res<Style>("ShowTitleStyle") });
                text.Children.Add(new TextBlock { Text = Words.Relation(r.Relation, r.Format), Style = Res<Style>("HazeCaptionStyle") });
                link.Content = text;
                var target = r.Id;
                link.Click += (_, _) => ctx.OpenTitle(target);
                list.Children.Add(link);
            }
            host.Children.Add(list);
        }
    }

    void ShowEpisodes()
    {
        if (!ctx.Known.Episodes.TryGetValue(id, out var rows))
            return;
        var next = ctx.Known.Entry(id)?.Progress?.Next ?? 1;
        var items = rows.Select(r =>
        {
            var spoiler = uint.TryParse(r.Number, out var n) && n > next && !r.Completed;
            var item = new EpisodeItem(r, spoiler);
            // A spoiler's still loads too, and shows blurred.
            ctx.Art.Bind(r.Thumbnail, s => item.Still = s);
            return item;
        }).ToList();
        EpisodeList.ItemsSource = items;
        ShowDownloads(ctx.Known.Downloads);
        var metadataOnly = rows.Length > 0 && rows.All(r => r.MetadataOnly);
        EpisodesNote.Text = metadataOnly ? "Episodes from AniList. Add a source to play them." : "";
        EpisodesNote.Visibility = metadataOnly ? Visibility.Visible : Visibility.Collapsed;
    }

    static T Res<T>(string key) => (T)Application.Current.Resources[key];

    /// <summary>Each episode row's download cell, from the queue.</summary>
    void ShowDownloads(DownloadRow[] rows)
    {
        if (EpisodeList.ItemsSource is not IEnumerable<EpisodeItem> items)
            return;
        foreach (var item in items)
            item.Download = EpisodeDownload.Of(rows, id, item.Row.Number);
#if ANISTREAM_TESTING
        // A shot's hover: the row after the one playing next, as if the pointer were on it.
        if (ctx.Options.Show == "episode-download" && ctx.Known.Entry(id)?.Progress?.Next is { } next)
            foreach (var item in items.Where(i => i.Row.Number == (next + 1).ToString()))
            {
                item.Reveal = true;
                DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
                    (EpisodeList.ContainerFromItem(item) as UIElement)?.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0.5, AnimationDesired = false }));
            }
#endif
    }

    readonly HashSet<ListViewItem> watchedContainers = [];

    /// <summary>A row's container: its pointer and focus reveal the row's Download, whichever episode it carries now.</summary>
    void OnEpisodeContainer(ListViewBase sender, ContainerContentChangingEventArgs args)
    {
        if (args.ItemContainer is not ListViewItem container || !watchedContainers.Add(container))
            return;
        static void Set(object sender, bool on)
        {
            if (sender is ListViewItem { Content: EpisodeItem item } c)
                item.Reveal = on || c.FocusState != FocusState.Unfocused;
        }
        container.PointerEntered += (s, _) => Set(s, true);
        container.PointerExited += (s, _) => Set(s, false);
        container.PointerCanceled += (s, _) => Set(s, false);
        container.GotFocus += (s, _) => Set(s, true);
        container.LostFocus += (s, _) =>
        {
            // Focus moving within the row, to its Download, keeps the cell shown.
            if (s is ListViewItem { Content: EpisodeItem item } c && XamlRoot is { } root
                && FocusManager.GetFocusedElement(root) is DependencyObject now && IsWithin(now, c))
                return;
            Set(s, false);
        };
    }

    static bool IsWithin(DependencyObject element, DependencyObject ancestor)
    {
        for (var at = element; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == ancestor)
                return true;
        return false;
    }

    void OnDownloadCellFocus(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item)
            item.Reveal = true;
    }

    /// <summary>The download cell: Download when there is none or it failed, else the Downloads page.</summary>
    void OnDownloadCell(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is not { Playable: true } item)
            return;
        if (item.Download.Asks)
            ctx.Send(new Command.DownloadEpisode(id, item.Row.Number));
        else
            ctx.Go("downloads");
    }

    void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        Banner.Height = Math.Min(360, Math.Max(220, e.NewSize.Width * 9 / 32));
        Columns.Padding = new Thickness(36, Banner.Height - 120, 36, 24);
        if (ctx.Known.Entry(id) is { } entry)
            ShowDetails(entry);
    }

    void OnPrimary(object sender, RoutedEventArgs e)
    {
        if (action is { Playable: true })
            ctx.Play(id, action.Episode.ToString());
    }

    void OnEpisodeClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is EpisodeItem { Playable: true } item)
            ctx.Play(id, item.Row.Number);
    }

    void OnWatchedToggle(object sender, RoutedEventArgs e)
    {
        if (sender is CheckBox { Tag: EpisodeItem item } box)
            ctx.Send(new Command.SetWatched(id, [item.Row.Number], box.IsChecked == true));
    }

    static EpisodeItem? ItemOf(object sender) => (sender as FrameworkElement)?.Tag as EpisodeItem;

    void OnEpisodePlay(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { Playable: true } item)
            ctx.Play(id, item.Row.Number);
    }

    void OnEpisodeFromStart(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { Playable: true } item)
            ctx.Play(id, item.Row.Number, fromStart: true);
    }

    /// <summary>An episode on the TV: from where it was left when it is the one being resumed.</summary>
    void OnEpisodeOnTv(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is not { Playable: true } item || ctx.Known.Entry(id) is not { } entry)
            return;
        var from = action is { } a && a.Episode.ToString() == item.Row.Number ? a.Resume?.Position : null;
        ctx.Tv.Offer(new TvPlay(id, entry.Title, item.Row.Number, from));
    }

    void OnPlayOnTv(object sender, RoutedEventArgs e)
    {
        if (action is { Playable: true } a && ctx.Known.Entry(id) is { } entry)
            ctx.Tv.Offer(new TvPlay(id, entry.Title, a.Episode.ToString(), a.Resume?.Position));
    }

    void OnEpisodeWatched(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item)
            ctx.Send(new Command.SetWatched(id, [item.Row.Number], !item.Row.Completed));
    }

    void OnEpisodeDownload(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { CanDownload: true } item)
            ctx.Send(new Command.DownloadEpisode(id, item.Row.Number));
    }

    /// <summary>Ctrl+Shift+M: the focused episode, or the next one, marked watched (or unwatched).</summary>
    public bool MarkWatched()
    {
        var focused = FocusManager.GetFocusedElement(XamlRoot) as DependencyObject;
        var item = focused is not null ? EpisodeList.ItemFromContainer(focused) as EpisodeItem : null;
        if (item is null && action is not null && EpisodeList.ItemsSource is IEnumerable<EpisodeItem> all)
            item = all.FirstOrDefault(i => i.Row.Number == action.Episode.ToString());
        if (item is null)
            return false;
        ctx.Send(new Command.SetWatched(id, [item.Row.Number], !item.Row.Completed));
        return true;
    }

    void OnShowSettings(object sender, RoutedEventArgs e) => OpenShowSettings();

    void OnPickSource(object sender, RoutedEventArgs e) => OpenSourcePicker();

    void OpenSourcePicker()
    {
        if (ctx.Known.Entry(id) is { } entry && XamlRoot is { } root)
            _ = SourcePickerDialog.Open(ctx, id, entry.Title, root);
    }

    void OnWatchTogether(object sender, RoutedEventArgs e)
    {
        if (action is { Playable: true })
            ctx.Window.ShowParty(new PartyDialog.For.Episode(id, action.Episode.ToString()));
    }

    void OpenShowSettings()
    {
        if (ctx.Known.Entry(id) is { } entry && XamlRoot is { } root)
            _ = ShowSettingsDialog.Open(ctx, id, entry.Title, root);
    }

    /// <summary>Download…: a range of the show's episodes, from the next one to the last by default.</summary>
    async void OnDownloadRange(object sender, RoutedEventArgs e)
    {
        if (XamlRoot is not { } root || !ctx.Known.Episodes.TryGetValue(id, out var rows))
            return;
        // A range skips what no source lists: nothing could play it.
        var playable = EpisodeSources.Downloadable(rows);
        if (playable.Length == 0)
            return;
        var next = action?.Episode.ToString();
        var start = Math.Max(0, Array.IndexOf(playable, next));
        var from = new ComboBox { Header = "From", MinWidth = 120 };
        var to = new ComboBox { Header = "To", MinWidth = 120 };
        foreach (var n in playable)
        {
            from.Items.Add($"Episode {n}");
            to.Items.Add($"Episode {n}");
        }
        from.SelectedIndex = start;
        to.SelectedIndex = playable.Length - 1;
        var pick = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        pick.Children.Add(from);
        pick.Children.Add(to);
        var body = new StackPanel { Spacing = 12 };
        body.Children.Add(pick);
        body.Children.Add(Ui.Text("Downloads use the quality in Settings › Downloads.", "HazeCaptionStyle"));
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = "Download episodes",
            Content = body,
            PrimaryButtonText = "Download",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
        };
        void Check() => dialog.IsPrimaryButtonEnabled = from.SelectedIndex >= 0 && to.SelectedIndex >= from.SelectedIndex;
        from.SelectionChanged += (_, _) => Check();
        to.SelectionChanged += (_, _) => Check();
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
            return;
        var chosen = playable[from.SelectedIndex..(to.SelectedIndex + 1)];
        ctx.Send(new Command.DownloadMany(id, chosen));
    }

    void OnMarkPrevious(object sender, RoutedEventArgs e)
    {
        if (action is null || !ctx.Known.Episodes.TryGetValue(id, out var rows))
            return;
        var before = rows.Where(r => uint.TryParse(r.Number, out var n) && n < action.Episode && !r.Completed).Select(r => r.Number).ToArray();
        if (before.Length > 0)
            ctx.Send(new Command.SetWatched(id, before, true));
    }

    void OnTrailer(object sender, RoutedEventArgs e)
    {
        if (ctx.Known.Entry(id) is { TrailerUrl: { } url } entry)
            ctx.Send(new Command.PlayTrailer(url, entry.Title));
    }

    void OnOpenAnilist(object sender, RoutedEventArgs e) =>
        ctx.Send(new Command.OpenExternal($"https://anilist.co/anime/{id}"));
}
