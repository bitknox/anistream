// Settings in the main window, on the Mac and a wide iPad: the panes as tabs, the pane's rows in
// a column of cards, and beside them a column that explains or shows what the pane is about.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// Settings' panes. Integrations stays out of v1.
public enum SettingsPane: String, CaseIterable, Identifiable, Hashable, Sendable {
    case playback, downloads, sources, accounts, advanced

    public var id: String { rawValue }

    var title: String {
        switch self {
        case .playback: "Playback"
        case .downloads: "Downloads"
        case .sources: "Sources"
        case .accounts: "Accounts"
        case .advanced: "Advanced"
        }
    }
}

/// A page of the iPhone's Settings sheet.
public enum PhoneSettingsPage: Hashable, Sendable {
    case root, playback, downloads, sources, accounts, providers, stores
    case store(String)
    case plugin(String)
}

struct SettingsScreen: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        @Bindable var navigator = navigator
        VStack(alignment: .leading, spacing: 0) {
            SettingsTabs(selection: $navigator.settingsPane)
                .padding(EdgeInsets(top: 4, leading: 24, bottom: 0, trailing: 24))
            ScrollView {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 76) {
                        pane.frame(width: 600)
                        SettingsAside(pane: navigator.settingsPane)
                            .frame(width: 320)
                            .padding(.top, 20)
                    }
                    VStack(alignment: .leading, spacing: 24) {
                        pane.frame(maxWidth: 600)
                        SettingsAside(pane: navigator.settingsPane).frame(maxWidth: 600)
                    }
                }
                .padding(EdgeInsets(top: 20, leading: 24, bottom: 24, trailing: 24))
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollIndicators(.never)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    @ViewBuilder
    private var pane: some View {
        VStack(alignment: .leading, spacing: 20) {
            if !navigator.settingsQuery.trimmingCharacters(in: .whitespaces).isEmpty {
                SettingsResults(query: navigator.settingsQuery)
            } else {
                switch navigator.settingsPane {
            case .playback: PlaybackPane()
            case .downloads: DownloadsPane()
            case .sources: SourcesPane()
            case .accounts: AccountsPane()
            case .advanced: AdvancedPane()
                }
            }
        }
    }
}

/// Settings' search: every row whose name has the words, with the pane it is on.
struct SettingsResults: View {
    @Environment(Navigator.self) private var navigator
    let query: String

    /// Each row's name, and its pane.
    static let index: [(title: String, pane: SettingsPane)] = [
        ("Translation", .playback), ("Subtitles", .playback), ("Quality", .playback),
        ("Auto-play next", .playback), ("Up Next countdown", .playback),
        ("Skip opening automatically", .playback), ("Skip filler automatically", .playback),
        ("Mark opening and ending", .playback), ("Counts as watched at", .playback),
        ("Spoiler protection", .playback), ("Upscaling", .playback), ("Night mode", .playback),
        ("Start fullscreen", .playback),
        ("Download folder", .downloads), ("Download quality", .downloads), ("Merge subtitles in", .downloads),
        ("Keep seeding", .downloads), ("After each download", .downloads), ("Watched downloads", .downloads),
        ("Source order", .sources), ("Torrent source", .sources), ("Torrent index", .sources),
        ("Protect with", .sources), ("Last check", .sources), ("If a check fails", .sources),
        ("Seed after watching", .sources), ("Seed for at most", .sources), ("Upload limit", .sources),
        ("AniList", .accounts), ("MyAnimeList", .accounts), ("Kitsu", .accounts), ("Simkl", .accounts),
        ("Trakt", .accounts), ("Sync now", .accounts), ("Sign out", .accounts),
        ("mpv options", .advanced),
    ]

