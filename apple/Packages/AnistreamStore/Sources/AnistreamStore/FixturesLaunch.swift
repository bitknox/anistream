// How the Fixtures app and the dev harness start the testing engine.

#if FIXTURES
import AnistreamKit
import Foundation

extension EngineLaunch {
    /// The launch argument that names the scene: `-AnistreamScene watching` or `fresh`.
    public static let sceneArgument = "-AnistreamScene"

    /// The scene `arguments` name, Watching when they name none.
    public static func scene(arguments: [String]) -> FfiScene? {
        guard let index = arguments.firstIndex(of: sceneArgument) else { return .watching }
        guard arguments.indices.contains(index + 1) else { return nil }
        switch arguments[index + 1].lowercased() {
        case "watching": return .watching
        case "fresh": return .fresh
        default: return nil
        }
    }

    /// The testing engine on the scene the arguments name, in the home override when there is
    /// one and in a new temporary directory otherwise: a scene seeds the database it starts on,
    /// so every launch starts from an empty one, and never from the terminal's. `temporary` is
    /// that directory, for the caller to remove at quit.
    public static func fixtures(
        platform: Platform,
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
#endif
