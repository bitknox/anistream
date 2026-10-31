using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// The order failover tries the sources in, and how a move writes it. Ports of
/// <c>anistream_model::sources::order_with_source_moved</c>, as the Apple and Android apps keep them.
/// </summary>
public static class SourceOrder
{
    /// <summary>
    /// <paramref name="order"/> with the source <paramref name="id"/> moved one place up the chain
    /// (<paramref name="step"/> below zero) or down it, or null when it is already at that end or
    /// has no row. An entry with no row (<c>plugins</c>, torrents while they are off, a plugin
    /// named but not installed) is stepped over and kept; <c>plugins</c> is first spelled out as
    /// the plugins on screen and stays right after them.
    /// </summary>
    public static List<string>? Moved(IReadOnlyList<string> order, IReadOnlyList<ProviderRow> rows, string id, int step)
    {
        var result = order.ToList();
        var unnamed = rows.Where(r => r.Kind == "plugin" && !result.Contains(r.Id)).Select(r => r.Id).ToList();
        var at = result.IndexOf("plugins");
        result.InsertRange(at < 0 ? result.Count : at, unnamed);

        var from = result.IndexOf(id);
        if (from < 0)
            return null;
        bool Shown(string entry) => entry != "plugins" && rows.Any(r => r.Id == entry);
        int to;
        if (step < 0)
        {
            to = result.Take(from).ToList().FindLastIndex(Shown);
            if (to < 0)
                return null;
        }
        else
        {
            var after = result.Skip(from + 1).ToList().FindIndex(Shown);
            if (after < 0)
                return null;
            to = from + 1 + after;
        }
        var entry = result[from];
        result.RemoveAt(from);
        result.Insert(to, entry);
        return result;
    }

    /// <summary>The rows in the order failover tries them: a plugin the order does not name stands where <c>plugins</c> does, or last.</summary>
    public static ProviderRow[] Sorted(IReadOnlyList<ProviderRow> rows, IReadOnlyList<string> order)
    {
        int Rank(ProviderRow row)
        {
            var at = order.ToList().IndexOf(row.Id);
            if (at >= 0)
                return at;
            if (row.Kind == "plugin")
            {
                var plugins = order.ToList().IndexOf("plugins");
                if (plugins >= 0)
                    return plugins;
            }
            return order.Count;
        }
        return [.. rows.Select((row, i) => (row, i)).OrderBy(x => Rank(x.row)).ThenBy(x => x.i).Select(x => x.row)];
    }
}

/// <summary>How a source stands, in the words and the dot the Sources list gives it.</summary>
public enum ProviderStanding
{
    Working,
    Slow,
    Failing,
    HeldBack,
    /// <summary>Switched off by name: kept, and not tried until it is on again.</summary>
    Off,
}

public static class ProviderRows
{
    public static ProviderStanding Standing(this ProviderRow row)
    {
        if (row.Disabled)
            return ProviderStanding.Off;
        if (row.HeldBack)
            return ProviderStanding.HeldBack;
        if (!row.Usable || row.State == "down")
            return ProviderStanding.Failing;
        return row.State == "degraded" ? ProviderStanding.Slow : ProviderStanding.Working;
    }

    /// <summary>The words beside a source's name when it needs saying, or null for one that works.</summary>
    public static string? StandingLine(this ProviderStanding standing) => standing switch
    {
        ProviderStanding.Failing => T.SourcesStatusNotAnswering,
        ProviderStanding.HeldBack => T.SourcesStatusHeldBack,
        ProviderStanding.Off => T.SourceNoteOff,
        ProviderStanding.Slow => T.SourcesStatusSlow,
        _ => null,
    };
}

/// <summary>What the Sources page says of one source, from its row and the stores it came from.</summary>
public sealed record ProviderInfo(ProviderRow Row, string Name, string Origin, StoreRow? Store, StorePluginRow? Listing)
{
    public bool IsPlugin => Row.Kind == "plugin";

    /// <summary>A newer version its store offers.</summary>
    public string? Update => Listing is { Status: StorePluginStatus.Update } ? Listing.Version : null;

    public IReadOnlyList<string> Hosts => Listing?.Hosts ?? [];

