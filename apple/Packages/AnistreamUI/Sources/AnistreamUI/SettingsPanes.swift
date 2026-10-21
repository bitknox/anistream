// Settings' Downloads, Sources and Accounts panes, on the Mac and a wide iPad.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

// MARK: - Downloads

struct DownloadsPane: View {
    @Environment(AppStore.self) private var store
    @State private var confirmDelete = false

    var body: some View {
        SettingsGroup(title: "Files") {
            SettingsRow(title: SettingKey.downloadDir.title, note: DownloadFolder.note(store)) {
                DownloadFolderMenu()
            }
            // E8: `DownloadQuality` is not a setting the engine has yet.
            SettingsRow(title: "Quality", note: "Higher takes the best version a source offers, and more space.") {
                DeferredMenu(text: "Standard", waitsOn: "E8")
            }
            SettingsRow(title: SettingKey.mergeSubtitles.title, note: "Subtitles stay with the episode wherever the file goes.") {
                SettingSwitch(key: .mergeSubtitles)
            }
            SettingsRow(title: SettingKey.keepSeeding.title, note: "A download that keeps seeding stays in anistream’s cache, not the download folder.") {
                SettingSwitch(key: .keepSeeding)
            }
        }
        #if os(macOS)
        SettingsGroup(title: "After each download", footnote: nil) {
            HookRow()
        }
        SettingsFootnote(
            Text("Runs once a download is moved and its subtitles merged, with ")
                + code("ANISTREAM_PATH") + Text(", ") + code("ANISTREAM_TITLE") + Text(" and ")
                + code("ANISTREAM_EPISODE")
                + Text(" set. A command that fails never affects the download.")
        )
        .padding(.top, -14)
        #endif
        let watched = WatchedDownloads(store)
        SettingsGroup {
            SettingsRow(title: "Watched downloads", note: watched.line) {
                Button("Delete Watched Downloads…") { confirmDelete = true }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .disabled(watched.rows.isEmpty)
            }
        }
        .confirmationDialog(
            "Delete \(Wording.plural(watched.rows.count, "watched download"))?",
            isPresented: $confirmDelete
        ) {
            Button("Delete", role: .destructive) {
                for row in watched.rows { store.deleteDownload(row.id) }
            }
        } message: {
            Text("The files are deleted from this \(Device.name). Episodes not yet watched stay.")
        }
    }

    private func code(_ text: String) -> Text {
        Text(text).font(.system(size: 11, design: .monospaced)).foregroundStyle(.silk)
    }
}

/// Where downloads go, as the row says it.
@MainActor
enum DownloadFolder {
    /// The folder's path with the home folder as `~`, or where they go when none is set.
    static func note(_ store: AppStore) -> String {
        guard let path = store.settings.row(.downloadDir)?.typedText, !path.isEmpty else {
            return "Kept with the torrent cache"
        }
        let home = NSHomeDirectory()
        return path.hasPrefix(home) ? "~" + path.dropFirst(home.count) : path
    }

    /// The folder's own name, for the menu.
    static func name(_ store: AppStore) -> String {
        guard let path = store.settings.row(.downloadDir)?.typedText, !path.isEmpty else { return "Default" }
        return URL(fileURLWithPath: path).lastPathComponent
    }
}

