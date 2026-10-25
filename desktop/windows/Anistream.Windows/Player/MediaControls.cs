using System.Runtime.InteropServices;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.Win32;
using Windows.Media;
using Windows.Storage.Streams;

namespace Anistream.Windows.Player;

/// <summary>
/// What the player tells Windows while an episode plays: the media overlay and lock screen through
/// SystemMediaTransportControls (the episode, the show, its banner, the timeline, and the media
/// keys answered), and Previous, Play or Pause, and Next under the taskbar thumbnail. The Mac's
/// <c>NowPlaying</c> with the remote commands, done the Windows way. Previous and Next step an
/// episode, as there. Both are cleared when the player closes. The timeline is written on play,
/// pause, seek and episode changes, not on every tick: Windows runs the clock between.
/// </summary>
sealed partial class MediaControls
{
    const uint WM_COMMAND = 0x0111, THBN_CLICKED = 0x1800;
    const uint PreviousId = 1, PlayPauseId = 2, NextId = 3;
    const nuint SubclassId = 0x4154;

    static MediaControls? current;

    readonly nint hwnd;
    readonly Taskbar? taskbar;
    readonly Art art;
    readonly DispatcherQueue ui;
    readonly Action<PlayerCommand> send;
    readonly SystemMediaTransportControls? smtc;
    readonly uint buttonCreated;
    readonly nint[] icons = new nint[4];

    bool open, paused, buttonsAdded;
    string show = "", episode = "", artUrl = "";
    double position, duration, writtenAt = double.NaN;
    bool writtenPaused;
    readonly System.Diagnostics.Stopwatch sinceWritten = new();

    unsafe MediaControls(nint hwnd, Taskbar? taskbar, Art art, DispatcherQueue ui, Action<PlayerCommand> send)
    {
        this.hwnd = hwnd;
        this.taskbar = taskbar;
        this.art = art;
        this.ui = ui;
        this.send = send;
        try
        {
            smtc = SystemMediaTransportControlsInterop.GetForWindow(hwnd);
            smtc.IsEnabled = false;
            smtc.IsPlayEnabled = smtc.IsPauseEnabled = true;
            smtc.IsNextEnabled = smtc.IsPreviousEnabled = true;
            smtc.IsStopEnabled = false;
            smtc.ButtonPressed += (_, e) => ui.TryEnqueue(() => Pressed(e.Button));
            smtc.PlaybackPositionChangeRequested += (_, e) => ui.TryEnqueue(() => send(new PlayerCommand.SeekTo(e.RequestedPlaybackPosition.TotalSeconds)));
        }
        catch (Exception e)
        {
            // Without the system's media controls the player still plays.
            Note($"no media controls: {e.GetType().Name} 0x{e.HResult:X8} {e.Message}");
            smtc = null;
        }
        fixed (char* name = "TaskbarButtonCreated")
            buttonCreated = RegisterWindowMessageW(name);
        current = this;
        SetWindowSubclass(hwnd, &Proc, SubclassId, 0);
        // The taskbar button exists already when the window is shown before this.
        AddButtons();
    }

    public static MediaControls Start(nint hwnd, Taskbar? taskbar, Art art, DispatcherQueue ui, Action<PlayerCommand> send) =>
        new(hwnd, taskbar, art, ui, send);

    void Pressed(SystemMediaTransportControlsButton button)
    {
        switch (button)
        {
            case SystemMediaTransportControlsButton.Play when paused:
            case SystemMediaTransportControlsButton.Pause when !paused:
                send(new PlayerCommand.TogglePause());
                break;
            case SystemMediaTransportControlsButton.Next:
                send(new PlayerCommand.Step(1));
                break;
            case SystemMediaTransportControlsButton.Previous:
                send(new PlayerCommand.Step(-1));
                break;
        }
    }

    // What the player and the engine say.

    public void Heard(FfiPlayerEvent e)
    {
        switch (e)
        {
            case FfiPlayerEvent.Opened:
                Open(true);
                break;
            case FfiPlayerEvent.Closed:
                Open(false);
                break;
            case FfiPlayerEvent.State s:
                State(s.StateValue);
                break;
        }
    }

