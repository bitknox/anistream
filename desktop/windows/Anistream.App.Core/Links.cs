namespace Anistream.App;

/// <summary>
/// The <c>anistream://</c> links the app is opened with, as the Mac answers them: a watch
/// party's invite, a plugin store's invite, as the https link it wraps, and the widgets' destinations, a title, an episode
/// to play, the Calendar, Settings › Accounts and a finished download.
/// </summary>
public abstract record Link
{
    public const string Scheme = "anistream";

    /// <summary>A store's invite: the https link itself, <c>https://…/join/{code}#{key}</c>, with <c>?open</c> for an open link.</summary>
    public sealed record Invite(string Https) : Link;

    /// <summary>A title's page: <c>anistream://title/900001</c>.</summary>
    public sealed record Title(uint Id) : Link;

    /// <summary>An episode playing, from its title's page: <c>anistream://play/900001/7</c>.</summary>
    public sealed record Play(uint Id, uint Episode) : Link;

    /// <summary>The Calendar: <c>anistream://calendar</c>.</summary>
    public sealed record Calendar : Link;

    /// <summary>Settings › Accounts: <c>anistream://settings/accounts</c>.</summary>
    public sealed record Accounts : Link;

    /// <summary>A finished download playing, or Downloads while it is not there: <c>anistream://download/42</c>.</summary>
    public sealed record Download(long Id) : Link;

    /// <summary>
    /// A watch party's invite, <c>anistream://party?server=…&amp;room=…&amp;title=…&amp;episode=…</c>, as
    /// it came: the engine reads it (<c>parse_party_invite</c>), so every app agrees on what it says.
    /// </summary>
    public sealed record Party(string Text) : Link;

    /// <summary>
    /// What <paramref name="text"/> opens, or null when it is not a link the app answers. A store
    /// invite comes as <c>anistream://join?link=&lt;the https link&gt;</c>, or as the https link
    /// with the app's scheme in front of the host, <c>anistream://plugins.example/join/{code}#{key}</c>.
    /// A sign-in coming back (<c>anistream://auth/…</c>) is not one: Windows signs in over loopback.
    /// </summary>
    public static Link? Parse(string? text)
    {
        text = text?.Trim().Trim('"');
        if (string.IsNullOrEmpty(text))
            return null;
        if (text.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
            return InviteIn(text) is { } https ? new Invite(https) : null;
        var prefix = Scheme + "://";
        if (!text.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            return null;
        var rest = text[prefix.Length..];
        var hostEnd = rest.IndexOfAny(['/', '?', '#']);
        var host = (hostEnd < 0 ? rest : rest[..hostEnd]).ToLowerInvariant();
        var tail = hostEnd < 0 ? "" : rest[hostEnd..];
        var path = tail.Split(['?', '#'], 2)[0];
        var parts = path.Split('/', StringSplitOptions.RemoveEmptyEntries);
        switch (host)
        {
            case "join":
                return Query(tail, "link") is { } wrapped && InviteIn(wrapped) is { } inner ? new Invite(inner) : null;
            case "auth":
                return null;
            case "party":
                return new Party(text);
            case "title" when parts is [var id] && uint.TryParse(id, out var t) && t > 0:
                return new Title(t);
            case "play" when parts is [var id, var ep] && uint.TryParse(id, out var p) && p > 0
                                                       && uint.TryParse(ep, out var e) && e > 0:
                return new Play(p, e);
            case "calendar" when parts.Length == 0:
                return new Calendar();
            case "settings" when parts is ["accounts"]:
                return new Accounts();
            case "download" when parts is [var id] && long.TryParse(id, out var d) && d > 0:
                return new Download(d);
            case "title" or "play" or "calendar" or "settings" or "download" or "":
                return null;
        }
        // The store's https link with anistream:// in front.
        return InviteIn("https://" + rest) is { } invite ? new Invite(invite) : null;
    }

    /// <summary>
    /// The link a launch carries: the first argument with the app's scheme, as the installer's
    /// <c>anistream.exe "%1"</c> hands it over, whole or as a command line.
    /// </summary>
    public static string? InArguments(IEnumerable<string> args) =>
        args.Select(a => a.Trim().Trim('"'))
            .FirstOrDefault(a => a.StartsWith(Scheme + ":", StringComparison.OrdinalIgnoreCase));

    /// <summary>The same, from a whole command line, as a redirected launch brings it.</summary>
    public static string? InCommandLine(string? line)
    {
        if (string.IsNullOrEmpty(line))
            return null;
        var at = line.IndexOf(Scheme + "://", StringComparison.OrdinalIgnoreCase);
        if (at < 0)
            return null;
        var quoted = at > 0 && line[at - 1] == '"';
        var end = quoted ? line.IndexOf('"', at) : line.IndexOf(' ', at);
        return end < 0 ? line[at..] : line[at..end];
    }

    /// <summary>The https invite <paramref name="text"/> is, as the store makes it: a path with <c>/join/</c> in it.</summary>
    static string? InviteIn(string text) =>
        Uri.TryCreate(text, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps
            && uri.AbsolutePath.Contains("/join/", StringComparison.Ordinal) && uri.Host.Length > 0
            ? text
            : null;

    static string? Query(string tail, string name)
    {
        var query = tail.Split('?', 2) is [_, var q] ? q.Split('#', 2)[0] : "";
        foreach (var pair in query.Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            var (key, value) = pair.Split('=', 2) is [var k, var v] ? (k, v) : (pair, "");
            if (key == name)
                return Uri.UnescapeDataString(value.Replace('+', ' '));
        }
        return null;
    }
}
