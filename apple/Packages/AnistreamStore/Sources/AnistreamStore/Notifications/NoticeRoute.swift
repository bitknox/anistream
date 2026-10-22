// Where a new-episode notification takes the app: what was tapped, read back from the
// notification's own record, so the app does the same whether it was running or not.

import AnistreamKit
import Foundation
import UserNotifications

/// A notification's actions, by the identifier the system hands back.
public enum NoticeAction: String, Sendable, CaseIterable {
    /// Opens the app into the player.
    case play
    /// Marks the episode watched, in the background, as the engine's mark does: queued for the
    /// trackers when offline.
    case markWatched = "mark-watched"
}

/// What the app does for a notification that was tapped, or one of whose actions was.
public enum NoticeRoute: Equatable, Sendable {
    /// The notification itself: the show's title page.
    case open(AnilistId)
    /// Play: the episode it names, which for someone behind is the next one they have to watch.
    case play(AnilistId, episode: UInt32)
    /// Mark watched: the episode that came out.
    case markWatched(AnilistId, episode: UInt32)

    /// The keys of a notification's record.
    enum Key {
        static let title = "title"
        static let episode = "episode"
        static let play = "play"
    }

    /// What a notification keeps of `notice`: the show, the episode, and the episode Play starts.
    public static func userInfo(_ notice: PlannedNotice) -> [String: Int] {
        [Key.title: Int(notice.show), Key.episode: Int(notice.episode), Key.play: Int(notice.playEpisode)]
    }

    /// The route for `action`, the identifier the system handed back, on a notification whose
    /// record is `userInfo`. `nil` for a dismissal, an action this build does not know, or a
    /// notification that is not a new episode's.
    public init?(action: String, userInfo: [AnyHashable: Any]) {
        func number(_ key: String) -> Int? {
            switch userInfo[key] {
            case let value as Int: value
            case let value as NSNumber: value.intValue
            case let value as String: Int(value)
            default: nil
            }
        }
        guard let title = number(Key.title), let show = AnilistId(exactly: title), show > 0 else { return nil }
        let episode = number(Key.episode).flatMap(UInt32.init(exactly:))
        let play = number(Key.play).flatMap(UInt32.init(exactly:)) ?? episode
        switch action {
        case UNNotificationDefaultActionIdentifier:
            self = .open(show)
        case NoticeAction.play.rawValue:
            guard let play, play > 0 else { return nil }
            self = .play(show, episode: play)
        case NoticeAction.markWatched.rawValue:
            guard let episode, episode > 0 else { return nil }
            self = .markWatched(show, episode: episode)
        default:
            return nil
        }
    }

    /// Whether the app has to be in front for it: Play and opening the page do, a mark does not.
    public var needsForeground: Bool {
        if case .markWatched = self { return false }
        return true
    }
}

extension AppStore {
    /// Do what a notification's action asks, of the engine: play the episode, or mark it watched.
    /// Opening the title page is the navigator's.
    public func perform(_ route: NoticeRoute) {
        switch route {
        case .open:
            break
        case .play(let show, let episode):
            send(.play(id: show, episode: String(episode)))
        case .markWatched(let show, let episode):
            send(.setWatched(id: show, episodes: [String(episode)], watched: true))
            refreshNotices()
        }
    }
}
