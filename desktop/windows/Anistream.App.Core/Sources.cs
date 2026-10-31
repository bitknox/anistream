using Anistream.Engine.Ffi;
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
    /// has no row. The engine's rule: an entry with no row is stepped over and kept, and a kind's
    /// group entry is first spelled out as the sources on screen it stands for.
    /// </summary>
    public static List<string>? Moved(IReadOnlyList<string> order, IReadOnlyList<ProviderRow> rows, string id, int step) =>
        AnistreamFfiMethods.MoveSource([.. order], [.. rows], id, step) is { } moved ? [.. moved] : null;

    /// <summary>
    /// <paramref name="order"/> with the source <paramref name="id"/> dropped at place
    /// <paramref name="index"/> among the rows on screen, for a drag, or null when it has no row.
    /// </summary>
    public static List<string>? Placed(IReadOnlyList<string> order, IReadOnlyList<ProviderRow> rows, string id, int index) =>
        AnistreamFfiMethods.PlaceSource([.. order], [.. rows], id, (uint)index) is { } placed ? [.. placed] : null;

    /// <summary>
    /// Where a source dropped among its section's rows lands among all the rows on screen. The
    /// section keeps the places its rows had; <paramref name="before"/> is the section as it was,
    /// <paramref name="after"/> as it is now, and <paramref name="id"/> the row dropped.
    /// </summary>
    public static int DropIndex(IReadOnlyList<string> everything, IReadOnlyList<string> before, IReadOnlyList<string> after, string id)
    {
        var slots = before.Select(b => everything.ToList().IndexOf(b)).OrderBy(i => i).ToList();
        var to = after.ToList().IndexOf(id);
        return slots.Count == 0 || to < 0 ? everything.ToList().IndexOf(id) : slots[Math.Min(to, slots.Count - 1)];
    }

    /// <summary>The entry in the order that stands for every source of a kind the order does not name by id.</summary>
    static string? GroupEntry(string kind) => kind switch
    {
        "plugin" => "plugins",
        "library" => "library",
        "server" => "servers",
        "catalogue" => "catalogues",
        "demo" => "demo",
        _ => null,
    };

    /// <summary>The rows in the order failover tries them: a source the order does not name stands where its kind's group entry does, or last.</summary>
    public static ProviderRow[] Sorted(IReadOnlyList<ProviderRow> rows, IReadOnlyList<string> order)
    {
        int Rank(ProviderRow row)
        {
            var list = order.ToList();
            var at = list.IndexOf(row.Id);
            if (at >= 0)
                return at;
            if (GroupEntry(row.Kind) is { } group && list.IndexOf(group) is >= 0 and var place)
                return place;
            return row.Kind is "library" or "server" or "catalogue" ? -1 : order.Count;
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
    public bool IsTorrent => Row.Kind == "torrent";
    public bool IsPlugin => Row.Kind == "plugin";

    /// <summary>A newer version its store offers.</summary>
    public string? Update => Listing is { Status: StorePluginStatus.Update } ? Listing.Version : null;

    public IReadOnlyList<string> Hosts => Listing?.Hosts ?? [];

    public static ProviderInfo Of(ProviderRow row, IReadOnlyList<StoreRow> stores)
    {
        var listing = Stores.OriginOf(stores, row.Id);
        switch (row.Kind)
        {
            case "torrent":
                return new(row, T.SourcesTorrents, T.SourcesOriginBuiltIn, null, null);
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
