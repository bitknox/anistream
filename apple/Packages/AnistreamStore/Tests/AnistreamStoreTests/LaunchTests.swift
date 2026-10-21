// The store over a real engine: the testing one, on a scene, in a directory of the test's own.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct LaunchTests {
    /// A store on the testing engine started on `scene` under `root`, with every event applied.
    func started(_ scene: FfiScene, root: URL) async throws -> (AppStore, Recorder) {
        let store = AppStore()
        let recorder = Recorder()
        store.onApplied = { recorder.events += $0 }
        let launch = EngineLaunch(
            engine: .fixtures(scene),
            home: .override(root: root),
            platform: .macOs,
            secrets: .host
        )
        await store.start(launch, host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        return (store, recorder)
    }

    @Test func launchSaysTheSettingsFirst() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, recorder) = try await started(.fresh, root: root)
        defer { store.shutdown() }

        try await waitFor { store.settings.view != nil }
        guard case .settings = recorder.events.first else {
            Issue.record("launch's settings first: \(recorder.events.prefix(3))")
            return
        }
        // The files went where the override says, and nowhere else.
        #expect(FileManager.default.fileExists(atPath: root.appending(path: "data/anistream.db").path))
    }

    @Test func aFreshLaunchHasNothingToContinue() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.fresh, root: root)
        defer { store.shutdown() }

        store.loadContinue()
        #expect(store.continuing.list.state == .loading)
        try await waitFor { store.continuing.list.state != .loading }
        #expect(store.continuing.list.state == .empty)
    }

    @Test func theWatchingSceneFillsContinue() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.watching, root: root)
        defer { store.shutdown() }

        store.loadContinue()
        // The list, then the broadcast line that follows it for the airing titles.
        try await waitFor {
            !store.continuing.queue(store.titles).newThisWeek.isEmpty
        }
        let queue = store.continuing.queue(store.titles)
        #expect(queue.inProgress.map(\.entry.secondary) == [
            "Hoshikuzu no Tabibito", "Hagane no Niwashi", "Kumo no Ue no Toshokan",
        ])
        #expect(queue.inProgress.map(\.action) == [
            "Resume E7 · 10:06", "Resume E2 · 13:30", "Start E4",
        ])
        #expect(queue.newThisWeek.map(\.entry.secondary) == ["Natsukaze Relay"])
        #expect(queue.newThisWeek.first?.action.hasPrefix("Start E8 · out 19h") == true)
    }

    @Test func aTitleWatchedUpToItsNextBroadcastIsCaughtUpAndOffersNothingToPlay() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.watching, root: root)
        defer { store.shutdown() }

        // Natsukaze Relay's episode 8 is out and 9 airs in six days: watch 8, and it is caught up.
        store.send(.setWatched(id: 900_006, episodes: ["8"], watched: true))
        store.loadContinue(force: true)
        try await waitFor {
            store.continuing.queue(store.titles).all.first { $0.id == 900_006 }?.episode == 9
        }
        let item = try #require(store.continuing.queue(store.titles).all.first { $0.id == 900_006 })
        #expect(!item.playable)
        #expect(item.action.hasPrefix("Caught up · E9 "), "\(item.action)")
        #expect(item.buttonTitle.hasPrefix("Episode 9 airs "), "\(item.buttonTitle)")
        #expect(!item.isNew)
        // The others still play.
        let others = store.continuing.queue(store.titles).all.filter { $0.id != 900_006 }
        #expect(others.count == 3 && others.filter(\.playable).count == 3)
    }

    @Test func shutdownStopsTheStore() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.fresh, root: root)
        store.shutdown()
        try await waitFor { store.phase == .stopped }
        #expect(store.send(.loadContinue) == nil)
    }

    @Test func aConfigThatDoesNotLoadFailsTheStartWithItsReason() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let config = root.appending(path: "config", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: config, withIntermediateDirectories: true)
        try "[playback]\nbogus = 1\n".write(to: config.appending(path: "config.toml"), atomically: true, encoding: .utf8)

        let store = AppStore()
        await store.start(
            EngineLaunch(engine: .fixtures(.fresh), home: .override(root: root), platform: .macOs, secrets: .host),
            host: quietHost()
        )
        guard case .failed(let reason) = store.phase else {
            Issue.record("a config that does not load fails the start: \(store.phase)")
            return
        }
        #expect(reason.contains("bogus"), "\(reason)")
    }
}

/// Every event the store applied, in order.
@MainActor
final class Recorder {
    var events: [Event] = []
}

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct SettleTests {
    @Test func aStoreSettlesOnceContinueHasLoadedAndTheArtHasArrived() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        await store.start(
            EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host),
            host: quietHost()
        )
        defer { store.shutdown() }
        store.loadContinue()
        #expect(!store.isSettled)
        #expect(await store.settle(quiet: .milliseconds(300), timeout: .seconds(20)))
        #expect(store.continuing.queue(store.titles).all.count == 4)
        // The testing engine's fake serves the fixtures' art, so a cover arrives as an image, in
        // the cover's own colour beneath it until then.
        let entry = try #require(store.titles[900_001])
        #expect(entry.coverColor == "#e4a15d")
        let cover = try #require(entry.coverUrl)
        store.artwork.request(cover, maxPixelSize: 108)
        #expect(await store.settle(quiet: .milliseconds(300), timeout: .seconds(20)))
        #expect(store.artwork.image(cover)?.height == 108)
    }
}

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct EarlyTests {
    @Test func whatIsAskedForWhileTheEngineStartsIsSentOnceItHas() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        store.loadContinue()
        #expect(store.continuing.list.state == .loading)
        await store.start(
            EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host),
            host: quietHost()
        )
        defer { store.shutdown() }
        try await waitFor { store.continuing.list.state.ids.count == 4 }
    }
}
