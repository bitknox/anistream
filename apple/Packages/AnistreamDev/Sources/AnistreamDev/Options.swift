// What the harness was asked to do.

import AnistreamKit
import AnistreamPlayerUI
import AnistreamStore
import Foundation

/// The harness's command line.
///
///     swift run AnistreamDev                          the Watching scene, in a new temporary home
///     swift run AnistreamDev --scene fresh            a first launch
///     swift run AnistreamDev --scene playing          Continue starts the generated episode
///     swift run AnistreamDev --scene playing --player-shot up-next --snapshot shot.png
///                                                     play it to a state, render the player
///                                                     window, and quit
///     swift run AnistreamDev --snapshot shot.png      render the window once settled, and quit
///     swift run AnistreamDev --size 1440x900          another window size
///     swift run AnistreamDev --scene playing --media <file>
///                                                     the Playing scene on another file
///     swift run AnistreamDev --scene playing --player-shot playing --full-screen 3 --snapshot shot.png
///                                                     in and out of full screen three times,
///                                                     playing and then paused, shot each way
///     swift run AnistreamDev --art-trouble 3,400 --snapshot shot.png --frames 0,3000,9000
///                                                     every second image fails 3 times first and
///                                                     each waits 400 ms, shot once settled and
///                                                     then 3 s and 9 s after, untouched
///     swift run AnistreamDev -AnistreamTitle 920001 -AnistreamMotion season --art-cache 8 \
///         --snapshot shot.png --frames 0,50,100       the next season opened from the page's
///                                                     picker with the art cache kept to 8 MB,
///                                                     shot as it changes
///     swift run AnistreamDev --episodes-delay 800     each episode table held back 800 ms, as a
///                                                     slow source answers
///     swift run AnistreamDev --search-delay 1500      each search answer held back 1.5 s, as a
///                                                     slow network answers
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
    /// The player state to play to, whose window the snapshot renders.
    var playerShot: PlayerShot?
    /// What the Playing scene streams instead of the fixture episode.
    var media: String?
    /// The times to take the player in and out of full screen once at its shot.
    var fullScreenCycles = 0
    /// The milliseconds after the launch's motion demo (`-AnistreamMotion`) to snapshot at, each
    /// to the snapshot's name with `-<ms>` before its extension.
    var frames: [Int] = []
    /// How badly the fake serves the art: every second image fails this many times before it is
    /// served, and each waits the delay.
    var artTrouble: (failFirst: Int, delay: Duration)?
    /// How many megabytes of decoded art the cache keeps beside what is on screen, when not the
    /// app's own.
    var artCache: Int?
    /// How long each episode table waits before the screens see it, as a slow source answers.
    var episodesDelay: Duration?
    /// How long each search answer waits before the screens see it, as a slow network answers.
    var searchDelay: Duration?
    var width: CGFloat = 1280
    var height: CGFloat = 800

    static let usage = """
        usage: AnistreamDev [--scene watching|fresh|playing | --real [--shared]] [--home <dir>]
                            [--player-shot <state>] [--snapshot <file.png>] [--size <width>x<height>]
                            [--media <file>] [--full-screen <times>] [--art-trouble <fails>[,<ms>]]
                            [--art-cache <MB>] [--episodes-delay <ms>] [--search-delay <ms>]
        the player's states: \(PlayerShot.allCases.map(\.rawValue).joined(separator: ", "))
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
            case "--frames":
                let list = try value(flag)
                frames = list.split(separator: ",").compactMap { Int($0) }
                guard !frames.isEmpty else { throw Problem.bad(flag, list) }
            case "--player-shot":
                let name = try value(flag)
                guard let shot = PlayerShot(rawValue: name) else { throw Problem.bad(flag, name) }
                playerShot = shot
            case "--media":
                media = try value(flag)
            case "--full-screen":
                let times = try value(flag)
                guard let count = Int(times), count > 0 else { throw Problem.bad(flag, times) }
                fullScreenCycles = count
            case "--art-trouble":
                let spec = try value(flag)
                let parts = spec.split(separator: ",").map { Int($0) }
                guard let fails = parts.first ?? nil, fails >= 0, parts.count <= 2,
                      let delay = parts.count == 2 ? parts[1] : 0
                else { throw Problem.bad(flag, spec) }
                artTrouble = (fails, .milliseconds(delay))
            case "--art-cache":
                let megabytes = try value(flag)
                guard let size = Int(megabytes), size > 0 else { throw Problem.bad(flag, megabytes) }
                artCache = size
            case "--episodes-delay":
                let ms = try value(flag)
                guard let delay = Int(ms), delay >= 0 else { throw Problem.bad(flag, ms) }
                episodesDelay = .milliseconds(delay)
            case "--search-delay":
                let ms = try value(flag)
                guard let delay = Int(ms), delay >= 0 else { throw Problem.bad(flag, ms) }
                searchDelay = .milliseconds(delay)
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
        if let media, case .fixtures(.playing) = engine { engine = .fixtures(.playing(media: media)) }
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
