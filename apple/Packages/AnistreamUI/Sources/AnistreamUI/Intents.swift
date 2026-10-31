// What the App Intents do: Continue watching, Open a title, and What's airing today. The intents
// themselves, their title entity and the App Shortcuts are the app target's (App/Intents.swift),
// where Siri's phrase training can read the entity; what they do is here, where it is tested.
//
// The intents reach the app through `IntentsBridge`, which the app fills at launch. Titles come
// from the shelf: Continue and the Library's lists as the engine has said them this session, over
// the shelf Spotlight saved, so they are there before the engine has answered.

import AnistreamKit
import AnistreamStore
import AnistreamStrings
import Foundation
import SwiftUI

// MARK: - Titles

/// The titles an intent can name: the shelf's, and any other the app has been told of, for a
/// shortcut made with a title that has since left your lists.
struct TitleDirectory {
    let shelf: Shelf
    /// Titles the app knows beyond the shelf.
    var known: [AnilistId: ShelfTitle] = [:]

    func titles(for identifiers: [Int]) -> [ShelfTitle] {
        identifiers.compactMap { raw in
            guard let id = AnilistId(exactly: raw) else { return nil }
            return shelf.titles[id] ?? known[id]
        }
    }

    /// The shelf's titles with every word of `string` in one of their names (romaji, English,
    /// native) or among their genres, format and year, in the shelf's order. Case, accents and
    /// width do not count: "hoshikuzu" finds "Hoshikuzu no Tabibito".
    func matching(_ string: String) -> [ShelfTitle] {
        let words = Self.fold(string).split(whereSeparator: \.isWhitespace)
        guard !words.isEmpty else { return [] }
        return shelf.order.compactMap { id in
            guard let title = shelf.titles[id] else { return nil }
            let names = Self.fold(([title.title] + title.otherTitles + title.keywords).joined(separator: " "))
            return words.allSatisfy { names.contains($0) } ? title : nil
        }
    }

    /// Every title on the shelf, Continue's first.
    var suggested: [ShelfTitle] {
        shelf.order.compactMap { shelf.titles[$0] }
    }

    static func fold(_ text: String) -> String {
        text.folding(options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive], locale: nil)
    }
}

// MARK: - The app, as the intents reach it

/// What the intents reach the app through, which the app fills at launch.
@MainActor
public final class IntentsBridge {
    public static let shared = IntentsBridge()

    public var store: AppStore?
    public var navigator: Navigator?
    /// Where Spotlight's shelf is saved, read while the engine has not answered.
    public var shelfFile: URL?
    /// Start the engine, for an intent that runs before the app has.
    public var start: (@MainActor () -> Void)?
    /// How long an intent waits for the engine's answer.
    var patience: Duration = .seconds(10)
    private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        defaults.register(defaults: [SpotlightIndexer.preference: true])
    }

    /// Whether titles are suggested, to Shortcuts and as App Shortcuts in Spotlight: as "Show
    /// titles in Spotlight" says.
    var suggestsTitles: Bool { defaults.bool(forKey: SpotlightIndexer.preference) }

    /// The shelf as the app has it now.
    func shelf() -> Shelf {
        let saved = shelfFile.flatMap(Shelf.load) ?? Shelf()
        guard let store else { return saved }
        return Shelf.current(store, over: saved)
    }

    func directory() -> TitleDirectory {
        var directory = TitleDirectory(shelf: shelf())
        if let store {
            for (id, entry) in store.titles.entries where !entry.title.isEmpty {
                directory.known[id] = ShelfTitle(entry)
            }
        }
        return directory
    }

    // MARK: The titles query

    /// The titles with these ids, in that order: the shelf's, or any other the app knows.
    public func titles(for identifiers: [Int]) -> [ShelfTitle] {
        directory().titles(for: identifiers)
    }

    /// The shelf's titles with every word of `string` in one of their names.
    public func titles(matching string: String) -> [ShelfTitle] {
        directory().matching(string)
    }

    /// The titles to suggest, Continue's first; none with "Show titles in Spotlight" off.
    public func suggestedTitles() -> [ShelfTitle] {
        suggestsTitles ? directory().suggested : []
    }

    // MARK: What the intents do

    /// Open a title's page.
    public func open(_ id: AnilistId) {
        navigator?.reveal(id)
    }

    /// What "Continue watching" does with Continue's hero, its first row: play its episode, or
    /// say there is nothing out to play.
    public func continueWatching(now: Date = .now) async -> ContinueAnswer {
        guard let store else { return .nothing }
        if await engineRunning() {
            if !store.continuing.list.state.isAnswered { store.loadContinue() }
            await wait { store.continuing.list.state.isAnswered }
        }
        let queue = store.continuing.queue(store.titles)
        if let hero = queue.inProgress.first ?? queue.newThisWeek.first {
            let title = hero.entry.showTitle
            guard hero.playable else {
                return .caughtUp(title: title, episode: hero.episode, airs: hero.airing.spoken(now: now))
            }
            return .play(hero.id, episode: hero.episode, title: title)
        }
        // The engine has not answered: Continue's first title as it was saved, at its next episode.
        let shelf = shelf()
        if let id = shelf.sources[.continue]?.first, let saved = shelf.titles[id], let episode = saved.episode {
            return .play(id, episode: episode, title: saved.title)
        }
        return .nothing
    }

    /// Play the hero's episode from where it was left, over Continue: the player opens.
    public func play(_ id: AnilistId, episode: UInt32) {
        navigator?.open(.home)
        store?.send(.play(id: id, episode: String(episode)))
    }

    /// Today's broadcasts, earliest first, yours marked, or `nil` when the schedule could not be
    /// had. Signed out, nothing is yours and the schedule is the public one.
    public func airingToday(now: Date = .now) async -> [TodaysBroadcasts.Row]? {
        guard let store, await engineRunning() else { return nil }
        if !store.calendar.list.state.isAnswered {
            if store.calendar.list.state != .loading { store.loadCalendar() }
            await wait { store.calendar.list.state.isAnswered }
        }
        guard store.calendar.list.state.isAnswered else { return nil }
        return TodaysBroadcasts.rows(store.broadcasts(mine: false), on: now)
    }

    /// Whether the engine is up, starting it if the app has not, and waiting a while for it.
    private func engineRunning() async -> Bool {
        guard let store else { return false }
        if store.phase == .starting { start?() }
        await wait { store.phase != .starting }
        return store.phase == .running
    }

    private func wait(_ done: () -> Bool) async {
        let deadline = ContinuousClock.now + patience
        while !done(), ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(50))
        }
    }
}

