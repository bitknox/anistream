using System.Runtime.InteropServices;

namespace Anistream.PlayerSpike;

/// <summary>
/// The few calls of libmpv's client API (2.x) the spike makes. The app proper drives mpv through
/// the engine's hosted player protocol and anistream-mpv-host; this is only enough to put a
/// picture in the panel and read back what mpv says about colour.
/// </summary>
internal static partial class Mpv
{
    private const string Lib = "libmpv-2.dll";

    public const int FormatString = 1;
    public const int FormatFlag = 3;
    public const int FormatInt64 = 4;
    public const int FormatDouble = 5;

    public const int EventNone = 0;
    public const int EventShutdown = 1;
    public const int EventLogMessage = 2;
    public const int EventEndFile = 7;
    public const int EventFileLoaded = 8;
    public const int EventVideoReconfig = 17;
    public const int EventPropertyChange = 22;

    [StructLayout(LayoutKind.Sequential)]
    public struct Event
    {
        public int EventId;
        public int Error;
        public ulong ReplyUserdata;
        public nint Data;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct EventProperty
    {
        public nint Name;
        public int Format;
        public nint Data;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct EventLog
    {
        public nint Prefix;
        public nint Level;
        public nint Text;
        public int LogLevel;
    }

    [LibraryImport(Lib, EntryPoint = "mpv_create")]
    public static partial nint Create();

    [LibraryImport(Lib, EntryPoint = "mpv_initialize")]
    public static partial int Initialize(nint ctx);

    [LibraryImport(Lib, EntryPoint = "mpv_terminate_destroy")]
    public static partial void TerminateDestroy(nint ctx);

    [LibraryImport(Lib, EntryPoint = "mpv_client_api_version")]
    public static partial ulong ClientApiVersion();

    [LibraryImport(Lib, EntryPoint = "mpv_error_string")]
    public static partial nint ErrorString(int error);

    [LibraryImport(Lib, EntryPoint = "mpv_set_option_string", StringMarshalling = StringMarshalling.Utf8)]
    public static partial int SetOptionString(nint ctx, string name, string value);

    [LibraryImport(Lib, EntryPoint = "mpv_set_property_string", StringMarshalling = StringMarshalling.Utf8)]
    public static partial int SetPropertyString(nint ctx, string name, string value);

    [LibraryImport(Lib, EntryPoint = "mpv_get_property_string", StringMarshalling = StringMarshalling.Utf8)]
    private static partial nint GetPropertyStringRaw(nint ctx, string name);

    [LibraryImport(Lib, EntryPoint = "mpv_get_property", StringMarshalling = StringMarshalling.Utf8)]
    public static unsafe partial int GetProperty(nint ctx, string name, int format, void* data);

    [LibraryImport(Lib, EntryPoint = "mpv_observe_property", StringMarshalling = StringMarshalling.Utf8)]
    public static partial int ObserveProperty(nint ctx, ulong userdata, string name, int format);

    [LibraryImport(Lib, EntryPoint = "mpv_request_log_messages", StringMarshalling = StringMarshalling.Utf8)]
    public static partial int RequestLogMessages(nint ctx, string minLevel);

    [LibraryImport(Lib, EntryPoint = "mpv_command_string", StringMarshalling = StringMarshalling.Utf8)]
    public static partial int CommandString(nint ctx, string args);

    [LibraryImport(Lib, EntryPoint = "mpv_wait_event")]
    public static partial nint WaitEvent(nint ctx, double timeout);

    [LibraryImport(Lib, EntryPoint = "mpv_wakeup")]
    public static partial void Wakeup(nint ctx);

    [LibraryImport(Lib, EntryPoint = "mpv_free")]
    public static partial void Free(nint data);

    public static string Error(int code) => Marshal.PtrToStringUTF8(ErrorString(code)) ?? code.ToString();

    public static void Check(int code, string what)
    {
        if (code < 0)
            throw new InvalidOperationException($"mpv: {what}: {Error(code)}");
    }

    public static string? GetString(nint ctx, string name)
    {
        var raw = GetPropertyStringRaw(ctx, name);
        if (raw == 0)
            return null;
        try { return Marshal.PtrToStringUTF8(raw); }
        finally { Free(raw); }
    }

    public static unsafe long? GetInt64(nint ctx, string name)
    {
        long value = 0;
        return GetProperty(ctx, name, FormatInt64, &value) >= 0 ? value : null;
    }

    public static unsafe double? GetDouble(nint ctx, string name)
    {
        double value = 0;
        return GetProperty(ctx, name, FormatDouble, &value) >= 0 ? value : null;
    }
}