    public void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.PlayerUi { Ui: PlayerUi.Starting s }:
                    Open(true);
                    Now(s.Title, s.EpisodeTitle is { Length: > 0 } t ? $"Episode {s.EpisodeValue} · {t}" : $"Episode {s.EpisodeValue}", s.BannerUrl ?? s.CoverUrl);
                    break;
                case Event.PlayerUi { Ui: PlayerUi.Episode ep }:
                    Now(ep.Show, ep.Title is { Length: > 0 } title ? $"{ep.Label} · {title}" : ep.Label, null);
                    break;
            }
        }
    }

    void Open(bool on)
    {
        if (open == on)
            return;
        open = on;
        if (!on)
        {
            show = episode = artUrl = "";
            position = duration = 0;
            writtenAt = double.NaN;
        }
        if (smtc is not null)
        {
            smtc.IsEnabled = on;
            smtc.PlaybackStatus = on ? MediaPlaybackStatus.Changing : MediaPlaybackStatus.Closed;
            if (!on)
                smtc.DisplayUpdater.ClearAll();
        }
        Buttons();
    }

    /// <summary>The episode and show, as the overlay names them, and the show's banner as its picture.</summary>
    void Now(string showTitle, string episodeLine, string? picture)
    {
        if (smtc is null || (showTitle == show && episodeLine == episode && (picture is null || picture == artUrl)))
            return;
        show = showTitle;
        episode = episodeLine;
        var display = smtc.DisplayUpdater;
        display.Type = MediaPlaybackType.Video;
        display.VideoProperties.Title = episode;
        display.VideoProperties.Subtitle = show;
        display.Update();
        writtenAt = double.NaN;
        if (picture is not { Length: > 0 } url || url == artUrl)
            return;
        artUrl = url;
        art.Bytes(url, bytes => ui.TryEnqueue(async () =>
        {
            if (artUrl != url || !open)
                return;
            try
            {
                var stream = new InMemoryRandomAccessStream();
                using (var writer = new DataWriter(stream))
                {
                    writer.WriteBytes(bytes);
                    await writer.StoreAsync();
                    writer.DetachStream();
                }
                stream.Seek(0);
                display.Thumbnail = RandomAccessStreamReference.CreateFromStream(stream);
                display.Update();
            }
            catch (Exception)
            {
                // The overlay without its picture.
            }
        }));
    }

    void State(FfiPlayerState state)
    {
        var wasPaused = paused;
        paused = state.Paused;
        if (state.Position is { } p)
            position = p;
        if (state.Duration is { } d && d > 0)
            duration = d;
        if (wasPaused != paused)
            Buttons();
        if (smtc is null || !open)
            return;
        smtc.PlaybackStatus = paused ? MediaPlaybackStatus.Paused : MediaPlaybackStatus.Playing;
        // Written again on pause or play, on a jump the clock would not have made, and now and then.
        var expected = double.IsNaN(writtenAt) ? double.NaN : writtenAt + (writtenPaused ? 0 : sinceWritten.Elapsed.TotalSeconds);
        if (double.IsNaN(expected) || paused != writtenPaused || Math.Abs(position - expected) > 2 || sinceWritten.Elapsed.TotalSeconds > 15)
        {
            smtc.UpdateTimelineProperties(new SystemMediaTransportControlsTimelineProperties
            {
                StartTime = TimeSpan.Zero,
                MinSeekTime = TimeSpan.Zero,
                Position = TimeSpan.FromSeconds(position),
                EndTime = TimeSpan.FromSeconds(duration),
                MaxSeekTime = TimeSpan.FromSeconds(duration),
            });
            writtenAt = position;
            writtenPaused = paused;
            sinceWritten.Restart();
        }
    }

    /// <summary>A line in the temp folder's log, beside the crash log, for controls that never showed.</summary>
    static void Note(string line)
    {
        try
        {
            var dir = Path.Combine(Path.GetTempPath(), "anistream-windows");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "media.txt"), $"{DateTime.Now:O} {line}\n");
        }
        catch (IOException)
        {
        }
    }

    // The thumbnail's buttons.

    void AddButtons()
    {
        if (taskbar is null || buttonsAdded)
            return;
        buttonsAdded = taskbar.ThumbButtons(Buttons(hidden: true), first: true);
        Note(buttonsAdded ? "thumbnail buttons added" : "thumbnail buttons refused, waiting for the taskbar button");
        if (buttonsAdded && open)
            Buttons();
    }

    void Buttons()
    {
        if (taskbar is not null && buttonsAdded)
            taskbar.ThumbButtons(Buttons(hidden: !open), first: false);
    }

    Taskbar.ThumbButton[] Buttons(bool hidden)
    {
        Taskbar.ThumbButton Button(uint id, Glyph glyph, string tip) => new()
        {
            Mask = Taskbar.ThumbButton.Icon | Taskbar.ThumbButton.Tooltip | Taskbar.ThumbButton.Flags,
            Id = id,
            IconHandle = Icon(glyph),
            Tip = tip,
            State = hidden ? Taskbar.ThumbButton.Hidden : Taskbar.ThumbButton.Enabled,
        };
        return
        [
            Button(PreviousId, Glyph.Previous, "Previous episode"),
            paused ? Button(PlayPauseId, Glyph.Play, "Play") : Button(PlayPauseId, Glyph.Pause, "Pause"),
            Button(NextId, Glyph.Next, "Next episode"),
        ];
    }

    [UnmanagedCallersOnly]
    static nint Proc(nint hwnd, uint message, nint wparam, nint lparam, nuint id, nuint data)
    {
        if (current is { } controls)
        {
            if (message == controls.buttonCreated && controls.buttonCreated != 0)
            {
                // Explorer restarted, or the button came after this: the buttons go on again.
                controls.buttonsAdded = false;
                controls.AddButtons();
            }
            else if (message == WM_COMMAND && ((wparam >> 16) & 0xFFFF) == THBN_CLICKED)
            {
                switch ((uint)(wparam & 0xFFFF))
                {
                    case PreviousId:
                        controls.send(new PlayerCommand.Step(-1));
                        break;
                    case PlayPauseId:
                        controls.send(new PlayerCommand.TogglePause());
                        break;
                    case NextId:
                        controls.send(new PlayerCommand.Step(1));
                        break;
                }
                return 0;
            }
        }
        return DefSubclassProc(hwnd, message, wparam, lparam);
    }

    // The buttons' icons, drawn here: the shell takes an HICON, and Segoe's glyphs are not one.

    enum Glyph { Previous, Play, Pause, Next }

    nint Icon(Glyph glyph)
    {
        if (icons[(int)glyph] is var made and not 0)
            return made;
        return icons[(int)glyph] = Draw(glyph, GetSystemMetrics(SM_CXSMICON), LightTaskbar());
    }

    /// <summary>Whether the taskbar, and so the thumbnail under which the buttons sit, is light.</summary>
    static bool LightTaskbar()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            return key?.GetValue("SystemUsesLightTheme") is int light && light == 1;
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// <summary>The shapes on a unit square: triangles as three corners, bars as rectangles.</summary>
    static (double[][] Triangles, double[][] Bars) Shapes(Glyph glyph) => glyph switch
    {
        Glyph.Play => (new[] { new[] { 0.30, 0.18, 0.30, 0.82, 0.84, 0.50 } }, Array.Empty<double[]>()),
        Glyph.Pause => (Array.Empty<double[]>(), new[] { new[] { 0.26, 0.18, 0.42, 0.82 }, new[] { 0.58, 0.18, 0.74, 0.82 } }),
        Glyph.Next => (new[] { new[] { 0.18, 0.18, 0.18, 0.82, 0.66, 0.50 } }, new[] { new[] { 0.68, 0.18, 0.82, 0.82 } }),
        _ => (new[] { new[] { 0.82, 0.18, 0.82, 0.82, 0.34, 0.50 } }, new[] { new[] { 0.18, 0.18, 0.32, 0.82 } }),
    };

    /// <summary>A <paramref name="size"/>-pixel icon of <paramref name="glyph"/>, antialiased by sampling each pixel 4 × 4 times.</summary>
    static uint[] Pixels(Glyph glyph, int size, bool light)
    {
        var (triangles, bars) = Shapes(glyph);
        static double Side(double ax, double ay, double bx, double by, double x, double y) => (bx - ax) * (y - ay) - (by - ay) * (x - ax);
        bool Inside(double x, double y)
        {
            foreach (var b in bars)
                if (x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3])
                    return true;
            foreach (var t in triangles)
            {
                var d1 = Side(t[0], t[1], t[2], t[3], x, y);
                var d2 = Side(t[2], t[3], t[4], t[5], x, y);
                var d3 = Side(t[4], t[5], t[0], t[1], x, y);
                if (!((d1 < 0 || d2 < 0 || d3 < 0) && (d1 > 0 || d2 > 0 || d3 > 0)))
                    return true;
            }
            return false;
        }
        var colour = light ? 0x1A1A1Au : 0xFFFFFFu;
        var pixels = new uint[size * size];
        for (var py = 0; py < size; py++)
            for (var px = 0; px < size; px++)
            {
                var hits = 0;
                for (var sy = 0; sy < 4; sy++)
                    for (var sx = 0; sx < 4; sx++)
                        if (Inside((px + (sx + 0.5) / 4) / size, (py + (sy + 0.5) / 4) / size))
                            hits++;
                var alpha = (uint)Math.Round(hits / 16.0 * 255);
                pixels[py * size + px] = alpha << 24 | colour;
            }
        return pixels;
    }

    static unsafe nint Draw(Glyph glyph, int size, bool light)
    {
        var pixels = Pixels(glyph, size, light);
        var header = new BITMAPINFOHEADER
        {
            biSize = (uint)sizeof(BITMAPINFOHEADER), biWidth = size, biHeight = -size, biPlanes = 1, biBitCount = 32,
        };
        void* bits;
        var colour = CreateDIBSection(0, &header, 0, &bits, 0, 0);
        if (colour == 0)
            return 0;
        fixed (uint* from = pixels)
            System.Buffer.MemoryCopy(from, bits, pixels.Length * 4L, pixels.Length * 4L);
        var maskBits = new byte[(size + 15) / 16 * 2 * size];
        nint mask;
        fixed (byte* m = maskBits)
            mask = CreateBitmap(size, size, 1, 1, m);
        var info = new ICONINFO { fIcon = 1, hbmMask = mask, hbmColor = colour };
        var icon = CreateIconIndirect(&info);
        DeleteObject(colour);
        DeleteObject(mask);
        return icon;
    }

    const int SM_CXSMICON = 49;

    [StructLayout(LayoutKind.Sequential)]
    struct BITMAPINFOHEADER
    {
        public uint biSize;
        public int biWidth, biHeight;
        public ushort biPlanes, biBitCount;
        public uint biCompression, biSizeImage;
        public int biXPelsPerMeter, biYPelsPerMeter;
        public uint biClrUsed, biClrImportant;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct ICONINFO
    {
        public int fIcon;
        public uint xHotspot, yHotspot;
        public nint hbmMask, hbmColor;
    }

    [LibraryImport("user32.dll")]
    private static unsafe partial uint RegisterWindowMessageW(char* name);

    [LibraryImport("user32.dll")]
    private static partial int GetSystemMetrics(int index);

    [LibraryImport("user32.dll")]
    private static unsafe partial nint CreateIconIndirect(ICONINFO* info);

    [LibraryImport("gdi32.dll")]
    private static unsafe partial nint CreateDIBSection(nint dc, BITMAPINFOHEADER* info, uint usage, void** bits, nint section, uint offset);

    [LibraryImport("gdi32.dll")]
    private static unsafe partial nint CreateBitmap(int width, int height, uint planes, uint bitsPerPixel, void* bits);

    [LibraryImport("gdi32.dll")]
    private static partial int DeleteObject(nint handle);

    [LibraryImport("comctl32.dll")]
    private static unsafe partial int SetWindowSubclass(nint hwnd, delegate* unmanaged<nint, uint, nint, nint, nuint, nuint, nint> proc, nuint id, nuint data);

    [LibraryImport("comctl32.dll")]
    private static partial nint DefSubclassProc(nint hwnd, uint message, nint wparam, nint lparam);
}
