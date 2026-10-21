// The menu bar, as MacMenuBar draws it: every item there, those that do not apply right now
// dimmed in place, never hidden. Mac and iPad share it.

import AnistreamKit
import AnistreamStore
import SwiftUI

/// The app's menus beside the system's. The Playback menu's items and Window's player item come
/// from the player the app hosts, which this package does not know.
public struct AnistreamCommands<Playback: View, PlayerWindowItems: View>: Commands {
    let store: AppStore
    let navigator: Navigator
    let playback: Playback
    let playerWindowItems: PlayerWindowItems
    /// Library › Back while a player is up in the window: it returns from the player instead.
    let playerBack: PlayerBack?

    /// Whether a player has the window, and how it goes back to the page it came from.
    public struct PlayerBack {
        public let isUp: @MainActor () -> Bool
        public let back: @MainActor () -> Void

        public init(isUp: @escaping @MainActor () -> Bool, back: @escaping @MainActor () -> Void) {
            self.isUp = isUp
            self.back = back
        }
    }

    public init(
        store: AppStore,
        navigator: Navigator,
        playerBack: PlayerBack? = nil,
        @ViewBuilder playback: () -> Playback,
        @ViewBuilder playerWindowItems: () -> PlayerWindowItems
    ) {
        self.store = store
        self.navigator = navigator
        self.playerBack = playerBack
        self.playback = playback()
        self.playerWindowItems = playerWindowItems()
    }

    public var body: some Commands {
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { navigator.openSettings(navigator.settingsPane) }
                .keyboardShortcut(",", modifiers: .command)
        }

        // File.
        CommandGroup(replacing: .newItem) {
            Button("Join Store…") { navigator.invite = "" }
            // The invite link on the pasteboard, into the join sheet.
            Button("Open Invite Link") { navigator.invite = Pasteboard.invite ?? "" }
            Divider()
            // Exporting lists and history has no engine command yet.
            Button("Export Lists and History…") {}
                .disabled(true)
            #if os(macOS)
            Button("Show in Finder") { DownloadFolder.show(store) }
                .disabled((store.settings.row(.downloadDir)?.typedText ?? "").isEmpty)
            #endif
        }

        CommandMenu("Library") {
            sectionButton(.continue, "1")
            sectionButton(.calendar, "2")
            sectionButton(.seasonal, "3")
            sectionButton(.library, "4")
            sectionButton(.downloads, "5")
            Divider()
            Button("Search") { navigator.open(.search) }
                .keyboardShortcut("f", modifiers: .command)
            Button("Search Filters") {}
                .keyboardShortcut("f", modifiers: [.command, .option])
                .disabled(true)
            Button("Command Palette") { navigator.paletteShown.toggle() }
                .keyboardShortcut("k", modifiers: .command)
            Divider()
            Button("Back") {
                if let playerBack, playerBack.isUp() {
                    playerBack.back()
                } else {
                    withAnimationIfPossible { navigator.back() }
                }
            }
            .keyboardShortcut("[", modifiers: .command)
            .disabled(!navigator.canGoBack && playerBack?.isUp() != true)
            Button("Sync Now") { store.send(.syncNow) }
                .keyboardShortcut("r", modifiers: .command)
                .disabled(store.accounts.summary.state == .signedOut)
            Divider()
            Button("Pause All Downloads") { store.pauseAllDownloads() }
                .disabled(!store.downloads.rows.contains { $0.state == .active || $0.state == .queued })
            Button("Clear Finished Downloads") { store.clearCompletedDownloads() }
        }

        CommandMenu("Playback") {
            playback
        }

        CommandMenu("Episode") {
            EpisodeCommands(store: store, navigator: navigator)
        }

        CommandMenu("Sources") {
            sectionButton(.providers, "6")
            sectionButton(.pluginStores, "7")
            Divider()
            Button("Check for Updates") { store.loadStores() }
        }

        CommandGroup(after: .windowArrangement) {
            Divider()
            #if os(macOS)
            MainWindowButton()
            #endif
            playerWindowItems
            Button("Messages") { navigator.messagesShown = true }
                .keyboardShortcut("l", modifiers: [.command, .option])
        }

