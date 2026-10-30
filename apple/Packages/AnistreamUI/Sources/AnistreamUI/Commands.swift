// The menu bar, as MacMenuBar draws it: every item there, those that do not apply right now
// dimmed in place, never hidden. Mac and iPad share it.

import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The app's menus beside the system's. The Playback menu's items and Window's player item come
/// from the player the app hosts, which this package does not know.
public struct AnistreamCommands<Playback: View, PlayerWindowItems: View>: Commands {
    @Environment(\.openURL) private var openURL
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
            Button(L.menuSettings) { navigator.openSettings(navigator.settingsPane) }
                .keyboardShortcut(",", modifiers: .command)
        }

        // File.
        CommandGroup(replacing: .newItem) {
            Button(L.menuJoinPluginStore) { navigator.invite = "" }
            // The invite link on the pasteboard, into the join sheet.
            Button(L.menuOpenInviteLink) { navigator.invite = Pasteboard.invite ?? "" }
            #if os(macOS)
            Divider()
            Button(L.menuShowInFinder) { DownloadFolder.show(store) }
                .disabled((store.settings.row(.downloadDir)?.typedText ?? "").isEmpty)
            #endif
        }

        CommandMenu(L.navLibrary) {
            sectionButton(.continue, "1")
            sectionButton(.calendar, "2")
            sectionButton(.seasonal, "3")
            sectionButton(.library, "4")
            sectionButton(.downloads, "5")
            Divider()
            Button(L.navSearch) { navigator.open(.search) }
                .keyboardShortcut("f", modifiers: .command)
            Button(L.menuSearchFilters) {
                navigator.section = .search
                navigator.searchFiltersShown.toggle()
            }
            .keyboardShortcut("f", modifiers: [.command, .option])
            Button(L.menuCommandPalette) { navigator.paletteShown.toggle() }
                .keyboardShortcut("k", modifiers: .command)
            Divider()
            Button(L.actionBack) {
                if let playerBack, playerBack.isUp() {
                    playerBack.back()
                } else {
                    withAnimationIfPossible { navigator.back() }
                }
            }
            .keyboardShortcut("[", modifiers: .command)
            .disabled(!navigator.canGoBack && playerBack?.isUp() != true)
            Button(L.menuSyncNow) { store.send(.syncNow) }
                .keyboardShortcut("r", modifiers: .command)
                .disabled(store.accounts.summary.state == .signedOut)
            Divider()
            Button(L.menuPauseAllDownloads) { store.pauseAllDownloads() }
                .disabled(!store.downloads.rows.contains { $0.state == .active || $0.state == .queued })
            Button(L.menuClearFinishedDownloads) { store.clearCompletedDownloads() }
        }

        CommandMenu(L.menuPlayback) {
            playback
        }

        CommandMenu(L.menuEpisode) {
            EpisodeCommands(store: store, navigator: navigator)
        }

        CommandMenu(L.sourcesTitle) {
            sectionButton(.providers, "6")
            sectionButton(.pluginStores, "7")
            Divider()
            Button(L.menuCheckForUpdates) { store.loadStores() }
        }

        // View: the calendar's weeks, outside text fields, while the calendar is the section.
        CommandGroup(after: .toolbar) {
            let bounds = store.calendarBounds
            let week = navigator.calendarWeek
            let here = navigator.section == .calendar && navigator.titleOnMac == nil
            Button(L.menuGoToToday) { navigator.showCalendarWeek(0) }
                .keyboardShortcut("t", modifiers: .command)
                .disabled(!here || week == 0)
            // In a text field, such as the command palette's over the calendar, the keys keep
            // moving the caret to the line's ends.
            Button(L.menuPreviousWeek) {
                if !CaretKeys.pass(toLineEnd: .left) { navigator.showCalendarWeek(week - 1) }
            }
            .keyboardShortcut(.leftArrow, modifiers: .command)
            .disabled(!here || !bounds.touches(weekDays(offset: week - 1)))
            Button(L.menuNextWeek) {
                if !CaretKeys.pass(toLineEnd: .right) { navigator.showCalendarWeek(week + 1) }
            }
            .keyboardShortcut(.rightArrow, modifiers: .command)
            .disabled(!here || !bounds.touches(weekDays(offset: week + 1)))
            Divider()
            Button(L.calendarFilterAllTitle) { store.setSwitch(.calendarMine, on: false) }
                .disabled(navigator.section != .calendar)
            Button(L.calendarFilterMineTitle) { store.setSwitch(.calendarMine, on: true) }
                .disabled(navigator.section != .calendar)
        }

        CommandGroup(after: .windowArrangement) {
            Divider()
            #if os(macOS)
            MainWindowButton()
            #endif
            playerWindowItems
            Button(L.menuMessages) { navigator.messagesShown = true }
                .keyboardShortcut("l", modifiers: [.command, .option])
        }

        CommandGroup(replacing: .help) {
            Button(L.menuReportBug) { ReportBugAction(store: store, openURL: openURL)() }
        }
    }

    private func sectionButton(_ section: AppSection, _ key: Character) -> some View {
        Button(section == .pluginStores ? L.menuPluginStores : section.title) { navigator.open(section) }
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
            Button(L.actionPlay) {}.keyboardShortcut(.space, modifiers: [])
            Button(L.menuSkipForward) {}
            Button(L.menuSkipBack) {}
            Button(L.menuNextEpisode) {}
            Button(L.menuPreviousEpisode) {}
            Button(L.menuSkipOpening) {}
        }
        .disabled(true)
    }
}

