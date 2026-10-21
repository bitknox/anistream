// The engine driven from Swift through the generated bindings: started on the test kit's fake
// AniList, sent the model's own commands, and read with `nextEvents()` for the model's own events
// until the job a command started has ended.

import AnistreamKit
import Foundation
import Testing

/// A host that opens nothing, shows nothing and keeps no secrets.
final class QuietHost: FfiHost {
    func openUrl(url: String) -> Bool { false }
    func notify(title: String, body: String) {}
    func secretGet(id: String) -> String? { nil }
    func secretSet(id: String, value: String) -> Bool { false }
    func secretDelete(id: String) -> Bool { true }
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

/// Every event until the one that ends `job`, that one included.
func events(until job: JobId, from engine: FfiEngine) async throws -> [Event] {
    var read: [Event] = []
    while true {
        let batch = await engine.nextEvents()
        try #require(!batch.isEmpty, "the engine stopped before the job ended: \(read)")
        read += batch
        if batch.contains(.jobDone(job)) {
            return read
        }
    }
}

@Suite(.timeLimit(.minutes(1)))
struct EngineTests {
    @Test func aSearchComesBackWithTheFixtureTitles() async throws {
        let (engine, root) = try startedEngine()
        defer { try? FileManager.default.removeItem(at: root) }
        defer { engine.shutdown() }

        let job = try #require(
            engine.send(command: .search(query: "hoshikuzu", filter: SearchFilters()))
        )
        let read = try await events(until: job, from: engine)

        let titles = read.lazy.compactMap { event -> [Entry]? in
            guard case .content(view: .search(query: "hoshikuzu", filter: _), content: .entries(let entries)) = event
            else { return nil }
            return entries
        }.first
        let found = try #require(titles, "the search's titles: \(read)")
        #expect(found.map(\.id) == [900_001, 900_004])
        #expect(found.map(\.title) == ["Stardust Wayfarer", "Stardust Wayfarer Season 2"])
        #expect(
            found.map(\.secondary) == ["Hoshikuzu no Tabibito", "Hoshikuzu no Tabibito 2nd Season"]
        )
        // What launch said came first, the settings ahead of everything.
        guard case .settings = read.first else {
            Issue.record("launch's settings first: \(read)")
            return
        }
    }

    @Test func continueIsEmptyOnANewDatabase() async throws {
        let (engine, root) = try startedEngine()
        defer { try? FileManager.default.removeItem(at: root) }
        defer { engine.shutdown() }

        let job = try #require(engine.send(command: .loadContinue))
        let read = try await events(until: job, from: engine)
        // Nothing has been watched yet, and the list says so rather than failing.
        #expect(read.contains(.content(view: .continue, content: .empty)), "\(read)")
    }

    @Test func aTitleComesBackWholeAfterTheSourcePinnedForIt() async throws {
        let (engine, root) = try startedEngine()
        defer { try? FileManager.default.removeItem(at: root) }
        defer { engine.shutdown() }

        let job = try #require(engine.send(command: .loadDetail(900_001)))
        let read = try await events(until: job, from: engine)

        let pinned = try #require(read.firstIndex(of: .providerPreference(nil)), "\(read)")
        let page = try #require(
            read.firstIndex { if case .detail = $0 { true } else { false } }, "\(read)"
        )
        // Said ahead of the page, so the page never shows another title's pin.
        #expect(pinned < page)
        guard case .detail(let entry) = read[page] else { return }
        #expect(entry.id == 900_001)
        #expect(entry.title == "Stardust Wayfarer")
        #expect(!entry.related.isEmpty, "the detail-only fields came with it")
    }

    @Test func aFixturePlaysToItsEndInAHostedPlayer() async throws {
        let (engine, root) = try startedEngine()
        defer { try? FileManager.default.removeItem(at: root) }
        defer { engine.shutdown() }

        // The finale, so the playback ends with the episode rather than rolling on.
        _ = engine.send(command: .play(id: 900_001, episode: "12"))
        var opened = false
        var loaded: PlaybackId? = nil
        var ending: Ending? = nil
        while ending == nil {
            let batch = await engine.nextEvents()
            try #require(!batch.isEmpty, "the engine stopped before the playback ended")
            for event in batch {
                switch event {
                case .playerOps(let playback, let ops):
                    for op in ops {
                        switch op {
                        case .open:
                            opened = true
                        case .load(let url, _) where loaded == nil:
                            #expect(url.hasPrefix("av://lavfi:"))
                            loaded = playback
                            // As libmpv reports it: the file, its length, a playhead, the end.
                            let report = { (report: PlayerReport) in
                                _ = engine.send(
                                    command: .playerReport(playback: playback, report: report)
                                )
                            }
                            report(.fileLoaded)
                            report(.property(name: "duration", json: "30"))
                            for second in 0..<30 {
                                report(.property(name: "time-pos", json: "\(second)"))
                            }
                            report(.ended(reason: .eof))
                        default:
                            break
                        }
                    }
                case .playbackEnded(_, let ended, _):
                    ending = ended
                default:
                    break
                }
            }
        }
        #expect(opened)
        #expect(loaded != nil)
        #expect(ending == .complete(next: nil))
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
