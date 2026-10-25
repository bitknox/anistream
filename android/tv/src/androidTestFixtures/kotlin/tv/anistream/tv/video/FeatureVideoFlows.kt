// The features page's TV video: the TV app's Fixtures build resuming an episode, while
// website/scripts/videos/record.sh films the screen with `adb shell screenrecord`. As on the
// phone (android/app/src/androidTestFixtures), a flow logs "ANISTREAM_VIDEO start|plays|end
// <seconds since 1970>" under the tag anistream-video: where the clip starts, where the episode
// begins playing (from there nothing is squeezed) and where it ends. The instrumentation argument
// media names the episode every stream plays, website/scripts/videos/episode.sh's cut of
// こうしす！ #3.1 (CC BY 4.0), pushed to the device.
package tv.anistream.tv.video

import android.content.Intent
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.BySelector
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

    /** A show's page on the TV, and episode 7 resumed from where it was left. */
    @Test
    fun resume() {
        launch("title" to "900001")
        // The hero's action has the focus, as it does for a person with a remote.
        find(By.textStartsWith("Resume episode 7"), timeout = 60_000)
        settle()
        mark("start")
        pause(1.5)
        device.pressDPadCenter()
        pause(4.5)
        mark("plays")
        pause(4.0)
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

    /** Where the clip starts, the episode plays or the clip ends, for the recorder. */
    private fun mark(edge: String) {
        val now = System.currentTimeMillis() / 1000.0
        Log.i(TAG, "ANISTREAM_VIDEO $edge ${String.format(Locale.ROOT, "%.3f", now)}")
    }

    /** A pause between two steps. Until the episode plays, wherever the screen stands still the
     *  encode keeps one frame in three, so 2.4 s here is 0.8 s in the clip. */
    private fun pause(seconds: Double) = Thread.sleep((seconds * 1000).toLong())

    /** The art's fade-in after a screen's list arrives. */
    private fun settle() = pause(2.4)

    private companion object {
        const val TAG = "anistream-video"
    }
}
