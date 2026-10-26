// The features page's TV video: the TV app's Fixtures build, through its transitions to resuming
// an episode, while website/scripts/videos/record.sh films the screen with `adb shell
// screenrecord`. As on the phone (android/app/src/androidTestFixtures), a flow logs
// "ANISTREAM_VIDEO start|plays|end <seconds since 1970>" under the tag anistream-video: where the
// clip starts, where the episode begins playing (from there nothing is squeezed) and where it
// ends. The instrumentation argument media names the episode every stream plays,
// website/scripts/videos/episode.sh's cut of こうしす！ #3.1 (CC BY 4.0), pushed to the device.
package tv.anistream.tv.video

import android.content.Intent
import android.os.SystemClock
import android.util.Log
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
     * The TV with a remote, at a promo's pace: focus moves from Continue's Resume to Title page
     * (TV1), the page crossfades in (TV5), down to the seasons, Season 2's page in this one's
     * place and back to Season 1, down to the episodes where episode 7 has the focus, and Center
     * resumes it (TV6). Storyboard: ~/anistream-notes/videos/storyboard-android.md.
     */
    @Test
    fun resume() {
        launch()
        // The hero's action has the focus, as it does for a person with a remote.
        find(By.textStartsWith("Resume episode 7"), timeout = 60_000)
        settle()
        // From here every wait is the flow's own, timed to the motion it waits on, and looked for
        // without waiting for the app to be idle, which it never is while a video plays.
        Configurator.getInstance().waitForIdleTimeout = 0
        mark("start")
        device.pressDPadRight()
        focusOn(By.text("Title page"))
        hold(FOCUS)
        device.pressDPadCenter()
        await(By.text("Seasons"))
        hold(PAGE)
        device.pressDPadDown()
        focusOn(By.text("Season 1"))
        hold(SCROLL)
        device.pressDPadRight()
        focusOn(By.text("Season 2"))
        hold(FOCUS)
        device.pressDPadCenter()
        await(By.text("2 of 2"))
        hold(SEASON)
        device.pressDPadLeft()
        focusOn(By.text("Season 1"))
        hold(FOCUS)
        device.pressDPadCenter()
        await(By.text("1 of 2"))
        hold(SEASON)
        device.pressDPadDown()
        focusOn(By.descStartsWith("E7 · Rain over Kiriya Pass"))
        hold(SCROLL)
        device.pressDPadCenter()
        // The player's Pause button shows once the picture is up and playing. The mark comes a
        // moment after the picture, and feature-videos.ts finds where the episode's own frames
        // begin before it.
        await(By.text("Pause"), timeout = 30_000)
        mark("plays")
        pause(2.0)
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
     * The TV's Fixtures app on the Watching scene, with the episode the recorder names as every
     * episode's stream, the extras given, and each instrumentation argument extra.<name> passed
     * on as the extra <name>.
     */
    private fun launch(vararg extras: Pair<String, String>) {
        val context = instrumentation.targetContext
        val arguments = InstrumentationRegistry.getArguments()
        val intent = Intent()
            .setClassName(context.packageName, "tv.anistream.tv.TvActivity")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            .putExtra("scene", "watching")
        arguments.getString("media")?.let { intent.putExtra("media", it) }
        for (key in arguments.keySet().filter { it.startsWith("extra.") }) {
            intent.putExtra(key.removePrefix("extra."), arguments.getString(key))
        }
        for ((key, value) in extras) intent.putExtra(key, value)
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

    /** The element once it is on screen, looked for every 16 ms. */
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

    /** Waits for the focus to be on [selector]: that element, or the one holding it. */
    private fun focusOn(selector: BySelector) {
        val deadline = SystemClock.uptimeMillis() + 5_000
        while (device.findObject(By.focused(true).hasDescendant(selector, 3)) == null) {
            val own = device.findObjects(selector).any { it.isFocused }
            if (own) return
            if (SystemClock.uptimeMillis() > deadline) {
                dump("no focus on: $selector")
                throw AssertionError("no focus on: $selector")
            }
            SystemClock.sleep(16)
        }
    }

    /** Where the clip starts, the episode plays or the clip ends, for the recorder. */
    private fun mark(edge: String) {
        val now = System.currentTimeMillis() / 1000.0
        Log.i(TAG, "ANISTREAM_VIDEO $edge ${String.format(Locale.ROOT, "%.3f", now)}")
    }

    /** A pause between two steps. Until the episode plays, wherever the screen stands still the
     *  encode keeps one frame in three, so 2.4 s here is 0.8 s in the clip. */
    private fun pause(seconds: Double) = Thread.sleep((seconds * 1000).toLong())

    /** A motion of [seconds] played out, then [READ] of the screen at rest. */
    private fun hold(seconds: Double) = pause(seconds + READ)

    /** The art's fade-in after a screen's list arrives. */
    private fun settle() = pause(2.4)

    private companion object {
        const val TAG = "anistream-video"

        // Each step's motion, from android/docs/tv-design-brief.md's Motion on TV.
        /** TV1: the focus ring and scale on quick, about 0.21 s. */
        const val FOCUS = 0.25
        /** TV5: the page's crossfade on fade-in, after its content arrives. */
        const val PAGE = 0.4
        /** TV2: the page scrolling the focused row to its keyline, with the focus. */
        const val SCROLL = 0.45
        /** The season's page in this one's place: its rows' fade. */
        const val SEASON = 0.45

        /** The screen at rest after each step: 0.6 s, which the encode's squeeze makes 0.2 s. */
        const val READ = 0.6
    }
}
