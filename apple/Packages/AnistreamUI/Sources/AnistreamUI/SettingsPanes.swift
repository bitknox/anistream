// Settings' Downloads, Sources and Accounts panes, on the Mac and a wide iPad.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import Network
import SwiftUI

// MARK: - Downloads

struct DownloadsPane: View {
    @Environment(AppStore.self) private var store
    @State private var confirmDelete = false

    var body: some View {
        SettingsGroup(title: L.settingsGroupFiles) {
            SettingsRow(title: SettingKey.downloadDir.title, note: DownloadFolder.note(store)) {
                DownloadFolderMenu()
            }
            SettingsRow(title: SettingKey.downloadQuality.title, note: L.settingsQualityDetail) {
                SettingMenu(key: .downloadQuality)
            }
            SettingsRow(title: SettingKey.keepNextEpisodes.title, note: L.settingsKeepNextDetail) {
                SettingMenu(key: .keepNextEpisodes)
            }
            SettingsRow(title: SettingKey.mergeSubtitles.title, note: L.settingsMergeSubtitlesDetail) {
                SettingSwitch(key: .mergeSubtitles)
            }
            SettingsRow(title: SettingKey.keepSeeding.title, note: L.settingsKeepSeedingDetailMac) {
                SettingSwitch(key: .keepSeeding)
            }
        }
        #if os(macOS)
        SettingsGroup(title: L.settingsAfterDownload, footnote: nil) {
            HookRow()
        }
        SettingsFootnote(
            Text("""
                Runs once a download is moved and its subtitles merged, with \(code("ANISTREAM_PATH")), \
                \(code("ANISTREAM_TITLE")) and \(code("ANISTREAM_EPISODE")) set. A command that fails never \
                affects the download.
                """)
        )
        .padding(.top, -14)
        #endif
        let watched = WatchedDownloads(store)
        SettingsGroup {
            SettingsRow(title: L.settingsWatchedDownloads, note: watched.line) {
                Button(L.downloadsDeleteWatched) { confirmDelete = true }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    // Whole in a narrow pane, as on an iPad in portrait: the label wraps instead.
                    .fixedSize()
                    .disabled(watched.rows.isEmpty)
            }
        }
        .confirmationDialog(
            L.settingsDeleteWatchedTitle(count: watched.rows.count),
            isPresented: $confirmDelete
        ) {
            Button(L.actionDelete, role: .destructive) {
                for row in watched.rows { store.deleteDownload(row.id) }
            }
        } message: {
            Text(L.settingsDeleteWatchedDetail)
        }
    }

    private func code(_ text: String) -> Text {
        Text(text).font(.mac(11, design: .monospaced)).foregroundStyle(.silk)
    }
}

/// Where downloads go, as the row says it.
@MainActor
enum DownloadFolder {
    /// The folder's path with the home folder as `~`, or where they go when none is set.
    static func note(_ store: AppStore) -> String {
        guard let path = store.settings.row(.downloadDir)?.typedText, !path.isEmpty else {
            return L.settingsFolderDefault
        }
        let home = NSHomeDirectory()
        return path.hasPrefix(home) ? "~" + path.dropFirst(home.count) : path
    }

    #if os(macOS)
    /// Show the folder in the Finder, when one is set.
    static func show(_ store: AppStore) {
        guard let path = store.settings.row(.downloadDir)?.typedText, !path.isEmpty else { return }
        NSWorkspace.shared.open(URL(fileURLWithPath: path))
    }
    #endif

    /// The folder's own name, for the menu.
    static func name(_ store: AppStore) -> String {
        guard let path = store.settings.row(.downloadDir)?.typedText, !path.isEmpty else { return L.valueDefault }
        return URL(fileURLWithPath: path).lastPathComponent
    }
}

