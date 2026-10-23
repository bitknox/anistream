// What the store asks the engine for, and what it does not ask twice: AniList answers about thirty
// requests a minute, and a screen shown again or a selection moving back and forth must not
// spend them.

@testable import AnistreamStore
import AnistreamKit
import Foundation
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

    @Test func titlesNamedAnotherWayAskAgainForWhatWasShown() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.apply(.content(view: .library(.watching), content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.loadDetail(900_001)
        store.apply(.titlesChanged(.romaji))
        #expect(store.sentLog == ["loadDetail", "loadContinue", "loadLibrary"], "Seasonal was never shown")
        store.loadDetail(900_001)
        #expect(store.sentLog.last == "loadDetail", "the page asks again")
    }

    @Test func stillsAndTheEpisodesShareOneQuestion() {
        let store = AppStore()
        store.apply(.detail(entry(900_005, "The Steel Gardener")))
        store.loadEpisodes(900_005)
        #expect(!store.loadStills(900_005), "the episodes are on their way")
        #expect(store.sentLog == ["loadEpisodes"])
    }

    @Test func aSourceReadyWhereNoneWasAsksAgainForEpisodesThatWereAniListsAlone() {
        let store = AppStore()
        let row = { (metadataOnly: Bool) in
            EpisodeRow(number: "1", title: nil, thumbnail: nil, description: nil, durationSecs: nil, watched: 0,
                       completed: false, kind: nil, skippable: false, airsAt: nil, delayedFrom: nil,
                       metadataOnly: metadataOnly)
        }
        store.loadEpisodes(900_001)
        store.loadEpisodes(900_005)
        store.apply(.episodes(id: 900_001, rows: [row(true)]))
        store.apply(.episodes(id: 900_005, rows: [row(false)]))
        #expect(store.titles.unlisted(900_001))
        #expect(!store.titles.unlisted(900_005), "a source listed it")
        store.apply(.providers([provider("fixture", usable: false)]))
        #expect(store.sentLog.count == 2, "none ready yet")
        // Ready now: asked again at once, though it was asked a moment ago.
        store.apply(.providers([provider("fixture")]))
        #expect(store.sentLog == ["loadEpisodes", "loadEpisodes", "loadEpisodes"])
        store.apply(.providers([provider("fixture")]))
        #expect(store.sentLog.count == 3, "ready already: nothing new")
    }

    @Test func theSystemsSignInSheetOpensWhenTheEngineAsksAndItsAnswerGoesBack() throws {
        let store = AppStore()
        store.apply(.authorize(tracker: "anilist", url: "https://anilist.co/api/v2/oauth/authorize?client_id=1",
                               callbackScheme: "anistream"))
        let request = try #require(store.accounts.signIn)
        #expect(request.tracker == "anilist" && request.callbackScheme == "anistream")
        store.finishSignIn(request, callback: URL(string: "anistream://auth/anilist#access_token=x")!)
        #expect(store.accounts.signIn == nil)
        #expect(store.sentLog == ["finishSignIn"])

        store.apply(.authorize(tracker: "mal", url: "https://myanimelist.net/v1/oauth2/authorize?code_challenge=c",
                               callbackScheme: "anistream"))
        store.cancelSignIn(try #require(store.accounts.signIn))
        #expect(store.accounts.signIn == nil)
        #expect(store.sentLog == ["finishSignIn", "cancelSignIn"])
    }
}