/// The download folder's menu: choose another, or show it.
struct DownloadFolderMenu: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Menu {
            #if os(macOS)
            Button("Choose…") { choose() }
            if let path = store.settings.row(.downloadDir)?.typedText, !path.isEmpty {
                Button("Show in Finder") {
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
        .accessibilityLabel("Download folder")
    }

    #if os(macOS)
    private func choose() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Choose"
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
            Text("Command")
                .font(.system(size: 13))
                .foregroundStyle(.silk)
                .frame(width: 196, alignment: .leading)
            TextField("Command", text: $text, prompt: Text("None"))
                .textFieldStyle(.plain)
                .font(.system(size: 13, design: .monospaced))
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
            line = "None yet"
            return
        }
        let size = Wording.bytes(rows.reduce(0) { $0 + $1.bytesOnDisk })
        let titles = Set(rows.map(\.anilistId))
        let episodes = Wording.plural(rows.count, "episode")
        line = titles.count == 1 ? "\(episodes) of \(rows[0].title) · \(size)" : "\(episodes) · \(size)"
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
        Text("Storage").groupHeaderStyle().fixedLineHeight(14)
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
        legend("Not watched yet", unwatched, .silk)
        legend("Watched", watched, .haze)
        Text(summary(total))
            .font(.system(size: 13))
            .fixedLineHeight(18)
            .foregroundStyle(.silk)
    }

    private func legend(_ label: String, _ bytes: UInt64, _ color: Color) -> some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 9, height: 9)
            Text(label).foregroundStyle(.silk)
            Spacer()
            Text(Wording.bytes(bytes)).foregroundStyle(.haze).monospacedDigit()
        }
        .font(.system(size: 13))
    }

    private func summary(_ total: UInt64) -> String {
        let used = "\(Wording.bytes(total)) in downloads."
        guard let free = FreeSpace.bytes else { return used }
        return "\(used) \(Wording.bytes(free)) free on this \(Device.name)."
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

    var body: some View {
        let on = store.settings.isOn(.torrents)
        SettingsGroup(title: "Sources", footnote: "anistream ships no sources of its own and hosts no content. Every source is external.") {
            Button {
                navigator.open(.providers)
            } label: {
                // "Overridden for 1 show" waits on E8's override counts.
                SettingsRow(title: SettingKey.sourceOrder.title) {
                    HStack(spacing: 8) {
                        Text(Wording.plural(store.providers.rows.count, "source"))
                            .font(.system(size: 13))
                            .foregroundStyle(.haze)
                        Image(systemName: "chevron.right")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(.haze)
                    }
                    .padding(.trailing, 6)
                }
            }
            .buttonStyle(.plain)
        }
        SettingsGroup(
            title: "Torrents",
            footnote: on
                ? "Choosing Warn only asks you to confirm first, as No protection does."
                : "Off by default. Turning this on asks how to protect your connection first."
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
                // E9: the torrent index's address, `rss_url`, is not in the protocol yet.
                SettingsRow(title: "Torrent index", note: "The RSS search address anistream asks for torrents. anistream ships none.") {
                    TextField("Torrent index", text: .constant(""), prompt: Text("https://…/?q={query}"))
                        .textFieldStyle(.plain)
                        .font(.system(size: 13, design: .monospaced))
                        .padding(.horizontal, 10)
                        .frame(width: 280, height: 28)
                        .background(.ground, in: .rect(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.glassRule, lineWidth: 1))
                        .disabled(true)
                        .help("Comes with engine step E9")
                }
                // E8: `SetTorrentProtection` changes this; the file's own choice shows meanwhile.
                SettingsRow(title: "Protect with") {
                    DeferredMenu(text: Protection.label(store.settings.value(.vpnMode)), waitsOn: "E8")
                }
                GuardRow()
                // E8: `VpnOnFailure` is not a setting the engine has yet.
                SettingsRow(title: "If a check fails") { DeferredMenu(text: "Pause torrents", waitsOn: "E8") }
            }
        }
        if on {
            SettingsGroup(title: "Seeding", footnote: "Seeding runs only while anistream is open.") {
                SettingsRow(title: SettingKey.seedRatio.title) { SettingMenu(key: .seedRatio) }
                SettingsRow(title: SettingKey.seedHours.title) { SettingMenu(key: .seedHours) }
                SettingsRow(title: SettingKey.uploadLimit.title) { SettingMenu(key: .uploadLimit) }
            }
        }
        Color.clear.frame(height: 0)
            .sheet(isPresented: $turningOn) { TorrentSheet() }
            .onLaunchSheet("torrent", navigator) { turningOn = true }
    }
}

/// What protects torrent traffic, in the words the turn-on sheet gives each choice.
enum Protection {
    static func label(_ mode: String?) -> String {
        switch mode {
        case "socks5": "SOCKS5 proxy"
        case "external": "VPN on this device"
        case "none": "No protection"
        default: "–"
        }
    }
}

