using System.Numerics;
using Anistream.App;
using Anistream.Engine.Core;
using Anistream.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;

namespace Anistream.Windows.Pages;

/// <summary>
/// Calendar: the week, Monday first, All airing or My list. Seven columns with today's raised from
/// heading to foot while the content is wide enough; an agenda of days below that. Each
/// broadcast says "E10 · 16:30", "E10 dubbed · 16:30" for a dub, "· Moved from 23:00" when it moved,
/// and carries a water dot when it has aired and is not watched. The engine's window sets how far
/// the weeks reach; weeks beyond what is loaded page in as they are approached, and a silk hairline
/// marks the time now in today's column.
/// </summary>
public sealed partial class CalendarPage : Page
{
    const double WeekWidth = 760;

    readonly Ctx ctx = Ctx.Current;
    readonly TimeZoneInfo zone = TimeZoneInfo.Local;
    readonly CalendarReach reach = new();
    int week;
    bool mine, picked, settling;
    bool? wide;
    Entry[] rows = [];
    /// <summary>Whether the engine has answered once, and the 600 ms of silence before placeholders passed.</summary>
    bool loaded, placeholders, slow, waitingNow;
    DateTimeOffset placeholdersSince;
    string? failed;
    TextBlock? nowTime;
    long nowAt;
    int airedToday = -1;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer silence, activity, minute;

