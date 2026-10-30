using System.Runtime.CompilerServices;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Anistream.Windows.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Foundation;

namespace Anistream.Windows;

/// <summary>
/// The preview a cover opens when the pointer rests on it: the banner, the title, a facts line,
/// two lines of synopsis and the title's main action, in a flyout. Where the title has a trailer
/// and trailers may play, it plays muted over the banner as on the title page.
/// The pointer must rest: less than 4 px of movement for 800 ms, or for 300 ms when a preview
/// closed less than a second ago (scanning a row). The preview closes 150 ms after
/// the pointer leaves both the cover and the preview, and at once on a scroll, a key press, a
/// click elsewhere or the window losing focus. Keyboard focus never opens one.
/// </summary>
static class HoverPreview
{
    static readonly TimeSpan Dwell = TimeSpan.FromMilliseconds(800);
    /// <summary>The dwell for the next cover when a preview closed less than <see cref="Scanning"/> ago.</summary>
    static readonly TimeSpan NextDwell = TimeSpan.FromMilliseconds(300);
    static readonly TimeSpan Scanning = TimeSpan.FromSeconds(1);
    static DateTime closedAt = DateTime.MinValue;
    static readonly TimeSpan Leave = TimeSpan.FromMilliseconds(150);
    /// <summary>The pointer may drift this far and still be resting.</summary>
    const double Rest = 4;
    /// <summary>The video's width: 384 by 216, over YouTube's 356 by 200 floor.</summary>
    internal const double Width = 384;

    static readonly ConditionalWeakTable<FrameworkElement, Resting> targets = [];
    static Open? open;

    /// <summary>What one cover keeps while the pointer is on it.</summary>
    sealed class Resting
    {
        public Point Anchor;
        public DispatcherQueueTimer? Timer;
        public Func<uint> Id = () => 0;
    }

    /// <summary>Lets <paramref name="target"/> open a preview of title <paramref name="id"/> when the pointer rests on it.</summary>
    public static void Attach(FrameworkElement target, uint id) => Attach(target, () => id);

    /// <summary>As above, for a container that may be given another title: <paramref name="id"/> is asked when the preview opens.</summary>
    public static void Attach(FrameworkElement target, Func<uint> id)
    {
        var state = targets.GetOrCreateValue(target);
        state.Id = id;
        // A container reused for another title keeps its handlers, and takes the new id.
        if (state.Timer is not null)
            return;
        // The timer reaches its cover weakly: a handler holding it would keep the page alive.
        state.Timer = Timers.Weak(target, Dwell, false, static at =>
        {
            if (targets.TryGetValue(at, out var resting))
                Show(at, resting.Id());
        });
        target.PointerEntered += (_, e) => Resume(target, state, e);
        target.PointerMoved += (_, e) =>
        {
            var at = e.GetCurrentPoint(target).Position;
            if (Math.Abs(at.X - state.Anchor.X) >= Rest || Math.Abs(at.Y - state.Anchor.Y) >= Rest)
                Resume(target, state, e);
        };
        target.PointerExited += (_, _) =>
        {
            state.Timer.Stop();
            if (open is { } o && o.Target == target)
                o.Leaving();
        };
        // A press on the cover opens the title: no preview then.
        target.PointerPressed += (_, _) =>
        {
            state.Timer.Stop();
            Close();
        };
        target.Unloaded += (_, _) => state.Timer.Stop();
    }

    /// <summary>The pointer is at a new resting place: wait out the dwell again.</summary>
    static void Resume(FrameworkElement target, Resting state, PointerRoutedEventArgs e)
    {
        state.Anchor = e.GetCurrentPoint(target).Position;
        state.Timer!.Stop();
        state.Timer.Interval = DateTime.UtcNow - closedAt < Scanning ? NextDwell : Dwell;
        // A touch has no hover; a pen or a mouse does.
        if (e.Pointer.PointerDeviceType == Microsoft.UI.Input.PointerDeviceType.Touch)
            return;
        if (open is { } o && o.Target == target)
        {
            o.Stay();
            return;
        }
        state.Timer.Start();
    }