    public static ProviderInfo Of(ProviderRow row, IReadOnlyList<StoreRow> stores)
    {
        var listing = Stores.OriginOf(stores, row.Id);
        switch (row.Kind)
        {
            case "remote":
                return new(row, T.SourcesRemote, T.SourcesOriginConfig, null, null);
        }
        var name = listing?.Plugin.Name ?? row.Id;
        var origin = listing is null || listing.Value.Plugin.Status is StorePluginStatus.AddedByHand
            ? T.SourcesOriginByHand
            : listing.Value.Store.Name;
        return new(row, name, origin, listing?.Store, listing?.Plugin);
    }
}

/// <summary>Reading the joined stores.</summary>
public static class Stores
{
    /// <summary>
    /// Where an installed plugin came from: the store that keeps it updated and its row there, or
    /// the first store that lists it as added by hand.
    /// </summary>
    public static (StoreRow Store, StorePluginRow Plugin)? OriginOf(IReadOnlyList<StoreRow> stores, string plugin)
    {
        (StoreRow, StorePluginRow)? byHand = null;
        foreach (var store in stores)
        {
            var row = store.Plugins?.FirstOrDefault(p => p.Id == plugin);
            if (row is null)
                continue;
            switch (row.Status)
            {
                case StorePluginStatus.Installed or StorePluginStatus.Update:
                    return (store, row);
                case StorePluginStatus.AddedByHand:
                    byHand ??= (store, row);
                    break;
            }
        }
        return byHand;
    }

    public static int Updates(StoreRow store) => store.Plugins?.Count(p => p.Status is StorePluginStatus.Update) ?? 0;

    /// <summary>What a plugin may contact, in a line: "Contacts open-archive.example and one more".</summary>
    public static string ContactsLine(StorePluginRow plugin) => plugin.Hosts is { Length: > 0 } hosts
        ? hosts.Length > 1 ? T.SourcesContactsLine(hosts[0], hosts.Length - 1) : T.SourcesContactsOne(hosts[0])
        : T.SourcesContactsNone;

    /// <summary>The version installed here: the one an update replaces, or the store's own.</summary>
    public static string InstalledVersion(StorePluginRow plugin) =>
        plugin.Status is StorePluginStatus.Update u ? u.From : plugin.Version;

    /// <summary>Where a plugin stands on this device in a word or two, when it is here at all.</summary>
    public static string? Standing(StorePluginRow plugin) => plugin.Status switch
    {
        StorePluginStatus.Installed => T.StoresPluginStateInstalled,
        StorePluginStatus.Update u => T.StoresPluginUpdateLine(u.From, plugin.Version),
        StorePluginStatus.AddedByHand => T.SourcesOriginByHand,
        StorePluginStatus.OtherStore o => T.StoresPluginFrom(o.V1),
        _ => null,
    };

    /// <summary>Who runs a store and where it answers: "Run by Ren · plugins.example", or the host alone.</summary>
    public static string RunBy(string[]? owners, string host) =>
        owners is { Length: > 0 } ? T.StoresRunBy(string.Join(", ", owners), host) : host;

    /// <summary>Who this device joined as: a name, or none for one that joined with the store's open link.</summary>
    public static string JoinedAs(StoreRow store) => store.Anonymous ? T.StoresJoinedNameless : T.StoresDeviceJoinedAs(store.Member);

    /// <summary>An invite link inside what was pasted: the https invite, or one wrapped in the app's scheme.</summary>
    public static string? InviteIn(string pasted)
    {
        var text = pasted.Trim();
        if (!Uri.TryCreate(text, UriKind.Absolute, out var uri))
            return null;
        switch (uri.Scheme.ToLowerInvariant())
        {
            case "https":
                return uri.AbsolutePath.Contains("/join/") ? text : null;
            case "anistream":
                if (uri.Host == "join")
                {
                    var link = uri.Query.TrimStart('?').Split('&')
                        .Select(p => p.Split('=', 2))
                        .FirstOrDefault(p => p[0] == "link" && p.Length == 2);
                    return link is null ? null : InviteIn(Uri.UnescapeDataString(link[1]));
                }
                return uri.AbsolutePath.Contains("/join/") ? "https://" + text["anistream://".Length..] : null;
            default:
                return null;
        }
    }
}
