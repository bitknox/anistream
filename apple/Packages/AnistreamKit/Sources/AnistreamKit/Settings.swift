// The Settings screen's rules as a native screen needs them: the values each stepped setting
// takes and the words the engine gives each one. Ports of `anistream_model::settings`'s ladders
// (`step`) and its wording (`value`), kept to the same rules, so a menu offers exactly what the
// engine's arrows step through and names each choice as the row will once it is chosen.

import Foundation

/// One value a setting can be set to, as `SetSetting` takes it, and the words its row then says.
public struct SettingOption: Hashable, Sendable {
    public let value: SettingValue
    /// As the engine words it: `1080p`, `natural · ArtCNN`, `until ratio 1.0`, `on`.
    public let label: String

    public init(value: SettingValue, label: String) {
        self.value = value
        self.label = label
    }
}

extension SettingKey {
    /// The settings with two states, on and off, that a switch shows.
    public var isSwitch: Bool {
        switch self {
        case .motion, .autoNext, .skipOpening, .skipFiller, .markChapters, .fullscreen, .nightMode,
             .mergeSubtitles, .keepSeeding, .presence, .presenceTitle, .torrents, .syncplay,
             .updateCheck:
            true
        default:
            false
        }
    }

    /// The values a stepped setting takes, in the order its arrows walk them, each with its
    /// words: `step`'s ladders, the two choices of theme, sub or dub and the player interface,
    /// and off then on for a switch. Empty for one that is typed, read-only or the source order.
    public var options: [SettingOption] {
        switch self {
        case .quality:
            [480, 720, 1080, 2160].map { SettingOption(value: .int(Int64($0)), label: "\($0)p") }
        case .commitThreshold:
            [70, 80, 85, 90, 95].map { SettingOption(value: .float(Double($0) / 100), label: "\($0)%") }
        case .upscaling:
            [("off", "off"), ("natural", "natural · ArtCNN"), ("sharp", "sharp · Anime4K")]
                .map { SettingOption(value: .str($0.0), label: $0.1) }
        case .seedRatio:
            // Tenths, as the engine matches them.
            [0, 5, 10, 15, 20].map { tenths in
                let ratio = Double(tenths) / 10
                return SettingOption(
                    value: .float(ratio),
                    label: tenths > 0 ? String(format: "until ratio %.1f", ratio) : "off"
                )
            }
        case .seedHours:
            [1, 6, 12, 24, 72].map { SettingOption(value: .int(Int64($0)), label: "\($0) h") }
        case .uploadLimit:
            [0, 256, 512, 1024, 2048, 5120].map {
                SettingOption(value: .int(Int64($0)), label: Self.uploadLabel(kib: UInt64($0)))
            }
        case .translation:
            ["sub", "dub"].map { SettingOption(value: .str($0), label: $0) }
        case .theme:
            ["adaptive", "immersive"].map { SettingOption(value: .str($0), label: $0) }
        case .playerUi:
            [("anistream", "anistream"), ("mpv", "your mpv setup")]
                .map { SettingOption(value: .str($0.0), label: $0.1) }
        default:
            isSwitch
                ? [SettingOption(value: .bool(false), label: "off"), SettingOption(value: .bool(true), label: "on")]
                : []
        }
    }

    /// An upload cap as people read it: `unlimited`, `524 kB/s`, `2.1 MB/s`. `upload_label`.
    static func uploadLabel(kib: UInt64) -> String {
        kib == 0 ? "unlimited" : "\(Wording.bytes(kib * 1024))/s"
    }
}

extension SettingRow {
    /// The option the row stands on, found by its words. Theme and sub or dub say the saved
    /// choice first, with the session's beside it when that differs, and stand on the saved one.
    public var option: SettingOption? {
        let saved = key == .theme || key == .translation
            ? String(value.split(separator: " · ").first ?? Substring(value))
            : value
        return key.options.first { $0.label == saved }
    }

    /// Whether a switch is on.
    public var isOn: Bool {
        value == "on"
    }

    /// The text a typed setting starts editing from, when it is typed.
    public var typedText: String? {
        if case .typed(let text) = change { return text }
        return nil
    }

    /// Whether the row can be changed from here at all: `SettingChange::editable`.
    public var isEditable: Bool {
        if case .fixed = change { return false }
        return true
    }
}

extension Wording {
    /// A size as people read it: `9.4 GB`, `140 MB`, `0 B`. `wording::human_bytes`.
    ///
    /// One decimal below ten and none above, in decimal units.
    public static func bytes(_ bytes: UInt64) -> String {
        let units: [(String, UInt64)] = [("GB", 1_000_000_000), ("MB", 1_000_000), ("kB", 1_000), ("B", 1)]
        for (unit, scale) in units where bytes >= scale {
            let value = Double(bytes) / Double(scale)
            return value < 10 && scale > 1
                ? String(format: "%.1f %@", value, unit)
                : "\(UInt64(value.rounded())) \(unit)"
        }
        return "0 B"
    }
}