    /// <summary>Close the preview on screen, if there is one.</summary>
    public static void Close()
    {
        var was = open;
        open = null;
        if (was is not null)
            closedAt = DateTime.UtcNow;
        was?.Dismiss();
    }

    /// <summary>Open a preview of the title now, for a screenshot or a key.</summary>
    public static void Show(FrameworkElement target, uint id)
    {
        var ctx = Ctx.Current;
        if (target.XamlRoot is null || !ctx.Window.InFront || TrailerHub.PageShowing)
            return;
        if (ctx.Known.Entry(id) is not { } entry)
            return;
        Close();
        var preview = new Open(ctx, target, entry);
        open = preview;
        preview.Show();
    }

    /// <summary>The preview on screen.</summary>
    sealed class Open
    {
        readonly Ctx ctx;
        readonly Flyout flyout = new();
        readonly TrailerStage? stage;
        readonly DispatcherQueueTimer leave;
        readonly Button sound = new();
        bool soundOn;

        public FrameworkElement Target { get; }

        public Open(Ctx ctx, FrameworkElement target, Entry entry)
        {
            this.ctx = ctx;
            Target = target;
            // The video is flush at the top of the flyout, clipped only by the flyout's own corners. The
            // art lies over it and holds its place until a frame plays.
            var video = new Grid { Width = Width, Height = Width * 9 / 16 };
            var banner = new Image { Stretch = Stretch.UniformToFill };
            var art = new Grid { Background = (Brush)Application.Current.Resources["RaisedBrush"] };
            art.Children.Add(banner);
            video.Children.Add(art);
            ctx.Art.Bind(entry.BannerUrl ?? entry.CoverUrl, s => banner.Source = s);

            var column = new StackPanel { Width = Width, Spacing = 0 };
            column.Children.Add(video);
            var text = new StackPanel { Spacing = 0, Padding = new Thickness(16, 12, 16, 16) };

            var heading = new Grid { ColumnSpacing = 8, MinHeight = 32 };
            heading.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            heading.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            heading.Children.Add(new TextBlock
            {
                Text = entry.Title,
                Style = (Style)Application.Current.Resources["ShowTitleStyle"],
                MaxLines = 2,
                TextWrapping = TextWrapping.WrapWholeWords,
                TextTrimming = TextTrimming.CharacterEllipsis,
            });
            sound.Content = new FontIcon { Glyph = "\uE74F", FontSize = 14 };
            sound.Width = 32;
            sound.Height = 32;
            sound.Padding = new Thickness(0);
            sound.CornerRadius = new CornerRadius(4);
            sound.Style = (Style)Application.Current.Resources["SubtleButtonStyle"];
            // Hidden until a frame plays, but its place stays, so the card is the same size with or without a trailer.
            sound.Opacity = 0;
            sound.IsHitTestVisible = false;
            ToolTipService.SetToolTip(sound, T.TitleTrailerSound);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(sound, T.TitleTrailerSound);
            Grid.SetColumn(sound, 1);
            heading.Children.Add(sound);
            text.Children.Add(heading);

            if (Wording.MetaLine(entry) is { Length: > 0 } facts)
            {
                var line = Ui.Text(facts, "HazeCaptionStyle");
                line.Margin = new Thickness(0, 4, 0, 0);
                text.Children.Add(line);
            }
            if (Trailers.SpoilerFree(entry) is { Length: > 0 } synopsis)
            {
                var lines = Ui.Text(synopsis, "BodyTextBlockStyle");
                lines.Margin = new Thickness(0, 8, 0, 0);
                lines.MaxLines = 2;
                lines.TextTrimming = TextTrimming.CharacterEllipsis;
                text.Children.Add(lines);
            }
            var queue = PlayableNext.Item(entry, ctx.Known.Episodes.GetValueOrDefault(entry.Id), Casing.Sentence);
            if (queue.Button is { } label)
            {
                var play = new Button
                {
                    Content = label,
                    Style = (Style)Application.Current.Resources["PrimaryActionStyle"],
                    Margin = new Thickness(0, 14, 0, 0),
                    HorizontalAlignment = HorizontalAlignment.Left,
                    IsEnabled = queue.Playable,
                };
                play.Click += (_, _) =>
                {
                    Close();
                    ctx.Play(entry.Id, queue.Episode.ToString());
                };
                text.Children.Add(play);
            }
            column.Children.Add(text);

            var presenter = new Style(typeof(FlyoutPresenter));
            presenter.Setters.Add(new Setter(Control.PaddingProperty, new Thickness(0)));
            presenter.Setters.Add(new Setter(FrameworkElement.MaxWidthProperty, Width + 8d));
            presenter.Setters.Add(new Setter(Control.CornerRadiusProperty, new CornerRadius(8)));
            // The canvas's Raised with the system's flyout hairline.
            presenter.Setters.Add(new Setter(Control.BackgroundProperty, Application.Current.Resources["RaisedBrush"]));
            presenter.Setters.Add(new Setter(Control.BorderBrushProperty, Application.Current.Resources["SurfaceStrokeColorFlyoutBrush"]));
            presenter.Setters.Add(new Setter(Control.BorderThicknessProperty, new Thickness(1)));
            flyout.FlyoutPresenterStyle = presenter;
            flyout.Content = column;
            flyout.Placement = FlyoutPlacementMode.RightEdgeAlignedTop;
            flyout.ShowMode = FlyoutShowMode.Transient;
            flyout.LightDismissOverlayMode = LightDismissOverlayMode.Off;
            flyout.Closed += (_, _) =>
            {
                stage?.Stop();
                if (open?.flyout == flyout)
                {
                    open = null;
                    closedAt = DateTime.UtcNow;
                }
            };

            column.PointerEntered += (_, _) => Stay();
            column.PointerExited += (_, _) => Leaving();
            leave = target.DispatcherQueue.CreateTimer();
            leave.Interval = Leave;
            leave.IsRepeating = false;
            leave.Tick += (_, _) => Close();

            if (TrailerHub.Available && Trailers.Embeddable(entry))
            {
                stage = new TrailerStage(video, EmbedStyle.Banner, art);
                stage.Size(video.Width, video.Height);
                stage.Showing += shown =>
                {
                    soundOn = false;
                    sound.IsHitTestVisible = shown;
                    if (shown)
                    {
                        (sound.Content as FontIcon)!.Glyph = "\uE74F";
                        Motion.Fade(sound, 1);
                    }
                    else
                        Motion.Fade(sound, 0);
                };
                sound.Click += (_, _) =>
                {
                    soundOn = !soundOn;
                    stage.SetSound(soundOn);
                    (sound.Content as FontIcon)!.Glyph = soundOn ? "\uE767" : "\uE74F";
                };
            }
            entryToPlay = entry;
        }