/// The download folder's menu: choose another, or show it.
struct DownloadFolderMenu: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Menu {
            #if os(macOS)
            Button(L.settingsChoose) { choose() }
            if let path = store.settings.row(.downloadDir)?.typedText, !path.isEmpty {
                Button(L.shellShowInFinder) {
                    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                }
            }
            #else
            Text(DownloadFolder.note(store))
            #endif
        } label: {
            MenuCapsule(text: DownloadFolder.name(store), symbol: "folder")
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityLabel(L.downloadsFolder)
    }

    #if os(macOS)
    private func choose() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = L.settingsFolderChoose
        if panel.runModal() == .OK, let url = panel.url {
            store.setSetting(.downloadDir, to: .str(url.path))
        }
    }
    #endif
}

#if os(macOS)
/// The command run after each download, typed, and set when it is submitted or left.
struct HookRow: View {
    @Environment(AppStore.self) private var store
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 16) {
            Text(L.settingsHookCommand)
                .font(.mac(13))
                .foregroundStyle(.silk)
                .frame(width: 196, alignment: .leading)
            TextField(L.settingsHookCommand, text: $text, prompt: Text(L.valueNone))
                .textFieldStyle(.plain)
                .font(.mac(13, design: .monospaced))
                .padding(.horizontal, 10)
                .frame(height: 28)
                .background(.ground, in: .rect(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.glassRule, lineWidth: 1))
                .focused($focused)
                .onSubmit(save)
                .labelsHidden()
        }
        .padding(EdgeInsets(top: 4, leading: 10, bottom: 4, trailing: 6))
        .frame(minHeight: 36)
        .padding(.horizontal, 4)
        .onAppear { text = store.settings.row(.downloadHook)?.typedText ?? "" }
        .onChange(of: focused) { _, now in if !now { save() } }
    }

    private func save() {
        let saved = store.settings.row(.downloadHook)?.typedText ?? ""
        guard text != saved else { return }
        store.setSetting(.downloadHook, to: .str(text))
    }
}
#endif

/// The finished downloads of episodes already watched.
struct WatchedDownloads {
    let rows: [DownloadRow]
    let line: String

    @MainActor
    init(_ store: AppStore) {
        rows = store.downloads.rows.filter { $0.state == .done && $0.isWatched(in: store.titles) }
        guard !rows.isEmpty else {
            line = L.settingsNoneYet
            return
        }
        let size = Wording.bytes(rows.reduce(0) { $0 + $1.bytesOnDisk })
        let titles = Set(rows.map(\.anilistId))
        let episodes = L.episodesCount(count: rows.count)
        line = titles.count == 1 ? L.settingsWatchedOne(episodes: episodes, title: rows[0].title, size: size) : L.settingsWatchedMany(episodes: episodes, size: size)
    }
}

extension DownloadRow {
    /// Whether its episode is among those watched, as far as the title's progress says.
    @MainActor
    func isWatched(in titles: TitleCache) -> Bool {
        guard let done = titles[anilistId]?.progress?.done, let number = Double(episode) else { return false }
        return number <= Double(done)
    }
}

/// What downloads take, watched and not, and what is free on the disk.
struct StorageAside: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let watched = WatchedDownloads(store).rows.reduce(UInt64(0)) { $0 + $1.bytesOnDisk }
        let total = store.downloads.bytesOnDisk
        let unwatched = total - min(watched, total)
        Text(L.settingsGroupStorage).groupHeaderStyle().macLineHeight(14)
            .accessibilityAddTraits(.isHeader)
        GeometryReader { proxy in
            HStack(spacing: 2) {
                if total > 0 {
                    Capsule().fill(.silk)
                        .frame(width: max(proxy.size.width * CGFloat(Double(unwatched) / Double(total)) - 1, 0))
                    Capsule().fill(.haze)
                } else {
                    Capsule().fill(.rule)
                }
            }
        }
        .frame(height: 8)
        .accessibilityHidden(true)
        legend(L.settingsStorageUnwatched, unwatched, .silk)
        legend(L.episodeWatched, watched, .haze)
        Text(summary(total))
            .font(.mac(13))
            .macLineHeight(18)
            .foregroundStyle(.silk)
    }

    private func legend(_ label: String, _ bytes: UInt64, _ color: Color) -> some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 9, height: 9)
            Text(label).foregroundStyle(.silk)
            Spacer()
            Text(Wording.bytes(bytes)).foregroundStyle(.haze).monospacedDigit()
        }
        .font(.mac(13))
    }

    private func summary(_ total: UInt64) -> String {
        guard let free = FreeSpace.bytes else { return L.settingsStorageUsed(size: Wording.bytes(total)) }
        return L.settingsStorageUsedFree(size: Wording.bytes(total), free: Wording.bytes(free))
    }
}

