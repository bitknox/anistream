// A show's own settings on the testing engine: each change the sheet makes comes back to it.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct ShowSettingsTests {
    let id: AnilistId = 900_001

    func started(root: URL) async throws -> AppStore {
        let store = AppStore()
        let launch = EngineLaunch(
            engine: .fixtures(.watching),
            home: .override(root: root),
            platform: .macOs,
            secrets: .host
        )
        await store.start(launch, host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        try await waitFor { store.settings.view != nil }
        return store
    }

    @Test func aRowSetForTheShowIsWhatItsSheetThenSays() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }

        store.loadShowSettings(id)
        try await waitFor { store.showSettings.sheets[id] != nil }
        #expect(store.showSettings.row(.skipOpening, of: id)?.own == nil)

        store.setShowSetting(id, .skipOpening, to: .bool(true))
        try await waitFor { store.showSettings.row(.skipOpening, of: id)?.own == .bool(true) }
        #expect(store.showSettings.row(.skipOpening, of: id)?.value == "on")

        store.setShowSetting(id, .translation, to: .str("dub"))
        try await waitFor { store.showSettings.row(.translation, of: id)?.own == .str("dub") }

        store.setShowSetting(id, .skipOpening, to: nil)
        try await waitFor { store.showSettings.row(.skipOpening, of: id)?.own == nil }
    }

    @Test func theSourceASheetPinsIsWhatItThenSaysAndAResetLetsItGo() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }

        try await waitFor { !store.providers.rows.isEmpty }
        let provider = try #require(store.providers.rows.first?.id)
        store.loadShowSettings(id)
        try await waitFor { store.showSettings.sheets[id] != nil }
        #expect(store.showSource(id) == nil)

        store.setProviderPreference(id, to: provider)
        try await waitFor { store.showSource(id) == provider }

        // Another title's page, opened meanwhile, has a source of its own to say.
        store.loadDetail(900_002)
        try await waitFor { store.titles.detailed.contains(900_002) }
        #expect(store.showSource(id) == provider)

        store.resetShowSettings(id)
        try await waitFor { store.showSource(id) == nil }
    }
}
