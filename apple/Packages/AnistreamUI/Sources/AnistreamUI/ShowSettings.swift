// One show's own settings (MacShowSettings, PhoneShowSettings): its audio and the subtitles the
// player remembers for it, its auto-skip and spoiler protection, each following Settings until
// the show sets its own, the source it keeps, and a reset that puts it all back.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// What a show's sheet says of its rows.
enum ShowWords {
    /// The audio a translation plays: Japanese for subtitled, English for dubbed.
    static func audio(_ value: String) -> String {
        let language = value.hasPrefix("dub") ? "en" : "ja"
        return (AirTime.locale.localizedString(forLanguageCode: language) ?? language).firstCapitalized
    }

    /// A row's value as the sheet says it: Japanese or English for the audio, On or Off.
    static func shown(_ key: ShowSettingKey, _ value: String) -> String {
        key == .translation ? audio(value) : (value == "on" ? L.valueOn : L.valueOff)
    }

    /// Following Settings: "Use default · On".
    static func useDefault(_ row: ShowSettingRow) -> String {
        L.valueUseDefaultIs(value: shown(row.key, row.default))
    }

    /// What a show can set a row to, beside following Settings.
    static func choices(_ key: ShowSettingKey) -> [(value: SettingValue, label: String)] {
        key == .translation
            ? [(.str("sub"), audio("sub")), (.str("dub"), audio("dub"))]
            : [(.bool(true), L.valueOn), (.bool(false), L.valueOff)]
    }

    /// The subtitles remembered for the audio in effect: "Off", "English", "English (Hotaru)", or
    /// following Settings when none are.
    static func subtitles(_ view: ShowSettingsView?, settings: String) -> String {
        let dub = view?.rows.first { $0.key == .translation }?.value.hasPrefix("dub") ?? false
        let remembered = view?.remembered.first { $0.translation == (dub ? .dub : .sub) }?.subtitles
        switch remembered {
        case .off: return L.valueOff
        case .track(let track): return trackWords(track)
        case nil:
            return settings.isEmpty ? L.valueUseDefault : L.valueUseDefaultIs(value: settings)
        }
    }

    /// A remembered track: its language by name, and its title in words of its own past the
    /// language it repeats, "english signs" being "Signs": "English (Signs)".
    static func trackWords(_ track: RememberedTrack) -> String {
        let language = track.language.map { code in
            AirTime.locale.localizedString(forLanguageCode: code) ?? code
        }
        let words = track.title.split(separator: " ").filter { word in
            language.map { word.lowercased() != $0.lowercased() } ?? true
        }
        let title = words.joined(separator: " ").firstCapitalized
        switch (language, title.isEmpty) {
        case (nil, true): return L.showSettingsUnnamedTrack
        case (nil, false): return title
        case (let language?, true): return language
        case (let language?, false): return L.showSettingsTrackWith(language: language, part: title)
        }
    }
}

/// A row's menu: following Settings, or one of the values the show can set, the one in effect
/// ticked.
struct ShowSettingMenu: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    let key: ShowSettingKey
    var phone = false

    var body: some View {
        let row = store.showSettings.row(key, of: id)
        let text = row.map { $0.own == nil ? ShowWords.useDefault($0) : ShowWords.shown(key, $0.value) } ?? "–"
        Menu {
            if let row {
                Toggle(isOn: Binding(get: { row.own == nil }, set: { if $0 { store.setShowSetting(id, key, to: nil) } })) {
                    Text(ShowWords.useDefault(row))
                }
                ForEach(ShowWords.choices(key), id: \.label) { choice in
                    Toggle(isOn: Binding(
                        get: { row.own == choice.value },
                        set: { if $0 { store.setShowSetting(id, key, to: choice.value) } }
                    )) {
                        Text(choice.label)
                    }
                }
            }
        } label: {
            MenuValue(text: text, phone: phone, fill: true)
        }
        .disabled(row == nil)
        .accessibilityValue(text)
    }
}

