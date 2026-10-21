// Rows the testing engine has none of, for screenshots of the screens that show them: providers
// from plugin stores, the stores themselves, a download queue, the VPN guard's verdict. Invented,
// as the design boards invent them, and only ever on the testing engine.

import AnistreamKit
import Foundation

#if FIXTURES

/// What a screenshot can ask the Fixtures app to show beside the testing engine's own data, with
/// `-AnistreamShowcase <name>`: `sources`, `stores`, `downloads`, `invite`, or `all`.
public enum Showcase: String, CaseIterable, Sendable {
    case sources, stores, downloads, invite, all

    /// The one the launch arguments name.
    public static func from(arguments: [String]) -> Showcase? {
        guard let index = arguments.firstIndex(of: "-AnistreamShowcase"), arguments.indices.contains(index + 1)
        else { return nil }
        return Showcase(rawValue: arguments[index + 1])
    }

    /// The events to apply, and which of the engine's own to set aside so they stay on screen.
    func events(settings: SettingsView?) -> (events: [Event], held: Set<String>) {
        var events: [Event] = []
        var held: Set<String> = []
        let parts: [Showcase] = self == .all ? [.sources, .stores, .downloads, .invite] : [self]
        for part in parts {
            switch part {
            case .sources:
                events += [.providers(Self.providerRows), .vpn(.protected(exit: "Lantern Networks"))]
                held.formUnion(["providers", "vpn"])
                if let settings {
                    events.append(.settings(Self.torrentsOn(settings)))
                    held.insert("settings")
                }
                // Providers names each plugin by its store's listing.
                events.append(.stores(Self.storeRows))
                held.insert("stores")
            case .stores:
                events.append(.stores(Self.storeRows))
                held.insert("stores")
            case .downloads:
                events += [.downloads(Self.downloadRows), .seeding(Self.seedRows)]
                held.formUnion(["downloads", "seeding"])
            case .invite:
                events.append(.storePreview(.store(InvitedStore(
                    link: "https://plugins.hotaru.example/join/fixture#key", name: "Hotaru Plugins",
                    url: "https://plugins.hotaru.example", fingerprint: "9f3c 1a2b 77de 0c41", alreadyJoined: false
                ))))
                held.insert("storePreview")
            case .all:
                break
            }
        }
        return (events, held)
    }

    static let providerRows: [ProviderRow] = [
        ProviderRow(id: "hotaru-open-archive", kind: "plugin", state: "ready", latencyMs: 180, lastError: nil, usable: true, heldBack: false),
        ProviderRow(id: "mukashi-archive", kind: "plugin", state: "ready", latencyMs: 320, lastError: nil, usable: true, heldBack: false),
        ProviderRow(id: "machi-library-films", kind: "plugin", state: "down", latencyMs: nil,
                    lastError: "the site did not answer in 15 s.", usable: false, heldBack: false),
        ProviderRow(id: "torrent", kind: "torrent", state: "ready", latencyMs: nil, lastError: nil, usable: true, heldBack: false),
    ]

