// How the app starts the engine, in one value.

import AnistreamKit
import Foundation

/// Everything a start takes, gathered so that the engine's start record, `FfiStart` (the theme,
/// the platform, the trackers' client ids and where secrets go), is filled from here and nowhere
/// else: when it lands, `start(host:)` is the one call that changes.
public struct EngineLaunch: Sendable {
    /// Which engine.
    public enum Engine: Sendable, Equatable {
        /// The app's own, on the real sources and AniList.
        case app
        #if FIXTURES
        /// The testing engine on a scene: fake AniList, fixture titles, nothing leaving the
        /// machine. The Fixtures app, the tests and the dev harness only.
        case fixtures(FfiScene)
        #endif
    }

    /// The device the engine runs on, which the engine's start record will say.
    public enum Platform: Sendable, Equatable {
        case mac, pad, phone
    }

    /// Where the engine keeps tokens, which the start record will say.
    public enum Secrets: Sendable, Equatable {
        /// Always the app's keychain, through its host.
        case host
        /// As the config's `trackers.token_storage` says, as the terminal app does: the
        /// keychain (the same items, through the host) or `0600` files under the data directory.
        case followConfig
    }

    public var engine: Engine
    public var home: EngineHome
    public var background: FfiBackground
    public var platform: Platform
    public var secrets: Secrets
    /// How mpv's interface is dressed once the player is the app's: the palette's hexes.
    public var theme: PlayerTheme
    /// The client ids that ship with the app, for the native sign-in. None are registered yet.
    public var anilistClientID: String?
    public var malClientID: String?

    public init(
        engine: Engine,
        home: EngineHome,
        background: FfiBackground = FfiBackground(),
        platform: Platform,
        secrets: Secrets,
        theme: PlayerTheme = EngineLaunch.theme,
        anilistClientID: String? = nil,
        malClientID: String? = nil
    ) {
        self.engine = engine
        self.home = home
        self.background = background
        self.platform = platform
        self.secrets = secrets
        self.theme = theme
        self.anilistClientID = anilistClientID
        self.malClientID = malClientID
    }

    /// The app's launch on this device: the Mac shares the terminal's directories and follows
    /// its token storage; iOS keeps everything in its container and keychain.
    public static func app(home: EngineHome = .resolve(), platform: Platform) -> EngineLaunch {
        EngineLaunch(
            engine: .app,
            home: home,
            platform: platform,
            secrets: platform == .mac ? .followConfig : .host
        )
    }

    /// The palette as the player is dressed in it, the terminal's immersive one: silk text, haze
    /// for what is dim, amber, water, vermilion, on the ground colour.
    public static let theme = PlayerTheme(
        text: "#E4E0D6",
        dim: "#9298B5",
        obi: "#F2A64B",
        state: "#7FC7D9",
        alert: "#E06B54",
        ground: "#161A2E",
        font: "Zen Kaku Gothic New"
    )

    /// Stand the engine up. Blocks while it opens its files, so call it off the main thread.
    public func start(host: any FfiHost) throws -> FfiEngine {
        switch engine {
        case .app:
            // Today's start takes the directories, the host and the background switches. The
            // platform, the theme, the client ids and `secrets` go into `FfiStart` here once the
            // engine takes it; until then the engine keeps every token in the host's keychain.
            return try FfiEngine.start(dirs: home.dirs, host: host, background: background)
        #if FIXTURES
        case .fixtures(let scene):
            return try FfiEngine.startScene(dirs: home.dirs, host: host, scene: scene)
        #endif
        }
    }
}

extension FfiError {
    /// What failed, in the engine's words, which read whole on screen.
    public var reason: String {
        switch self {
        case .Dirs(let reason), .Config(let reason), .Store(let reason), .Client(let reason),
             .Runtime(let reason), .Engine(let reason):
            reason
        }
    }
}
