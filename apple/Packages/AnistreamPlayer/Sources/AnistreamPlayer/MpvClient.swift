// One mpv, driven through its client API on a queue of its own.

import Foundation
import Mpv
import Synchronization

public struct MpvConfiguration: Sendable {
    /// `videotoolbox` maps decoded frames straight into the renderer; `videotoolbox-copy` and
    /// `no` are there for testing.
    public var hwdec = "videotoolbox"
    /// `target-colorspace-hint`: libplacebo picks the swapchain's colour space from the video, so
    /// HDR10 gets a PQ layer and EDR. mpv cannot change it after start without stalls.
    public var hdr = true
    /// The mpv log level the events carry: no, fatal, error, warn, info, v, debug.
    public var logLevel = "warn"
    /// Where libplacebo keeps compiled shaders, if anywhere.
    public var shaderCache: URL?
    /// A font for the subtitle glyphs no system font can draw. Core Text's fallback for Han,
    /// PingFang, is in a format FreeType cannot open; on the Mac apple/patches/libass falls back
    /// to Hiragino Sans and Hiragino Sans GB instead, but iOS has no font with simplified-only
    /// characters (简) that FreeType opens. Naming one here (a static font: libass matches the
    /// legacy family name, which for a variable font is its default instance's) makes it the
    /// first fallback, before Core Text's. On iOS it is the font the app bundles, on the Mac none.
    public var subtitleFallback: SubtitleFont?
    /// Any other mpv options, set before start, after these.
    public var options: [String: String] = [:]

    public init() {
        #if os(iOS)
        subtitleFallback = .bundled
        #endif
    }
}

public struct SubtitleFont: Sendable, Equatable {
    /// The folder the font is in, which mpv loads every font from.
    public var directory: URL
    /// Its family name, as the font's name table gives it (nameID 1).
    public var family: String

    public init(directory: URL, family: String) {
        self.directory = directory
        self.family = family
    }

    /// The family of the font apple/scripts/fonts.sh fetches: Noto Sans CJK SC Regular.
    public static let bundledFamily = "Noto Sans CJK SC"

    /// The CJK font the iOS app bundles, in its SubtitleFonts folder (apple/scripts/fonts.sh and
    /// project.yml), or nil where there is none, as in the Mac app.
    public static var bundled: SubtitleFont? {
        Bundle.main.url(forResource: "SubtitleFonts", withExtension: nil).map {
            SubtitleFont(directory: $0, family: bundledFamily)
        }
    }
}

public enum MpvError: Error, Equatable, CustomStringConvertible {
    case notStarted
    case alreadyStarted
    /// The client is shutting down or has, and takes no more calls.
    case terminated
    /// mpv said no: its error code and what was asked.
    case mpv(code: Int32, call: String)
    case unsupported(String)

    public var description: String {
        switch self {
        case .notStarted: "mpv has not started"
        case .alreadyStarted: "mpv has already started"
        case .terminated: "mpv has shut down"
        case .mpv(let code, let call): "\(call): \(String(cString: mpv_error_string(code)))"
        case .unsupported(let what): what
        }
    }
}

