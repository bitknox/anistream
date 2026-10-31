// Settings on iPhone: a sheet with a stack of its own, opened from the account button on
// Continue, its pages native grouped lists on the engine's own settings.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

extension PhoneSettingsPage: Identifiable {
    public var id: Self { self }
}

/// The sheet: Settings' first page, and the page it was opened at pushed over it.
struct PhoneSettingsSheet: View {
    @Environment(\.dismiss) private var dismiss
    let start: PhoneSettingsPage
    @State private var path: [PhoneSettingsPage] = []

    var body: some View {
        NavigationStack(path: $path) {
            PhoneSettingsRoot()
                .navigationDestination(for: PhoneSettingsPage.self) { page in
                    page.view
                }
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button(L.actionDone) { dismiss() }.fontWeight(.semibold).tint(.silk)
                    }
                }
        }
        .onAppear { if start != .root, path.isEmpty { path = [start] } }
        .modifier(DeviceCodePresenter(here: true))
        .modifier(KitsuPresenter(here: true))
        // Pair a TV, from the TVs page.
        .modifier(TVRemoteSheets(place: .settings))
        // What the engine says while Settings is open, a sign-in that failed among it: at the
        // foot of the sheet, clear of its bar.
        .overlay(alignment: .bottom) {
            NoticeCapsule(inSettingsSheet: true).padding(.horizontal, 16).padding(.bottom, 24)
        }
        // Silk for the bar's actions and links; switches set their own amber.
        .tint(.silk)
        .preferredColorScheme(.dark)
    }
}

extension PhoneSettingsPage {
    @MainActor @ViewBuilder
    var view: some View {
        switch self {
        case .root: PhoneSettingsRoot()
        case .general: PhoneGeneralPage()
        case .playback: PhonePlaybackPage()
        case .downloads: PhoneDownloadsPage()
        case .notifications: PhoneNotificationsPage()
        case .showNotices(let id): PhoneShowNoticesPage(id: id)
        case .sources: PhoneSourcesPage()
        case .accounts: PhoneAccountsPage()
        case .providers: PhoneProvidersPage()
        case .stores: PhoneStoresPage()
        case .about: PhoneAboutPage()
        case .acknowledgements: PhoneAcknowledgementsPage()
        case .tvs: PhoneTVsPage()
        case .hiddenTitles: PhoneHiddenTitlesPage()
        case .store(let id): PhoneStorePage(id: id)
        case .plugin(let id): PhonePluginPage(id: id)
        }
    }
}

/// A grouped list on the ground, its rows raised, as every page of the sheet draws it.
struct PhoneList<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        List { content }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(.ground)
            .listRowBackground(Palette.raised)
            .environment(\.defaultMinListRowHeight, 48)
    }
}

/// A section header as the sheet writes it: small capitals in haze.
struct PhoneSectionHeader: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text).textCase(.uppercase)
            .font(.ui(13, weight: .medium))
            .kerning(13 * 0.04)
            .foregroundStyle(.haze)
    }
}

/// A row's title, with a line beneath when it has one.
struct PhoneRowTitle: View {
    let title: String
    var note: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            if let note {
                Text(note).font(.ui(13)).foregroundStyle(.haze)
            }
        }
    }
}

/// A setting's switch in a list row.
struct PhoneSettingSwitch: View {
    @Environment(AppStore.self) private var store
    let key: SettingKey
    var note: String?

    var body: some View {
        Toggle(isOn: Binding(get: { store.settings.isOn(key) }, set: { store.setSwitch(key, on: $0) })) {
            PhoneRowTitle(title: key.title, note: note)
        }
        .tint(.amber)
        .disabled(store.settings.row(key) == nil)
        .listRowBackground(Palette.raised)
    }
}

/// A setting's menu of the engine's own choices in a list row.
struct PhoneSettingPicker: View {
    @Environment(AppStore.self) private var store
    let key: SettingKey
    var note: String?

