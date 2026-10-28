// Settings in the main window, on the Mac and a wide iPad: the panes as tabs, the pane's rows in
// a column of cards, and beside them a column that explains or shows what the pane is about.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// Settings' panes. Integrations stays out of v1.
public enum SettingsPane: String, CaseIterable, Identifiable, Hashable, Sendable {
    case general, playback, downloads, notifications, sources, accounts, integrations, advanced

    public var id: String { rawValue }

    var title: String {
        switch self {
        case .general: L.settingsGroupGeneral
        case .playback: L.settingsGroupPlayback
        case .downloads: L.navDownloads
        case .notifications: L.settingsGroupNotifications
        case .sources: L.sourcesTitle
        case .accounts: L.navAccounts
        case .integrations: L.settingsGroupIntegrations
        case .advanced: L.settingsGroupAdvanced
        }
    }

    /// The panes this device shows. Integrations is the Mac's: Discord is a desktop program, which
    /// an iPad or iPhone does not run. The iPad has Updates under Advanced.
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
    case root, general, playback, downloads, notifications, sources, accounts, providers, stores, about, acknowledgements
    /// The TVs paired for Play on TV.
    case tvs
    case store(String)
    case plugin(String)
    /// A show's own new-episode switch, from a row of the Notifications page's week.
    case showNotices(AnilistId)

    /// The page a launch argument names: a Settings pane's name, or providers or stores.
    static func named(_ name: String) -> PhoneSettingsPage? {
        switch name {
        case "general", "titles": .general
        case "playback": .playback
        case "downloads": .downloads
        case "notifications": .notifications
        case "sources": .sources
        case "accounts": .accounts
        case "providers": .providers
        case "about": .about
        case "tvs": .tvs
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
            case .general: GeneralPane()
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
        (L.settingsTitleLanguage, .general), (L.settingsAdult, .general),
        (L.settingsTranslation, .playback), (L.playerSubtitles, .playback), (L.settingsQuality, .playback),
        (L.settingsAutoPlay, .playback), (L.settingsUpNextCountdown, .playback),
        (L.settingsSkipOpening, .playback), (L.settingsSkipFiller, .playback),
        (L.settingsMarkOpening, .playback), (L.settingsCountsWatched, .playback),
        (L.settingsSpoiler, .playback), (L.playerUpscaling, .playback), (L.playerNightMode, .playback),
        (L.settingsStartFullScreen, .playback),
        (L.downloadsFolder, .downloads), (L.downloadsQuality, .downloads), (L.settingsMergeSubtitles, .downloads),
        (L.settingsKeepSeeding, .downloads), (L.settingsAfterDownload, .downloads), (L.settingsWatchedDownloads, .downloads),
        (L.settingsNewEpisodes, .notifications), (L.settingsSystemNotifications, .notifications),
        (L.sourcesSourceOrderRow, .sources), (L.sourcesTorrentSource, .sources), (L.torrentsIndex, .sources),
        (L.torrentsProtectWith, .sources), (L.torrentsLastCheck, .sources), (L.torrentsIfCheckFails, .sources),
        (L.settingsSeedAfter, .sources), (L.settingsSeedFor, .sources), (L.settingsUploadLimit, .sources),
        (AccountsModel.name("anilist"), .accounts), (AccountsModel.name("mal"), .accounts),
        (AccountsModel.name("kitsu"), .accounts), (AccountsModel.name("simkl"), .accounts),
        (AccountsModel.name("trakt"), .accounts), (L.accountsSyncNow, .accounts),
        (L.accountsSignOut, .accounts), (L.settingsPrivacy, .accounts),
        (L.settingsCheckUpdates, updatesPane),
        (L.settingsMpvOptions, .advanced),
    ].filter { SettingsPane.shown.contains($0.pane) }

    /// The rows about Discord, which only an engine with a presence has.
    static let presenceRows: [(title: String, pane: SettingsPane)] = [
        (L.settingsDiscordPresence, .integrations), (L.settingsPresenceTitle, .integrations),
        (L.settingsPresenceRich, .integrations),
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
            Text(L.settingsNoMatch)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .padding(.leading, 14)
        } else {
            SettingsGroup(title: L.settingsResults) {
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
        .accessibilityLabel(L.settingsPanesLabel)
    }
}

/// The column beside a pane: what the pane is about, for General the titles as the language
/// setting names them, and for Playback the upscaling preview.
struct SettingsAside: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let pane: SettingsPane

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            switch pane {
            case .general:
                TitleLanguagePreview()
            case .playback:
                UpscalingPreview()
            case .downloads:
                StorageAside()
            case .notifications:
                NotificationsAside()
            case .sources:
                if store.settings.isOn(.torrents) {
                    aside(L.settingsAsideCheckTitle, L.torrentsCheckNote)
                } else {
                    aside(L.sourcesTitle, L.settingsAsideSources)
                }
            case .accounts:
                aside(L.navAccounts, L.settingsAsideAccounts)
            case .integrations:
                if store.settings.hasPresence { PresencePreview() }
            case .advanced:
                aside(L.settingsGroupAdvanced, L.settingsAsideAdvanced)
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
        "Mac" // i18n-skip
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone" // i18n-skip
        #endif
    }
}

// MARK: - General

/// How titles read and which are listed: nothing here is about playing an episode.
struct GeneralPane: View {
    var body: some View {
        SettingsGroup(title: L.settingsGroupTitles) {
            TitleLanguageRow()
            SettingsRow(title: SettingKey.adultTitles.title, note: L.settingsAdultDetail(tracker: AccountsModel.name("anilist"))) {
                SettingSwitch(key: .adultTitles)
            }
        }
        SettingsGroup(title: L.settingsLanguage) {
            AppLanguageRow()
        }
    }
}

// MARK: - Playback

struct PlaybackPane: View {
    @Environment(AppStore.self) private var store
    @AppStorage(DevicePreferences.followSystemCaptions) private var followCaptions = true

