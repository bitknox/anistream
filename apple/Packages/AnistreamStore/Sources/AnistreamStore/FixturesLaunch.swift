// How the Fixtures app and the dev harness start the testing engine.

#if FIXTURES
import AnistreamKit
import Foundation

extension EngineLaunch {
    /// The launch argument that names the scene: `-AnistreamScene watching`, `fresh`,
    /// `sourceless` (Watching with no source set up), `unsure` (Watching with a source ahead that
    /// cannot choose 900001's match) or `playing`.
    public static let sceneArgument = "-AnistreamScene"

    /// The launch argument that turns on the Home world, `-AnistreamHomeWorld 1`: about ten
    /// invented titles this season with drawn art, which Home's shelves need to look like the
    /// board. The engine reads it as `ANISTREAM_FIXTURES_HOME_WORLD=1`, set here before it starts.
    public static let homeWorldArgument = "-AnistreamHomeWorld"

    /// Set the Home world in the environment when the arguments ask for it.
    public static func applyHomeWorld(arguments: [String] = ProcessInfo.processInfo.arguments) {
        guard let index = arguments.firstIndex(of: homeWorldArgument), arguments.indices.contains(index + 1),
              arguments[index + 1] == "1" else { return }
        setenv("ANISTREAM_FIXTURES_HOME_WORLD", "1", 1)
    }

    /// The launch argument that names what the Playing scene streams: `-AnistreamMedia <path>`.
    /// With the Watching scene, every episode streams it, so a resume plays it at once.
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
        case "watching":
            guard let index = arguments.firstIndex(of: mediaArgument), arguments.indices.contains(index + 1)
            else { return .watching }
            return .watchingMedia(media: arguments[index + 1])
        case "fresh": return .fresh
        case "suggested": return .suggested
        case "oneleft": return .oneLeft
        case "continuefull": return .continueFull
        case "notfound": return .notFound
        case "slow": return .slow
        case "failed": return .failed
        case "offline": return .offline
        case "sourceless": return .sourceless
        case "unsure": return .unsure
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
        applyHomeWorld(arguments: arguments)
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

    /// The launch argument that takes the fake AniList away once the first Home has loaded,
    /// `-AnistreamGoOffline 1`: Home's offline line over what it kept, for a shot of it.
    public static let goOfflineArgument = "-AnistreamGoOffline"

    /// Go offline when `arguments` ask, once settled, and ask Home again. Whether it did.
    public func applyGoOfflineArgument(_ arguments: [String] = ProcessInfo.processInfo.arguments) -> Bool {
        guard let index = arguments.firstIndex(of: Self.goOfflineArgument), arguments.indices.contains(index + 1),
              arguments[index + 1] == "1"
        else { return false }
        setFakeOffline(true)
        loadHome(force: true)
        return true
    }
}
#endif
