using System.ComponentModel;
using System.Text.RegularExpressions;
using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;

namespace Anistream.Windows.Pages;

/// <summary>One card or poster on a shelf: the engine's item, its art as it arrives and the words drawn from it.</summary>
public sealed class HomeCard
{
    internal HomeCard(HomeItem item, CoverItem cover, string line, string name)
    {
        Item = item;
        Cover = cover;
        Line = line;
        Name = name;
    }

    public HomeItem Item { get; }
    public CoverItem Cover { get; }
    public uint Id => Item.Entry.Id;
    public string Title => Item.Entry.Title;
    public string Line { get; }
    public string Name { get; }
    public double? Progress => Item.Progress;
}

/// <summary>Art that fades over its placeholder colour when it arrives, as covers do everywhere.</summary>
static class HomeArt
{
    public static Border Make(CoverItem cover, double width, double height, double radius, bool wide)
    {
        var image = new Image { Stretch = Stretch.UniformToFill, Opacity = 0 };
        AutomationProperties.SetAccessibilityView(image, AccessibilityView.Raw);
        var box = new Border { Width = width, Height = height, CornerRadius = new CornerRadius(radius), Background = cover.Placeholder, Child = image };
        void Fill(bool fade)
        {
            var source = wide ? cover.Banner ?? cover.Cover : cover.Cover;
            if (source is null || image.Source == source)
                return;
            image.Source = source;
            if (fade)
                Motion.Fade(image, 1);
            else
                image.Opacity = 1;
        }
        Fill(false);
        cover.PropertyChanged += (_, _) => Fill(true);
        return box;
    }
}

/// <summary>A spotlight, hover card or title synopsis with the words AniList marks as spoilers behind the Discussions bar.</summary>
static class SynopsisText
{
    public static void Fill(TextBlock block, Entry entry)
    {
        block.Inlines.Clear();
        var spans = entry.SynopsisSpans;
        if (spans is not { Length: > 0 } || !spans.Any(s => s.Spoiler))
        {
            block.Text = entry.Synopsis;
            return;
        }
        var bars = new List<(Border Bar, TextBlock Text)>();
        var open = false;
        void Set(bool on)
        {
            open = on;
            foreach (var (bar, text) in bars)
            {
                text.Opacity = on ? 1 : 0;
                bar.Opacity = on ? 0.55 : 1;
                AutomationProperties.SetName(bar, on ? text.Text : T.DiscussionSpoiler);
            }
        }
        foreach (var span in spans)
        {
            if (!span.Spoiler)
            {
                block.Inlines.Add(new Run { Text = span.Text });
                continue;
            }
            foreach (Match m in Regex.Matches(span.Text, @"\S+\s*|\s+"))
            {
                var text = new TextBlock { Text = m.Value.Replace('\n', ' '), FontSize = block.FontSize, Opacity = 0 };
                var bar = new Border
                {
                    Background = Ui.Res<Brush>("SpoilerBarBrush"),
                    CornerRadius = new CornerRadius(bars.Count == 0 ? 4 : 0, 0, 0, bars.Count == 0 ? 4 : 0),
                    Child = text,
                    IsTabStop = bars.Count == 0,
                    UseSystemFocusVisuals = true,
                };
                AutomationProperties.SetName(bar, T.DiscussionSpoiler);
                AutomationProperties.SetAutomationControlType(bar, AutomationControlType.Button);
                bar.Tapped += (_, e) =>
                {
                    e.Handled = true;
                    Set(!open);
                };
                bar.KeyDown += (_, e) =>
                {
                    if (e.Key is VirtualKey.Enter or VirtualKey.Space)
                    {
                        e.Handled = true;
                        Set(!open);
                    }
                };
                bars.Add((bar, text));
                block.Inlines.Add(new InlineUIContainer { Child = bar });
            }
        }
        Set(false);
    }
}

