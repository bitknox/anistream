// Handoff: the title page open on one device, or the episode playing there, picked up on another
// (native-spec/native-layer.md, Handoff).

import AnistreamKit
import Foundation

/// What one device hands another.
public enum Handoff: Equatable, Sendable {
    /// A title's page.
    case title(AnilistId)
    /// An episode, `position` seconds into it, in the translation it plays in when known.
    case playing(AnilistId, episode: String, position: Double, translation: Translation? = nil)

    public var id: AnilistId {
        switch self {
        case .title(let id), .playing(let id, _, _, _): id
        }
    }

    /// The activity's type: one of `types`.
    public func activityType(_ types: HandoffTypes) -> String {
        switch self {
        case .title: types.title
        case .playing: types.playing
        }
    }

    /// What the activity carries: `title`, the AniList id, and for an episode its number, where
    /// in it in seconds, and the translation.
    public var userInfo: [String: Any] {
        switch self {
        case .title(let id):
            return [Key.title: Int(id)]
        case .playing(let id, let episode, let position, let translation):
            var info: [String: Any] = [Key.title: Int(id), Key.episode: episode, Key.position: position]
            if let translation { info[Key.translation] = Self.name(translation) }
            return info
        }
    }

    /// What has to be there for the activity to mean anything: the translation may be missing.
    var requiredKeys: Set<String> {
        switch self {
        case .title: [Key.title]
        case .playing: [Key.title, Key.episode, Key.position]
        }
    }

    /// What an activity of one of `types` hands over, when it is whole.
    public init?(activityType: String, userInfo: [AnyHashable: Any]?, types: HandoffTypes) {
        guard let raw = userInfo?[Key.title] as? Int, let id = AnilistId(exactly: raw) else { return nil }
        switch activityType {
        case types.title:
            self = .title(id)
        case types.playing:
            guard let episode = userInfo?[Key.episode] as? String, !episode.isEmpty,
                  let position = userInfo?[Key.position] as? Double, position.isFinite, position >= 0
            else { return nil }
            let translation = (userInfo?[Key.translation] as? String).flatMap(Self.translation)
            self = .playing(id, episode: episode, position: position, translation: translation)
        default:
            return nil
        }
    }

    public init?(_ activity: NSUserActivity, types: HandoffTypes = .app) {
        self.init(activityType: activity.activityType, userInfo: activity.userInfo, types: types)
    }

    /// Say this on `activity`: what the system shows of it, what it carries, and that it may be
    /// handed off. Not offered to search or to Siri: Spotlight has its own.
    public func describe(_ activity: NSUserActivity, title: String) {
        activity.title = title
        activity.userInfo = userInfo
        activity.requiredUserInfoKeys = requiredKeys
        activity.isEligibleForHandoff = true
        activity.isEligibleForSearch = false
        #if os(iOS)
        activity.isEligibleForPrediction = false
        #endif
    }

    enum Key {
        static let title = "title"
        static let episode = "episode"
        static let position = "position"
        static let translation = "translation"
    }

    static func name(_ translation: Translation) -> String {
        switch translation {
        case .sub: "sub"
        case .dub: "dub"
        }
    }

    static func translation(_ name: String) -> Translation? {
        switch name {
        case "sub": .sub
        case "dub": .dub
        default: nil
        }
    }
}

/// The activity types an app hands off, which its Info.plist lists under `NSUserActivityTypes`:
/// the app's `tv.anistream.title` and `tv.anistream.playing`, and for another build its bundle
/// id's own (`tv.anistream.fixtures.title`), so the Fixtures app's fixture titles never reach the
/// app on another device, though both are signed by the same team.
public struct HandoffTypes: Equatable, Sendable {
    /// A title's page: `tv.anistream.title`.
    public let title: String
    /// An episode playing: `tv.anistream.playing`.
    public let playing: String

    public init(prefix: String) {
        title = prefix + ".title"
        playing = prefix + ".playing"
    }

    /// The types of the build with this bundle id.
    public init(bundleIdentifier: String?) {
        let id = bundleIdentifier ?? "tv.anistream.app"
        self.init(prefix: id == "tv.anistream.app" ? "tv.anistream" : id)
    }

    /// This app's.
    public static let app = HandoffTypes(bundleIdentifier: Bundle.main.bundleIdentifier)
}

/// An episode handed over from another device, offered on its title's page: "Resume Episode 7 ·
/// 12:40", which plays it from there rather than from this device's own place in it.
public struct HandedOff: Equatable, Sendable {
    public let id: AnilistId
    public let episode: String
    public let position: Double

    public init(id: AnilistId, episode: String, position: Double) {
        self.id = id
        self.episode = episode
        self.position = position
    }

    /// Where in the episode, as the resume clock says it: "12:40".
    public var clock: String {
        ResumePoint(position: position, fraction: nil).clock
    }
}

/// A play that is to start at a place of its own: the episode is played as any other, and once
/// its file is open the player is moved to `position`. The engine's own resume point is mpv's
/// `start` and is taken at the load, so the move comes after it and wins.
struct StartAt: Equatable {
    let id: AnilistId
    let episode: String
    let position: Double
    /// The engine has staged this play.
    var staged = false

    /// The engine staged a play: this one, or another, which this gives way to. Whether it is
    /// still to start.
    mutating func staged(id: AnilistId, episode: String) -> Bool {
        guard id == self.id, episode == self.episode else { return false }
        staged = true
        return true
    }

    /// What to send after `command` has gone: the move, once the staged play's file is open.
    func after(_ command: Command) -> Command? {
        guard staged, case .playerReport(_, .fileLoaded) = command else { return nil }
        return .player(.seekTo(position))
    }
}
