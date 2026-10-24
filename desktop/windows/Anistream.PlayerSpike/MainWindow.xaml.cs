using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using Anistream.Engine.Ffi;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.PlayerSpike;

/// <summary>
/// One mpv core in d3d11 composition mode. mpv makes a swapchain for composition (no window),
/// the app reads its address from <c>display-swapchain</c> and hands it to the SwapChainPanel,
/// and tells mpv the panel's size in pixels through <c>d3d11-composition-size</c>. The bars are
/// ordinary XAML over it.
/// </summary>
public sealed partial class MainWindow : Window
{
    private readonly SpikeOptions _options;
    private readonly DispatcherQueueTimer _timer;
    private readonly string _reportPath;
    private readonly List<string> _events = [];
    private nint _mpv;
    private Thread? _eventThread;
    private volatile bool _closing;
    private nint _boundSwapchain;
    private string _output;
    private bool _timelineFromMpv;
    private bool _paused, _pausedOnce, _switched, _resized, _quit;
    private double _position, _duration;
    private string _engine = "not loaded";
    private string _lastReport = "";

    internal MainWindow(SpikeOptions options)
    {
        _options = options;
        _output = options.Output;
        InitializeComponent();
        // The brief's default window, or as much of it as the display's work area holds.
        var area = Microsoft.UI.Windowing.DisplayArea.GetFromWindowId(AppWindow.Id, Microsoft.UI.Windowing.DisplayAreaFallback.Primary).WorkArea;
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(area.X, area.Y, Math.Min(1280, area.Width), Math.Min(800, area.Height)));
        HdrToggle.IsChecked = _output == "hdr";

        var dir = Path.GetDirectoryName(options.Report ?? "") is { Length: > 0 } d
            ? d
            : Path.Combine(Path.GetTempPath(), "anistream-player-spike");
        Directory.CreateDirectory(dir);
        _reportPath = options.Report ?? Path.Combine(dir, "report.txt");

        // The engine's library in the WinUI process: its log, through the generated bindings.
        try
        {
            var logs = Path.Combine(dir, "engine");
            Directory.CreateDirectory(logs);
            AnistreamFfiMethods.StartLog(logs);
            AnistreamFfiMethods.LogLine(FfiLogLevel.Info, "player-spike", "the WinUI spike started");
            _engine = "anistream_ffi.dll loaded, log started";
        }
        catch (Exception e)
        {
            _engine = $"failed: {e.GetType().Name}: {e.Message}";
        }

