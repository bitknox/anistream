// Settings in the main window, on the Mac and a wide iPad: the panes as tabs, the pane's rows in
// a column of cards, and beside them a column that explains or shows what the pane is about.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// Settings' panes. Integrations stays out of v1.
public enum SettingsPane: String, CaseIterable, Identifiable, Hashable, Sendable {
    case playback, downloads, notifications, sources, accounts, integrations, advanced

    public var id: String { rawValue }

    var title: String {
        switch self {
        case .playback: "Playback"
        case .downloads: "Downloads"
        case .notifications: "Notifications"
        case .sources: "Sources"
        case .accounts: "Accounts"
        case .integrations: "Integrations"
        case .advanced: "Advanced"
        }
    }

    /// The panes this device shows. Integrations is the Mac's: Syncplay and Discord are desktop
    /// programs, neither of which an iPad or iPhone runs. The iPad has Updates under Advanced.
    static var shown: [SettingsPane] {
        #if os(macOS)
        allCases
        #else
        allCases.filter { $0 != .integrations }
        #endif
    }
}

/// A page of the iPhone's Settings sheet.
public enum PhoneSettingsPage: Hashable, Sendable {
    case root, playback, downloads, notifications, sources, accounts, providers, stores, about, acknowledgements
    /// The TVs paired for Play on TV.
    case tvs
    case store(String)
    case plugin(String)
    /// A show's own new-episode switch, from a row of the Notifications page's week.
    case showNotices(AnilistId)

    /// The page a launch argument names: a Settings pane's name, or providers or stores.
    static func named(_ name: String) -> PhoneSettingsPage? {
        switch name {
        case "playback": .playback
        case "downloads": .downloads
        case "notifications": .notifications
        case "sources": .sources
        case "accounts": .accounts
        case "providers": .providers
        case "about": .about
        case "tvs": .tvs
        case "titles": .playback
        case "stores", "pluginStores": .stores
        // The showcase's own, for screenshots.
        case "store": .store("hotaru")
        case "plugin": .plugin("mukashi-archive")
        default: nil
        }
    }
}

struct SettingsScreen: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        @Bindable var navigator = navigator
        VStack(alignment: .leading, spacing: 0) {
            SettingsTabs(selection: $navigator.settingsPane)
                .padding(EdgeInsets(top: 4, leading: 24, bottom: 0, trailing: 24))
            ScrollView {
                SettingsColumns {
                    pane
                    SettingsAside(pane: navigator.settingsPane)
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
            case .notifications: NotificationsPane()
            case .sources: SourcesPane()
            case .accounts: AccountsPane()
            case .integrations: IntegrationsPane()
            case .advanced: AdvancedPane()
                }
            }
        }
    }
}

/// Settings' two columns: the pane's cards, and beside them the column that shows what the pane
/// is about. The column is 280 to 320 points and the pane at least 440. On iPad the pane takes
/// whatever width the column leaves, 32 points from it, so no strip is left empty beside it. The
/// Mac keeps its board's pane of 600, with the gap opening to 76 as the window allows. Where even
/// the narrowest pair does not fit, as in Split View or a small window, the pane takes the width
/// and the column comes under it, as wide.
struct SettingsColumns: Layout {
    static let paneMinimum: CGFloat = 440
    #if os(macOS)
    static let paneMaximum: CGFloat = 600
    static let gapMaximum: CGFloat = 76
    #else
    static let paneMaximum: CGFloat = .infinity
    static let gapMaximum: CGFloat = 32
    #endif
    /// The column's narrowest and widest.
    static let aside: ClosedRange<CGFloat> = 280...320
    static let gap: CGFloat = 32
    /// Beside the pane, the column starts this far down, level with the first group's card.
    static let asideTop: CGFloat = 20
    /// Under the pane, the column starts this far below it.
    static let stackedGap: CGFloat = 24

    struct Columns: Equatable {
        var pane: CGFloat
        var gap: CGFloat
        var aside: CGFloat
    }

