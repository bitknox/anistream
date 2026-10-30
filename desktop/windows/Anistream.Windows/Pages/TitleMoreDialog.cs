using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

namespace Anistream.Windows.Pages;

/// <summary>
/// More about this show: who the viewer follows who has it, its rankings and cast on the left;
/// where it streams, its staff and tags on the right; similar shows under both. Close is the
/// only button. Spoiler tags stay folded away until the viewer opens them.
/// </summary>
sealed class TitleMoreDialog
{
    readonly Ctx ctx;
    readonly Entry entry;
    readonly ContentDialog dialog;
    readonly StackPanel tagRow = new() { Spacing = 4 };
    bool spoilers;

    public TitleMoreDialog(Ctx ctx, XamlRoot root, Entry entry)
    {
        this.ctx = ctx;
        this.entry = entry;
        var header = new StackPanel { Spacing = 2 };
        header.Children.Add(Ui.Text(entry.Title, "TitleTextBlockStyle"));
        header.Children.Add(Ui.Haze(T.TitleMoreAbout));
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = header,
            Content = Body(),
            CloseButtonText = T.ActionClose,
            DefaultButton = ContentDialogButton.None,
            CloseButtonStyle = Close(),
        };
        dialog.Resources["ContentDialogMaxWidth"] = 820.0;
        dialog.Resources["ContentDialogMaxHeight"] = 700.0;
    }

    /// <summary>Close is the only button, 120 by 32 and not amber.</summary>
    static Style Close()
    {
        var style = new Style(typeof(Button)) { BasedOn = Ui.Res<Style>("DefaultButtonStyle") };
        style.Setters.Add(new Setter(FrameworkElement.WidthProperty, 120.0));
        style.Setters.Add(new Setter(FrameworkElement.HeightProperty, 32.0));
        return style;
    }

    public async void Show() => await dialog.ShowAsync();

    static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = Ui.Res<Style>("BodyStrongTextBlockStyle"),
    };

    UIElement Body()
    {
        var detail = entry.Detail;
        var left = new StackPanel { Spacing = 20 };
        var right = new StackPanel { Spacing = 20 };

        if (detail is { Following.Length: > 0 } && Follows(detail) is { } follows)
            left.Children.Add(follows);
        if (detail is { Rankings.Length: > 0 })
        {
            var rankings = Section(T.MoreRankings);
            foreach (var r in detail.Rankings)
                rankings.Children.Add(Ui.Text(TitleFacts.Ranking(r)));
            left.Children.Add(rankings);
        }
        if (detail is { Characters.Length: > 0 })
            left.Children.Add(Cast(detail.Characters));

        if (detail is { StreamingLinks.Length: > 0 })
            right.Children.Add(Streaming(detail.StreamingLinks));
        if (detail is { Staff.Length: > 0 })
        {
            var staff = Section(T.MoreStaff);
            foreach (var s in detail.Staff.Take(10))
            {
                var row = new StackPanel();
                row.Children.Add(Ui.Text(s.Name));
                row.Children.Add(Ui.Text(s.Role, "HazeCaptionStyle"));
                staff.Children.Add(row);
            }
            right.Children.Add(staff);
        }
        if (detail is { Tags.Length: > 0 })
            right.Children.Add(Tags(detail.Tags));

        var columns = new Grid { ColumnSpacing = 32 };
        columns.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        columns.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(260) });
        columns.Children.Add(left);
        Grid.SetColumn(right, 1);
        columns.Children.Add(right);

        var page = new StackPanel { Spacing = 24 };
        if (left.Children.Count + right.Children.Count == 0)
            page.Children.Add(Ui.Haze(T.MoreEmpty));
        else
            page.Children.Add(columns);
        if (detail is { Recommendations.Length: > 0 })
            page.Children.Add(Similar(detail.Recommendations));
        return new ScrollViewer
        {
            Content = page,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            Padding = new Thickness(0, 0, 16, 0),
            Width = 780,
        };
    }

    static StackPanel Section(string heading)
    {
        var s = new StackPanel { Spacing = 6 };
        s.Children.Add(Heading(heading));
        return s;
    }

    UIElement? Follows(TitleDetail detail)
    {
        var section = Section(T.MoreFollowing);
        section.Children.Add(Ui.Text(T.MoreFollowingSource, "HazeCaptionStyle"));
        foreach (var f in detail.Following)
        {
            var row = new Grid { ColumnSpacing = 12 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.Children.Add(Ui.Text(f.Name, wrap: false));
            var line = Ui.Haze(TitleFacts.Follow(f, Status), wrap: false);
            Grid.SetColumn(line, 1);
            row.Children.Add(line);
            section.Children.Add(row);
        }
        return section;
    }

    static string Status(LibrarySegment status) => status switch
    {
        LibrarySegment.Watching => T.LibraryStatusWatching,
        LibrarySegment.Planning => T.LibraryStatusPlanning,
        LibrarySegment.Completed => T.LibraryStatusCompleted,
        LibrarySegment.Paused => T.LibraryStatusPaused,
        _ => T.LibraryStatusDropped,
    };

    UIElement Cast(CastMember[] cast)
    {
        var section = Section(T.MoreCast);
        section.Children.Add(Ui.Text(T.MoreCastVoices, "HazeCaptionStyle"));
        var grid = new Grid { ColumnSpacing = 16, RowSpacing = 10 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var at = 0;
        foreach (var c in cast.Take(12))
        {
            if (at % 2 == 0)
                grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var cell = Portrait(c);
            Grid.SetRow(cell, at / 2);
            Grid.SetColumn(cell, at % 2);
            grid.Children.Add(cell);
            at++;
        }
        section.Children.Add(grid);
        return section;
    }

    FrameworkElement Portrait(CastMember c)
    {
        var cell = new Grid { ColumnSpacing = 10 };
        cell.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        cell.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var art = new Grid { Width = 40, Height = 40, CornerRadius = new CornerRadius(20), Background = Ui.Res<Brush>("RaisedBrush") };
        var image = new Image { Stretch = Stretch.UniformToFill };
        art.Children.Add(image);
        ctx.Art.Bind(c.ImageUrl, s => image.Source = s);
        cell.Children.Add(art);
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(c.Name, wrap: false));
        var role = c.Role switch
        {
            CastRole.Main => T.MoreRoleMain,
            CastRole.Supporting => T.MoreRoleSupporting,
            _ => T.MoreRoleBackground,
        };
        var voice = c.VoiceActors.FirstOrDefault();
        text.Children.Add(Ui.Text(voice is null ? role : $"{role} · {voice.Name}", "HazeCaptionStyle", wrap: false));
        Grid.SetColumn(text, 1);
        cell.Children.Add(text);
        return cell;
    }

    UIElement Streaming(StreamingLink[] links)
    {
        var section = Section(T.MediaStreamingOn);
        foreach (var link in links)
        {
            var row = new StackPanel { Spacing = 0 };
            var button = new HyperlinkButton { Padding = new Thickness(0), HorizontalAlignment = HorizontalAlignment.Left };
            var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            content.Children.Add(new TextBlock { Text = link.Site, TextDecorations = global::Windows.UI.Text.TextDecorations.Underline });
            content.Children.Add(new FontIcon { Glyph = "", FontSize = 12, VerticalAlignment = VerticalAlignment.Center });
            button.Content = content;
            button.Foreground = Ui.Res<Brush>("TextFillColorPrimaryBrush");
            var url = link.Url;
            button.Click += (_, _) => ctx.Send(new Command.OpenExternal(url));
            row.Children.Add(button);
            if (link.Language is { Length: > 0 } language)
                row.Children.Add(Ui.Text(T.MediaLinkLanguage(language), "HazeCaptionStyle"));
            section.Children.Add(row);
        }
        section.Children.Add(Ui.Text(T.MediaStreamingRegionNote, "HazeCaptionStyle"));
        return section;
    }

    UIElement Tags(TitleTag[] tags)
    {
        var section = Section(T.TitleTags);
        var plain = tags.Where(t => !(t.MediaSpoiler || t.GeneralSpoiler)).Select(t => t.Name).ToList();
        var hidden = tags.Where(t => t.MediaSpoiler || t.GeneralSpoiler).Select(t => t.Name).ToList();
        if (plain.Count > 0)
            section.Children.Add(Ui.Haze(string.Join(" · ", plain)));
        if (hidden.Count == 0)
            return section;
        var spoilerText = Ui.Haze(string.Join(" · ", hidden));
        spoilerText.Opacity = 0;
        spoilerText.Visibility = Visibility.Collapsed;
        spoilerText.OpacityTransition = new ScalarTransition { Duration = TimeSpan.FromMilliseconds(200) };
        var toggle = new HyperlinkButton { Padding = new Thickness(0), Content = T.MoreShowSpoilerTags(hidden.Count) };
        toggle.Click += (_, _) =>
        {
            spoilers = !spoilers;
            toggle.Content = spoilers ? T.MoreHideSpoilerTags : T.MoreShowSpoilerTags(hidden.Count);
            if (spoilers)
            {
                spoilerText.Visibility = Visibility.Visible;
                spoilerText.Opacity = 1;
            }
            else
            {
                spoilerText.Opacity = 0;
                spoilerText.Visibility = Visibility.Collapsed;
            }
        };
        section.Children.Add(spoilerText);
        section.Children.Add(toggle);
        return section;
    }

    UIElement Similar(LinkedTitle[] titles)
    {
        var section = Section(T.MoreLikeThis);
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        foreach (var t in titles.Take(8))
        {
            var button = new Button { Padding = new Thickness(0), Background = null, BorderThickness = new Thickness(0), Width = 88 };
            var art = new Grid { Width = 88, Height = 132, CornerRadius = new CornerRadius(6), Background = Ui.Res<Brush>("RaisedBrush") };
            var image = new Image { Stretch = Stretch.UniformToFill };
            art.Children.Add(image);
            ctx.Art.Bind(t.CoverUrl, s => image.Source = s);
            button.Content = art;
            AutomationProperties.SetName(button, t.Title);
            ToolTipService.SetToolTip(button, t.Title);
            var target = t.Id;
            button.Click += (_, _) =>
            {
                dialog.Hide();
                ctx.OpenTitle(target);
            };
            row.Children.Add(button);
        }
        section.Children.Add(new ScrollViewer
        {
            Content = row,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            HorizontalScrollMode = ScrollMode.Auto,
        });
        return section;
    }
}
