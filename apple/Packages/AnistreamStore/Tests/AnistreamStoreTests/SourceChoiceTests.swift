// The source picker's state: the options the engine lists, the choice shown at once, and the one
// notice a pin it could not use is owed.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
struct SourceChoiceTests {
    private func option(_ id: String, covered: UInt32 = 6, pinned: Bool = false,
                        releases: [SourceCandidate] = []) -> SourceOption {
        SourceOption(providerId: id, name: id, covered: covered, of: 13, pinned: pinned, note: nil,
                     releases: releases, pinnedRelease: nil)
    }

    private func release(_ id: String) -> SourceCandidate {
        SourceCandidate(id: id, providerId: "torrents", title: "Wayfarer \(id)", quality: 1080, seeders: 4,
                        size: "1 GiB", dualAudio: false, dubbed: false, autoPick: false)
    }

    @Test func loadingLastsUntilTheOptionsComeAndKeepsTheOldOnesAfter() {
        let store = AppStore()
        store.loadSourceOptions(900_001)
        #expect(store.sentLog == ["loadSourceOptions"])
        #expect(store.sourceChoices.isLoading(900_001))
        store.apply(.sourceOptions(id: 900_001, options: [option("a"), option("b")]))
        #expect(!store.sourceChoices.isLoading(900_001))
        #expect(store.sourceChoices.options[900_001]?.map(\.providerId) == ["a", "b"])
        store.loadSourceOptions(900_001)
        #expect(!store.sourceChoices.isLoading(900_001), "the last listing stays up while the next comes")
    }

    @Test func aPickIsShownAtOnceAndSentToTheEngine() {
        let store = AppStore()
        store.apply(.sourceOptions(id: 900_001, options: [option("a"), option("b", pinned: true)]))
        store.chooseSource(900_001, provider: "a")
        #expect(store.sentLog == ["setProviderPreference"])
        #expect(store.sourceChoices.options[900_001]?.map(\.pinned) == [true, false])
        store.chooseSource(900_001, provider: nil)
        #expect(store.sourceChoices.options[900_001]?.allSatisfy { !$0.pinned } == true)
    }

    @Test func aReleaseIsPinnedWithItsSource() {
        let store = AppStore()
        let batch = release("batch")
        store.apply(.sourceOptions(id: 900_001, options: [option("a"), option("torrents", releases: [batch, release("one")])]))
        store.chooseRelease(900_001, provider: "torrents", release: batch)
        #expect(store.sentLog == ["pinRelease"])
        let torrents = store.sourceChoices.options[900_001]?.last
        #expect(torrents?.pinned == true)
        #expect(torrents?.pinnedRelease == "batch")
    }

    @Test func aPinTheEngineCouldNotUseIsSaidOnce() {
        let store = AppStore()
        store.apply(.pinUnavailable(id: 900_001, providerId: "a", name: "Hotaru"))
        let first = store.sourceChoices.pinNotice
        #expect(first?.name == "Hotaru")
        store.apply(.pinUnavailable(id: 900_001, providerId: "a", name: "Hotaru"))
        #expect(store.sourceChoices.pinNotice == first, "the same pin, the same visit")
        store.chooseSource(900_001, provider: "a")
        store.apply(.pinUnavailable(id: 900_001, providerId: "a", name: "Hotaru"))
        #expect(store.sourceChoices.pinNotice != first, "a new choice is said again")
    }
}
