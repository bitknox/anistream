// How the app starts the engine, in one value, and where the engine keeps its files.

package tv.anistream.store

import android.content.Context
import android.content.res.Configuration
import java.io.File
import tv.anistream.engine.ffi.FfiBackground
import tv.anistream.engine.ffi.FfiDirs
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.ffi.FfiHost
import tv.anistream.engine.ffi.FfiPlatform
import tv.anistream.engine.ffi.FfiStart
import tv.anistream.engine.ffi.SecretsMode
import tv.anistream.engine.model.PlayerTheme

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
    /** Where the AniList client redirects; `null` for `anistream://auth/anilist`. */
    val anilistRedirectUri: String? = null,
    val malClientId: String? = null,
    val simklClientId: String? = null,
    val traktClientId: String? = null,
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
            anilistRedirectUri = anilistRedirectUri,
            malClientId = malClientId,
            simklClientId = simklClientId,
            traktClientId = traktClientId,
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
         * clients anistream registered with AniList, MyAnimeList, Simkl and Trakt, which are not
         * secret.
         */
        fun app(home: EngineHome, androidAniListClientId: String? = ANDROID_ANILIST_CLIENT_ID): EngineLaunch {
            val anilist = aniListClient(androidAniListClientId)
            return EngineLaunch(
                engine = Engine.App,
                home = home,
                anilistClientId = anilist.id,
                anilistRedirectUri = anilist.redirect,
                malClientId = MAL_CLIENT_ID,
                simklClientId = SIMKL_CLIENT_ID,
                traktClientId = TRAKT_CLIENT_ID,
            )
        }

        /**
         * The AniList client the app signs in with. AniList's implicit grant puts the token in the
         * redirect, and any app can claim `anistream://`, so the Android client,
         * [ANDROID_ANILIST_CLIENT_ID], redirects to a verified App Link, [ANILIST_APP_LINK]. With
         * none given, the shared client and the app's scheme, as on Apple, whose sign-in sheet no
         * other app can answer.
         */
        fun aniListClient(androidClientId: String?): AniListClient =
            androidClientId?.takeIf { it.isNotBlank() }?.let { AniListClient(it, ANILIST_APP_LINK) }
                ?: AniListClient(ANILIST_CLIENT_ID, null)

        /**
         * anistream's AniList client for Apple and the terminal: the implicit grant, redirecting to
         * `anistream://auth/anilist`.
         */
        const val ANILIST_CLIENT_ID = "54537"

        /** anistream's Android AniList client: the implicit grant, redirecting to [ANILIST_APP_LINK]. */
        const val ANDROID_ANILIST_CLIENT_ID = "54545"

        /** Where the Android AniList client redirects: the App Link the manifest verifies. */
        const val ANILIST_APP_LINK = "https://anistream.tv/auth/anilist"

        /** anistream's MyAnimeList client: PKCE, redirecting to `anistream://auth/mal`. */
        const val MAL_CLIENT_ID = "0ccc4725381aca9513f26d32107a94cd"

        /** anistream's Simkl client, for its device sign-in. */
        const val SIMKL_CLIENT_ID = "016a3721d77c54d04f5aa6f5a213fe76f047d83d3ae35b2c892074859b88be0b"

        /** anistream's Trakt client, for its device sign-in: a client with no secret. */
        const val TRAKT_CLIENT_ID = "p2yWSSGe7aVAlnHEMiQQh0RhiL6ivZfJNaubOJi4DZE"

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

/** An AniList client, and where it redirects: `null` for the app's scheme. */
data class AniListClient(val id: String, val redirect: String?)

/**
 * What the engine is told the app runs on: a TV when the system says it is in television mode, else
 * a phone, or a tablet once the screen is at least 600 dp wide the short way. A plugin store this
 * device joins with an invite calls it by that kind, "Android TV", "Android phone" or "Android
 * tablet", unless the person joining names it otherwise. A phone with no [context] to read from.
 */
fun enginePlatform(context: Context? = null): FfiPlatform? {
    val configuration = context?.resources?.configuration
    val mode = (configuration?.uiMode ?: 0) and Configuration.UI_MODE_TYPE_MASK
    if (mode == Configuration.UI_MODE_TYPE_TELEVISION) return FfiPlatform.ANDROID_TV
    val shortSide = configuration?.smallestScreenWidthDp ?: 0
    return if (shortSide >= 600) FfiPlatform.ANDROID_TABLET else FfiPlatform.ANDROID
}

/**
 * What a fixtures launch puts in the database before the engine starts: the Apple app's
 * `-AnistreamScene` names.
 */
enum class FixtureScene {
    /** Nothing: a first launch. */
    FRESH,

    /** Signed in to the fake AniList, with four titles under way. */
    WATCHING,

    /** [WATCHING] with no source set up: every title's episodes are AniList's alone. */
    SOURCELESS,

    /** [WATCHING] with a first source that lists four titles for the first fixture and is sure of none: its page asks for the match. */
    UNSURE,

    /** A scene that plays, with the first six episodes of the first fixture watched. */
    PLAYING,

    /** Home's second case: every episode of the first fixture watched, so the spotlight is from Up next for you. */
    SUGGESTED,

    /** Signed out with one show left in Continue, the spotlight's first case. */
    ONE_LEFT,

    /** Fifteen shows under way, so Continue shows twelve with View all. */
    CONTINUE_FULL,

    /** Nothing watched and no source has any title: Add to Planning on every main action. */
    NOT_FOUND,

    /** [WATCHING] with AniList slow for the discovery shelves. */
    SLOW,

    /** [WATCHING] with AniList failing for the discovery shelves. */
    FAILED,

    /** Offline from the start with nothing saved. */
    OFFLINE;

    companion object {
        /** The scene [name] says, case aside; `null` for one there is none of. */
        fun named(name: String): FixtureScene? {
            val plain = name.replace("_", "").replace("-", "")
            return entries.firstOrNull { it.name.replace("_", "").equals(plain, ignoreCase = true) }
        }
    }
}