    var body: some View {
        let row = store.settings.row(key)
        let current = row?.option
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(key.title).font(.ui(17)).foregroundStyle(.silk)
                Spacer()
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
                } label: {
                    PhoneMenuValue(text: row.map { r in current.map { key.display($0) } ?? key.display(r.value) } ?? "–")
                }
                .disabled(row == nil)
            }
            if let note {
                Text(note).font(.ui(13)).foregroundStyle(.haze)
            }
        }
        .listRowBackground(Palette.raised)
        .accessibilityElement(children: .combine)
    }
}

/// A menu's choice at a row's end: the value and the up-and-down chevrons, in haze.
struct PhoneMenuValue: View {
    let text: String

    var body: some View {
        HStack(spacing: 6) {
            Text(text)
            Image(systemName: "chevron.up.chevron.down").font(.ui(11, weight: .semibold))
        }
        .font(.ui(17))
        .foregroundStyle(.haze)
    }
}

// MARK: - The first page

struct PhoneSettingsRoot: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dynamicTypeSize) private var typeSize

    var body: some View {
        let anilist = store.accounts.tracker("anilist")
        PhoneList {
            Section {
                NavigationLink(value: PhoneSettingsPage.accounts) {
                    HStack(spacing: 14) {
                        Avatar(name: anilist?.user, size: 48)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(anilist?.connected == true ? (anilist?.user ?? L.settingsSignedIn) : L.accountsStateSignedOut)
                                .font(.ui(17, weight: .semibold))
                                .foregroundStyle(.silk)
                            HStack(spacing: 6) {
                                if let anilist, anilist.connected {
                                    StateDot(state: AccountWords.dot(anilist, store.accounts))
                                    Text(AccountsModel.name("anilist") + " · " + AccountWords.status(anilist, store.accounts))
                                } else {
                                    Text(L.accountsLibraryNeeds(tracker: AccountsModel.name("anilist")))
                                }
                            }
                            .font(.ui(15))
                            .foregroundStyle(.haze)
                        }
                    }
                    .padding(.vertical, 6)
                }
                .listRowBackground(Palette.raised)
            }
            Section {
                link(.general, L.settingsGroupGeneral, "gearshape", value: store.titleChoice.label)
                link(.playback, L.settingsGroupPlayback, "play.rectangle", value: playbackValue)
                link(.downloads, L.navDownloads, "arrow.down.to.line", value: Wording.bytes(store.downloads.bytesOnDisk))
                link(.notifications, L.settingsGroupNotifications, "bell", value: store.settings.isOn(.episodeNotifications) ? L.valueOn : L.valueOff)
                link(.sources, L.sourcesTitle, "powerplug", value: store.settings.isOn(.torrents) ? L.settingsTorrentsValueOn : L.settingsTorrentsValueOff)
                link(.accounts, L.navAccounts, "person", value: anilist?.connected == true ? AccountsModel.name("anilist") : L.valueNone)
                if store.tv.available {
                    link(.tvs, L.tvTitleList, "tv", value: store.tv.paired.isEmpty ? L.valueNone : store.tv.paired.count.formatted(.number.locale(AirTime.locale)))
                }
            }
            Section {
                link(.about, L.aboutTitle, "info.circle", value: AppVersion.short)
                Link(destination: AppVersion.privacy) {
                    HStack {
                        Label(L.settingsPrivacy, systemImage: "hand.raised").font(.ui(17)).foregroundStyle(.silk)
                        Spacer()
                        Image(systemName: "arrow.up.forward").font(.ui(15)).foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
            } footer: {
                Text(L.legalHostsNoContent)
            }
        }
        .navigationTitle(L.settingsTitle)
        .navigationBarTitleDisplayMode(.large)
        .task {
            store.loadDownloads()
        }
    }

    private var playbackValue: String {
        let translation = store.settings.row(.translation).map { SettingKey.translation.display($0.value) } ?? ""
        let quality = store.settings.value(.quality) ?? ""
        return [translation, quality].filter { !$0.isEmpty }.joined(separator: " · ")
    }

    private func link(_ page: PhoneSettingsPage, _ title: String, _ symbol: String, value: String) -> some View {
        NavigationLink(value: page) {
            // At the accessibility sizes the value goes under the title, as the system's own
            // rows put it. The title drops its symbol and keeps to one line, shrinking a little
            // rather than breaking a word.
            if typeSize.isAccessibilitySize {
                VStack(alignment: .leading, spacing: 2) {
                    Label(title, systemImage: symbol)
                        .labelStyle(.titleOnly)
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                        .foregroundStyle(.silk)
                    Text(value).foregroundStyle(.haze)
                }
                .font(.ui(17))
                .padding(.vertical, 4)
            } else {
                HStack {
                    Label(title, systemImage: symbol)
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                    Spacer()
                    Text(value).font(.ui(17)).foregroundStyle(.haze)
                }
            }
        }
        .listRowBackground(Palette.raised)
    }
}

