// The links a widget opens the app with, through its anistream:// scheme.

import Foundation

/// Where a tap on a widget goes.
public enum WidgetLink: Sendable, Equatable {
    /// A title's page: `anistream://title/900001`.
    case title(Int)
    /// The player, on an episode of a title: `anistream://play/900001/7`.
    case play(Int, episode: Int)
    /// The Calendar, on today: `anistream://calendar`.
    case calendar
    /// Settings › Accounts, to connect a tracker: `anistream://settings/accounts`.
    case accounts

    public static let scheme = "anistream"

    public var url: URL {
        var components = URLComponents()
        components.scheme = Self.scheme
        switch self {
        case .title(let id):
            components.host = "title"
            components.path = "/\(id)"
        case .play(let id, let episode):
            components.host = "play"
            components.path = "/\(id)/\(episode)"
        case .calendar:
            components.host = "calendar"
        case .accounts:
            components.host = "settings"
            components.path = "/accounts"
        }
        return components.url!
    }

    /// The link `url` is, when it is one of these.
    public init?(url: URL) {
        guard url.scheme?.lowercased() == Self.scheme, let host = url.host()?.lowercased() else { return nil }
        let parts = url.pathComponents.filter { $0 != "/" }
        switch (host, parts.count) {
        case ("title", 1):
            guard let id = Int(parts[0]), id > 0 else { return nil }
            self = .title(id)
        case ("play", 2):
            guard let id = Int(parts[0]), id > 0, let episode = Int(parts[1]), episode > 0 else { return nil }
            self = .play(id, episode: episode)
        case ("calendar", 0):
            self = .calendar
        case ("settings", 1) where parts[0] == "accounts":
            self = .accounts
        default:
            return nil
        }
    }
}
