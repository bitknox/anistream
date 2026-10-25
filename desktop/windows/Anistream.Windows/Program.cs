using System.Runtime.InteropServices;
using Anistream.App;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace Anistream.Windows;

/// <summary>
/// The entry point, in place of the one XAML generates: one anistream runs at a time. A second
/// launch, an <c>anistream://</c> link clicked in a browser or the Start menu while the app is
/// open, hands what it was launched with to the running copy, through the Windows App SDK's
/// AppInstance, and exits; that copy comes to the front and opens the link.
/// </summary>
public static partial class Program
{
#if ANISTREAM_TESTING
    // The testing build keeps to itself, so a fixture run never takes the user's own app's links.
    const string Key = "anistream-testing";
#else
    const string Key = "anistream";
#endif

    /// <summary>Activations that come in before the window is there to take them.</summary>
    static readonly List<string> early = [];
    static Action<string?>? heard;

    [STAThread]
    static int Main(string[] args)
    {
        WinRT.ComWrappersSupport.InitializeComWrappers();
        var main = AppInstance.FindOrRegisterForKey(Key);
        if (!main.IsCurrent)
        {
            Redirect(main);
            return 0;
        }
        main.Activated += (_, e) => Activated(e);
        Application.Start(_ =>
        {
            SynchronizationContext.SetSynchronizationContext(new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread()));
            new App();
        });
        return 0;
    }

    /// <summary>
    /// Where a later launch goes, on the UI thread: its link, or null for a plain launch, which
    /// only brings the window forward. Ones that came before this was set are handed over now.
    /// </summary>
    internal static void Listen(Action<string?> on)
    {
        string[] waiting;
        lock (early)
        {
            heard = on;
            waiting = [.. early];
            early.Clear();
        }
        foreach (var link in waiting)
            on(link.Length == 0 ? null : link);
    }

    /// <summary>Another launch, redirected here: on a thread of the App SDK's, never the UI's.</summary>
    static void Activated(AppActivationArguments e)
    {
        var link = e.Data switch
        {
            global::Windows.ApplicationModel.Activation.IProtocolActivatedEventArgs p => p.Uri?.AbsoluteUri,
            global::Windows.ApplicationModel.Activation.ILaunchActivatedEventArgs l => Link.InCommandLine(l.Arguments),
            _ => null,
        };
        Action<string?>? on;
        lock (early)
        {
            on = heard;
            if (on is null)
                early.Add(link ?? "");
        }
        on?.Invoke(link);
    }

    /// <summary>
    /// This launch, to the running copy. The wait keeps the thread pumping COM, as the App SDK's
    /// sample does, and this launch, which Windows let take the foreground, lets the running copy
    /// take it instead.
    /// </summary>
    static void Redirect(AppInstance main)
    {
        AllowSetForegroundWindow(main.ProcessId);
        var args = AppInstance.GetCurrent().GetActivatedEventArgs();
        var done = CreateEventW(0, true, false, null);
        Task.Run(() =>
        {
            try
            {
                main.RedirectActivationToAsync(args).AsTask().Wait(TimeSpan.FromSeconds(10));
            }
            finally
            {
                SetEvent(done);
            }
        });
        _ = CoWaitForMultipleObjects(0, 0xFFFFFFFF, 1, [done], out _);
        CloseHandle(done);
    }

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool AllowSetForegroundWindow(uint processId);

    [LibraryImport("kernel32.dll", StringMarshalling = StringMarshalling.Utf16)]
    private static partial nint CreateEventW(nint attributes, [MarshalAs(UnmanagedType.Bool)] bool manualReset, [MarshalAs(UnmanagedType.Bool)] bool initialState, string? name);

    [LibraryImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool SetEvent(nint handle);

    [LibraryImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static partial bool CloseHandle(nint handle);

    [LibraryImport("ole32.dll")]
    private static partial int CoWaitForMultipleObjects(uint flags, uint timeout, uint count, nint[] handles, out uint index);
}
