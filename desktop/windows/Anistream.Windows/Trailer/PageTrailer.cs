using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// A title page's trailer: it plays over the banner once the page has settled, and the video
/// fades in when a frame is playing. Nothing is drawn over the video, so the title steps down
/// below it. Scrolling the page by more than 8 px pauses it; scrolling back to the top within
/// 30 s plays it again, and after 30 s away it is let go of for this visit. Leaving the page,
/// starting an episode, opening a sheet or the window losing focus releases it at once.
/// </summary>
sealed class PageTrailer
{
    static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(900);
    static readonly TimeSpan Away = TimeSpan.FromSeconds(30);
    /// <summary>Scrolling further than this pauses it.</summary>
    const double Scrolled = 8;

    readonly Ctx ctx = Ctx.Current;
    readonly TitlePage page;
    readonly Grid banner;
    readonly ScrollViewer scroll;
    readonly Button sound;
    bool soundOn;
    readonly TrailerStage stage;
    readonly DispatcherQueueTimer settle, away, watch;
    uint id;
    bool settled, abandoned, released;

    public PageTrailer(TitlePage page, Grid banner, ScrollViewer scroll, Button sound)
    {
        this.page = page;
        this.banner = banner;
        this.scroll = scroll;
        this.sound = sound;
        stage = new TrailerStage(banner, EmbedStyle.Banner);
        stage.Showing += Showing;
        settle = page.DispatcherQueue.CreateTimer();
        settle.Interval = Settle;
        settle.IsRepeating = false;
        settle.Tick += (_, _) =>
        {
            settled = true;
            MaybeStart();
        };
        away = page.DispatcherQueue.CreateTimer();
        away.Interval = Away;
        away.IsRepeating = false;
        away.Tick += (_, _) =>
        {
            // Not again on this visit.
            abandoned = true;
            stage.Stop();
        };
        // A sheet over the page lets the trailer go; the window's own popups are told apart by what they hold.
        watch = page.DispatcherQueue.CreateTimer();
        watch.Interval = TimeSpan.FromMilliseconds(400);
        watch.Tick += (_, _) =>
        {
            if (SheetOpen())
                stage.Stop();
        };
        sound.Click += (_, _) =>
        {
            Sound(!soundOn);
        };
        banner.SizeChanged += (_, e) => stage.Size(e.NewSize.Width, e.NewSize.Height);
        scroll.ViewChanged += OnScroll;
        ctx.Window.Activated += OnActivated;
    }

    public bool Playing => stage.Playing;

    /// <summary>The page is shown for title <paramref name="title"/>: wait for its transition to end, then see whether the trailer may play.</summary>
    public void Open(uint title)
    {
        id = title;
        settled = false;
        settle.Start();
    }

    /// <summary>Everything the page holds is let go of: the trailer, its timers and its hold on the window.</summary>
    public void Release()
    {
        released = true;
        settle.Stop();
        away.Stop();
        watch.Stop();
        scroll.ViewChanged -= OnScroll;
        ctx.Window.Activated -= OnActivated;
        stage.Stop();
        TrailerHub.PageShowing = false;
    }

    void OnActivated(object sender, WindowActivatedEventArgs e)
    {
        if (released)
            return;
        if (e.WindowActivationState == WindowActivationState.Deactivated)
        {
            stage.Stop();
            settle.Stop();
        }
        else if (settled)
        {
            // Back in front: settle again before it plays.
            settle.Start();
        }
    }

    bool SheetOpen() =>
        page.XamlRoot is { } root && VisualTreeHelper.GetOpenPopupsForXamlRoot(root).Any(p => p.Child is ContentDialog);

    /// <summary>Start the trailer if everything allows it now. Called as the title's facts arrive, as the engine's word on autoplay changes, and when the page settles.</summary>
    public void MaybeStart()
    {
        if (released || !settled || abandoned || stage.Active || scroll.VerticalOffset > Scrolled
            || !page.IsLoaded || ctx.Window.PlayerUp || !ctx.Window.InFront || SheetOpen())
            return;
        if (ctx.Known.Entry(id) is not { } entry || !Trailers.MayAutoplay(ctx.Known, entry))
            return;
        var (w, h) = (banner.ActualWidth, banner.ActualHeight);
        if (!stage.Fits(w, h))
            return;
        stage.Size(w, h);
        if (stage.Start(entry))
            watch.Start();
    }

    void OnScroll(object? sender, ScrollViewerViewChangedEventArgs e)
    {
        if (scroll.VerticalOffset > Scrolled)
        {
            if (stage.Playing)
            {
                stage.Pause();
                away.Start();
            }
        }
        else if (away.IsRunning)
        {
            away.Stop();
            stage.Resume();
        }
    }

    /// <summary>The video fades in or away: the sound button comes and goes, and the title steps below the video or back over the banner.</summary>
    void Showing(bool shown)
    {
        TrailerHub.PageShowing = shown;
        if (shown)
        {
            HoverPreview.Close();
            Sound(false);
            sound.Visibility = Visibility.Visible;
            Motion.FadeIn(sound);
        }
        else
            sound.Visibility = Visibility.Collapsed;
        // Gone for good, the title goes back over the banner; paused, it stays down.
        if (shown || !stage.Active)
            page.StepTitle(shown);
        if (!stage.Active)
            watch.Stop();
    }

    void Sound(bool on)
    {
        soundOn = on;
        stage.SetSound(on);
        if (sound.Content is FontIcon icon)
            icon.Glyph = on ? "\uE767" : "\uE74F";
    }
}
