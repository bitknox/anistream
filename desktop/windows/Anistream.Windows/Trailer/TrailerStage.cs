using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;

namespace Anistream.Windows;

/// <summary>
/// A trailer's web view in one place (the title page's banner, a hover preview, the Play trailer
/// window). The banner shows at once and the web view lies over it, transparent and unseen; the
/// video fades in only when the page says a frame is playing, so the wait is never seen. Nothing
/// is ever drawn over the video, and the view takes no pointer or keyboard input of its own.
/// </summary>
sealed class TrailerStage
{
    /// <summary>YouTube's smallest player: 200 by 200 px.</summary>
    public const double MinSide = 200;

    static readonly TimeSpan GiveUp = TimeSpan.FromSeconds(8);

    readonly Panel host;
    readonly EmbedStyle style;
    readonly TrailerFlow flow = new();
    readonly Func<Entry, string?> pageFor;
    WebView2? view;
    DispatcherQueueTimer? timeout;
    string? folder;
    int generation;
    double width, height;

    /// <summary>The video is on screen (true) or has gone (false).</summary>
    public event Action<bool>? Showing;

    /// <summary>The page refused it or it could not start, with YouTube's code (0 for the page's own).</summary>
    public event Action<uint>? Refused;

    public TrailerStage(Panel host, EmbedStyle style, Func<Entry, string?>? pageFor = null)
    {
        this.host = host;
        this.style = style;
        this.pageFor = pageFor ?? (entry => Trailers.Page(entry, style));
    }

    public TrailerFlow Flow => flow;
    public bool Active => flow.Active;
    public bool Playing => flow.Showing;

    /// <summary>
    /// The size of the area the video may fill. The player letterboxes inside it and crops
    /// nothing; the view is the largest 16:9 that fits, centred.
    /// </summary>
    public void Size(double w, double h)
    {
        (width, height) = (w, h);
        Fit();
    }

    void Fit()
    {
        if (view is null)
            return;
        if (style == EmbedStyle.Full)
        {
            // The window's own: the view fills it, and YouTube's player letterboxes inside.
            view.Width = view.Height = double.NaN;
            view.HorizontalAlignment = HorizontalAlignment.Stretch;
            view.VerticalAlignment = VerticalAlignment.Stretch;
            return;
        }
        view.Height = height;
        view.Width = Math.Min(width, height * 16 / 9);
        view.HorizontalAlignment = HorizontalAlignment.Center;
        view.VerticalAlignment = VerticalAlignment.Top;
    }

    /// <summary>This stage's area would give the player at least 200 by 200.</summary>
    public bool Fits(double w, double h) => flow.Phase != TrailerPhase.Done && Math.Min(w, h * 16 / 9) >= 356 && h >= MinSide;

