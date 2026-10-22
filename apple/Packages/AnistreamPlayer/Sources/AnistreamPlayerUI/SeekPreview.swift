// Scrubbing: the frame under the finger from a second, hidden mpv, and the pace the finger sets.

import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import CoreGraphics
import Foundation
import Observation
import os

/// What the preview opens: the stream, or the engine's `Thumbnails` address when it differs,
/// with the options that reach the stream (headers and the like), never the playback's own.
public struct PreviewSource: Equatable, Sendable {
    public var url: String
    public var options: [String: String]

    /// The options of a `Load` a second mpv needs to read the same stream.
    static func reaching(_ options: [String: String]) -> [String: String] {
        options.filter { name, _ in
            ["header", "user-agent", "referrer", "cookie", "-lavf-o", "tls-"].contains { name.contains($0) }
        }
    }
}

/// A second mpv, hidden, that shows the 160 × 90 frame at the scrubbing finger: opened on the
/// first scrub of a file, kept for the next ones, and closed with the file. Keyframe seeks, the
/// newest only: one waiting at a time while mpv works on the last.
@MainActor @Observable
public final class SeekPreview {
    /// The frame's size, in points.
    public static let size = CGSize(width: 160, height: 90)

    /// The layer the preview draws into, while there is one.
    public private(set) var layer: MpvLayer?
    /// It has drawn a frame.
    public private(set) var ready = false

    @ObservationIgnored private let make: @MainActor () -> any PlayerBackend
    @ObservationIgnored private var backend: (any PlayerBackend)?
    @ObservationIgnored private var source: PreviewSource?
    @ObservationIgnored private var listener: Task<Void, Never>?
    @ObservationIgnored private var busy = false
    @ObservationIgnored private var ending: [Task<Void, Never>] = []
    @ObservationIgnored private var target: Double?

    init(make: @escaping @MainActor () -> any PlayerBackend) {
        self.make = make
    }

    /// The preview's mpv: video only, paused, no audio or subtitles, SDR.
    public static func mpv() -> MpvClient {
        var configuration = MpvConfiguration()
        configuration.hdr = false
        configuration.logLevel = "error"
        configuration.subtitleFallback = nil
        configuration.options = [
            "aid": "no", "sid": "no", "ao": "null", "pause": "yes", "keep-open": "yes",
            "hr-seek": "no", "osd-level": "0",
        ]
        return MpvClient(configuration: configuration)
    }

    /// Show the frame at `seconds` of `source`.
    func show(_ seconds: Double, of source: PreviewSource) {
        if backend == nil || self.source != source { open(source, at: seconds) }
        target = seconds
        pump()
    }

    private func open(_ source: PreviewSource, at seconds: Double) {
        close()
        let backend = make()
        let layer = MpvLayer()
        layer.resize(to: Self.size, scale: 2)
        self.backend = backend
        self.layer = layer
        self.source = source
        let events = backend.events()
        let opened = ContinuousClock.now
        AppLog.info("preview", "opening \(AppLog.address(source.url)) with \(source.options.keys.sorted().joined(separator: ", ")) at \(Int(seconds)) s")
        listener = Task { @MainActor [weak self] in
            for await event in events {
                guard let self, self.backend === backend else { return }
                switch event {
                case .playbackRestart where !self.ready:
                    self.ready = true
                    AppLog.info("preview", "first frame, \(AppLog.ms(since: opened)) ms after opening")
                case .endFile(let end) where end.reason == "error":
                    AppLog.warn("preview", "the preview could not open its file: \(end.error ?? "no reason given")")
                case .log(let line) where line.level == "error" || line.level == "fatal":
                    AppLog.warn("preview", "mpv \(line.prefix): \(line.text)")
                default:
                    break
                }
            }
        }
        busy = true
        let handed = HandedLayer(layer: layer)
        Task { @MainActor [weak self] in
            do {
                try await backend.begin(on: handed.layer)
                var options = source.options
                options["start"] = String(seconds)
                _ = try await backend.perform(.load(url: source.url, options: options))
            } catch {
                AppLog.warn("preview", "the preview did not open: \(error)")
            }
            guard let self, self.backend === backend else { return }
            self.busy = false
            self.pump()
        }
    }

    /// Seek to the newest target, when mpv is free.
    private func pump() {
        guard !busy, let backend, let seconds = target else { return }
        target = nil
        busy = true
        Task { @MainActor [weak self] in
            _ = try? await backend.perform(.seek(seconds: seconds, mode: .keyframe))
            guard let self, self.backend === backend else { return }
            self.busy = false
            self.pump()
        }
    }

    /// The layer changed size.
    func resized() {
        guard let backend else { return }
        Task { await backend.resized() }
    }

    /// Wait until every preview closed has shut down.
    func finished() async {
        let tasks = ending
        ending = []
        for task in tasks { await task.value }
    }

    /// Done with this file, or with previews: mpv shuts down.
    func close() {
        listener?.cancel()
        listener = nil
        if let backend { ending.append(Task.detached { await backend.end() }) }
        backend = nil
        layer = nil
        source = nil
        target = nil
        busy = false
        ready = false
    }
}

/// How fast scrubbing moves: the whole line is the whole file, at half speed with the finger
/// 48 pt above it, a quarter past 96 pt, or with ⌥ held on the Mac. A change of pace starts
/// from where the thumb is, so the playhead never jumps.
public struct ScrubPace: Equatable, Sendable {
    public private(set) var factor: Double = 1
    private var anchorTime: Double = 0
    private var anchorX: CGFloat = 0

    public init() {}

    /// The pace for a finger `lifted` points above the line, or ⌥ held.
    public static func factor(lifted: CGFloat, option: Bool) -> Double {
        if option || lifted > 96 { return 0.25 }
        if lifted > 48 { return 0.5 }
        return 1
    }

    /// "Half-speed scrubbing", while it is.
    public static func label(_ factor: Double) -> String? {
        switch factor {
        case 0.5: "Half-speed scrubbing"
        case 0.25: "Quarter-speed scrubbing"
        default: nil
        }
    }

    /// Where the drag starts: the time under the finger.
    public mutating func start(at x: CGFloat, width: CGFloat, duration: Double) -> Double {
        factor = 1
        anchorX = x
        anchorTime = min(duration, max(0, Double(x / max(width, 1)) * duration))
        return anchorTime
    }

    /// The time for the finger at `x`, `lifted` above the line, at `current` now.
    public mutating func time(
        at x: CGFloat, lifted: CGFloat, option: Bool, width: CGFloat, duration: Double, current: Double
    ) -> Double {
        let pace = Self.factor(lifted: lifted, option: option)
        if pace != factor {
            factor = pace
            anchorX = x
            anchorTime = current
        }
        let moved = Double((x - anchorX) / max(width, 1)) * duration * factor
        return min(duration, max(0, anchorTime + moved))
    }
}