/// The VPN guard's last word: where traffic left through and when, and whether it passed.
struct GuardRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        TimelineView(.periodic(from: .now, by: 30)) { context in
            let (note, status, color) = words(now: context.date)
            SettingsRow(title: "Last check", note: note, selected: true) {
                HStack(spacing: 12) {
                    HStack(spacing: 6) {
                        Circle().fill(color).frame(width: 6, height: 6)
                        Text(status).font(.system(size: 13)).foregroundStyle(.silk)
                    }
                    // The guard re-checks on its own every minute; no command re-runs it now.
                    Button("Check Now") {}
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .disabled(true)
                        .help("Waits on an engine command to run the check")
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
            return ("Leaving through \(exit)" + (ago.map { " · \($0)" } ?? ""), "Passed", .water)
        case .checking?:
            return ("Checking where torrent traffic leaves from", "Checking", .haze)
        case .leaking?:
            return ("Traffic left outside the protection. Torrents are paused.", "Failed", .vermilion)
        case .unreachable?:
            return ("The check could not be made. Torrents are paused.", "Failed", .vermilion)
        case .unprotected?:
            return ("No protection is set", "Unprotected", .haze)
        case nil:
            return ("Not checked yet", "Waiting", .haze)
        }
    }
}

// MARK: - Accounts

struct AccountsPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var sheet: AccountSheet?

    var body: some View {
        let anilist = store.accounts.tracker("anilist")
        SettingsGroup(
            title: "AniList",
            footnote: "Progress and list changes sync to AniList. Library and the Calendar’s My List show your AniList lists."
        ) {
            if let anilist, anilist.connected {
                AccountRow(state: anilist) {
                    HStack(spacing: 8) {
                        Button("Sync Now") { store.syncNow() }
                            .disabled(store.accounts.isWorking("sync"))
                        Button("Sign Out") { store.disconnect("anilist") }
                    }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
                if !store.accounts.conflicts.isEmpty {
                    SettingsRow(title: "\(Wording.plural(store.accounts.conflicts.count, "title")) differ from AniList") {
                        Button("Review…") { sheet = .conflicts }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
            } else {
                // E6: AniList's sign-in in the app comes with native sign-in.
                SettingsRow(title: "AniList", note: "Signing in here comes in a later build.") {
                    Button("Sign In…") {}
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .disabled(true)
                }
            }
        }
        SettingsGroup(title: "Other trackers", footnote: AccountWords.tokens(store.accounts)) {
            ForEach(["mal", "kitsu", "simkl", "trakt"], id: \.self) { tracker in
                TrackerRow(tracker: tracker, sheet: $sheet)
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
                    Text(state.user ?? "Signed in")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.silk)
                    HStack(spacing: 6) {
                        StateDot(state: AccountWords.dot(state, store.accounts))
                        Text(AccountWords.status(state, store.accounts, now: context.date))
                            .font(.system(size: 12))
                            .foregroundStyle(.haze)
                    }
                }
                .fixedLineHeight(16)
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
                    Button("Sign Out") { store.disconnect(tracker) }
                } else {
                    switch tracker {
                    case "kitsu":
                        Button("Sign In…") { sheet = .kitsu }
                    case "simkl", "trakt":
                        Button("Sign In…") { store.connect(tracker) }
                            .disabled(store.accounts.isWorking(tracker))
                    default:
                        // E6: MyAnimeList signs in through the browser, with native sign-in.
                        Button("Sign In…") {}
                            .disabled(true)
                            .help("Comes with engine step E6")
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
        if state.needsReauth { return "Sign in again to keep syncing" }
        if let error = state.pullError { return "Not synced: \(error.reason)" }
        if accounts.isWorking("sync") { return "Syncing…" }
        if state.outbox > 0 { return "\(Wording.plural(Int(state.outbox), "change")) waiting to sync" }
        if let at = accounts.syncedAt[state.tracker] {
            let seconds = Int64(now.timeIntervalSince(at))
            return seconds < 60 ? "Synced just now" : "Synced \(Wording.ago(seconds))"
        }
        // Nothing waiting and nothing failed, as the sync capsule says it.
        return "Synced"
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
        return storage.lowercased().contains("file")
            ? "Sign-in tokens are kept in a file on this \(Device.name)."
            : "Sign-in tokens are kept in the Keychain on this \(Device.name)."
    }
}
