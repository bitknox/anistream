// Rows the testing engine has none of, for screenshots of the screens that show them: providers
// from plugin stores, the stores themselves, a download queue, the VPN guard's verdict. Invented,
// as the design boards invent them, and only ever on the testing engine.

import AnistreamKit
import CoreGraphics
import Foundation
import ImageIO
#if canImport(UIKit)
import UIKit
#endif

#if FIXTURES

/// What a screenshot can ask the Fixtures app to show beside the testing engine's own data, with
/// `-AnistreamShowcase <name>`: `sources`, `stores`, `downloads`, `invite`, or `all`; and
/// `update`, a newer release with its notice, and `pale`, 900001 renamed to a title long enough to
/// wrap and given a near-white banner, which `all` leaves out; `offline`, the device without a
/// network, what was loaded staying; `stranded`, offline with nothing loaded; and `failed`, the
/// lists failed to load. `all` leaves these out too.
public enum Showcase: String, CaseIterable, Sendable {
    case sources, stores, downloads, invite, update, pale, offline, stranded, failed, all

    /// The one the launch arguments name.
    public static func from(arguments: [String]) -> Showcase? {
        guard let index = arguments.firstIndex(of: "-AnistreamShowcase"), arguments.indices.contains(index + 1)
        else { return nil }
        return Showcase(rawValue: arguments[index + 1])
    }

