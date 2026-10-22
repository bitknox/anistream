// Settings on iPhone: a sheet with a stack of its own, opened from the account button on
// Continue, its pages native grouped lists on the engine's own settings.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
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
                        Button("Done") { dismiss() }.fontWeight(.semibold).tint(.silk)
                    }
                }
        }
        .onAppear { if start != .root, path.isEmpty { path = [start] } }
        .modifier(DeviceCodePresenter(here: true))
        // What the engine says while Settings is open, a sign-in that failed among it: at the
        // foot of the sheet, clear of its bar.
        .overlay(alignment: .bottom) {
            NoticeCapsule(opensMessages: false).padding(.horizontal, 16).padding(.bottom, 24)
        }
        .tint(.amber)
        .preferredColorScheme(.dark)
    }
}

extension PhoneSettingsPage {
    @MainActor @ViewBuilder
    var view: some View {
        switch self {
        case .root: PhoneSettingsRoot()
        case .playback: PhonePlaybackPage()
        case .downloads: PhoneDownloadsPage()
        case .sources: PhoneSourcesPage()
        case .accounts: PhoneAccountsPage()
        case .providers: PhoneProvidersPage()
        case .stores: PhoneStoresPage()
        case .about: PhoneAboutPage()
        case .acknowledgements: PhoneAcknowledgementsPage()
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
        Text(text.uppercased())
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
                            Text(key.display(option.label)).tag(Optional(option.value))
                        }
                    }
                    .pickerStyle(.inline)
                } label: {
                    PhoneMenuValue(text: row.map { key.display(current?.label ?? $0.value) } ?? "–")
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

/// A row the engine cannot set yet: its value as the board shows it, disabled.
struct PhoneDeferredRow: View {
    let title: String
    var note: String?
    let value: String
    let waitsOn: String

    var body: some View {
        HStack {
            PhoneRowTitle(title: title, note: note)
            Spacer()
            HStack(spacing: 6) {
                Text(value)
                Image(systemName: "chevron.up.chevron.down").font(.ui(11, weight: .semibold))
            }
            .font(.ui(17))
            .foregroundStyle(.haze)
        }
        .opacity(0.5)
        .listRowBackground(Palette.raised)
        .accessibilityElement(children: .combine)
        .accessibilityHint("Not available yet")
        .help("Comes with engine step \(waitsOn)")
    }
}

/// A switch the engine cannot set yet.
struct PhoneDeferredSwitch: View {
    let title: String
    var note: String?
    let on: Bool

    var body: some View {
        Toggle(isOn: .constant(on)) { PhoneRowTitle(title: title, note: note) }
            .tint(.amber)
            .disabled(true)
            .listRowBackground(Palette.raised)
    }
}

// MARK: - The first page

struct PhoneSettingsRoot: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let anilist = store.accounts.tracker("anilist")
        PhoneList {
            Section {
                NavigationLink(value: PhoneSettingsPage.accounts) {
                    HStack(spacing: 14) {
                        Avatar(name: anilist?.user, size: 48)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(anilist?.connected == true ? (anilist?.user ?? "Signed in") : "Not signed in")
                                .font(.ui(17, weight: .semibold))
                                .foregroundStyle(.silk)
                            HStack(spacing: 6) {
                                if let anilist, anilist.connected {
                                    StateDot(state: AccountWords.dot(anilist, store.accounts))
                                    Text("AniList · " + AccountWords.status(anilist, store.accounts))
                                } else {
                                    Text("Library and sync need an AniList account")
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
                link(.playback, "Playback", "play.rectangle", value: playbackValue)
                link(.downloads, "Downloads", "arrow.down.to.line", value: Wording.bytes(store.downloads.bytesOnDisk))
                link(.sources, "Sources", "powerplug", value: store.settings.isOn(.torrents) ? "Torrents on" : "Torrents off")
                link(.accounts, "Accounts", "person", value: anilist?.connected == true ? "AniList" : "None")
            }
            Section {
                link(.about, "About", "info.circle", value: AppVersion.short)
            } footer: {
                Text("anistream ships no sources of its own and hosts no content. Every source is external.")
            }
        }
        .navigationTitle("Settings")
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
            HStack {
                Label(title, systemImage: symbol)
                    .font(.ui(17))
                    .foregroundStyle(.silk)
                Spacer()
                Text(value).font(.ui(17)).foregroundStyle(.haze)
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

// MARK: - Playback

struct PhonePlaybackPage: View {
    @Environment(AppStore.self) private var store
    @AppStorage("anistream.spoilerProtection") private var spoilers = true

    var body: some View {
        PhoneList {
            Section {
                PhoneTitleLanguageRow()
                // "Overridden for 3 shows" waits on E8's override counts.
                PhoneSettingPicker(key: .translation)
                PhoneSubtitlesPicker()
                PhoneSettingPicker(key: .quality)
            } header: {
                PhoneSectionHeader("Language")
            } footer: {
                Text("Each show remembers its own audio and subtitles.")
            }
            Section {
                PhoneSettingSwitch(key: .autoNext)
                // E8: `UpNextCountdown`.
                PhoneDeferredRow(title: "Up Next countdown", value: "10 s", waitsOn: "E8")
                PhoneSettingSwitch(key: .skipOpening)
                PhoneSettingSwitch(key: .skipFiller)
                PhoneSettingSwitch(key: .markChapters)
                PhoneSettingPicker(key: .commitThreshold)
                // Kept on this device until E8's `SpoilerProtection`.
                Toggle(isOn: $spoilers) {
                    PhoneRowTitle(title: "Spoiler protection", note: "Blurs stills after the next episode")
                }
                .tint(.amber)
                .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader("Episodes")
            }
            Section {
                PhoneSettingPicker(key: .upscaling, note: upscalingNote)
                PhoneSettingSwitch(key: .nightMode, note: "Evens out loud and quiet passages, so dialogue stays clear at low volume.")
            } header: {
                PhoneSectionHeader("Picture and sound")
            }
        }
        .navigationTitle("Playback")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var upscalingNote: String {
        switch store.settings.row(.upscaling)?.option?.label {
        case "off": "The source, scaled to fit without processing."
        case "sharp · Anime4K": "Crisper lines and stronger edges, at some cost to texture."
        default: "Faithful line work and texture, nothing restyled."
        }
    }
}

/// The subtitle language, by name.
struct PhoneSubtitlesPicker: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let code = store.settings.row(.subtitles)?.typedText ?? ""
        HStack {
            Text("Subtitles").font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            Menu {
                Picker("Subtitles", selection: Binding(get: { code }, set: { store.setSetting(.subtitles, to: .str($0)) })) {
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
            Section {
                // E8: `DownloadQuality`.
                PhoneDeferredRow(title: "Quality", note: "Higher takes the best version a source offers, and more space.",
                                 value: "Standard", waitsOn: "E8")
                PhoneSettingSwitch(key: .mergeSubtitles, note: "Subtitles stay with the episode wherever the file goes.")
                PhoneSettingSwitch(key: .keepSeeding, note: "A download that keeps seeding stays in anistream’s cache.")
            } header: {
                PhoneSectionHeader("Files")
            }
            Section {
                Button("Delete Watched Downloads", role: .destructive) { confirmDelete = true }
                    .disabled(watched.rows.isEmpty)
                    .listRowBackground(Palette.raised)
            } footer: {
                Text("\(Wording.bytes(store.downloads.bytesOnDisk)) in downloads"
                     + (FreeSpace.bytes.map { ". \(Wording.bytes($0)) free on this iPhone." } ?? "."))
            }
        }
        .inProgressRotor(store.downloads.groups, titles: store.titles, in: rotor)
        .navigationTitle("Downloads")
        .navigationBarTitleDisplayMode(.inline)
        .confirmationDialog("Delete \(Wording.plural(watched.rows.count, "watched download"))?", isPresented: $confirmDelete) {
            Button("Delete", role: .destructive) { for row in watched.rows { store.deleteDownload(row.id) } }
        }
        .task { store.loadDownloads() }
    }
}

/// One download in the iPhone's list: the episode, where it stands, and its actions on swipe.
struct PhoneDownloadRow: View {
    @Environment(AppStore.self) private var store
    let row: DownloadRow

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("Episode \(row.episode)").font(.ui(17)).foregroundStyle(.silk)
                Spacer()
                if row.total > 0 {
                    Text(Wording.bytes(row.total)).font(.ui(15)).foregroundStyle(.haze).monospacedDigit()
                }
            }
            Text(status).font(.ui(13)).foregroundStyle(row.state == .failed ? .vermilion : .haze)
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
        .swipeActions(edge: .trailing) {
            switch row.state {
            case .done:
                Button("Delete", role: .destructive) { store.deleteDownload(row.id) }
            default:
                Button("Remove", role: .destructive) { store.cancelDownload(row.id) }
            }
        }
        .swipeActions(edge: .leading) {
            switch row.state {
            case .done: Button("Play") { store.playDownload(row) }.tint(.amber)
            case .active, .queued: Button("Pause") { store.toggleDownload(row.id) }
            case .paused: Button("Resume") { store.toggleDownload(row.id) }.tint(.water)
            case .failed: Button("Retry") { store.retryDownload(row) }.tint(.amber)
            }
        }
    }

    private var status: String {
        switch row.state {
        case .done: "Downloaded"
        case .active where row.total == 0: "Waiting for metadata"
        case .active:
            "\(Wording.bytes(row.downloaded)) of \(Wording.bytes(row.total))"
                + (store.downloads.secondsLeft(row).map { " · \(Wording.countdown($0)) left" } ?? "")
        case .paused: "Paused at \(Wording.bytes(row.downloaded)) of \(Wording.bytes(row.total))"
        case .queued: "Queued · starts when a download slot is free"
        case .failed: "Failed: \(row.error ?? "the download stopped")"
        }
    }
}

// MARK: - Sources

struct PhoneSourcesPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var turningOn = false

    var body: some View {
        let on = store.settings.isOn(.torrents)
        PhoneList {
            Section {
                NavigationLink(value: PhoneSettingsPage.providers) {
                    valueRow("Source order", Wording.plural(store.providers.rows.count, "source"))
                }
                .listRowBackground(Palette.raised)
                NavigationLink(value: PhoneSettingsPage.stores) {
                    valueRow("Plugin stores", storesValue)
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
                    // E9: `rss_url`.
                    PhoneDeferredRow(title: "Torrent index", note: "The RSS search address anistream asks for torrents",
                                     value: "", waitsOn: "E9")
                    // E8: `TorrentsOnCellular`, `SetTorrentProtection`, `VpnOnFailure`.
                    PhoneDeferredSwitch(title: "Torrents on cellular", on: false)
                    PhoneDeferredRow(title: "Protect with", value: Protection.label(store.settings.value(.vpnMode)), waitsOn: "E8")
                    PhoneDeferredRow(title: "If a check fails", value: "Pause torrents", waitsOn: "E8")
                    PhoneGuardRow()
                }
            } header: {
                PhoneSectionHeader("Torrents")
            } footer: {
                Text(on
                     ? "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you."
                     : "Off by default. Turning this on asks how to protect your connection first.")
            }
            if on {
                Section {
                    PhoneSettingPicker(key: .seedRatio, note: "Best effort on iPhone: only while anistream is open")
                    PhoneSettingPicker(key: .seedHours)
                    PhoneSettingPicker(key: .uploadLimit)
                } header: {
                    PhoneSectionHeader("Seeding")
                }
            }
        }
        .navigationTitle("Sources")
        .navigationBarTitleDisplayMode(.inline)
        .sheet(isPresented: $turningOn) { PhoneTorrentSheet() }
        .onLaunchSheet("torrent", navigator) { turningOn = true }
        .task {
            store.checkProviders()
            if !store.stores.loaded { store.loadStores() }
        }
    }

    private var storesValue: String {
        let count = store.stores.stores.count
        let updates = store.stores.updates
        return updates > 0 ? "\(count) · \(Wording.plural(updates, "update"))" : "\(count)"
    }

    private func valueRow(_ title: String, _ value: String) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            Text(value).font(.ui(17)).foregroundStyle(.haze)
        }
    }
}

/// The VPN guard's last word, in a list row.
struct PhoneGuardRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let (note, status, color) = GuardWords.words(store.providers.guardView, at: store.providers.guardAt)
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text("Last check").font(.ui(17)).foregroundStyle(.silk)
                Spacer()
                HStack(spacing: 6) {
                    Circle().fill(color).frame(width: 8, height: 8)
                    Text(status).font(.ui(17)).foregroundStyle(.haze)
                }
            }
            Text(note).font(.ui(13)).foregroundStyle(.haze)
        }
        .listRowBackground(Palette.raised)
        .accessibilityElement(children: .combine)
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
                            Text(anilist.user ?? "Signed in").font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
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
                    Button("Sync now") { store.syncNow() }
                        .fontWeight(.semibold)
                        .foregroundStyle(.silk)
                        .disabled(store.accounts.isWorking("sync"))
                        .listRowBackground(Palette.raised)
                    if !store.accounts.conflicts.isEmpty {
                        Button("Review \(Wording.plural(store.accounts.conflicts.count, "difference"))") { conflicts = true }
                            .foregroundStyle(.silk)
                            .listRowBackground(Palette.raised)
                    }
                    Button("Sign out") { store.disconnect("anilist") }
                        .fontWeight(.semibold)
                        .foregroundStyle(.silk)
                        .listRowBackground(Palette.raised)
                } else {
                    HStack {
                        PhoneRowTitle(title: "AniList", note: "Brings in your lists and fills Continue with what you are watching.")
                        Spacer()
                        Button("Sign in") { store.connect("anilist") }
                            .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
                            .disabled(store.accounts.isWorking("anilist"))
                    }
                    .listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader("AniList")
            } footer: {
                Text("Library shows your lists from AniList.")
            }
            Section {
                ForEach(["mal", "kitsu", "simkl", "trakt"], id: \.self) { tracker in
                    PhoneTrackerRow(tracker: tracker, kitsu: $kitsu)
                }
            } header: {
                PhoneSectionHeader("Other trackers")
            } footer: {
                Text(AccountWords.tokens(store.accounts))
            }
        }
        .navigationTitle("Accounts")
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
                    Button("Sign out") { store.disconnect(tracker) }
                } else {
                    switch tracker {
                    case "kitsu": Button("Sign in") { kitsu = true }
                    default:
                        // MyAnimeList through the system's sign-in sheet, Simkl and Trakt with a
                        // code to type on another device.
                        Button("Sign in") { store.connect(tracker) }.disabled(store.accounts.isWorking(tracker))
                    }
                }
            }
            .buttonStyle(SecondaryCapsuleStyle(height: 30, raisedSurface: true, fontSize: 15))
        }
        .listRowBackground(Palette.raised)
    }
}
#endif
