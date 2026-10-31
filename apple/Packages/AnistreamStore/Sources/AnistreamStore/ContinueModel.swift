// Continue: what you were watching, from local history, as a short queue.

import AnistreamKit
import AnistreamStrings
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
    /// Nothing from the next episode on has a source, so there is nothing to play.
    public var unsourced = false

    /// Whether this is a new episode rather than one under way.
    public var isNew: Bool { airedAgo != nil }

    /// Whether the episode is out: one left part-way is, and otherwise the title's broadcasts say.
    public var airing: EpisodeAvailability {
        resume == nil ? entry.airing(of: episode) : .aired
    }

    /// Whether the row has an episode to play: not when everything out is watched and the next
    /// one is still to air.
    public var playable: Bool { airing.hasAired && !unsourced }

    /// Whether the show was watched dubbed: its episode line says so, and a sub is left unmarked.
    public var dubbed: Bool { entry.watchedIn == .dub }

    /// The episode as a row names it, "E7", or "E7 · Dubbed" for a dub.
    public var episodeShort: String {
        dubbed ? L.titleEpisodeShortDubbed(episode: Int(episode)) : L.episodeNumberShort(number: Int(episode))
    }

    /// What the row says it plays: "Continue E7 · 10:06", "Play E4", "Play E9 · out 2h ago", or
    /// with nothing out to watch, "Caught up · E9 Saturday 18:30". A dub is marked after the
    /// episode, "Continue E7 · Dubbed · 10:06"; what is still to air is not.
    public var action: String {
        let number = Int(episode)
        if let resume {
            return dubbed
                ? L.continueRowResumeDubbed(episode: number, clock: resume.clock)
                : L.continueRowResume(episode: number, clock: resume.clock)
        }
        if unsourced { return episodeShort }
        if !playable {
            guard let when = airing.airs() else { return L.continueRowCaughtUpPlain }
            return L.continueRowCaughtUp(episode: number, when: when)
        }
        if let airedAgo {
            let ago = Wording.ago(airedAgo)
            return dubbed
                ? L.continueRowStartAiredDubbed(episode: number, ago: ago)
                : L.continueRowStartAired(episode: number, ago: ago)
        }
        return dubbed ? L.continueRowStartDubbed(episode: number) : L.continueRowStart(episode: number)
    }

    /// The same, as a button says it: "Resume episode 7 · 10:06" or "Play episode 4", a dub
    /// marked after the episode, "Resume episode 7 · Dubbed · 10:06". With nothing out to play it
    /// is no button but a line: "Episode 9 airs Saturday 18:30".
    public var buttonTitle: String {
        let number = Int(episode)
        if !playable {
            return airing.airs().map { L.episodeAirs(episode: number, when: $0) } ?? L.episodeNotOut(episode: number)
        }
        if let resume {
            return dubbed
                ? L.titleButtonResumeDubbed(episode: number, time: resume.clock)
                : L.titleButtonResume(episode: number, time: resume.clock)
        }
        return dubbed ? L.titleButtonPlayDubbed(episode: number) : L.titleButtonPlay(episode: number)
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

    /// At most this many rows: the engine's own limit for the queue.
    public static let rows = 15
    /// What counts as this week.
    public static let week: Int64 = 7 * 86_400

    /// The queue of `ids`, newest watch first as the engine orders them, with their rows from
    /// `titles`.
    @MainActor
    public init(ids: [AnilistId], titles: TitleCache) {
        let items = ids.compactMap { titles[$0] }.prefix(Self.rows).map { Self.item($0, rows: titles.episodes[$0.id]) }
        inProgress = items.filter { !$0.isNew }
        newThisWeek = items.filter(\.isNew)
    }

    static func item(_ entry: Entry, rows: [EpisodeRow]? = nil) -> ContinueItem {
        let target = entry.playableNext(in: rows)
        let episode = target.episode
        let aired = entry.latestAired
        let airedAgo: Int64? =
            if entry.resume == nil, let aired, aired.episode == episode,
               let ago = aired.ago, ago < week { ago } else { nil }
        var item = ContinueItem(entry: entry, episode: episode, resume: entry.resume, airedAgo: airedAgo)
        item.unsourced = target.unsourced
        return item
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

    @discardableResult
    func apply(_ content: ContentList, titles: TitleCache) -> Bool {
        if list.apply(content, titles: titles, local: true) { return true }
        hidden = []
        return false
    }

    func hide(_ id: AnilistId) {
        hidden.insert(id)
    }
}
