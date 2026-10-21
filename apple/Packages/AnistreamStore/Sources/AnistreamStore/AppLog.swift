// The app's log: the engine's own tracing and the app's lines, in one file a day under the
// cache's `logs` directory (`app.log.<date>`), beside the terminal's `anistream.log.<date>`.

import AnistreamKit
import Foundation
import os

/// Lines for the app's log, which also go to the unified log under the same source.
public enum AppLog {
    /// Start writing the log under `cache`, once a process.
    public static func start(cache: URL) {
        _ = startLog(cache: cache.path(percentEncoded: false))
    }

    public static func info(_ source: String, _ message: String) { write(.info, source, message) }
    public static func warn(_ source: String, _ message: String) { write(.warn, source, message) }
    public static func error(_ source: String, _ message: String) { write(.error, source, message) }
    public static func debug(_ source: String, _ message: String) { write(.debug, source, message) }

    private static func write(_ level: FfiLogLevel, _ source: String, _ message: String) {
        logLine(level: level, source: source, message: message)
        let logger = Logger(subsystem: "tv.anistream", category: source)
        switch level {
        case .error: logger.error("\(message, privacy: .public)")
        case .warn: logger.warning("\(message, privacy: .public)")
        case .info: logger.info("\(message, privacy: .public)")
        case .debug: logger.debug("\(message, privacy: .public)")
        }
    }

    /// Milliseconds from `start` to now, for timings in the log.
    public static func ms(since start: ContinuousClock.Instant) -> Int {
        let elapsed = ContinuousClock.now - start
        return Int(elapsed.components.seconds * 1000 + elapsed.components.attoseconds / 1_000_000_000_000_000)
    }

    /// An address as a log may hold it: scheme, host and port, never its path or query, which can
    /// carry tokens.
    public static func address(_ url: String) -> String {
        guard let parts = URLComponents(string: url), let scheme = parts.scheme else {
            return url.hasPrefix("/") ? "a local file" : "an address"
        }
        guard let host = parts.host else { return "\(scheme):" }
        return parts.port.map { "\(scheme)://\(host):\($0)" } ?? "\(scheme)://\(host)"
    }
}
