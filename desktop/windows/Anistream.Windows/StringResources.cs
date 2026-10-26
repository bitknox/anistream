using Anistream.App;
using Microsoft.Windows.ApplicationModel.Resources;

namespace Anistream.Windows;

/// <summary>
/// Connects <see cref="T"/>, the generated strings in Anistream.App.Core, to this app's resource
/// files (Strings/&lt;language&gt;/Resources.resw, which the build packs into resources.pri). Until
/// it is installed, and for any key the person's language lacks, T answers with its English.
/// </summary>
static class StringResources
{
    public static void Install()
    {
        try
        {
            var loader = new ResourceLoader();
            T.Resolver = key =>
            {
                try { return loader.GetString(key); }
                catch { return null; }
            };
            T.Language = System.Globalization.CultureInfo.CurrentUICulture.Name;
        }
        catch
        {
            // No resources.pri beside the executable (a bare `dotnet run`): English from T.
        }
    }
}
