import Foundation

#if canImport(Mpv)
import Mpv
#else
import Libmpv
#endif

/// A value read from mpv as an `mpv_node`, copied out so it outlives the event it came in.
public enum MPVValue: Sendable, Equatable {
    case none
    case flag(Bool)
    case int(Int64)
    case double(Double)
    case string(String)
    case array([MPVValue])
    case map([String: MPVValue])
    case bytes(Data)

    init(_ node: mpv_node) {
        switch node.format {
        case MPV_FORMAT_STRING, MPV_FORMAT_OSD_STRING:
            self = node.u.string.map { .string(String(cString: $0)) } ?? .none
        case MPV_FORMAT_FLAG: self = .flag(node.u.flag != 0)
        case MPV_FORMAT_INT64: self = .int(node.u.int64)
        case MPV_FORMAT_DOUBLE: self = .double(node.u.double_)
        case MPV_FORMAT_NODE_ARRAY:
            guard let list = node.u.list?.pointee else { self = .array([]); return }
            self = .array((0..<Int(list.num)).map { MPVValue(list.values[$0]) })
        case MPV_FORMAT_NODE_MAP:
            guard let list = node.u.list?.pointee else { self = .map([:]); return }
            var map: [String: MPVValue] = [:]
            for i in 0..<Int(list.num) {
                map[String(cString: list.keys[i]!)] = MPVValue(list.values[i])
            }
            self = .map(map)
        case MPV_FORMAT_BYTE_ARRAY:
            guard let ba = node.u.ba?.pointee, let data = ba.data else { self = .bytes(Data()); return }
            self = .bytes(Data(bytes: data, count: ba.size))
        default: self = .none
        }
    }

    public var string: String? { if case .string(let s) = self { s } else { nil } }
    public var bool: Bool? { if case .flag(let b) = self { b } else { nil } }
    public var double: Double? {
        switch self {
        case .double(let d): d
        case .int(let i): Double(i)
        default: nil
        }
    }
    public var int: Int64? {
        switch self {
        case .int(let i): i
        case .double(let d): Int64(d)
        default: nil
        }
    }
    public var array: [MPVValue] { if case .array(let a) = self { a } else { [] } }
    public subscript(key: String) -> MPVValue {
        if case .map(let m) = self { m[key] ?? .none } else { .none }
    }

    /// The value as Foundation types, for JSON.
    public var json: Any {
        switch self {
        case .none: NSNull()
        case .flag(let b): b
        case .int(let i): i
        case .double(let d): d.isFinite ? d : NSNull()
        case .string(let s): s
        case .array(let a): a.map(\.json)
        case .map(let m): m.mapValues(\.json)
        case .bytes(let d): "<\(d.count) bytes>"
        }
    }
}

public struct MPVLogLine: Sendable {
    public let prefix: String
    public let level: String
    public let text: String
}

public enum MPVEvent: Sendable {
    case property(name: String, value: MPVValue)
    case log(MPVLogLine)
    case fileLoaded
    case endFile(reason: UInt32, error: Int32)
    case playbackRestart
    case videoReconfig
    case seek
    case idle
    case shutdown
    case other(String)
}

public struct MPVError: Error, CustomStringConvertible {
    public let code: Int32
    public let what: String
    public var description: String { "\(what): \(String(cString: mpv_error_string(code)))" }
}

/// One mpv instance driven through the client API: options before `initialize()`, then
/// commands, properties and one thread that waits on `mpv_wait_event`.
///
/// Every call here may block on mpv's core, and some (a screenshot, a VO reconfig) wait on
/// the render thread, which itself hops to the main thread to switch EDR on. So nothing here
/// is called synchronously from the main thread: the UI uses the `async` variants.
public final class MPVClient: @unchecked Sendable {
    private let handle: OpaquePointer
    private let lock = NSLock()
    // Calls in flight, and whether the handle may still be used: `shutdown()` waits for the
    // calls to drain before it frees the handle, and refuses new ones. Without it, a late
    // property change (video-params going away as the file closes) set shaders on a freed
    // handle and crashed at exit.
    private let gate = NSCondition()
    private var alive = true
    private var inFlight = 0
    private var handlers: [UUID: @Sendable (MPVEvent) -> Void] = [:]
    private var thread: Thread?
    private let finished = DispatchSemaphore(value: 0)

