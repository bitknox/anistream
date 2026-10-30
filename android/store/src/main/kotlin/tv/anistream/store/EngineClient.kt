// The engine as the app holds it: where commands go, and one coroutine reading what happened.

package tv.anistream.store

import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tv.anistream.engine.ffi.FfiEngine
import tv.anistream.engine.model.Command
import tv.anistream.engine.model.Event
import tv.anistream.engine.model.JobId

/**
 * Owns the [FfiEngine] and the one coroutine that loops over `nextEvents()`, handing each batch to
 * the main thread in the order the engine applied them. The next batch is read only once the last
 * has been applied, so batches never overtake each other. An empty batch means the engine stopped,
 * and ends the loop.
 *
 * @param deliver handed each batch, on [main].
 * @param stopped called once on [main], after the last batch.
 * @param reading where the batches are awaited and read out of the engine, off the main thread:
 *   a batch can carry artwork's bytes.
 */
class EngineClient(
    val engine: FfiEngine,
    scope: CoroutineScope,
    deliver: (List<Event>) -> Unit,
    stopped: () -> Unit,
    main: CoroutineDispatcher = Dispatchers.Main,
    reading: CoroutineDispatcher = Dispatchers.Default,
) {
    private val reader: Job = scope.launch(reading) {
        try {
            while (isActive) {
                val batch = engine.nextEvents()
                if (batch.isEmpty()) break
                // A batch that throws is dropped, not the reader: the next one still comes.
                withContext(main) {
                    try {
                        deliver(batch)
                    } catch (e: CancellationException) {
                        throw e
                    } catch (e: Exception) {
                        Log.e("anistream", "a batch of events could not be delivered", e)
                    }
                }
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.e("anistream", "the engine's events could not be read", e)
        }
        withContext(main) { stopped() }
    }

    /**
     * Hand the engine a command, and say which job it starts when it starts one. Only puts it on a
     * channel, so the main thread may call it. `null` too once the engine has stopped.
     */
    fun send(command: Command): JobId? = engine.send(command)

    /**
     * Stop the engine after everything sent before, and wait while it winds down, which with
     * torrents on takes up to a second. The reader then sees the stream end.
     */
    fun shutdown() = engine.shutdown()

    /** Stop reading, without stopping the engine. */
    fun cancel() = reader.cancel()
}
