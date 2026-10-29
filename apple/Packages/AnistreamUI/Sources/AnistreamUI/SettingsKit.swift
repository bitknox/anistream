// The parts every settings screen is made of on the Mac and iPad: a group's heading over a raised
// card of rows, a row's title with a line beneath, and the controls a row ends in: a switch, a
// menu of the engine's own choices, a value to type.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// A group of rows: its heading in small capitals, the rows on a raised card with hairlines
/// between them, and a footnote beneath when it has one.
struct SettingsGroup<Content: View>: View {
    var title: String?
    var footnote: String?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let title {
                Text(title)
                    .groupHeaderStyle()
                    .macLineHeight(14)
                    .padding(.leading, 14)
                    .accessibilityAddTraits(.isHeader)
            }
            VStack(spacing: 0) {
                Group(subviews: content) { rows in
                    ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                        if index > 0 {
                            Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, 14).padding(.trailing, 4)
                        }
                        row
                    }
                }
            }
            .padding(.vertical, 4)
            .background(.raised, in: .rect(cornerRadius: 12))
            if let footnote {
                SettingsFootnote(footnote)
            }
        }
    }
}

/// A line of explanation under a group.
struct SettingsFootnote: View {
    let text: Text

    init(_ text: String) {
        self.text = Text(text)
    }

    init(_ text: Text) {
        self.text = text
    }

    var body: some View {
        text
            .font(.mac(12))
            .macLineHeight(17)
            .foregroundStyle(.haze)
            .padding(.horizontal, 14)
            .padding(.top, 4)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// One row: its title, a line beneath when it has one, and what changes it at the end.
struct SettingsRow<Accessory: View>: View {
    let title: String
    var note: String?
    var noteColor: Color = .haze
    var selected = false
    @ViewBuilder let accessory: Accessory

    var body: some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.mac(13, weight: selected ? .medium : .regular))
                    .foregroundStyle(selected ? .amber : .silk)
                if let note {
                    Text(note)
                        .font(.mac(12))
                        .foregroundStyle(noteColor)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .macLineHeight(16)
            Spacer(minLength: 0)
            accessory
        }
        .padding(note == nil ? EdgeInsets(top: 4, leading: 10, bottom: 4, trailing: 6)
                             : EdgeInsets(top: 8, leading: 10, bottom: 8, trailing: 6))
        .frame(minHeight: note == nil ? 36 : 50)
        .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
        .contentShape(.rect)
        .padding(.horizontal, 4)
        .accessibilityElement(children: .combine)
    }
}

extension SettingsRow where Accessory == EmptyView {
    init(title: String, note: String? = nil) {
        self.init(title: title, note: note) { EmptyView() }
    }
}

/// A setting's switch, on the engine's word: a flip sends the step and the row follows the
/// settings the engine answers with.
struct SettingSwitch: View {
    @Environment(AppStore.self) private var store
    let key: SettingKey

    var body: some View {
        Toggle(key.title, isOn: Binding(
            get: { store.settings.isOn(key) },
            set: { store.setSwitch(key, on: $0) }
        ))
        .toggleStyle(.switch)
        .tint(.amber)
        .labelsHidden()
        .disabled(store.settings.row(key) == nil)
    }
}

extension SettingsModel {
    /// "Overridden for 3 shows", under a row some shows set for themselves.
    func overriddenNote(_ key: SettingKey) -> String? {
        overriddenShows(key).map { L.settingsOverridden(count: Int($0)) }
    }
}

/// A menu's label: the capsule on the Mac and iPad, the plain value with chevrons in an iPhone's
/// list row.
struct MenuValue: View {
    let text: String
    let phone: Bool
    var fill = false

    var body: some View {
        #if os(iOS)
        if phone {
            PhoneMenuValue(text: text)
        } else {
            MenuCapsule(text: text, fill: fill)
        }
        #else
        MenuCapsule(text: text, fill: fill)
        #endif
    }
}

