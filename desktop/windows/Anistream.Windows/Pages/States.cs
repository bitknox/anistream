using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// The five states of one place on a page, after the spec: nothing for the first 600 ms, then
/// static placeholders in the real layout, the job's activity line under them past 4 s, one
/// crossfade to the content, and in place of content an empty state, "You're offline" or a
/// failure with Try again. The first child (from XAML) is the content.
/// </summary>
public sealed partial class StateHost : Grid
{
    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(600), Activity = TimeSpan.FromSeconds(4), Least = TimeSpan.FromMilliseconds(300);

    readonly DispatcherQueueTimer silence, activity;
    readonly StackPanel waiting = new() { Spacing = 12, Visibility = Visibility.Collapsed };
    readonly Grid message = new() { Visibility = Visibility.Collapsed };
    readonly TextBlock activityLine;
    DateTimeOffset placeholdersSince;
    bool loading, placeholdersShown, layered;

    public StateHost()
    {
        var queue = DispatcherQueue.GetForCurrentThread();
        silence = queue.CreateTimer();
        silence.Interval = Silence;
        silence.IsRepeating = false;
        silence.Tick += (_, _) => ShowWaiting();
        activity = queue.CreateTimer();
        activity.Interval = Activity;
        activity.IsRepeating = false;
        activity.Tick += (_, _) => Motion.Show(activityLine);
        activityLine = Ui.Text("", "HazeCaptionStyle");
        activityLine.Visibility = Visibility.Collapsed;
        AutomationProperties.SetLiveSetting(activityLine, AutomationLiveSetting.Polite);
    }

    /// <summary>Builds the placeholders in the content's layout.</summary>
    public Func<UIElement>? Placeholders { get; set; }

    /// <summary>The job's activity line past 4 s: "Loading this season".</summary>
    public string ActivityText { get; set; } = "Loading";

    UIElement? Content => Children.Count > 0 && Children[0] != waiting && Children[0] != message ? Children[0] : null;

    void Layer()
    {
        if (layered)
            return;
        layered = true;
        Children.Add(waiting);
        Children.Add(message);
    }

    /// <summary>
    /// A load has started. With content already showing, it stays until the answer; on a first
    /// visit the place stays empty for 600 ms, then shows placeholders.
    /// </summary>
    public void Wait(bool hasContent = false)
    {
        Layer();
        // Already waiting: a second ask does not restart the 600 ms.
        if (loading && !hasContent && (silence.IsRunning || placeholdersShown))
            return;
        loading = true;
        if (hasContent)
            return;
        Motion.Set(message, false);
        if (Content is { } content && !placeholdersShown)
            content.Visibility = Visibility.Collapsed;
        silence.Stop();
        silence.Start();
    }

    void ShowWaiting()
    {
        if (!loading)
            return;
        if (Ctx.Current.Known.Offline)
        {
            Offline();
            return;
        }
        waiting.Children.Clear();
        if (Placeholders?.Invoke() is { } placeholders)
            waiting.Children.Add(placeholders);
        activityLine.Text = ActivityText;
        activityLine.Visibility = Visibility.Collapsed;
        waiting.Children.Add(activityLine);
        placeholdersShown = true;
        placeholdersSince = DateTimeOffset.UtcNow;
        Motion.Set(waiting, true);
        activity.Stop();
        activity.Start();
    }

    /// <summary>The answer: the content, crossfaded over the placeholders if they showed.</summary>
    public async void Done()
    {
        Layer();
        loading = false;
        silence.Stop();
        activity.Stop();
        Motion.Set(message, false);
        if (placeholdersShown)
        {
            var left = Least - (DateTimeOffset.UtcNow - placeholdersSince);
            if (left > TimeSpan.Zero)
                await Task.Delay(left);
            if (loading)
                return;
            placeholdersShown = false;
            Motion.HideStandard(waiting);
            if (Content is { } content)
                Motion.FadeIn(content, Motion.Standard);
        }
        else
        {
            Motion.Set(waiting, false);
            if (Content is { } content)
                content.Visibility = Visibility.Visible;
        }
    }

    /// <summary>An answer with nothing in it: the empty state in the content's place.</summary>
    public void Empty(UIElement state)
    {
        Done();
        if (Content is { } content)
            content.Visibility = Visibility.Collapsed;
        Say(state);
    }

    /// <summary>Failed to load: the reason's first clause and Try again.</summary>
    public void Failed(string reason, Action retry)
    {
        if (Ctx.Current.Known.Offline)
        {
            Done();
            if (Content is { } c)
                c.Visibility = Visibility.Collapsed;
            Offline();
            return;
        }
        Done();
        if (Content is { } content)
            content.Visibility = Visibility.Collapsed;
        Say(States.Unavailable(Words.Failure(reason), null, "Try again", () =>
        {
            Wait();
            retry();
        }, error: true));
    }

    /// <summary>Offline with nothing cached: "You're offline" with Show downloads.</summary>
    void Offline()
    {
        loading = false;
        silence.Stop();
        activity.Stop();
        Motion.Set(waiting, false);
        placeholdersShown = false;
        Say(States.Unavailable("You're offline", "Downloaded episodes still play.", "Show downloads", () => Ctx.Current.Window.Go("downloads")));
    }

    void Say(UIElement state)
    {
        Layer();
        message.Children.Clear();
        message.Children.Add(state);
        Motion.Show(message);
    }

    /// <summary>Clears any empty, offline or failed state; the content shows.</summary>
    public void Clear()
    {
        Done();
    }
}