/// <summary>Builds the element for one <see cref="HomeCard"/>: a wide Continue card or a poster, as a flat button.</summary>
sealed partial class HomeCardFactory(HomeShape shape, HomePage page) : IElementFactory
{
    public UIElement GetElement(ElementFactoryGetArgs args) => args.Data is HomeCard card ? Build(card) : new Grid();

    public void RecycleElement(ElementFactoryRecycleArgs args) { }

    Button Build(HomeCard card)
    {
        var wide = shape == HomeShape.Card;
        var width = wide ? HomeWording.CardWidth : HomeWording.PosterWidth;
        var height = wide ? HomeWording.CardHeight : HomeWording.PosterHeight;
        var stack = new StackPanel { Width = width };
        stack.Children.Add(HomeArt.Make(card.Cover, width, height, 4, wide));
        if (wide)
        {
            var track = new Border
            {
                Height = 3,
                Margin = new Thickness(0, 6, 0, 0),
                CornerRadius = new CornerRadius(2),
                Background = Ui.Res<Brush>("RuleBrush"),
                Opacity = card.Progress is null ? 0 : 1,
            };
            var fill = new Border { Height = 3, HorizontalAlignment = HorizontalAlignment.Left, Width = width * Math.Clamp(card.Progress ?? 0, 0, 1), CornerRadius = new CornerRadius(2), Background = Ui.Res<Brush>("SilkBrush") };
            track.Child = fill;
            stack.Children.Add(track);
        }
        var title = new TextBlock { Text = card.Title, Style = Ui.Res<Style>("ShowTitleStyle"), FontSize = 14, Margin = new Thickness(0, 8, 0, 0), IsTextScaleFactorEnabled = true };
        stack.Children.Add(title);
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        if (card.Item.New)
            line.Children.Add(Ui.Dot(Colours.Water, false));
        line.Children.Add(new TextBlock { Text = card.Line, Style = Ui.Res<Style>("HazeCaptionStyle"), TextTrimming = TextTrimming.CharacterEllipsis });
        stack.Children.Add(line);
        if (card.Item.Playable == Playable.NotFound)
            stack.Children.Add(new TextBlock { Text = T.HomeNotFound, Style = Ui.Res<Style>("HazeCaptionStyle") });

        var button = new Button
        {
            Style = Ui.Res<Style>("HomeCardButtonStyle"),
            Content = stack,
            Tag = card,
            Width = width,
        };
        AutomationProperties.SetName(button, card.Name);
        button.Click += (_, _) => page.OpenCard(card, shape);
        button.ContextFlyout = page.MenuFor(card, shape);
        button.KeyDown += (_, e) => page.CardKey(card, shape, e);
        HoverPreview.Attach(button, card.Id);
        return button;
    }
}

/// <summary>
/// A shelf: the heading with its count and link, and the cards in an <see cref="ItemsRepeater"/>
/// that scrolls sideways by whole cards. The arrows show while the pointer is over the shelf.
/// </summary>
sealed partial class HomeShelf : Grid
{
    readonly HomePage page;
    readonly ScrollViewer scroll = new()
    {
        HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden,
        HorizontalScrollMode = ScrollMode.Disabled,
        VerticalScrollMode = ScrollMode.Disabled,
        VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
        IsTabStop = false,
    };
    /// <summary>The wheel moves the page, not a sideways strip; Shift and a sideways wheel move the strip.</summary>
    internal static void ForwardWheel(ScrollViewer inner, HomePage page) =>
        inner.AddHandler(PointerWheelChangedEvent, new PointerEventHandler((_, e) =>
        {
            var point = e.GetCurrentPoint(inner).Properties;
            e.Handled = true;
            if (point.IsHorizontalMouseWheel || e.KeyModifiers.HasFlag(VirtualKeyModifiers.Shift))
            {
                inner.ChangeView(Math.Clamp(inner.HorizontalOffset + point.MouseWheelDelta, 0, inner.ScrollableWidth), null, null, true);
                return;
            }
            var outer = page.PageScroll;
            outer.ChangeView(null, Math.Clamp(outer.VerticalOffset - point.MouseWheelDelta, 0, outer.ScrollableHeight), null, true);
        }), true);

