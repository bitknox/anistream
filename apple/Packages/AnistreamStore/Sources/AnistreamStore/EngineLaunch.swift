// How the app starts the engine, in one value.

import AnistreamKit
import Foundation

/// Everything a start takes, in one place: the directories, the background work, and what the
/// engine's start record, `FfiStart`, says of the app (the player's theme, the platform, the
/// sign-in clients and where tokens go).
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

    public var engine: Engine
    public var home: EngineHome
    public var background: FfiBackground
    /// The device the engine runs on.
    public var platform: FfiPlatform
    /// Where tokens go: the host's keychain, or as the config's `trackers.token_storage` says,
    /// as the terminal app keeps them (the keychain through the host, or `0600` files).
    public var secrets: SecretsMode
    /// How mpv's interface is dressed once the player is the app's: the palette's hexes.
    public var theme: PlayerTheme
    /// The client ids that ship with the app, for the native sign-in: `app`'s registered ones,
    /// none for the Fixtures app, whose trackers are the fake's.
    public var anilistClientID: String?
    public var malClientID: String?
    /// The device sign-ins' clients: Simkl's, and Trakt's, which has no secret.
    public var simklClientID: String?
    public var traktClientID: String?

    /// anistream's AniList client: implicit grant, redirecting to `anistream://auth/anilist`.
    /// Not a secret; it ships in the app.
    public static let anilistClient = "54537"
    /// anistream's MyAnimeList client: type "other", PKCE, redirecting to `anistream://auth/mal`.
    /// Not a secret either.
    public static let malClient = "0ccc4725381aca9513f26d32107a94cd"
    /// anistream's Simkl client, for its device-code sign-in. Not a secret.
    public static let simklClient = "016a3721d77c54d04f5aa6f5a213fe76f047d83d3ae35b2c892074859b88be0b"
    /// anistream's Trakt client, for its device-code sign-in, registered without a secret.
    public static let traktClient = "p2yWSSGe7aVAlnHEMiQQh0RhiL6ivZfJNaubOJi4DZE"

    public init(
        engine: Engine,
        home: EngineHome,
        background: FfiBackground = FfiBackground(),
        platform: FfiPlatform,
        secrets: SecretsMode,
        theme: PlayerTheme = EngineLaunch.theme,
        anilistClientID: String? = nil,
        malClientID: String? = nil,
        simklClientID: String? = nil,
        traktClientID: String? = nil
    ) {
        self.engine = engine
        self.home = home
        self.background = background
        self.platform = platform
        self.secrets = secrets
        self.theme = theme
        self.anilistClientID = anilistClientID
        self.malClientID = malClientID
        self.simklClientID = simklClientID
        self.traktClientID = traktClientID
    }

    /// The app's launch on this device: the Mac shares the terminal's directories and follows
    /// its token storage; iOS keeps everything in its container and keychain.
    public static func app(home: EngineHome = .resolve(), platform: FfiPlatform) -> EngineLaunch {
        EngineLaunch(
            engine: .app,
            home: home,
            platform: platform,
            secrets: platform == .macOs ? .followConfig : .host,
            anilistClientID: anilistClient,
            malClientID: malClient,
            simklClientID: simklClient,
            traktClientID: traktClient
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

    /// What the engine is told of the app. The sign-ins redirect to the app's own scheme,
    /// `anistream://auth/<tracker>`, so the redirect fields stay unset.
    public var record: FfiStart {
        FfiStart(
            theme: theme,
            platform: platform,
            anilistClientId: anilistClientID,
            malClientId: malClientID,
            simklClientId: simklClientID,
            traktClientId: traktClientID,
            secrets: secrets
        )
    }

    /// Stand the engine up. Blocks while it opens its files, so call it off the main thread.
    public func start(host: any FfiHost) throws -> FfiEngine {
        switch engine {
        case .app:
            return try FfiEngine.start(dirs: home.dirs, host: host, background: background, start: record)
        #if FIXTURES
        case .fixtures(let scene):
            // The testing engine keeps tokens in files under the data directory, whatever
            // `secrets` says.
            return try FfiEngine.startScene(dirs: home.dirs, host: host, scene: scene, start: record)
        #endif
        }
    }
}

extension FfiError {
    /// What failed, in the engine's words, which read whole on screen.
    public var reason: String {
        switch self {
        case .Dirs(let reason), .Config(let reason), .Store(let reason), .Client(let reason),
             .Runtime(let reason), .Engine(let reason), .Remote(let reason):
            reason
        }
    }
}
