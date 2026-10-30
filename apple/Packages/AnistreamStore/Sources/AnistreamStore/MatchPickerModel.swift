// What the match sheet shows: the question a source could not answer for a title, what a search
// of the source by hand found, the pick on its way, and what the title is matched to now.

import AnistreamKit
import Foundation
import Observation

/// The titles a source offered for a show when it could not choose between them, from
/// `Event::MatchChoices` answering the ladder.
public struct MatchAsked: Equatable, Sendable {
    public let providerId: String
    /// The source's name, as a person reads it.
    public let source: String
    public let candidates: [MatchCandidate]

    /// The candidates the ladder did not rule out, which the page's line counts.
    public var possible: Int { candidates.count { $0.rejected == nil } }
}

/// A search of a source by hand, from the sheet's field.
public enum MatchSearch: Equatable, Sendable {
    case searching(query: String)
    /// What the source found. `source` is its name, `providerId` the one to pin.
    case found(query: String, providerId: String, source: String, candidates: [MatchCandidate])
    /// The source answered with nothing.
    case empty(query: String, source: String)
    /// The source could not be searched. `reason` is the engine's, worded.
    case failed(query: String, reason: String)

    public var query: String {
        switch self {
        case .searching(let query), .found(let query, _, _, _), .empty(let query, _), .failed(let query, _): query
        }
    }
}

/// A pick that has settled, for the notice that says it and the sheet that closes on it.
public struct MatchSettled: Equatable, Sendable {
    public let id: AnilistId
    public let title: String
    public let source: String
    /// Tells two of the same picks apart.
    public let serial: Int
}

/// The match sheet's state per title.
@Observable @MainActor
public final class MatchPickerModel {
    /// The open question per title: the source could not choose, so the page asks.
    public private(set) var asked: [AnilistId: MatchAsked] = [:]
    /// What each title is matched to now, from `Event::TitleMatch`. A title in `known` but not
    /// here is matched to nothing.
    public private(set) var current: [AnilistId: CurrentMatch] = [:]
    /// Titles whose match the engine has said, matched or not.
    public private(set) var known: Set<AnilistId> = []
    /// The newest search by hand per title.
    public private(set) var searches: [AnilistId: MatchSearch] = [:]
    /// The key picked per title, while the source lists its episodes.
    public private(set) var picking: [AnilistId: ProviderKey] = [:]
    /// Why the last pick for a title could not be used.
    public private(set) var pickFailed: [AnilistId: String] = [:]
    /// The newest pick that settled.
    public private(set) var settled: MatchSettled?

    @ObservationIgnored private var pickedTitle: [AnilistId: (title: String, source: String)] = [:]
    @ObservationIgnored private var serial = 0

    public init() {}

    /// Whether there is a source to match a title on, for Fix this match.
    public func canFix(_ providers: ProvidersModel) -> Bool {
        providers.rows.contains { $0.usable }
    }

    /// Whether the page for `id` asks for its match in place of Play.
    public func asks(_ id: AnilistId) -> Bool {
        asked[id] != nil
    }

    /// The source the sheet for `id` searches: the one that asked, else the one matched now.
    public func source(for id: AnilistId) -> (providerId: String, name: String)? {
        if let asked = asked[id] { return (asked.providerId, asked.source) }
        if let current = current[id] { return (current.providerId, current.source) }
        if case .found(_, let providerId, let source, _) = searches[id] { return (providerId, source) }
        return nil
    }

    func asked(id: AnilistId, providerId: String, source: String, candidates: [MatchCandidate]) {
        asked[id] = MatchAsked(providerId: providerId, source: source, candidates: candidates)
    }

    func searching(_ id: AnilistId, query: String) {
        searches[id] = .searching(query: query)
        pickFailed[id] = nil
    }

    func found(id: AnilistId, query: String, providerId: String, source: String, candidates: [MatchCandidate]) {
        // An answer to a search the sheet has moved on from is not shown.
        guard searches[id]?.query == query else { return }
        searches[id] = candidates.isEmpty
            ? .empty(query: query, source: source)
            : .found(query: query, providerId: providerId, source: source, candidates: candidates)
    }

    func searchFailed(id: AnilistId, query: String, reason: String) {
        guard searches[id]?.query == query else { return }
        searches[id] = .failed(query: query, reason: reason)
    }

    func picking(_ id: AnilistId, key: ProviderKey, title: String, source: String) {
        picking[id] = key
        pickFailed[id] = nil
        pickedTitle[id] = (title, source)
    }

    func fixFailed(id: AnilistId, reason: String) {
        picking[id] = nil
        pickedTitle[id] = nil
        pickFailed[id] = reason
    }

    func matched(id: AnilistId, to match: CurrentMatch?) {
        known.insert(id)
        current[id] = match
        guard let match else { return }
        // Matched: the question is answered.
        asked[id] = nil
        if picking.removeValue(forKey: id) != nil {
            let said = pickedTitle.removeValue(forKey: id)
            serial += 1
            settled = MatchSettled(
                id: id, title: match.title ?? said?.title ?? "", source: match.source, serial: serial
            )
            searches[id] = nil
        }
    }

    /// The person let the engine match `id` again: what they chose is forgotten here at once.
    func cleared(_ id: AnilistId) {
        current[id] = nil
        searches[id] = nil
        picking[id] = nil
        pickFailed[id] = nil
    }

    /// The sheet for `id` closed: a search or pick it showed is not kept for the next one.
    public func closed(_ id: AnilistId) {
        searches[id] = nil
        pickFailed[id] = nil
    }
}