/// An account's initial in a circle.
struct Avatar: View {
    let name: String?
    let size: CGFloat

    var body: some View {
        Text(name.flatMap(\.first).map { String($0).uppercased() } ?? "A")
            .font(.system(size: size * 0.44, weight: .semibold))
            .foregroundStyle(.silk)
            .frame(width: size, height: size)
            .background(.raisedSelected, in: .circle)
            .accessibilityHidden(true)
    }
}

// MARK: - General

/// How titles read and which are listed.
struct PhoneGeneralPage: View {
    var body: some View {
        PhoneList {
            Section {
                PhoneTitleLanguageRow()
                PhoneSettingPicker(key: .trailers, note: L.settingsTrailersNoteApple)
                PhoneSettingSwitch(key: .adultTitles, note: L.settingsAdultDetail(tracker: AccountsModel.name("anilist")))
                NavigationLink(value: PhoneSettingsPage.hiddenTitles) {
                    PhoneRowTitle(title: L.settingsHiddenTitles)
                }
            } header: {
                PhoneSectionHeader(L.settingsGroupTitles)
            }
        }
        .navigationTitle(L.settingsGroupGeneral)
        .navigationBarTitleDisplayMode(.inline)
    }
}

// MARK: - Playback

struct PhonePlaybackPage: View {
    @Environment(AppStore.self) private var store
    @AppStorage(DevicePreferences.followSystemCaptions) private var followCaptions = true

    var body: some View {
        let overridden = store.settings.overriddenNote
        PhoneList {
            Section {
                PhoneSettingPicker(key: .translation, note: overridden(.translation))
                PhoneSubtitlesPicker()
                // Kept on this device: the style it follows is this device's.
                Toggle(isOn: $followCaptions) {
                    PhoneRowTitle(
                        title: L.settingsSystemCaptions,
                        note: L.settingsSystemCaptionsDetail)
                }
                .tint(.amber)
                .listRowBackground(Palette.raised)
                PhoneSettingPicker(key: .quality, note: overridden(.quality))
            } header: {
                PhoneSectionHeader(L.settingsLanguage)
            } footer: {
                Text(L.settingsAudioDetail)
            }
            Section {
                PhoneSettingSwitch(key: .autoNext)
                PhoneSettingPicker(key: .upNextCountdown)
                    .disabled(!store.settings.isOn(.autoNext))
                PhoneSettingSwitch(key: .skipOpening, note: overridden(.skipOpening))
                PhoneSettingSwitch(key: .skipFiller, note: overridden(.skipFiller))
                PhoneSettingSwitch(key: .markChapters)
                PhoneSettingSwitch(key: .skipButton, note: L.settingsSkipButtonDetail)
                PhoneSettingPicker(key: .commitThreshold)
                PhoneSettingSwitch(key: .spoilerProtection, note: L.showSettingsSpoilerDetail)
            } header: {
                PhoneSectionHeader(L.settingsGroupEpisodes)
            }
            Section {
                PhoneSettingPicker(key: .upscaling, note: upscalingNote)
                PhoneSettingSwitch(key: .nightMode, note: L.playerNightModeDetail)
            } header: {
                PhoneSectionHeader(L.playerPictureAndSound)
            }
        }
        .navigationTitle(L.settingsGroupPlayback)
        .navigationBarTitleDisplayMode(.inline)
    }