/// What is free on the disk the app's data is on.
enum FreeSpace {
    static var bytes: UInt64? {
        let values = try? URL(fileURLWithPath: NSHomeDirectory())
            .resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
        return values?.volumeAvailableCapacityForImportantUsage.map { UInt64(max($0, 0)) }
    }
}

// MARK: - Sources

struct SourcesPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var turningOn = false
    /// Whether this device has a cellular connection to choose about: an iPad with one.
    @State private var cellular = false

    var body: some View {
        let on = store.settings.isOn(.torrents)
        SettingsGroup(title: L.sourcesTitle, footnote: L.legalHostsNoContent) {
            Button {
                navigator.open(.providers)
            } label: {
                SettingsRow(title: SettingKey.sourceOrder.title, note: store.settings.overriddenNote(.sourceOrder)) {
                    HStack(spacing: 8) {
                        Text(L.settingsSourceCount(count: store.providers.rows.count))
                            .font(.mac(13))
                            .foregroundStyle(.haze)
                        Image(systemName: "chevron.right")
                            .font(.mac(11, weight: .semibold))
                            .foregroundStyle(.haze)
                    }
                    .padding(.trailing, 6)
                }
            }
            .buttonStyle(.plain)
        }
        SettingsGroup(
            title: L.torrentsTitle,
            footnote: on ? L.settingsTorrentsWarnNote : L.torrentsOffDetail
        ) {
            SettingsRow(title: SettingKey.torrents.title) {
                Toggle(SettingKey.torrents.title, isOn: Binding(
                    get: { on },
                    set: { wanted in
                        if wanted { turningOn = true } else { store.setSwitch(.torrents, on: false) }
                    }
                ))
                .toggleStyle(.switch)
                .tint(.amber)
                .labelsHidden()
            }
            if on {
                SettingsRow(title: SettingKey.torrentIndex.title, note: L.torrentsIndexDetail + " " + L.legalHostsNoContent) {
                    TorrentIndexField()
                        .padding(.horizontal, 10)
                        .frame(width: 280, height: 28)
                        .background(.ground, in: .rect(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.glassRule, lineWidth: 1))
                }
                if cellular {
                    SettingsRow(title: SettingKey.torrentsOnCellular.title) { SettingSwitch(key: .torrentsOnCellular) }
                }
                // Beside the choice, once a proxy is set: where it is, and its form to edit.
                SettingsRow(title: L.torrentsProtectWith, note: store.settings.torrent?.proxyLine) {
                    HStack(spacing: 10) {
                        EditProxyButton()
                        ProtectionMenu()
                    }
                }
                GuardRow()
                SettingsRow(title: SettingKey.vpnOnFailure.title) { FailedCheckMenu() }
            }
        }
        if on {
            SettingsGroup(title: L.downloadsSeedingTitle, footnote: L.settingsSeedingOpenOnly) {
                SettingsRow(title: SettingKey.seedRatio.title) { SettingMenu(key: .seedRatio) }
                SettingsRow(title: SettingKey.seedHours.title) { SettingMenu(key: .seedHours) }
                SettingsRow(title: SettingKey.uploadLimit.title) { SettingMenu(key: .uploadLimit) }
            }
        }
        Color.clear.frame(height: 0)
            .sheet(isPresented: $turningOn) { TorrentSheet() }
            .onLaunchSheet("torrent", navigator) { turningOn = true }
            #if os(iOS)
            .task { cellular = await CellularCheck.available() }
            #endif
    }
}

