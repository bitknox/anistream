// The Fixtures app's playback, end to end: the testing engine on the Playing scene, the player
// model on a real mpv drawing into a layer in no window, and the generated episode. It starts,
// offers the opening's skip, shows Up Next at the ending, rolls on to the next episode in the
// same player when the countdown runs out, and ends with the engine's PlaybackEnded.

import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import Foundation
import Testing

@testable import AnistreamPlayerUI

@MainActor
@Suite(.serialized, .timeLimit(.minutes(2)))
struct FixturesPlaybackTests {
    /// apple/.mpv/media/episode.mkv, which apple/scripts/player-media.sh makes.
    static func episode() throws -> String {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        let path = url.appending(path: ".mpv/media/episode.mkv").path
        try #require(FileManager.default.fileExists(atPath: path), "run apple/scripts/player-media.sh")
        return path
    }

    /// Hoshikuzu no Tabibito, the test kit's finished title; the scene has watched 1 to 6.
    static let finished: AnilistId = 900_001

    @Test func anEpisodePlaysThroughUpNextIntoTheNextAndEnds() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "anistream-player-\(UUID().uuidString)", directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        let model = PlayerModel(
            send: { [weak store] command in store?.send(command) },
            backend: {
                var configuration = MpvConfiguration()
                configuration.options["ao"] = "null"
                return MpvClient(configuration: configuration)
            })
        var events: [Event] = []
        store.onPlayback = { [weak model] event in model?.apply(event) }
        store.onApplied = { events += $0 }
        await store.start(
            EngineLaunch(
                engine: .fixtures(.playing(media: try Self.episode())), home: .override(root: root),
                platform: .macOs, secrets: .host),
            host: AppHost(secrets: MemorySecrets(), open: { _ in false }))
        #expect(store.phase == .running)

        // Start: the starting screen, then the first frame over it.
        store.send(.play(id: Self.finished, episode: "7"))
        try await until("the player", timeout: .seconds(20)) { model.isPresented }
        #expect(model.staged?.episode == "7")
        try await until("the first frame", timeout: .seconds(20)) { model.hasPicture && model.starting == nil }
        let first = try #require(model.playback)
        #expect(model.episode?.number == "7")
        #expect(model.duration.map { abs($0 - 60) < 0.5 } == true)
        #expect(model.chapters.map(\.title) == ["Prologue", "Opening", "Part A", "Ending", "Preview"])
        #expect(model.segments.map(\.kind) == [.plain, .opening, .plain, .ending, .plain])
        #expect(model.audioTracks.count == 2 && model.subtitleTracks.count == 2)

        // Scrubbing: the hidden mpv opens the stream and shows the frame under the finger.
        model.scrub(to: 9, pace: 0.5)
        try await until("the preview's frame", timeout: .seconds(10)) { model.preview.ready }
        #expect(model.chapter(at: 9) == "Opening")
        model.endScrub()

        // The opening's skip, offered and not taken by itself: the scene offers skips.
        model.seek(to: 6)
        try await until("the skip offered", timeout: .seconds(10)) { model.skip?.kind == .opening }
        #expect(model.corner == .skip)

        // Up Next from the ending, counting down; at zero it plays the next episode.
        model.seek(to: 43)
        try await until("Up Next", timeout: .seconds(10)) { model.upNextShown }
        #expect(model.upNext?.next.number == "8")
        #expect(model.upNext?.endingAt.map { abs($0 - 42) < 0.5 } == true)
        let countdown = try #require(model.countdown)
        #expect((8...10).contains(countdown.seconds))
        model.seek(to: 51)
        try await until("the next episode", timeout: .seconds(20)) {
            (model.playback ?? 0) > first && model.hasPicture && model.episode?.number == "8"
        }
        #expect(model.isPresented, "the same player")
        #expect(model.stillWatching == false)

        // The next one plays to its end with nothing after it: the engine ends the playback and
        // closes the player.
        model.seek(to: 43)
        try await until("its Up Next", timeout: .seconds(10)) { model.upNextShown }
        model.cancelNext()
        model.seek(to: 58)
        try await until("the end", timeout: .seconds(20)) { !model.isPresented }
        let ended = events.compactMap { event -> (Bool, Ending, Bool)? in
            if case .playbackEnded(let watched, let ending, let rollingOn) = event { (watched, ending, rollingOn) } else { nil }
        }
        #expect(ended.last.map { $0.1 } == .complete(next: nil), "\(ended)")
        #expect(ended.last?.0 == true, "watched")
        store.shutdown()
    }
}