    readonly ItemsRepeater repeater;
    readonly Button previous, next;
    readonly StackPanel heading = new() { Orientation = Orientation.Horizontal, Spacing = 12 };
    readonly TextBlock lead, title, aside;
    readonly HyperlinkButton viewAll;
    readonly double cardWidth;
    bool inside;

    public SectionKind Kind { get; }
    public HomeShape Shape { get; }
    public List<HomeCard> Cards { get; private set; } = [];

    public HomeShelf(HomePage page, HomeSection section, SeasonYear? season)
    {
        this.page = page;
        Kind = section.Kind;
        Shape = HomeWording.Shape(section.Kind);
        cardWidth = Shape == HomeShape.Card ? HomeWording.CardWidth : HomeWording.PosterWidth;
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0);

        var head = new Grid { Margin = new Thickness(0, 0, 0, 12) };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        lead = new TextBlock { FontSize = 20, LineHeight = 28, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Bottom, TextWrapping = TextWrapping.NoWrap };
        title = new TextBlock { FontSize = 20, LineHeight = 28, FontFamily = Ui.Res<FontFamily>("ShowFont"), FontWeight = FontWeights.Bold, TextTrimming = TextTrimming.CharacterEllipsis, VerticalAlignment = VerticalAlignment.Bottom };
        aside = new TextBlock { Style = Ui.Res<Style>("HazeBodyStyle"), VerticalAlignment = VerticalAlignment.Bottom, TextWrapping = TextWrapping.NoWrap };
        var words = new StackPanel { Orientation = Orientation.Horizontal };
        words.Children.Add(lead);
        words.Children.Add(title);
        heading.Children.Add(words);
        heading.Children.Add(aside);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        head.Children.Add(heading);
        viewAll = new HyperlinkButton { Padding = new Thickness(0), VerticalAlignment = VerticalAlignment.Bottom };
        viewAll.Click += (_, _) => page.OpenViewAll(Kind, section.ViewAll);
        Grid.SetColumn(viewAll, 1);
        head.Children.Add(viewAll);
        Children.Add(head);

        repeater = new ItemsRepeater
        {
            Layout = new StackLayout { Orientation = Orientation.Horizontal, Spacing = HomeWording.Gap },
            ItemTemplate = new HomeCardFactory(Shape, page),
        };
        scroll.Content = repeater;
        // The wheel moves the page, not the shelf; Shift and a sideways wheel move the shelf.
        ForwardWheel(scroll, page);
        scroll.ViewChanged += (_, _) => Arrows();
        scroll.SizeChanged += (_, _) => Arrows();
        Grid.SetRow(scroll, 1);
        Children.Add(scroll);

        var artHeight = Shape == HomeShape.Card ? HomeWording.CardHeight : HomeWording.PosterHeight;
        previous = Arrow("", T.HomePrevious, false, artHeight);
        next = Arrow("", T.HomeNext, true, artHeight);
        Children.Add(previous);
        Children.Add(next);