    public CalendarPage()
    {
        mine = SettingsCatalog.IsOn(ctx.Known.Setting(SettingKey.CalendarMine));
        InitializeComponent();
        settling = true;
        Whose.SelectedItem = mine ? MineItem : AllItem;
        settling = false;
        var queue = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        silence = queue.CreateTimer();
        silence.Interval = TimeSpan.FromMilliseconds(600);
        silence.IsRepeating = false;
        silence.Tick += (_, _) =>
        {
            placeholders = true;
            placeholdersSince = DateTimeOffset.UtcNow;
            Show();
        };
        activity = queue.CreateTimer();
        activity.Interval = TimeSpan.FromSeconds(4);
        activity.IsRepeating = false;
        activity.Tick += (_, _) =>
        {
            slow = true;
            Show();
        };
        minute = queue.CreateTimer();
        minute.Interval = TimeSpan.FromSeconds(20);
        minute.IsRepeating = true;
        minute.Tick += (_, _) => Tick();
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) =>
        {
            ctx.Batch -= Heard;
            silence.Stop();
            activity.Stop();
            minute.Stop();
        };
    }

    string TodayTip => T.CalendarTodayTooltip("Ctrl+T");

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        rows = ctx.Known.ListRows(new ContentFor.Calendar());
        loaded = ctx.Known.Lists.TryGetValue(new ContentFor.Calendar(), out var list) && list is ContentList.Entries or ContentList.Empty || rows.Length > 0;
        if (!loaded)
            silence.Start();
        minute.Start();
        ctx.Send(new Command.LoadCalendar());
        Show();
    }

    /// <summary>Asks again, past anything remembered: Try again on a failed first load, Ctrl+R.</summary>
    void Refresh()
    {
        failed = null;
        placeholders = false;
        if (!loaded)
            silence.Start();
        ctx.Send(new Command.RefreshCalendar());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        var again = false;
        var arrived = false;
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Content { View: ContentFor.Calendar } c when c.ContentValue is not ContentList.Loading:
                    rows = ctx.Known.ListRows(new ContentFor.Calendar());
                    failed = c.ContentValue is ContentList.Failed f && rows.Length == 0 ? f.V1 : null;
                    loaded = failed is null;
                    silence.Stop();
                    again = true;
                    break;
                case Event.CalendarWindow w:
                    reach.Window(w);
                    again = true;
                    break;
                case Event.CalendarBackfill b:
                    reach.Backfill(b.Page, b.MorePast);
                    if (b.Page is not null)
                        rows = [.. b.Entries, .. rows];
                    (again, arrived) = (true, true);
                    break;
                case Event.CalendarAhead a:
                    reach.Ahead(a.Page, a.MoreAhead);
                    if (a.Page is not null)
                        rows = [.. rows, .. a.Entries];
                    (again, arrived) = (true, true);
                    break;
                case Event.Settings when !picked:
                    var now = SettingsCatalog.IsOn(ctx.Known.Setting(SettingKey.CalendarMine));
                    if (now != mine)
                    {
                        mine = now;
                        settling = true;
                        Whose.SelectedItem = mine ? MineItem : AllItem;
                        settling = false;
                        again = true;
                    }
                    break;
                case Event.Progress:
                    again = true;
                    break;
            }
        }
        if (arrived)
            Arrived();
        else if (again)
            Show();
    }

    /// <summary>
    /// Pages came in: the placeholders, if they showed, stay at least 0.3 s, and the days then
    /// replace them in one standard crossfade.
    /// </summary>
    async void Arrived()
    {
        if (placeholders)
        {
            var left = TimeSpan.FromMilliseconds(300) - (DateTimeOffset.UtcNow - placeholdersSince);
            if (left > TimeSpan.Zero)
                await Task.Delay(left);
            Show();
            Motion.FadeIn(Days, Motion.Standard);
        }
        else
            Show();
    }

    long Now => DateTimeOffset.UtcNow.ToUnixTimeSeconds();

    /// <summary>The loaded broadcasts' first and last instants, whoever's list they are on.</summary>
    (long? Oldest, long? Latest) Loaded(List<Broadcast> all) =>
        all.Count == 0 ? (null, null) : (all[0].Airs, all[^1].Airs);

    /// <summary>Asks for the weeks either side of the one shown when the loaded part ends within them.</summary>
    void Ask(DateOnly monday, long? oldest, long? latest, bool retry = false)
    {
        if (!loaded || failed is not null)
            return;
        if (reach.Past(CalendarReach.Start(monday.AddDays(-7), zone), oldest, retry) is { } past)
            ctx.Send(past);
        if (reach.Later(CalendarReach.Start(monday.AddDays(14), zone), latest, retry) is { } later)
            ctx.Send(later);
    }

    /// <summary>Starts or ends the 600 ms of silence and the 4 s of the activity line with the asks that are out.</summary>
    void Track()
    {
        var out_ = reach.AskingPast || reach.AskingAhead;
        if (out_ && !waitingNow)
        {
            placeholders = false;
            slow = false;
            silence.Stop();
            silence.Start();
            activity.Stop();
            activity.Start();
        }
        else if (!out_ && waitingNow)
        {
            silence.Stop();
            activity.Stop();
            placeholders = slow = false;
        }
        waitingNow = out_;
    }

    void Show()
    {
        var anchor = ctx.Known.CalendarAnchor ?? Now;
        var monday = Calendar.WeekStart(Now, week, zone);
        WeekTitle.Text = Calendar.WeekTitle(monday);
        ThisWeek.IsEnabled = week != 0;
        Earlier.IsEnabled = reach.CanGoEarlier(monday, zone);
        Later.IsEnabled = reach.CanGoLater(monday, zone);
        var (oldest, latest) = Loaded(Calendar.Broadcasts(rows, anchor, ctx.Known, false));
        var broadcasts = Calendar.Broadcasts(rows, anchor, ctx.Known, mine);
#if ANISTREAM_TESTING
        // --show=calendar-marks: the fixture has no estimated or dubbed broadcast, so every other
        // row becomes an estimated dub, and the rest sub rows, plain, beside one, for a screenshot.
        if (ctx.Options.Show == "calendar-marks")
            broadcasts = [.. broadcasts.Select((b, i) => i % 2 == 0
                ? b with { Entry = b.Entry with { AiringApproximate = true, AiringTranslation = Anistream.Engine.Core.Translation.Dub } }
                : b with { Entry = b.Entry with { AiringTranslation = Anistream.Engine.Core.Translation.Sub } })];
#endif
        Ask(monday, oldest, latest);
        Track();
        var days = Calendar.Week(monday, broadcasts, zone);
        var today = Calendar.Day(Now, zone);
        Days.Children.Clear();
        Days.ColumnDefinitions.Clear();
        Days.RowDefinitions.Clear();
        nowTime = null;
        wide = Scroll.ActualWidth == 0 || Scroll.ActualWidth >= WeekWidth;
        if (failed is not null)
        {
            Days.Children.Add(States.Unavailable(T.CalendarLoadFailed, T.StateCheckConnection, T.ActionTryAgain, Refresh, error: true));
            return;
        }
        if (!loaded && ctx.Known.Offline)
        {
            Days.Children.Add(States.Unavailable(T.StateOfflineTitle, T.StateOfflineDetail, T.StateOfflineShowDownloads, () => ctx.Window.Go("downloads")));
            return;
        }
        var plan = new Plan(this, days, oldest, latest);
        if (wide == true)
            ShowWeek(days, today, plan);
        else
            ShowAgenda(days, today, plan);
    }

    /// <summary>What each day of the shown week is: inside the reach or not, loaded or waiting, and the lines that follow.</summary>
    sealed class Plan
    {
        public readonly bool[] InRange = new bool[7], Pending = new bool[7], PastSide = new bool[7], Past = new bool[7], Beyond = new bool[7], End = new bool[7];
        public readonly bool MineEmpty;
        public readonly bool PastFailed, AheadFailed;

        public Plan(CalendarPage page, List<(DateOnly Day, List<Broadcast> Broadcasts)> days, long? oldest, long? latest)
        {
            var reach = page.reach;
            var floor = reach.Known ? Calendar.Day(reach.Floor, page.zone) : DateOnly.MinValue;
            var ceiling = reach.Known ? Calendar.Day(reach.Ceiling, page.zone) : DateOnly.MaxValue;
            var latestDay = latest is { } l ? Calendar.Day(l, page.zone) : (DateOnly?)null;
            var any = false;
            for (var i = 0; i < 7; i++)
            {
                var day = days[i].Day;
                InRange[i] = reach.InRange(day, page.zone);
                Past[i] = day < floor;
                Beyond[i] = day > ceiling;
                var start = CalendarReach.Start(day, page.zone);
                Pending[i] = page.loaded && reach.Pending(start, CalendarReach.Start(day.AddDays(1), page.zone), oldest, latest);
                PastSide[i] = Pending[i] && oldest is { } o && CalendarReach.Start(day.AddDays(1), page.zone) <= o;
                End[i] = InRange[i] && !Pending[i] && page.loaded && !reach.MoreAhead && latestDay is { } d && day > d;
                any |= days[i].Broadcasts.Count > 0;
            }
            PastFailed = reach.PastFailed;
            AheadFailed = reach.AheadFailed;
            var open = Enumerable.Range(0, 7).Any(i => InRange[i] && !Pending[i] && !End[i]);
            MineEmpty = page.mine && page.loaded && !any && open && !Pending.Any(p => p);
        }

        public bool Failed(int i) => Pending[i] && (PastSide[i] ? PastFailed : AheadFailed);

        /// <summary>The days with nothing of their own to show: outside the reach, waiting, failed or past the end.</summary>
        public bool Plain(int i) => !InRange[i] || Pending[i] || End[i];
    }

    void ShowWeek(List<(DateOnly Day, List<Broadcast> Broadcasts)> days, DateOnly today, Plan plan)
    {
        Days.ColumnSpacing = 8;
        Days.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Days.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        for (var i = 0; i < 7; i++)
            Days.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        for (var i = 0; i < 7; i++)
        {
            var (day, broadcasts) = days[i];
            var column = new StackPanel { Spacing = 12, Padding = new Thickness(8, 10, 8, 16), CornerRadius = new CornerRadius(8) };
            if (day == today)
                column.Background = (Brush)Application.Current.Resources["CardBackgroundFillColorDefaultBrush"];
            column.Children.Add(DayHeading(day, today));
            if (!loaded)
            {
                // Never an empty day before the answer: nothing for 600 ms, then plates.
                if (placeholders)
                    for (var p = 0; p < (i % 3) + 1; p++)
                        column.Children.Add(CellPlaceholder());
            }
            else if (plan.Pending[i])
            {
                if (placeholders && !plan.Failed(i))
                    for (var p = 0; p < (i % 3) + 1; p++)
                        column.Children.Add(CellPlaceholder());
            }
            else if (!plan.Plain(i))
            {
                if (broadcasts.Count == 0 && !plan.MineEmpty)
                    column.Children.Add(Haze(T.CalendarDayEmpty));
                AddBroadcasts(column, broadcasts, day == today, true);
            }
            Grid.SetColumn(column, i);
            Days.Children.Add(column);
        }
        // One line spans each run of days that cannot show their own, under their headings.
        Run(plan, i => plan.Past[i], () => Line(T.CalendarFloor));
        Run(plan, i => plan.Beyond[i], () => Line(T.CalendarCeiling));
        Run(plan, i => plan.End[i], () => Line(T.CalendarEnd));
        Run(plan, i => plan.Failed(i) && plan.PastSide[i], () => Failure(T.CalendarLoadEarlierFailed));
        Run(plan, i => plan.Failed(i) && !plan.PastSide[i], () => Failure(T.CalendarLoadLaterFailed));
        if (plan.MineEmpty)
            Run(plan, i => !plan.Plain(i), MineEmptyLine);
        if (slow)
        {
            Run(plan, i => plan.Pending[i] && !plan.Failed(i) && plan.PastSide[i], () => Activity(T.CalendarLoadingEarlier), row: 1);
            Run(plan, i => plan.Pending[i] && !plan.Failed(i) && !plan.PastSide[i], () => Activity(T.CalendarLoadingLater), row: 1);
        }
    }

    /// <summary>Adds an element across each run of consecutive days <paramref name="pick"/> chooses.</summary>
    void Run(Plan plan, Func<int, bool> pick, Func<FrameworkElement> make, int row = 0)
    {
        for (var i = 0; i < 7; i++)
        {
            if (!pick(i))
                continue;
            var to = i;
            while (to + 1 < 7 && pick(to + 1))
                to++;
            var element = make();
            if (row == 0)
                element.Margin = new Thickness(8, 10 + 20 + 12, 8, 0);
            Grid.SetRow(element, row);
            Grid.SetColumn(element, i);
            Grid.SetColumnSpan(element, to - i + 1);
            element.VerticalAlignment = VerticalAlignment.Top;
            Days.Children.Add(element);
            i = to;
        }
    }

    static FrameworkElement Line(string text)
    {
        var line = Haze(text);
        line.TextWrapping = TextWrapping.WrapWholeWords;
        return line;
    }

    static FrameworkElement Activity(string text)
    {
        var line = Haze(text);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetLiveSetting(line, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        line.Margin = new Thickness(8, 4, 8, 0);
        return line;
    }

    /// <summary>One line with Try again where the days that could not load would be; vermilion on the glyph only.</summary>
    FrameworkElement Failure(string text)
    {
        var panel = new StackPanel { Spacing = 4, HorizontalAlignment = HorizontalAlignment.Left };
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        line.Children.Add(States.ErrorGlyph(12));
        var words = Haze(text);
        words.TextWrapping = TextWrapping.WrapWholeWords;
        line.Children.Add(words);
        panel.Children.Add(line);
        var retry = new HyperlinkButton { Content = T.ActionTryAgain, Padding = new Thickness(0), Foreground = (Brush)Application.Current.Resources["SilkBrush"] };
        retry.Click += (_, _) => Retry();
        panel.Children.Add(retry);
        return panel;
    }

    /// <summary>"Nothing on your list airs this week." with the way to All airing, in place of the days' own lines.</summary>
    FrameworkElement MineEmptyLine()
    {
        var panel = new StackPanel { Spacing = 4, HorizontalAlignment = HorizontalAlignment.Left };
        var words = Haze(T.CalendarEmptyMine);
        words.TextWrapping = TextWrapping.WrapWholeWords;
        panel.Children.Add(words);
        var all = new HyperlinkButton { Content = T.CalendarShowAll, Padding = new Thickness(0), Foreground = (Brush)Application.Current.Resources["SilkBrush"] };
        all.Click += (_, _) => ShowAll();
        panel.Children.Add(all);
        return panel;
    }

    void Retry()
    {
        var monday = Calendar.WeekStart(Now, week, zone);
        var (oldest, latest) = Loaded(Calendar.Broadcasts(rows, ctx.Known.CalendarAnchor ?? Now, ctx.Known, false));
        Ask(monday, oldest, latest, retry: true);
        Show();
    }

    void ShowAll()
    {
        Pick(false);
        settling = true;
        Whose.SelectedItem = AllItem;
        settling = false;
    }

    /// <summary>The filter chosen: remembered in the setting, and shown.</summary>
    void Pick(bool onlyMine)
    {
        mine = onlyMine;
        picked = true;
        ctx.Send(new Command.SetSetting(SettingKey.CalendarMine, new SettingValue.Bool(onlyMine)));
        Show();
    }

    /// <summary>
    /// A day's broadcasts in order. Today's column carries the silk hairline of the time now,
    /// between the last that aired and the next, with the time in 11 semibold.
    /// </summary>
    void AddBroadcasts(Panel into, List<Broadcast> broadcasts, bool isToday, bool grid)
    {
        var now = Now;
        var line = isToday && broadcasts.Count > 0;
        var placed = false;
        var aired = broadcasts.Count(b => b.Airs <= now);
        if (isToday)
            airedToday = aired;
        foreach (var b in broadcasts)
        {
            if (line && !placed && b.Airs > now)
            {
                into.Children.Add(NowLine());
                placed = true;
            }
            into.Children.Add(grid ? Cell(b) : AgendaRow(b));
        }
        if (line && !placed)
            into.Children.Add(NowLine());
    }

    FrameworkElement NowLine()
    {
        nowAt = Now;
        var time = Calendar.Time(nowAt, zone);
        nowTime = new TextBlock
        {
            Text = time,
            FontSize = 11,
            FontWeight = FontWeights.SemiBold,
            Foreground = (Brush)Application.Current.Resources["SilkBrush"],
            VerticalAlignment = VerticalAlignment.Center,
        };
        var line = new Grid { ColumnSpacing = 6, Height = 18 };
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.Children.Add(nowTime);
        var rule = new Rectangle { Height = 1, Fill = (Brush)Application.Current.Resources["SilkBrush"], VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(rule, 1);
        line.Children.Add(rule);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(line, T.CalendarNowSpoken(time));
        return line;
    }

    /// <summary>Each 20 s: the time on the hairline, and a rebuild when a broadcast has passed it.</summary>
    void Tick()
    {
        if (week != 0 || !loaded)
            return;
        if (nowTime is not null)
            nowTime.Text = Calendar.Time(Now, zone);
        var anchor = ctx.Known.CalendarAnchor ?? Now;
        var today = Calendar.Day(Now, zone);
        var passed = Calendar.Broadcasts(rows, anchor, ctx.Known, mine).Count(b => Calendar.Day(b.Airs, zone) == today && b.Airs <= Now);
        if (passed != airedToday)
            Show();
    }

    void ShowAgenda(List<(DateOnly Day, List<Broadcast> Broadcasts)> days, DateOnly today, Plan plan)
    {
        Days.ColumnSpacing = 0;
        var list = new StackPanel { Spacing = 20 };
        // Today first, then the days after it, then the ones before.
        var index = Enumerable.Range(0, 7).OrderBy(i => days[i].Day < today ? 1 : 0).ThenBy(i => days[i].Day).ToList();
        var ceilingDone = false;
        foreach (var i in index)
        {
            var (day, broadcasts) = days[i];
            if (!ceilingDone && days[i].Day < today && plan.Beyond.Any(b => b))
            {
                list.Children.Add(Line(T.CalendarCeiling));
                ceilingDone = true;
            }
            if (!plan.InRange[i])
                continue;
            if (plan.End[i])
                continue;
            if (plan.Pending[i])
            {
                var section = new StackPanel { Spacing = 8 };
                section.Children.Add(DayHeading(day, today));
                if (plan.Failed(i))
                    section.Children.Add(Failure(plan.PastSide[i] ? T.CalendarLoadEarlierFailed : T.CalendarLoadLaterFailed));
                else if (placeholders)
                    section.Children.Add(States.Rows(1, 96, 54));
                else if (slow)
                    section.Children.Add(Activity(plan.PastSide[i] ? T.CalendarLoadingEarlier : T.CalendarLoadingLater));
                list.Children.Add(section);
                continue;
            }
            if (broadcasts.Count == 0 && day != today && loaded)
                continue;
            var body = new StackPanel { Spacing = 8 };
            body.Children.Add(DayHeading(day, today));
            if (!loaded)
            {
                if (placeholders)
                    body.Children.Add(States.Rows(1, 96, 54));
            }
            else if (broadcasts.Count == 0)
            {
                if (plan.MineEmpty)
                    body.Children.Add(MineEmptyLine());
                else
                    body.Children.Add(Haze(T.CalendarEmptyToday));
            }
            AddBroadcasts(body, broadcasts, day == today, false);
            list.Children.Add(body);
        }
        if (!ceilingDone && plan.Beyond.Any(b => b))
            list.Children.Add(Line(T.CalendarCeiling));
        if (plan.End.Any(b => b))
            list.Children.Add(Line(T.CalendarEnd));
        if (plan.Past.Any(b => b))
            list.Children.Add(Line(T.CalendarFloor));
        Days.Children.Add(list);
    }

    /// <summary>The day in the system's locale; today in silk and the others in haze, by weight not colour.</summary>
    static TextBlock DayHeading(DateOnly day, DateOnly today)
    {
        var culture = System.Globalization.CultureInfo.CurrentCulture;
        var text = day == today ? T.CalendarTodayHeading(day.ToString("ddd d", culture)) : day.ToString("ddd d", culture);
        var heading = new TextBlock
        {
            Text = text,
            Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"],
        };
        if (day != today)
        {
            heading.Foreground = (Brush)Application.Current.Resources["HazeBrush"];
            heading.FontWeight = Microsoft.UI.Text.FontWeights.Normal;
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetHeadingLevel(heading, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        return heading;
    }

    static StackPanel CellPlaceholder()
    {
        var cell = new StackPanel { Spacing = 6 };
        cell.Children.Add(new Border { Height = 56, CornerRadius = new CornerRadius(6), Background = (Brush)Application.Current.Resources["RaisedBrush"] });
        cell.Children.Add(States.Plate(90, 12, 3));
        cell.Children.Add(States.Plate(60, 10, 3));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(cell, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return cell;
    }

    static TextBlock Haze(string text) => new() { Text = text, Style = (Style)Application.Current.Resources["HazeCaptionStyle"] };

    FrameworkElement Cell(Broadcast b)
    {
        var item = new CoverItem(b.Entry);
        var button = new Button
        {
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        button.Click += (_, _) => ctx.OpenTitle(b.Entry.Id);
        if (b.Watched)
        {
            // Watched: one quiet line, a dub still marked.
            var quiet = T.CalendarWatchedLine(b.Entry.Title, b.EpisodeText);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, T.CalendarWatchedSpoken(b.Entry.Title, b.EpisodeText));
            button.Content = new TextBlock { Text = quiet, Style = (Style)Application.Current.Resources["HazeCaptionStyle"] };
            return button;
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, Spoken(b));
        var panel = new StackPanel { Spacing = 4 };
        var banner = new Grid { CornerRadius = new CornerRadius(6), Background = item.Placeholder, Height = 56 };
        var image = new Image { Stretch = Stretch.UniformToFill };
        banner.Children.Add(image);
        panel.Children.Add(banner);
        ctx.Art.Bind(b.Entry.BannerUrl ?? b.Entry.CoverUrl, s => image.Source = s);
        panel.Children.Add(new TextBlock { Text = b.Entry.Title, Style = (Style)Application.Current.Resources["ShowTitleStyle"], TextWrapping = TextWrapping.WrapWholeWords, MaxLines = 2 });
        panel.Children.Add(EpisodeLine(b));
        button.Content = panel;
        return button;
    }

    FrameworkElement AgendaRow(Broadcast b)
    {
        var item = new CoverItem(b.Entry);
        var grid = new Grid { ColumnSpacing = 12, Height = 62 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(96) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var banner = new Grid { CornerRadius = new CornerRadius(6), Background = item.Placeholder, Width = 96, Height = 54 };
        var image = new Image { Stretch = Stretch.UniformToFill };
        banner.Children.Add(image);
        ctx.Art.Bind(b.Entry.BannerUrl ?? b.Entry.CoverUrl, s => image.Source = s);
        grid.Children.Add(banner);
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Spacing = 2 };
        text.Children.Add(new TextBlock { Text = b.Entry.Title, Style = (Style)Application.Current.Resources["ShowTitleStyle"] });
        text.Children.Add(EpisodeLine(b));
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        var button = new Button
        {
            Content = grid,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, Spoken(b));
        button.Click += (_, _) => ctx.OpenTitle(b.Entry.Id);
        return button;
    }

    /// <summary>What Narrator reads for a broadcast: its title and its line, the dub's mark in it and an estimate said as one.</summary>
    string Spoken(Broadcast b) => $"{b.Entry.Title}, {b.Spoken(zone)}";

    FrameworkElement EpisodeLine(Broadcast b)
    {
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        if (b.IsNew(Now))
            line.Children.Add(new Ellipse { Width = 6, Height = 6, Fill = (Brush)Application.Current.Resources["WaterBrush"], VerticalAlignment = VerticalAlignment.Center });
        // A dub is marked in the line after its episode, "E10 dubbed · 16:30", as every episode line is.
        line.Children.Add(new TextBlock { Text = b.Line(zone), Style = (Style)Application.Current.Resources["HazeCaptionStyle"], TextWrapping = TextWrapping.WrapWholeWords });
        return line;
    }

    void OnSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var nowWide = e.NewSize.Width >= WeekWidth;
        if (nowWide != wide)
            Show();
    }

    void OnWhoseChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs e)
    {
        if (settling)
            return;
        Pick(sender.SelectedItem?.Tag as string == "mine");
    }

    /// <summary>Moves to a week with the old one sliding out: one width on the standard token, towards the side it went.</summary>
    void Go(int to)
    {
        var step = Math.Sign(to - week);
        if (step == 0)
            return;
        week = to;
        Show();
        Motion.FadeIn(WeekTitle, Motion.Standard);
        var width = (float)Scroll.ActualWidth;
        if (!Motion.Enabled || width <= 0)
        {
            Motion.FadeIn(Days, Motion.Standard);
            return;
        }
        Days.TranslationTransition = null;
        Days.Translation = new Vector3(step * width, 0, 0);
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () =>
        {
            Days.TranslationTransition = new Vector3Transition { Duration = Motion.Standard };
            Days.Translation = Vector3.Zero;
        });
    }

    void OnEarlier(object sender, RoutedEventArgs e) => Go(week - 1);

    void OnLater(object sender, RoutedEventArgs e) => Go(week + 1);

    void OnThisWeek(object sender, RoutedEventArgs e) => Go(0);

    void OnAccelToday(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        if (ThisWeek.IsEnabled)
            Go(0);
    }

    void OnAccelEarlier(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        if (Earlier.IsEnabled)
            Go(week - 1);
    }

    void OnAccelLater(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        if (Later.IsEnabled)
            Go(week + 1);
    }

    void OnAccelRefresh(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        Refresh();
    }
}
