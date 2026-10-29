// What the app notifies of new episodes, worked out from what the engine says: one notification
// per episode, at the time it airs, in the words and with the actions the Native layer spec gives.
// Pure, so it is tested without the system's notification centre.

import AnistreamKit
import AnistreamStrings
import Foundation

/// One notification, to schedule for when an episode airs or to post now.
public struct PlannedNotice: Equatable, Sendable {
    /// `ep.<show>.<episode>.sub`: the same episode is the same notification, so scheduling it
    /// again replaces it rather than adding a second.
    public var identifier: String
    public var show: AnilistId
    public var episode: UInt32
    /// The episode Play starts: this one when caught up, the next one not watched when behind.
    public var playEpisode: UInt32
    /// The show's title, in the form the title setting picks: the notification's title, and the
    /// name a group of them lists.
    public var title: String
    public var body: String
    public var category: NoticeCategory
    /// When it fires: when the episode airs. `nil` posts it now.
    public var fireAt: Date?
    /// The picture: AniList's banner where there is one, otherwise the poster. Never a still.
    public var artURL: String?

    public init(
        identifier: String, show: AnilistId, episode: UInt32, playEpisode: UInt32, title: String,
        body: String, category: NoticeCategory, fireAt: Date?, artURL: String?
    ) {
        self.identifier = identifier
        self.show = show
        self.episode = episode
        self.playEpisode = playEpisode
        self.title = title
        self.body = body
        self.category = category
        self.fireAt = fireAt
        self.artURL = artURL
    }

    /// What tells one version of a notification from another, so an unchanged one is not
    /// scheduled again.
    var fingerprint: String {
        [title, body, category.rawValue, fireAt.map { String(Int($0.timeIntervalSince1970)) } ?? "now",
         artURL ?? "", String(playEpisode)].joined(separator: "\u{1F}")
    }
}

/// The two kinds of new-episode notification, by the actions they offer.
public enum NoticeCategory: String, Sendable, CaseIterable {
    /// Caught up: Play and Mark watched.
    case newEpisode = "new-episode"
    /// Behind: Play only, which starts the next episode for you. No Mark watched, which would
    /// mark an episode you have not reached.
    case behind = "new-episode-behind"
}

public enum NoticePlan {
    /// How many are scheduled at most: iOS keeps 64 pending for an app, and a few are left over.
    public static let limit = 60
    /// Every new-episode notification's thread, which the system groups them by.
    public static let thread = "new-episodes"
    /// What every new-episode notification's identifier starts with.
    public static let prefix = "ep."
    /// What the system says under a group: "3 more new episodes".
    public static var summaryFormat: String { L.noticeSummary }
    /// What a notification says when previews are hidden.
    public static var hiddenPreview: String { L.noticeHiddenPreview }

    /// The notification for episode `episode` of `show`: `sub` for one scheduled at the
    /// broadcast, which is the subtitled release's, and `dub` for one the check says once the
    /// dub is out.
    public static func identifier(show: AnilistId, episode: UInt32, translation: String = "sub") -> String {
        "\(prefix)\(show).\(episode).\(translation)"
    }

    /// The identifier's word for `translation`.
    static func word(_ translation: Translation?) -> String {
        translation == .dub ? "dub" : "sub"
    }

    /// The week's notifications, soonest first: each episode still to air of a show that
    /// notifies, as the engine says (a show switched on for itself whatever the setting, one
    /// being watched while it is on), once, at most `limit` of them. A show watched dubbed is
    /// left to the background check, which says it once its dub is out: AniList dates only the
    /// broadcast.
    public static func schedule(_ week: UpcomingNotices, now: Date) -> [PlannedNotice] {
        var seen = Set<String>()
        let rows = week.airings
            .filter { $0.notifies && $0.translation != .dub }
            .filter { Date(timeIntervalSince1970: TimeInterval($0.airsAt)) > now }
            .sorted { ($0.airsAt, $0.id, $0.episode) < ($1.airsAt, $1.id, $1.episode) }
        var planned: [PlannedNotice] = []
        for row in rows where planned.count < limit {
            let identifier = identifier(show: row.id, episode: row.episode)
            guard seen.insert(identifier).inserted else { continue }
            let words = words(episode: row.episode, nextToWatch: row.nextToWatch)
            planned.append(PlannedNotice(
                identifier: identifier, show: row.id, episode: row.episode, playEpisode: words.play,
                title: row.title, body: words.body, category: words.category,
                fireAt: Date(timeIntervalSince1970: TimeInterval(row.airsAt)),
                artURL: row.bannerUrl ?? row.coverUrl
            ))
        }
        return planned
    }

    /// What the background check found aired that no scheduled notification said, to post now:
    /// an episode of a show followed since the last schedule, one AniList moved earlier, or the
    /// dub of one watched dubbed, out now. Whether the viewer is behind comes from the week,
    /// where it has the show.
    public static func catchUp(
        _ said: [NewEpisode], scheduled: Set<String>, week: UpcomingNotices?
    ) -> [PlannedNotice] {
        var standing: [AnilistId: UpcomingNotice] = [:]
        for row in week?.airings ?? [] where standing[row.id] == nil {
            standing[row.id] = row
        }
        var seen = Set<String>()
        return said.compactMap { episode in
            let dubbed = episode.translation == .dub
            let identifier = identifier(show: episode.id, episode: episode.episode, translation: word(episode.translation))
            guard !scheduled.contains(identifier), seen.insert(identifier).inserted else { return nil }
            let next = standing[episode.id]?.nextToWatch ?? episode.nextForYou ?? episode.episode
            let words = words(episode: episode.episode, nextToWatch: next, dubbed: dubbed)
            return PlannedNotice(
                identifier: identifier, show: episode.id, episode: episode.episode,
                playEpisode: words.play, title: episode.title, body: words.body,
                category: words.category, fireAt: nil,
                artURL: standing[episode.id]?.bannerUrl ?? episode.coverUrl
            )
        }
    }

    /// The body and actions for `episode`, by where the viewer is: caught up, "Episode 9 is
    /// out.", with Play and Mark watched; behind, "Episode 10 is out. Next for you: episode 8.",
    /// with Play starting episode 8. Out in the dub the viewer watches the show in, "Episode 9 is
    /// out, dubbed." Never an episode's name.
    public static func words(episode: UInt32, nextToWatch: UInt32, dubbed: Bool = false)
        -> (body: String, category: NoticeCategory, play: UInt32)
    {
        let number = Int(episode)
        if nextToWatch < episode {
            let body = dubbed
                ? L.noticeOutDubbedBehind(episode: number, next: Int(nextToWatch))
                : L.noticeOutBehind(episode: number, next: Int(nextToWatch))
            return (body, .behind, nextToWatch)
        }
        return (dubbed ? L.noticeOutDubbed(episode: number) : L.noticeOut(episode: number), .newEpisode, episode)
    }
}
