// An episode's discussion: the AniList thread open in a sheet, pane or window, and which episodes
// have one, as the engine says.

import AnistreamKit
import Foundation
import Observation

/// Which episode of which title a discussion belongs to.
public struct DiscussionKey: Hashable, Codable, Sendable {
    public var media: AnilistId
    public var episode: UInt32

    public init(media: AnilistId, episode: UInt32) {
        self.media = media
        self.episode = episode
    }
}

@Observable @MainActor
public final class DiscussionModel {
    /// The discussion that is open, whole, said again on every change; nil once it closes.
    public private(set) var open: DiscussionView?
    /// The episodes the engine found a thread for, with every comment in it.
    public private(set) var found: [DiscussionKey: UInt32] = [:]
    /// The discussion a person asked to read, shown until dismissed.
    public var target: DiscussionKey?

    public init() {}

    /// Ask to read `key`: the sheet, pane or window it opens in follows.
    public func show(_ key: DiscussionKey) {
        target = key
    }

    func view(_ view: DiscussionView) {
        open = view
    }

    func found(_ key: DiscussionKey, comments: UInt32) {
        found[key] = comments
    }

    func closed() {
        open = nil
    }

    /// The comments the engine counts for an episode; nil when it has no thread or has not said.
    public func count(_ media: AnilistId, episode: UInt32) -> UInt32? {
        found[DiscussionKey(media: media, episode: episode)]
    }
}

/// Whether the player offers the discussion of the episode on screen: from 85% of it, the point it
/// counts as watched, and then for the rest of that episode, even if someone seeks back.
public struct PlayerDiscussion: Sendable {
    /// The share of an episode after which it counts as watched.
    public static let watchedAt = 0.85

    public private(set) var reached = false

    public init() {}

    /// Take the playhead's place; true once the episode has been far enough on.
    @discardableResult
    public mutating func see(position: Double, duration: Double?) -> Bool {
        if !reached, let duration, duration > 0, position >= duration * Self.watchedAt { reached = true }
        return reached
    }

    /// The episode number a discussion belongs to; nil for a special or a number with a fraction.
    public static func episode(_ number: String?) -> UInt32? {
        number.flatMap { UInt32($0) }
    }
}
