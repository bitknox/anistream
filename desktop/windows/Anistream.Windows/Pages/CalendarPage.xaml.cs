using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;

namespace Anistream.Windows.Pages;

/// <summary>
/// Calendar: the week, Monday first, Mine or All airing. Seven columns with today's raised from
/// heading to foot while the content is wide enough; an agenda of days below that. Each
/// broadcast says "Ep 10 · 16:30", "(was 23:00)" when it moved, and carries a water dot when it
/// has aired and is not watched.
/// </summary>
public sealed partial class CalendarPage : Page
{
    const double WeekWidth = 760;

    readonly Ctx ctx = Ctx.Current;
    readonly TimeZoneInfo zone = TimeZoneInfo.Local;
    int week;
    bool mine = true;
    bool? wide;
    Entry[] rows = [];
    uint pages;
    bool morePast, askingPast;
    /// <summary>Whether the engine has answered once, and the 600 ms of silence before placeholders passed.</summary>
    bool loaded, placeholders;
    string? failed;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer silence;

    public CalendarPage()
    {
        InitializeComponent();
        silence = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().CreateTimer();
        silence.Interval = TimeSpan.FromMilliseconds(600);
        silence.IsRepeating = false;
        silence.Tick += (_, _) =>
        {
            placeholders = true;
            if (!loaded)
                Show();
        };
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        rows = ctx.Known.ListRows(new ContentFor.Calendar());
        loaded = ctx.Known.Lists.TryGetValue(new ContentFor.Calendar(), out var list) && list is ContentList.Entries or ContentList.Empty || rows.Length > 0;
        if (!loaded)
            silence.Start();
        ctx.Send(new Command.LoadCalendar());
        Show();
    }