#if os(iOS)
/// Whether the device has a cellular interface the system could use, from one look at its paths.
enum CellularCheck {
    static func available() async -> Bool {
        let monitor = NWPathMonitor()
        let queue = DispatchQueue(label: "anistream.cellular-check") // i18n-skip
        let path: NWPath = await withCheckedContinuation { done in
            // Called on the serial queue: the first path answers, and later ones are dropped.
            nonisolated(unsafe) var answered = false
            monitor.pathUpdateHandler = { path in
                guard !answered else { return }
                answered = true
                done.resume(returning: path)
            }
            monitor.start(queue: queue)
        }
        monitor.cancel()
        return path.availableInterfaces.contains { $0.type == .cellular }
    }
}
#endif

/// The VPN guard's last word: where traffic left through and when, and whether it passed.
struct GuardRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        TimelineView(.periodic(from: .now, by: 30)) { context in
            let (note, status, color) = words(now: context.date)
            // The guard checks again on its own every minute; no command runs it sooner.
            SettingsRow(title: L.torrentsLastCheck, note: note) {
                HStack(spacing: 6) {
                    Circle().fill(color).frame(width: 6, height: 6)
                    Text(status).font(.mac(13)).foregroundStyle(.silk)
                }
            }
        }
    }

    private func words(now: Date) -> (String, String, Color) {
        GuardWords.words(store.providers.guardView, at: store.providers.guardAt, now: now)
    }
}

/// The VPN guard's verdict in words: where traffic left through and when, a word for how it
/// stands, and the dot's colour.
enum GuardWords {
    static func words(_ view: GuardView?, at: Date?, now: Date = .now) -> (String, String, Color) {
        let ago = at.map { Wording.ago(Int64(now.timeIntervalSince($0))) }
        switch view {
        case .protected(let exit)?:
            return (L.torrentsCheckPassedExit(exit: exit) + (ago.map { " · \($0)" } ?? ""), L.torrentsCheckPassed, .water)
        case .checking?:
            return (L.torrentsCheckRunningLine, L.torrentsCheckRunning, .haze)
        case .leaking?:
            return (L.torrentsCheckLeaked(then: L.torrentsFailPauseResult), L.torrentsCheckFailed, .vermilion)
        case .unreachable?:
            return (L.torrentsCheckUnmade(then: L.torrentsFailPauseResult), L.torrentsCheckFailed, .vermilion)
        case .unprotected?:
            return (L.torrentsCheckNoneLine, L.torrentsCheckNone, .haze)
        case nil:
            return (L.torrentsCheckWaitingLine, L.torrentsCheckWaiting, .haze)
        }
    }
}

// MARK: - Accounts

