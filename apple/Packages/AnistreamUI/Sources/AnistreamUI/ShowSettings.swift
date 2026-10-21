// One show's own settings: its source today, and what E8 brings beside it, the audio and
// subtitles it remembers and the playback settings it overrides.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// What a show's sheet says of each setting it can override, until the engine says the show's
/// own (E8's `LoadShowSettings`): the default, and what that default is.
@MainActor
enum ShowDefaults {
    static func skipOpening(_ store: AppStore) -> String {
        "Use Default · " + (store.settings.isOn(.skipOpening) ? "On" : "Off")
    }

    static func skipFiller(_ store: AppStore) -> String {
        "Use Default · " + (store.settings.isOn(.skipFiller) ? "On" : "Off")
    }

    static func spoilers(_ on: Bool) -> String {
        "Use Default · " + (on ? "On" : "Off")
    }

    static func subtitles(_ store: AppStore) -> String {
        let code = store.settings.row(.subtitles)?.typedText ?? ""
        return code.isEmpty ? "Use Default" : "Use Default · " + SubtitlesMenu.name(code)
    }
}

/// The source a show keeps: automatic, or one of the providers, pinned for it alone.
struct ShowSourcePicker<Label: View>: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    @ViewBuilder let label: Label

    var body: some View {
        Picker(selection: Binding(
            get: { store.providers.preference },
            set: { store.setProviderPreference(id, to: $0) }
        )) {
            Text("Automatic").tag(String?.none)
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
    @AppStorage("anistream.spoilerProtection") private var spoilers = true
    let entry: Entry

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 14) {
                CoverArt(entry: entry, width: 36)
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry.showTitle).font(.mac(15, weight: .semibold)).foregroundStyle(.silk)
                    Text("Settings for this show").font(.mac(13)).foregroundStyle(.haze)
                }
            }
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 12, verticalSpacing: 12) {
                // E8: the audio and subtitles a show remembers, and its overrides, are the
                // engine's `LoadShowSettings` and `SetShowSetting`.
                row("Audio:") { deferred("Use Default") }
                row("Subtitles:") { deferred(ShowDefaults.subtitles(store)) }
                note("Remembered from your last episode.")
                rule
                row("Skip opening:") { deferred(ShowDefaults.skipOpening(store)) }
                row("Skip filler:") { deferred(ShowDefaults.skipFiller(store)) }
                row("Spoiler protection:") { deferred(ShowDefaults.spoilers(spoilers)) }
                note("Blurs stills after the next episode.")
                rule
                row("New episodes:") { ShowNoticesMenu(id: entry.id, mac: true).frame(width: 270) }
                note("A notification when an episode comes out.")
                rule
                row("Source:") {
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
                note("Automatic tries your sources in order.")
                rule
                GridRow {
                    Color.clear.gridCellUnsizedAxes([.horizontal, .vertical])
                    Button("Reset to Defaults") {}
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .disabled(true)
                        .help("Comes with engine step E8")
                }
                note("Also clears the remembered audio and subtitles.")
            }
            .padding(.top, 24)
            HStack {
                Spacer()
                Button("Done") { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 34))
                    .keyboardShortcut(.defaultAction)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .frame(width: 500)
        .background(.raised)
        .closesOnEscape { dismiss() }
        // The pinned source comes with the title's detail.
        .task { store.loadDetail(entry.id) }
    }

    private func row(_ label: String, @ViewBuilder control: () -> some View) -> some View {
        GridRow {
            Text(label)
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
        guard let id = store.providers.preference else { return "Automatic" }
        return store.providers.row(id).map { store.info($0).name } ?? id
    }

    private func deferred(_ text: String) -> some View {
        MenuCapsule(text: text, fill: true)
            .frame(width: 270, alignment: .leading)
            .opacity(0.45)
            .help("Comes with engine step E8")
    }
}
#else
/// The iPhone's sheet, over the title's page.
struct PhoneShowSettingsSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @AppStorage("anistream.spoilerProtection") private var spoilers = true
    let entry: Entry

    var body: some View {
        NavigationStack {
            PhoneList {
                Section {
                    // E8: the remembered tracks and the overrides.
                    PhoneDeferredRow(title: "Audio", value: "Use default", waitsOn: "E8")
                    PhoneDeferredRow(title: "Subtitles", value: phone(ShowDefaults.subtitles(store)), waitsOn: "E8")
                } header: {
                    PhoneSectionHeader("Audio and subtitles")
                } footer: {
                    Text("Remembered from your last episode.")
                }
                Section {
                    PhoneDeferredRow(title: "Skip opening", value: phone(ShowDefaults.skipOpening(store)), waitsOn: "E8")
                    PhoneDeferredRow(title: "Skip filler", value: phone(ShowDefaults.skipFiller(store)), waitsOn: "E8")
                    PhoneDeferredRow(title: "Spoiler protection", note: "Blurs stills after the next episode",
                                     value: phone(ShowDefaults.spoilers(spoilers)), waitsOn: "E8")
                } header: {
                    PhoneSectionHeader("Episodes")
                }
                Section {
                    HStack {
                        PhoneRowTitle(title: "New episodes")
                        Spacer()
                        ShowNoticesMenu(id: entry.id)
                    }
                    .listRowBackground(Palette.raised)
                } header: {
                    PhoneSectionHeader("Notifications")
                } footer: {
                    Text("When an episode of this show comes out")
                }
                Section {
                    HStack {
                        Text("Source").font(.mac(17)).foregroundStyle(.silk)
                        Spacer()
                        Menu {
                            ShowSourcePicker(id: entry.id) { EmptyView() }.pickerStyle(.inline)
                        } label: {
                            PhoneMenuValue(text: sourceName)
                        }
                    }
                    .listRowBackground(Palette.raised)
                } footer: {
                    Text("Automatic tries your sources in order.")
                }
                Section {
                    Button("Reset to defaults") {}
                        .fontWeight(.semibold)
                        .disabled(true)
                        .listRowBackground(Palette.raised)
                } footer: {
                    Text("Also clears the remembered audio and subtitles.")
                }
            }
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    HStack(spacing: 12) {
                        CoverArt(entry: entry, width: 36)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(entry.showTitle).font(.macShowTitle(size: 17, relativeTo: .body)).foregroundStyle(.silk)
                            Text("Settings for this show").font(.mac(15)).foregroundStyle(.haze)
                        }
                        .fixedSize()
                    }
                }
                .sharedBackgroundVisibility(.hidden)
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }.fontWeight(.semibold).tint(.silk)
                }
            }
        }
        .tint(.amber)
        .task { store.loadDetail(entry.id) }
    }

    /// The Mac's words as the iPhone writes them: "Use default · On".
    private func phone(_ text: String) -> String {
        text.replacingOccurrences(of: "Use Default", with: "Use default")
    }

    private var sourceName: String {
        guard let id = store.providers.preference else { return "Automatic" }
        return store.providers.row(id).map { store.info($0).name } ?? id
    }
}
#endif