    /// The columns side by side in `width`, or nil when the column goes under the pane.
    static func columns(in width: CGFloat) -> Columns? {
        guard width >= paneMinimum + gap + aside.lowerBound else { return nil }
        let asideWidth = min(aside.upperBound, width - gap - paneMinimum)
        let pane = min(paneMaximum, width - gap - asideWidth)
        return Columns(pane: pane, gap: min(gapMaximum, width - pane - asideWidth), aside: asideWidth)
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard subviews.count == 2 else { return .zero }
        let width = proposal.width ?? Self.paneMinimum + Self.gap + Self.aside.upperBound
        let (pane, aside) = (subviews[0], subviews[1])
        if let columns = Self.columns(in: width) {
            let height = max(
                pane.sizeThatFits(ProposedViewSize(width: columns.pane, height: nil)).height,
                Self.asideTop + aside.sizeThatFits(ProposedViewSize(width: columns.aside, height: nil)).height
            )
            return CGSize(width: width, height: height)
        }
        let whole = ProposedViewSize(width: width, height: nil)
        return CGSize(
            width: width,
            height: pane.sizeThatFits(whole).height + Self.stackedGap + aside.sizeThatFits(whole).height
        )
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        guard subviews.count == 2 else { return }
        let (pane, aside) = (subviews[0], subviews[1])
        if let columns = Self.columns(in: bounds.width) {
            pane.place(at: bounds.origin, proposal: ProposedViewSize(width: columns.pane, height: nil))
            aside.place(
                at: CGPoint(x: bounds.minX + columns.pane + columns.gap, y: bounds.minY + Self.asideTop),
                proposal: ProposedViewSize(width: columns.aside, height: nil)
            )
        } else {
            let whole = ProposedViewSize(width: bounds.width, height: nil)
            pane.place(at: bounds.origin, proposal: whole)
            aside.place(
                at: CGPoint(x: bounds.minX, y: bounds.minY + pane.sizeThatFits(whole).height + Self.stackedGap),
                proposal: whole
            )
        }
    }
}