        // A transparent ground makes the whole shelf, gaps included, part of the hover.
        Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
        PointerEntered += (_, _) =>
        {
            inside = true;
            Arrows();
        };
        PointerMoved += (_, _) =>
        {
            if (inside)
                return;
            inside = true;
            Arrows();
        };
        PointerExited += (_, _) =>
        {
            inside = false;
            Arrows();
        };
#if ANISTREAM_TESTING
        // --show=arrows: the pointer is taken to be over every shelf, for a screenshot; the VM's
        // synthetic pointer does not raise PointerEntered.
        inside = Ctx.Current.Options.Show == "arrows";
#endif
        Update(section, season, []);
    }

    Button Arrow(string glyph, string name, bool forward, double artHeight)
    {
        var button = new Button
        {
            Width = 36,
            Height = 36,
            Padding = new Thickness(0),
            CornerRadius = new CornerRadius(18),
            HorizontalAlignment = forward ? HorizontalAlignment.Right : HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(forward ? 0 : -18, 40 + artHeight / 2 - 18, forward ? -18 : 0, 0),
            Background = Ui.Res<Brush>("RaisedBrush"),
            Content = new FontIcon { Glyph = glyph, FontSize = 14 },
            Opacity = 0,
            IsHitTestVisible = false,
        };
        Canvas.SetZIndex(button, 5);
        AutomationProperties.SetName(button, name);
        Grid.SetRow(button, 1);
        button.Click += (_, _) => Page(forward);
        button.GotFocus += (_, _) => Motion.Fade(button, 1);
        button.LostFocus += (_, _) => Arrows();
        return button;
    }

    /// <summary>Shows the arrows that have somewhere to go while the pointer is over the shelf, and hides the rest.</summary>
    void Arrows()
    {
        Set(previous, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, false));
        Set(next, inside && ShelfPaging.Can(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, true));
    }

    static void Set(Button arrow, bool shown)
    {
        arrow.IsHitTestVisible = shown;
        if (shown == arrow.Opacity > 0.5)
            return;
        Motion.Fade(arrow, shown ? 1 : 0);
    }

    /// <summary>A page: by the whole cards that fit, landing on a card edge, on the standard token.</summary>
    void Page(bool forward)
    {
        var to = ShelfPaging.Target(scroll.HorizontalOffset, scroll.ViewportWidth, scroll.ExtentWidth, cardWidth, Cards.Count, forward);
        Motion.Ease(scroll, scroll.HorizontalOffset, to, v => scroll.ChangeView(v, null, null, true));
    }

    /// <summary>Takes the section as the engine now has it. Cards that left fade out first when <paramref name="leaving"/> names them.</summary>
    public void Update(HomeSection section, SeasonYear? season, IReadOnlyCollection<uint> leaving)
    {
        var heading_ = HomeWording.Heading(section);
        lead.Text = heading_.Lead;
        title.Text = heading_.Title;
        title.Visibility = heading_.Title.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        var small = HomeWording.Aside(section, season);
        aside.Text = small ?? "";
        aside.Visibility = small is null ? Visibility.Collapsed : Visibility.Visible;
        var all = HomeWording.ViewAll(section);
        viewAll.Content = all is null ? null : new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 4,
            Children = { new TextBlock { Text = all, FontWeight = FontWeights.SemiBold }, new FontIcon { Glyph = "", FontSize = 10 } },
        };
        viewAll.Visibility = all is null ? Visibility.Collapsed : Visibility.Visible;
        if (all is not null)
            AutomationProperties.SetName(viewAll, all);
        var now = DateTimeOffset.Now;
        var covers = Cards.ToDictionary(c => c.Id, c => c.Cover);
        Cards = [.. section.Items.Select(item =>
        {
            var cover = covers.TryGetValue(item.Entry.Id, out var known) ? known : new CoverItem(item.Entry).Load(Ctx.Current.Art, withBanner: Shape == HomeShape.Card);
            return new HomeCard(item, cover, HomeWording.Line(item, now, TimeZoneInfo.Local), HomeWording.Name(item, now, TimeZoneInfo.Local));
        })];
        repeater.ItemsSource = Cards;
    }

    /// <summary>The element showing title <paramref name="id"/> now, if it is realised.</summary>
    public UIElement? ElementOf(uint id)
    {
        for (var i = 0; i < Cards.Count; i++)
            if (Cards[i].Id == id)
                return repeater.TryGetElement(i);
        return null;
    }

    /// <summary>A card leaves: it fades out (fade-out), then the cards after it close the gap.</summary>
    public void Leave(uint id, Action done)
    {
        if (ElementOf(id) is not { } element || !Motion.Enabled)
        {
            done();
            return;
        }
        Motion.Fade(element, 0);
        _ = Task.Delay(Motion.FadeOutTime).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => done()));
    }

    public void Focus() => repeater.TryGetElement(0)?.Focus(FocusState.Programmatic);
}
