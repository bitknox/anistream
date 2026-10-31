using System.ComponentModel;
using Anistream.App;
using Anistream.Windows.Discussion;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Anistream.Windows;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>An episode row: "Episode 7 · The Lantern Road", its runtime or what is left, and its still.</summary>
public sealed partial class EpisodeItem : INotifyPropertyChanged
{
    ImageSource? still;

    internal EpisodeItem(EpisodeRow row, bool spoiler, uint title = 0)
    {
        Row = row;
        Spoiler = spoiler;
        Title = title;
        Discussion = Ctx.Current is { } ctx && uint.TryParse(row.Number, out var n) ? ctx.Known.Discussions.GetValueOrDefault((title, n)) : 0;
    }

    public uint Title { get; }

    uint discussion;

    /// <summary>The comments in the episode's discussion, once the engine found one.</summary>
    public uint Discussion
    {
        get => discussion;
        set
        {
            if (discussion == value)
                return;
            discussion = value;
            Changed(nameof(DiscussionLine), nameof(DiscussionShown));
        }
    }

    public string DiscussionLine => T.DiscussionRow(Discussion);
    public Visibility DiscussionShown => Discussion > 0 ? Visibility.Visible : Visibility.Collapsed;

    public EpisodeRow Row { get; }
    public bool Spoiler { get; }
    public string Heading => long.TryParse(Row.Number, out var n)
        ? Row.Title is { Length: > 0 } t ? T.EpisodeWithTitle(n, t) : T.EpisodeNumber(n)
        : Row.Title is { Length: > 0 } named ? T.EpisodeTextWithTitle(Row.Number, named) : T.EpisodeNumberText(Row.Number);
    /// <summary>What Narrator reads for the row: its heading, then its line when it has one.</summary>
    public string Name => Line.Length > 0 ? T.EpisodeRowSpoken(Heading, Line) : Heading;
    public double Watched => Row.Watched;
    public bool Completed => Row.Completed;
    /// <summary>An episode no source lists is dimmed whole; a watched one is only a little.</summary>
    public double Dim => Row.Unsourced ? 0.55 : Row.Completed ? 0.6 : 1;
    /// <summary>Play on TV in the episode's menu, once the engine gives a sender.</summary>
    public Visibility OnTv => Ctx.Current.Tv.Available ? Visibility.Visible : Visibility.Collapsed;
    public Visibility PartWay => Row.Watched is > 0 and < 1 && !Row.Completed ? Visibility.Visible : Visibility.Collapsed;
    public bool Playable => EpisodeSources.Playable(Row);
    public string WatchedAction => Row.Completed ? T.EpisodeMarkUnwatched : T.EpisodeMarkWatched;
    public Visibility SpoilerShown => Spoiler ? Visibility.Visible : Visibility.Collapsed;

