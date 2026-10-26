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
        // The engine's own marks, which skipping goes by, take over from the chapters.
        try await until("the engine's marks", timeout: .seconds(10)) { model.timelines[first] != nil }
        #expect(model.segments.map(\.kind) == [.plain, .opening, .plain, .ending, .plain])

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

    /// Stats for nerds on a real mpv: the engine asks for the figures, mpv answers them, and the
    /// rows read the file as it plays, a file on the device with no Network row.
    @Test func statsForNerdsReadTheFileAsItPlays() async throws {
        let root = FileManager.default.temporaryDirectory
            .appending(path: "anistream-player-\(UUID().uuidString)", directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        var copied: [String] = []
        let model = PlayerModel(
            send: { [weak store] command in store?.send(command) },
            backend: {
                var configuration = MpvConfiguration()
                configuration.options["ao"] = "null"
                return MpvClient(configuration: configuration)
            },
            copy: { copied.append($0) })
        store.onPlayback = { [weak model] event in model?.apply(event) }
        await store.start(
            EngineLaunch(
                engine: .fixtures(.playing(media: try Self.episode())), home: .override(root: root),
                platform: .macOs, secrets: .host),
            host: AppHost(secrets: MemorySecrets(), open: { _ in false }))
        // Up before the file opens: the dashes first, then the file's rows.
        store.send(.play(id: Self.finished, episode: "7"))
        try await until("the player", timeout: .seconds(20)) { model.isPresented }
        model.toggleStats()
        try await until("the first frame", timeout: .seconds(20)) { model.hasPicture && model.starting == nil }
        // mpv knows the decoder a sample or so after the picture's size, and the row waits for it.
        try await until("the rows", timeout: .seconds(10)) {
            let rows = model.statsPanel?.rows ?? []
            return rows.first { $0.label == "Video" }?.value.contains("×") == true
                && rows.contains { $0.label == "Decoding" }
        }
        let rows = try #require(model.statsPanel?.rows)
        let labels = rows.map(\.label)
        // No Display row: a layer in no window is on no screen.
        #expect(labels.starts(with: ["Video", "Decoding"]), "\(labels)")
        #expect(!labels.contains("Display"), "\(labels)")
        // Frames the display drops are the machine's, busy with the other tests, not the file's.
        #expect(rows.allSatisfy { !$0.warn || $0.label == "Dropped frames" }, "a file on the device playing well: \(rows)")
        #expect(!labels.contains("Network"), "a file on the device")
        #expect(!labels.contains("HDR"), "an SDR picture")
        #expect(rows.allSatisfy { !$0.value.isEmpty && $0.value != "–" }, "\(rows)")
        #expect(model.statsWaitingSince == nil)

        model.statsAbout = { StatsAbout(appVersion: "2.4.0", system: "macOS 26.1", device: "Mac15,3") }
        model.copyStats()
        try await until("the lines copied", timeout: .seconds(5)) { !copied.isEmpty }
        let text = try #require(copied.first)
        #expect(text.hasPrefix("Stats for nerds\nVideo: "), "\(text)")
        #expect(text.hasSuffix("App: anistream 2.4.0\nSystem: macOS 26.1\nDevice: Mac15,3"), "\(text)")
        #expect(!text.contains(try Self.episode()), "no path")

        model.toggleStats()
        #expect(model.statsPanel == nil)
        model.viewerClosed()
        try await until("the player closed", timeout: .seconds(10)) { !model.isPresented }
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
        // mpv's cache of the stream, reported to the engine and said back for the seek line.
        try await until("the buffered ranges", timeout: .seconds(10)) { !model.buffered.isEmpty }
        #expect(model.buffered.allSatisfy { $0.end > $0.start })
        // Hovering the seek line: the hidden mpv reads the mended stream, from the engine's
        // thumbnails address through the mender, and shows a frame.
        model.hover(at: 9)
        try await until("the preview's frame", timeout: .seconds(15)) { model.preview.ready }
        #expect(model.thumbnails?.hasPrefix("http://127.0.0.1:") == true)
        model.hover(at: nil)
        let requests = server.requests
        let refused = requests.filter { !$0.allowed }.map(\.path)
        #expect(!requests.isEmpty)
        #expect(refused.isEmpty, "every fetch carries the referer")
        model.viewerClosed()
        store.shutdown()
        await model.finished()
    }
}
