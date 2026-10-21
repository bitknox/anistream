// New episodes of the shows followed, as the background check finds them, and the engine that
// background work runs on.

package tv.anistream.store

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.job
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.withContext
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.FfiBackground
import tv.anistream.engine.ffi.FfiHost

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