    /// <summary>Load the title's trailer. False when it cannot: no runtime, nothing the player plays, or one already held.</summary>
    public bool Start(Entry entry)
    {
        if (!TrailerHub.Available || view is not null || !flow.CanStart)
            return false;
        var page = pageFor(entry);
        if (page is null)
            return false;
        flow.Start();
        var mine = ++generation;
        // The view takes no input of its own: a pointer over it does nothing on the viewer's behalf.
        view = new WebView2
        {
            DefaultBackgroundColor = Colors.Transparent,
            IsHitTestVisible = style == EmbedStyle.Full,
            IsTabStop = style == EmbedStyle.Full,
            Opacity = style == EmbedStyle.Full ? 1 : 0,
        };
        AutomationProperties.SetAccessibilityView(view, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        Fit();
        host.Children.Add(view);
        timeout = host.DispatcherQueue.CreateTimer();
        timeout.Interval = GiveUp;
        timeout.IsRepeating = false;
        timeout.Tick += (_, _) => Apply(flow.GiveUp());
        if (style == EmbedStyle.Banner)
            timeout.Start();
        if (style == EmbedStyle.Banner)
            TrailerHub.Claim(this);
        _ = Load(view, page, mine);
        return true;
    }

    async Task Load(WebView2 mine, string page, int token)
    {
        try
        {
            var env = await TrailerHub.Environment();
            await mine.EnsureCoreWebView2Async(env, await TrailerHub.Private());
            if (token != generation || view != mine)
                return;
            var core = mine.CoreWebView2;
            var settings = core.Settings;
            settings.AreDefaultContextMenusEnabled = false;
            settings.AreDevToolsEnabled = false;
            settings.AreBrowserAcceleratorKeysEnabled = false;
            settings.IsStatusBarEnabled = false;
            settings.IsZoomControlEnabled = false;
            settings.IsSwipeNavigationEnabled = false;
            settings.IsPasswordAutosaveEnabled = false;
            settings.IsGeneralAutofillEnabled = false;
            core.NewWindowRequested += (_, e) => e.Handled = true;
            core.WebMessageReceived += (_, e) => Heard(token, e.TryGetWebMessageAsString());
            // The page is served from the app's own origin, so YouTube sees it as the Referer.
            folder = Path.Combine(Path.GetTempPath(), "anistream-windows", "trailers", Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(folder);
            await File.WriteAllTextAsync(Path.Combine(folder, "trailer.html"), page);
            core.SetVirtualHostNameToFolderMapping(Trailers.Host, folder, CoreWebView2HostResourceAccessKind.Allow);
            mine.CoreWebView2.Navigate($"{Trailers.Origin}/trailer.html");
        }
        catch (Exception)
        {
            if (token == generation)
                Fail();
        }
    }

    void Heard(int token, string? text)
    {
        if (token != generation)
            return;
        var message = text is null ? null : AnistreamFfiMethods.TrailerMessage(text);
        if (message is TrailerMessage.Error error)
            Refused?.Invoke(error.Code);
        Apply(flow.Heard(message));
    }

    void Fail()
    {
        Refused?.Invoke(0);
        Apply(flow.Failed());
    }

    void Apply(TrailerEffect effect)
    {
        if (effect.HasFlag(TrailerEffect.PausePlayer))
            Control(TrailerControl.Pause);
        if (effect.HasFlag(TrailerEffect.PlayPlayer))
            Control(TrailerControl.Play);
        if (effect.HasFlag(TrailerEffect.ShowVideo) && view is not null)
        {
            timeout?.Stop();
            if (style == EmbedStyle.Banner)
                Motion.Video(view, true);
            Showing?.Invoke(true);
        }
        if (effect.HasFlag(TrailerEffect.HideVideo))
        {
            if (!effect.HasFlag(TrailerEffect.Release) && view is not null)
                Motion.Video(view, false);
            Showing?.Invoke(false);
        }
        if (effect.HasFlag(TrailerEffect.Release))
            Let(fade: effect.HasFlag(TrailerEffect.HideVideo) && style == EmbedStyle.Banner);
    }

    /// <summary>Hold the trailer: it pauses and the banner comes back over it.</summary>
    public void Pause() => Apply(flow.Pause());

    /// <summary>Play it again from where it paused; the video shows when the page says it is playing.</summary>
    public void Resume()
    {
        Apply(flow.Resume());
        timeout?.Stop();
    }

    /// <summary>Sound on or off, for this trailer only.</summary>
    public void SetSound(bool on) => Control(on ? TrailerControl.Unmute : TrailerControl.Mute);

    /// <summary>Let go of the view and bring the banner back.</summary>
    public void Stop() => Apply(flow.Stop());

    void Control(TrailerControl control)
    {
        if (view?.CoreWebView2 is { } core)
        {
            if (control is TrailerControl.Mute or TrailerControl.Unmute)
                core.IsMuted = control == TrailerControl.Mute;
            _ = core.ExecuteScriptAsync(AnistreamFfiMethods.TrailerScript(control));
        }
    }

    /// <summary>The view goes, with the page's folder: after the video has faded away when it was showing.</summary>
    void Let(bool fade)
    {
        generation++;
        timeout?.Stop();
        timeout = null;
        TrailerHub.Release(this);
        var gone = view;
        view = null;
        void Close()
        {
            host.Children.Remove(gone);
            try
            {
                gone.Close();
            }
            catch (Exception)
            {
                // Already closed with its window.
            }
        }
        if (gone is not null)
        {
            if (fade)
            {
                _ = gone.CoreWebView2?.ExecuteScriptAsync(AnistreamFfiMethods.TrailerScript(TrailerControl.Pause));
                Motion.Video(gone, false, Close);
            }
            else
                Close();
        }
        if (folder is not null)
        {
            var dir = folder;
            folder = null;
            _ = Task.Run(() =>
            {
                try { Directory.Delete(dir, true); } catch (IOException) { }
            });
        }
    }
}