/// <summary>The state pieces pages share: empty states, the offline line, placeholders.</summary>
static partial class States
{
    /// <summary>
    /// An empty state: Nagare (cell 5) above a title, one line and at most one button, centred in
    /// the place it fills.
    /// </summary>
    public static StackPanel Empty(string title, string line, string? button = null, Action? act = null, int cell = 5)
    {
        var panel = new StackPanel { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, MaxWidth = 420, Margin = new Thickness(0, 48, 0, 48) };
        var nagare = new Nagare(cell) { HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 0, 0, 12) };
        panel.Children.Add(nagare);
        Words(panel, title, line, button, act, error: false);
        return panel;
    }

    /// <summary>
    /// The small unavailable state, with no Nagare: offline, failed, a filtered list. A failure
    /// has the vermilion glyph beside its title, and nothing else in vermilion.
    /// </summary>
    public static StackPanel Unavailable(string title, string? line, string? button = null, Action? act = null, bool error = false)
    {
        var panel = new StackPanel { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, MaxWidth = 420, Margin = new Thickness(0, 48, 0, 48) };
        Words(panel, title, line, button, act, error);
        return panel;
    }

    static void Words(StackPanel panel, string title, string? line, string? button, Action? act, bool error)
    {
        var heading = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center };
        if (error)
            heading.Children.Add(ErrorGlyph());
        var text = new TextBlock { Text = title, Style = Ui.Res<Style>("SubtitleTextBlockStyle"), TextWrapping = TextWrapping.WrapWholeWords, TextAlignment = TextAlignment.Center };
        AutomationProperties.SetHeadingLevel(text, AutomationHeadingLevel.Level2);
        heading.Children.Add(text);
        panel.Children.Add(heading);
        if (line is { Length: > 0 })
        {
            var words = Ui.Haze(line);
            words.TextAlignment = TextAlignment.Center;
            words.HorizontalAlignment = HorizontalAlignment.Center;
            panel.Children.Add(words);
        }
        if (button is not null && act is not null)
        {
            var b = new Button { Content = button, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 8, 0, 0), MinHeight = 32 };
            b.Click += (_, _) => act();
            panel.Children.Add(b);
        }
    }

    /// <summary>The error glyph, the one vermilion thing on an error.</summary>
    public static FontIcon ErrorGlyph(double size = 16) => new()
    {
        Glyph = "",
        FontSize = size,
        Foreground = new SolidColorBrush(Ui.Vermilion),
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>Raised plates in the cover grid's layout: covers 150 by 225 and two caption bars.</summary>
    public static UIElement Covers(int count = 12)
    {
        var grid = new ItemsRepeater
        {
            Layout = new UniformGridLayout { MinItemWidth = 150, MinColumnSpacing = 16, MinRowSpacing = 20, ItemsJustification = UniformGridLayoutItemsJustification.Start },
            ItemsSource = Enumerable.Range(0, count).ToList(),
            ItemTemplate = new CoverPlaceholderFactory(),
        };
        AutomationProperties.SetAccessibilityView(grid, AccessibilityView.Raw);
        return grid;
    }

    sealed partial class CoverPlaceholderFactory : IElementFactory
    {
        public UIElement GetElement(ElementFactoryGetArgs args)
        {
            var panel = new StackPanel { Width = 150, Spacing = 6 };
            panel.Children.Add(Plate(150, 225, 8));
            panel.Children.Add(Plate(110, 12, 3));
            panel.Children.Add(Plate(70, 10, 3));
            return panel;
        }

        public void RecycleElement(ElementFactoryRecycleArgs args) { }
    }

    /// <summary>Placeholder rows: a thumbnail plate and two bars, in a list's layout.</summary>
    public static UIElement Rows(int count, double thumbWidth, double thumbHeight)
    {
        var panel = new StackPanel { Spacing = 12 };
        for (var i = 0; i < count; i++)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 14 };
            if (thumbWidth > 0)
                row.Children.Add(Plate(thumbWidth, thumbHeight, 6));
            var bars = new StackPanel { Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
            bars.Children.Add(Plate(220, 14, 3));
            bars.Children.Add(Plate(140, 10, 3));
            row.Children.Add(bars);
            panel.Children.Add(row);
        }
        AutomationProperties.SetAccessibilityView(panel, AccessibilityView.Raw);
        return panel;
    }

    public static Border Plate(double width, double height, double radius) => new()
    {
        Width = width,
        Height = height,
        CornerRadius = new CornerRadius(radius),
        HorizontalAlignment = HorizontalAlignment.Left,
        Background = Ui.Res<Brush>("RaisedBrush"),
    };
}

/// <summary>
/// "Offline. Downloaded episodes still play." under a page's title in haze beside the glyph,
/// while the engine says there is no connection. Not a coloured banner.
/// </summary>
public sealed partial class OfflineLine : StackPanel
{
    public OfflineLine()
    {
        Orientation = Orientation.Horizontal;
        Spacing = 8;
        Margin = new Thickness(0, 4, 0, 0);
        Visibility = Visibility.Collapsed;
        Children.Add(new FontIcon { Glyph = "", FontSize = 14, Foreground = Ui.Res<Brush>("HazeBrush"), VerticalAlignment = VerticalAlignment.Center });
        Children.Add(Ui.Text("Offline. Downloaded episodes still play.", "HazeBodyStyle", wrap: false));
        AutomationProperties.SetLiveSetting(this, AutomationLiveSetting.Polite);
        Loaded += (_, _) =>
        {
            Visibility = Ctx.Current.Known.Offline ? Visibility.Visible : Visibility.Collapsed;
            Ctx.Current.Batch += Heard;
        };
        Unloaded += (_, _) => Ctx.Current.Batch -= Heard;
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (!batch.Any(e => e is Event.Connectivity))
            return;
        if (Ctx.Current.Known.Offline)
            Motion.Show(this);
        else
            Motion.Hide(this);
    }
}