/// Settings' search: every row whose name has the words, with the pane it is on.
struct SettingsResults: View {
    @Environment(AppStore.self) private var store
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
        ("New episodes", .notifications), ("Notification settings", .notifications),
        ("Source order", .sources), ("Torrent source", .sources), ("Torrent index", .sources),
        ("Protect with", .sources), ("Last check", .sources), ("If a check fails", .sources),
        ("Seed after watching", .sources), ("Seed for at most", .sources), ("Upload limit", .sources),
        ("AniList", .accounts), ("MyAnimeList", .accounts), ("Kitsu", .accounts), ("Simkl", .accounts),
        ("Trakt", .accounts), ("Sync now", .accounts), ("Sign out", .accounts), ("Privacy", .accounts),
        ("Syncplay parties", .integrations), ("Syncplay server", .integrations),
        ("Syncplay room", .integrations), ("Check for updates", updatesPane),
        ("mpv options", .advanced),
    ].filter { SettingsPane.shown.contains($0.pane) }

    /// The rows about Discord, which only an engine with a presence has.
    static let presenceRows: [(title: String, pane: SettingsPane)] = [
        ("Discord presence", .integrations), ("Presence shows the title", .integrations),
        ("Rich presence", .integrations),
    ].filter { SettingsPane.shown.contains($0.pane) }

    /// The rows to search, those about Discord only when `presence` says the engine has one.
    static func rows(presence: Bool) -> [(title: String, pane: SettingsPane)] {
        presence ? index + presenceRows : index
    }

    /// Where Updates is: Integrations on the Mac, Advanced on the iPad.
    static var updatesPane: SettingsPane {
        #if os(macOS)
        .integrations
        #else
        .advanced
        #endif
    }

    var body: some View {
        let words = query.lowercased().split(separator: " ")
        let found = Self.rows(presence: store.settings.hasPresence).filter { row in words.allSatisfy { row.title.lowercased().contains($0) } }
        if found.isEmpty {
            Text("No setting is called that.")
                .font(.mac(13))
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
                            Text(row.pane.title).font(.mac(13)).foregroundStyle(.haze).padding(.trailing, 6)
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
            ForEach(SettingsPane.shown) { pane in
                let selected = pane == selection
                Button {
                    selection = pane
                } label: {
                    Text(pane.title)
                        .font(.mac(13, weight: selected ? .medium : .regular))
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
    @Environment(Navigator.self) private var navigator
    let pane: SettingsPane

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            switch pane {
            case .playback:
                if navigator.playbackFocus == .titleLanguage { TitleLanguagePreview() } else { UpscalingPreview() }
            case .downloads:
                StorageAside()
            case .notifications:
                NotificationsAside()
            case .sources:
                if store.settings.isOn(.torrents) {
                    aside("Connection check", "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.")
                } else {
                    aside("Sources", "Sources are tried in order, top to bottom. A show can keep its own source, chosen from its title page.")
                }
            case .accounts:
                aside("Accounts", "anistream works without an account. Your watch history stays on this \(Device.name) when you sign out.")
            case .integrations:
                if store.settings.hasPresence { PresencePreview() }
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
        Text(title).groupHeaderStyle().macLineHeight(14)
            .accessibilityAddTraits(.isHeader)
        Text(text)
            .font(.mac(13))
            .macLineHeight(18)
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
    @Environment(Navigator.self) private var navigator
    @AppStorage(DevicePreferences.followSystemCaptions) private var followCaptions = true

    var body: some View {
        let overridden = store.settings.overriddenNote
        SettingsGroup(title: "Language") {
            TitleLanguageRow()
            SettingsRow(title: SettingKey.translation.title, note: overridden(.translation)) { SettingMenu(key: .translation) }
            SettingsRow(title: SettingKey.subtitles.title) { SubtitlesMenu() }
            // Kept on this device: the style it follows is this device's.
            SettingsRow(
                title: "Subtitles follow your system caption style",
                note: "Plain subtitles take the font, size, colours and edges set in System Settings › Accessibility › Captions. Styled subtitles keep their own look."
            ) {
                Toggle("Subtitles follow your system caption style", isOn: $followCaptions)
                    .toggleStyle(.switch)
                    .tint(.amber)
                    .labelsHidden()
            }
            SettingsRow(title: SettingKey.quality.title, note: overridden(.quality)) { SettingMenu(key: .quality) }
        }
        SettingsGroup(title: "Episodes") {
            SettingsRow(title: SettingKey.autoNext.title) { SettingSwitch(key: .autoNext) }
            SettingsRow(title: SettingKey.upNextCountdown.title) { SettingMenu(key: .upNextCountdown) }
                .disabled(!store.settings.isOn(.autoNext))
                .opacity(store.settings.isOn(.autoNext) ? 1 : 0.45)
            SettingsRow(title: SettingKey.skipOpening.title, note: overridden(.skipOpening)) { SettingSwitch(key: .skipOpening) }
            SettingsRow(title: SettingKey.skipFiller.title, note: overridden(.skipFiller)) { SettingSwitch(key: .skipFiller) }
            SettingsRow(title: SettingKey.markChapters.title) { SettingSwitch(key: .markChapters) }
            SettingsRow(title: SettingKey.skipButton.title, note: "While the opening or ending plays, when it isn't skipped automatically.") {
                SettingSwitch(key: .skipButton)
            }
            SettingsRow(title: SettingKey.commitThreshold.title, note: "Or when the ending starts, if it is marked.") {
                SettingMenu(key: .commitThreshold)
            }
            SettingsRow(title: SettingKey.spoilerProtection.title,
                        note: "Blurs stills of unwatched episodes after the next one.") {
                SettingSwitch(key: .spoilerProtection)
            }
        }
        SettingsGroup(title: "Picture and sound") {
            SettingsRow(title: SettingKey.upscaling.title, selected: navigator.playbackFocus == .upscaling) {
                SettingMenu(key: .upscaling)
            }
            .onTapGesture { navigator.playbackFocus = .upscaling }
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
        Text("Upscaling").groupHeaderStyle().macLineHeight(14)
            .accessibilityAddTraits(.isHeader)
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
            .font(.mac(13))
            .macLineHeight(18)
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
            .font(.mac(11, weight: .medium))
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
            Text("mpv options").groupHeaderStyle().macLineHeight(14).padding(.leading, 14)
                .accessibilityAddTraits(.isHeader)
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    Text("Option").frame(width: 344, alignment: .leading)
                    Text("Value")
                    Spacer()
                }
                .font(.mac(12))
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
                .font(.mac(14))
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
            #if os(iOS)
            // The iPad's: the Mac has these under Integrations, which an iPad does not show.
            UpdatesGroup()
                .padding(.top, 24)
            AboutGroup()
                .padding(.top, 24)
            #endif
        }
    }
}