/// Owns one mpv handle.
///
/// - Every call runs on the client's own serial queue (its executor), so callers await them and
///   nothing waits on mpv from the main thread. That matters: a screenshot or a reconfigure
///   waits on mpv's render thread, which itself hops to the main thread when MoltenVK switches
///   EDR on, so a main thread blocked on mpv would deadlock.
/// - One thread waits on `mpv_wait_event` and hands each event to every stream from `events()`
///   and, as the engine's reports, `reports()`.
/// - From `shutdown()` on, every call throws `MpvError.terminated`; the handle is freed only once
///   the event thread has seen mpv go.
public actor MpvClient {
    public nonisolated let configuration: MpvConfiguration
    private let queue: DispatchSerialQueue
    public nonisolated var unownedExecutor: UnownedSerialExecutor { queue.asUnownedSerialExecutor() }

    private var handle: OpaquePointer?
    private var layer: MpvLayer?
    private var terminating = false
    private let eventThreadDone = DispatchSemaphore(value: 0)
    private nonisolated let eventHub = Hub<MpvEvent>()
    private nonisolated let reportHub = Hub<MpvReport>()

    public init(configuration: MpvConfiguration = .init()) {
        self.configuration = configuration
        self.queue = DispatchSerialQueue(label: "tv.anistream.mpv", qos: .userInitiated) // i18n-skip
    }

    // MARK: Lifetime

    /// Creates mpv rendering into `layer`, which the client keeps alive while mpv lives: `wid`
    /// is its address. `vo=gpu-next` on Vulkan through MoltenVK, VideoToolbox decoding.
    public func start(on layer: MpvLayer) throws {
        guard !terminating else { throw MpvError.terminated }
        guard handle == nil else { throw MpvError.alreadyStarted }
        guard let mpv = mpv_create() else { throw MpvError.mpv(code: -1, call: "mpv_create") }
        do {
            var wid = layer.wid
            try check(mpv_set_option(mpv, "wid", MPV_FORMAT_INT64, &wid), "option wid")
            var options: [(String, String)] = [
                ("vo", "gpu-next"),
                ("gpu-api", "vulkan"),
                ("gpu-context", "moltenvk"),
                ("hwdec", configuration.hwdec),
                ("target-colorspace-hint", configuration.hdr ? "yes" : "no"),
                ("idle", "yes"),
                ("input-default-bindings", "no"),
                ("input-vo-keyboard", "no"),
                ("embeddedfonts", "yes"),
            ]
            if let fallback = configuration.subtitleFallback {
                options.append(("sub-fonts-dir", fallback.directory.path))
                options.append(("sub-font", fallback.family))
            }
            if let cache = configuration.shaderCache {
                options.append(("gpu-shader-cache-dir", cache.path))
                options.append(("icc-cache-dir", cache.path))
            }
            options += configuration.options.sorted { $0.key < $1.key }.map { ($0.key, $0.value) }
            for (name, value) in options {
                try check(mpv_set_option_string(mpv, name, value), "option \(name)=\(value)")
            }
            // Only where mpv was built with Lua: the macOS slice, not iOS.
            _ = mpv_set_option_string(mpv, "ytdl", "no")
            mpv_request_log_messages(mpv, configuration.logLevel)
            try check(mpv_initialize(mpv), "mpv_initialize")
        } catch {
            mpv_terminate_destroy(mpv)
            throw error
        }
        handle = mpv
        self.layer = layer
        startEventThread(mpv)
    }

    /// Quits mpv, waits for the event thread to see it go, frees the handle and ends every
    /// stream. Calls from here on throw `MpvError.terminated`. Safe to call twice.
    public func shutdown() {
        guard !terminating else { return }
        terminating = true
        guard let mpv = handle else {
            finishStreams()
            return
        }
        _ = withCArgs(["quit"]) { mpv_command(mpv, $0) }
        // On the client's own queue, not the main thread or the cooperative pool.
        _ = eventThreadDone.wait(timeout: .now() + 10)
        mpv_terminate_destroy(mpv)
        handle = nil
        layer = nil
        finishStreams()
    }

    /// Not shut down: shut down now, on the client's queue (an isolated deinit runs there).
    isolated deinit {
        shutdown()
    }

    private func startEventThread(_ mpv: OpaquePointer) {
        nonisolated(unsafe) let mpv = mpv
        let events = eventHub
        let reports = reportHub
        let done = eventThreadDone
        let thread = Thread {
            while true {
                guard let pointer = mpv_wait_event(mpv, -1) else { continue }
                let raw = pointer.pointee
                if raw.event_id == MPV_EVENT_NONE { continue }
                if let event = MpvEvent(raw) {
                    events.yield(event)
                    if let report = MpvReport(event) { reports.yield(report) }
                }
                if raw.event_id == MPV_EVENT_SHUTDOWN { break }
            }
            done.signal()
        }
        thread.name = "mpv events"
        thread.qualityOfService = .userInitiated
        thread.start()
    }

    private func finishStreams() {
        eventHub.finish()
        reportHub.finish()
    }

    // MARK: Streams

    /// Every event from now on, until mpv shuts down. Each call is a stream of its own.
    public nonisolated func events() -> AsyncStream<MpvEvent> { eventHub.stream() }

    /// The engine's reports from now on: observed properties as JSON, mpv's events by name, the
    /// end of each file, and the display when `updateDisplay(_:)` gives it.
    public nonisolated func reports() -> AsyncStream<MpvReport> { reportHub.stream() }

    // MARK: Calls

    private func live() throws -> OpaquePointer {
        guard !terminating else { throw MpvError.terminated }
        guard let handle else { throw MpvError.notStarted }
        return handle
    }

    private func check(_ status: Int32, _ call: String) throws {
        if status < 0 { throw MpvError.mpv(code: status, call: call) }
    }

    /// A command as mpv's input commands spell it: `["seek", "10", "relative"]`.
    public func command(_ arguments: [String]) throws {
        let mpv = try live()
        try check(withCArgs(arguments) { mpv_command(mpv, $0) }, arguments.joined(separator: " "))
    }

    /// A command with named arguments, which takes nodes (a map of options, a list) and returns
    /// one: `command("loadfile", ["url": "…", "options": ["start": "10"]])`.
    @discardableResult
    public func command(_ name: String, _ arguments: [String: MpvValue]) throws -> MpvValue {
        let mpv = try live()
        let arena = NodeArena()
        var named = arguments
        named["name"] = .string(name)
        var node = arena.node(.map(named))
        var result = mpv_node()
        try check(mpv_command_node(mpv, &node, &result), name)
        defer { mpv_free_node_contents(&result) }
        return MpvValue(result)
    }

    public func get(_ property: String) throws -> MpvValue {
        let mpv = try live()
        var node = mpv_node()
        try check(mpv_get_property(mpv, property, MPV_FORMAT_NODE, &node), "get \(property)")
        defer { mpv_free_node_contents(&node) }
        return MpvValue(node)
    }

    /// Sets a property as a node: a list property takes an array with no separator to escape.
    public func set(_ property: String, _ value: MpvValue) throws {
        let mpv = try live()
        let arena = NodeArena()
        var node = arena.node(value)
        try check(mpv_set_property(mpv, property, MPV_FORMAT_NODE, &node), "set \(property)")
    }

    /// Reports every change of these properties, as events and as reports.
    public func observe(_ properties: [String]) throws {
        let mpv = try live()
        for property in properties {
            try check(mpv_observe_property(mpv, 0, property, MPV_FORMAT_NODE), "observe \(property)")
        }
    }

    // MARK: The engine's ops

    public func apply(_ op: MpvOp) throws {
        switch op {
        case .load(let url, let options): try load(url, options: options)
        case .addSubtitle(let url, let title, let language, let select):
            try addSubtitle(url, title: title, language: language, select: select)
        case .set(let property, let value): try set(property, value)
        case .seek(let seconds, let mode): try seek(seconds, mode: mode)
        case .shaders(let paths): try setShaders(paths)
        case .audioFilter(let graph): try setAudioFilter(graph)
        case .stop: try command(["stop"])
        }
    }

    /// Loads `url` in place of the playlist, and answers with the playlist entry mpv made for
    /// it: the one its `start-file` names.
    @discardableResult
    public func load(_ url: String, options: [String: String] = [:]) throws -> Int64? {
        var arguments: [String: MpvValue] = ["url": .string(url), "flags": "replace"]
        if !options.isEmpty { arguments["options"] = .map(options.mapValues(MpvValue.string)) }
        guard case .map(let answer) = try command("loadfile", arguments) else { return nil }
        return switch answer["playlist_entry_id"] {
        case .int(let entry): entry
        default: nil
        }
    }

    public func addSubtitle(
        _ url: String, title: String? = nil, language: String? = nil, select: Bool = true
    ) throws {
        var arguments: [String: MpvValue] = ["url": .string(url), "flags": select ? "select" : "auto"]
        if let title { arguments["title"] = .string(title) }
        if let language { arguments["lang"] = .string(language) }
        try command("sub-add", arguments)
    }

    public func seek(_ seconds: Double, mode: SeekMode = .absolute) throws {
        try command(["seek", String(seconds), mode.rawValue])
    }

    /// `glsl-shaders` as one array, so no path is split on a list separator.
    public func setShaders(_ paths: [String]) throws {
        try set("glsl-shaders", .array(paths.map(MpvValue.string)))
    }

    public func setAudioFilter(_ graph: String?) throws {
        try set("af", .string(graph ?? ""))
    }

    // MARK: The surface

    /// Tells mpv about the screen: the refresh rate as `display-fps-override`, since mpv cannot
    /// read it here, and the whole of it as a `.display` report for the engine.
    public func updateDisplay(_ display: DisplayInfo) throws {
        try set("display-fps-override", .double(display.refreshRate))
        reportHub.yield(.display(display))
    }

    /// After the layer changed size. mpv's render thread checks the size each time it wakes, and
    /// while paused it sleeps; reading `current-window-scale` asks the render thread something,
    /// which wakes it, with nothing drawn or changed.
    public func surfaceResized() {
        guard let mpv = handle, !terminating else { return }
        var node = mpv_node()
        if mpv_get_property(mpv, "current-window-scale", MPV_FORMAT_NODE, &node) >= 0 {
            mpv_free_node_contents(&node)
        }
    }

    // For Screenshot.swift.
    func liveHandle() throws -> OpaquePointer { try live() }
}