    public string Line
    {
        get
        {
            if (Row.Unsourced)
                return T.EpisodeUnsourced;
            if (Row.DurationSecs is not { } secs)
                return Row.MetadataOnly ? T.EpisodeListedByAnilist : "";
            if (Row.Watched is > 0 and < 1 && !Row.Completed)
                return T.TimeMinutesLeft(Math.Max(1, (long)Math.Round(secs * (1 - Row.Watched) / 60)));
            return Wording.Minutes(secs);
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
            (LibrarySegment.Watching, T.LibraryStatusWatching), (LibrarySegment.Planning, T.LibraryStatusPlanning),
            (LibrarySegment.Completed, T.LibraryStatusCompleted), (LibrarySegment.Paused, T.LibraryStatusPaused),
            (LibrarySegment.Dropped, T.LibraryStatusDropped),
        })
        {
            var item = new RadioMenuFlyoutItem { Text = name, GroupName = "status", Tag = segment };
            item.Click += (_, _) => ctx.Send(new Command.SetStatus(id, segment));
            StatusMenu.Items.Add(item);
        }
        SourceItem.Text = T.SourcePickerTitle;
        State.Placeholders = Placeholders;
        State.ActivityText = T.TitleLoading;
        var settings = new Microsoft.UI.Xaml.Input.KeyboardAccelerator { Key = global::Windows.System.VirtualKey.I, Modifiers = global::Windows.System.VirtualKeyModifiers.Control };
        settings.Invoked += (_, a) =>
        {
            a.Handled = true;
            OpenShowSettings();
        };
        KeyboardAccelerators.Add(settings);
        // The page's own key is in the shortcuts list; a tooltip naming it over the whole page is not wanted.
        KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Hear(this, Heard, () =>
        {
            discussion?.Close();
        });
    }

    /// <summary>The banner's height and where the cover and title start under it.</summary>
    void Layout()
    {
        var band = Math.Min(360, Math.Max(220, Scroll.ActualWidth * 9 / 32));
        Banner.Height = band;
        Columns.Padding = new Thickness(36, band - 120, 36, 24);
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
        if (ctx.Options.Show is { } more && more.StartsWith("more-open") && ctx.Options.Title == id)
            _ = Task.Delay(1500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                if (Current() is { } now && XamlRoot is { } root)
                    new TitleMoreDialog(ctx, root, now).Show();
            }));
        if (ctx.Options.Show is "source-picker" or "source-picker-pinned" or "source-picker-releases" or "source-picker-loading" && ctx.Options.Title == id)
            _ = Task.Delay(1500).ContinueWith(_ => DispatcherQueue.TryEnqueue(OpenSourcePicker));
        // --show=source-unsourced-end: the table scrolled to its end, where the last unsourced rows are.
        if (ctx.Options.Show == "source-unsourced-end")
            _ = Task.Delay(2500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Scroll.ChangeView(null, Scroll.ScrollableHeight, null, true)));
        // --show=discussion...: the quiet rows under watched episodes, and the pane open on episode 2.
        if (Discussion.DiscussionShowcase.Is(ctx.Options.Show) && ctx.Options.Title == id)
            _ = Task.Delay(1800).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                Event[] found = [new Event.DiscussionFound(id, 1, 38), new Event.DiscussionFound(id, 2, 214)];
                foreach (var f in found)
                    ctx.Known.Take(f);
                ctx.Heard(found);
                if (ctx.Options.Show == "discussion-row")
                    return;
                OpenDiscussion(2, null);
                var show = ctx.Options.Show!;
                _ = Task.Delay(700).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
                {
                    if (discussion is { } pane)
                        Discussion.DiscussionShowcase.Then(show, pane);
                }));
            }));
        // --show=trailer-window, trailer-window-stuck: the Play trailer window open, playing or never starting.
        if (ctx.Options.Show is "trailer-window" or "trailer-window-stuck" && ctx.Options.Title == id)
            _ = Task.Delay(3500).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                if (ctx.Known.Entry(id) is not { } entry)
                    return;
                if (ctx.Options.Show == "trailer-window-stuck")
                    entry = entry with { Trailer = new Trailer(TrailerHost.YouTube, Trailers.FixtureStuckId) };
                TrailerWindow.Play(ctx, entry);
            }));
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
                case Event.DiscussionFound found when found.Media == id:
                    if (EpisodeList.ItemsSource is IEnumerable<EpisodeItem> shown)
                        foreach (var item in shown.Where(i => i.Row.Number == found.Episode.ToString()))
                            item.Discussion = found.Comments;
                    break;
                case Event.TitleDub td when td.Id == id:
                    var line = Wording.DubLine(td.Dub);
                    if (line is not null)
                        DubLine.Text = line;
                    DubLine.Opacity = line is null ? 0 : 1;
                    break;
                case Event.Availability av when av.V1.Id == id:
                    ShowAction(Current()!);
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

    Entry? Current() =>
#if ANISTREAM_TESTING
        ctx.Known.Entry(id) is { } e ? Sampled(e) : null;
#else
        ctx.Known.Entry(id);
#endif

