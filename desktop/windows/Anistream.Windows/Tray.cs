using System.Runtime.InteropServices;
using Anistream.App;

namespace Anistream.Windows;

/// <summary>
/// The notification area icon while anistream keeps downloading with its window closed (Keep
/// going), as the brief has it: a click opens the window, and its menu holds Open anistream,
/// Pause all downloads and Quit. Shell_NotifyIcon on the main window's own HWND, whose messages
/// it reads through a subclass; the first time, a notification says where anistream went.
/// </summary>
sealed unsafe partial class Tray : IDisposable
{
    const uint WM_APP = 0x8000, CallbackMessage = WM_APP + 0x41;
    const uint NIM_ADD = 0, NIM_MODIFY = 1, NIM_DELETE = 2, NIM_SETVERSION = 4;
    const uint NIF_MESSAGE = 1, NIF_ICON = 2, NIF_TIP = 4, NIF_INFO = 0x10, NIF_SHOWTIP = 0x80;
    const uint NIIF_USER = 4, NIIF_LARGE_ICON = 0x20;
    const uint NOTIFYICON_VERSION_4 = 4;
    const uint WM_CONTEXTMENU = 0x007B, WM_LBUTTONUP = 0x0202, NIN_SELECT = 0x0400, NIN_KEYSELECT = 0x0401;
    const uint MF_STRING = 0, MF_SEPARATOR = 0x800, MF_GRAYED = 1;
    const uint TPM_RIGHTBUTTON = 2, TPM_RETURNCMD = 0x100, TPM_BOTTOMALIGN = 0x20;
    const uint IMAGE_ICON = 1, LR_LOADFROMFILE = 0x10;
    const int SM_CXSMICON = 49, SM_CYSMICON = 50;
    const nuint SubclassId = 0x4153;

    static Tray? current;

    readonly nint hwnd;
    readonly nint icon;
    bool shown;