    /// The events to apply, and which of the engine's own to set aside so they stay on screen.
    @MainActor func events(
        settings: SettingsView?, entry: (AnilistId) -> Entry? = { _ in nil }
    ) -> (events: [Event], held: Set<String>) {
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
                    url: "https://plugins.hotaru.example", host: "plugins.hotaru.example",
                    fingerprint: "9f3c 1a2b 77de 0c41", alreadyJoined: false, deviceLabel: Self.deviceKind
                ))))
                held.insert("storePreview")
            case .update:
                events.append(.update(.available(release: Self.release, notify: true)))
                held.insert("update")
            case .pale:
                // The banner first, so the page finds it here and never asks the engine for it.
                if let bytes = Self.paleBanner, let title = entry(Self.paleTitle) {
                    events.append(.artworkReady(url: Self.paleBannerUrl, bytes: bytes))
                    events.append(.detail(Self.pale(title)))
                }
            case .offline:
                events.append(.connectivity(offline: true))
                held.insert("connectivity")
            case .stranded, .failed:
                if self == .stranded { events.append(.connectivity(offline: true)) }
                let reason = self == .stranded ? "no route to host" : "AniList unreachable: error sending request"
                let views: [ContentFor] = [.continue, .seasonal, .calendar]
                    + LibraryModel.order.map { ContentFor.library($0) }
                events += views.map { .content(view: $0, content: .failed(reason)) }
                held.formUnion(["connectivity", "content"])
            case .all:
                break
            }
        }
        return (events, held)
    }

    /// What the engine offers a store to call this device: its kind.
    @MainActor static var deviceKind: String {
        #if os(macOS)
        "Mac"
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone"
        #endif
    }

    static let providerRows: [ProviderRow] = [
        ProviderRow(id: "hotaru-open-archive", kind: "plugin", state: "ready", latencyMs: 180, lastError: nil,
                    usable: true, heldBack: false, name: "Hotaru Open Archive", version: "2.3.0",
                    hosts: ["open-archive.hotaru.example", "media.open-archive.hotaru.example"],
                    origin: .store(id: "hotaru", name: "Hotaru Plugins"), disabled: false, update: nil),
        ProviderRow(id: "mukashi-archive", kind: "plugin", state: "ready", latencyMs: 320, lastError: nil,
                    usable: true, heldBack: false, name: "Mukashi Archive", version: "1.4.0",
                    hosts: ["mukashi-archive.example"], origin: .store(id: "hotaru", name: "Hotaru Plugins"),
                    disabled: false,
                    update: ProviderUpdate(version: "1.5.0", newHosts: ["media2.mukashi-archive.example"])),
        ProviderRow(id: "machi-library-films", kind: "plugin", state: "down", latencyMs: nil,
                    lastError: "the site did not answer in 15 s.", usable: false, heldBack: false,
                    name: "Machi Library Films", version: "0.3.0", hosts: ["films.machi-library.example"],
                    origin: .byHand, disabled: false, update: nil),
        ProviderRow(id: "torrent", kind: "torrent", state: "ready", latencyMs: nil, lastError: nil, usable: true,
                    heldBack: false, name: "Torrent source", version: nil, hosts: [], origin: .builtIn,
                    disabled: false, update: nil),
    ]

    /// A release newer than any build, with a file for each app.
    static let release: Release = {
        let file = { (name: String, size: UInt64) in
            ReleaseAsset(
                name: name, url: "https://github.com/bitknox/anistream/releases/download/v9.9.0/\(name)",
                sha256: String(repeating: "0f", count: 32), size: size)
        }
        return Release(
            version: "9.9.0", publishedAt: 1_792_664_639,
            pageUrl: "https://github.com/bitknox/anistream/releases/tag/v9.9.0",
            notes: "Smoother calendar\n\nThe calendar scrolls without dropping frames, and the week starts on the day you choose.",
            macos: file("anistream-9.9.0-macos.dmg", 56_208_258),
            ios: file("anistream-9.9.0-ios-unsigned.ipa", 59_624_502),
            android: file("anistream-9.9.0-android-arm64.apk", 81_836_978))
    }()

    static let storeRows: [StoreRow] = [
        StoreRow(id: "hotaru", name: "Hotaru Plugins", url: "https://plugins.hotaru.example", host: "plugins.hotaru.example",
                 fingerprint: "9f3c 1a2b 77de 0c41", member: "Mika", owner: false, state: .ready, checkedAt: nil, plugins: [
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
        StoreRow(id: "mika", name: "Mika's plugins", url: "https://plugins.mika.example", host: "plugins.mika.example",
                 fingerprint: "2b7e 90c4 d1f3 5a68", member: "Mika", owner: true, state: .ready, checkedAt: nil, plugins: [
                    StorePluginRow(id: "machi-library-films", name: "Machi Library Films", version: "0.3.0",
                                   hosts: ["films.machi-library.example"], status: .addedByHand),
                    StorePluginRow(id: "kobunko-film-library", name: "Kōbunko Film Library", version: "0.9.1",
                                   hosts: ["films.kobunko.example"], status: .otherStore("Hotaru Plugins")),
                 ]),
        StoreRow(id: "tsuki", name: "Tsuki Club", url: "https://tsuki-club.example", host: "tsuki-club.example",
                 fingerprint: "e815 3c6a 0b9d 42f7", member: "Mika", owner: false,
                 state: .offline("the store did not answer"), checkedAt: nil, plugins: [
                    StorePluginRow(id: "sora-festival", name: "Sora Animation Festival", version: "1.1.0",
                                   hosts: ["archive.sora-festival.example"], status: .available),
                 ]),
        StoreRow(id: "ame", name: "Ame Plugins", url: "https://plugins.ame.example", host: "plugins.ame.example",
                 fingerprint: "71aa c3e0 58b2 9d14", member: "Mika", owner: false, state: .revoked, checkedAt: nil, plugins: [
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

    /// The title `pale` restyles: the one the shots open most.
    static let paleTitle: AnilistId = 900_001
    static let paleBannerUrl = "https://img.example/banner/pale.png"

    /// `entry` with a made-up title long enough to wrap to two lines at any width, and the
    /// near-white banner.
    static func pale(_ entry: Entry) -> Entry {
        var entry = entry
        entry.title = "Kamifusen Toudai no Bannin to Nanatsu no Kaze no Tegami"
        entry.secondary = "The Paper Balloon Lighthouse Keeper and the Letters of the Seven Winds"
        entry.titleRomaji = entry.title
        entry.titleEnglish = entry.secondary
        entry.titleNative = "紙風船灯台の番人と七つの風の手紙"
        entry.bannerUrl = paleBannerUrl
        return entry
    }

    /// A near-white banner as PNG: white warming to paper, with a few faint pastel shapes, the
    /// lightest art a title can have. Small, so its one decode answers every size asked of it.
    static let paleBanner: Data? = {
        let (width, height) = (960, 360)
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let context = CGContext(
                  data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: space,
                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        let paper = [CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1), CGColor(srgbRed: 0.97, green: 0.96, blue: 0.94, alpha: 1)]
        if let wash = CGGradient(colorsSpace: space, colors: paper as CFArray, locations: [0, 1]) {
            context.drawLinearGradient(wash, start: .zero, end: CGPoint(x: width, y: height), options: [])
        }
        let shapes: [(CGRect, CGColor)] = [
            (CGRect(x: 60, y: 200, width: 220, height: 220), CGColor(srgbRed: 0.98, green: 0.90, blue: 0.92, alpha: 1)),
            (CGRect(x: 640, y: 40, width: 260, height: 260), CGColor(srgbRed: 0.90, green: 0.95, blue: 0.99, alpha: 1)),
            (CGRect(x: 380, y: 250, width: 120, height: 120), CGColor(srgbRed: 0.99, green: 0.96, blue: 0.86, alpha: 1)),
        ]
        for (rect, color) in shapes {
            context.setFillColor(color)
            context.fillEllipse(in: rect)
        }
        guard let image = context.makeImage() else { return nil }
        let data = NSMutableData()
        guard let png = CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(png, image, nil)
        return CGImageDestinationFinalize(png) ? data as Data : nil
    }()

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
        case .update: "update"
        case .connectivity: "connectivity"
        case .content: "content"
        default: nil
        }
    }
}
