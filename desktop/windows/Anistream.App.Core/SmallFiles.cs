namespace Anistream.App;

/// <summary>
/// The app's own small state files (a flag, a choice, a marker), read and written so that a disk
/// that is full, read-only or locked by another program costs the choice and nothing else.
/// </summary>
public static class SmallFiles
{
    /// <summary>The file's text, trimmed, or null when it is not there or can't be read.</summary>
    public static string? Read(string path)
    {
        try
        {
            return File.Exists(path) ? File.ReadAllText(path).Trim() : null;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
            return null;
        }
    }

    /// <summary>Writes the file, its folder first; false when it could not be.</summary>
    public static bool Write(string path, string text)
    {
        try
        {
            if (Path.GetDirectoryName(path) is { Length: > 0 } dir)
                Directory.CreateDirectory(dir);
            File.WriteAllText(path, text);
            return true;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
            return false;
        }
    }

    /// <summary>Removes the file if it is there; false when it is and could not be removed.</summary>
    public static bool Delete(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.SetAttributes(path, FileAttributes.Normal);
                File.Delete(path);
            }
            return true;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or System.Security.SecurityException)
        {
            return false;
        }
    }
}