/// The source a show keeps: automatic, or one of the providers, pinned for it alone.
struct ShowSourcePicker<Label: View>: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    @ViewBuilder let label: Label

    var body: some View {
        Picker(selection: Binding(
            get: { store.showSource(id) },
            set: { store.setProviderPreference(id, to: $0) }
        )) {
            Text(L.valueAutomatic).tag(String?.none)
            ForEach(store.orderedProviders, id: \.id) { row in
                Text(store.info(row).name).tag(Optional(row.id))
            }
        } label: {
            label
        }
    }
}

#if os(macOS)
/// The Mac's sheet, over the title's page.
struct ShowSettingsSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let entry: Entry

    var body: some View {
        let view = store.showSettings.sheets[entry.id]
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 14) {
                CoverArt(entry: entry, width: 36)
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry.showTitle).font(.macShowTitle(size: 15)).foregroundStyle(.silk)
                    Text(L.showSettingsTitle).font(.mac(13)).foregroundStyle(.haze)
                }
            }
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 12, verticalSpacing: 12) {
                row(L.settingsAudio) { menu(.translation) }
                row(L.playerSubtitles) {
                    // The player remembers these as they are chosen while watching; nothing here
                    // sets them.
                    Text(ShowWords.subtitles(view, settings: settingsSubtitles))
                        .font(.mac(13))
                        .foregroundStyle(.silk)
                        .padding(.leading, 12)
                        .frame(width: 270, height: 28, alignment: .leading)
                }
                note(L.showSettingsSubsRemembered)
                rule
                row(L.showSettingsSkipOpening) { menu(.skipOpening) }
                row(L.showSettingsSkipFiller) { menu(.skipFiller) }
                row(L.settingsSpoiler) { menu(.spoilerProtection) }
                note(L.showSettingsSpoilerDetail)
                rule
                row(L.settingsNewEpisodes) { ShowNoticesMenu(id: entry.id, mac: true).frame(width: 270) }
                note(L.settingsNotifyShow)
                rule
                row(L.showSettingsSource) {
                    Menu {
                        ShowSourcePicker(id: entry.id) { EmptyView() }
                            .pickerStyle(.inline)
                            .labelsHidden()
                    } label: {
                        MenuCapsule(text: sourceName, fill: true)
                    }
                    .menuStyle(.button)
                    .buttonStyle(.plain)
                    .menuIndicator(.hidden)
                    .frame(width: 270)
                }
                note(L.showSettingsSourceDetail)
                rule
                GridRow {
                    Color.clear.gridCellUnsizedAxes([.horizontal, .vertical])
                    Button(L.showSettingsReset) { store.resetShowSettings(entry.id) }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .disabled(view == nil)
                }
                note(L.showSettingsResetDetail)
            }
            .padding(.top, 24)
            HStack {
                Spacer()
                Button(L.actionDone) { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 34))
                    .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .frame(width: 500)
        .background(.raised)
        .closesOnEscape { dismiss() }
        .task {
            store.loadDetail(entry.id)
            store.loadShowSettings(entry.id)
        }
    }

    private func menu(_ key: ShowSettingKey) -> some View {
        ShowSettingMenu(id: entry.id, key: key)
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .frame(width: 270, alignment: .leading)
    }

    private var settingsSubtitles: String {
        let code = store.settings.row(.subtitles)?.typedText ?? ""
        return code.isEmpty ? "" : SubtitlesMenu.name(code)
    }

    private func row(_ label: String, @ViewBuilder control: () -> some View) -> some View {
        GridRow {
            Text(L.showSettingsRowLabel(label: label))
                .font(.mac(13))
                .foregroundStyle(.silk)
                .gridColumnAlignment(.trailing)
                .frame(width: 130, alignment: .trailing)
            control()
        }
    }

    private func note(_ text: String) -> some View {
        GridRow {
            Color.clear.gridCellUnsizedAxes([.horizontal, .vertical])
            Text(text).font(.mac(12)).foregroundStyle(.haze)
        }
    }

    private var rule: some View {
        Rectangle().fill(.faintRule).frame(height: 1).gridCellColumns(2)
    }

    /// The pinned source's name, or Automatic.
    private var sourceName: String {
        guard let id = store.showSource(entry.id) else { return L.valueAutomatic }
        return store.providers.row(id).map { store.info($0).name } ?? id
    }
}
#else
/// The iPhone's sheet, over the title's page.
struct PhoneShowSettingsSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let entry: Entry

    var body: some View {
        let view = store.showSettings.sheets[entry.id]
        NavigationStack {
            PhoneList {
                Section {
                    menuRow(L.settingsAudio, key: .translation)
                    HStack {
                        Text(L.playerSubtitles).font(.ui(17)).foregroundStyle(.silk)
                        Spacer()
                        Text(ShowWords.subtitles(view, settings: settingsSubtitles))
                            .font(.ui(17))
                            .foregroundStyle(.haze)
                    }
                    .listRowBackground(Palette.raised)
                    .accessibilityElement(children: .combine)
                } header: {
                    PhoneSectionHeader(L.showSettingsAudioSubtitles)
                } footer: {
                    Text(L.showSettingsSubsRemembered)
                }
                Section {
                    menuRow(L.showSettingsSkipOpening, key: .skipOpening)
                    menuRow(L.showSettingsSkipFiller, key: .skipFiller)
                    menuRow(L.settingsSpoiler, key: .spoilerProtection, note: L.showSettingsSpoilerDetail)
                } header: {
                    PhoneSectionHeader(L.settingsGroupEpisodes)
                }
                Section {
                    HStack {
                        PhoneRowTitle(title: L.settingsNewEpisodes)
                        Spacer()
                        ShowNoticesMenu(id: entry.id)
                    }
                    .listRowBackground(Palette.raised)
                } header: {
                    PhoneSectionHeader(L.settingsGroupNotifications)
                } footer: {
                    Text(L.settingsNotifyShow)
                }
                Section {
                    HStack {
                        Text(L.showSettingsSource).font(.ui(17)).foregroundStyle(.silk)
                        Spacer()
                        Menu {
                            ShowSourcePicker(id: entry.id) { EmptyView() }.pickerStyle(.inline)
                        } label: {
                            PhoneMenuValue(text: sourceName)
                        }
                    }
                    .listRowBackground(Palette.raised)
                } footer: {
                    Text(L.showSettingsSourceDetail)
                }
                Section {
                    Button(L.showSettingsReset) { store.resetShowSettings(entry.id) }
                        .fontWeight(.semibold)
                        .foregroundStyle(.silk)
                        .disabled(view == nil)
                        .listRowBackground(Palette.raised)
                } footer: {
                    Text(L.showSettingsResetDetail)
                }
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    HStack(spacing: 12) {
                        CoverArt(entry: entry, width: 36)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(entry.showTitle).font(.macShowTitle(size: 17, relativeTo: .body)).foregroundStyle(.silk)
                            Text(L.showSettingsTitle).font(.mac(15)).foregroundStyle(.haze)
                        }
                        .fixedSize()
                    }
                }
                .sharedBackgroundVisibility(.hidden)
                ToolbarItem(placement: .topBarTrailing) {
                    Button(L.actionDone) { dismiss() }.fontWeight(.semibold).tint(.silk)
                }
            }
        }
        .task {
            store.loadDetail(entry.id)
            store.loadShowSettings(entry.id)
        }
    }

    private func menuRow(_ title: String, key: ShowSettingKey, note: String? = nil) -> some View {
        HStack {
            PhoneRowTitle(title: title, note: note)
            Spacer()
            ShowSettingMenu(id: entry.id, key: key, phone: true)
        }
        .listRowBackground(Palette.raised)
    }

    private var settingsSubtitles: String {
        let code = store.settings.row(.subtitles)?.typedText ?? ""
        return code.isEmpty ? "" : SubtitlesMenu.name(code)
    }

    private var sourceName: String {
        guard let id = store.showSource(entry.id) else { return L.valueAutomatic }
        return store.providers.row(id).map { store.info($0).name } ?? id
    }
}
#endif
