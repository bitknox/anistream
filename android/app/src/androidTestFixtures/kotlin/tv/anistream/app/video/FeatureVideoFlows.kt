// The features page's videos: each test runs through the Fixtures app's best transitions at a
// promo's pace, while website/scripts/videos/record.sh films the screen with `adb shell
// screenrecord`. A flow marks where its clip starts and ends by logging "ANISTREAM_VIDEO
// start|end <seconds since 1970>" under the tag anistream-video, and the recorder cuts there,
// with a moment of the settled screen before the start. Nothing here asserts more than a flow
// needs to find its way; when a step finds nothing, the screen's elements are logged.
//
// The instrumentation argument media (am instrument -e media <path>) names the episode every stream
// plays: website/scripts/videos/episode.sh's cut of こうしす！ #3.1 (CC BY 4.0), pushed to the device.
package tv.anistream.app.video

import android.content.Intent
import android.os.SystemClock
import android.util.Log
import android.view.InputDevice
import android.view.MotionEvent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
import androidx.test.uiautomator.Configurator
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.UiObject2
import androidx.test.uiautomator.Until
import java.io.ByteArrayOutputStream
import java.util.Locale
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class FeatureVideoFlows {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val device = UiDevice.getInstance(instrumentation)

    /**
     * The phone's best transitions, at a promo's pace: Continue's hero grows into Stardust
     * Wayfarer's page (T1, the page's ground and content flying as two layers around the art),
     * the season picker morphs to Season 2 and the page crossfades into it, and back (T7), the
     * page scrolled to its episodes under the banner's parallax, then predictive back: a peek at Continue under the shrinking page, let go, and a back that
     * lands the page in the hero (T1 in reverse), so the clip ends on the Continue it began on.
     * Storyboard: ~/anistream-notes/videos/storyboard-android.md.
     */
    @Test
    fun title() {
        launch()
        find(By.text("Resume E7 · 10:06"), timeout = 60_000)
        settle()
        // From here every wait is the flow's own, timed to the motion it waits on.
        Configurator.getInstance().waitForIdleTimeout = 0
        mark("start")
        await(By.text("Stardust Wayfarer")).click()
        await(By.text("Season 2"))
        hold(FLIGHT)
        await(By.text("Season 2")).click()
        await(By.text("Start E1"))
        hold(SEASON)
        await(By.text("Season 1")).click()
        await(By.text("Resume E7 · 10:06"))
        hold(SEASON)
        // The episodes, with the banner going up at half the list's speed.
        scroll(from = 0.78f, to = 0.5f, millis = 640)
        pause(READ)
        // Predictive back: the page shrinks toward the hero under the finger, and the finger
        // going back to the edge lets it grow back; then a back that commits lands it in the hero.
        back(reach = 0.3f, commit = false)
        hold(SPRING_BACK)
        back(reach = 0.42f, commit = true)
        await(By.text("In progress"))
        hold(FLIGHT)
        mark("end")
    }

    /** Explore a screen: launch with the instrumentation's extras and log what is on it. For
     *  writing a flow, never recorded. */
    @Test
    fun explore() {
        launch()
        pause(6.0)
        dump("explore")
    }

    /**
     * The Fixtures app on the Watching scene, as a first launch, with the episode the recorder
     * names as every episode's stream, and each instrumentation argument extra.<name> passed on
     * as the extra <name>.
     */
    private fun launch() {
        val context = instrumentation.targetContext
        val arguments = InstrumentationRegistry.getArguments()
        val intent = Intent()
            .setClassName(context.packageName, "tv.anistream.app.MainActivity")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            .putExtra("scene", "watching")
        arguments.getString("media")?.let { intent.putExtra("media", it) }
        for (key in arguments.keySet().filter { it.startsWith("extra.") }) {
            intent.putExtra(key.removePrefix("extra."), arguments.getString(key))
        }
        context.startActivity(intent)
    }

    /** The element once it is there, or the screen's elements logged and the flow stopped. */
    private fun find(selector: BySelector, timeout: Long = 10_000): UiObject2 =
        device.wait(Until.findObject(selector), timeout) ?: run {
            dump("not found: $selector")
            throw AssertionError("not found: $selector")
        }

    private fun dump(why: String) {
        val out = ByteArrayOutputStream()
        device.dumpWindowHierarchy(out)
        Log.i(TAG, "ANISTREAM_VIDEO_DUMP $why")
        out.toString().lines().forEach { Log.i(TAG, it) }
    }

    /**
     * A back gesture from the left edge, drawn as a finger does it: out to [reach] of the screen's
     * width, easing out, a rest, then either let go there, which commits, or eased back to the
     * edge first, which cancels.
     */
    private fun back(reach: Float, commit: Boolean) {
        val y = device.displayHeight * 0.55f
        val far = device.displayWidth * reach
        Finger(2f, y).apply {
            move(far, y, 560, easeOut)
            rest()
            if (!commit) {
                move(2f, y, 480, easeInOut)
                rest()
            }
            lift()
        }
    }

    /** A finger drawn up the middle of the screen, between fractions of its height, then lifted
     *  after a rest, so the list comes to rest where it lifts, with no fling. */
    private fun scroll(from: Float, to: Float, millis: Long) {
        val x = device.displayWidth / 2f
        Finger(x, device.displayHeight * from).apply {
            move(x, device.displayHeight * to, millis, easeOut)
            rest()
            lift()
        }
    }

    /**
     * One finger on the screen, put down where it is made. Each move is sent every 8 ms without
     * waiting for the app, so what follows the finger draws on every frame: UiDevice.swipe waits on
     * each of its steps, and on an emulator that drew on every second or third frame.
     *
     * A rest is [REST]: a shorter one, 130 to 200 ms, is squeezed by the encode into a single
     * repeated frame between two that move, a stutter.
     */
    private inner class Finger(private var x: Float, private var y: Float) {
        private val down = SystemClock.uptimeMillis()
        private var t = 0L

        init {
            send(MotionEvent.ACTION_DOWN, down)
        }

        fun move(toX: Float, toY: Float, millis: Long, ease: (Double) -> Double) {
            val fromX = x
            val fromY = y
            val start = t
            while (t < start + millis) {
                t += 8
                keepTime()
                val p = ease(((t - start).toDouble() / millis).coerceAtMost(1.0)).toFloat()
                x = fromX + (toX - fromX) * p
                y = fromY + (toY - fromY) * p
                send(MotionEvent.ACTION_MOVE, down + t)
            }
        }

        fun rest() {
            t += REST
            keepTime()
        }

        fun lift() = send(MotionEvent.ACTION_UP, SystemClock.uptimeMillis())

        private fun keepTime() = SystemClock.sleep((down + t - SystemClock.uptimeMillis()).coerceAtLeast(0))

        private fun send(action: Int, at: Long) {
            val event = MotionEvent.obtain(down, at, action, x, y, 0).apply { source = InputDevice.SOURCE_TOUCHSCREEN }
            instrumentation.uiAutomation.injectInputEvent(event, false)
            event.recycle()
        }
    }

    private val easeOut = { p: Double -> 1 - Math.pow(1 - p, 3.0) }
    private val easeInOut = { p: Double -> if (p < 0.5) 4 * p * p * p else 1 - Math.pow(-2 * p + 2, 3.0) / 2 }

    /**
     * The element once it is on screen, looked for every 16 ms without waiting for the app to be
     * idle, so a step follows the one before as soon as its result shows.
     */
    private fun await(selector: BySelector, timeout: Long = 10_000): UiObject2 {
        val deadline = SystemClock.uptimeMillis() + timeout
        while (true) {
            device.findObject(selector)?.let { return it }
            if (SystemClock.uptimeMillis() > deadline) {
                dump("not found: $selector")
                throw AssertionError("not found: $selector")
            }
            SystemClock.sleep(16)
        }
    }

    /** Where the clip starts or ends, for the recorder. */
    private fun mark(edge: String) {
        val now = System.currentTimeMillis() / 1000.0
        Log.i(TAG, "ANISTREAM_VIDEO $edge ${String.format(Locale.ROOT, "%.3f", now)}")
    }

    /** A pause between two steps. Wherever the screen stands still the encode keeps one frame
     *  in three, so 2.4 s here is 0.8 s in the clip. */
    private fun pause(seconds: Double) = Thread.sleep((seconds * 1000).toLong())

    /** A motion of [seconds] played out, then [READ] of the screen at rest. */
    private fun hold(seconds: Double) = pause(seconds + READ)

    /** The art's fade-in after a screen's list arrives. */
    private fun settle() = pause(2.4)

    private companion object {
        const val TAG = "anistream-video"

        // Each step's motion, from android/docs/motion-android.md, as it plays on the emulator.
        /** T1: the shared bounds' spring, about 0.31 s, and the page's content fading in. */
        const val FLIGHT = 0.45
        /** T7: the picker's morph, about 0.3 s, and the pages' crossfade on standard. */
        const val SEASON = 0.5
        /** T1's predictive back let go before it commits: the page grows back on standard. */
        const val SPRING_BACK = 0.35

        /** The screen at rest after each step: 0.6 s, which the encode's squeeze makes 0.2 s. */
        const val READ = 0.6
        /** A finger at rest in a gesture, 0.1 s in the clip. */
        const val REST = 300L
    }
}
