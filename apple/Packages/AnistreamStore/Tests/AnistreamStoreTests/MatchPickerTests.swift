// The match sheet's state: the question a source asks, a search by hand, a pick and what the
// title is matched to after it.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
struct MatchPickerTests {
    private let id: AnilistId = 900_001

    private func candidate(_ key: String, _ title: String, rejected: Msg? = nil) -> MatchCandidate {
        MatchCandidate(title: title, key: key, similarity: 0.66, rejected: rejected, year: 2024, episodes: 12, format: .tv)
    }

    private var asked: Event {
        .matchChoices(id: id, providerId: "hotaru", candidates: [
            candidate("a", "Hoshi Tabi Diaries"),
            candidate("b", "Tabibito to Hoshikuzu"),
            candidate("c", "Hoshikuzu no Tabibito", rejected: .matchRuledOutYear),
        ], answers: .ladder, source: "Hotaru Open Archive")
    }

    @Test func theLadderAsksAndTheToastIsLeftToThePage() {
        let store = AppStore()
        store.apply(asked)
        store.apply(.toast(Notice(kind: .info, text: "", hint: nil, msg: .noticeMatchUncertain(source: "Hotaru Open Archive"))))
        store.apply(.titleMatch(id: id, current: nil))
        #expect(store.match.asks(id))
        #expect(store.match.asked[id]?.possible == 2)
        #expect(store.notices.current == nil, "the page's line says it")
        #expect(store.match.known.contains(id))
    }

    @Test func aPickSettlesWhenTheTitleIsMatchedAndSaysSo() {
        let store = AppStore()
        store.apply(asked)
        let pick = candidate("a", "Hoshi Tabi Diaries")
        store.pickMatch(id, providerId: "hotaru", source: "Hotaru Open Archive", candidate: pick)
        #expect(store.sentLog == ["fixMatch"])
        #expect(store.match.picking[id] == "a")
        store.apply(.titleMatch(id: id, current: CurrentMatch(
            providerId: "hotaru", source: "Hotaru Open Archive", title: "Hoshi Tabi Diaries", chosenByYou: true)))
        #expect(!store.match.asks(id))
        #expect(store.match.picking[id] == nil)
        #expect(store.match.settled?.id == id)
        let said = store.notices.current?.notice.text ?? ""
        #expect(said.contains("Hoshi Tabi Diaries") && said.contains("Hotaru Open Archive"), "\(said)")
    }

    @Test func aPickThatFailedStopsWaitingAndKeepsTheQuestion() {
        let store = AppStore()
        store.apply(asked)
        store.pickMatch(id, providerId: "hotaru", source: "Hotaru Open Archive", candidate: candidate("a", "Hoshi Tabi Diaries"))
        store.apply(.matchFixFailed(id: id, reason: .noticeMatchDidNotStick))
        store.apply(.titleMatch(id: id, current: nil))
        #expect(store.match.picking[id] == nil)
        #expect(store.match.pickFailed[id] == Msg.noticeMatchDidNotStick.text)
        #expect(store.match.asks(id))
        #expect(store.match.settled == nil)
    }

    @Test func aSearchShowsItsAnswerAndLeavesAStaleOneOut() {
        let store = AppStore()
        store.searchMatch(id, query: "  stardust wayfarer ", providerId: "hotaru")
        #expect(store.sentLog == ["manualSearch"])
        #expect(store.match.searches[id] == .searching(query: "stardust wayfarer"))
        // An answer to an older search is not shown.
        store.apply(.matchChoices(id: id, providerId: "hotaru", candidates: [], answers: .search(query: "hoshikuzu"), source: "Hotaru Open Archive"))
        #expect(store.match.searches[id] == .searching(query: "stardust wayfarer"))
        store.apply(.matchChoices(id: id, providerId: "hotaru", candidates: [], answers: .search(query: "stardust wayfarer"), source: "Hotaru Open Archive"))
        #expect(store.match.searches[id] == .empty(query: "stardust wayfarer", source: "Hotaru Open Archive"))
        store.searchMatch(id, query: "stardust", providerId: "hotaru")
        store.apply(.matchSearchFailed(id: id, query: "stardust", reason: .matchSearchTimedOut))
        #expect(store.match.searches[id] == .failed(query: "stardust", reason: Msg.matchSearchTimedOut.text))
        // The ladder's question is not touched by a search.
        #expect(!store.match.asks(id))
    }

    @Test func matchAutomaticallyForgetsTheChoiceAtOnce() {
        let store = AppStore()
        store.apply(.titleMatch(id: id, current: CurrentMatch(
            providerId: "hotaru", source: "Hotaru Open Archive", title: "Stardust Wayfarer", chosenByYou: true)))
        #expect(store.match.current[id]?.chosenByYou == true)
        store.matchAutomatically(id)
        #expect(store.sentLog == ["clearMatch"])
        #expect(store.match.current[id] == nil)
        #expect(store.notices.current != nil)
    }
}
