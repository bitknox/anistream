using System.Runtime.InteropServices;
using System.Text.Json;
using Anistream.Engine.Model;
using Microsoft.UI.Windowing;
using Windows.Graphics;

namespace Anistream.Windows;

/// <summary>Where the window was left: its normal frame in physical pixels, and whether it was maximised.</summary>
sealed record Place(RectInt32 Frame, bool Maximised)
{
    sealed record Stored(int X, int Y, int Width, int Height, bool Maximised);

    public static Place? Read(string path)
    {
        try
        {
            if (!File.Exists(path))
                return null;
            var s = JsonSerializer.Deserialize<Stored>(File.ReadAllText(path));
            return s is { Width: > 0, Height: > 0 } ? new Place(new RectInt32(s.X, s.Y, s.Width, s.Height), s.Maximised) : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    public static void Write(string path, Place place)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            var f = place.Frame;
            File.WriteAllText(path, JsonSerializer.Serialize(new Stored(f.X, f.Y, f.Width, f.Height, place.Maximised)));
        }
        catch (Exception)
        {
            // Not kept: the next launch centres the window, which is harmless.
        }
    }

    /// <summary>Whether enough of the frame's title strip lies on a display to be grabbed.</summary>
    public static bool OnADisplay(RectInt32 frame)
    {
        foreach (var display in DisplayArea.FindAll())
        {
            var area = display.WorkArea;
            var left = Math.Max(area.X, frame.X);
            var right = Math.Min(area.X + area.Width, frame.X + frame.Width);
            var top = frame.Y;
            if (right - left >= 120 && top >= area.Y - 8 && top < area.Y + area.Height - 40)
                return true;
        }
        return false;
    }
}

/// <summary>
/// The taskbar button's progress: overall download progress while anything downloads, paused
/// when everything waits, red when one failed (ITaskbarList3).
/// </summary>
sealed class Taskbar
{
    const int NoProgress = 0, Normal = 2, Error = 4, Paused = 8;

    readonly ITaskbarList3 list;
    readonly nint hwnd;

    Taskbar(ITaskbarList3 list, nint hwnd)
    {
        this.list = list;
        this.hwnd = hwnd;
    }

    public static Taskbar? For(nint hwnd)
    {
        try
        {
            var list = (ITaskbarList3)new TaskbarListClass();
            list.HrInit();
            return new Taskbar(list, hwnd);
        }
        catch (Exception)
        {
            return null;
        }
    }

    public void Downloads(DownloadRow[] rows)
    {
        try
        {
            var going = rows.Where(r => r.State is DownloadState.Active or DownloadState.Queued or DownloadState.Paused).ToArray();
            if (going.Length == 0)
            {
                list.SetProgressState(hwnd, rows.Any(r => r.State == DownloadState.Failed) ? Error : NoProgress);
                if (rows.Any(r => r.State == DownloadState.Failed))
                    list.SetProgressValue(hwnd, 100, 100);
                return;
            }
            ulong done = 0, total = 0;
            foreach (var r in going)
            {
                done += r.Downloaded;
                total += r.Total;
            }
            var state = rows.Any(r => r.State == DownloadState.Failed) ? Error
                : going.All(r => r.State == DownloadState.Paused) ? Paused
                : Normal;
            list.SetProgressState(hwnd, state);
            if (total > 0)
                list.SetProgressValue(hwnd, done, total);
        }
        catch (Exception)
        {
            // The taskbar is decoration: nothing to do.
        }
    }

    public void Clear()
    {
        try
        {
            list.SetProgressState(hwnd, NoProgress);
        }
        catch (Exception)
        {
        }
    }

    [ComImport, Guid("56FDF344-FD6D-11d0-958A-006097C9A090"), ClassInterface(ClassInterfaceType.None)]
    class TaskbarListClass;

    [ComImport, Guid("ea1afb91-9e28-4b86-90e9-9e9f8a5eefaf"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface ITaskbarList3
    {
        // ITaskbarList
        void HrInit();
        void AddTab(nint hwnd);
        void DeleteTab(nint hwnd);
        void ActivateTab(nint hwnd);
        void SetActiveAlt(nint hwnd);
        // ITaskbarList2
        void MarkFullscreenWindow(nint hwnd, [MarshalAs(UnmanagedType.Bool)] bool fullscreen);
        // ITaskbarList3
        void SetProgressValue(nint hwnd, ulong completed, ulong total);
        void SetProgressState(nint hwnd, int flags);
    }
}