    private var upscalingNote: String {
        switch store.settings.row(.upscaling)?.option?.label {
        case "off": L.settingsUpscalingNoteNone
        case "sharp · Anime4K": L.settingsUpscalingNoteSharp
        default: L.settingsUpscalingNoteNatural
        }
    }
}

/// The subtitle language, by name.
struct PhoneSubtitlesPicker: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let code = store.settings.row(.subtitles)?.typedText ?? ""
        HStack {
            Text(L.playerSubtitles).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            Menu {
                Picker(L.playerSubtitles, selection: Binding(get: { code }, set: { store.setSetting(.subtitles, to: .str($0)) })) {
                    ForEach(SubtitlesMenu.languages, id: \.self) { Text(SubtitlesMenu.name($0)).tag($0) }
                    if !code.isEmpty, !SubtitlesMenu.languages.contains(code) {
                        Text(SubtitlesMenu.name(code)).tag(code)
                    }
                }
                .pickerStyle(.inline)
            } label: {
                PhoneMenuValue(text: code.isEmpty ? "–" : SubtitlesMenu.name(code))
            }
        }
        .listRowBackground(Palette.raised)
    }
}

// MARK: - Downloads

struct PhoneDownloadsPage: View {
    @Environment(AppStore.self) private var store
    @State private var confirmDelete = false
    @Namespace private var rotor

    var body: some View {
        let watched = WatchedDownloads(store)
        PhoneList {
            if store.downloads.loaded, store.downloads.groups.isEmpty {
                NoDownloads()
                    .frame(minHeight: 280)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
            }
            ForEach(store.downloads.groups) { group in
                Section {
                    ForEach(group.rows, id: \.id) { row in
                        PhoneDownloadRow(row: row)
                            .accessibilityRotorEntry(id: row.id, in: rotor)
                    }
                } header: {
                    PhoneSectionHeader(store.titles[group.id]?.showTitle ?? group.title)
                }
            }
            if !store.downloads.seeding.isEmpty {
                Section {
                    ForEach(Array(store.downloads.seeding.enumerated()), id: \.offset) { _, row in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(row.name).font(.ui(17)).foregroundStyle(.silk).lineLimit(2)
                            Text(SeedingList.line(row)).font(.ui(13)).foregroundStyle(.haze).monospacedDigit()
                        }
                        .accessibilityElement(children: .combine)
                        .listRowBackground(Palette.raised)
                    }
                } header: {
                    PhoneSectionHeader(L.downloadsSeedingAfterWatching)
                }
            }
            Section {
                PhoneSettingPicker(key: .downloadQuality, note: L.settingsQualityDetail)
                PhoneSettingPicker(key: .keepNextEpisodes, note: L.settingsKeepNextDetail)
                PhoneSettingSwitch(key: .downloadsOnCellular)
                PhoneSettingSwitch(key: .mergeSubtitles, note: L.settingsMergeSubtitlesDetail)
                PhoneSettingSwitch(key: .keepSeeding, note: L.settingsKeepSeedingDetail)
            } header: {
                PhoneSectionHeader(L.settingsGroupFiles)
            }
            Section {
                Button(L.downloadsClearFinished) { store.clearCompletedDownloads() }
                    .disabled(!store.downloads.groups.contains { $0.rows.contains { $0.state == .done } })
                    .listRowBackground(Palette.raised)
                Button(L.downloadsDeleteWatched, role: .destructive) { confirmDelete = true }
                    .disabled(watched.rows.isEmpty)
                    .listRowBackground(Palette.raised)
            } footer: {
                Text(FreeSpace.bytes.map { L.settingsStorageUsedFree(size: Wording.bytes(store.downloads.bytesOnDisk), free: Wording.bytes($0)) }
                     ?? L.settingsStorageUsed(size: Wording.bytes(store.downloads.bytesOnDisk)))
            }
        }
        .inProgressRotor(store.downloads.groups, titles: store.titles, in: rotor)
        .pullToRefresh(store, .downloads)
        .navigationTitle(L.navDownloads)
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog(L.settingsDeleteWatchedTitle(count: watched.rows.count), isPresented: $confirmDelete) {
            Button(L.actionDelete, role: .destructive) { for row in watched.rows { store.deleteDownload(row.id) } }
        }
        .task { store.loadDownloads() }
    }
}

