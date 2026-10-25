// Continue: what you were watching, from local history, as a short queue.

import AnistreamKit
import Foundation
import Observation

/// One row of the queue: a title and the episode it plays next.
public struct ContinueItem: Identifiable, Equatable, Sendable {
    public var id: AnilistId { entry.id }
    public let entry: Entry
    /// The episode the row plays.
    public let episode: UInt32
    /// Where that episode was left, when it was.
    public let resume: ResumePoint?
    /// How many seconds ago that episode aired, when it is the newest out and aired this week.
    public let airedAgo: Int64?

    /// Whether this is a new episode rather than one under way.
    public var isNew: Bool { airedAgo != nil }

    /// Whether the episode is out: one left part-way is, and otherwise the title's broadcasts say.
    public var airing: EpisodeAvailability {
        resume == nil ? entry.airing(of: episode) : .aired
    }

    /// Whether the row has an episode to play: not when everything out is watched and the next
    /// one is still to air.
    public var playable: Bool { airing.hasAired }

    /// The episode's mark: "Dub" for a show watched dubbed, the engine's word for it, and nil for
    /// a sub, which is left unmarked.
    public var mark: String? { episodeMark(translation: entry.watchedIn) }

    /// `label` with the mark after it, as the engine words it: "E7 · Dub", or "E7" for a sub.
    public func marked(_ label: String) -> String {
        episodeLine(label: label, translation: entry.watchedIn)
    }

    /// What the row says it plays: "Resume E7 · 10:06", "Start E4", "Start E9 · out 2h ago", or
    /// with nothing out to watch, "Caught up · E9 Saturday 18:30". A dub is marked after the
    /// episode, "Resume E7 · Dub · 10:06"; what is still to air is not, as the engine has it.
    public var action: String {
        if let resume { return "Resume \(marked("E\(episode)")) · \(resume.clock)" }
        if !playable { return "Caught up · E\(episode) " + (airing.airs() ?? "not out yet") }
        if let airedAgo { return "Start \(marked("E\(episode)")) · out \(Wording.ago(airedAgo))" }
        return "Start \(marked("E\(episode)"))"
    }

    /// The same, as a Mac button says it, in title case: "Resume Episode 7 · 10:06", or for a dub
    /// "Resume Episode 7 · Dub · 10:06". With nothing out to play it is no button but a line:
    /// "Episode 9 airs Saturday 18:30".
    public var buttonTitle: String {
        if let resume { return "Resume \(marked("Episode \(episode)")) · \(resume.clock)" }
        if !playable { return "Episode \(episode) " + (airing.airs().map { "airs \($0)" } ?? "is not out yet") }
        return "Start \(marked("Episode \(episode)"))"
    }

    /// How far into the episode, for the row's meter, when it was left part-way.
    public var fraction: Double? {
        resume?.fraction
    }

    /// Whole minutes left in the episode, when its runtime is known: "14 min left".
    public var minutesLeft: Int? {
        guard let resume, let fraction = resume.fraction, fraction > 0 else { return nil }
        let left = resume.position / fraction - resume.position
        return max(1, Int((left / 60).rounded()))
    }
}

/// The queue, as Continue groups it.
public struct ContinueQueue: Equatable, Sendable {
    /// Titles under way: an episode left part-way, or the next one waiting.
    public var inProgress: [ContinueItem]
    /// Titles whose next episode is the newest out, and aired in the last week.
    public var newThisWeek: [ContinueItem]

    public var all: [ContinueItem] { inProgress + newThisWeek }
    public var isEmpty: Bool { inProgress.isEmpty && newThisWeek.isEmpty }

    /// At most this many rows: Continue is a shortlist, not a second library.
    public static let rows = 5
    /// What counts as this week.
    public static let week: Int64 = 7 * 86_400

    /// The queue of `ids`, newest watch first as the engine orders them, with their rows from
    /// `titles`.
    @MainActor
    public init(ids: [AnilistId], titles: TitleCache) {
        let items = ids.compactMap { titles[$0] }.prefix(Self.rows).map(Self.item)
        inProgress = items.filter { !$0.isNew }
        newThisWeek = items.filter(\.isNew)
    }

    static func item(_ entry: Entry) -> ContinueItem {
        let episode = entry.progress?.next ?? 1
        let aired = entry.latestAired
        let airedAgo: Int64? =
            if entry.resume == nil, let aired, aired.episode == episode,
               let ago = aired.ago, ago < week { ago } else { nil }
        return ContinueItem(entry: entry, episode: episode, resume: entry.resume, airedAgo: airedAgo)
    }
}

/// The Continue list.
@Observable @MainActor
public final class ContinueModel {
    public let list = ListModel()
    /// Titles taken off the queue in this session, ahead of the engine's next answer.
    public private(set) var hidden: Set<AnilistId> = []

    public init() {}

    /// The queue as it stands.
    public func queue(_ titles: TitleCache) -> ContinueQueue {
        ContinueQueue(ids: list.state.ids.filter { !hidden.contains($0) }, titles: titles)
    }

    func apply(_ content: ContentList, titles: TitleCache) {
        list.apply(content, titles: titles)
        hidden = []
    }

    func hide(_ id: AnilistId) {
        hidden.insert(id)
    }
}
