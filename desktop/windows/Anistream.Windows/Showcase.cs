#if ANISTREAM_TESTING
using Anistream.Engine.Core;
using Anistream.Engine.Model;

namespace Anistream.Windows;

/// <summary>
/// Rows the testing engine has none of, for screenshots of the pages that show them: sources
/// from plugin stores, the stores themselves, a download queue and what is seeding. Invented, as
/// the design boards invent them, and applied only by <c>--showcase</c> on the testing build,
/// which also holds back the engine's own events of the same kinds so these stay on screen.
/// </summary>
static class Showcase
{
    /// <summary>Events of these kinds from the engine are dropped while a showcase is up.</summary>
    public static bool Holds(Event e) => e is Event.Providers or Event.Stores or Event.Downloads or Event.Seeding or Event.StorePreview;

    public static Event[] Events() =>
    [
        new Event.Providers(Providers),
        new Event.Stores(StoreRows),
        new Event.Downloads(Downloads),
        new Event.Seeding(Seeds),
    ];

    /// <summary>
    /// A newer release, as the engine's check names one, with both Windows installers; or, from
    /// <c>--update-from=&lt;url&gt;,&lt;sha256&gt;,&lt;size&gt;,&lt;version&gt;</c>, one with that installer for both.
    /// </summary>
    public static Release Release(string? from = null)
    {
        if (from?.Split(',') is [var url, var sha, var bytes, var version])
        {
            var asset = new ReleaseAsset(Name: Path.GetFileName(new Uri(url).AbsolutePath), Url: url, Sha256: sha, Size: ulong.Parse(bytes, System.Globalization.CultureInfo.InvariantCulture));
            return new Release(
                Version: version, PublishedAt: 1_792_900_000, PageUrl: "https://releases.example/v" + version, Notes: "",
                Macos: null, Ios: null, Android: null, WindowsX64: asset, WindowsArm64: asset, LinuxX8664: null, LinuxAarch64: null);
        }
        static ReleaseAsset File(string arch, ulong size) => new(
            Name: $"anistream-2.1.0-windows-{arch}-setup.exe",
            Url: $"https://releases.example/v2.1.0/anistream-2.1.0-windows-{arch}-setup.exe",
            Sha256: string.Concat(Enumerable.Repeat("5e", 32)),
            Size: size);
        return new Release(
            Version: "2.1.0", PublishedAt: 1_792_900_000, PageUrl: "https://releases.example/v2.1.0",
            Notes: "Episodes open sooner after a pick, and the calendar marks the ones already watched.",
            Macos: null, Ios: null, Android: null,
            WindowsX64: File("x64", 71_400_000), WindowsArm64: File("arm64", 68_900_000),
            LinuxX8664: null, LinuxAarch64: null);
    }

    /// <summary>What a pasted invite leads to, in place of asking a store that does not exist.</summary>
    public static Event Preview(string link) => new Event.StorePreview(new InvitePreview.Store(new InvitedStore(
        Link: link, Name: "Hotaru Plugins", Url: "https://plugins.hotaru.example", Host: "plugins.hotaru.example",
        Fingerprint: "9f3c 1a2b 77de 0c41", AlreadyJoined: false, Owners: ["Ren"],
        Plugins: StoreRows[0].Plugins, Open: false, Anonymous: false, DeviceLabel: "Computer")));

    /// <summary>
    /// What each source has of a title, for the source picker's shots: one with all of it, one
    /// with part, one that failed, one switched off and a torrent source with a batch and singles.
    /// <paramref name="show"/> says which is pinned: source-picker-pinned, or source-picker-releases
    /// for a release within the torrent source.
    /// </summary>
    public static SourceOption[] PickerOptions(string show)
    {
        SourceCandidate Release(string id, string title, uint quality, uint seeders, string size) =>
            new(id, "torrent", title, quality, size, $"{seeders} seeders");
        SourceCandidate[] releases =
        [
            Release("batch-1080", "Hoshikuzu no Tabibito (Batch) 1080p", 1080, 84, "14.2 GiB"),
            Release("batch-720", "Hoshikuzu no Tabibito (Batch) 720p", 720, 31, "6.8 GiB"),
            Release("single-1080", "Hoshikuzu no Tabibito - 01 1080p", 1080, 12, "1.1 GiB"),
        ];
        var pinned = show switch
        {
            "source-picker-pinned" => "mukashi-archive",
            "source-picker-releases" => "torrent",
            _ => null,
        };
        SourceOption Option(string id, string name, uint covered, SourceNote? note = null, SourceCandidate[]? with = null) =>
            new(id, name, covered, 13, id == pinned, note, with ?? [], id == pinned && with is not null ? "batch-720" : null);
        var torrent = Option("torrent", "Torrent source", 6, with: show == "source-picker-pinned" ? null : releases);
        return show switch
        {
            // Every state a source can be in: all of it, part of it, failed, off.
            "source-picker-pinned" =>
            [
                Option("hotaru-open-archive", "Hotaru Open Archive", 13),
                Option("mukashi-archive", "Mukashi Archive", 9),
                torrent,
                Option("machi-library-films", "Machi Library Films", 0, new SourceNote.Failed("the site did not answer in 15 s.")),
                Option("sora-festival", "Sora Animation Festival", 0, new SourceNote.Off()),
            ],
            _ =>
            [
                Option("hotaru-open-archive", "Hotaru Open Archive", 13),
                Option("mukashi-archive", "Mukashi Archive", 9),
                torrent,
                Option("machi-library-films", "Machi Library Films", 0, new SourceNote.Failed("the site did not answer in 15 s.")),
            ],
        };
    }