        CommandGroup(replacing: .help) {
            Button("anistream Help") {}
                .disabled(true)
            Button("Keyboard Shortcuts") {}
                .keyboardShortcut("/", modifiers: .command)
                .disabled(true)
        }
    }

    private func sectionButton(_ section: AppSection, _ key: Character) -> some View {
        Button(section.title) { navigator.open(section) }
            .keyboardShortcut(KeyEquivalent(key), modifiers: .command)
    }

    private func withAnimationIfPossible(_ change: () -> Void) {
        withAnimation(.standard, change)
    }
}

extension AnistreamCommands where Playback == DimmedPlayback, PlayerWindowItems == EmptyView {
    /// The menus with nothing that plays: Playback dimmed throughout.
    public init(store: AppStore, navigator: Navigator) {
        self.init(store: store, navigator: navigator, playback: { DimmedPlayback() }, playerWindowItems: { EmptyView() })
    }
}

/// The Playback menu where there is no player: every item, dimmed in place.
public struct DimmedPlayback: View {
    public var body: some View {
        Group {
            Button("Play") {}.keyboardShortcut(.space, modifiers: [])
            Button("Skip Forward 10 Seconds") {}
            Button("Skip Back 10 Seconds") {}
            Button("Next Episode") {}
            Button("Previous Episode") {}
            Button("Skip Opening") {}
        }
        .disabled(true)
    }
}

#if os(macOS)
/// Window › Main Window: the app's one main window, brought back or to the front.
struct MainWindowButton: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Button("Main Window") { openWindow(id: "main") }
            .keyboardShortcut("0", modifiers: .command)
    }
}
#endif

/// The Episode menu: the focused title's next episode, then the title.
struct EpisodeCommands: View {
    let store: AppStore
    let navigator: Navigator

    var body: some View {
        let entry = navigator.focusedTitle.flatMap { store.titles[$0] }
        let next = entry?.progress?.next ?? 1
        let name = entry.flatMap { store.titles.episodes[$0.id]?.first { $0.number == String(next) }?.title }
        let watched = entry.flatMap { store.titles.episodes[$0.id]?.first { $0.number == String(next) }?.completed } ?? false
        Section(entry == nil ? "No Episode Selected" : name.map { "Episode \(next) · \($0)" } ?? "Episode \(next)") {
            Button("Mark Watched") {
                if let entry { store.send(.setWatched(id: entry.id, episodes: [String(next)], watched: true)) }
            }
            .keyboardShortcut("u", modifiers: [.command, .shift])
            .disabled(entry == nil || watched)
            Button("Mark Unwatched") {
                if let entry { store.send(.setWatched(id: entry.id, episodes: [String(next)], watched: false)) }
            }
            .disabled(entry == nil || !watched)
            Button("Mark All Previous Watched") {
                if let entry {
                    store.send(.setWatched(id: entry.id, episodes: (1..<max(next, 1)).map(String.init), watched: true))
                }
            }
            .disabled(entry == nil || next <= 1)
            Button("Download") {}.disabled(true)
            Button("Play in Syncplay Party") {}.disabled(true)
        }
        Section(entry?.showTitle ?? "No Title Selected") {
            Menu("Status") {
                ForEach(LibraryModel.order, id: \.self) { segment in
                    Button(segment.label.capitalizedSentence) {
                        if let entry { store.setStatus(entry.id, segment) }
                    }
                }
            }
            .disabled(entry == nil)
            Button("Hide from Continue") {
                if let entry { store.hideFromContinue(entry.id) }
            }
            .keyboardShortcut(.delete, modifiers: .command)
            .disabled(entry?.progress == nil)
            Button("Play Trailer") {
                if let entry, let trailer = entry.trailerUrl { store.send(.playTrailer(url: trailer, title: entry.title)) }
            }
            .disabled(entry?.trailerUrl == nil)
            Button("Open on AniList") {
                if let entry { store.openOnAniList(entry.id) }
            }
            .disabled(entry == nil)
        }
        Divider()
        Button("Show Settings for This Show…") {
            if let entry { navigator.showSettings = entry.id }
        }
        .keyboardShortcut("i", modifiers: .command)
        .disabled(entry == nil)
    }
}
