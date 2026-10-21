// What the store asks the engine for, and what it does not ask twice: AniList answers about thirty
// requests a minute, and a screen shown again or a selection moving back and forth must not
// spend them.

@testable import AnistreamStore
import AnistreamKit
import Testing

@MainActor
struct AskingTests {
    @Test func aListAnsweredWithinAMinuteIsNotAskedForAgain() {
        let store = AppStore()
        store.loadContinue()
        store.loadContinue()
        #expect(store.sentLog == ["loadContinue", "loadContinue"], "unanswered, it may be asked again")
        store.apply(.content(view: .continue, content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.loadContinue()
        #expect(store.sentLog.count == 2, "answered just now")
        store.loadContinue(force: true)
        #expect(store.sentLog.count == 3, "a watch asks anyway")
    }

    @Test func aQuestionAboutATitleIsNotPutAgainWhileItsAnswerIsOnItsWay() {
        let store = AppStore()
        store.loadDetail(900_001)
        store.loadEpisodes(900_001)
        store.loadDetail(900_001)
        store.loadEpisodes(900_001)
        store.loadDetail(900_005)
        #expect(store.sentLog == ["loadDetail", "loadEpisodes", "loadDetail"])
    }

    @Test func stillsAndTheEpisodesShareOneQuestion() {
        let store = AppStore()
        store.apply(.detail(entry(900_005, "The Steel Gardener")))
        store.loadEpisodes(900_005)
        #expect(!store.loadStills(900_005), "the episodes are on their way")
        #expect(store.sentLog == ["loadEpisodes"])
    }
}
