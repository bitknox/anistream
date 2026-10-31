using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Windows.System.Power;
using Windows.UI.ViewManagement;

namespace Anistream.Windows;

/// <summary>
/// What Windows says of the device, said to the engine at launch and on every change
/// (<c>DeviceState</c>): animation effects off, energy saver on, Narrator running and a machine
/// with under 4 GB of memory. With the network path and the Play trailers setting the engine
/// decides from these whether trailers start by themselves. The thermal state is not something
/// Windows tells an app, so it is always said to be fine.
/// </summary>
sealed class DeviceWatch : IDisposable
{
    readonly Action<Command> send;
    readonly DispatcherQueue ui;
    readonly DispatcherQueueTimer poll;
    readonly UISettings settings = new();
    (bool, bool, bool, bool)? last;

    public DeviceWatch(Action<Command> send, DispatcherQueue ui)
    {
        this.send = send;
        this.ui = ui;
        PowerManager.EnergySaverStatusChanged += Changed;
        // Animation effects and a screen reader have no event to wait for; both are cheap to read.
        poll = ui.CreateTimer();
        poll.Interval = TimeSpan.FromSeconds(5);
        poll.Tick += (_, _) => Say();
        poll.Start();
        Say();
    }

    void Changed(object? sender, object e) => ui.TryEnqueue(Say);

    void Say()
    {
        var state = (!settings.AnimationsEnabled, PowerManager.EnergySaverStatus == EnergySaverStatus.On, ScreenReaderRunning(), LowMemory);
        if (state == last)
            return;
        last = state;
        send(new Command.DeviceState(ReduceMotion: state.Item1, LowPower: state.Item2, ScreenReader: state.Item3, LowMemory: state.Item4));
    }

    static bool LowMemory => GC.GetGCMemoryInfo().TotalAvailableMemoryBytes < 4L * 1024 * 1024 * 1024;

    const uint GetScreenReader = 0x0046;

    /// <summary>Narrator, and the screen readers that follow it, set this flag while they run.</summary>
    internal static bool ScreenReaderRunning() => SystemParametersInfo(GetScreenReader, 0, out var on, 0) && on;

    [System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true)]
    [return: System.Runtime.InteropServices.MarshalAs(System.Runtime.InteropServices.UnmanagedType.Bool)]
    static extern bool SystemParametersInfo(uint action, uint param, [System.Runtime.InteropServices.MarshalAs(System.Runtime.InteropServices.UnmanagedType.Bool)] out bool value, uint winIni);

    public void Dispose()
    {
        PowerManager.EnergySaverStatusChanged -= Changed;
        poll.Stop();
    }
}
