package tv.anistream.widgets

import android.appwidget.AppWidgetProviderInfo
import android.app.Application
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Path
import android.graphics.RectF
import android.provider.Settings
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.TextView
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.glance.ExperimentalGlanceApi
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.compose
import androidx.glance.appwidget.composeForPreview
import androidx.test.core.app.ApplicationProvider
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File
import java.nio.file.Files
import java.time.Clock
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.Shadows.shadowOf
import tv.anistream.engine.model.WidgetData
import tv.anistream.store.AppClock
import tv.anistream.testkit.FixtureArt
import tv.anistream.ui.AppLink
import tv.anistream.widgets.Fixtures.NOW
import tv.anistream.widgets.Fixtures.ZONE
import tv.anistream.widgets.Fixtures.at

/**
 * The widgets as the launcher draws them: composed by Glance into RemoteViews, applied, and drawn
 * on the JVM at the boards' sizes (NativeWidgetsAndroid, NativeWidgetsStates), from the canvas's
 * fixtures at Tuesday 19:30. `./gradlew :widgets:recordRoborazziFixturesDebug` writes them to
 * android/.shots/roborazzi/widget-*.png. The other tests here read what is drawn.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-xhdpi")
class WidgetShots {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private lateinit var files: File

    /** The app, as the widgets see it: a record that can change, the fixtures' art, and links as plain intents. */
    private inner class Host(record: WidgetData?) : WidgetHost {
        override val widgetRecord = MutableStateFlow(record)
        override val widgetArt = WidgetArt.of(files)
        var asked = 0

        override fun widgetIntent(link: AppLink?) =
            Intent(Intent.ACTION_VIEW).apply { link?.fields?.forEach { (field, value) -> putExtra(field, value) } }

        override fun refreshWidgetRecord() {
            asked++
        }
    }

    @Before
    fun setUp() {
        AppClock.clock = Clock.fixed(NOW, ZONE)
        // The boards' clock is the 24-hour one; Robolectric's phone is on twelve.
        Settings.System.putString(context.contentResolver, Settings.System.TIME_12_24, "24")
        files = Files.createTempDirectory("widgets").toFile()
        // The art the app keeps for the record, from the fixture titles' own.
        val art = WidgetArt.of(files)
        for (id in listOf(Fixtures.HOSHIKUZU, Fixtures.HAGANE, Fixtures.KUMO)) {
            for (url in listOf(Fixtures.banner(id), Fixtures.poster(id))) {
                val bytes = FixtureArt.bytes(url) ?: continue
                art.keep(url, BitmapFactory.decodeByteArray(bytes, 0, bytes.size))
            }
        }
    }

    @After
    fun tearDown() {
        AppClock.clock = Clock.systemDefaultZone()
        files.deleteRecursively()
    }

    // NativeWidgetsAndroid: the two sizes in the picker, and Continue as it is resized.

    @Test
    fun continue_2x2() = shoot("continue-2x2", ContinueWidget { Host(Fixtures.record()) }, SMALL)

    @Test
    fun continue_3x2() = shoot("continue-3x2", ContinueWidget { Host(Fixtures.record()) }, DpSize(252.dp, 176.dp))

    @Test
    fun continue_4x2() = shoot("continue-4x2", ContinueWidget { Host(Fixtures.record()) }, WIDE)

    @Test
    fun continue_4x3() = shoot("continue-4x3", ContinueWidget { Host(Fixtures.record()) }, DpSize(344.dp, 268.dp))

    @Test
    fun airing_2x2() = shoot("airing-2x2", AiringWidget { Host(Fixtures.record()) }, SMALL)

    @Test
    fun airing_4x2() = shoot("airing-4x2", AiringWidget { Host(Fixtures.record()) }, WIDE)

    // NativeWidgetsStates, at 2 × 2.

    @Test
    fun continue_signed_out() = shoot("continue-signed-out", ContinueWidget { Host(Fixtures.record(signedIn = false, continuing = emptyList())) }, SMALL)

    @Test
    fun continue_caught_up() = shoot("continue-caught-up", ContinueWidget { Host(Fixtures.record(continuing = emptyList())) }, SMALL)

    @Test
    fun continue_stale() = shoot("continue-stale", ContinueWidget { Host(Fixtures.record(generatedAt = at(18, 20, 15))) }, SMALL)

    @Test
    fun continue_stale_4x2() = shoot("continue-stale-4x2", ContinueWidget { Host(Fixtures.record(generatedAt = at(18, 20, 15))) }, WIDE)

    @Test
    fun airing_nothing_today() = shoot("airing-nothing-today", AiringWidget { Host(Fixtures.record(today = emptyList())) }, SMALL)

    @Test
    fun airing_stale() = shoot("airing-stale", AiringWidget { Host(Fixtures.record(generatedAt = at(12, 9, 0), today = emptyList(), nextDays = emptyList())) }, SMALL)

    @Test
    fun airing_no_marks() = shoot("airing-no-marks", AiringWidget { Host(Fixtures.record(signedIn = false, continuing = emptyList())) }, SMALL)

    // What is drawn, read back.

    @Test
    fun the_widget_draws_the_record_and_asks_for_one_when_there_is_none() {
        val host = Host(null)
        val widget = ContinueWidget { host }
        assertTrue(texts(widget, SMALL).contains(tv.anistream.i18n.Strings.widgetConnectDetail(RuntimeEnvironment.getApplication())))
        assertEquals("no record: a new one is asked for", 1, host.asked)

        // The record lands: drawn again, it shows it.
        host.widgetRecord.value = Fixtures.record()
        val drawn = texts(widget, SMALL)
        assertTrue(drawn.toString(), drawn.containsAll(listOf("Hoshikuzu no Tabibito", "E7 · 14 min left")))
        assertEquals(1, host.asked)
    }

    @Test
    fun airing_today_asks_for_a_record_once_it_has_none_for_today() {
        val host = Host(Fixtures.record(generatedAt = at(12, 9, 0), today = emptyList(), nextDays = emptyList()))
        assertTrue(texts(AiringWidget { host }, SMALL).contains("No schedule for today yet. Open anistream to update it."))
        assertEquals(1, host.asked)
    }

    @Test
    fun a_stale_record_keeps_its_titles_and_says_when_it_is_from() {
        val drawn = texts(ContinueWidget { Host(Fixtures.record(generatedAt = at(18, 20, 15))) }, SMALL)
        assertTrue(drawn.toString(), drawn.containsAll(listOf("Hoshikuzu no Tabibito", "E7 · 14 min left", "Updated Sunday")))
    }

    @Test
    fun the_wide_airing_today_says_each_state() {
        val drawn = texts(AiringWidget { Host(Fixtures.record()) }, WIDE)
        assertTrue(drawn.toString(), drawn.containsAll(listOf("Tuesday", "17:30", "E9 out", "E9 · Dub", "Kazamachi Shokudo", "E8")))
        // The rows the 4 × 2 has room for: the one out, and the four to come.
        assertEquals(listOf("17:30", "21:00", "22:00", "22:30", "23:45"), drawn.filter { Regex("""\d\d:\d\d""").matches(it) })
    }

    @Test
    fun each_tap_opens_the_app_where_the_board_says() {
        val continuing = drawn(ContinueWidget { Host(Fixtures.record()) }, WIDE)
        // A show opens its title page; its play button continues it.
        tap(continuing.textView("Hagane no Niwashi"))
        assertEquals(AppLink.Title(Fixtures.HAGANE), started())
        tap(continuing.described("Resume episode 7 of Hoshikuzu no Tabibito"))
        assertEquals(AppLink.Continue(Fixtures.HOSHIKUZU), started())

        // Signed out, Connect opens Settings › Accounts.
        tap(drawn(ContinueWidget { Host(Fixtures.record(signedIn = false, continuing = emptyList())) }, SMALL).textView("Connect"))
        assertEquals(AppLink.Accounts, started())

        // A row opens its title; the header opens the Calendar.
        val airing = drawn(AiringWidget { Host(Fixtures.record()) }, WIDE)
        tap(airing.textView("Kazamachi Shokudo"))
        assertEquals(AppLink.Title(910006u), started())
        tap(airing.textView("Airing today"))
        assertEquals(AppLink.Calendar, started())
    }

    /** The link of the activity a tap started last, from its intent's extras. */
    private fun started(): AppLink? =
        shadowOf(context as Application).nextStartedActivity?.let { intent -> AppLink.of(intent::getStringExtra) }

    /** Click [view] where the launcher would: the nearest view that takes the click. */
    private fun tap(view: View) {
        generateSequence(view) { it.parent as? View }.first { it.hasOnClickListeners() }.performClick()
    }

    private fun View.all(): Sequence<View> = sequence {
        yield(this@all)
        if (this@all is ViewGroup) for (i in 0 until childCount) yieldAll(getChildAt(i).all())
    }

    private fun View.textView(text: String): View = all().first { it is TextView && it.text.toString() == text }

    private fun View.described(label: String): View = all().first { it.contentDescription?.toString() == label }

    @Test
    fun the_pickers_previews_compose_and_end() = runBlocking {
        // A preview's composition waits for every effect in it to end: one that followed the
        // record never ended, and the previews were never published.
        val host = Host(Fixtures.record())
        withTimeout(20_000) {
            ContinueWidget { host }.composeForPreview(context, AppWidgetProviderInfo.WIDGET_CATEGORY_HOME_SCREEN)
            AiringWidget { host }.composeForPreview(context, AppWidgetProviderInfo.WIDGET_CATEGORY_HOME_SCREEN)
        }
        Unit
    }

    @OptIn(ExperimentalGlanceApi::class)
    private fun drawn(widget: GlanceAppWidget, size: DpSize): FrameLayout {
        val views = runBlocking { widget.compose(context, size = size) }
        val density = context.resources.displayMetrics.density
        val (w, h) = (size.width.value * density).roundToInt() to (size.height.value * density).roundToInt()
        val frame = FrameLayout(context)
        frame.addView(views.apply(context, frame), FrameLayout.LayoutParams(w, h))
        frame.measure(View.MeasureSpec.makeMeasureSpec(w, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(h, View.MeasureSpec.EXACTLY))
        frame.layout(0, 0, w, h)
        return frame
    }

    /** Every text the widget draws at [size], in order. */
    private fun texts(widget: GlanceAppWidget, size: DpSize): List<String> {
        val found = mutableListOf<String>()
        fun walk(view: View) {
            if (view is TextView && view.visibility == View.VISIBLE) found += view.text.toString()
            if (view is ViewGroup) for (i in 0 until view.childCount) walk(view.getChildAt(i))
        }
        walk(drawn(widget, size))
        return found
    }

    /** The widget at [size] on the boards' dark band, 16 dp around it. */
    private fun shoot(name: String, widget: GlanceAppWidget, size: DpSize) {
        val frame = drawn(widget, size)
        val margin = (16 * context.resources.displayMetrics.density).roundToInt()
        val image = Bitmap.createBitmap(frame.width + margin * 2, frame.height + margin * 2, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(image)
        canvas.drawColor(0xFF0D1020.toInt())
        canvas.translate(margin.toFloat(), margin.toFloat())
        // The launcher clips a widget to the system's corners; drawn here in software, which
        // clips no outline, the clip is the shot's.
        val radius = context.resources.getDimension(android.R.dimen.system_app_widget_background_radius)
        canvas.clipPath(Path().apply { addRoundRect(RectF(0f, 0f, frame.width.toFloat(), frame.height.toFloat()), radius, radius, Path.Direction.CW) })
        frame.draw(canvas)
        image.captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/widget-$name.png")
    }

    private companion object {
        /** The boards' 2 × 2 and 4 × 2, on a 412 dp phone. */
        val SMALL = DpSize(160.dp, 176.dp)
        val WIDE = DpSize(344.dp, 176.dp)
    }
}
