// How the app starts the engine, in one value, and where the engine keeps its files.

package tv.anistream.store

import java.io.File
import uniffi.anistream_ffi.FfiBackground
import uniffi.anistream_ffi.FfiDirs
import uniffi.anistream_ffi.FfiEngine
import uniffi.anistream_ffi.FfiHost
import uniffi.anistream_ffi.FfiPlatform
import uniffi.anistream_ffi.FfiStart
import uniffi.anistream_ffi.SecretsMode
import uniffi.anistream_model.PlayerTheme

/**
 * The engine's three directories: config.toml and the plugins, the database, and what can be
 * fetched again.
 *
 * The app's are its sandbox's own, which nothing else reads: config and data in the files
 * directory, the cache in the cache directory, which the system may clear. The fixtures flavour
 * puts each launch in a directory of its own instead, [override], so every launch starts empty.
 */
data class EngineHome(
    val config: File,
    val data: File,
    val cache: File,
    /** Whether this is an override rather than the sandbox's directories. */
    val isOverride: Boolean = false,
) {
    /** The directories as the engine takes them. */
    val dirs: FfiDirs get() = FfiDirs(config = config.path, data = data.path, cache = cache.path)

    companion object {
        /** The sandbox's directories: `filesDir` and `cacheDir`. */
        fun sandbox(files: File, cache: File): EngineHome =
            EngineHome(config = files.resolve("anistream"), data = files.resolve("anistream"), cache = cache.resolve("anistream"))

        /** The directories under [root], as an override lays them out. */
        fun override(root: File): EngineHome = EngineHome(
            config = root.resolve("config"),
            data = root.resolve("data"),
            cache = root.resolve("cache"),
            isOverride = true,
        )
    }
}

/**
 * Everything a start takes, in one place: the directories, the background work, and what the
 * engine's start record, [FfiStart], says of the app (the player's theme, the platform, the
 * sign-in clients and where tokens go).
 */
data class EngineLaunch(
    val engine: Engine,
    val home: EngineHome,
    val background: FfiBackground = FfiBackground(),
    /** The device the engine runs on, when the engine has a name for it. */
    val platform: FfiPlatform? = enginePlatform(),
    /** Where tokens go: always the host's keystore-backed file on Android. */
    val secrets: SecretsMode = SecretsMode.HOST,
    /** How mpv's interface is dressed once the player is the app's: the palette's hexes. */
    val theme: PlayerTheme = THEME,
    /** The sign-in clients that ship with the app. */
    val anilistClientId: String? = null,
    val malClientId: String? = null,
) {
    /** Which engine. */
    sealed interface Engine {
        /** The app's own, on the real sources and AniList. */
        data object App : Engine

        /**
         * The testing engine on a scene: fake AniList, fixture titles, nothing leaving the
         * machine. The fixtures flavour and the tests only; the app flavour cannot start it.
         */
        data class Fixtures(val scene: FixtureScene) : Engine
    }

    /** What the engine is told of the app. */
    val record: FfiStart
        get() = FfiStart(
            theme = theme,
            platform = platform,
            anilistClientId = anilistClientId,
            malClientId = malClientId,
            secrets = secrets,
        )

    /** Stand the engine up. Blocks while it opens its files, so call it off the main thread. */
    fun start(host: FfiHost): FfiEngine = when (engine) {
        Engine.App -> FfiEngine.start(home.dirs, host, background, record)
        // The testing engine keeps tokens in files under the data directory, whatever `secrets` says.
        is Engine.Fixtures -> startFixtures(engine.scene, home.dirs, host, record)
    }

    companion object {
        /**
         * The app's launch: the sandbox's directories, tokens through the host, and the sign-in
         * clients anistream registered with AniList and MyAnimeList, which are not secret.
         */
        fun app(home: EngineHome): EngineLaunch = EngineLaunch(
            engine = Engine.App,
            home = home,
            anilistClientId = ANILIST_CLIENT_ID,
            malClientId = MAL_CLIENT_ID,
        )

        /** anistream's AniList client: the implicit grant, redirecting to `anistream://auth/anilist`. */
        const val ANILIST_CLIENT_ID = "54537"

        /** anistream's MyAnimeList client: PKCE, redirecting to `anistream://auth/mal`. */
        const val MAL_CLIENT_ID = "0ccc4725381aca9513f26d32107a94cd"

        /**
         * The palette as the player is dressed in it, the terminal's immersive one: silk text, haze
         * for what is dim, amber, water, vermilion, on the ground colour.
         */
        val THEME = PlayerTheme(
            text = "#E4E0D6",
            dim = "#9298B5",
            obi = "#F2A64B",
            state = "#7FC7D9",
            alert = "#E06B54",
            ground = "#161A2E",
            font = "Zen Kaku Gothic New",
        )
    }
}

/**
 * What the engine is told the app runs on. `FfiPlatform` has no Android yet (the engine's EA1 adds
 * it), and nothing in the engine differs by it, so the app names none until then.
 */
fun enginePlatform(): FfiPlatform? = null

/**
 * What a fixtures launch puts in the database before the engine starts: the Apple app's
 * `-AnistreamScene` names.
 */
enum class FixtureScene {
    /** Nothing: a first launch. */
    FRESH,

    /** Signed in to the fake AniList, with four titles under way. */
    WATCHING,

    /** A scene that plays, with the first six episodes of the first fixture watched. */
    PLAYING;

    companion object {
        /** The scene [name] says, case aside; `null` for one there is none of. */
        fun named(name: String): FixtureScene? = entries.firstOrNull { it.name.equals(name, ignoreCase = true) }
    }
}
