// The engine driven from Swift through the generated bindings: started on the test kit's fake
// AniList, sent a search, and read with `nextEvents()` until the search's titles come back.

import AnistreamKit
import Foundation
import Testing

/// A host that opens nothing and shows nothing.
final class QuietHost: FfiHost {
    func openUrl(url: String) -> Bool { false }
    func notify(title: String, body: String) {}
}

/// The engine started for a test in a new temporary directory, which the caller removes.
func startedEngine() throws -> (FfiEngine, URL) {
    let root = FileManager.default.temporaryDirectory
        .appendingPathComponent("anistream-kit-\(UUID().uuidString)")
    let dirs = FfiDirs(
        config: root.appendingPathComponent("config").path,
        data: root.appendingPathComponent("data").path,
        cache: root.appendingPathComponent("cache").path
    )
    return (try FfiEngine.startTesting(dirs: dirs, host: QuietHost()), root)
}

@Suite(.timeLimit(.minutes(1)))
struct EngineTests {
    @Test func aSearchComesBackWithTheFixtureTitles() async throws {
        let (engine, root) = try startedEngine()
        defer { try? FileManager.default.removeItem(at: root) }
        defer { engine.shutdown() }

        let job = try #require(engine.send(command: .search(query: "hoshikuzu")))
        var titles: [FfiTitle]?
        var ended = false
        while titles == nil || !ended {
            let events = await engine.nextEvents()
            try #require(!events.isEmpty, "the engine stopped before it answered")
            for event in events {
                switch event {
                case .content(view: .search(query: "hoshikuzu"), content: .titles(let found)):
                    titles = found
                case .jobDone(job: job):
                    ended = true
                default:
                    break
                }
            }
        }
        #expect(
            titles == [
                FfiTitle(id: 900_001, title: "Stardust Wayfarer", secondary: "Hoshikuzu no Tabibito"),
                FfiTitle(
                    id: 900_004,
                    title: "Stardust Wayfarer Season 2",
                    secondary: "Hoshikuzu no Tabibito 2nd Season"
                ),
            ]
        )
    }

    @Test func shutdownEndsTheStream() async throws {
        let (engine, root) = try startedEngine()
        defer { try? FileManager.default.removeItem(at: root) }

        engine.shutdown()
        // What launch said is still there to read, and then the stream ends.
        var batches = 0
        while !(await engine.nextEvents()).isEmpty {
            batches += 1
            try #require(batches < 100, "the stream ends once the engine has stopped")
        }
        #expect(engine.send(command: .loadContinue) == nil)
    }
}
