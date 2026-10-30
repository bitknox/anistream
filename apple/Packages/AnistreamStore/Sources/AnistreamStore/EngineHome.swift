// Where the engine keeps its files: config.toml and the plugins, the database, and what can be
// fetched again.

import AnistreamKit
import Foundation

/// The engine's three directories.
///
/// - macOS: the terminal app's own, so both apps share one config, one database and one set of
///   plugins: `~/Library/Application Support/anistream` for config and data, and
///   `~/Library/Caches/anistream` for the cache. It is the rule the terminal resolves them by,
///   `directories::ProjectDirs::from("", "", "anistream")`. Only one of the two should be open
///   at a time: nothing stops a second one, and each takes the queued downloads as its own.
/// - iOS and iPadOS: the app container's Application Support and Caches.
/// - Either, overridden: a root of the caller's, with `config`, `data` and `cache` under it, from
///   the `ANISTREAM_HOME_OVERRIDE` variable or the `-AnistreamHome <path>` launch argument. Tests,
///   the dev harness and the screenshot runs point it at a directory of their own, so nothing
///   they do reaches the real one.
public struct EngineHome: Sendable, Equatable {
    public var config: URL
    public var data: URL
    public var cache: URL
    /// Whether this is an override rather than the platform's directories.
    public var isOverride: Bool

    public init(config: URL, data: URL, cache: URL, isOverride: Bool = false) {
        self.config = config
        self.data = data
        self.cache = cache
        self.isOverride = isOverride
    }

    /// The environment variable that overrides the directories.
    public static let overrideVariable = "ANISTREAM_HOME_OVERRIDE"
    /// The launch argument that does, followed by the path.
    public static let overrideArgument = "-AnistreamHome"

    /// The directories as the engine takes them.
    public var dirs: FfiDirs {
        FfiDirs(config: config.path, data: data.path, cache: cache.path)
    }

    /// The directories under `root`, as an override lays them out.
    public static func override(root: URL) -> EngineHome {
        EngineHome(
            config: root.appending(path: "config", directoryHint: .isDirectory),
            data: root.appending(path: "data", directoryHint: .isDirectory),
            cache: root.appending(path: "cache", directoryHint: .isDirectory),
            isOverride: true
        )
    }

    /// The terminal app's directories on macOS for the user whose home is `home`.
    public static func shared(home: URL) -> EngineHome {
        let library = home.appending(path: "Library", directoryHint: .isDirectory)
        let support = library.appending(path: "Application Support/anistream", directoryHint: .isDirectory)
        return EngineHome(
            config: support,
            data: support,
            cache: library.appending(path: "Caches/anistream", directoryHint: .isDirectory)
        )
    }

    /// An app container's directories on iOS.
    public static func container(applicationSupport: URL, caches: URL) -> EngineHome {
        EngineHome(config: applicationSupport, data: applicationSupport, cache: caches)
    }

    /// The override's root, when the environment or the arguments give one.
    public static func overrideRoot(
        environment: [String: String],
        arguments: [String]
    ) -> URL? {
        if let index = arguments.firstIndex(of: overrideArgument),
           arguments.indices.contains(index + 1) {
            return URL(filePath: arguments[index + 1], directoryHint: .isDirectory)
        }
        if let path = environment[overrideVariable], !path.isEmpty {
            return URL(filePath: path, directoryHint: .isDirectory)
        }
        return nil
    }

    /// This process's directories: the override when there is one, the platform's otherwise.
    public static func resolve(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        arguments: [String] = ProcessInfo.processInfo.arguments,
        fileManager: FileManager = .default
    ) -> EngineHome {
        if let root = overrideRoot(environment: environment, arguments: arguments) {
            return override(root: root)
        }
        #if os(macOS)
        return shared(home: fileManager.homeDirectoryForCurrentUser)
        #else
        return container(
            applicationSupport: fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0],
            caches: fileManager.urls(for: .cachesDirectory, in: .userDomainMask)[0]
        )
        #endif
    }
}
