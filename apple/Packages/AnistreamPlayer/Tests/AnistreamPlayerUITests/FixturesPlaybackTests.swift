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
        let endedBefore = Self.endings(events).count
        model.seek(to: 58)
        // The engine closes the player and then says how the playback ended, which can come in a
        // later batch of events than the close: the ending is waited for, not assumed.
        try await until("the ending", timeout: .seconds(20)) { Self.endings(events).count > endedBefore }
        try await until("the player closed") { !model.isPresented }
        let ended = try #require(Self.endings(events).last)
        #expect(ended.ending == .complete(next: nil), "\(Self.endings(events))")
        #expect(ended.watched, "watched")
        store.shutdown()
        await model.finished()
    }

    /// The `PlaybackEnded`s among `events`, in order.
    static func endings(_ events: [Event]) -> [(watched: Bool, ending: Ending, rollingOn: Bool)] {
        events.compactMap { event in
            if case .playbackEnded(let watched, let ending, let rollingOn) = event { (watched, ending, rollingOn) } else { nil }
        }
    }

    /// The episode as HLS, which apple/scripts/player-media.sh makes beside it.
    static func webEpisode() throws -> URL {
        let directory = URL(fileURLWithPath: try episode()).deletingLastPathComponent().appending(path: "episode-hls")
        try #require(FileManager.default.fileExists(atPath: directory.appending(path: "master.m3u8").path),
            "run apple/scripts/player-media.sh")
        return directory
    }

    /// A start from a config with the player full screen and natural upscaling, on a web
    /// stream behind a referer lock that the engine's mender fetches: the first frame lifts the
    /// starting screen, and the controls come up.
    @Test func aWebStreamInFullScreenGetsPastTheStartingScreen() async throws {
        let server = try StreamServer(directory: try Self.webEpisode(), referer: "https://lantern-archive.example/")
        let port = try await server.start()
        defer { server.stop() }
        let root = FileManager.default.temporaryDirectory
            .appending(path: "anistream-player-\(UUID().uuidString)", directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root.appending(path: "config"), withIntermediateDirectories: true)
        try Data("[playback]\nskip_opening = false\nfullscreen = true\nupscaling = \"natural\"\n".utf8)
            .write(to: root.appending(path: "config/config.toml"))
        let store = AppStore()
        let model = PlayerModel(
            send: { [weak store] command in store?.send(command) },
            backend: {
                var configuration = MpvConfiguration()
                configuration.options["ao"] = "null"
                return MpvClient(configuration: configuration)
            })
        store.onPlayback = { [weak model] event in model?.apply(event) }
        await store.start(
            EngineLaunch(
                engine: .fixtures(.playing(media: "http://127.0.0.1:\(port)/master.m3u8")),
                home: .override(root: root), platform: .macOs, secrets: .host),
            host: AppHost(secrets: MemorySecrets(), open: { _ in false }))
        #expect(store.phase == .running)

        store.send(.play(id: Self.finished, episode: "7"))
        try await until("the player", timeout: .seconds(20)) { model.isPresented }
        try await until("full screen, as the config has it", timeout: .seconds(20)) { model.fullScreen }
        try await until("the first frame", timeout: .seconds(30)) { model.hasPicture && model.starting == nil }
        #expect(model.startingLine == nil)
        #expect(model.duration.map { abs($0 - 60) < 1 } == true)
        model.poke()
        #expect(model.controlsVisible)
        let requests = server.requests
        let refused = requests.filter { !$0.allowed }.map(\.path)
        #expect(!requests.isEmpty)
        #expect(refused.isEmpty, "every fetch carries the referer")
        model.viewerClosed()
        store.shutdown()
        await model.finished()
    }
}
