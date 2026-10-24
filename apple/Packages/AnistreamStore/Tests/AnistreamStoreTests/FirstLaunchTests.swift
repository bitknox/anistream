// First launch: a new install sees it; one already in use, after an upgrade, does not.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
struct FirstLaunchTests {
    private func anilist(connected: Bool) -> SyncState {
        SyncState(
            tracker: "anilist", connected: connected, user: connected ? "Mika" : nil, storage: nil,
            storageDegraded: false, outbox: 0, deferred: 0, needsReauth: false, last: nil, pullError: nil
        )
    }

    @Test func aNewInstallSeesIt() async {
        let store = AppStore()
        store.apply(.providers([provider("torrent", "torrent", usable: false)]))
        store.apply(.content(view: .continue, content: .entries([])))
        store.apply(.sync(anilist(connected: false)))
        #expect(!store.hasBeenUsed)
        #expect(await store.firstLaunchDue(seen: false, limit: .milliseconds(200)))
    }

    @Test func onceSeenItDoesNotComeBack() async {
        #expect(await !AppStore().firstLaunchDue(seen: true))
    }

    @Test func aSourceASignInOrSomethingWatchedEachSkipIt() async {
        let plugin = AppStore()
        plugin.apply(.providers([provider("alpha")]))
        #expect(plugin.hasBeenUsed)
        #expect(await !plugin.firstLaunchDue(seen: false, limit: .milliseconds(200)))

        let joined = AppStore()
        joined.apply(.stores([storeRow("mika", [])]))
        #expect(joined.hasBeenUsed)

        let signedIn = AppStore()
        signedIn.apply(.sync(anilist(connected: true)))
        #expect(signedIn.hasBeenUsed)

        let watched = AppStore()
        watched.apply(.content(view: .continue, content: .entries([entry(900_001, "Stardust Wayfarer")])))
        #expect(watched.hasBeenUsed)
    }

    @Test func aTorrentSourceThatCanPlayCountsAsASource() {
        let store = AppStore()
        store.apply(.providers([provider("torrent", "torrent", usable: true)]))
        #expect(store.hasBeenUsed)
    }
}