#if os(macOS)
/// The app's one main window, opened or brought forward, for the app's delegate. SwiftUI hands its
/// way to open a window only to scenes and views; Window › Main Window is built with the menu
/// bar at launch, so it is there with or without a window, and hands it on.
@MainActor
public enum MainWindow {
    public internal(set) static var open: (() -> Void)?
}

/// Window › Main Window: the app's one main window, brought back or to the front.
struct MainWindowButton: View {
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        let _ = MainWindow.open = { [openWindow] in openWindow(id: "main") }
        Button(L.menuMainWindow) { openWindow(id: "main") }
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
        // An episode still to air cannot be watched yet.
        let aired = entry?.airing(of: next).hasAired ?? true
        Section(entry == nil ? L.menuNoEpisodeSelected : name.map { L.episodeWithTitle(number: Int(next), title: $0) } ?? L.episodeNumber(number: Int(next))) {
            Button(L.menuMarkWatched) {
                if let entry { store.send(.setWatched(id: entry.id, episodes: [String(next)], watched: true)) }
            }
            // The same keys as Mark Unwatched, held by whichever of the two can act: a menu on
            // iPad may not list one shortcut twice.
            .keyboardShortcut(watched ? nil : KeyboardShortcut("u", modifiers: [.command, .shift]))
            .disabled(entry == nil || watched || !aired)
            Button(L.menuMarkUnwatched) {
                if let entry { store.send(.setWatched(id: entry.id, episodes: [String(next)], watched: false)) }
            }
            .keyboardShortcut(watched ? KeyboardShortcut("u", modifiers: [.command, .shift]) : nil)
            .disabled(entry == nil || !watched)
            Button(L.menuMarkAllPreviousWatched) {
                if let entry {
                    store.send(.setWatched(id: entry.id, episodes: (1..<max(next, 1)).map(String.init), watched: true))
                }
            }
            .disabled(entry == nil || next <= 1)
            Button(L.menuDownload) {
                if let entry { store.send(.downloadEpisode(id: entry.id, episode: String(next))) }
            }
            .disabled(entry.map { store.hasDownload($0.id, episode: String(next)) } ?? true || !aired)
        }
        Section(entry?.showTitle ?? L.menuNoTitleSelected) {
            Menu(L.filterStatus) {
                ForEach(LibraryModel.order, id: \.self) { segment in
                    Button(segment.label.capitalizedSentence) {
                        if let entry { store.setStatus(entry.id, segment) }
                    }
                }
            }
            .disabled(entry == nil)
            Button(L.continueHide) {
                if let entry { store.hideFromContinue(entry.id) }
            }
            .keyboardShortcut(.delete, modifiers: .command)
            .disabled(entry?.progress == nil)
            Button(L.menuPlayTrailer) {
                if let entry { navigator.playTrailer(entry, store: store) }
            }
            .disabled(entry?.trailerUrl == nil)
            Button(L.menuMoreAbout) {
                if let entry { navigator.moreAbout = entry.id }
            }
            .keyboardShortcut("i", modifiers: [.command, .option])
            .disabled(entry == nil)
            Button(L.titleOpenAnilist) {
                if let entry { store.openOnAniList(entry.id) }
            }
            .disabled(entry == nil)
            Button(L.menuSourceForTitle) {
                if let entry { navigator.sourcePicker = entry.id }
            }
            .disabled(entry == nil)
        }
        Divider()
        Button(L.menuShowSettings) {
            if let entry { navigator.showSettings = entry.id }
        }
        // ⌘I, as the boards have it (Johan, 24 October 2026); More About This Show is ⌥⌘I.
        .keyboardShortcut("i", modifiers: .command)
        .disabled(entry == nil)
    }
}

/// The keys a menu item shares with text editing, handed back to the text field that has the
/// keyboard. iPad's text fields keep theirs before the menu's, so only the Mac needs this.
@MainActor
enum CaretKeys {
    enum Side { case left, right }

    /// Moves the caret to the line's end on that side when a text field has the keyboard, and
    /// says whether it did.
    static func pass(toLineEnd side: Side) -> Bool {
        #if os(macOS)
        guard NSApp.keyWindow?.firstResponder is NSText else { return false }
        let move = side == .left
            ? #selector(NSResponder.moveToLeftEndOfLine(_:))
            : #selector(NSResponder.moveToRightEndOfLine(_:))
        return NSApp.sendAction(move, to: nil, from: nil)
        #else
        return false
        #endif
    }
}