    public init() throws {
        guard let handle = mpv_create() else { throw MPVError(code: -1, what: "mpv_create") }
        self.handle = handle
    }

    // MARK: Setup

    public func setOption(_ name: String, _ value: String) throws {
        try check(using(-1) { mpv_set_option_string($0, name, value) }, "option \(name)=\(value)")
    }

    public func setOption(_ name: String, int64 value: Int64) throws {
        var value = value
        try check(using(-1) { mpv_set_option($0, name, MPV_FORMAT_INT64, &value) }, "option \(name)")
    }

    public func requestLog(level: String) {
        _ = using(-1) { mpv_request_log_messages($0, level) }
    }

    public func initialize() throws {
        try check(using(-1) { mpv_initialize($0) }, "mpv_initialize")
        let thread = Thread { [self] in loop() }
        thread.name = "mpv events"
        thread.qualityOfService = .userInitiated
        self.thread = thread
        thread.start()
    }

    /// Asks for `name` as a node on every change; it arrives as `.property`.
    public func observe(_ name: String) {
        _ = using(-1) { mpv_observe_property($0, 0, name, MPV_FORMAT_NODE) }
    }

    // MARK: Events

    @discardableResult
    public func addHandler(_ handler: @escaping @Sendable (MPVEvent) -> Void) -> UUID {
        let id = UUID()
        lock.withLock { handlers[id] = handler }
        return id
    }

    public func removeHandler(_ id: UUID) {
        lock.withLock { _ = handlers.removeValue(forKey: id) }
    }

    /// The next event `matching` returns true for, or nil after `timeout`.
    public func next(
        timeout: Duration = .seconds(10),
        matching: @escaping @Sendable (MPVEvent) -> Bool
    ) async -> MPVEvent? {
        let box = OnceBox<MPVEvent?>()
        return await withCheckedContinuation { continuation in
            box.set(continuation)
            let id = addHandler { event in
                if matching(event) { box.resume(event) }
            }
            box.onResume = { [weak self] in self?.removeHandler(id) }
            Task {
                try? await Task.sleep(for: timeout)
                box.resume(nil)
            }
        }
    }

    private func loop() {
        while true {
            guard let pointer = mpv_wait_event(handle, -1) else { continue }
            let event = pointer.pointee
            let decoded: MPVEvent?
            switch event.event_id {
            case MPV_EVENT_NONE: decoded = nil
            case MPV_EVENT_PROPERTY_CHANGE:
                let property = event.data.assumingMemoryBound(to: mpv_event_property.self).pointee
                let name = String(cString: property.name)
                let value: MPVValue =
                    property.format == MPV_FORMAT_NODE
                    ? MPVValue(property.data.assumingMemoryBound(to: mpv_node.self).pointee)
                    : .none
                decoded = .property(name: name, value: value)
            case MPV_EVENT_LOG_MESSAGE:
                let message = event.data.assumingMemoryBound(to: mpv_event_log_message.self).pointee
                decoded = .log(
                    MPVLogLine(
                        prefix: String(cString: message.prefix),
                        level: String(cString: message.level),
                        text: String(cString: message.text).trimmingCharacters(in: .newlines)))
            case MPV_EVENT_FILE_LOADED: decoded = .fileLoaded
            case MPV_EVENT_END_FILE:
                let end = event.data.assumingMemoryBound(to: mpv_event_end_file.self).pointee
                decoded = .endFile(reason: end.reason.rawValue, error: end.error)
            case MPV_EVENT_PLAYBACK_RESTART: decoded = .playbackRestart
            case MPV_EVENT_VIDEO_RECONFIG: decoded = .videoReconfig
            case MPV_EVENT_SEEK: decoded = .seek
            case MPV_EVENT_IDLE: decoded = .idle
            case MPV_EVENT_SHUTDOWN: decoded = .shutdown
            default: decoded = .other(String(cString: mpv_event_name(event.event_id)))
            }
            if let decoded {
                let current = lock.withLock { Array(handlers.values) }
                for handler in current { handler(decoded) }
            }
            if event.event_id == MPV_EVENT_SHUTDOWN {
                finished.signal()
                return
            }
        }
    }

