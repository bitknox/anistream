// The episode a Play or Resume button plays, which is the engine's next one unless no source
// lists it: AniList says it is out, nothing can play it, and the button goes to the next episode
// that something can.

import AnistreamKit

/// The episode a title's main button plays.
public struct PlayableNext: Equatable, Sendable {
    public let episode: UInt32
    /// Every episode from the next one on is one no source lists: there is nothing to play.
    public let unsourced: Bool
}

extension Entry {
    /// The next episode to play, skipping those no source lists, in `rows` when the table is
    /// loaded. A part-watched episode is resumed as it is. With every later episode unsourced too
    /// the answer is the engine's own next one, flagged `unsourced`.
    public func playableNext(in rows: [EpisodeRow]?) -> PlayableNext {
        let next = progress?.next ?? 1
        guard resume == nil, let rows else { return PlayableNext(episode: next, unsourced: false) }
        let numbered = rows.compactMap { row in UInt32(row.number).map { ($0, row) } }
        guard numbered.contains(where: { $0.0 == next && $0.1.unsourced }) else {
            return PlayableNext(episode: next, unsourced: false)
        }
        let later = numbered.filter { $0.0 > next && !$0.1.unsourced }.map(\.0).min()
        return later.map { PlayableNext(episode: $0, unsourced: false) }
            ?? PlayableNext(episode: next, unsourced: true)
    }
}
