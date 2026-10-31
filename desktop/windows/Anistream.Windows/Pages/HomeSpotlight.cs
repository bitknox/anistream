using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// The spotlight: one title, its text beside the art (56% of the width, 16:9, radius 16) with
/// no scrim and no gradient, or under the art when the window is narrow. A change of title or
/// case crossfades in place between two faces.
/// </summary>
public sealed partial class HomeSpotlight : Grid
{
    /// <summary>Narrower than this the art sits above the text.</summary>
    const double SideBySide = 860;

    HomePage page = null!;
    SpotFace? shown;

    public HomeSpotlight()
    {
        Visibility = Visibility.Collapsed;
    }

    /// <summary>The page whose actions the buttons run.</summary>
    internal void Use(HomePage owner) => page = owner;

    public uint? Id => shown?.Spot.Entry.Id;

    /// <summary>The main action's button, for the first focus and for a test.</summary>
    public Button? Main => shown?.Main;

    /// <summary>Draws <paramref name="spot"/>; with <paramref name="crossfade"/> the old face fades out as the new one fades in.</summary>
    public void Show(Spotlight? spot, bool crossfade)
    {
        if (spot is null)
        {
            if (shown is { } gone)
            {
                shown = null;
                Motion.Hide(gone);
                Visibility = Visibility.Collapsed;
            }
            return;
        }
        var face = new SpotFace(page, spot);
        var old = shown;
        shown = face;
        Children.Add(face);
        if (old is not null && crossfade && Motion.Enabled)
        {
            face.Opacity = 0;
            Motion.Fade(face, 1);
            Motion.Fade(old, 0);
            _ = Task.Delay(Motion.FadeOutTime).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Children.Remove(old)));
        }
        else
        {
            if (old is not null)
                Children.Remove(old);
            if (Visibility != Visibility.Visible)
                Motion.FadeIn(this);
        }
        Visibility = Visibility.Visible;
    }

    /// <summary>The planned state of the shown title changed (Adding, then On your Planning list).</summary>
    public void Refresh() => shown?.Words();

    sealed partial class SpotFace : Grid
    {
        readonly HomePage page;
        readonly ColumnDefinition textColumn = new() { Width = new GridLength(1, GridUnitType.Star) };
        readonly ColumnDefinition gap = new() { Width = new GridLength(40) };
        readonly ColumnDefinition artColumn = new() { Width = GridLength.Auto };
        readonly RowDefinition artRow = new() { Height = GridLength.Auto };
        readonly RowDefinition textRow = new() { Height = GridLength.Auto };
        readonly Border art;
        readonly StackPanel text = new() { VerticalAlignment = VerticalAlignment.Center };
        readonly Button main = new() { Style = Ui.Res<Style>("AccentButtonStyle"), MinHeight = 32 };
        readonly Button second = new() { MinHeight = 32 };
        readonly Button more = new() { MinHeight = 32, MinWidth = 32, Padding = new Thickness(0), Content = new FontIcon { Glyph = "", FontSize = 16 } };
        bool wide = true;

        public Spotlight Spot { get; }
        public Button Main => main;

        public SpotFace(HomePage page, Spotlight spot)
        {
            this.page = page;
            Spot = spot;
            ColumnDefinitions.Add(textColumn);
            ColumnDefinitions.Add(gap);
            ColumnDefinitions.Add(artColumn);
            RowDefinitions.Add(textRow);
            RowDefinitions.Add(artRow);

            var cover = new CoverItem(spot.Entry).Load(Ctx.Current.Art, withBanner: true);
            art = HomeArt.Make(cover, HomeWording.ArtMaxWidth, HomeWording.ArtMaxWidth * 9 / 16, 16, wide: true);
            art.Tapped += (_, _) => page.OpenTitle(spot.Entry.Id);
            AutomationProperties.SetAccessibilityView(art, AccessibilityView.Raw);

            BuildText();
            Children.Add(text);
            Children.Add(art);
            Arrange(true);
            SizeChanged += (_, e) =>
            {
                var side = e.NewSize.Width >= SideBySide;
                if (side != wide)
                    Arrange(side);
                SizeArt(e.NewSize.Width);
            };
            AutomationProperties.SetName(text, HomeWording.Name(spot));
            AutomationProperties.SetLocalizedLandmarkType(text, T.HomeSpotlightContinuing);
        }

        void Arrange(bool side)
        {
            wide = side;
            textColumn.Width = side ? new GridLength(1, GridUnitType.Star) : new GridLength(1, GridUnitType.Star);
            gap.Width = side ? new GridLength(40) : new GridLength(0);
            artColumn.Width = side ? GridLength.Auto : new GridLength(0);
            Grid.SetColumn(text, 0);
            Grid.SetRow(text, side ? 0 : 1);
            Grid.SetColumn(art, side ? 2 : 0);
            Grid.SetColumnSpan(art, side ? 1 : 3);
            Grid.SetRow(art, 0);
            text.Margin = new Thickness(0, side ? 0 : 16, 0, 0);
            if (!side)
            {
                art.HorizontalAlignment = HorizontalAlignment.Left;
            }
        }

        void SizeArt(double width)
        {
            var w = wide ? Math.Min(HomeWording.ArtMaxWidth, Math.Round(width * HomeWording.ArtShare)) : Math.Min(HomeWording.ArtMaxWidth, width);
            if (w <= 0 || Math.Abs(art.Width - w) < 0.5)
                return;
            art.Width = w;
            art.Height = Math.Round(w * 9 / 16);
        }

        void BuildText()
        {
            var s = Spot;
            var entry = s.Entry;
            text.Children.Add(new TextBlock { Text = s.Label.Text(), Style = Ui.Res<Style>("HazeBodyStyle"), FontSize = 14, LineHeight = 19, Margin = new Thickness(0, 0, 0, 8) });
            var title = new TextBlock
            {
                Text = entry.Title,
                FontFamily = Ui.Res<FontFamily>("ShowFont"),
                FontWeight = FontWeights.Bold,
                FontSize = 30,
                LineHeight = 38,
                MaxLines = 2,
                TextWrapping = TextWrapping.Wrap,
                TextTrimming = TextTrimming.CharacterEllipsis,
            };
            AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level2);
            text.Children.Add(title);
            if (entry.Secondary is { Length: > 0 } secondary && secondary != entry.Title)
                text.Children.Add(new TextBlock { Text = secondary, Style = Ui.Res<Style>("HazeCaptionStyle"), Margin = new Thickness(0, 4, 0, 0) });
            var facts = HomeWording.Facts(entry);
            if (s.Starts is { } starts)
                facts = string.Join(" · ", new[] { facts, T.UpNextStarts(UpNextWording.Date(starts, DateTimeOffset.Now)) }.Where(x => x.Length > 0));
            if (facts.Length > 0)
                text.Children.Add(new TextBlock
                {
                    Text = facts.ToUpper(System.Globalization.CultureInfo.CurrentCulture),
                    Style = Ui.Res<Style>("HazeCaptionStyle"),
                    CharacterSpacing = 80,
                    FontWeight = FontWeights.Medium,
                    Margin = new Thickness(0, 10, 0, 0),
                });
            var synopsis = new TextBlock
            {
                Style = Ui.Res<Style>("HazeBodyStyle"),
                FontSize = 14,
                LineHeight = 20,
                MaxLines = 3,
                MaxWidth = 440,
                HorizontalAlignment = HorizontalAlignment.Left,
                TextWrapping = TextWrapping.Wrap,
                TextTrimming = TextTrimming.CharacterEllipsis,
                Margin = new Thickness(0, 12, 0, 0),
            };
            SynopsisText.Fill(synopsis, entry);
            if (synopsis.Inlines.Count > 0 || synopsis.Text.Length > 0)
                text.Children.Add(synopsis);

            if (s.Line is { } line)
            {
                var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, Margin = new Thickness(0, 16, 0, 0) };
                if (s.Resume?.Fraction is { } fraction)
                {
                    var track = new Border { Width = 200, Height = 3, CornerRadius = new CornerRadius(2), Background = Ui.Res<Brush>("RuleBrush"), VerticalAlignment = VerticalAlignment.Center };
                    track.Child = new Border { Height = 3, Width = 200 * Math.Clamp(fraction, 0, 1), HorizontalAlignment = HorizontalAlignment.Left, CornerRadius = new CornerRadius(2), Background = Ui.Res<Brush>("SilkBrush") };
                    row.Children.Add(track);
                    row.Children.Add(new TextBlock { Text = line.Text(), Style = Ui.Res<Style>("HazeCaptionStyle") });
                }
                else
                {
                    var dot = Ui.Dot(Colours.Water, false);
                    dot.Width = dot.Height = 6;
                    row.Spacing = 6;
                    row.Children.Add(dot);
                    row.Children.Add(new TextBlock { Text = line.Text(), Style = Ui.Res<Style>("HazeCaptionStyle"), Foreground = Ui.Res<Brush>("SilkBrush") });
                }
                text.Children.Add(row);
            }

            var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 18, 0, 0) };
            buttons.Children.Add(main);
            buttons.Children.Add(second);
            buttons.Children.Add(more);
            text.Children.Add(buttons);
            AutomationProperties.SetName(more, T.HomeMore);
            ToolTipService.SetToolTip(more, T.HomeMore);
            more.Flyout = MoreMenu();
            main.Click += (_, _) => page.MainAction(Spot, Effective());
            second.Click += (_, _) => page.SecondAction(Spot, Secondary());
            if (s.Playable == Playable.NotFound)
                text.Children.Add(new TextBlock { Text = T.HomeNotFound, Style = Ui.Res<Style>("HazeCaptionStyle"), Margin = new Thickness(0, 12, 0, 0) });
            Words();
        }

        HomeAction Effective() => HomeWording.Effective(Spot.Action, Spot.Playable);

        /// <summary>What the second button is: Details, Add to Planning, or the state it is in once added.</summary>
        SpotSecond Secondary()
        {
            var planned = page.PlannedHere.Contains(Spot.Entry.Id);
            if (Effective() == HomeAction.AddToPlanning && planned)
                return SpotSecond.Planned;
            if (Spot.Case == SpotlightCase.Continuing || Effective() != HomeAction.Play)
                return SpotSecond.Details;
            if (planned)
                return SpotSecond.Planned;
            return Spot.Reason == UpNextReason.Planning ? SpotSecond.Details : SpotSecond.Add;
        }

        /// <summary>The words on the buttons, from the engine's action and what was done here.</summary>
        public void Words()
        {
            var action = Effective();
            var planned = page.PlannedHere.Contains(Spot.Entry.Id);
            var label = HomeWording.SpotlightAction(Spot.Action, Spot.Episode, Spot.Playable);
            if (action == HomeAction.AddToPlanning && planned)
                label = null;
            main.Visibility = label is null ? Visibility.Collapsed : Visibility.Visible;
            if (label is not null)
            {
                var icon = action == HomeAction.AddToPlanning ? "" : "";
                main.Content = Button(icon, label);
                AutomationProperties.SetName(main, label);
            }
            var s = Secondary();
            var (glyph, words) = s switch
            {
                SpotSecond.Add => ("", T.UpNextAddPlanning),
                SpotSecond.Planned => ("", page.AddingHere.Contains(Spot.Entry.Id) ? T.HomeAdding : T.UpNextReasonPlanning),
                _ => ("", T.HomeDetails),
            };
            second.Content = glyph.Length > 0 ? Button(glyph, words) : words;
            second.IsEnabled = s != SpotSecond.Planned;
            AutomationProperties.SetName(second, words);
        }

        static StackPanel Button(string glyph, string words) => new()
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { new FontIcon { Glyph = glyph, FontSize = 12 }, new TextBlock { Text = words } },
        };

        MenuFlyout MoreMenu()
        {
            var menu = new MenuFlyout();
            var s = Spot;
            if (s.Case == SpotlightCase.Continuing)
            {
                var watched = new MenuFlyoutItem { Text = T.HomeMarkWatched(s.Episode), Icon = new FontIcon { Glyph = "" } };
                watched.Click += (_, _) => page.MarkWatched(s.Entry.Id, s.Episode);
                menu.Items.Add(watched);
                var remove = new MenuFlyoutItem { Text = T.HomeRemoveContinue, Icon = new FontIcon { Glyph = "" }, KeyboardAcceleratorTextOverride = "Delete" };
                remove.Click += (_, _) => page.RemoveFromContinue(s.Entry.Id);
                menu.Items.Add(remove);
            }
            else
            {
                var no = new MenuFlyoutItem { Text = T.UpNextNotInterested, Icon = new FontIcon { Glyph = "" } };
                no.Click += (_, _) => page.NotInterested(s.Entry, s.Starts is not null);
                menu.Items.Add(no);
            }
            menu.Items.Add(new MenuFlyoutSeparator());
            var details = new MenuFlyoutItem { Text = T.HomeDetails, Icon = new FontIcon { Glyph = "" } };
            details.Click += (_, _) => page.OpenTitle(s.Entry.Id);
            menu.Items.Add(details);
            return menu;
        }
    }
}

/// <summary>What the spotlight's second button is.</summary>
enum SpotSecond
{
    Details,
    Add,
    Planned,
}
