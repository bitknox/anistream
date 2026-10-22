// Where the widgets' files are: the App Group container the app and the widget extension share.

import Foundation

/// The widgets' directory, `widgets/` in the App Group container:
///
/// - `snapshot.json`, which the app writes and the widgets read;
/// - `art/`, the snapshot's art as small JPEGs, so a widget reads local files only;
/// - `public-schedule.json`, the extension's own copy of AniList's public schedule, for a widget
///   with no snapshot to show;
/// - `live/`, the downloads' Live Activity's covers.
///
/// The group is the `AnistreamAppGroup` key of the bundle's Info.plist, which the build fills
/// from the `ANISTREAM_APP_GROUP` setting: `group.tv.anistream` on iPhone and iPad, the
/// team-prefixed `<team>.tv.anistream` on the Mac, which needs no provisioning profile. A bundle
/// without the key, as the Fixtures app is, has no widgets' directory, so it never writes over
/// the app's.
///
/// Sideloaded, the group has another name. AltStore and SideStore sign the app and the extension
/// with the person's own Apple ID, and register the group the release's signature names as
/// `<group>.<their team>`, which is the one the app is then signed into: AltStore also lists it in
/// Info.plist as `ALTAppGroups`, and both put it in each bundle's `embedded.mobileprovision`. So
/// that group comes first, when there is one.
public struct WidgetFiles: Sendable, Equatable {
    public let directory: URL

    public init(directory: URL) {
        self.directory = directory
    }

    /// The Info.plist key that names the App Group.
    public static let groupKey = "AnistreamAppGroup"

    /// The App Group `bundle` names, when it names one.
    public static func group(in bundle: Bundle = .main) -> String? {
        guard let group = bundle.object(forInfoDictionaryKey: groupKey) as? String else { return nil }
        let trimmed = group.trimmingCharacters(in: .whitespaces)
        // Unexpanded, or expanded from an empty team: not a group to write to.
        guard !trimmed.isEmpty, !trimmed.contains("$("), !trimmed.hasPrefix(".") else { return nil }
        return trimmed
    }

    /// The widgets' directory in the App Group `identifier`, when this process may use it.
    public static func appGroup(_ identifier: String, fileManager: FileManager = .default) -> WidgetFiles? {
        guard let container = fileManager.containerURL(forSecurityApplicationGroupIdentifier: identifier)
        else { return nil }
        return WidgetFiles(directory: container.appending(path: "widgets", directoryHint: .isDirectory))
    }

    /// This process's widgets' directory: the App Group its bundle names, or the one a
    /// sideloading store made of it.
    public static func shared(bundle: Bundle = .main) -> WidgetFiles? {
        guard let group = group(in: bundle) else { return nil }
        let profile = try? Data(contentsOf: bundle.bundleURL.appending(path: "embedded.mobileprovision"))
        let granted = (bundle.object(forInfoDictionaryKey: altStoreGroupsKey) as? [String] ?? [])
            + provisionedGroups(profile ?? Data())
        if let sideloaded = sideloadedGroup(for: group, among: granted), let files = appGroup(sideloaded) {
            return files
        }
        return appGroup(group)
    }

    /// The Info.plist key AltStore lists the App Groups it signed the bundle into under.
    public static let altStoreGroupsKey = "ALTAppGroups"

    /// Of `groups`, the one a sideloading store registered for `group` under its team:
    /// `<group>.<team>`.
    public static func sideloadedGroup(for group: String, among groups: [String]) -> String? {
        groups.first { $0.hasPrefix(group + ".") && !$0.dropFirst(group.count + 1).contains(".") }
    }

    /// The App Groups a provisioning profile's entitlements grant: the plist inside its signature.
    public static func provisionedGroups(_ profile: Data) -> [String] {
        guard let start = profile.firstRange(of: Data("<?xml".utf8)),
            let end = profile.firstRange(of: Data("</plist>".utf8), in: start.upperBound..<profile.endIndex),
            let plist = try? PropertyListSerialization.propertyList(
                from: profile[start.lowerBound..<end.upperBound], format: nil) as? [String: Any],
            let entitlements = plist["Entitlements"] as? [String: Any]
        else { return [] }
        return entitlements["com.apple.security.application-groups"] as? [String] ?? []
    }

    public var snapshotFile: URL { directory.appending(path: "snapshot.json") }
    public var artDirectory: URL { directory.appending(path: "art", directoryHint: .isDirectory) }
    public var publicScheduleFile: URL { directory.appending(path: "public-schedule.json") }
    /// The Live Activity's covers, apart from the snapshot's art, which a snapshot clears.
    public var liveDirectory: URL { directory.appending(path: "live", directoryHint: .isDirectory) }

    /// The art file `name`.
    public func art(_ name: String) -> URL {
        artDirectory.appending(path: name)
    }

    // MARK: The snapshot

    /// The snapshot as it is on disk; nil when there is none.
    public func read() -> WidgetSnapshot.Read? {
        guard let data = try? Data(contentsOf: snapshotFile) else { return nil }
        return WidgetSnapshot.read(data)
    }

    /// The snapshot, when there is one this build reads.
    public func snapshot() -> WidgetSnapshot? {
        if case .snapshot(let snapshot) = read() { return snapshot }
        return nil
    }

    /// Write `snapshot` in place of the last, whole, then delete the art no snapshot names.
    public func write(_ snapshot: WidgetSnapshot) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try snapshot.encoded().write(to: snapshotFile, options: .atomic)
        removeArt(except: snapshot.artFiles)
    }

    /// Write `data` as the art file `name`, whole.
    public func writeArt(_ data: Data, named name: String) throws {
        try FileManager.default.createDirectory(at: artDirectory, withIntermediateDirectories: true)
        try data.write(to: art(name), options: .atomic)
    }

    /// Whether the art file `name` is there.
    public func hasArt(_ name: String) -> Bool {
        FileManager.default.fileExists(atPath: art(name).path)
    }

    /// Delete every art file but `kept`.
    public func removeArt(except kept: Set<String>) {
        let files = (try? FileManager.default.contentsOfDirectory(atPath: artDirectory.path)) ?? []
        for file in files where !kept.contains(file) {
            try? FileManager.default.removeItem(at: art(file))
        }
    }

    /// The file name art fetched from `url` is kept under, in `kind`: the same address is the
    /// same file, so a snapshot written again fetches nothing it has.
    public static func artName(for url: String, kind: ArtKind) -> String {
        // FNV-1a, 64 bits: stable across launches, unlike Hasher.
        var hash: UInt64 = 0xcbf2_9ce4_8422_2325
        for byte in url.utf8 {
            hash ^= UInt64(byte)
            hash = hash &* 0x0000_0100_0000_01b3
        }
        return "\(kind.rawValue)-\(String(hash, radix: 16)).jpg"
    }
}

/// The two shapes of art the widgets draw.
public enum ArtKind: String, Sendable, CaseIterable {
    /// AniList's banner, for the 16:9 cards, the large widgets' rows and the small Continue.
    case wide
    /// The cover, beside a title when there is no banner.
    case poster

    /// The size the file is scaled to cover, in pixels: the largest family at 2×.
    public var pixelSize: (width: Int, height: Int) {
        switch self {
        case .wide: (680, 240)
        case .poster: (96, 144)
        }
    }
}