/// One download in the iPhone's list: the episode, where it stands, and its actions on swipe.
struct PhoneDownloadRow: View {
    @Environment(AppStore.self) private var store
    let row: DownloadRow
    /// Whether Delete is asking first.
    @State private var deleting = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(L.downloadsEpisodeName(episode: Int(episode: row.episode))).font(.ui(17)).foregroundStyle(.silk)
                Spacer()
                if row.total > 0 {
                    Text(Wording.bytes(row.total)).font(.ui(15)).foregroundStyle(.haze).monospacedDigit()
                }
            }
            if row.state == .failed {
                FailedLine(status, size: 11)
                if let reason = row.error, !reason.isEmpty {
                    Text(reason).font(.ui(13)).foregroundStyle(.haze).lineLimit(3)
                }
            } else {
                Text(status).font(.ui(13)).foregroundStyle(.haze)
            }
            if row.state == .active || row.state == .paused, row.total > 0 {
                Capsule().fill(.rule)
                    .overlay(alignment: .leading) {
                        GeometryReader { proxy in
                            Capsule().fill(row.state == .active ? .water : .haze)
                                .frame(width: proxy.size.width * min(max(row.fraction, 0), 1))
                        }
                    }
                    .frame(height: 3)
            }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .listRowBackground(Palette.raised)
        // No full swipe: a file goes only by a tap on Delete, and Delete asks first.
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            switch row.state {
            case .done:
                Button(L.actionDelete, role: .destructive) { deleting = true }
            default:
                Button(L.actionCancel, role: .destructive) { store.cancelDownload(row.id) }
            }
        }
        // Untinted: the screen's one amber is not spent on a swipe (Johan kept these, decision 12).
        .swipeActions(edge: .leading) {
            switch row.state {
            case .done: Button(L.actionPlay) { store.playDownload(row) }
            case .active: Button(L.actionPause) { store.toggleDownload(row.id) }
            case .queued:
                Button(L.downloadsStartNow) { store.startDownloadNow(row.id) }
                Button(L.actionPause) { store.toggleDownload(row.id) }
            case .paused: Button(L.settingsSwipeResume) { store.toggleDownload(row.id) }
            case .failed: Button(L.settingsSwipeRetry) { store.retryDownload(row) }
            }
        }
        .confirmationDialog(L.downloadsDeleteTitle(name: L.downloadsEpisodeName(episode: Int(episode: row.episode))), isPresented: $deleting, titleVisibility: .visible) {
            Button(L.downloadsDeleteOneTitle, role: .destructive) { store.deleteDownload(row.id) }
            Button(L.actionCancel, role: .cancel) {}
        } message: {
            Text(L.downloadsDeleteOneDetail)
        }
    }

    private var status: String {
        switch row.state {
        case .done: L.downloadsCaptionDone
        case .active where row.total == 0: L.downloadsCaptionMetadata
        case .active:
            ([L.downloadsOfTotal(done: Wording.bytes(row.downloaded), total: Wording.bytes(row.total))]
                + (store.downloads.secondsLeft(row).map { [L.downloadsTimeLeftApple(left: Wording.countdown($0))] } ?? []))
                .joined(separator: " · ")
        case .paused: L.downloadsCaptionPaused(percent: row.fraction.formatted(.percent.precision(.fractionLength(0)).locale(AirTime.locale)))
        case .queued: L.downloadsCaptionQueuedSlot
        case .failed: L.downloadsCaptionFailed
        }
    }
}

// MARK: - Sources