    var body: some View {
        let words = query.lowercased().split(separator: " ")
        let found = Self.index.filter { row in words.allSatisfy { row.title.lowercased().contains($0) } }
        if found.isEmpty {
            Text("No setting is called that.")
                .font(.system(size: 13))
                .foregroundStyle(.haze)
                .padding(.leading, 14)
        } else {
            SettingsGroup(title: "Results") {
                ForEach(found, id: \.title) { row in
                    Button {
                        navigator.settingsPane = row.pane
                        navigator.settingsQuery = ""
                    } label: {
                        SettingsRow(title: row.title) {
                            Text(row.pane.title).font(.system(size: 13)).foregroundStyle(.haze).padding(.trailing, 6)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

/// The panes, as a row of tabs: the one on screen in amber on the tint.
struct SettingsTabs: View {
    @Binding var selection: SettingsPane

    var body: some View {
        HStack(spacing: 4) {
            ForEach(SettingsPane.allCases) { pane in
                let selected = pane == selection
                Button {
                    selection = pane
                } label: {
                    Text(pane.title)
                        .font(.system(size: 13, weight: selected ? .medium : .regular))
                        .foregroundStyle(selected ? .amber : .silk)
                        .padding(.horizontal, 12)
                        .frame(height: 28)
                        .background(selected ? Palette.selection : .clear, in: .capsule)
                        .contentShape(.capsule)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Settings panes")
    }
}

/// The column beside a pane: what the pane is about, or for Playback the upscaling preview.
struct SettingsAside: View {
    @Environment(AppStore.self) private var store
    let pane: SettingsPane

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            switch pane {
            case .playback:
                UpscalingPreview()
            case .downloads:
                StorageAside()
            case .sources:
                if store.settings.isOn(.torrents) {
                    aside("Connection check", "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.")
                } else {
                    aside("Sources", "Sources are tried in order, top to bottom. A show can keep its own source, chosen from its title page.")
                }
            case .accounts:
                aside("Accounts", "anistream works without an account. Your watch history stays on this \(Device.name) when you sign out.")
            case .advanced:
                aside("Advanced", "An option here can undo a Playback setting, such as upscaling or night mode. Most people never need this pane.")
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.raised, in: .rect(cornerRadius: 12))
    }

    @ViewBuilder
    private func aside(_ title: String, _ text: String) -> some View {
        Text(title).groupHeaderStyle().fixedLineHeight(14)
        Text(text)
            .font(.system(size: 13))
            .fixedLineHeight(18)
            .foregroundStyle(.silk)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// What the device is called in a sentence: "this Mac", "this iPad", "this iPhone".
enum Device {
    static var name: String {
        #if os(macOS)
        "Mac"
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone"
        #endif
    }
}

// MARK: - Playback

struct PlaybackPane: View {
    @Environment(AppStore.self) private var store
    @AppStorage("anistream.spoilerProtection") private var spoilers = true

    var body: some View {
        SettingsGroup(title: "Language") {
            // "Overridden for 3 shows" waits on E8's override counts.
            SettingsRow(title: SettingKey.translation.title) { SettingMenu(key: .translation) }
            SettingsRow(title: SettingKey.subtitles.title) { SubtitlesMenu() }
            SettingsRow(title: SettingKey.quality.title) { SettingMenu(key: .quality) }
        }
        SettingsGroup(title: "Episodes") {
            SettingsRow(title: SettingKey.autoNext.title) { SettingSwitch(key: .autoNext) }
            // E8: `UpNextCountdown` is not a setting the engine has yet.
            SettingsRow(title: "Up Next countdown") { DeferredMenu(text: "10 s", waitsOn: "E8") }
                .opacity(store.settings.isOn(.autoNext) ? 1 : 0.45)
            SettingsRow(title: SettingKey.skipOpening.title) { SettingSwitch(key: .skipOpening) }
            SettingsRow(title: SettingKey.skipFiller.title) { SettingSwitch(key: .skipFiller) }
            SettingsRow(title: SettingKey.markChapters.title) { SettingSwitch(key: .markChapters) }
            SettingsRow(title: SettingKey.commitThreshold.title, note: "Or when the ending starts, if it is marked.") {
                SettingMenu(key: .commitThreshold)
            }
            // Kept on this device until E8's `SpoilerProtection` moves it into the engine.
            SettingsRow(title: "Spoiler protection", note: "Blurs stills of unwatched episodes after the next one.") {
                Toggle("Spoiler protection", isOn: $spoilers)
                    .toggleStyle(.switch)
                    .tint(.amber)
                    .labelsHidden()
            }
        }
        SettingsGroup(title: "Picture and sound") {
            SettingsRow(title: SettingKey.upscaling.title, selected: true) { SettingMenu(key: .upscaling) }
            SettingsRow(title: SettingKey.nightMode.title, note: "Evens out loud and quiet passages, so dialogue stays clear at low volume.") {
                SettingSwitch(key: .nightMode)
            }
            #if os(macOS)
            SettingsRow(title: SettingKey.fullscreen.title) { SettingSwitch(key: .fullscreen) }
            #endif
        }
    }
}

/// The subtitle language: the common ones by name, in a menu; any other is typed in the file.
struct SubtitlesMenu: View {
    @Environment(AppStore.self) private var store
    static let languages = ["en", "ja", "es", "pt", "fr", "de", "it", "ru", "ar", "id", "ko", "zh"]

    var body: some View {
        let code = store.settings.row(.subtitles)?.typedText ?? store.settings.value(.subtitles) ?? ""
        Menu {
            Picker("Subtitles", selection: Binding(
                get: { code },
                set: { store.setSetting(.subtitles, to: .str($0)) }
            )) {
                ForEach(Self.languages, id: \.self) { language in
                    Text(Self.name(language)).tag(language)
                }
                if !code.isEmpty, !Self.languages.contains(code) {
                    Text(Self.name(code)).tag(code)
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            MenuCapsule(text: code.isEmpty ? "–" : Self.name(code))
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel("Subtitles")
    }

    /// A language code as the system names it in English: "en" reads "English".
    static func name(_ code: String) -> String {
        Locale(identifier: "en").localizedString(forLanguageCode: code) ?? code
    }
}

/// The upscaling modes shown on one drawing: the 480p source on the left, the mode on the right.
struct UpscalingPreview: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let mode = store.settings.row(.upscaling)?.option?.label ?? "natural · ArtCNN"
        Text("Upscaling").groupHeaderStyle().fixedLineHeight(14)
        GeometryReader { proxy in
            let half = proxy.size.width / 2
            ZStack(alignment: .leading) {
                drawing(mode: mode)
                drawing(mode: "off")
                    .frame(width: half, alignment: .leading)
                    .clipped()
                Rectangle().fill(.silk).frame(width: 2).offset(x: half - 1)
            }
            .overlay(alignment: .bottomLeading) { chip("480p source").padding(8) }
            .overlay(alignment: .bottomTrailing) { chip(SettingKey.upscaling.display(mode)).padding(8) }
        }
        .aspectRatio(16 / 9, contentMode: .fit)
        .clipShape(.rect(cornerRadius: 8))
        .accessibilityElement()
        .accessibilityLabel("The 480p source beside \(SettingKey.upscaling.display(mode))")
        Text(note(mode))
            .font(.system(size: 13))
            .fixedLineHeight(18)
            .foregroundStyle(.silk)
            .fixedSize(horizontal: false, vertical: true)
    }

    @ViewBuilder
    private func drawing(mode: String) -> some View {
        if let image = DesignArt.upscalingPreview {
            let picture = Image(decorative: image, scale: 1).resizable().scaledToFill()
            switch mode {
            case "off": picture.blur(radius: 1.6).saturation(0.85).contrast(0.92)
            case "sharp · Anime4K": picture.contrast(1.12).saturation(1.08)
            default: picture
            }
        } else {
            CoverPlate(color: nil)
        }
    }

    private func chip(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 11, weight: .medium))
            .foregroundStyle(.silk)
            .padding(.horizontal, 8)
            .frame(height: 20)
            .background(Palette.glassOverArt, in: .capsule)
            .overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
    }

    private func note(_ mode: String) -> String {
        switch mode {
        case "off": "The 480p source, scaled to fit without processing."
        case "sharp · Anime4K": "Crisper lines and stronger edges, at some cost to texture."
        default: "Faithful line work and texture, nothing restyled."
        }
    }
}

// MARK: - Advanced

/// Options passed to the player as written. The engine has no setting for them yet, so the table
/// is drawn as the board has it and nothing in it can be changed.
struct AdvancedPane: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("mpv options").groupHeaderStyle().fixedLineHeight(14).padding(.leading, 14)
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    Text("Option").frame(width: 344, alignment: .leading)
                    Text("Value")
                    Spacer()
                }
                .font(.system(size: 12))
                .foregroundStyle(.haze)
                .padding(.horizontal, 18)
                .frame(height: 34)
                Rectangle().fill(.faintRule).frame(height: 1)
                HStack(spacing: 18) {
                    Image(systemName: "plus")
                    Image(systemName: "minus")
                    Spacer()
                    Button("Reset") {}
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
                .font(.system(size: 14))
                .foregroundStyle(.silk)
                .padding(.leading, 18)
                .padding(.trailing, 6)
                .frame(height: 44)
            }
            .background(.raised, in: .rect(cornerRadius: 12))
            // No engine step carries the player's own options to the app yet: not in E8 or E9.
            .disabled(true)
            .opacity(0.6)
            .help("Waits on an engine setting for the player's options")
            SettingsFootnote("Passed to the player as written, after anistream’s own options, so a value here wins. Reset removes them all.")
        }
    }
}
