// The parts every settings screen is made of on the Mac and iPad: a group's heading over a raised
// card of rows, a row's title with a line beneath, and the controls a row ends in: a switch, a
// menu of the engine's own choices, a value to type.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
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
        overriddenShows(key).map { "Overridden for \(Wording.plural(Int($0), "show"))" }
    }
}

/// A switch the engine cannot set yet: drawn as the board has it, and disabled.
struct DeferredSwitch: View {
    let title: String
    let on: Bool
    /// The engine step it waits on, for the tooltip: "E8".
    let waitsOn: String

    var body: some View {
        Toggle(title, isOn: .constant(on))
            .toggleStyle(.switch)
            .tint(.amber)
            .labelsHidden()
            .disabled(true)
            .help("Comes with engine step \(waitsOn)")
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
                    Text(key.display(option.label)).tag(Optional(option.value))
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            MenuCapsule(text: row.map { key.display(current?.label ?? $0.value) } ?? "–")
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(row == nil)
        .accessibilityLabel(key.title)
    }
}

/// A menu the engine cannot set yet: its choice as the board shows it, disabled.
struct DeferredMenu: View {
    let text: String
    let waitsOn: String

    var body: some View {
        MenuCapsule(text: text)
            .opacity(0.45)
            .help("Comes with engine step \(waitsOn)")
            .accessibilityLabel("\(text), not available yet")
    }
}

extension SettingKey {
    /// The row's title as a screen writes it, in sentence case: `counts as watched at` reads
    /// "Counts as watched at".
    var title: String {
        switch self {
        case .titles: "Title language"
        case .adultTitles: "Show adult titles"
        case .upNextCountdown: "Up Next countdown"
        case .spoilerProtection: "Spoiler protection"
        case .downloadQuality: "Quality"
        case .keepNextEpisodes: "Keep next"
        case .downloadsOnCellular: "Download on cellular"
        case .torrentsOnCellular: "Torrents on cellular"
        case .vpnOnFailure: "If a check fails"
        case .torrentIndex: "Torrent index"
        case .syncplay: "Syncplay parties"
        case .syncplayServer: "Syncplay server"
        case .syncplayRoom: "Syncplay room"
        case .presence: "Discord presence"
        case .presenceTitle: "Presence shows the title"
        case .presenceRich: "Rich presence"
        case .updateCheck: "Check for updates"
        case .translation: "Translation"
        case .subtitles: "Subtitles"
        case .quality: "Quality"
        case .commitThreshold: "Counts as watched at"
        case .autoNext: "Auto-play next"
        case .skipOpening: "Skip opening automatically"
        case .skipFiller: "Skip filler automatically"
        case .markChapters: "Mark opening and ending"
        case .skipButton: "Show skip button"
        case .fullscreen: "Start fullscreen"
        case .upscaling: "Upscaling"
        case .nightMode: "Night mode"
        case .downloadDir: "Download folder"
        case .mergeSubtitles: "Merge subtitles in"
        case .keepSeeding: "Keep seeding"
        case .downloadHook: "Command"
        case .torrents: "Torrent source"
        case .vpnMode: "Protect with"
        case .seedRatio: "Seed after watching"
        case .seedHours: "Seed for at most"
        case .uploadLimit: "Upload limit"
        case .sourceOrder: "Source order"
        default: "\(self)".capitalizedSentence
        }
    }

    /// A choice's words as a menu shows them: the engine's, with a capital, and sub or dub said
    /// as the boards say it.
    func display(_ label: String) -> String {
        if self == .translation {
            switch label.split(separator: " · ").first.map(String.init) ?? label {
            case "sub": return "Subtitled"
            case "dub": return "Dubbed"
            default: break
            }
        }
        return label.firstCapitalized
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
