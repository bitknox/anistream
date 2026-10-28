using System.Xml.Linq;
using Anistream.App;
using Microsoft.Win32;
using Windows.Data.Xml.Dom;
using Windows.UI.Notifications;

namespace Anistream.Windows;

/// <summary>
/// Windows notifications: a new episode out, with Play and Open, a download finished, and the
/// engine's own notices while the window is away. The app is unpackaged and self-contained, where
/// the App SDK's <c>AppNotificationManager.Register</c> fails for want of its singleton package, so
/// this registers the app's id under the user's classes and posts through the system's own
/// <see cref="ToastNotificationManager"/>, as unpackaged desktop apps do. A click reaches the
/// running app; the notifications are cleared when it quits, so none is left that could not.
/// The picture is the show's banner, written to a file first, since a notification shows no web
/// image for an unpackaged app.
/// </summary>
sealed class Toasts
{
    /// <summary>The app's id for its notifications, and the name and icon Windows shows on them.</summary>
    const string AppId = "Anistream.Windows";

    readonly string artDir;
    readonly Art art;
    readonly ToastNotifier notifier;
    /// <summary>What a notification's button or body asks: the action, the show, the episode.</summary>
    readonly Action<string, uint, string?> invoked;

    Toasts(string artDir, Art art, ToastNotifier notifier, Action<string, uint, string?> invoked)
    {
        this.artDir = artDir;
        this.art = art;
        this.notifier = notifier;
        this.invoked = invoked;
    }

    /// <summary>Register the app's id and open its notifier, or null where Windows will not have it.</summary>
    public static Toasts? Start(string artDir, Art art, Action<string, uint, string?> invoked)
    {
        try
        {
            using (var key = Registry.CurrentUser.CreateSubKey($@"Software\Classes\AppUserModelId\{AppId}"))
            {
                key.SetValue("DisplayName", T.AppName);
                key.SetValue("IconUri", Path.Combine(AppContext.BaseDirectory, "Assets", "anistream-256.png"));
            }
            var notifier = ToastNotificationManager.CreateToastNotifier(AppId);
            return new Toasts(artDir, art, notifier, invoked);
        }
        catch (Exception e)
        {
            Note($"not registered: {e.GetType().Name} 0x{e.HResult:X8} {e.Message}");
            return null;
        }
    }

    /// <summary>Post <paramref name="notice"/>, with its picture when the art comes within two seconds.</summary>
    public void Post(SystemNotice notice)
    {
        if (notice.Art is not { Length: > 0 } url)
        {
            Show(notice, null);
            return;
        }
        var shown = 0;
        var late = new System.Threading.Timer(_ =>
        {
            if (Interlocked.Exchange(ref shown, 1) == 0)
                Show(notice, null);
        }, null, 2000, Timeout.Infinite);
        art.Bytes(url, bytes =>
        {
            if (Interlocked.Exchange(ref shown, 1) != 0)
                return;
            late.Dispose();
            Show(notice, Write(url, bytes));
        });
    }

    string? Write(string url, byte[] bytes)
    {
        // A notification shows PNG, JPEG and GIF, by the file's extension.
        var kind = bytes switch
        {
            [0x89, 0x50, 0x4E, 0x47, ..] => ".png",
            [0xFF, 0xD8, ..] => ".jpg",
            [0x47, 0x49, 0x46, ..] => ".gif",
            _ => null,
        };
        if (kind is null)
            return null;
        try
        {
            Directory.CreateDirectory(artDir);
            var file = Path.Combine(artDir, Convert.ToHexString(System.Security.Cryptography.SHA1.HashData(System.Text.Encoding.UTF8.GetBytes(url))) + kind);
            if (!File.Exists(file))
                File.WriteAllBytes(file, bytes);
            return file;
        }
        catch (IOException)
        {
            return null;
        }
    }

    /// <summary>The notification's XML: the show, the line, the banner as its hero, then Play and Open.</summary>
    public static string Xml(SystemNotice notice, string? picture)
    {
        static string Args(string action, uint show, string? episode = null) =>
            $"action={action}&show={show}" + (episode is null ? "" : $"&episode={Uri.EscapeDataString(episode)}");
        var binding = new XElement("binding", new XAttribute("template", "ToastGeneric"),
            new XElement("text", notice.Title),
            new XElement("text", notice.Body));
        if (picture is not null)
            binding.Add(new XElement("image", new XAttribute("placement", "hero"), new XAttribute("src", new Uri(picture).AbsoluteUri)));
        var actions = new XElement("actions");
        // A notice about no show, such as the engine's digest, opens the window alone.
        if (notice.Show != 0)
        {
            if (notice.Play is { } episode)
                actions.Add(new XElement("action", new XAttribute("content", T.ActionPlay), new XAttribute("arguments", Args("play", notice.Show, episode)), new XAttribute("activationType", "foreground")));
            actions.Add(new XElement("action", new XAttribute("content", T.ActionOpen), new XAttribute("arguments", Args("open", notice.Show)), new XAttribute("activationType", "foreground")));
        }
        var toast = new XElement("toast", new XAttribute("launch", Args("open", notice.Show)), new XElement("visual", binding));
        if (actions.HasElements)
            toast.Add(actions);
        return toast.ToString(SaveOptions.DisableFormatting);
    }

    /// <summary>A notification's arguments, "action=play&amp;show=900001&amp;episode=8", as a dictionary.</summary>
    public static Dictionary<string, string> Arguments(string text) =>
        text.Split('&', StringSplitOptions.RemoveEmptyEntries)
            .Select(pair => pair.Split('=', 2))
            .Where(pair => pair.Length == 2)
            .GroupBy(pair => pair[0])
            .ToDictionary(g => g.Key, g => Uri.UnescapeDataString(g.First()[1]));

    void Show(SystemNotice notice, string? picture)
    {
        try
        {
            var xml = new XmlDocument();
            xml.LoadXml(Xml(notice, picture));
            var toast = new ToastNotification(xml)
            {
                Tag = notice.Tag.Length <= 64 ? notice.Tag : notice.Tag[..64],
                Group = "anistream",
            };
            toast.Activated += (_, args) =>
            {
                if (args is ToastActivatedEventArgs a && Notices.Read(Arguments(a.Arguments)) is { } asked)
                    invoked(asked.Action, asked.Show, asked.Episode);
            };
            notifier.Show(toast);
            Note($"posted {notice.Tag}{(picture is null ? "" : " with its picture")}");
        }
        catch (Exception e)
        {
            // Notifications turned off for the app, or the system has none: nothing to say.
            Note($"not posted {notice.Tag}: {e.GetType().Name} 0x{e.HResult:X8} {e.Message}");
        }
    }

    /// <summary>A line in the temp folder's log, beside the crash log, for a notification that never showed.</summary>
    static void Note(string line)
    {
        try
        {
            var dir = Path.Combine(Path.GetTempPath(), "anistream-windows");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "notifications.txt"), $"{DateTime.Now:O} {line}\n");
        }
        catch (IOException)
        {
        }
    }

    /// <summary>The app is quitting: its notifications go too, since a click on one would reach nothing.</summary>
    public void Stop()
    {
        try
        {
            ToastNotificationManager.History.Clear(AppId);
        }
        catch (Exception)
        {
        }
    }
}
