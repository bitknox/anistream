// What the player's tests share: the media, a client on a layer with no window, and waits.
//
// The tests render for real, through MoltenVK on the Mac's GPU, into a CAMetalLayer that is in
// no window: MoltenVK sizes its swapchain from the layer's bounds, so the layer needs those and
// nothing else. Nothing appears on screen.

import AnistreamPlayer
import CoreGraphics
import Foundation
import Testing

/// Every suite runs under this one, one test at a time: each starts an mpv on the GPU, and several
/// at once make timing tests flaky.
@Suite(.serialized) struct PlayerTests {}

enum Media {
    /// apple/.mpv/media, which apple/scripts/player-media.sh fills.
    static let directory: URL = {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { url.deleteLastPathComponent() }
        return url.appending(path: ".mpv/media", directoryHint: .isDirectory)
    }()

    /// The repo's assets/shaders, where the engine vendors its shaders.
    static let shaders: URL = {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        return url.appending(path: "assets/shaders", directoryHint: .isDirectory)
    }()

    /// apple/.fonts/SubtitleFonts, which apple/scripts/fonts.sh fills.
    static func fonts() throws -> URL {
        let url = directory.deletingLastPathComponent().deletingLastPathComponent()
            .appending(path: ".fonts/SubtitleFonts", directoryHint: .isDirectory)
        try #require(
            FileManager.default.fileExists(atPath: url.appending(path: "NotoSansCJKsc-Regular.otf").path),
            "the subtitle font is missing: run apple/scripts/fonts.sh")
        return url
    }

    static func path(_ name: String) throws -> String {
        let url = directory.appending(path: name)
        try #require(
            FileManager.default.fileExists(atPath: url.path),
            "\(name) is missing: run apple/scripts/player-media.sh")
        return url.path
    }
}

/// A client started on a layer of `width`×`height` pixels that is in no window.
func startedClient(
    width: Int = 1280, height: Int = 720, configure: (inout MpvConfiguration) -> Void = { _ in }
) async throws -> (MpvClient, MpvLayer) {
    var configuration = MpvConfiguration()
    configuration.options["ao"] = "null"
    configure(&configuration)
    let layer = MpvLayer()
    layer.resize(to: CGSize(width: width / 2, height: height / 2), scale: 2)
    let client = MpvClient(configuration: configuration)
    try await client.start(on: layer)
    return (client, layer)
}

/// Loads `file` and waits for its first frame.
func play(_ client: MpvClient, _ file: String, options: [String: String] = [:]) async throws {
    let events = client.events()
    try await client.apply(.load(url: try Media.path(file), options: options))
    let started = await first(events) { event in
        if case .endFile(let end) = event, end.reason == "error" { return true }
        return event == .playbackRestart
    }
    try #require(started == .playbackRestart, "no first frame from \(file): \(String(describing: started))")
}

/// The first element of `stream` that `matches` within `timeout`, or nil. The stream is used up:
/// open one per wait, before what it waits for.
func first<T: Sendable>(
    _ stream: AsyncStream<T>, within timeout: Duration = .seconds(15),
    where matches: @escaping @Sendable (T) -> Bool
) async -> T? {
    await withTaskGroup(of: T?.self) { group in
        group.addTask {
            for await element in stream where matches(element) { return element }
            return nil
        }
        group.addTask {
            try? await Task.sleep(for: timeout)
            return nil
        }
        let result = await group.next() ?? nil
        group.cancelAll()
        return result
    }
}

/// `probe`'s first non-nil answer within `timeout`, asked every 100 ms.
func eventually<T: Sendable>(
    within timeout: Duration = .seconds(10), _ probe: @Sendable () async throws -> T?
) async rethrows -> T? {
    let deadline = ContinuousClock.now + timeout
    while ContinuousClock.now < deadline {
        if let value = try await probe() { return value }
        try? await Task.sleep(for: .milliseconds(100))
    }
    return try await probe()
}

/// The log lines a client prints from now on.
final class LogRecorder: Sendable {
    private let task: Task<[MpvLogLine], Never>

    init(_ client: MpvClient) {
        let events = client.events()
        task = Task {
            var lines: [MpvLogLine] = []
            for await event in events {
                if case .log(let line) = event { lines.append(line) }
            }
            return lines
        }
    }

    /// Everything printed until the client shut down.
    func lines() async -> [MpvLogLine] { await task.value }
}

/// The share of an image's pixels that are not all one colour, as a rough "something was drawn".
func variety(_ image: CGImage) -> Double {
    guard let data = image.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return 0 }
    let stride = image.bytesPerRow
    var colours = Set<UInt32>()
    for y in Swift.stride(from: 0, to: image.height, by: max(1, image.height / 40)) {
        for x in Swift.stride(from: 0, to: image.width, by: max(1, image.width / 40)) {
            let p = bytes + y * stride + x * 4
            colours.insert(UInt32(p[0]) << 16 | UInt32(p[1]) << 8 | UInt32(p[2]))
        }
    }
    return Double(colours.count) / 1600
}
