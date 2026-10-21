// The menu bar, as MacMenuBar draws it: every item there, those that do not apply right now
// dimmed in place, never hidden. Mac and iPad share it.

import AnistreamKit
import AnistreamStore
import SwiftUI

/// The app's menus beside the system's.
public struct AnistreamCommands: Commands {
    let store: AppStore
    let navigator: Navigator

    public init(store: AppStore, navigator: Navigator) {
        self.store = store
        self.navigator = navigator
    }

    public var body: some Commands {
        CommandGroup(replacing: .appSettings) {
            Button("Settings…") { navigator.open(.settings) }
                .keyboardShortcut(",", modifiers: .command)
        }

        // File.
        CommandGroup(replacing: .newItem) {
            Button("Join Store…") { navigator.open(.pluginStores) }
            Button("Open Invite Link") {}
                .disabled(true)
            Divider()
            Button("Export Lists and History…") {}
                .disabled(true)
            Button("Show in Finder") {}
                .disabled(true)
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
            Button("Back") { withAnimationIfPossible { navigator.back() } }
                .keyboardShortcut("[", modifiers: .command)
                .disabled(!navigator.canGoBack)
            Button("Sync Now") { store.send(.syncNow) }
                .keyboardShortcut("r", modifiers: .command)
                .disabled(store.accounts.summary.state == .signedOut)
            Divider()
            Button("Pause All Downloads") {}
                .disabled(true)
            Button("Clear Finished Downloads") { store.send(.downloadClearCompleted) }
        }

        // Nothing plays inside the app yet, so all of Playback is dimmed.
        CommandMenu("Playback") {
            Button("Play") {}.keyboardShortcut(.space, modifiers: []).disabled(true)
            Divider()
            Button("Skip Forward 10 Seconds") {}.keyboardShortcut(.rightArrow, modifiers: []).disabled(true)
            Button("Skip Back 10 Seconds") {}.keyboardShortcut(.leftArrow, modifiers: []).disabled(true)
            Divider()
            Button("Next Episode") {}.keyboardShortcut(.rightArrow, modifiers: .command).disabled(true)
            Button("Previous Episode") {}.keyboardShortcut(.leftArrow, modifiers: .command).disabled(true)
            Button("Skip Opening") {}.keyboardShortcut("s", modifiers: []).disabled(true)
            Divider()
            Menu("Speed") {
                Button("Slower") {}.keyboardShortcut("[", modifiers: []).disabled(true)
                Button("Faster") {}.keyboardShortcut("]", modifiers: []).disabled(true)
                Divider()
                ForEach(["0.5×", "0.75×", "Normal", "1.25×", "1.5×", "2×"], id: \.self) { speed in
                    Button(speed) {}.disabled(true)
                }
            }
            Menu("Subtitles and Audio") {
                Button("Audio") {}.disabled(true)
                Button("Subtitles") {}.disabled(true)
            }
            Divider()
            Button("Volume Up") {}.keyboardShortcut(.upArrow, modifiers: []).disabled(true)
            Button("Volume Down") {}.keyboardShortcut(.downArrow, modifiers: []).disabled(true)
            Button("Mute") {}.keyboardShortcut("m", modifiers: []).disabled(true)
            Divider()
            Button("Enter Full Screen") {}.keyboardShortcut("f", modifiers: [.command, .control]).disabled(true)
            Button("Picture in Picture") {}.keyboardShortcut("p", modifiers: [.command, .option]).disabled(true)
        }

        CommandMenu("Episode") {
            EpisodeCommands(store: store, navigator: navigator)
        }

        CommandMenu("Sources") {
            sectionButton(.providers, "6")
            sectionButton(.pluginStores, "7")
            Divider()
            Button("Check for Updates") {}
                .disabled(true)
        }

        CommandGroup(after: .windowArrangement) {
            Divider()
            #if os(macOS)
            MainWindowButton()
            #endif
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
        Button("Show Settings for This Show…") {}
            .keyboardShortcut("i", modifiers: .command)
            .disabled(true)
    }
}
