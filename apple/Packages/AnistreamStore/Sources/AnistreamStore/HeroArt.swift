// A title's wide art, by one rule every screen with a hero shares: Continue's preview, a title's
// page on every device, the iPhone's hero and cards, and the player's starting screen.

import AnistreamKit
import Foundation

/// What fills a title's wide frame.
///
/// 1. Its AniList banner, when it has one.
/// 2. Else a still the viewer has already seen: the last watched episode's, else episode 1's. A
///    still from further on could give away what happens, so none is ever used.
/// 3. On the Mac, else the cover filling the frame: cropped around the part of it that draws the
///    eye, or blurred behind itself where it is too small to crop (the hover card board's No wide
///    art rule, drawn by the screens).
/// 4. Else no wide art at all: the screen drops the frame and sets the cover beside the title.
///
/// The cover never stands in a plate of its own colour: that read as empty.
public enum HeroArt: Equatable, Sendable {
    case banner(String)
    case still(String)
    /// No banner and no seen still: the cover fills the frame.
    case cover(String)
    /// No banner, and the episodes whose stills might stand in are on their way: the cover's
    /// colour as a plate, without the cover, so nothing shows twice and nothing jumps.
    case waiting
    /// Nothing wide to show.
    case compact

    /// The art for `entry`, with its episode table as far as it is known: `nil` while it loads.
    /// `coverFills` lets the cover fill the frame when there is nothing wider.
    public static func resolve(_ entry: Entry, episodes: [EpisodeRow]?, coverFills: Bool = false) -> HeroArt {
        if let banner = entry.bannerUrl { return .banner(banner) }
        guard let episodes else { return .waiting }
        if let still = seenStill(episodes, done: entry.progress?.done ?? 0) { return .still(still) }
        if coverFills, let cover = entry.coverUrl { return .cover(cover) }
        return .compact
    }

    /// The still of the latest episode watched that has one, else episode 1's: never one from an
    /// episode not yet seen past the first. Watched is the table's own word, or within what the
    /// tracker counts as done.
    public static func seenStill(_ rows: [EpisodeRow], done: UInt32) -> String? {
        if let still = watchedStill(rows, done: done) { return still }
        let first = rows.first { Double($0.number) == 1 } ?? rows.first
        return first?.thumbnail
    }

    /// The still of the latest episode watched that has one, and nothing else: where the episode
    /// that would stand in unwatched could be the one the card is about, as a Calendar tile's
    /// airing episode is.
    public static func watchedStill(_ rows: [EpisodeRow], done: UInt32) -> String? {
        rows.filter { row in row.completed || Double(row.number).map { $0 <= Double(done) } == true }
            .filter { $0.thumbnail != nil }
            .max { (Double($0.number) ?? 0) < (Double($1.number) ?? 0) }?
            .thumbnail
    }

    /// The URL to draw, when there is one.
    public var url: String? {
        switch self {
        case .banner(let url), .still(let url): url
        case .cover, .waiting, .compact: nil
        }
    }

    public var isCompact: Bool { self == .compact }
}

extension AppStore {
    /// `entry`'s wide art, by the episodes known now.
    public func heroArt(_ entry: Entry) -> HeroArt {
        #if os(macOS)
        HeroArt.resolve(entry, episodes: titles.episodes[entry.id], coverFills: true)
        #else
        HeroArt.resolve(entry, episodes: titles.episodes[entry.id])
        #endif
    }
}

