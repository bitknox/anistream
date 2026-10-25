using System.Runtime.InteropServices.WindowsRuntime;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage.Streams;

namespace Anistream.Windows;

/// <summary>
/// Covers and banners, fetched by the engine (its cache, its network rules) and decoded here
/// once each. A picture asked for before its bytes arrive is shown when they do.
/// </summary>
sealed class Art(Func<Command, ulong?> send)
{
    readonly Dictionary<string, ImageSource> ready = [];
    readonly Dictionary<string, List<Action<ImageSource>>> waiting = [];
    readonly Dictionary<string, List<Action<byte[]>>> raw = [];

    /// <summary>The art's own bytes, once they are here: for a picture outside the window, such as a notification's.</summary>
    public void Bytes(string url, Action<byte[]> got)
    {
        if (raw.TryGetValue(url, out var list))
        {
            list.Add(got);
            return;
        }
        raw[url] = [got];
        send(new Command.FetchArtwork(url));
    }

    /// <summary>Show <paramref name="url"/>'s art through <paramref name="show"/>: now, or once it is here.</summary>
    public void Bind(string? url, Action<ImageSource> show)
    {
        if (string.IsNullOrEmpty(url))
            return;
        if (ready.TryGetValue(url, out var image))
        {
            show(image);
            return;
        }
        if (waiting.TryGetValue(url, out var list))
        {
            list.Add(show);
            return;
        }
        waiting[url] = [show];
        send(new Command.FetchArtwork(url));
    }

    public async void Arrived(string url, byte[] bytes)
    {
        if (raw.Remove(url, out var gots))
            foreach (var got in gots)
                got(bytes);
        if (!waiting.Remove(url, out var shows))
            shows = [];
        try
        {
            using var stream = new InMemoryRandomAccessStream();
            await stream.WriteAsync(bytes.AsBuffer());
            stream.Seek(0);
            var bitmap = new BitmapImage();
            await bitmap.SetSourceAsync(stream);
            ready[url] = bitmap;
            foreach (var show in shows)
                show(bitmap);
        }
        catch (Exception)
        {
            // Not an image this can decode: the placeholder stays.
        }
    }

    /// <summary>Asked again the next time a page shows it.</summary>
    public void Failed(string url)
    {
        waiting.Remove(url);
        raw.Remove(url);
    }

    public void Take(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.ArtworkReady a: Arrived(a.Url, a.Bytes); break;
                case Event.ImageFailed f: Failed(f.Url); break;
            }
        }
    }
}