        _timer = DispatcherQueue.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(250);
        _timer.Tick += (_, _) => Tick();
        Closed += (_, _) => Shutdown();
    }

    private (int Width, int Height) PanelPixels()
    {
        var scale = VideoPanel.XamlRoot?.RasterizationScale ?? 1.0;
        return ((int)Math.Max(1, Math.Round(VideoPanel.ActualWidth * scale)),
                (int)Math.Max(1, Math.Round(VideoPanel.ActualHeight * scale)));
    }

    private void OnPanelSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (e.NewSize.Width < 1 || e.NewSize.Height < 1)
            return;
        var (w, h) = PanelPixels();
        if (_mpv == 0)
        {
            try
            {
                Start(w, h);
            }
            catch (Exception ex)
            {
                Note($"start failed: {ex.Message}");
                Info.Text = ex.ToString();
                File.WriteAllText(_reportPath, ex.ToString());
            }
            return;
        }
        // Composition has no window for mpv to measure: the app says the size, and mpv resizes
        // its swapchain's buffers (VOCTRL_EXTERNAL_RESIZE).
        Mpv.SetPropertyString(_mpv, "d3d11-composition-size", $"{w}x{h}");
        Note($"resized to {w}x{h}");
    }

    private void Start(int width, int height)
    {
        _mpv = Mpv.Create();
        if (_mpv == 0)
            throw new InvalidOperationException("mpv_create failed");
        void Set(string name, string value) => Mpv.Check(Mpv.SetOptionString(_mpv, name, value), $"{name}={value}");

        Set("vo", _options.Vo);
        Set("gpu-api", "d3d11");
        Set("gpu-context", "d3d11");
        Set("d3d11-output-mode", "composition");
        Set("d3d11-composition-size", $"{width}x{height}");
        Set("hwdec", "auto-safe");
        Set("keep-open", "yes");
        Set("loop-file", "inf");
        Set("idle", "yes");
        Set("osc", "no");
        Set("input-default-bindings", "no");
        Set("log-file", Path.Combine(Path.GetDirectoryName(_reportPath)!, "mpv.log"));
        Set("msg-level", "all=v");
        ApplyOutput(Set);
        if (_options.Peak is { } peak)
            Set("target-peak", peak.ToString(CultureInfo.InvariantCulture));
        foreach (var (name, value) in _options.Set)
            Set(name, value);

        Mpv.Check(Mpv.Initialize(_mpv), "initialize");
        Mpv.ObserveProperty(_mpv, 1, "time-pos", Mpv.FormatDouble);
        Mpv.ObserveProperty(_mpv, 2, "duration", Mpv.FormatDouble);
        Mpv.ObserveProperty(_mpv, 3, "pause", Mpv.FormatFlag);

        _eventThread = new Thread(EventLoop) { IsBackground = true, Name = "mpv events" };
        _eventThread.Start();

        if (_options.File is { } file)
            Mpv.Check(Mpv.CommandString(_mpv, $"loadfile \"{file.Replace("\\", "/")}\""), "loadfile");
        Note($"mpv {Mpv.GetString(_mpv, "mpv-version")}, client API {Mpv.ClientApiVersion() >> 16}.{Mpv.ClientApiVersion() & 0xffff}");
        _timer.Start();
    }

    /// <summary>
    /// The swapchain's format and colour space, and whether libplacebo may follow the content:
    /// <c>sdr</c> is mpv's defaults; <c>pq</c> and <c>scrgb</c> set only mpv's d3d11 options;
    /// <c>hdr</c> adds <c>target-colorspace-hint</c>, which is what makes gpu-next render HDR10.
    /// </summary>
    private void ApplyOutput(Action<string, string> set)
    {
        var (format, csp, hint) = _output switch
        {
            "pq" => ("rgb10_a2", "pq", "no"),
            "hdr" => ("rgb10_a2", "pq", "yes"),
            "scrgb" => ("rgba16f", "linear", "no"),
            _ => ("auto", "auto", "no"),
        };
        set("d3d11-output-format", format);
        set("d3d11-output-csp", csp);
        if (!_options.Set.Any(o => o.Name == "target-colorspace-hint"))
            set("target-colorspace-hint", hint);
    }

    private void EventLoop()
    {
        while (!_closing)
        {
            var ev = Marshal.PtrToStructure<Mpv.Event>(Mpv.WaitEvent(_mpv, -1));
            switch (ev.EventId)
            {
                case Mpv.EventShutdown:
                    return;
                case Mpv.EventPropertyChange:
                {
                    var prop = Marshal.PtrToStructure<Mpv.EventProperty>(ev.Data);
                    if (prop.Data == 0)
                        break;
                    var id = ev.ReplyUserdata;
                    double number = prop.Format == Mpv.FormatDouble ? Marshal.PtrToStructure<double>(prop.Data) : 0;
                    bool flag = prop.Format == Mpv.FormatFlag && Marshal.ReadInt32(prop.Data) != 0;
                    DispatcherQueue.TryEnqueue(() =>
                    {
                        switch (id)
                        {
                            case 1: _position = number; ShowPosition(); break;
                            case 2: _duration = number; Timeline.Maximum = Math.Max(1, number); ShowPosition(); break;
                            case 3: _paused = flag; PlayPauseIcon.Glyph = flag ? "" : ""; break;
                        }
                    });
                    break;
                }
                case Mpv.EventFileLoaded:
                    DispatcherQueue.TryEnqueue(() => Note("file loaded"));
                    break;
                case Mpv.EventVideoReconfig:
                    DispatcherQueue.TryEnqueue(() => Note("video reconfigured"));
                    break;
                case Mpv.EventEndFile:
                    DispatcherQueue.TryEnqueue(() => Note($"end of file ({ev.Error})"));
                    break;
            }
        }
    }

    /// <summary>
    /// Picks up mpv's swapchain whenever it is a new one (the first frame, and after anything
    /// that rebuilds the VO, such as a change of output colour space), and refreshes the record.
    /// </summary>
    private void Tick()
    {
        if (_mpv == 0)
            return;
        var swapchain = (nint)(Mpv.GetInt64(_mpv, "display-swapchain") ?? 0);
        if (swapchain != _boundSwapchain && swapchain != 0)
        {
            Dxgi.SetSwapChain(VideoPanel, swapchain);
            var scale = VideoPanel.CompositionScaleX;
            if (Math.Abs(scale - 1) > 0.001)
                Dxgi.SetInverseScale(swapchain, VideoPanel.CompositionScaleX, VideoPanel.CompositionScaleY);
            Note($"bound swapchain 0x{swapchain:x} to the SwapChainPanel (composition scale {scale:0.##})");
            _boundSwapchain = swapchain;
        }

        if (_options.SwitchAt is { } switchAt && !_switched && _position >= switchAt)
        {
            _switched = true;
            HdrToggle.IsChecked = _output != "hdr";
            OnHdrToggle(HdrToggle, new RoutedEventArgs());
        }
        if (_options.ResizeAt is { } resizeAt && !_resized && _position >= resizeAt)
        {
            _resized = true;
            AppWindow.Resize(new Windows.Graphics.SizeInt32(800, 520));
        }
        if (_options.PauseAt is { } pauseAt && !_pausedOnce && _position >= pauseAt)
        {
            _pausedOnce = true;
            Mpv.SetPropertyString(_mpv, "pause", "yes");
        }
        if (_options.QuitAt is { } quitAt && !_quit && _position >= quitAt)
        {
            _quit = true;
            WriteReport();
            Close();
            return;
        }
        WriteReport();
    }

    private void WriteReport()
    {
        var text = Describe();
        Info.Text = text;
        if (text == _lastReport)
            return;
        _lastReport = text;
        var all = new StringBuilder(text).AppendLine().AppendLine("events:");
        foreach (var e in _events)
            all.Append("  ").AppendLine(e);
        File.WriteAllText(_reportPath, all.ToString());
    }

    private string Describe()
    {
        string P(string name) => Mpv.GetString(_mpv, name) ?? "-";
        var sb = new StringBuilder();
        sb.AppendLine($"vo {P("current-vo")} / {P("current-gpu-context")}, d3d11 composition, hwdec {P("hwdec-current")}");
        sb.AppendLine($"output asked: {_output}{string.Concat(_options.Set.Select(o => $", {o.Name}={o.Value}"))}; composition size {P("d3d11-composition-size")}");
        if (_boundSwapchain != 0)
        {
            var (w, h, f) = Dxgi.Describe(_boundSwapchain);
            sb.AppendLine($"swapchain 0x{_boundSwapchain:x}: {w}x{h} {Dxgi.FormatName(f)}");
        }
        sb.AppendLine($"video: {P("video-params/pixelformat")} {P("video-params/w")}x{P("video-params/h")}, " +
                      $"{P("video-params/colormatrix")} / {P("video-params/primaries")} / {P("video-params/gamma")}, " +
                      $"sig-peak {P("video-params/sig-peak")}, max-cll {P("video-params/max-cll")}");
        sb.AppendLine($"target: {P("video-target-params/primaries")} / {P("video-target-params/gamma")}, " +
                      $"sig-peak {P("video-target-params/sig-peak")}, max-luma {P("video-target-params/max-luma")}");
        sb.AppendLine($"display-names {P("display-names")}, display-fps {P("display-fps")}");
        foreach (var output in Dxgi.Outputs())
            sb.AppendLine($"dxgi: {output}");
        sb.AppendLine($"engine: {_engine}");
        return sb.ToString().TrimEnd();
    }

    private void Note(string what)
    {
        _events.Add($"{DateTime.Now:HH:mm:ss.fff} {what}");
        Subtitle.Text = $"Episode 1 · {what}";
    }

    private void ShowPosition()
    {
        Elapsed.Text = Clock(_position);
        Remaining.Text = "-" + Clock(Math.Max(0, _duration - _position));
        _timelineFromMpv = true;
        Timeline.Value = _position;
        _timelineFromMpv = false;
    }

    private static string Clock(double seconds) => TimeSpan.FromSeconds(seconds).ToString(@"m\:ss");

    private void OnPlayPause(object sender, RoutedEventArgs e) => Mpv.CommandString(_mpv, "cycle pause");
    private void OnBack(object sender, RoutedEventArgs e) => Mpv.CommandString(_mpv, "seek -10");
    private void OnForward(object sender, RoutedEventArgs e) => Mpv.CommandString(_mpv, "seek 10");

    private void OnTimelineChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs e)
    {
        if (!_timelineFromMpv && _mpv != 0)
            Mpv.CommandString(_mpv, $"seek {e.NewValue.ToString(CultureInfo.InvariantCulture)} absolute");
    }

    /// <summary>
    /// SDR and HDR10 output at runtime, by <c>target-colorspace-hint</c> alone: libplacebo then
    /// reconfigures the swapchain it has (format and colour space) instead of mpv building a new
    /// VO, which the d3d11 options would make it do.
    /// </summary>
    private void OnHdrToggle(object sender, RoutedEventArgs e)
    {
        _output = HdrToggle.IsChecked == true ? "hdr" : "sdr";
        Mpv.SetPropertyString(_mpv, "target-colorspace-hint", _output == "hdr" ? "yes" : "no");
        Note($"output switched to {_output}");
    }

    private void Shutdown()
    {
        _timer.Stop();
        if (_mpv == 0)
            return;
        try { Dxgi.SetSwapChain(VideoPanel, 0); } catch (COMException) { }
        _closing = true;
        Mpv.Wakeup(_mpv);
        _eventThread?.Join(TimeSpan.FromSeconds(2));
        Mpv.TerminateDestroy(_mpv);
        _mpv = 0;
    }
}