    /// <summary>
    /// A title's table as if the sources fell short of it: the next episode, the one after and the
    /// last three are listed by no source.
    /// </summary>
    public static Event.Episodes Unsourced(Event.Episodes table, uint next)
    {
        var last = table.Rows.Length;
        return table with { Rows = [.. table.Rows.Select((row, at) =>
            uint.TryParse(row.Number, out var n) && (n == next || n == next + 1 || at >= last - 3) ? row with { Unsourced = true } : row)] };
    }

    static ProviderRow Plugin(string id, string name, string version, string[] hosts, ProviderOrigin origin,
        string state = "ready", string? error = null, ProviderUpdate? update = null, ulong? latency = 200) =>
        new(Id: id, Kind: "plugin", State: state, Usable: state != "down", Name: name, Origin: origin,
            LatencyMs: state == "down" ? null : latency, LastError: error, HeldBack: false, Version: version,
            Hosts: hosts, Disabled: false, Update: update);

    static readonly ProviderOrigin Hotaru = new ProviderOrigin.Store("hotaru", "Hotaru Plugins");

    static readonly ProviderRow[] Providers =
    [
        Plugin("hotaru-open-archive", "Hotaru Open Archive", "2.3.0",
            ["open-archive.hotaru.example", "media.open-archive.hotaru.example"], Hotaru, latency: 180),
        Plugin("mukashi-archive", "Mukashi Archive", "1.4.0", ["mukashi-archive.example"], Hotaru, latency: 320,
            update: new ProviderUpdate("1.5.0", ["media2.mukashi-archive.example"])),
        Plugin("machi-library-films", "Machi Library Films", "0.3.0", ["films.machi-library.example"],
            new ProviderOrigin.ByHand(), state: "down", error: "the site did not answer in 15 s."),
        new ProviderRow(Id: "torrent", Kind: "torrent", State: "ready", Usable: true, Name: "Torrent source",
            Origin: new ProviderOrigin.BuiltIn(), Hosts: [], Version: null),
    ];

    static StorePluginRow Listing(string id, string name, string version, string[] hosts, StorePluginStatus status) =>
        new(id, name, version, status, hosts);

    static readonly StoreRow[] StoreRows =
    [
        new(Id: "hotaru", Name: "Hotaru Plugins", Url: "https://plugins.hotaru.example", Host: "plugins.hotaru.example",
            Fingerprint: "9f3c 1a2b 77de 0c41", Member: "Mika", State: new StoreState.Ready(), Owner: false, Owners: ["Ren"],
            Plugins:
            [
                Listing("hotaru-open-archive", "Hotaru Open Archive", "2.3.0",
                    ["open-archive.hotaru.example", "media.open-archive.hotaru.example"], new StorePluginStatus.Installed()),
                Listing("mukashi-archive", "Mukashi Archive", "1.5.0",
                    ["mukashi-archive.example", "media2.mukashi-archive.example"],
                    new StorePluginStatus.Update("1.4.0", ["media2.mukashi-archive.example"])),
                Listing("atelier-hotaru-shorts", "Atelier Hotaru Shorts", "1.0.2",
                    ["shorts.atelier-hotaru.example"], new StorePluginStatus.Available()),
            ]),
        new(Id: "mika", Name: "Mika's plugins", Url: "https://plugins.mika.example", Host: "plugins.mika.example",
            Fingerprint: "2b7e 90c4 d1f3 5a68", Member: "Mika", State: new StoreState.Ready(), Owner: true, Owners: ["Mika"],
            Plugins:
            [
                Listing("machi-library-films", "Machi Library Films", "0.3.0",
                    ["films.machi-library.example"], new StorePluginStatus.AddedByHand()),
            ]),
        new(Id: "tsuki", Name: "Tsuki Club", Url: "https://tsuki-club.example", Host: "tsuki-club.example",
            Fingerprint: "e815 3c6a 0b9d 42f7", Member: "Mika", State: new StoreState.Offline("the store did not answer"),
            Owner: false, Plugins:
            [
                Listing("sora-festival", "Sora Animation Festival", "1.1.0",
                    ["archive.sora-festival.example"], new StorePluginStatus.Available()),
            ]),
    ];

    static DownloadRow Row(long id, uint title, string name, string episode, DownloadState state, ulong done, ulong total,
        string? path = null, string? error = null) =>
        new(id, title, name, episode, state, total > 0 ? (double)done / total : 0, done, total, error, path);

    static readonly DownloadRow[] Downloads =
    [
        Row(1, 900001, "Hoshikuzu no Tabibito", "8", DownloadState.Done, 1_400_000_000, 1_400_000_000, path: @"C:\clip-8.mkv"),
        Row(2, 900001, "Hoshikuzu no Tabibito", "9", DownloadState.Active, 300_000_000, 1_400_000_000),
        Row(3, 900001, "Hoshikuzu no Tabibito", "10", DownloadState.Queued, 0, 0),
        Row(5, 900002, "Kumo no Ue no Toshokan", "5", DownloadState.Paused, 210_000_000, 480_000_000),
        Row(6, 900002, "Kumo no Ue no Toshokan", "6", DownloadState.Queued, 0, 0),
        Row(7, 900005, "Hagane no Niwashi", "1", DownloadState.Done, 460_000_000, 460_000_000, path: @"C:\clip-1.mkv"),
        Row(8, 900005, "Hagane no Niwashi", "4", DownloadState.Failed, 130_000_000, 470_000_000,
            error: "the source stopped responding at 130 MB"),
    ];

    static readonly SeedRow[] Seeds =
    [
        new("Hoshikuzu no Tabibito - 06.mkv", 90_000_000, 0.2, 1, 64_800),
        new("Hoshikuzu no Tabibito - 05.mkv", 360_000_000, 0.8, 1, 21_600),
    ];
}
#endif
