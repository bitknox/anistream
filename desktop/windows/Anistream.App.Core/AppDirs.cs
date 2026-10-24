using Anistream.Engine.Ffi;

namespace Anistream.App;

/// <summary>
/// Where the engine keeps its files. The user's own run uses the profile's folders, as Windows
/// apps do: the config roams with the profile, the database and the cache stay on this machine.
/// A fixture run keeps everything under a temporary root, never the profile.
/// </summary>
public static class AppDirs
{
    public static FfiDirs User()
    {
        var roaming = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return new FfiDirs(
            Config: Path.Combine(roaming, "anistream"),
            Data: Path.Combine(local, "anistream", "data"),
            Cache: Path.Combine(local, "anistream", "cache"));
    }

    /// <summary>The engine's folders under <paramref name="root"/>.</summary>
    public static FfiDirs Under(string root) =>
        new(Config: Path.Combine(root, "config"), Data: Path.Combine(root, "data"), Cache: Path.Combine(root, "cache"));

    /// <summary>A new root in the temporary folder for a run that is not the user's own.</summary>
    public static string NewScratchRoot(string what) =>
        Path.Combine(Path.GetTempPath(), $"anistream-{what}-{Guid.NewGuid():N}");
}
