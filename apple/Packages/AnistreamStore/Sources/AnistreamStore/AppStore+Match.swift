// Fixing a source match: the commands the match sheet sends, and the engine's answers to them.

import AnistreamKit
import AnistreamStrings
import Foundation

extension AppStore {
    /// Search the title's source for `query`, as the sheet's field does on Return. `providerId`
    /// names the source; `nil` searches the sources in order.
    public func searchMatch(_ id: AnilistId, query: String, providerId: String?) {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return }
        match.searching(id, query: query)
        send(.manualSearch(id: id, query: query, providerId: providerId))
    }

    /// Match `id` to `candidate` on `providerId`'s source. The engine lists its episodes, and the
    /// sheet closes when they come.
    public func pickMatch(_ id: AnilistId, providerId: String, source: String, candidate: MatchCandidate) {
        match.picking(id, key: candidate.key, title: candidate.title, source: source)
        send(.fixMatch(id: id, providerId: providerId, key: candidate.key, title: candidate.title))
    }

    /// Forget the person's choice for `id` and let the engine match it again.
    public func matchAutomatically(_ id: AnilistId) {
        match.cleared(id)
        notices.post(Notice(kind: .info, text: L.matchCleared, hint: nil))
        send(.clearMatch(id: id))
    }

    func applyMatch(_ event: Event) {
        switch event {
        case .matchChoices(let id, let providerId, let candidates, let answers, let source):
            switch answers {
            case .ladder:
                match.asked(id: id, providerId: providerId, source: source, candidates: candidates)
            case .search(let query):
                match.found(id: id, query: query, providerId: providerId, source: source, candidates: candidates)
            }
        case .matchSearchFailed(let id, let query, let reason):
            match.searchFailed(id: id, query: query, reason: reason.text)
        case .matchFixFailed(let id, let reason):
            match.fixFailed(id: id, reason: reason.text)
        case .titleMatch(let id, let current):
            let before = match.settled
            match.matched(id: id, to: current)
            if let settled = match.settled, settled != before {
                let text = L.matchPicked(title: settled.title, source: settled.source)
                notices.post(Notice(kind: .info, text: text, hint: nil))
            }
        default:
            break
        }
    }
}
