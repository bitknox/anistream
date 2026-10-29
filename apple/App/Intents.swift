// The app's App Intents and App Shortcuts, as native-spec/native-layer.md has them: Continue
// watching, Open title and Airing today, which Shortcuts, Siri and Spotlight offer once the app is
// installed. None needs the Siri entitlement. Their snippets are the system's look until the
// NativeIntents board is drawn into them.
//
// They are the app target's own, the title entity among them: Siri's phrase training reads an
// entity's name from the app's metadata, and with the entity in a framework it found none, so
// "Open … in anistream" left the phrases untrained. What each does is AnistreamUI's
// `IntentsBridge`, where it is tested.

import AnistreamKit
import AnistreamUI
import AppIntents
import SwiftUI

// MARK: - Titles

/// A title on your lists, as Shortcuts and Siri name it.
struct TitleEntity: AppEntity, Identifiable, Hashable, Sendable {
    static var typeDisplayRepresentation: TypeDisplayRepresentation { "Title" }
    static var defaultQuery: TitleQuery { TitleQuery() }

    /// The AniList id.
    let id: Int
    let title: String
    /// "Stardust Wayfarer · TV · 2026".
    let line: String

    init(_ title: ShelfTitle) {
        id = Int(title.id)
        self.title = title.title
        line = title.line
    }

    var displayRepresentation: DisplayRepresentation {
        line.isEmpty
            ? DisplayRepresentation(title: "\(title)")
            : DisplayRepresentation(title: "\(title)", subtitle: "\(line)")
    }
}

/// Finds titles on your lists: by id, by the words of any of their names, and as suggestions,
/// Continue's first.
struct TitleQuery: EntityStringQuery {
    @MainActor
    func entities(for identifiers: [Int]) async throws -> [TitleEntity] {
        IntentsBridge.shared.titles(for: identifiers).map(TitleEntity.init)
    }

    @MainActor
    func entities(matching string: String) async throws -> [TitleEntity] {
        IntentsBridge.shared.titles(matching: string).map(TitleEntity.init)
    }

    @MainActor
    func suggestedEntities() async throws -> [TitleEntity] {
        IntentsBridge.shared.suggestedTitles().map(TitleEntity.init)
    }
}

// MARK: - The intents

/// Plays the episode Continue's first title plays next, from where it was left: the app opens
/// into the player. With nothing out to play it answers, and the app stays where it is.
struct ContinueWatchingIntent: AppIntent {
    static let title: LocalizedStringResource = "Continue watching"
    static let description = IntentDescription("Plays the next episode of the title you are watching, from where you left it.")
    static let supportedModes: IntentModes = [.background, .foreground(.dynamic)]

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        let bridge = IntentsBridge.shared
        let answer = await bridge.continueWatching()
        if case .play(let id, let episode, _) = answer {
            try await continueInForeground(alwaysConfirm: false)
            bridge.play(id, episode: episode)
        }
        return .result(dialog: "\(answer.spoken)")
    }
}

/// Opens a title's page.
struct OpenTitleIntent: OpenIntent {
    static let title: LocalizedStringResource = "Open title"
    static let description = IntentDescription("Opens a title from your lists.")
    static let supportedModes: IntentModes = .foreground(.immediate)

    @Parameter(title: "Title") // i18n-skip
    var target: TitleEntity

    @MainActor
    func perform() async throws -> some IntentResult {
        if let id = AnilistId(exactly: target.id) { IntentsBridge.shared.open(id) }
        return .result()
    }
}

/// Today's broadcasts with their times, yours marked, without opening the app.
struct AiringTodayIntent: AppIntent {
    static let title: LocalizedStringResource = "Airing today"
    static let description = IntentDescription("Lists what airs today, with the times, and marks the titles on your lists.")
    static let supportedModes: IntentModes = .background

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog & ShowsSnippetView {
        guard let rows = await IntentsBridge.shared.airingToday() else {
            return .result(dialog: "Today's schedule could not be loaded.", view: AiringTodaySnippet(rows: []))
        }
        return .result(dialog: "\(TodaysBroadcasts.summary(rows))", view: AiringTodaySnippet(rows: rows))
    }
}

// MARK: - App Shortcuts

struct AnistreamShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: ContinueWatchingIntent(),
            phrases: [
                "Continue watching in \(.applicationName)",
                "Resume \(.applicationName)",
                "Play my next episode in \(.applicationName)",
            ],
            shortTitle: "Continue watching",
            systemImageName: "play.circle"
        )
        AppShortcut(
            intent: OpenTitleIntent(),
            phrases: [
                "Open \(\.$target) in \(.applicationName)",
                "Show \(\.$target) in \(.applicationName)",
            ],
            shortTitle: "Open title",
            systemImageName: "rectangle.portrait"
        )
        AppShortcut(
            intent: AiringTodayIntent(),
            phrases: [
                "What's airing today in \(.applicationName)",
                "What's on \(.applicationName) today",
            ],
            shortTitle: "Airing today",
            systemImageName: "calendar"
        )
    }
}