    static let storeRows: [StoreRow] = [
        StoreRow(id: "hotaru", name: "Hotaru Plugins", url: "https://plugins.hotaru.example",
                 fingerprint: "9f3c 1a2b 77de 0c41", member: "Mika", owner: false, state: .ready, plugins: [
                    StorePluginRow(id: "hotaru-open-archive", name: "Hotaru Open Archive", version: "2.3.0",
                                   hosts: ["open-archive.hotaru.example", "media.open-archive.hotaru.example"], status: .installed),
                    StorePluginRow(id: "mukashi-archive", name: "Mukashi Archive", version: "1.5.0",
                                   hosts: ["mukashi-archive.example", "media2.mukashi-archive.example"],
                                   status: .update(from: "1.4.0", newHosts: ["media2.mukashi-archive.example"])),
                    StorePluginRow(id: "kobunko-film-library", name: "Kōbunko Film Library", version: "0.9.1",
                                   hosts: ["films.kobunko.example"], status: .installed),
                    StorePluginRow(id: "atelier-hotaru-shorts", name: "Atelier Hotaru Shorts", version: "1.0.2",
                                   hosts: ["shorts.atelier-hotaru.example"], status: .available),
                 ]),
        StoreRow(id: "mika", name: "Mika's plugins", url: "https://plugins.mika.example",
                 fingerprint: "2b7e 90c4 d1f3 5a68", member: "Mika", owner: true, state: .ready, plugins: [
                    StorePluginRow(id: "machi-library-films", name: "Machi Library Films", version: "0.3.0",
                                   hosts: ["films.machi-library.example"], status: .addedByHand),
                    StorePluginRow(id: "kobunko-film-library", name: "Kōbunko Film Library", version: "0.9.1",
                                   hosts: ["films.kobunko.example"], status: .otherStore("Hotaru Plugins")),
                 ]),
        StoreRow(id: "tsuki", name: "Tsuki Club", url: "https://tsuki-club.example",
                 fingerprint: "e815 3c6a 0b9d 42f7", member: "Mika", owner: false,
                 state: .offline("the store did not answer"), plugins: [
                    StorePluginRow(id: "sora-festival", name: "Sora Animation Festival", version: "1.1.0",
                                   hosts: ["archive.sora-festival.example"], status: .available),
                 ]),
        StoreRow(id: "ame", name: "Ame Plugins", url: "https://plugins.ame.example",
                 fingerprint: "71aa c3e0 58b2 9d14", member: "Mika", owner: false, state: .revoked, plugins: [
                    StorePluginRow(id: "ame-shorts", name: "Ame Shorts", version: "0.2.0",
                                   hosts: ["shorts.ame.example"], status: .available),
                 ]),
    ]

    static let downloadRows: [DownloadRow] = [
        row(1, 900_001, "Hoshikuzu no Tabibito", "8", .done, 1_400_000_000, of: 1_400_000_000, path: "/tmp/anistream-fixture-8.mkv"),
        row(2, 900_001, "Hoshikuzu no Tabibito", "9", .active, 300_000_000, of: 1_400_000_000),
        row(3, 900_001, "Hoshikuzu no Tabibito", "10", .queued, 0, of: 0),
        row(4, 900_006, "Natsukaze Relay", "9", .active, 0, of: 0),
        row(5, 900_002, "Kumo no Ue no Toshokan", "5", .paused, 210_000_000, of: 480_000_000),
        row(6, 900_002, "Kumo no Ue no Toshokan", "6", .queued, 0, of: 0),
        row(7, 900_005, "Hagane no Niwashi", "1", .done, 460_000_000, of: 460_000_000, path: "/tmp/anistream-fixture-3.mkv"),
        row(8, 900_005, "Hagane no Niwashi", "4", .failed, 130_000_000, of: 470_000_000,
            error: "the source stopped responding at 130 MB"),
    ]

    static let seedRows: [SeedRow] = [
        SeedRow(name: "Hoshikuzu no Tabibito - 06.mkv", uploadedBytes: 90_000_000, ratio: 0.2, goal: 1, leftSecs: 64_800),
        SeedRow(name: "Hoshikuzu no Tabibito - 05.mkv", uploadedBytes: 360_000_000, ratio: 0.8, goal: 1, leftSecs: 21_600),
    ]

    private static func row(_ id: Int64, _ title: AnilistId, _ name: String, _ episode: String, _ state: DownloadState,
                            _ done: UInt64, of total: UInt64, path: String? = nil, error: String? = nil) -> DownloadRow {
        DownloadRow(id: id, anilistId: title, title: name, episode: episode, state: state,
                    fraction: total > 0 ? Double(done) / Double(total) : 0, downloaded: done, total: total,
                    error: error, path: path)
    }

    /// The settings with torrents on, behind a VPN on this device, as Sources shows them so.
    static func torrentsOn(_ view: SettingsView) -> SettingsView {
        var view = view
        view.rows = view.rows.map { row in
            switch row.key {
            case .torrents: SettingRow(key: .torrents, value: "on", change: .steps(back: true, forward: false))
            case .vpnMode: SettingRow(key: .vpnMode, value: "external", change: .fixed)
            default: row
            }
        }
        view.sources.torrent = true
        return view
    }
}

#endif

extension Event {
    /// The kind of event, as a showcase sets the engine's own aside by.
    var showcaseKind: String? {
        switch self {
        case .providers: "providers"
        case .vpn: "vpn"
        case .settings: "settings"
        case .stores: "stores"
        case .downloads: "downloads"
        case .seeding: "seeding"
        case .storePreview: "storePreview"
        default: nil
        }
    }
}
