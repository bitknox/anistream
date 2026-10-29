using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows;

/// <summary>
/// Play trailer: the trailer in a window of its own, with YouTube's controls and sound on. The
/// client area is 16:9 at 960 by 540 and keeps that shape when resized, under a dark title bar.
/// The title's art fills it until the first frame, with a progress ring after 600 ms; after 8 s
/// without a frame, or on a player error, it says "Couldn't open the trailer" with Try again.
/// When the embed is refused (embedding disabled by its owner, a region lock), or the web view
/// is not there, the window closes and the watch page opens in the browser, which tells so.
/// </summary>
static class TrailerWindow
{
    static Window? current;

    /// <summary>YouTube's codes for a video its owner or region does not let an embed play.</summary>
    static bool Refusal(uint code) => code is 101 or 150 or 153;

    public static void Play(Ctx ctx, Entry entry)
    {
        if (entry.TrailerUrl is not { } url)
            return;
        void Browser() => ctx.Send(new Command.PlayTrailer(url, entry.Title));
        if (!TrailerHub.Available || !Trailers.Embeddable(entry))
        {
            Browser();
            return;
        }
        // Whatever plays by itself gives way to a trailer the viewer asked for.
        TrailerHub.StopAll();
        current?.Close();

        var surface = new Grid { Background = new SolidColorBrush(Colours.Ground) };
        var image = new Image { Stretch = Stretch.UniformToFill };
        var art = new Grid { Background = new SolidColorBrush(Colours.Raised) };
        art.Children.Add(image);
        ctx.Art.Bind(entry.BannerUrl ?? entry.CoverUrl, s => image.Source = s);
        surface.Children.Add(art);

        var ring = new ProgressRing { Width = 32, Height = 32, IsActive = false, Visibility = Visibility.Collapsed, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        var failure = new StackPanel { Spacing = 14, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center, Visibility = Visibility.Collapsed };
        failure.Children.Add(new TextBlock { Text = T.TitleTrailerFailed, Style = (Style)Application.Current.Resources["SectionTitleStyle"], HorizontalAlignment = HorizontalAlignment.Center });
        var retry = new Button { Content = T.ActionTryAgain, HorizontalAlignment = HorizontalAlignment.Center, Height = 40 };
        failure.Children.Add(retry);
        surface.Children.Add(ring);
        surface.Children.Add(failure);

        var window = new Window { Title = $"{entry.Title} · {T.TitleTrailer}", Content = surface };
        Dark(window);
        var stage = new TrailerStage(surface, EmbedStyle.Full, art);
        var refused = false;
        // No spinner for the first 600 ms, then the system's, until the first frame.
        var spin = surface.DispatcherQueue.CreateTimer();
        spin.Interval = TimeSpan.FromMilliseconds(600);
        spin.IsRepeating = false;
        spin.Tick += (_, _) =>
        {
            ring.Visibility = Visibility.Visible;
            ring.IsActive = true;
        };
        void Waiting(bool on)
        {
            spin.Stop();
            ring.IsActive = false;
            ring.Visibility = Visibility.Collapsed;
            if (on)
            {
                failure.Visibility = Visibility.Collapsed;
                spin.Start();
            }
        }
        void Failed()
        {
            Waiting(false);
            failure.Visibility = Visibility.Visible;
            retry.Focus(FocusState.Programmatic);
        }
        void Try()
        {
            Waiting(true);
            stage.Start(entry);
        }
        retry.Click += (_, _) => Try();
        stage.TimedOut += Failed;
        stage.Showing += shown =>
        {
            if (shown)
                Waiting(false);
        };
        stage.Refused += code =>
        {
            if (refused)
                return;
            if (!Refusal(code))
            {
                Failed();
                return;
            }
            refused = true;
            window.Close();
            Browser();
        };
        window.Closed += (_, _) =>
        {
            spin.Stop();
            stage.Stop();
            if (current == window)
                current = null;
        };
        var escape = new KeyboardAccelerator { Key = global::Windows.System.VirtualKey.Escape };
        escape.Invoked += (_, a) =>
        {
            a.Handled = true;
            window.Close();
        };
        surface.KeyboardAccelerators.Add(escape);

        KeepShape(window);
        current = window;
        window.Activate();
        Try();
    }

    /// <summary>The title bar in the app's dark colours rather than the system's light one.</summary>
    static void Dark(Window window)
    {
        var bar = window.AppWindow.TitleBar;
        bar.BackgroundColor = bar.InactiveBackgroundColor = Colours.Ground;
        bar.ForegroundColor = Colours.Silk;
        bar.InactiveForegroundColor = Colours.Haze;
        bar.ButtonBackgroundColor = bar.ButtonInactiveBackgroundColor = Colours.Ground;
        bar.ButtonForegroundColor = Colours.Silk;
        bar.ButtonInactiveForegroundColor = Colours.Haze;
        bar.ButtonHoverBackgroundColor = global::Windows.UI.Color.FromArgb(0xFF, 0x2F, 0x32, 0x42);
        bar.ButtonHoverForegroundColor = Colours.Silk;
        bar.ButtonPressedBackgroundColor = Colours.Raised;
        bar.ButtonPressedForegroundColor = Colours.Silk;
        window.AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "anistream.ico"));
    }

    /// <summary>The client area opens at 960 by 540 and stays 16:9 as the window is resized, like the player window.</summary>
    static void KeepShape(Window window)
    {
        var app = window.AppWindow;
        app.ResizeClient(new global::Windows.Graphics.SizeInt32(960, 540));
        var fixing = false;
        app.Changed += (_, change) =>
        {
            if (!change.DidSizeChange || fixing)
                return;
            var client = app.ClientSize;
            var height = (int)Math.Round(client.Width * 9d / 16);
            if (Math.Abs(height - client.Height) < 2)
                return;
            fixing = true;
            try
            {
                app.ResizeClient(new global::Windows.Graphics.SizeInt32(client.Width, height));
            }
            finally
            {
                fixing = false;
            }
        };
    }
}