    var body: some View {
        let overridden = store.settings.overriddenNote
        SettingsGroup(title: L.settingsLanguage) {
            SettingsRow(title: SettingKey.translation.title, note: overridden(.translation)) { SettingMenu(key: .translation) }
            SettingsRow(title: SettingKey.subtitles.title) { SubtitlesMenu() }
            // Kept on this device: the style it follows is this device's.
            SettingsRow(
                title: L.settingsSystemCaptions,
                note: L.settingsSystemCaptionsDetail
            ) {
                Toggle(L.settingsSystemCaptions, isOn: $followCaptions)
                    .toggleStyle(.switch)
                    .tint(.amber)
                    .labelsHidden()
            }
            SettingsRow(title: SettingKey.quality.title, note: overridden(.quality)) { SettingMenu(key: .quality) }
        }
        SettingsGroup(title: L.settingsGroupEpisodes) {
            SettingsRow(title: SettingKey.autoNext.title) { SettingSwitch(key: .autoNext) }
            SettingsRow(title: SettingKey.upNextCountdown.title) { SettingMenu(key: .upNextCountdown) }
                .disabled(!store.settings.isOn(.autoNext))
                .opacity(store.settings.isOn(.autoNext) ? 1 : 0.45)
            SettingsRow(title: SettingKey.skipOpening.title, note: overridden(.skipOpening)) { SettingSwitch(key: .skipOpening) }
            SettingsRow(title: SettingKey.skipFiller.title, note: overridden(.skipFiller)) { SettingSwitch(key: .skipFiller) }
            SettingsRow(title: SettingKey.markChapters.title) { SettingSwitch(key: .markChapters) }
            SettingsRow(title: SettingKey.skipButton.title, note: L.settingsSkipButtonDetail) {
                SettingSwitch(key: .skipButton)
            }
            SettingsRow(title: SettingKey.commitThreshold.title, note: L.settingsCountsWatchedDetail) {
                SettingMenu(key: .commitThreshold)
            }
            SettingsRow(title: SettingKey.spoilerProtection.title,
                        note: L.settingsSpoilerDetail) {
                SettingSwitch(key: .spoilerProtection)
            }
        }
        SettingsGroup(title: L.playerPictureAndSound) {
            // Selected: the column beside the pane is about it.
            SettingsRow(title: SettingKey.upscaling.title, selected: true) {
                SettingMenu(key: .upscaling)
            }
            SettingsRow(title: SettingKey.nightMode.title, note: L.playerNightModeDetail) {
                SettingSwitch(key: .nightMode)
            }
            #if os(macOS)
            SettingsRow(title: SettingKey.fullscreen.title) { SettingSwitch(key: .fullscreen) }
            #endif
        }
        #if os(iOS)
        // The TVs paired to this iPad, beside the rest of how episodes play. The Mac keeps them
        // in Integrations; the iPhone in its own page of the settings sheet.
        TVsGroup()
        #endif
    }
}

/// The subtitle language: the common ones by name, in a menu; any other is typed in the file.
struct SubtitlesMenu: View {
    @Environment(AppStore.self) private var store
    static let languages = ["en", "ja", "es", "pt", "fr", "de", "it", "ru", "ar", "id", "ko", "zh"]

    var body: some View {
        let code = store.settings.row(.subtitles)?.typedText ?? store.settings.value(.subtitles) ?? ""
        Menu {
            Picker(L.playerSubtitles, selection: Binding(
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
        .accessibilityLabel(L.playerSubtitles)
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
        Text(L.playerUpscaling).groupHeaderStyle().macLineHeight(14)
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
            .overlay(alignment: .bottomLeading) { chip(L.settingsUpscalingSource).padding(8) }
            .overlay(alignment: .bottomTrailing) { chip(SettingKey.upscaling.display(mode)).padding(8) }
        }
        .aspectRatio(16 / 9, contentMode: .fit)
        .clipShape(.rect(cornerRadius: 8))
        .accessibilityElement()
        .accessibilityLabel(L.settingsUpscalingCompare(mode: SettingKey.upscaling.display(mode)))
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
        case "off": L.settingsUpscalingNoteNone
        case "sharp · Anime4K": L.settingsUpscalingNoteSharp
        default: L.settingsUpscalingNoteNatural
        }
    }
}

// MARK: - Advanced

/// Options passed to the player. The engine has no setting for the player's own options yet,
/// so the pane says the player runs with anistream's, and offers nothing it cannot keep.
struct AdvancedPane: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L.settingsAdvancedNone)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .padding(.leading, 14)
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