/// The capsule a menu shows its choice in: the choice, and the up-and-down chevrons.
struct MenuCapsule: View {
    let text: String
    var symbol: String?
    /// Whether it takes the width it is given, as a form's column of menus does.
    var fill = false

    var body: some View {
        HStack(spacing: 8) {
            if let symbol {
                Image(systemName: symbol).font(.mac(12))
            }
            Text(text)
                .font(.mac(13))
                .monospacedDigit()
                .lineLimit(1)
            if fill { Spacer(minLength: 0) }
            Image(systemName: "chevron.up.chevron.down")
                .font(.mac(9, weight: .semibold))
        }
        .foregroundStyle(.silk)
        .padding(.leading, 12)
        .padding(.trailing, 10)
        .frame(height: 28)
        .background(.raisedSelected, in: .capsule)
        .overlay(Capsule().strokeBorder(Palette.glassEdge, lineWidth: 1))
        .contentShape(.capsule)
    }
}

/// A setting's menu: the engine's own choices, in the order its arrows step, the one in effect
/// ticked. Choosing one sends it as the file takes it.
struct SettingMenu: View {
    @Environment(AppStore.self) private var store
    let key: SettingKey

    var body: some View {
        let row = store.settings.row(key)
        let current = row?.option
        Menu {
            Picker(key.title, selection: Binding(
                get: { current?.value },
                set: { value in if let value { store.setSetting(key, to: value) } }
            )) {
                ForEach(key.options, id: \.value) { option in
                    Text(key.display(option)).tag(Optional(option.value))
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            MenuCapsule(text: row.map { r in current.map { key.display($0) } ?? key.display(r.value) } ?? "–")
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(row == nil)
        .accessibilityLabel(key.title)
    }
}

extension SettingKey {
    /// The row's title as a screen writes it, in sentence case: `counts as watched at` reads
    /// "Counts as watched at".
    var title: String {
        switch self {
        case .titles: L.settingsTitleLanguage
        case .trailers: L.settingsTrailersRow
        case .adultTitles: L.settingsAdult
        case .upNextCountdown: L.settingsUpNextCountdown
        case .spoilerProtection: L.settingsSpoiler
        case .downloadQuality: L.settingsQuality
        case .keepNextEpisodes: L.settingsKeepNext
        case .downloadsOnCellular: L.settingsDownloadCellular
        case .torrentsOnCellular: L.settingsTorrentsCellular
        case .vpnOnFailure: L.torrentsIfCheckFails
        case .torrentIndex: L.torrentsIndex
        case .syncplay: L.settingsSyncplay
        case .syncplayServer: L.settingsSyncplayServer
        case .syncplayRoom: L.settingsSyncplayRoom
        case .presence: L.settingsDiscordPresence
        case .presenceTitle: L.settingsPresenceTitle
        case .presenceRich: L.settingsPresenceRich
        case .updateCheck: L.settingsCheckUpdates
        case .translation: L.settingsTranslation
        case .subtitles: L.playerSubtitles
        case .quality: L.settingsQuality
        case .commitThreshold: L.settingsCountsWatched
        case .autoNext: L.settingsAutoPlay
        case .skipOpening: L.settingsSkipOpening
        case .skipFiller: L.settingsSkipFiller
        case .markChapters: L.settingsMarkOpening
        case .skipButton: L.settingsSkipButton
        case .fullscreen: L.settingsStartFullScreen
        case .upscaling: L.playerUpscaling
        case .nightMode: L.playerNightMode
        case .downloadDir: L.downloadsFolder
        case .mergeSubtitles: L.settingsMergeSubtitles
        case .keepSeeding: L.settingsKeepSeeding
        case .downloadHook: L.settingsHookCommand
        case .torrents: L.sourcesTorrentSource
        case .vpnMode: L.torrentsProtectWith
        case .seedRatio: L.settingsSeedAfter
        case .seedHours: L.settingsSeedFor
        case .uploadLimit: L.settingsUploadLimit
        case .sourceOrder: L.sourcesSourceOrderRow
        default: "\(self)".capitalizedSentence
        }
    }

    /// A choice's words as a menu shows them: the engine's, in the person's language, with sub
    /// or dub said as the boards say it. The numbers follow the person's formats.
    func display(_ option: SettingOption) -> String {
        let locale = AirTime.locale
        switch (self, option.value) {
        case (.commitThreshold, .float(let fraction)):
            return fraction.formatted(.percent.locale(locale))
        case (.seedRatio, .float(let ratio)) where ratio > 0:
            return L.settingsOptionUntilRatio(ratio: ratio.formatted(.number.precision(.fractionLength(1)).locale(locale)))
        case (.seedHours, .int(let hours)):
            return Duration.seconds(hours * 3600).formatted(.units(allowed: [.hours], width: .abbreviated).locale(locale))
        case (.upNextCountdown, .int(let seconds)) where seconds > 0:
            return Duration.seconds(seconds).formatted(.units(allowed: [.seconds], width: .abbreviated).locale(locale))
        case (.uploadLimit, .int(let kib)):
            return kib == 0 ? L.settingsOptionUnlimited : L.settingsOptionPerSecond(size: Wording.bytes(UInt64(kib) * 1024))
        case (.keepNextEpisodes, .int(let count)) where count > 0:
            return L.episodesCount(count: Int(count))
        default:
            return display(option.label)
        }
    }

    /// A choice's engine words as a menu shows them.
    func display(_ label: String) -> String {
        if self == .translation {
            switch label.split(separator: " · ").first.map(String.init) ?? label {
            case "sub": return L.audioSubbed
            case "dub": return L.audioDubbed
            default: break
            }
        }
        switch (self, label) {
        case (_, "off"): return L.valueOff
        case (_, "on"): return L.valueOn
        case (.upscaling, "natural · ArtCNN"): return L.settingsOptionNatural // i18n-skip: the engine's words
        case (.upscaling, "sharp · Anime4K"): return L.settingsOptionSharp // i18n-skip: the engine's words
        case (.downloadQuality, "standard"): return L.settingsOptionStandard
        case (.downloadQuality, "higher"): return L.settingsOptionHigher
        case (.theme, "adaptive"): return L.settingsOptionAdaptive
        case (.theme, "immersive"): return L.settingsOptionImmersive
        case (.titles, "AniList's choice"): return L.settingsTitleTracker(tracker: AccountsModel.name("anilist"))
        case (.trailers, "always"): return L.settingsTrailersAlways
        case (.trailers, "Wi-Fi only"): return L.settingsTrailersWifiOnly
        case (.titles, "English"): return L.settingsTitleEnglish
        case (.titles, "Romaji"): return L.settingsTitleRomaji
        case (.playerUi, "your mpv setup"): return L.settingsOptionMpv
        case (.vpnOnFailure, "pause torrents"): return L.torrentsFailPause
        case (.vpnOnFailure, "stop torrents"): return L.torrentsFailStop
        case (.vpnOnFailure, "warn only"): return L.torrentsFailWarn
        default: return label.firstCapitalized
        }
    }
}

extension View {
    /// A sheet's width on the Mac. On iPhone and iPad a sheet takes the width it is given.
    func sheetWidth(_ width: CGFloat) -> some View {
        #if os(macOS)
        frame(width: width)
        #else
        frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        #endif
    }

    /// Run `open` once the window is up, for a sheet a launch argument asks for: a sheet asked
    /// for while the window is still coming up is not shown.
    func onLaunchSheet(_ name: String, _ navigator: Navigator, open: @escaping () -> Void) -> some View {
        task {
            guard navigator.launchSheet == name else { return }
            try? await Task.sleep(for: .milliseconds(500))
            navigator.launchSheet = nil
            open()
        }
    }
}

extension String {
    /// The first letter in capitals and the rest as it is: "natural · ArtCNN" reads
    /// "Natural · ArtCNN".
    var firstCapitalized: String {
        prefix(1).uppercased() + dropFirst()
    }
}
