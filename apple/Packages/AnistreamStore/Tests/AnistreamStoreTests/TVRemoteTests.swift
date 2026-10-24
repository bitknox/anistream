// Play on TV as the store holds it, on the testing engine and its two made-up TVs: the picker
// finds them, a code pairs one, the card follows what it plays, and Forget lets it go.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct TVRemoteTests {
    func started(root: URL) async throws -> AppStore {
        let store = AppStore()
        let launch = EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host)
        await store.start(launch, host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        try #require(store.tv.available)
        return store
    }

    @Test func aCodePairsATvAndTheCardFollowsWhatItPlays() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }
        let tv = store.tv

        store.playOnTV(900_001, episode: "2", from: 30)
        #expect(tv.sheet == .picker)
        tv.startBrowsing()
        try await waitFor { tv.rows.count == 2 }
        let living = try #require(tv.rows.first { $0.name == "Living room" })
        #expect(living.kind == .new)
        #expect(living.detail == "Not paired yet")

        tv.choose(living)
        try await waitFor { tv.pairing?.working == false }
        #expect(tv.pairing?.tvName == "Living room")
        #expect(tv.pairing?.needsCode == true)
        let fake = try #require(fakes(of: store).first)
        let code = try #require(fake.shownCode())

        tv.submit(code: "000000" == code ? "111111" : "000000")
        try await waitFor { tv.pairing?.problem != nil }
        #expect(tv.pairing?.problem?.hasPrefix("That code didn't match") == true)

        tv.submit(code: String(code.prefix(3)) + " " + String(code.dropFirst(3)))
        try await waitFor { tv.sheet == .remote && tv.card?.now != nil }
        #expect(tv.pairing == nil)
        #expect(tv.paired.map(\.name) == ["Living room"])
        #expect(tv.card?.tvName == "Living room")
        #expect(tv.card?.now?.episode == "2")
        #expect(tv.rows.first { $0.name == "Living room" }?.kind == .paired)

        tv.send(.togglePause)
        try await waitFor { tv.card?.now?.paused == true }
        tv.send(.volume(step: 5))
        try await waitFor { tv.card?.now?.volume == 45 }

        tv.forget(try #require(tv.paired.first).fingerprint)
        #expect(tv.connected == nil)
        #expect(tv.sheet == nil)
        try await waitFor { tv.paired.isEmpty && tv.rows.first { $0.name == "Living room" }?.kind == .new }
        tv.stopBrowsing()
    }

    @Test func aScannedLinkPairsWithoutACode() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }
        let bedroom = try #require(fakes(of: store).last)

        #expect(store.tv.open(URL(string: bedroom.pairLink(code: bedroom.openCode()))!))
        try await waitFor { store.tv.sheet == .remote }
        #expect(store.tv.paired.map(\.name) == ["Bedroom"])
        #expect(!store.tv.open(URL(string: "https://example.com")!))
    }

    /// The made-up TVs the store's browse finds.
    func fakes(of store: AppStore) -> [FfiFakeTv] {
        (store.tv.browser as? FixtureTVBrowser)?.tvs ?? []
    }
}