        readonly Entry entryToPlay;

        UIElement? root;
        readonly KeyEventHandler onKey = (_, _) => Close();
        readonly PointerEventHandler onWheel = (_, _) => Close();

        public void Show()
        {
            // A key press or a scroll closes it at once.
            root = Target.XamlRoot?.Content;
            root?.AddHandler(UIElement.KeyDownEvent, onKey, true);
            root?.AddHandler(UIElement.PointerWheelChangedEvent, onWheel, true);
            flyout.ShowAt(Target, new FlyoutShowOptions { Placement = FlyoutPlacementMode.RightEdgeAlignedTop, ShowMode = FlyoutShowMode.Transient });
            // Banner first; the trailer loads under it and fades in only when a frame plays.
            if (stage is not null && Motion.Enabled && Trailers.MayAutoplay(ctx.Known, entryToPlay))
                stage.Start(entryToPlay);
        }

        /// <summary>The pointer is on the cover or the preview: stay open.</summary>
        public void Stay()
        {
            leave.Stop();
        }

        /// <summary>The pointer left the cover or the preview: close in 150 ms unless it comes back to either.</summary>
        public void Leaving()
        {
            leave.Stop();
            leave.Start();
        }

        public void Dismiss()
        {
            leave.Stop();
            root?.RemoveHandler(UIElement.KeyDownEvent, onKey);
            root?.RemoveHandler(UIElement.PointerWheelChangedEvent, onWheel);
            stage?.Stop();
            flyout.Hide();
        }
    }
}