extension MpvReport {
    init?(_ event: MpvEvent) {
        switch event {
        case .property(let name, let value): self = .property(name: name, json: value.json)
        case .endFile(let end): self = .ended(end)
        case .log, .shutdown: return nil
        default: self = .event(event.name)
        }
    }
}

/// Runs `body` with `arguments` as the NULL-terminated C array mpv's commands take.
func withCArgs<T>(_ arguments: [String], _ body: (UnsafeMutablePointer<UnsafePointer<CChar>?>) -> T) -> T {
    var pointers: [UnsafePointer<CChar>?] = arguments.map { UnsafePointer(strdup($0)) }
    pointers.append(nil)
    defer { for pointer in pointers { free(UnsafeMutablePointer(mutating: pointer)) } }
    return pointers.withUnsafeMutableBufferPointer { body($0.baseAddress!) }
}

/// Hands each element to every open stream; streams opened later see only what comes after.
final class Hub<Element: Sendable>: Sendable {
    private typealias Listeners = [UUID: AsyncStream<Element>.Continuation]
    private let state = Mutex<(listeners: Listeners, finished: Bool)>(([:], false))

    func stream() -> AsyncStream<Element> {
        let (stream, continuation) = AsyncStream.makeStream(of: Element.self, bufferingPolicy: .unbounded)
        let id = UUID()
        let added = state.withLock { state -> Bool in
            guard !state.finished else { return false }
            state.listeners[id] = continuation
            return true
        }
        guard added else {
            continuation.finish()
            return stream
        }
        continuation.onTermination = { [weak self] _ in
            self?.state.withLock { _ = $0.listeners.removeValue(forKey: id) }
        }
        return stream
    }

    func yield(_ element: Element) {
        for continuation in state.withLock({ Array($0.listeners.values) }) {
            continuation.yield(element)
        }
    }

    func finish() {
        let listeners = state.withLock { state in
            state.finished = true
            defer { state.listeners.removeAll() }
            return Array(state.listeners.values)
        }
        for continuation in listeners { continuation.finish() }
    }
}
