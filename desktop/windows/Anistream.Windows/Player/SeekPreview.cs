using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Player;

/// <summary>
/// The frame under the pointer on the timeline, from a second mpv that is never heard: video
/// only, paused, no audio or subtitles, opened on the first hover of a file and kept for the
/// next ones, closed with the file. Seeks are to keyframes, and the newest waits its turn while
/// mpv works on the last. The other apps do the same (Apple's <c>SeekPreview</c>).
/// </summary>
sealed class SeekPreview : IDisposable
{
    /// <summary>The preview's own playback number: the engine knows no such playback and drops its reports.</summary>
    const ulong Playback = 0x7FFF_0000_0001;

    sealed class Quiet : FfiPlayerSink
    {
        public void Event(FfiPlayerEvent e)
        {
        }
    }

    readonly FfiEngine engine;
    readonly SwapChainPanel panel;
    readonly DispatcherQueueTimer pump;
    FfiPlayer? player;
    PreviewSource? source;
    nint bound;
    double? target;
    bool opened;
    DateTime lastSeek;

    public SeekPreview(FfiEngine engine, SwapChainPanel panel, DispatcherQueue queue)
    {
        this.engine = engine;
        this.panel = panel;
        pump = queue.CreateTimer();
        pump.Interval = TimeSpan.FromMilliseconds(80);
        pump.Tick += (_, _) => Pump();
    }

    /// <summary>Whether a player is up for previews at all.</summary>
    public bool Alive => player is not null;

    /// <summary>Show the frame at <paramref name="seconds"/> of <paramref name="from"/>.</summary>
    public void Show(double seconds, PreviewSource from, double scale)
    {
        if (player is null || source != from)
            Open(from, seconds, scale);
        target = seconds;
        if (!pump.IsRunning)
            pump.Start();
    }

    void Open(PreviewSource from, double seconds, double scale)
    {
        Close();
        try
        {
            player = new FfiPlayer(new FfiPlayerOptions(
                Video: new FfiVideo.Composition((uint)(160 * scale), (uint)(90 * scale)),
                AudioOutput: "null",
                LogLevel: "error",
                Extra:
                [
                    new MpvOption("aid", "no"), new MpvOption("sid", "no"), new MpvOption("hr-seek", "no"),
                    new MpvOption("osd-level", "0"), new MpvOption("keep-open", "yes"), new MpvOption("pause", "yes"),
                ]), engine, new Quiet());
            player.CompositionSize((uint)(160 * scale), (uint)(90 * scale));
            player.Display((uint)(160 * scale), (uint)(90 * scale), scale, null);
            source = from;
            MpvOption[] options = [.. from.Options, new MpvOption("start", seconds.ToString(System.Globalization.CultureInfo.InvariantCulture))];
            player.Apply(Playback, [new PlayerOp.Open(false, null, null), new PlayerOp.Load(from.Url, options)]);
            opened = true;
            lastSeek = DateTime.UtcNow;
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the seek preview did not open: {e.Message}");
            Close();
        }
    }

    void Pump()
    {
        if (player is null)
        {
            pump.Stop();
            return;
        }
        Bind();
        if (target is { } seconds && opened && DateTime.UtcNow - lastSeek > TimeSpan.FromMilliseconds(120))
        {
            target = null;
            lastSeek = DateTime.UtcNow;
            try
            {
                player.Apply(Playback, [new PlayerOp.Seek(seconds, true)]);
            }
            catch (Exception)
            {
                // Closed under it: the next hover opens again.
            }
        }
    }

    /// <summary>Hand mpv's swapchain to the panel once it has made one.</summary>
    void Bind()
    {
        var swapchain = (nint)(player?.Swapchain() ?? 0);
        if (swapchain == 0 || swapchain == bound)
            return;
        try
        {
            Dxgi.SetSwapChain(panel, swapchain);
            bound = swapchain;
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"binding the preview's swapchain failed: {e.Message}");
        }
    }

    /// <summary>Done with this file, or with previews: that mpv shuts down.</summary>
    public void Close()
    {
        pump.Stop();
        target = null;
        opened = false;
        bound = 0;
        source = null;
        if (player is { } old)
        {
            player = null;
            try
            {
                old.Shutdown();
                old.Dispose();
            }
            catch (Exception)
            {
                // Already gone.
            }
        }
    }

    public void Dispose() => Close();
}
