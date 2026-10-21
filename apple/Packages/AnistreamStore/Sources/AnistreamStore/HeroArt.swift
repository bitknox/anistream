// A title's wide art, by one rule every screen with a hero shares: Continue's preview, a title's
// page on every device, the iPhone's hero and cards, and the player's starting screen.

import AnistreamKit
import Foundation

/// What fills a title's wide frame.
///
/// 1. Its AniList banner, when it has one.
/// 2. Else a still the viewer has already seen: the last watched episode's, else episode 1's. A
///    still from further on could give away what happens, so none is ever used.
/// 3. Else no wide art at all: the screen drops the frame and sets the cover beside the title.
///
/// A portrait cover never fills the frame, and never stands in a plate of its own colour: that
/// read as empty, and put the cover on screen twice where the page shows it too.
public enum HeroArt: Equatable, Sendable {
    case banner(String)
    case still(String)
    /// No banner, and the episodes whose stills might stand in are on their way: the cover's
    /// colour as a plate, without the cover, so nothing shows twice and nothing jumps.
    case waiting
    /// Nothing wide to show.
    case compact

    /// The art for `entry`, with its episode table as far as it is known: `nil` while it loads.
    public static func resolve(_ entry: Entry, episodes: [EpisodeRow]?) -> HeroArt {
        if let banner = entry.bannerUrl { return .banner(banner) }
        guard let episodes else { return .waiting }
        if let still = seenStill(episodes, done: entry.progress?.done ?? 0) { return .still(still) }
        return .compact
    }

    /// The still of the latest episode watched that has one, else episode 1's: never one from an
    /// episode not yet seen past the first. Watched is the table's own word, or within what the
    /// tracker counts as done.
    public static func seenStill(_ rows: [EpisodeRow], done: UInt32) -> String? {
        let watched = rows.filter { row in
            row.completed || Double(row.number).map { $0 <= Double(done) } == true
        }
        let latest = watched
            .filter { $0.thumbnail != nil }
            .max { (Double($0.number) ?? 0) < (Double($1.number) ?? 0) }
        if let still = latest?.thumbnail { return still }
        let first = rows.first { Double($0.number) == 1 } ?? rows.first
        return first?.thumbnail
    }

    /// The URL to draw, when there is one.
    public var url: String? {
        switch self {
        case .banner(let url), .still(let url): url
        case .waiting, .compact: nil
        }
    }

    public var isCompact: Bool { self == .compact }
}

extension AppStore {
    /// `entry`'s wide art, by the episodes known now.
    public func heroArt(_ entry: Entry) -> HeroArt {
        HeroArt.resolve(entry, episodes: titles.episodes[entry.id])
    }
}