    /// Quits mpv, waits for the event thread to see it go and for calls in flight to return,
    /// and frees the handle. Calls after this return errors and do nothing.
    public func shutdown() {
        command(["quit"])
        _ = finished.wait(timeout: .now() + 5)
        gate.lock()
        guard alive else { gate.unlock(); return }
        alive = false
        while inFlight > 0 { gate.wait() }
        gate.unlock()
        mpv_terminate_destroy(handle)
    }

    private func using<T>(_ fallback: T, _ body: (OpaquePointer) -> T) -> T {
        gate.lock()
        guard alive else {
            gate.unlock()
            return fallback
        }
        inFlight += 1
        gate.unlock()
        defer {
            gate.lock()
            inFlight -= 1
            gate.broadcast()
            gate.unlock()
        }
        return body(handle)
    }

    // MARK: Commands

    @discardableResult
    public func command(_ args: [String]) -> Int32 {
        withCArgs(args) { cargs in using(-1) { mpv_command($0, cargs) } }
    }

    /// A command that returns a node, such as `screenshot-raw`.
    public func commandResult(_ args: [String]) -> (status: Int32, result: MPVValue) {
        var node = mpv_node()
        let status = withCArgs(args) { cargs in using(-1) { mpv_command_ret($0, cargs, &node) } }
        guard status >= 0 else { return (status, .none) }
        defer { mpv_free_node_contents(&node) }
        return (status, MPVValue(node))
    }

    /// Fire and forget, for the main thread.
    public func commandAsync(_ args: [String]) {
        _ = withCArgs(args) { cargs in using(-1) { mpv_command_async($0, 0, cargs) } }
    }

    private func withCArgs(_ args: [String], _ body: (UnsafeMutablePointer<UnsafePointer<CChar>?>) -> Int32) -> Int32 {
        var cargs: [UnsafePointer<CChar>?] = args.map { UnsafePointer(strdup($0)) }
        cargs.append(nil)
        defer { for pointer in cargs { free(UnsafeMutablePointer(mutating: pointer)) } }
        return cargs.withUnsafeMutableBufferPointer { body($0.baseAddress!) }
    }

    // MARK: Properties

    public func get(_ name: String) -> MPVValue {
        var node = mpv_node()
        guard using(-1, { mpv_get_property($0, name, MPV_FORMAT_NODE, &node) }) >= 0 else { return .none }
        defer { mpv_free_node_contents(&node) }
        return MPVValue(node)
    }

    public func string(_ name: String) -> String? {
        guard let cstring = using(nil, { mpv_get_property_string($0, name) }) else { return nil }
        defer { mpv_free(cstring) }
        return String(cString: cstring)
    }

    @discardableResult
    public func set(_ name: String, _ value: String) -> Int32 {
        using(-1) { mpv_set_property_string($0, name, value) }
    }

    public func setAsync(_ name: String, _ value: String) {
        commandAsync(["set", name, value])
    }

    /// Sets a list property (`glsl-shaders`) as one native array, so no path is ever split on
    /// a list separator.
    @discardableResult
    public func set(_ name: String, strings: [String]) -> Int32 {
        let cstrings = strings.map { strdup($0) }
        defer { cstrings.forEach { free($0) } }
        var values = cstrings.map { cstring -> mpv_node in
            var node = mpv_node()
            node.format = MPV_FORMAT_STRING
            node.u.string = cstring
            return node
        }
        return values.withUnsafeMutableBufferPointer { buffer in
            var list = mpv_node_list(num: Int32(buffer.count), values: buffer.baseAddress, keys: nil)
            return withUnsafeMutablePointer(to: &list) { listPointer in
                var node = mpv_node()
                node.format = MPV_FORMAT_NODE_ARRAY
                node.u.list = listPointer
                return using(-1) { mpv_set_property($0, name, MPV_FORMAT_NODE, &node) }
            }
        }
    }

    private func check(_ status: Int32, _ what: String) throws {
        if status < 0 { throw MPVError(code: status, what: what) }
    }
}

/// Resumes a continuation once, whichever of an event or a timeout comes first.
private final class OnceBox<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<T, Never>?
    var onResume: (@Sendable () -> Void)?

    func set(_ continuation: CheckedContinuation<T, Never>) {
        lock.withLock { self.continuation = continuation }
    }

    func resume(_ value: T) {
        let taken: CheckedContinuation<T, Never>? = lock.withLock {
            defer { continuation = nil }
            return continuation
        }
        guard let taken else { return }
        onResume?()
        taken.resume(returning: value)
    }
}
