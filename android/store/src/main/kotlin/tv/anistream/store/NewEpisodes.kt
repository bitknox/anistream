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
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.job
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.withContext
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.ffi.FfiBackground
import tv.anistream.engine.core.Translation
import tv.anistream.engine.ffi.FfiHost
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.NoticeReason
import tv.anistream.engine.model.UpcomingNotice
import tv.anistream.engine.model.UpcomingNotices
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
    /**
     * The episode the viewer is up to, when that is before this one: "Next for you: episode 8",
     * and the one Play starts.
     */
    val nextForYou: UInt? = null,
    /** Whether the viewer watches the show dubbed. */
    val dubbed: Boolean = false,
    /** The show's banner, fetched like the cover, for the expanded picture. */
    val banner: Bitmap? = null,
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
 * and hands it to [widgets] to keep, so the widgets are as fresh as the check: with the store
 * whose engine made it, for its art to be fetched while that engine is up.
 */
class EngineEpisodeCheck(
    private val engine: BackgroundEngine,
    private val widgets: suspend (AppStore, WidgetData) -> Unit,
    private val coverSize: Int = COVER_SIZE,
) : EpisodeCheck {
    override suspend fun newEpisodes(): List<NewEpisode> = engine.use { store ->
        val said = store.ask(Command.CheckNewEpisodes, CHECK_MILLIS) { (it as? Event.NewEpisodes)?.v1 }
            ?: throw Unanswered("the check for new episodes")
        val art = store.covers(said.flatMap { listOfNotNull(it.coverUrl, it.bannerUrl) }.distinct(), coverSize, COVERS_MILLIS)
        store.ask(Command.WidgetSnapshot, SNAPSHOT_MILLIS) { (it as? Event.WidgetSnapshot)?.v1 }?.let { widgets(store, it) }
        said.map {
            NewEpisode(
                id = it.id,
                title = it.title,
                episode = it.episode,
                airedAt = it.airedAt,
                coverUrl = it.coverUrl,
                art = it.coverUrl?.let(art::get),
                nextForYou = it.nextForYou,
                dubbed = it.translation == Translation.DUB,
                banner = it.bannerUrl?.let(art::get),
            )
        }
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
         * else, no sync loops, update check, digest or keep next of its own.
         */
        val QUIET = FfiBackground(
            prune = false,
            updateCheck = false,
            digest = false,
            datasets = false,
            syncLoops = false,
            storeRefresh = false,
            playerCheck = false,
            keepNext = false,
        )
    }
}

/**
 * The Notifications settings' "Coming up · next 7 days": the week's broadcasts of the shows the
 * new-episode check follows, and of those a show's own switch leaves out, as the engine says them
 * (`UpcomingNotices`), each saying whether it notifies.
 */
class ComingModel {
    private val _week = MutableStateFlow<UpcomingNotices?>(null)

    /** The week as the engine last said it; `null` until it has. */
    val week: StateFlow<UpcomingNotices?> = _week.asStateFlow()

    internal fun apply(week: UpcomingNotices) {
        _week.value = week
    }

    companion object {
        /**
         * The rows the pane lists, in the order they air: everything but what a show's own switch
         * leaves out, which the footer names instead.
         */
        fun listed(week: UpcomingNotices): List<UpcomingNotice> = week.airings.filter { it.reason != NoticeReason.SWITCHED_OFF }

        /** The shows a switch of their own leaves out, each once, by title. */
        fun switchedOff(week: UpcomingNotices): List<String> =
            week.airings.filter { it.reason == NoticeReason.SWITCHED_OFF }.distinctBy { it.id }.map { it.title }

        /** The shows watched dubbed that notify, each once: told when the dub is out, not at the broadcast. */
        fun dubbed(week: UpcomingNotices): List<String> =
            week.airings.filter { it.notifies && it.translation == Translation.DUB }.distinctBy { it.id }.map { it.title }

        /**
         * The footer under the week: what notifies later than its broadcast and why, what a switch
         * of its own leaves out, and what a row does, or that nothing airs.
         */
        fun footer(week: UpcomingNotices): String {
            val dubbed = dubbed(week)
            val off = switchedOff(week)
            return listOfNotNull(
                when (dubbed.size) {
                    0 -> null
                    1 -> "${dubbed[0]} notifies when its dub is out."
                    else -> "${Wording.list(dubbed)} notify when their dubs are out."
                },
                when (off.size) {
                    0 -> null
                    1 -> "${off[0]} is off in its own settings."
                    else -> "${Wording.list(off)} are off in their own settings."
                },
                if (listed(week).isEmpty()) "Nothing airs this week for the shows followed." else "A show opens its settings.",
            ).joinToString(" ")
        }
    }
}