#if ANISTREAM_TESTING
    /// <summary>--show=more...: made-up facts and sheet content, as the fixtures carry none. +film, +hiatus, +cancelled.</summary>
    Entry Sampled(Entry e)
    {
        if (ctx.Options.Show is not { } show || !show.StartsWith("more"))
            return e;
        var filmed = show.Contains("+film");
        AiringFact run = show.Contains("+hiatus") ? new AiringFact.Hiatus(new FuzzyDate(2026, 3, null))
            : show.Contains("+cancelled") ? new AiringFact.Cancelled(6)
            : filmed ? new AiringFact.Finished(new FuzzyDate(2025, 5, 2), null)
            : new AiringFact.Finished(new FuzzyDate(2023, 4, 8), new FuzzyDate(2023, 6, 24));
        return e with
        {
            Format = filmed ? "MOVIE" : "ONA",
            Studio = "Ginrei Studio",
            Source = SourceMaterial.LightNovel,
            Airing = run,
            FilmLength = filmed ? new RunLength(1, 52) : null,
            Country = "CN",
            MadeOutsideJapan = show.Contains("+china"),
            Detail = new TitleDetail(
                Tags: [new TitleTag("Found family", 90, false, false, null), new TitleTag("Rural", 70, false, false, null), new TitleTag("Betrayal", 60, true, false, null), new TitleTag("Hidden past", 50, false, true, null)],
                Rankings: [new Ranking(4, RankingKind.Rated, "", false, Season.Spring, 2023), new Ranking(11, RankingKind.Popular, "", true, null, null)],
                Characters: [new CastMember(Name: "Mira Okonkwo", Role: CastRole.Main, VoiceActors: [new Person("Haru Tanabe", null)]), new CastMember(Name: "Tobias Lindqvist", Role: CastRole.Main, VoiceActors: [new Person("Ren Aoyama", null)]), new CastMember(Name: "Elsa Marchetti", Role: CastRole.Supporting, VoiceActors: [])],
                Staff: [new StaffCredit("Noor Haddad", "Director"), new StaffCredit("Pavel Novak", "Series composition")],
                StreamingLinks: [new StreamingLink("Lanternflix", "https://lanternflix.example/show", null), new StreamingLink("Kiri Stream", "https://kiri.example/show", "Japanese")],
                Following: show.Contains("+signed-out") ? [] : [new FollowedEntry("Ines", null, LibrarySegment.Completed, 90, 10), new FollowedEntry("Jonas", null, LibrarySegment.Watching, null, 4)]),
        };
    }
