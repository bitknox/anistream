// The features page's videos: each test is one task done in the Fixtures app at a person's pace,
// while website/scripts/videos/record.sh films the screen with `adb shell screenrecord`. A flow
// marks where its clip starts and ends by logging "ANISTREAM_VIDEO start|end <seconds since 1970>"
// under the tag anistream-video, and the recorder cuts there, with a moment of the settled screen
// before the start. Nothing here asserts more than a flow needs to find its way; when a step finds
// nothing, the screen's elements are logged.
//
// The instrumentation argument media (am instrument -e media <path>) names the episode every stream
// plays: website/scripts/videos/episode.sh's cut of こうしす！ #3.1 (CC BY 4.0), pushed to the device.
package tv.anistream.app.video

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

    /** The app on Continue, then a show's page and its episodes. */
    @Test
    fun title() {
        launch()
        find(By.text("Resume E7 · 10:06"), timeout = 60_000)
        settle()
        mark("start")
        find(By.text("Stardust Wayfarer")).click()
        find(By.text("More about this show"))
        pause(2.4)
        device.swipe(540, 1900, 540, 1000, 60)
        pause(2.4)
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

    /** Where the clip starts or ends, for the recorder. */
    private fun mark(edge: String) {
        val now = System.currentTimeMillis() / 1000.0
        Log.i(TAG, "ANISTREAM_VIDEO $edge ${String.format(Locale.ROOT, "%.3f", now)}")
    }

    /** A pause between two steps. Wherever the screen stands still the encode keeps one frame
     *  in three, so 2.4 s here is 0.8 s in the clip. */
    private fun pause(seconds: Double) = Thread.sleep((seconds * 1000).toLong())

    /** The art's fade-in after a screen's list arrives. */
    private fun settle() = pause(2.4)

    private companion object {
        const val TAG = "anistream-video"
    }
}