struct AccountsPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.openURL) private var openURL
    @State private var sheet: AccountSheet?

    var body: some View {
        let anilist = store.accounts.tracker("anilist")
        SettingsGroup(
            title: AccountsModel.name("anilist"),
            footnote: L.settingsAccountsSyncNote(tracker: AccountsModel.name("anilist"))
        ) {
            if let anilist, anilist.connected {
                AccountRow(state: anilist) {
                    HStack(spacing: 8) {
                        Working(store.accounts.isWorking("sync"))
                        Button(L.accountsSyncNow) { store.syncNow() }
                            .disabled(store.accounts.isWorking("sync"))
                        Button(L.actionSignOut) { store.disconnect("anilist") }
                    }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
                if !store.accounts.conflicts.isEmpty {
                    SettingsRow(title: L.settingsConflicts(count: store.accounts.conflicts.count, tracker: AccountsModel.name("anilist"))) {
                        Button(L.settingsReview) { sheet = .conflicts }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
            } else {
                SettingsRow(title: AccountsModel.name("anilist"), note: L.accountsBringsLists) {
                    HStack(spacing: 8) {
                        Working(store.accounts.isWorking("anilist"))
                        Button(L.actionSignIn) { store.connect("anilist") }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                            .disabled(store.accounts.isWorking("anilist"))
                    }
                }
            }
        }
        SettingsGroup(title: L.accountsOtherTrackers, footnote: AccountWords.tokens(store.accounts)) {
            ForEach(["mal", "kitsu", "simkl", "trakt"], id: \.self) { tracker in
                TrackerRow(tracker: tracker, sheet: $sheet)
            }
        }
        SettingsGroup {
            SettingsRow(title: L.settingsPrivacy, note: L.settingsPrivacyNote) {
                Button(L.settingsReadOnline) { openURL(AppVersion.privacy) }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
        Color.clear.frame(height: 0)
            .sheet(item: $sheet) { sheet in
                switch sheet {
                case .kitsu: KitsuSheet()
                case .conflicts: ConflictsSheet()
                }
            }
            .onLaunchSheet("kitsu", navigator) { sheet = .kitsu }
    }
}

/// The signed-in account: its name, and how its sync stands beside the dot.
struct AccountRow<Accessory: View>: View {
    @Environment(AppStore.self) private var store
    let state: SyncState
    @ViewBuilder let accessory: Accessory

    var body: some View {
        TimelineView(.periodic(from: .now, by: 30)) { context in
            HStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 2) {
                    // The account's own name, once the tracker says it.
                    Text(state.user ?? L.settingsSignedIn)
                        .font(.mac(13, weight: .semibold))
                        .foregroundStyle(.silk)
                    HStack(spacing: 6) {
                        StateDot(state: AccountWords.dot(state, store.accounts))
                        Text(AccountWords.status(state, store.accounts, now: context.date))
                            .font(.mac(12))
                            .foregroundStyle(.haze)
                    }
                }
                .macLineHeight(16)
                Spacer(minLength: 0)
                accessory
            }
            .padding(EdgeInsets(top: 8, leading: 10, bottom: 8, trailing: 6))
            .frame(minHeight: 50)
            .padding(.horizontal, 4)
            .accessibilityElement(children: .combine)
        }
    }
}

/// A sheet an account row opens.
enum AccountSheet: String, Identifiable {
    case kitsu, conflicts
    var id: String { rawValue }
}

/// One of the other trackers: signed in, with Sign Out, or not, with how to sign in.
struct TrackerRow: View {
    @Environment(AppStore.self) private var store
    let tracker: String
    @Binding var sheet: AccountSheet?

    var body: some View {
        let state = store.accounts.tracker(tracker)
        SettingsRow(
            title: AccountsModel.name(tracker),
            note: state?.connected == true ? AccountWords.status(state!, store.accounts) : nil
        ) {
            Group {
                if state?.connected == true {
                    Button(L.actionSignOut) { store.disconnect(tracker) }
                } else {
                    switch tracker {
                    case "kitsu":
                        Button(L.actionSignIn) { sheet = .kitsu }
                    default:
                        // MyAnimeList through the system's sign-in sheet, Simkl and Trakt with a
                        // code to type on another device.
                        HStack(spacing: 8) {
                            Working(store.accounts.isWorking(tracker))
                            Button(L.actionSignIn) { store.connect(tracker) }
                                .disabled(store.accounts.isWorking(tracker))
                        }
                    }
                }
            }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
        }
    }
}

/// How an account stands, in a line.
@MainActor
enum AccountWords {
    @MainActor
    static func status(_ state: SyncState, _ accounts: AccountsModel, now: Date = .now) -> String {
        if state.needsReauth { return L.accountsStateExpiredShort }
        if let error = state.pullError { return L.accountsSyncFailed(why: error.reason) }
        if accounts.isWorking("sync") { return L.accountsSyncRunning }
        if state.outbox > 0 { return L.settingsChangesWaiting(count: Int(state.outbox)) }
        if let at = accounts.syncedAt[state.tracker] {
            let seconds = Int64(now.timeIntervalSince(at))
            return seconds < 60 ? L.settingsSyncedJustNow : L.settingsSyncedAgo(ago: Wording.ago(seconds))
        }
        // Nothing waiting and nothing failed, as the sync capsule says it.
        return L.accountsSyncDone
    }

    /// The dot beside the status: water once synced, vermilion when sync is failing.
    static func dot(_ state: SyncState, _ accounts: AccountsModel) -> SyncSummary.State {
        if state.needsReauth || state.pullError != nil { return .problem }
        return state.outbox == 0 && !accounts.isWorking("sync") ? .synced : .waiting
    }

    /// Where the sign-in tokens are kept, as the trackers say it.
    @MainActor
    static func tokens(_ accounts: AccountsModel) -> String {
        let storage = accounts.trackers.compactMap(\.storage).first ?? "keychain"
        return storage.lowercased().contains("file") ? L.accountsTokensFile : L.accountsTokensKeychain
    }
}

// MARK: - Integrations

/// TVs, Discord and updates (MacSettingsIntegrations): the engine's integrations settings, which
/// the Mac shares with the terminal app's config. A watch party's server and room are chosen per
/// party, in the form that starts it.
struct IntegrationsPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let presence = store.settings.isOn(.presence)
        let titled = store.settings.isOn(.presenceTitle)
        TVsGroup()
        // Only where the engine has a presence to show: a phone's has none.
        if store.settings.hasPresence {
            SettingsGroup(
                title: L.settingsDiscord,
                footnote: L.settingsPresenceNote(tracker: AccountsModel.name("anilist"))
            ) {
                SettingsRow(title: SettingKey.presence.title) { SettingSwitch(key: .presence) }
                SettingsRow(title: SettingKey.presenceTitle.title) { SettingSwitch(key: .presenceTitle) }
                    .disabled(!presence)
                    .opacity(presence ? 1 : 0.45)
                SettingsRow(title: SettingKey.presenceRich.title) { SettingSwitch(key: .presenceRich) }
                    .disabled(!presence || !titled)
                    .opacity(presence && titled ? 1 : 0.45)
            }
        }
        UpdatesGroup()
        AboutGroup()
    }
}

