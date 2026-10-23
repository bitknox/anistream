// How the Fixtures app and the dev harness start the testing engine.

#if FIXTURES
import AnistreamKit
import Foundation

extension EngineLaunch {
    /// The launch argument that names the scene: `-AnistreamScene watching`, `fresh`,
    /// `sourceless` (Watching with no source set up) or `playing`.
    public static let sceneArgument = "-AnistreamScene"

    /// The launch argument that names what the Playing scene streams: `-AnistreamMedia <path>`.
    public static let mediaArgument = "-AnistreamMedia"

    /// What the Playing scene streams when the arguments name nothing: the episode
    /// `apple/scripts/player-media.sh` makes in this checkout.
    public static let fixtureEpisode = URL(filePath: #filePath)
        .deletingLastPathComponent()
        .appending(path: "../../../../.mpv/media/episode.mkv")
        .standardizedFileURL

    /// The scene `arguments` name, Watching when they name none.
    public static func scene(arguments: [String]) -> FfiScene? {
        guard let index = arguments.firstIndex(of: sceneArgument) else { return .watching }
        guard arguments.indices.contains(index + 1) else { return nil }
        switch arguments[index + 1].lowercased() {
        case "watching": return .watching
        case "fresh": return .fresh
        case "sourceless": return .sourceless
        case "playing": return .playing(media: media(arguments: arguments))
        default: return nil
        }
    }

    /// What the Playing scene streams: the file or URL `-AnistreamMedia` names, otherwise the
    /// fixture episode.
    static func media(arguments: [String]) -> String {
        if let index = arguments.firstIndex(of: mediaArgument), arguments.indices.contains(index + 1) {
            return arguments[index + 1]
        }
        return fixtureEpisode.path(percentEncoded: false)
    }

    /// The testing engine on the scene the arguments name, in the home override when there is
    /// one and in a new temporary directory otherwise: a scene seeds the database it starts on,
    /// so every launch starts from an empty one, and never from the terminal's. `temporary` is
    /// that directory, for the caller to remove at quit.
    public static func fixtures(
        platform: FfiPlatform,
        arguments: [String] = ProcessInfo.processInfo.arguments,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> (launch: EngineLaunch, temporary: URL?) {
        let scene = scene(arguments: arguments) ?? .watching
        let root = EngineHome.overrideRoot(environment: environment, arguments: arguments)
        let temporary = root == nil
            ? FileManager.default.temporaryDirectory
                .appending(path: "anistream-fixtures-\(UUID().uuidString)", directoryHint: .isDirectory)
            : nil
        let launch = EngineLaunch(
            engine: .fixtures(scene),
            home: .override(root: root ?? temporary!),
            platform: platform,
            secrets: .host
        )
        return (launch, temporary)
    }
}
extension AppStore {
    /// The launch argument that marks episodes watched once the engine is up, as if they had just
    /// been watched: `-AnistreamWatched 900006:8`, the episodes joined by commas. For a shot of a
    /// title caught up with its broadcasts.
    public static let watchedArgument = "-AnistreamWatched"

    /// Mark what `arguments` name watched.
    public func applyWatchedArgument(_ arguments: [String] = ProcessInfo.processInfo.arguments) {
        guard let index = arguments.firstIndex(of: Self.watchedArgument), arguments.indices.contains(index + 1)
        else { return }
        let parts = arguments[index + 1].split(separator: ":", maxSplits: 1)
        guard parts.count == 2, let id = AnilistId(parts[0]) else { return }
        send(.setWatched(id: id, episodes: parts[1].split(separator: ",").map(String.init), watched: true))
        // Asked again after the mark, which the engine takes in order: what the screens asked for
        // while it started went ahead of it, and says the title as it was.
        continuing.list.asked()
        send(.loadContinue)
        send(.loadDetail(id))
    }
}
#endif