    public Tray(nint hwnd, string iconPath)
    {
        this.hwnd = hwnd;
        fixed (char* path = iconPath)
            icon = LoadImageW(0, path, IMAGE_ICON, GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON), LR_LOADFROMFILE);
        current = this;
        SetWindowSubclass(hwnd, &Proc, SubclassId, 0);
    }

    public Action? Open { get; init; }
    public Action? PauseAll { get; init; }
    public Action? Quit { get; init; }
    public Func<bool>? AnyRunning { get; init; }

    /// <summary>Adds the icon; with <paramref name="tell"/>, the one-time notification too.</summary>
    public void Show(bool tell)
    {
        var data = Data(NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP);
        if (!shown)
        {
            shown = Shell_NotifyIconW(NIM_ADD, &data) != 0;
            data.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &data);
        }
        if (shown && tell)
        {
            var info = Data(NIF_INFO);
            Copy(T.TrayStillDownloading, info.szInfoTitle, 64);
            Copy(T.TrayWhere, info.szInfo, 256);
            info.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON;
            info.hBalloonIcon = icon;
            Shell_NotifyIconW(NIM_MODIFY, &info);
        }
    }

    public void Hide()
    {
        if (!shown)
            return;
        var data = Data(0);
        Shell_NotifyIconW(NIM_DELETE, &data);
        shown = false;
    }

    NOTIFYICONDATAW Data(uint flags)
    {
        var data = new NOTIFYICONDATAW
        {
            cbSize = (uint)sizeof(NOTIFYICONDATAW),
            hWnd = hwnd,
            uID = 1,
            uFlags = flags,
            uCallbackMessage = CallbackMessage,
            hIcon = icon,
        };
        Copy(T.AppName, data.szTip, 128);
        return data;
    }

    static void Copy(string text, char* into, int size)
    {
        var n = Math.Min(text.Length, size - 1);
        for (var i = 0; i < n; i++)
            into[i] = text[i];
        into[n] = '\0';
    }

    [UnmanagedCallersOnly]
    static nint Proc(nint hwnd, uint message, nint wparam, nint lparam, nuint id, nuint data)
    {
        if (message == CallbackMessage && current is { } tray)
        {
            var e = (uint)(lparam & 0xFFFF);
            if (e is WM_LBUTTONUP or NIN_SELECT or NIN_KEYSELECT)
                tray.Open?.Invoke();
            else if (e == WM_CONTEXTMENU)
                tray.Menu((short)(wparam & 0xFFFF), (short)((wparam >> 16) & 0xFFFF));
            return 0;
        }
        return DefSubclassProc(hwnd, message, wparam, lparam);
    }

    void Menu(int x, int y)
    {
        var menu = CreatePopupMenu();
        Append(menu, MF_STRING, 1, T.TrayOpen);
        Append(menu, AnyRunning?.Invoke() == false ? MF_STRING | MF_GRAYED : MF_STRING, 2, T.DownloadsPauseAllLabel);
        Append(menu, MF_SEPARATOR, 0, "");
        Append(menu, MF_STRING, 3, T.TrayQuit);
        // The menu closes when it loses focus only if the window is in front first.
        SetForegroundWindow(hwnd);
        var chosen = TrackPopupMenuEx(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_BOTTOMALIGN, x, y, hwnd, 0);
        DestroyMenu(menu);
        switch (chosen)
        {
            case 1: Open?.Invoke(); break;
            case 2: PauseAll?.Invoke(); break;
            case 3: Quit?.Invoke(); break;
        }
    }

    static void Append(nint menu, uint flags, nuint id, string text)
    {
        fixed (char* t = text)
            AppendMenuW(menu, flags, id, t);
    }

    public void Dispose()
    {
        Hide();
        RemoveWindowSubclass(hwnd, &Proc, SubclassId);
        if (icon != 0)
            DestroyIcon(icon);
        if (current == this)
            current = null;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct NOTIFYICONDATAW
    {
        public uint cbSize;
        public nint hWnd;
        public uint uID;
        public uint uFlags;
        public uint uCallbackMessage;
        public nint hIcon;
        public fixed char szTip[128];
        public uint dwState;
        public uint dwStateMask;
        public fixed char szInfo[256];
        public uint uVersion;
        public fixed char szInfoTitle[64];
        public uint dwInfoFlags;
        public Guid guidItem;
        public nint hBalloonIcon;
    }

    [LibraryImport("shell32.dll")]
    private static partial int Shell_NotifyIconW(uint message, NOTIFYICONDATAW* data);

    [LibraryImport("comctl32.dll")]
    private static partial int SetWindowSubclass(nint hwnd, delegate* unmanaged<nint, uint, nint, nint, nuint, nuint, nint> proc, nuint id, nuint data);

    [LibraryImport("comctl32.dll")]
    private static partial int RemoveWindowSubclass(nint hwnd, delegate* unmanaged<nint, uint, nint, nint, nuint, nuint, nint> proc, nuint id);

    [LibraryImport("comctl32.dll")]
    private static partial nint DefSubclassProc(nint hwnd, uint message, nint wparam, nint lparam);

    [LibraryImport("user32.dll")]
    private static partial nint LoadImageW(nint instance, char* name, uint type, int cx, int cy, uint flags);

    [LibraryImport("user32.dll")]
    private static partial int GetSystemMetrics(int index);

    [LibraryImport("user32.dll")]
    private static partial int DestroyIcon(nint icon);

    [LibraryImport("user32.dll")]
    private static partial nint CreatePopupMenu();

    [LibraryImport("user32.dll")]
    private static partial int AppendMenuW(nint menu, uint flags, nuint id, char* text);

    [LibraryImport("user32.dll")]
    private static partial int TrackPopupMenuEx(nint menu, uint flags, int x, int y, nint hwnd, nint parameters);

    [LibraryImport("user32.dll")]
    private static partial int DestroyMenu(nint menu);

    [LibraryImport("user32.dll")]
    private static partial int SetForegroundWindow(nint hwnd);
}