/// The app's version as its bundle says it: "1.1.0", and "Version 1.1.0 (1)" with the build.
enum AppVersion {
    static var short: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? ""
    }

    static var long: String {
        let build = Bundle.main.infoDictionary?["CFBundleVersion"] as? String
        return L.aboutVersion(version: short + (build.map { " (\($0))" } ?? ""))
    }

    /// The website, which About opens.
    static let website = URL(string: "https://anistream.tv")!

    /// The privacy page: what anistream sends, where, and what it keeps on the device.
    static let privacy = URL(string: "https://anistream.tv/docs/privacy/")!
}

/// What Discord is sent for an episode, as the three switches leave it: nothing, the plain
/// presence, or the rich one. The preview draws this, so what it shows is what goes out.
struct PresenceSketch: Equatable {
    enum Mode: Equatable { case off, plain, rich }

    var mode: Mode
    /// The line above the episode: the show, or only that something is playing.
    var heading: String
    /// The episode line, empty when the title is kept off.
    var detail: String
    /// Whether the cover, the bar and the AniList button go with it.
    var cover: Bool { mode == .rich }
    var bar: Bool { mode == .rich }
    var button: String? { mode == .rich ? L.settingsPresenceButton(tracker: AccountsModel.name("anilist")) : nil }

    init(enabled: Bool, titled: Bool, rich: Bool, show: String, episode: Int, total: Int?) {
        // The rich presence names the show on every part of it, so it follows the title.
        mode = !enabled ? .off : (rich && titled ? .rich : .plain)
        heading = titled ? show : L.settingsPresenceNothing
        guard titled else { detail = ""; return }
        if mode == .rich, let total, episode <= total {
            detail = L.settingsPresenceEpisodeOf(episode: episode, total: total)
        } else {
            detail = L.settingsPresenceEpisode(episode: episode)
        }
    }
}