// MARK: - What the intents say

/// What "Continue watching" found.
public enum ContinueAnswer: Equatable, Sendable {
    /// The hero's episode, out and ready to play.
    case play(AnilistId, episode: UInt32, title: String)
    /// Everything out is watched: the hero's next episode, and when it airs, said aloud.
    case caughtUp(title: String, episode: UInt32, airs: String?)
    /// Nothing on Continue.
    case nothing

    /// What Siri says (native-spec/native-layer.md, App Intents): shows and episode numbers,
    /// never episode titles.
    public var spoken: String {
        switch self {
        case .play(_, let episode, let title):
            "Starting episode \(episode) of \(title)."
        case .caughtUp(let title, let episode, let airs?):
            "You're caught up. \(title) episode \(episode) airs \(airs)."
        case .caughtUp(let title, let episode, nil):
            "You're caught up. \(title) episode \(episode) is not out yet."
        case .nothing:
            "Nothing to continue yet."
        }
    }
}

extension EpisodeAvailability {
    /// When it airs, as Siri says it: "Thursday at 16:30", "today at 16:30", "29 Oct at 16:30".
    func spoken(now: Date = .now) -> String? {
        guard case .upcoming(let seconds?) = self else { return nil }
        let date = now.addingTimeInterval(TimeInterval(seconds))
        let time = AirTime.clock(date)
        let words = AirTime.words(date, now: now)
        guard words.hasSuffix(time) else { return words }
        let day = words.dropLast(time.count).trimmingCharacters(in: CharacterSet(charactersIn: ", "))
        return "\(day) at \(time)"
    }
}

/// Today's broadcasts, as the snippet lists them: every one airing, yours marked.
public enum TodaysBroadcasts {
    public struct Row: Identifiable, Equatable, Sendable {
        public var id: String { "\(title)-\(episode)-\(time)" }
        /// "16:30", on the device's clock.
        public let time: String
        public let title: String
        public let episode: UInt32
        /// Whether it has aired already.
        public let aired: Bool
        /// Whether it is yours: under way here, or on your Watching or Planning list.
        public let marked: Bool
    }

    /// `broadcasts` on the day `now` falls on, earliest first.
    @MainActor
    static func rows(_ broadcasts: [Broadcast], on now: Date, calendar: Calendar = .current) -> [Row] {
        broadcasts
            .filter { calendar.isDate($0.airs, inSameDayAs: now) }
            .sorted { $0.airs < $1.airs }
            .map {
                Row(time: AirTime.clock($0.airs), title: $0.entry.showTitle, episode: $0.episode,
                    aired: $0.airs <= now, marked: AppStore.isMine($0.entry))
            }
    }

    /// What Siri says: your shows' times, or that none of yours airs today.
    public static func summary(_ rows: [Row]) -> String {
        let yours = rows.filter(\.marked)
        guard !yours.isEmpty else {
            switch rows.count {
            case 0: return "Nothing airing today."
            case 1: return "None of your shows airs today. 1 other title does."
            default: return "None of your shows airs today. \(rows.count) other titles do."
            }
        }
        let said = yours.map { "\($0.title) episode \($0.episode) at \($0.time)" }
        // In English, as everything the app says is: "A, B and C".
        let joined = said.count < 2 ? said.joined() : said.dropLast().joined(separator: ", ") + " and " + said.last!
        return "Today: \(joined)."
    }
}

/// The snippet: each of today's broadcasts, its time and the title's episode, yours with a
/// bookmark. The system's own look until the NativeIntents board is drawn into it.
public struct AiringTodaySnippet: View {
    let rows: [TodaysBroadcasts.Row]

    public init(rows: [TodaysBroadcasts.Row]) {
        self.rows = rows
    }

    public var body: some View {
        if !rows.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(rows) { row in
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(row.time)
                            .monospacedDigit()
                            .foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(row.title)
                                .lineLimit(1)
                                .foregroundStyle(row.marked ? .primary : .secondary)
                            Text(row.aired ? "Episode \(row.episode) · out" : "Episode \(row.episode)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer(minLength: 0)
                        if row.marked {
                            Image(systemName: "bookmark.fill")
                                .foregroundStyle(.tint)
                                .accessibilityLabel(L.libraryOnYourList)
                        }
                    }
                    .accessibilityElement(children: .combine)
                }
            }
            .padding()
        }
    }
}
