using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// A title page's trailer: a title that may play one opens with the band tall enough for it,
/// decided once per visit, so nothing on the page moves. It plays in its own 448 by 252 place at
/// the trailing end of the band once the page has settled; the art fades away entirely when a
/// frame is playing and comes back half a second before the end. Scrolling the page by more than
/// 8 px fades the art back and pauses the trailer under it; scrolling back to the top within 30 s
/// plays it again and fades the art away once it plays, and after 30 s away it is let go of for
/// this visit. Leaving the page, starting an episode, opening a sheet or the window losing focus
/// releases it at once.
/// </summary>
sealed class PageTrailer
{
    static readonly TimeSpan Settle = TimeSpan.FromMilliseconds(900);
    static readonly TimeSpan Away = TimeSpan.FromSeconds(30);
    /// <summary>Scrolling further than this pauses it.</summary>
    const double Scrolled = 8;

    /// <summary>The video's place in the band: 448 by 252, 12 below its top and 36 from its trailing edge.</summary>
    public const double VideoWidth = 448, VideoHeight = 252, VideoTop = 12, VideoMargin = 36;

    /// <summary>The page is wide enough for the video beside the cover and title.</summary>
    const double WideEnough = 760;

    readonly Ctx ctx = Ctx.Current;
    readonly TitlePage page;
    readonly Grid banner;
    readonly ScrollViewer scroll;
    readonly Button sound;
    readonly Func<bool> reserved;
    bool soundOn;
    readonly TrailerStage stage;
    readonly DispatcherQueueTimer settle, away, watch;
    uint id;
    bool settled, abandoned, released;

    public PageTrailer(TitlePage page, Grid banner, UIElement art, ScrollViewer scroll, Button sound, Func<bool> reserved)
    {
        this.page = page;
        this.banner = banner;
        this.scroll = scroll;
        this.sound = sound;
        this.reserved = reserved;
        stage = new TrailerStage(banner, EmbedStyle.Banner, art);
        stage.Place(VideoWidth, VideoHeight, new Thickness(0, VideoTop, VideoMargin, 0), HorizontalAlignment.Right);
        stage.Showing += Showing;
        settle = Timers.Weak(this, Settle, false, static trailer =>
        {
            trailer.settled = true;
            trailer.MaybeStart();
        });
        // Not again on this visit.
        away = Timers.Weak(this, Away, false, static trailer =>
        {
            trailer.abandoned = true;
            trailer.stage.Stop();
        });
        // A sheet over the page lets the trailer go; the window's own popups are told apart by what they hold.
        watch = Timers.Weak(this, TimeSpan.FromMilliseconds(400), true, static trailer =>
        {
            if (trailer.SheetOpen())
                trailer.stage.Stop();
        });
        sound.Click += (_, _) =>
        {
            Sound(!soundOn);
        };
        scroll.ViewChanged += OnScroll;
        ctx.Window.Activated += OnActivated;
    }

    public bool Playing => stage.Playing;

    /// <summary>
    /// The title may play a trailer here: the engine allows it on this device and network, the
    /// title's trailer is one the app plays and the spoiler rule lets it, animation effects are
    /// on, the web view is there and the page is wide enough. The page asks once per visit and
    /// opens with the band tall enough when the answer is yes.
    /// </summary>
    public bool Wanted(Entry entry) =>
        TrailerHub.Available && Motion.Enabled && page.ActualWidth >= WideEnough && Trailers.MayAutoplay(ctx.Known, entry);

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
            || !reserved() || !Motion.Enabled || !page.IsLoaded || ctx.Window.PlayerUp || !ctx.Window.InFront || SheetOpen())
            return;
        if (ctx.Known.Entry(id) is not { } entry || !Trailers.MayAutoplay(ctx.Known, entry))
            return;
        if (!stage.Fits(banner.ActualWidth, banner.ActualHeight))
            return;
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

    /// <summary>The video shows or goes: the sound button fades in or away with the art, and the page does not move.</summary>
    void Showing(bool shown)
    {
        TrailerHub.PageShowing = shown;
        if (shown)
        {
            HoverPreview.Close();
            Sound(false);
            Motion.FadeIn(sound);
        }
        else
            Motion.Hide(sound);
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