/// What Discord shows while an episode plays: the show and the episode, or only "Watching anime"
/// when the title is kept off it, and with Rich presence the cover, a progress bar and a button.
struct PresencePreview: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let example = store.continuing.queue(store.titles).all.first?.entry
        let sketch = PresenceSketch(
            enabled: store.settings.isOn(.presence), titled: store.settings.isOn(.presenceTitle),
            rich: store.settings.isOn(.presenceRich), show: example?.showTitle ?? "Hoshikuzu no Tabibito", // i18n-skip: a made-up sample title
            episode: Int(example?.progress?.next ?? 7), total: example?.episodes.map(Int.init)
        )
        VStack(alignment: .leading, spacing: 12) {
            Text(L.settingsDiscordPresence).groupHeaderStyle().macLineHeight(14)
                .accessibilityAddTraits(.isHeader)
            HStack(alignment: .top, spacing: 10) {
                if sketch.cover {
                    RoundedRectangle(cornerRadius: 6)
                        .fill(.haze.opacity(0.3))
                        .frame(width: 48, height: 68)
                        .overlay(Text(L.settingsPresenceCover).font(.mac(11)).foregroundStyle(.haze))
                        .accessibilityHidden(true)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(sketch.heading)
                        .font(.mac(13, weight: .semibold))
                        .foregroundStyle(.silk)
                    if !sketch.detail.isEmpty {
                        Text(sketch.detail).font(.mac(12)).foregroundStyle(.haze)
                    }
                    if sketch.bar {
                        Capsule().fill(.haze.opacity(0.3)).frame(width: 120, height: 4)
                            .overlay(alignment: .leading) { Capsule().fill(.silk).frame(width: 40, height: 4) }
                            .padding(.top, 4)
                            .accessibilityHidden(true)
                    }
                    if let button = sketch.button {
                        Text(button).font(.mac(12, weight: .semibold)).foregroundStyle(.silk)
                            .padding(.horizontal, 10).padding(.vertical, 4)
                            .background(.haze.opacity(0.25), in: RoundedRectangle(cornerRadius: 5))
                            .padding(.top, 6)
                    }
                }
            }
            .opacity(sketch.mode == .off ? 0.6 : 1)
            Text(sketch.mode != .off ? L.settingsPresenceOn : L.settingsPresenceOff)
                .font(.mac(13))
                .macLineHeight(18)
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// A typed setting in a field: what the file holds, sent when the field is left or Return is
/// pressed.
struct TypedSettingField: View {
    @Environment(AppStore.self) private var store
    let key: SettingKey
    let prompt: String
    var width: CGFloat = 196
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextField(key.title, text: $text, prompt: Text(prompt))
            .textFieldStyle(.plain)
            .font(.mac(13, design: .monospaced))
            .padding(.horizontal, 10)
            .frame(width: width, height: 28)
            .background(.ground, in: .rect(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.glassRule, lineWidth: 1))
            .focused($focused)
            .onSubmit(save)
            .labelsHidden()
            .onAppear { text = store.settings.row(key)?.typedText ?? "" }
            .onChange(of: focused) { _, now in if !now { save() } }
    }

    private func save() {
        let saved = store.settings.row(key)?.typedText ?? ""
        guard text != saved else { return }
        store.setSetting(key, to: .str(text))
    }
}
