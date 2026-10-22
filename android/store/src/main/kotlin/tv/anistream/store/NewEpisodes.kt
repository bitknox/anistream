// New episodes of the shows followed, as the background check finds them, and the engine that
// background work runs on.

package tv.anistream.store

import android.graphics.Bitmap
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.job
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.withContext
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.FfiBackground
import tv.anistream.engine.ffi.FfiHost
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.WidgetData

/**
 * An episode of a show being followed that aired since the show was last checked, said once: the
 * engine's `NewEpisode`. Spoiler-safe by what it leaves out, the episode's name.
 */
data class NewEpisode(
    /** The show. */
    val id: AnilistId,
    /** The show's title, as every list names it. */
    val title: String,
    /** The episode's number. */
    val episode: UInt,
    /** When it aired, in seconds since the Unix epoch. */
    val airedAt: Long,
    /** The show's cover, for the notification's picture. */
    val coverUrl: String? = null,
    /** That cover, fetched for the notification while the check's engine was up. */
    val art: Bitmap? = null,
)

/**
 * The background check for new episodes, `Command::CheckNewEpisodes`: what the periodic work asks,
 * behind an interface a test fakes.
 */
fun interface EpisodeCheck {
    /**
     * The episodes to notify of, in the order they aired; empty when nothing is new or
     * notifications are off. Throws when the engine could not be asked, so the work tries again.
     */
    suspend fun newEpisodes(): List<NewEpisode>
}

/**
 * The check on the engine, run as `CheckNewEpisodes` says to run it in the background: on the
 * app's engine while it runs, or on a quiet one of the check's own. It takes the episodes the
 * check says, fetches each one's cover for its notification, and then asks for the widgets' record
 * and hands it to [widgets] to keep, so the widgets are as fresh as the check.
 */
class EngineEpisodeCheck(
    private val engine: BackgroundEngine,
    private val widgets: (WidgetData) -> Unit,
    private val coverSize: Int = COVER_SIZE,
) : EpisodeCheck {
    override suspend fun newEpisodes(): List<NewEpisode> = engine.use { store ->
        val said = store.ask(Command.CheckNewEpisodes, CHECK_MILLIS) { (it as? Event.NewEpisodes)?.v1 }
            ?: throw Unanswered("the check for new episodes")
        val covers = store.covers(said.mapNotNull { it.coverUrl }.distinct(), coverSize, COVERS_MILLIS)
        store.ask(Command.WidgetSnapshot, SNAPSHOT_MILLIS) { (it as? Event.WidgetSnapshot)?.v1 }?.let(widgets)
        said.map { NewEpisode(it.id, it.title, it.episode, it.airedAt, it.coverUrl, it.coverUrl?.let(covers::get)) }
    }

    /** The engine did not answer [what] in time. */
    class Unanswered(what: String) : Exception("$what was not answered")

    companion object {
        /** The check answers within 15 s whatever the network does; this leaves it room. */
        const val CHECK_MILLIS = 30_000L

        /** How long the covers may take, all of them; a notification without one still posts. */
        const val COVERS_MILLIS = 10_000L

        const val SNAPSHOT_MILLIS = 20_000L

        /** A cover's long side for a notification: its large icon and its expanded picture. */
        const val COVER_SIZE = 720
    }
}

/**
 * The art for [urls], fetched through the engine and decoded to at most [size] on its long side,
 * by URL: as much of it as came within [timeoutMillis].
 */
suspend fun AppStore.covers(urls: List<String>, size: Int, timeoutMillis: Long): Map<String, Bitmap> {
    for (url in urls) artwork.request(url, size)
    withTimeoutOrNull(timeoutMillis) {
        while (urls.any(artwork::isPending)) delay(50)
    }
    return urls.mapNotNull { url -> artwork.image(url)?.let { url to it.image } }.toMap()
}

/**
 * The engine background work runs on: the app's own while it runs, or one started for the work
 * alone, with every background switch off, and stopped once the work is done.
 *
 * @param running the app's store, when its engine is up or starting.
 * @param launch the launch for an engine of the work's own; its background switches are turned off.
 * @param host the host that engine calls back into.
 */
class BackgroundEngine(
    private val running: () -> AppStore?,
    private val launch: () -> EngineLaunch,
    private val host: () -> FfiHost,
) {
    /** The work running on an engine of its own, while one does. Read and set on the main thread. */
    private val sessions = mutableSetOf<Job>()

    /** Whether work runs on an engine of its own now, which the app's must wait for. */
    val busy: Boolean get() = sessions.isNotEmpty()

    /**
     * Stop the work on engines of their own, for the app's engine to start: each stops as it
     * would after its work, having done what was sent, and its caller gets [Yielded].
     */
    suspend fun stop() {
        for (session in sessions.toList()) session.cancel(Yielded())
        joinAll(*sessions.toTypedArray())
    }

    /**
     * Run [work] on a store with an engine behind it, on the main thread, as every store call is.
     * An engine started for it stops after it, having done everything sent before; one that does
     * not start throws [Unavailable], and one stopped for the app's throws [Yielded].
     */
    suspend fun <T> use(work: suspend (AppStore) -> T): T = withContext(Dispatchers.Main) {
        running()?.let { return@withContext work(it) }
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
        val store = AppStore(scope)
        val session = coroutineContext.job
        sessions += session
        try {
            store.start(launch().copy(background = QUIET), host())
            when (val phase = store.phase.first { it != AppStore.Phase.Starting }) {
                is AppStore.Phase.Failed -> throw Unavailable(phase.reason)
                else -> {}
            }
            work(store)
        } finally {
            withContext(NonCancellable) {
                // An engine stopped as it starts is stopped once it has: it would otherwise run on.
                store.phase.first { it != AppStore.Phase.Starting }
                withContext(Dispatchers.IO) { store.shutdown() }
            }
            scope.cancel()
            sessions -= session
        }
    }

    /** The engine did not start, in its words. */
    class Unavailable(reason: String) : Exception(reason)

    /** The work was stopped for the app's engine to start: it can run again on that one. */
    class Yielded : CancellationException("the app's engine is starting")

    companion object {
        /**
         * Every background switch off: the work's engine answers what it is sent and nothing
         * else, no sync loops, update check or digest of its own.
         */
        val QUIET = FfiBackground(
            prune = false,
            updateCheck = false,
            digest = false,
            datasets = false,
            syncLoops = false,
            storeRefresh = false,
            playerCheck = false,
        )
    }
}
