// The widgets' record on disk: the engine's `WidgetData` written where the widget extension
// reads it, `widgets/snapshot.json` in the App Group container, with its art as small JPEGs in
// `widgets/art/`. The shape is the widgets' own (AnistreamWidgetData's `WidgetSnapshot`): the
// engine record's field names, times in whole seconds since 1970, and each row's art by file name.

import AnistreamKit
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Where the widgets' record is written, and the writing.
public struct WidgetRecordFile: Sendable {
    /// The widgets' directory: `widgets/` in the App Group container.
    public let directory: URL

    public init(directory: URL) {
        self.directory = directory
    }

    /// The Info.plist key naming the App Group, filled from the `ANISTREAM_APP_GROUP` build
    /// setting: `group.tv.anistream` on iPhone and iPad, `<team>.tv.anistream` on the Mac. The
    /// Fixtures app has none, so it never writes over the app's record.
    public static let groupKey = "AnistreamAppGroup"

    /// This app's widgets' directory, when its bundle names an App Group it may use.
    public static func shared(bundle: Bundle = .main) -> WidgetRecordFile? {
        guard let group = (bundle.object(forInfoDictionaryKey: groupKey) as? String)?
            .trimmingCharacters(in: .whitespaces),
              !group.isEmpty, !group.contains("$("), !group.hasPrefix("."),
              let container = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
        else { return nil }
        return WidgetRecordFile(directory: container.appending(path: "widgets", directoryHint: .isDirectory))
    }

    public var snapshotFile: URL { directory.appending(path: "snapshot.json") }
    public var artDirectory: URL { directory.appending(path: "art", directoryHint: .isDirectory) }

    /// The two shapes of art the widgets draw, and the size each is scaled to cover: the largest
    /// family's at 2x.
    public enum ArtKind: String, Sendable {
        case wide, poster

        var pixels: (width: Int, height: Int) {
            switch self {
            case .wide: (680, 240)
            case .poster: (96, 144)
            }
        }
    }

    /// The file art from `url` is kept under: the same address, the same file, so writing the
    /// record again fetches nothing it has. FNV-1a, which is stable from one launch to the next.
    public static func artName(for url: String, kind: ArtKind) -> String {
        var hash: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in url.utf8 {
            hash ^= UInt64(byte)
            hash = hash &* 0x0000_0100_0000_01b3
        }
        return "\(kind.rawValue)-\(String(hash, radix: 16)).jpg"
    }

    /// Write `data` in place of the last record, whole, its art fetched with `fetch` where the
    /// directory lacks it, then delete the art it no longer names.
    public func write(_ data: WidgetData, fetch: @Sendable (String) async -> Data?) async throws {
        try FileManager.default.createDirectory(at: artDirectory, withIntermediateDirectories: true)
        var named = Set<String>()
        func art(_ url: String?, _ kind: ArtKind) async -> String? {
            guard let url, !url.isEmpty else { return nil }
            let name = Self.artName(for: url, kind: kind)
            let file = artDirectory.appending(path: name)
            if !FileManager.default.fileExists(atPath: file.path) {
                guard let bytes = await fetch(url), let jpeg = Self.scaled(bytes, kind: kind) else { return nil }
                guard (try? jpeg.write(to: file, options: .atomic)) != nil else { return nil }
            }
            named.insert(name)
            return name
        }

        var continuing: [[String: Any]] = []
        for row in data.continueWatching {
            var item: [String: Any] = [
                "id": Int(row.id), "title": row.title, "episode": Int(row.episode),
                "watched": Int(row.watched), "resume": row.resume,
            ]
            item["episodes"] = row.episodes.map { Int($0) }
            item["fraction"] = row.fraction
            item["episode_title"] = row.episodeTitle
            item["position_secs"] = row.positionSecs.map { Int($0) }
            item["duration_secs"] = row.durationSecs.map { Int($0) }
            item["minutes_left"] = row.minutesLeft.map { Int($0) }
            item["next_airing"] = row.nextAiring.map { Int($0) }
            item["next_airing_episode"] = row.nextAiringEpisode.map { Int($0) }
            item["art"] = Self.art(
                wide: await art(row.bannerUrl, .wide), poster: await art(row.coverUrl, .poster),
                color: row.coverColor)
            continuing.append(item)
        }
        func airing(_ rows: [WidgetAiring]) async -> [[String: Any]] {
            var items: [[String: Any]] = []
            for row in rows {
                var item: [String: Any] = [
                    "id": Int(row.id), "title": row.title, "episode": Int(row.episode),
                    "airs_at": Int(row.airsAt), "on_list": row.onList, "watched": row.watched,
                ]
                item["translation"] = row.translation.map { $0 == .dub ? "dub" : "sub" }
                item["art"] = Self.art(
                    wide: await art(row.bannerUrl, .wide), poster: await art(row.coverUrl, .poster),
                    color: row.coverColor)
                items.append(item)
            }
            return items
        }

        var record: [String: Any] = [
            "version": Int(data.version),
            "generated_at": Int(data.generatedAt),
            "signed_in": data.signedIn,
            "continue_watching": continuing,
            "airing_today": await airing(data.airingToday),
            "next_days": await airing(data.nextDays),
        ]
        record["tracker"] = data.tracker
        record["next_airing"] = data.nextAiring.map { next -> [String: Any] in
            ["id": Int(next.id), "title": next.title, "episode": Int(next.episode), "airs_at": Int(next.airsAt)]
        }
        let json = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
        try json.write(to: snapshotFile, options: .atomic)

        let files = (try? FileManager.default.contentsOfDirectory(atPath: artDirectory.path)) ?? []
        for file in files where !named.contains(file) {
            try? FileManager.default.removeItem(at: artDirectory.appending(path: file))
        }
    }

    private static func art(wide: String?, poster: String?, color: String?) -> [String: Any] {
        var art: [String: Any] = [:]
        art["wide"] = wide
        art["poster"] = poster
        art["color"] = color
        return art
    }

    /// `bytes` as a JPEG scaled to cover `kind`'s size, cropped to it from the middle.
    static func scaled(_ bytes: Data, kind: ArtKind) -> Data? {
        guard let source = CGImageSourceCreateWithData(bytes as CFData, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
        else { return nil }
        let (width, height) = kind.pixels
        let scale = max(Double(width) / Double(image.width), Double(height) / Double(image.height))
        let drawn = CGSize(width: Double(image.width) * scale, height: Double(image.height) * scale)
        guard let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        else { return nil }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(
            x: (Double(width) - drawn.width) / 2, y: (Double(height) - drawn.height) / 2,
            width: drawn.width, height: drawn.height))
        guard let scaled = context.makeImage() else { return nil }
        let out = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(out, UTType.jpeg.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destination, scaled, [kCGImageDestinationLossyCompressionQuality: 0.82] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return out as Data
    }
}