struct PhoneSourcesPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var turningOn = false
    @State private var editingProxy = false

    var body: some View {
        let on = store.settings.isOn(.torrents)
        PhoneList {
            Section {
                NavigationLink(value: PhoneSettingsPage.providers) {
                    valueRow(L.sourcesSourceOrderRow, L.settingsSourceCount(count: store.providers.rows.count))
                }
                .listRowBackground(Palette.raised)
                NavigationLink(value: PhoneSettingsPage.stores) {
                    valueRow(L.navPluginStores, storesValue)
                }
                .listRowBackground(Palette.raised)
            }
            Section {
                Toggle(isOn: Binding(
                    get: { on },
                    set: { wanted in if wanted { turningOn = true } else { store.setSwitch(.torrents, on: false) } }
                )) {
                    PhoneRowTitle(title: SettingKey.torrents.title)
                }
                .tint(.amber)
                .listRowBackground(Palette.raised)
                if on {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(SettingKey.torrentIndex.title).font(.ui(17)).foregroundStyle(.silk)
                        TorrentIndexField(phone: true).foregroundStyle(.haze)
                    }
                    .listRowBackground(Palette.raised)
                    PhoneSettingSwitch(key: .torrentsOnCellular)
                    menuRow(L.torrentsProtectWith) { ProtectionMenu(phone: true) }
                    if let line = store.settings.torrent?.proxyLine {
                        Button { editingProxy = true } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(L.settingsEditProxy).font(.ui(17)).foregroundStyle(.silk)
                                Text(line).font(.ui(13)).foregroundStyle(.haze)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(.rect)
                        }
                        .listRowBackground(Palette.raised)
                        .onLaunchSheet("proxy", navigator) { editingProxy = true }
                    }
                    menuRow(SettingKey.vpnOnFailure.title) { FailedCheckMenu(phone: true) }
                    PhoneGuardRow()
                }
            } header: {
                PhoneSectionHeader(L.torrentsTitle)
            } footer: {
                Text(on ? L.torrentsCheckNote : L.torrentsOffDetail)
            }
            if on {
                Section {
                    PhoneSettingPicker(key: .seedRatio, note: L.settingsSeedRatioDetail)
                    PhoneSettingPicker(key: .seedHours)
                    PhoneSettingPicker(key: .uploadLimit)
                    PhoneSettingSwitch(key: .torrentsWhileCasting, note: CastWords.torrentsNote)
                } header: {
                    PhoneSectionHeader(L.downloadsSeedingTitle)
                }
            }
        }
        .navigationTitle(L.sourcesTitle)
        .navigationBarTitleDisplayMode(.inline)
        .sheet(isPresented: $turningOn) { PhoneTorrentSheet() }
        .sheet(isPresented: $editingProxy) { PhoneTorrentSheet(mode: .editProxy) }
        .onLaunchSheet("torrent", navigator) { turningOn = true }
        .task {
            store.checkProviders()
            if !store.stores.loaded { store.loadStores() }
        }
    }

    private var storesValue: String {
        let count = store.stores.stores.count
        let updates = store.stores.updates
        return updates > 0 ? L.settingsStoresValueUpdates(count: count, updates: L.storesUpdatesAvailable(count: updates)) : count.formatted(.number.locale(AirTime.locale))
    }

    private func valueRow(_ title: String, _ value: String) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            Text(value).font(.ui(17)).foregroundStyle(.haze)
        }
    }

    private func menuRow(_ title: String, @ViewBuilder menu: () -> some View) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            menu()
        }
        .listRowBackground(Palette.raised)
    }
}

/// The VPN guard's last word, in a list row.
struct PhoneGuardRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let (note, status, color) = GuardWords.words(store.providers.guardView, at: store.providers.guardAt)
        VStack(alignment: .leading, spacing: 6) {
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(L.torrentsLastCheck).font(.ui(17)).foregroundStyle(.silk)
                    Spacer()
                    HStack(spacing: 6) {
                        Circle().fill(color).frame(width: 8, height: 8)
                        Text(status).font(.ui(17)).foregroundStyle(.haze)
                    }
                }
                Text(note).font(.ui(13)).foregroundStyle(.haze)
            }
            .accessibilityElement(children: .combine)
            Button(L.torrentsCheckNow) { store.checkConnection() }
                .font(.ui(15, weight: .semibold))
                .buttonStyle(.borderless)
                .foregroundStyle(.silk)
                .disabled(store.providers.guardView == .checking)
        }
        .listRowBackground(Palette.raised)
    }
}

