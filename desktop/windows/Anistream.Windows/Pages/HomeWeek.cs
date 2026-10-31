using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// This week: seven columns from today, each a day's heading and up to three titles with their
/// air time and episode, then "N more" which opens the calendar.
/// </summary>
sealed partial class HomeWeek : Grid
{
    const double ColumnMin = 150;

    public HomeWeek(HomePage page, HomeSection section)
    {
        Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0);
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var head = new Grid { Margin = new Thickness(0, 0, 0, 12) };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var title = new TextBlock { Text = HomeWording.Heading(section).ToString(), FontSize = 20, LineHeight = 28, FontWeight = FontWeights.SemiBold };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
        head.Children.Add(title);
        if (HomeWording.ViewAll(section) is { } all)
        {
            var link = new HyperlinkButton
            {
                Padding = new Thickness(0),
                Content = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 4,
                    Children = { new TextBlock { Text = all, FontWeight = FontWeights.SemiBold }, new FontIcon { Glyph = "", FontSize = 10 } },
                },
            };
            AutomationProperties.SetName(link, all);
            link.Click += (_, _) => page.OpenViewAll(SectionKind.ThisWeek, section.ViewAll);
            Grid.SetColumn(link, 1);
            head.Children.Add(link);
        }
        Children.Add(head);

        var days = HomeWording.ByDay(section);
        var now = DateTimeOffset.Now;
        var strip = new Grid
        {
            ColumnSpacing = 16,
            BorderBrush = Ui.Res<Brush>("FaintRuleBrush"),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(0, 12, 0, 0),
            MinWidth = 7 * ColumnMin + 6 * 16,
        };
        for (var d = 0; d < days.Length; d++)
        {
            strip.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var column = new StackPanel { Spacing = 12 };
            var day = new TextBlock
            {
                Text = HomeWording.Day((uint)d, now),
                FontSize = 12,
                LineHeight = 16,
                FontWeight = FontWeights.SemiBold,
                Foreground = Ui.Res<Brush>(d == 0 ? "SilkBrush" : "HazeBrush"),
            };
            AutomationProperties.SetHeadingLevel(day, AutomationHeadingLevel.Level3);
            column.Children.Add(day);
            if (days[d].Count == 0)
                column.Children.Add(Ui.Text(T.HomeNothingAiring, "HazeCaptionStyle"));
            foreach (var item in days[d].Take(3))
                column.Children.Add(Entry(item, now));
            if (days[d].Count > 3)
            {
                var more = new HyperlinkButton { Content = T.HomeDayMore(days[d].Count - 3), Padding = new Thickness(0), FontSize = 12 };
                more.Click += (_, _) => page.OpenViewAll(SectionKind.ThisWeek, null);
                column.Children.Add(more);
            }
            Grid.SetColumn(column, d);
            strip.Children.Add(column);
        }
        var scroll = new ScrollViewer
        {
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollMode = ScrollMode.Auto,
            VerticalScrollMode = ScrollMode.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            IsTabStop = false,
            Content = strip,
        };
        Grid.SetRow(scroll, 1);
        Children.Add(scroll);
    }

    static Button Entry(HomeItem item, DateTimeOffset now)
    {
        var cover = new CoverItem(item.Entry).Load(Ctx.Current.Art);
        var row = new Grid { ColumnSpacing = 8, VerticalAlignment = VerticalAlignment.Top };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.Children.Add(HomeArt.Make(cover, 44, 66, 4, false));
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Top };
        var when = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        if (item.New)
            when.Children.Add(Ui.Dot(Colours.Water, false));
        var time = item.AirsAt is { } at ? HomeWording.When(at, now, TimeZoneInfo.Local) : "";
        when.Children.Add(new TextBlock { Text = time, FontSize = 12, LineHeight = 16, FontWeight = FontWeights.SemiBold, TextTrimming = TextTrimming.CharacterEllipsis });
        text.Children.Add(when);
        text.Children.Add(new TextBlock { Text = item.Entry.Title, Style = Ui.Res<Style>("ShowTitleStyle"), FontSize = 13, LineHeight = 17, MaxLines = 2, TextWrapping = TextWrapping.Wrap });
        var episode = item.Episode is { } ep ? T.EpisodeNumber(ep) : "";
        text.Children.Add(new TextBlock { Text = episode, Style = Ui.Res<Style>("HazeCaptionStyle") });
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        var button = new Button { Style = Ui.Res<Style>("HomeCardButtonStyle"), Content = row, HorizontalAlignment = HorizontalAlignment.Stretch };
        AutomationProperties.SetName(button, $"{item.Entry.Title}, {episode}, {time}");
        button.Click += (_, _) => Ctx.Current.OpenTitle(item.Entry.Id);
        return button;
    }
}
