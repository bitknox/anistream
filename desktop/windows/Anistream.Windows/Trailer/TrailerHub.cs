using Microsoft.Web.WebView2.Core;

namespace Anistream.Windows;

/// <summary>
/// What every trailer shares: the one WebView2 environment (private, so nothing a trailer sets is
/// kept), whether the runtime is there, and the rule that one trailer plays by itself at a time.
/// </summary>
static class TrailerHub
{
    static Task<CoreWebView2Environment>? environment;
    static bool? present;
    static readonly List<TrailerStage> playing = [];

    /// <summary>The WebView2 runtime is installed. An old Windows 10 without it means no trailers, silently.</summary>
    public static bool Available
    {
        get
        {
            present ??= Check();
            return present.Value;
        }
    }

    static bool Check()
    {
        try
        {
            return !string.IsNullOrEmpty(CoreWebView2Environment.GetAvailableBrowserVersionString());
        }
        catch (Exception)
        {
            return false;
        }
    }

    /// <summary>The environment every trailer's web view is made in.</summary>
    public static Task<CoreWebView2Environment> Environment() => environment ??= Make();

    static async Task<CoreWebView2Environment> Make()
    {
        var folder = Path.Combine(Path.GetTempPath(), "anistream-windows", "webview2");
        var options = new CoreWebView2EnvironmentOptions
        {
            // The window's trailer plays with sound the moment it opens, which Chromium holds back
            // without a click. The banner and preview ones are muted and need nothing.
            AdditionalBrowserArguments = "--autoplay-policy=no-user-gesture-required",
        };
        return await CoreWebView2Environment.CreateWithOptionsAsync(null, folder, options);
    }

    /// <summary>A web view made private: nothing it stores is kept.</summary>
    public static async Task<CoreWebView2ControllerOptions> Private()
    {
        var options = (await Environment()).CreateCoreWebView2ControllerOptions();
        options.IsInPrivateModeEnabled = true;
        return options;
    }

    /// <summary>
    /// This trailer is about to play by itself: any other that does is let go of, because only one
    /// automatically playing YouTube player may be on screen at a time.
    /// </summary>
    public static void Claim(TrailerStage stage)
    {
        foreach (var other in playing.Where(s => s != stage).ToList())
            other.Stop();
        if (!playing.Contains(stage))
            playing.Add(stage);
    }

    public static void Release(TrailerStage stage) => playing.Remove(stage);

    /// <summary>Every trailer is let go of: the window lost focus, an episode starts, a sheet opens.</summary>
    public static void StopAll()
    {
        foreach (var stage in playing.ToList())
            stage.Stop();
        HoverPreview.Close();
    }
}
