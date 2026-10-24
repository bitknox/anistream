using Anistream.App;
using Anistream.Engine.Ffi;
using Microsoft.UI.Dispatching;

namespace Anistream.Windows;

/// <summary>
/// What the engine asks of Windows, from the engine's own threads: links open in the default
/// browser, notices become in-app toasts, and tracker tokens live in Credential Manager. A
/// fixture run keeps its tokens in memory, so nothing it does reaches the user's credentials.
/// </summary>
sealed class AppHost(DispatcherQueue ui, bool fixture) : FfiHost
{
    readonly Dictionary<string, string> memory = [];

    /// <summary>A notice for the window, on the UI thread.</summary>
    public Action<string, string>? Notified { get; set; }

    public bool OpenUrl(string url)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri))
            return false;
        return ui.TryEnqueue(async () => await global::Windows.System.Launcher.LaunchUriAsync(uri));
    }

    public void Notify(string title, string body) => ui.TryEnqueue(() => Notified?.Invoke(title, body));

    public string? SecretGet(string id)
    {
        if (fixture)
            lock (memory) return memory.GetValueOrDefault(id);
        return Credentials.Get(id);
    }

    public bool SecretSet(string id, string value)
    {
        if (fixture)
        {
            lock (memory) memory[id] = value;
            return true;
        }
        return Credentials.Set(id, value);
    }

    public bool SecretDelete(string id)
    {
        if (fixture)
            lock (memory) { memory.Remove(id); return true; }
        return Credentials.Delete(id);
    }
}
