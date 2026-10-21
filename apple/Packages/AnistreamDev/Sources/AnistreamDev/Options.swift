// What the harness was asked to do.

import AnistreamKit
import AnistreamStore
import Foundation

/// The harness's command line.
///
///     swift run AnistreamDev                          the Watching scene, in a new temporary home
///     swift run AnistreamDev --scene fresh            a first launch
///     swift run AnistreamDev --snapshot shot.png      render the window once settled, and quit
///     swift run AnistreamDev --size 1440x900          another window size
///     swift run AnistreamDev --home <dir>             keep the files in <dir> instead
///     swift run AnistreamDev --real                   the app's own engine, real sources and
///                                                     AniList, in a new temporary home
///     swift run AnistreamDev --real --shared          the app's own engine on the terminal's
///                                                     directories and keychain: your own data
struct Options {
    enum Engine: Equatable {
        case fixtures(FfiScene)
        case real
    }

    var engine: Engine = .fixtures(.watching)
    /// The override root, when given.
    var home: URL?
    /// Whether to run on the terminal's own directories.
    var shared = false
    /// Where to write the snapshot, when one is asked for.
    var snapshot: URL?
    var width: CGFloat = 1280
    var height: CGFloat = 800

    static let usage = """
        usage: AnistreamDev [--scene watching|fresh | --real [--shared]] [--home <dir>]
                            [--snapshot <file.png>] [--size <width>x<height>]
        """

    enum Problem: Error, CustomStringConvertible {
        case unknown(String)
        case missing(String)
        case bad(String, String)
        case sharedWithoutReal

        var description: String {
            switch self {
            case .unknown(let flag): "unknown option \(flag)"
            case .missing(let flag): "\(flag) needs a value"
            case .bad(let flag, let value): "\(flag) does not take \(value)"
            case .sharedWithoutReal: "--shared needs --real: the testing engine never runs on your data"
            }
        }
    }

    init(arguments: [String]) throws {
        var rest = arguments.dropFirst()[...]
        func value(_ flag: String) throws -> String {
            guard let value = rest.popFirst() else { throw Problem.missing(flag) }
            return value
        }
        while let flag = rest.popFirst() {
            switch flag {
            case "--scene":
                let name = try value(flag)
                guard let scene = EngineLaunch.scene(arguments: [EngineLaunch.sceneArgument, name])
                else { throw Problem.bad(flag, name) }
                engine = .fixtures(scene)
            case "--real":
                engine = .real
            case "--shared":
                shared = true
            case "--home":
                home = URL(filePath: try value(flag), directoryHint: .isDirectory)
            case "--snapshot":
                snapshot = URL(filePath: try value(flag))
            case "--size":
                let size = try value(flag)
                let parts = size.split(separator: "x").compactMap { Double($0) }
                guard parts.count == 2 else { throw Problem.bad(flag, size) }
                (width, height) = (parts[0], parts[1])
            default:
                // What AppKit passes a process on its own, such as -NSDocumentRevisionsDebugMode.
                if flag.hasPrefix("-"), !flag.hasPrefix("--") {
                    _ = rest.popFirst()
                    continue
                }
                throw Problem.unknown(flag)
            }
        }
        if shared, engine != .real { throw Problem.sharedWithoutReal }
    }

    /// The launch these options describe, and the temporary home to remove at quit.
    func launch() -> (EngineLaunch, temporary: URL?) {
        let temporary: URL? = home == nil && !shared
            ? FileManager.default.temporaryDirectory
                .appending(path: "anistream-dev-\(UUID().uuidString)", directoryHint: .isDirectory)
            : nil
        let home: EngineHome = shared ? .resolve(environment: [:], arguments: []) : .override(root: (self.home ?? temporary)!)
        switch engine {
        case .fixtures(let scene):
            return (EngineLaunch(engine: .fixtures(scene), home: home, platform: .macOs, secrets: .host), temporary)
        case .real:
            return (EngineLaunch.app(home: home, platform: .macOs), temporary)
        }
    }
}
