// What a title page's trailer does during one visit: wait, load, play, give way to the banner when
// the page scrolls away or the trailer ends, and let go.

package tv.anistream.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import tv.anistream.engine.model.TrailerControl

/**
 * One visit to a title page. The page asks [begin] once it has settled and the engine allows a
 * trailer; from then on the web view exists while [live] and the video is in front of the banner
 * while [shown]. The banner fades back before anything is paused or released, so the player's own
 * paused and end screens never show.
 *
 * - The banner stays, silently, if nothing plays within [loadTimeout] or the player reports an
 *   error.
 * - Scrolling away fades the banner back, pauses the video once it is covered, and releases the
 *   view after [awayLimit]; scrolling back before that plays it again and fades it in once it
 *   reports a picture.
 * - It ends once: the banner returns when the trailer is about to end, and it does not loop.
 *
 * @param send tells the page to play or pause.
 * @param fade how long the banner takes to come back, which the pause waits for.
 */
class TrailerVisit(
    private val scope: CoroutineScope,
    private val send: (TrailerControl) -> Unit,
    private val loadTimeout: Long = 8_000,
    private val awayLimit: Long = 30_000,
    private val fade: Long = 300,
) {
    private enum class Stage { Waiting, Loading, Playing, Held, Done }

    private var stage = Stage.Waiting
    private var away = false
    private var timer: Job? = null
    private var pause: Job? = null

    private val _live = MutableStateFlow(false)

    /** Whether the web view exists. */
    val live: StateFlow<Boolean> = _live.asStateFlow()

    private val _shown = MutableStateFlow(false)

    /** Whether the video is in front of the banner art. */
    val shown: StateFlow<Boolean> = _shown.asStateFlow()

    /** Start loading: the page has settled and trailers may play. Once per visit. */
    fun begin() {
        if (stage != Stage.Waiting) return
        stage = Stage.Loading
        _live.value = true
        timer = scope.launch {
            delay(loadTimeout)
            if (stage == Stage.Loading) finish()
        }
    }

    /** The page scrolled past the point where the video would be under the page, or back to the top. */
    fun scrolledAway(value: Boolean) {
        if (away == value) return
        away = value
        when (stage) {
            Stage.Playing -> if (value) {
                _shown.value = false
                pause = scope.launch {
                    delay(fade)
                    send(TrailerControl.PAUSE)
                    stage = Stage.Held
                    timer = scope.launch {
                        delay(awayLimit)
                        if (stage == Stage.Held) finish()
                    }
                }
            }
            Stage.Held -> if (!value) {
                timer?.cancel()
                stage = Stage.Loading
                send(TrailerControl.PLAY)
                timer = scope.launch {
                    delay(loadTimeout)
                    if (stage == Stage.Loading) finish()
                }
            }
            else -> {}
        }
        // Back before the pause landed: the video is still playing, so it simply comes forward.
        if (!value && stage == Stage.Playing && pause?.isActive == true) {
            pause?.cancel()
            _shown.value = true
        }
    }

    /** What the page said. */
    fun signal(signal: TrailerSignal) {
        when (signal) {
            TrailerSignal.Playing -> if (stage == Stage.Loading) {
                timer?.cancel()
                if (away) {
                    // Scrolled away while it loaded: it plays under the page for a moment, then waits.
                    stage = Stage.Playing
                    away = false
                    scrolledAway(true)
                } else {
                    stage = Stage.Playing
                    _shown.value = true
                }
            }
            TrailerSignal.Ending, TrailerSignal.Ended -> if (stage == Stage.Playing || stage == Stage.Loading) {
                _shown.value = false
                pause?.cancel()
                timer?.cancel()
                timer = scope.launch {
                    delay(fade)
                    finish()
                }
                stage = Stage.Done
            }
            is TrailerSignal.Failed -> if (stage != Stage.Done) finish()
            TrailerSignal.Quiet -> {}
        }
    }

    /** The conditions for playing no longer hold: let go, if the trailer had started. */
    fun leave() {
        if (stage != Stage.Waiting) release()
    }

    /** The page was left, a sheet or the player opened over it, or the app went to the background. */
    fun release() {
        if (stage != Stage.Done || _live.value) finish()
    }

    private fun finish() {
        timer?.cancel()
        pause?.cancel()
        stage = Stage.Done
        _shown.value = false
        _live.value = false
    }
}
