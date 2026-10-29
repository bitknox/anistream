// Jobs: work the user started and is waiting on.

package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import tv.anistream.engine.model.JobId
import tv.anistream.engine.model.Msg

/** One job, from the `JobStarted` that names it to the `JobDone` that ends it. */
data class RunningJob(
    val id: JobId,
    /** What it is doing, in the engine's words: "loading continue watching". */
    val activity: String,
    /** When it started, in milliseconds on the model's clock. */
    val started: Long,
    /** What it is held up on, when that is not the work itself. */
    val note: Msg? = null,
    /** Until when, on the same clock, when that is known, for the note to count down. */
    val waitUntil: Long? = null,
)

/**
 * The jobs running, and the ones that have run long enough to show.
 *
 * The 600 ms rule, the terminal's: work earns a place on screen only once it has run that long.
 * Most loads answer well inside it, and a row that blinked in and out on every tap would shift the
 * screen for nothing; past it, the wait is long enough to wonder about.
 *
 * @param now the clock in milliseconds, which a test replaces.
 * @param scope where the recheck at the delay runs; `null` for a test that steps its own clock and
 *   calls [refresh].
 */
class JobsModel(
    private val now: () -> Long = { System.nanoTime() / 1_000_000 },
    private val scope: CoroutineScope? = null,
) {
    private val _running = MutableStateFlow<List<RunningJob>>(emptyList())

    /** Every job running, oldest first. */
    val running: StateFlow<List<RunningJob>> = _running.asStateFlow()

    private val _shown = MutableStateFlow<List<RunningJob>>(emptyList())

    /** The jobs to show now, newest first. */
    val shown: StateFlow<List<RunningJob>> = _shown.asStateFlow()

    /** The jobs that have run at least [DELAY_MILLIS] by [instant], newest first. */
    fun visible(instant: Long): List<RunningJob> =
        _running.value.filter { instant - it.started >= DELAY_MILLIS }.reversed()

    /** Recompute what shows, at the clock's now. */
    fun refresh() {
        val visible = visible(now())
        if (visible != _shown.value) _shown.value = visible
    }

    internal fun started(job: JobId, activity: String) {
        _running.value = _running.value + RunningJob(job, activity, now())
        scope?.launch {
            delay(DELAY_MILLIS)
            refresh()
        }
    }

    internal fun note(job: JobId, note: Msg, waitMillis: Long?) {
        _running.value = _running.value.map {
            if (it.id == job) it.copy(note = note, waitUntil = waitMillis?.let { wait -> now() + wait }) else it
        }
        refresh()
    }

    internal fun done(job: JobId) {
        _running.value = _running.value.filterNot { it.id == job }
        refresh()
    }

    companion object {
        /** How long work runs before it shows. */
        const val DELAY_MILLIS = 600L
    }
}