#endif

    void Show()
    {
        if (Current() is not { } entry)
            return;
        State.Done();
        var cover = new CoverItem(entry);
        CoverFrame.Background = cover.Placeholder;
        Art.Background = cover.Placeholder;
        ctx.Art.Bind(entry.CoverUrl, s => CoverImage.Source = s);
        ctx.Art.Bind(entry.BannerUrl ?? entry.CoverUrl, s => BannerImage.Source = s);
        ShowTitle.Text = entry.Title;
        Secondary.Text = entry.Secondary ?? "";
        Secondary.Visibility = entry.Secondary is { Length: > 0 } ? Visibility.Visible : Visibility.Collapsed;
        Meta.Text = Wording.MetaLine(entry);
        Synopsis.MaxLines = 8;
        SynopsisMore.Visibility = Visibility.Collapsed;
        Synopsis.Text = entry.Synopsis;

        ShowAction(entry);

        StatusButton.Content = entry.ListStatus is { } status ? StatusName(status) : T.LibraryAddToList;
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
    /// <summary>More shows only where the synopsis is cut at its eight lines.</summary>
    void OnSynopsisTrimmed(TextBlock sender, IsTextTrimmedChangedEventArgs args) =>
        SynopsisMore.Visibility = sender.IsTextTrimmed ? Visibility.Visible : Visibility.Collapsed;

    void OnSynopsisMore(object sender, RoutedEventArgs args)
    {
        Synopsis.MaxLines = 0;
        SynopsisMore.Visibility = Visibility.Collapsed;
    }

    TitleAvailability? Avail => ctx.Known.Availability.GetValueOrDefault(id);
    bool? askedPlaysHere;

    /// <summary>
    /// Which licensed services have the title, once the sources have answered. The engine decides the
    /// case; this only asks again when whether it plays here changes.
    /// </summary>
    void AskAvailability(bool playsHere)
    {
        if (askedPlaysHere == playsHere)
            return;
        askedPlaysHere = playsHere;
        ctx.Send(new Command.Licensed(new LicensedCommand.LoadAvailability(id, playsHere)));
    }

    void OpenService(ServiceLink link, uint? episode = null) =>
        ctx.Send(new Command.Licensed(new LicensedCommand.Open(id, link.Service, link.Url, episode)));

    /// <summary>Case A and C lines, the Case B action and the More menu's Watch on items.</summary>
    void ShowAvailability()
    {
        AvailLine.Children.Clear();
        AvailLine.Visibility = Visibility.Collapsed;
        foreach (var old in MoreMenu.Items.Where(i => i.Tag as string == "avail").ToList())
            MoreMenu.Items.Remove(old);
        if (Avail is not { } a)
            return;
        var at = 0;
        foreach (var link in a.Yours ?? [])
        {
            var item = new MenuFlyoutItem { Text = T.TitleWatchOn(link.Service), Tag = "avail", Icon = new FontIcon { Glyph = "\uE8A7" } };
            item.Click += (_, _) => OpenService(link);
            MoreMenu.Items.Insert(at++, item);
        }
        if (at > 0)
            MoreMenu.Items.Insert(at, new MenuFlyoutSeparator { Tag = "avail" });

        void Text(string text) => AvailLine.Children.Add(new TextBlock { Text = text, Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.WrapWholeWords });
        HyperlinkButton Link(string text, Action go)
        {
            var b = new HyperlinkButton { Content = text, Padding = new Thickness(4, 0, 4, 0) };
            b.Click += (_, _) => go();
            AvailLine.Children.Add(b);
            return b;
        }
        // The not-found line is a sentence and a link: stacked, so a narrow window wraps the sentence instead of clipping the link.
        AvailLine.Orientation = a.Case == AvailabilityCase.NotInSources ? Orientation.Vertical : Orientation.Horizontal;
        switch (a.Case)
        {
            case AvailabilityCase.PlaysAlsoOn or AvailabilityCase.OnService when (a.AlsoOn ?? []).Length > 0:
                Text(T.TitleAlsoOnServices("").TrimEnd());
                foreach (var link in a.AlsoOn!)
                    Link(link.Service, () => OpenService(link));
                break;
            case AvailabilityCase.NotInSources when (a.Others ?? []).Length > 0:
                Text(T.TitleListedOn(string.Join(", ", a.Others!)));
                if (a.ChooseServices)
                    Link(T.TitleChooseServices, () => ctx.Window.Push(typeof(LicensedPage), null));
                break;
        }
        AvailLine.Visibility = AvailLine.Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void ShowAction(Entry entry)
    {
        action = PlayableNext.Item(entry, ctx.Known.Episodes.GetValueOrDefault(id), Casing.Sentence);
        if (ctx.Known.Episodes.ContainsKey(id))
            AskAvailability(action is { Playable: true });
        PrimaryAction.Content = action.Button;
        PrimaryAction.Visibility = action.Button is not null ? Visibility.Visible : Visibility.Collapsed;
        WaitingLine.Text = action.Waiting ?? "";
        WaitingLine.Visibility = action.Waiting is not null ? Visibility.Visible : Visibility.Collapsed;
        // A party plays the episode the primary action would.
        PartyItem.IsEnabled = action is { Playable: true };
        TvAction.Visibility = action is { Playable: true } && ctx.Tv.Available ? Visibility.Visible : Visibility.Collapsed;
        // Case B: it does not play here, so the amber action opens the service that has it.
        if (Avail is { Case: AvailabilityCase.OnService, Main: { } main } && action is not { Playable: true })
        {
            PrimaryAction.Content = ExternalLabel(T.TitleWatchOn(main.Service));
            PrimaryAction.Visibility = Visibility.Visible;
            WaitingLine.Visibility = Visibility.Collapsed;
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(PrimaryAction, T.TitleWatchOn(main.Service));
        }
        else
            PrimaryAction.ClearValue(Microsoft.UI.Xaml.Automation.AutomationProperties.NameProperty);
        ShowAvailability();
    }

    static StackPanel ExternalLabel(string text)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        row.Children.Add(new TextBlock { Text = text });
        row.Children.Add(new FontIcon { Glyph = "\uE8A7", FontSize = 12 });
        return row;
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

        // Facts, a label and its value each. Studio and the season are in the metadata line.
        var facts = new Grid { ColumnSpacing = 12, RowSpacing = 6 };
        facts.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(72) });
        facts.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        void Fact(string label, string? value)
        {
            if (string.IsNullOrEmpty(value))
                return;
            var row = facts.RowDefinitions.Count;
            facts.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var name = new TextBlock { Text = label, FontSize = 12, Style = Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.WrapWholeWords };
            var text = new TextBlock { Text = value, TextWrapping = TextWrapping.WrapWholeWords };
            Grid.SetRow(name, row);
            Grid.SetRow(text, row);
            Grid.SetColumn(text, 1);
            facts.Children.Add(name);
            facts.Children.Add(text);
        }
        Fact(T.TitleBasedOn, entry.Source is { } source ? MaterialName(source) : null);
        if (TitleFacts.Airing(entry.Airing) is { } airing)
            Fact(airing.Label, airing.Value);
        Fact(T.FilterCountry, TitleFacts.Country(entry));
        Fact(T.TitleGenres, entry.Genres is { Length: > 0 } genres ? string.Join(" · ", genres.Select(Searches.GenreName)) : null);
        host.Children.Add(facts);
        var more = new Button { Content = T.TitleMoreAbout, MinHeight = 32, HorizontalAlignment = HorizontalAlignment.Stretch, Background = Res<Brush>("RaisedBrush") };
        more.Click += (_, _) =>
        {
            if (Current() is { } now && XamlRoot is { } root)
                new TitleMoreDialog(ctx, root, now).Show();
        };
        host.Children.Add(more);
        if (BroadcastLine(entry) is { Length: > 0 } broadcast)
            host.Children.Add(Ui.Haze(broadcast));

        if (Anistream.App.TitleSeasons.Related(entry) is { Length: > 0 } related)
        {
            var list = new StackPanel { Spacing = 6 };
            list.Children.Add(new TextBlock { Text = T.TitleRelated, Style = Res<Style>("BodyStrongTextBlockStyle") });
            foreach (var r in related.Take(8))
            {
                var link = new HyperlinkButton { Padding = new Thickness(0), HorizontalAlignment = HorizontalAlignment.Left };
                var text = new StackPanel();
                text.Children.Add(new TextBlock { Text = r.Title, Style = Res<Style>("ShowTitleStyle") });
                text.Children.Add(new TextBlock { Text = RelationLine(r.Relation, r.Format), Style = Res<Style>("HazeCaptionStyle") });
                link.Content = text;
                var target = r.Id;
                link.Click += (_, _) => ctx.OpenTitle(target);
                HoverPreview.Attach(link, target);
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
            var item = new EpisodeItem(r, spoiler, id);
            // A spoiler's still loads too, and shows blurred.
            ctx.Art.Bind(r.Thumbnail, s => item.Still = s);
            return item;
        }).ToList();
        EpisodeList.ItemsSource = items;
        ShowDownloads(ctx.Known.Downloads);
        var metadataOnly = rows.Length > 0 && rows.All(r => r.MetadataOnly);
        EpisodesNote.Text = metadataOnly ? T.TitleEpisodesNoSource : "";
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
        Layout();
        if (Current() is { } entry)
            ShowDetails(entry);
    }

    void OnPrimary(object sender, RoutedEventArgs e)
    {
        if (action is { Playable: true })
            ctx.Play(id, action.Episode.ToString());
        else if (Avail is { Case: AvailabilityCase.OnService, Main: { } main })
            OpenService(main);
    }

    void OnEpisodeClick(object sender, ItemClickEventArgs e)
    {
        if (e.ClickedItem is EpisodeItem { Playable: true } item)
            ctx.Play(id, item.Row.Number);
        else if (e.ClickedItem is EpisodeItem off && Avail is { Case: AvailabilityCase.OnService, Main: { } main } a
                 && uint.TryParse(off.Row.Number, out var n))
        {
            // An episode with its own page opens there; any other opens the title's link.
            var own = a.Episodes?.FirstOrDefault(l => l.Episode == n);
            OpenService(own is null ? main : new ServiceLink(own.Service, own.Url), n);
        }
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

    // The episode's discussion, in a pane on the trailing side; over the page when the window is narrow.

    DiscussionPane? discussion;
    FrameworkElement? opener;
    Microsoft.UI.Xaml.Shapes.Rectangle? scrim;

    void OnEpisodeDiscussion(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item && uint.TryParse(item.Row.Number, out var episode))
            OpenDiscussion(episode, sender as FrameworkElement);
    }

    internal void OpenDiscussion(uint episode, FrameworkElement? from)
    {
        discussion?.Close();
        opener = from;
        var pane = new DiscussionPane(ctx, id, episode);
        discussion = pane;
        pane.Closed += () =>
        {
            if (discussion != pane)
                return;
            discussion = null;
            Shell.Children.Remove(pane);
            if (scrim is not null)
                Shell.Children.Remove(scrim);
            scrim = null;
            ShellColumn(false);
            if (opener?.XamlRoot is not null)
                opener.Focus(FocusState.Programmatic);
        };
        Shell.Children.Add(pane);
        PlaceDiscussion();
        pane.Start();
    }

    /// <summary>Beside the page from 1000 epx of window; over it, with a light dismiss, below.</summary>
    void PlaceDiscussion()
    {
        if (discussion is not { } pane)
            return;
        var window = XamlRoot?.Size.Width ?? Shell.ActualWidth;
        var over = window > 0 && window < DiscussionPane.OverlayBelow;
        Grid.SetRow(pane, 0);
        if (over)
        {
            ShellColumn(false);
            Grid.SetColumn(pane, 0);
            Grid.SetColumnSpan(pane, 2);
            if (scrim is null)
            {
                scrim = new Microsoft.UI.Xaml.Shapes.Rectangle { Fill = (Brush)Application.Current.Resources["ScrimBrush"] };
                Grid.SetColumnSpan(scrim, 2);
                scrim.Tapped += (_, _) => discussion?.Close();
                Shell.Children.Insert(Shell.Children.IndexOf(pane), scrim);
            }
        }
        else
        {
            if (scrim is not null)
                Shell.Children.Remove(scrim);
            scrim = null;
            Grid.SetColumn(pane, 1);
            Grid.SetColumnSpan(pane, 1);
            ShellColumn(true);
        }
    }

    void ShellColumn(bool pane) =>
        Shell.ColumnDefinitions[1].Width = pane ? GridLength.Auto : new GridLength(0);

    void OnShellSized(object sender, SizeChangedEventArgs e) => PlaceDiscussion();

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

    void OnWatchParty(object sender, RoutedEventArgs e)
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
        var from = new ComboBox { Header = T.DownloadRangeFrom, MinWidth = 120 };
        var to = new ComboBox { Header = T.DownloadRangeTo, MinWidth = 120 };
        foreach (var n in playable)
        {
            var name = long.TryParse(n, out var whole) ? T.EpisodeNumber(whole) : T.EpisodeNumberText(n);
            from.Items.Add(name);
            to.Items.Add(name);
        }
        from.SelectedIndex = start;
        to.SelectedIndex = playable.Length - 1;
        var pick = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        pick.Children.Add(from);
        pick.Children.Add(to);
        var body = new StackPanel { Spacing = 12 };
        body.Children.Add(pick);
        body.Children.Add(Ui.Text(T.DownloadRangeQualityNote, "HazeCaptionStyle"));
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.DownloadRangeTitle,
            Content = body,
            PrimaryButtonText = T.TitleEpisodeDownload,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Primary,
        };
        void Check() => dialog.IsPrimaryButtonEnabled = from.SelectedIndex >= 0 && to.SelectedIndex >= from.SelectedIndex;
        from.SelectionChanged += (_, _) => Check();
        to.SelectionChanged += (_, _) => Check();
        if (await Dialogs.Show(dialog) != ContentDialogResult.Primary)
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
        if (ctx.Known.Entry(id) is { } entry)
            TrailerWindow.Play(ctx, entry);
    }

    static string StatusName(LibrarySegment status) => status switch
    {
        LibrarySegment.Watching => T.LibraryStatusWatching,
        LibrarySegment.Planning => T.LibraryStatusPlanning,
        LibrarySegment.Completed => T.LibraryStatusCompleted,
        LibrarySegment.Paused => T.LibraryStatusPaused,
        _ => T.LibraryStatusDropped,
    };

    static string MaterialName(SourceMaterial source) => source switch
    {
        SourceMaterial.Original => T.MaterialOriginal,
        SourceMaterial.Manga => T.MaterialManga,
        SourceMaterial.LightNovel => T.MaterialLightNovel,
        SourceMaterial.VisualNovel => T.MaterialVisualNovel,
        SourceMaterial.VideoGame => T.MaterialVideoGame,
        SourceMaterial.Novel => T.MaterialNovel,
        SourceMaterial.Doujinshi => T.MaterialDoujinshi,
        SourceMaterial.Anime => T.MaterialAnime,
        SourceMaterial.WebNovel => T.MaterialWebNovel,
        SourceMaterial.LiveAction => T.MaterialLiveAction,
        SourceMaterial.Game => T.MaterialGame,
        SourceMaterial.Comic => T.MaterialComic,
        SourceMaterial.MultimediaProject => T.MaterialMultimedia,
        SourceMaterial.PictureBook => T.MaterialPictureBook,
        _ => T.RelationOther,
    };

    /// <summary>A related title's line: "Sequel · TV". AniList's relation comes as its wire name, "SIDE_STORY".</summary>
    static string RelationLine(string relation, string? format)
    {
        var name = relation.ToUpperInvariant() switch
        {
            "PREQUEL" => T.RelationPrequel,
            "SEQUEL" => T.RelationSequel,
            "PARENT" => T.RelationParent,
            "SIDE_STORY" => T.RelationSideStory,
            "SPIN_OFF" => T.RelationSpinOff,
            "ALTERNATIVE" => T.RelationAlternative,
            "SUMMARY" => T.RelationSummary,
            "COMPILATION" => T.RelationCompilation,
            "CONTAINS" => T.RelationContains,
            "SAME_UNIVERSE" => T.RelationSameUniverse,
            "CHARACTER" => T.RelationSharesCharacters,
            "ADAPTATION" => T.RelationAdaptation,
            "SOURCE" => T.RelationOriginalWork,
            _ => T.RelationOther,
        };
        return format is { Length: > 0 } f ? T.TitleRelationFormat(name, Wording.Format(f)) : name;
    }

    /// <summary>
    /// What most recently aired and what is next: "E5 out 2d ago · Episode 6 airs in 4d 20h". Latest
    /// first, so a narrow column cuts the countdown before the news. A finished show has neither.
    /// </summary>
    static string BroadcastLine(Entry entry)
    {
        var parts = new List<string>();
        // An episode known out only because the next one's number says so has no time to date it.
        uint? proven = entry.NextEpisode is { } next && next >= 2 ? next - 1 : null;
        if (entry.LastAired is { } aired && !(proven > aired.Episode))
            parts.Add(T.TitleEpisodeOutAgo(aired.Episode, Wording.Ago(aired.Ago)));
        else if (proven is { } out_)
            parts.Add(T.CalendarEpisodeOut(out_));
        if (entry.NextAiringIn is { } seconds && seconds > 0)
            parts.Add(entry.NextEpisode is { } coming ? T.EpisodeAirs(coming, Wording.In(seconds)) : T.TitleNextAirs(Wording.In(seconds)));
        return string.Join(" · ", parts);
    }

    void OnOpenAnilist(object sender, RoutedEventArgs e) =>
        ctx.Send(new Command.OpenExternal($"https://anilist.co/anime/{id}"));
}