    void Load()
    {
        failed = null;
        placeholders = false;
        silence.Start();
        ctx.Send(new Command.LoadCalendar());
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        var again = false;
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
                    (pages, morePast) = (w.Pages, w.MorePast);
                    break;
                case Event.CalendarBackfill b:
                    askingPast = false;
                    if (b.Page is { } page)
                    {
                        (pages, morePast) = (page + 1, b.MorePast);
                        rows = [.. b.Entries, .. rows];
                        again = true;
                    }
                    break;
                case Event.Progress:
                    again = true;
                    break;
            }
        }
        if (again)
            Show();
    }

    long Now => DateTimeOffset.UtcNow.ToUnixTimeSeconds();

    void Show()
    {
        var anchor = ctx.Known.CalendarAnchor ?? Now;
        var monday = Calendar.WeekStart(Now, week, zone);
        WeekTitle.Text = Calendar.WeekTitle(monday);
        ThisWeek.IsEnabled = week != 0;
        var broadcasts = Calendar.Broadcasts(rows, anchor, ctx.Known, mine);
#if ANISTREAM_TESTING
        // --show=calendar-marks: the fixture has no estimated or dubbed broadcast, so every other
        // row becomes an estimated dub, and the rest sub rows, plain, beside one, for a screenshot.
        if (ctx.Options.Show == "calendar-marks")
            broadcasts = [.. broadcasts.Select((b, i) => i % 2 == 0
                ? b with { Entry = b.Entry with { AiringApproximate = true, AiringTranslation = Anistream.Engine.Core.Translation.Dub } }
                : b with { Entry = b.Entry with { AiringTranslation = Anistream.Engine.Core.Translation.Sub } })];
#endif
        var days = Calendar.Week(monday, broadcasts, zone);
        var today = Calendar.Day(Now, zone);
        Days.Children.Clear();
        Days.ColumnDefinitions.Clear();
        Days.RowDefinitions.Clear();
        wide = Scroll.ActualWidth == 0 || Scroll.ActualWidth >= WeekWidth;
        if (failed is not null)
        {
            Days.Children.Add(States.Unavailable(Words.Failure(failed), null, "Try again", Load, error: true));
            return;
        }
        if (!loaded && ctx.Known.Offline)
        {
            Days.Children.Add(States.Unavailable("You're offline", "Downloaded episodes still play.", "Show downloads", () => ctx.Window.Go("downloads")));
            return;
        }
        if (wide == true)
            ShowWeek(days, today);
        else
            ShowAgenda(days, today);
    }

    void ShowWeek(List<(DateOnly Day, List<Broadcast> Broadcasts)> days, DateOnly today)
    {
        Days.ColumnSpacing = 8;
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
                // Never "Nothing airs" before the answer: nothing for 600 ms, then plates.
                if (placeholders)
                    for (var p = 0; p < (i % 3) + 1; p++)
                        column.Children.Add(CellPlaceholder());
            }
            else if (broadcasts.Count == 0)
                column.Children.Add(Haze("Nothing airs"));
            foreach (var b in broadcasts)
                column.Children.Add(Cell(b));
            Grid.SetColumn(column, i);
            Days.Children.Add(column);
        }
    }

    void ShowAgenda(List<(DateOnly Day, List<Broadcast> Broadcasts)> days, DateOnly today)
    {
        Days.ColumnSpacing = 0;
        var list = new StackPanel { Spacing = 20 };
        // Today first, then the days after it, then the ones before.
        var order = days.OrderBy(d => d.Day < today ? 1 : 0).ThenBy(d => d.Day);
        foreach (var (day, broadcasts) in order)
        {
            if (broadcasts.Count == 0 && day != today && loaded)
                continue;
            var section = new StackPanel { Spacing = 8 };
            section.Children.Add(DayHeading(day, today));
            if (!loaded)
            {
                if (placeholders)
                    section.Children.Add(States.Rows(1, 96, 54));
            }
            else if (broadcasts.Count == 0)
                section.Children.Add(Haze("Nothing airs today"));
            foreach (var b in broadcasts)
                section.Children.Add(AgendaRow(b));
            list.Children.Add(section);
        }
        Days.Children.Add(list);
    }

    /// <summary>The day in the system's locale; today in silk and the others in haze, by weight not colour.</summary>
    static TextBlock DayHeading(DateOnly day, DateOnly today)
    {
        var culture = System.Globalization.CultureInfo.CurrentCulture;
        var text = day == today ? $"Today · {day.ToString("ddd d", culture)}" : day.ToString("ddd d", culture);
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
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, b.Mark is { } said ? $"{b.Entry.Title}, {b.Line(zone)}, {said}" : $"{b.Entry.Title}, {b.Line(zone)}");
        button.Click += (_, _) => ctx.OpenTitle(b.Entry.Id);
        if (b.Watched)
        {
            // Watched: one quiet line.
            button.Content = new TextBlock { Text = $"{b.Entry.Title} · Ep {b.Episode}", Style = (Style)Application.Current.Resources["HazeCaptionStyle"] };
            return button;
        }
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
        button.Click += (_, _) => ctx.OpenTitle(b.Entry.Id);
        return button;
    }

    FrameworkElement EpisodeLine(Broadcast b)
    {
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        if (b.IsNew(Now))
            line.Children.Add(new Ellipse { Width = 6, Height = 6, Fill = (Brush)Application.Current.Resources["WaterBrush"], VerticalAlignment = VerticalAlignment.Center });
        line.Children.Add(new TextBlock { Text = b.Line(zone), Style = (Style)Application.Current.Resources["HazeCaptionStyle"], TextWrapping = TextWrapping.WrapWholeWords });
        // Dub: a small outlined tag after the time, quiet as the line it ends.
        if (b.Mark is { } mark)
            line.Children.Add(new Border
            {
                BorderBrush = (Brush)Application.Current.Resources["HazeBrush"],
                BorderThickness = new Thickness(1),
                CornerRadius = new CornerRadius(4),
                Padding = new Thickness(5, 0, 5, 1),
                VerticalAlignment = VerticalAlignment.Center,
                Child = new TextBlock { Text = mark, FontSize = 11, Foreground = (Brush)Application.Current.Resources["HazeBrush"] },
            });
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
        mine = sender.SelectedItem?.Tag as string != "all";
        Show();
    }

    void OnEarlier(object sender, RoutedEventArgs e)
    {
        week--;
        // An earlier week than is loaded asks for the broadcasts before it.
        if (ctx.Known.CalendarAnchor is { } anchor && morePast && !askingPast)
        {
            askingPast = true;
            ctx.Send(new Command.LoadCalendarPast(anchor, pages));
        }
        Show();
    }

    void OnLater(object sender, RoutedEventArgs e)
    {
        week++;
        Show();
    }

    void OnThisWeek(object sender, RoutedEventArgs e)
    {
        week = 0;
        Show();
    }
}