// MARK: - Accounts

struct PhoneAccountsPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var kitsu = false
    @State private var conflicts = false

    var body: some View {
        let anilist = store.accounts.tracker("anilist")
        PhoneList {
            Section {
                if let anilist, anilist.connected {
                    HStack(spacing: 14) {
                        Avatar(name: anilist.user, size: 40)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(anilist.user ?? L.settingsSignedIn).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                            HStack(spacing: 6) {
                                StateDot(state: AccountWords.dot(anilist, store.accounts))
                                Text(AccountWords.status(anilist, store.accounts))
                            }
                            .font(.ui(15))
                            .foregroundStyle(.haze)
                        }
                    }
                    .padding(.vertical, 4)
                    .listRowBackground(Palette.raised)
                    Button(L.accountsSyncNow) { store.syncNow() }
                        .fontWeight(.semibold)
                        .foregroundStyle(.silk)
                        .disabled(store.accounts.isWorking("sync"))
                        .listRowBackground(Palette.raised)
                    if !store.accounts.conflicts.isEmpty {
                        Button(L.settingsReviewDifferences(count: store.accounts.conflicts.count)) { conflicts = true }
                            .foregroundStyle(.silk)
                            .listRowBackground(Palette.raised)
                    }
                    Button(L.actionSignOut) { store.disconnect("anilist") }
                        .fontWeight(.semibold)
                        .foregroundStyle(.silk)
                        .listRowBackground(Palette.raised)
                } else {
                    HStack {
                        PhoneRowTitle(title: AccountsModel.name("anilist"), note: L.accountsBringsLists)
                        Spacer()
                        Button(L.actionSignIn) { store.connect("anilist") }
                            .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
                            .disabled(store.accounts.isWorking("anilist"))
                    }
                    .listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader(AccountsModel.name("anilist"))
            } footer: {
                Text(L.accountsLibraryFrom(tracker: AccountsModel.name("anilist")))
            }
            Section {
                PhoneSettingSwitch(key: .episodeDiscussions, note: L.settingsAppleDiscussionsNote)
                if store.discussionsOn {
                    PhoneSettingSwitch(key: .commentImages, note: L.settingsAppleCommentImagesNote)
                }
            } header: {
                PhoneSectionHeader(L.discussionSettingsGroup)
            }
            Section {
                ForEach(["mal", "kitsu", "simkl", "trakt"], id: \.self) { tracker in
                    PhoneTrackerRow(tracker: tracker, kitsu: $kitsu)
                }
            } header: {
                PhoneSectionHeader(L.accountsOtherTrackers)
            } footer: {
                Text(AccountWords.tokens(store.accounts))
            }
        }
        .navigationTitle(L.navAccounts)
        .navigationBarTitleDisplayMode(.inline)
        .sheet(isPresented: $kitsu) { KitsuSheet().presentationDetents([.medium]) }
        .onLaunchSheet("kitsu", navigator) { kitsu = true }
        .sheet(isPresented: $conflicts) { ConflictsSheet() }
    }
}

struct PhoneTrackerRow: View {
    @Environment(AppStore.self) private var store
    let tracker: String
    @Binding var kitsu: Bool

    var body: some View {
        let state = store.accounts.tracker(tracker)
        HStack {
            PhoneRowTitle(title: AccountsModel.name(tracker),
                          note: state?.connected == true ? AccountWords.status(state!, store.accounts) : nil)
            Spacer()
            Group {
                if state?.connected == true {
                    Button(L.actionSignOut) { store.disconnect(tracker) }
                } else {
                    switch tracker {
                    case "kitsu": Button(L.actionSignIn) { kitsu = true }
                    default:
                        // MyAnimeList through the system's sign-in sheet, Simkl and Trakt with a
                        // code to type on another device.
                        Button(L.actionSignIn) { store.connect(tracker) }.disabled(store.accounts.isWorking(tracker))
                    }
                }
            }
            .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
        }
        .listRowBackground(Palette.raised)
    }
}
#endif
