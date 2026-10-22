package tv.anistream.app

import android.app.Application
import android.content.Context
import androidx.core.content.pm.ShortcutManagerCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.xmlpull.v1.XmlPullParser
import tv.anistream.engine.model.ContentFor
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.Event
import tv.anistream.store.AppStore
import tv.anistream.ui.AppLink

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = Application::class)
class ShortcutsTest {
    private val context: Context = RuntimeEnvironment.getApplication()

    /** One `<shortcut>` of res/xml/shortcuts.xml, as its attributes and its intent's extras read. */
    private data class Static(
        val id: String,
        val action: String?,
        val data: String?,
        val targetPackage: String?,
        val targetClass: String?,
        val icon: Int,
        val extras: MutableMap<String, String> = mutableMapOf(),
    ) {
        val link: AppLink? get() = if (action == ACTION_OPEN) AppLink.of { extras[linkExtra(it)] } else null
    }

    private fun statics(): List<Static> {
        val parser = context.resources.getXml(R.xml.shortcuts)
        val found = mutableListOf<Static>()
        var id: String? = null
        var icon = 0
        while (parser.next() != XmlPullParser.END_DOCUMENT) {
            if (parser.eventType != XmlPullParser.START_TAG) continue
            when (parser.name) {
                "shortcut" -> {
                    id = parser.getAttributeValue(ANDROID, "shortcutId")
                    icon = parser.getAttributeResourceValue(ANDROID, "icon", 0)
                }
                "extra" -> found.last().extras[parser.getAttributeValue(ANDROID, "name")] = parser.getAttributeValue(ANDROID, "value")
                "intent" -> found += Static(
                    id = requireNotNull(id),
                    action = parser.getAttributeValue(ANDROID, "action"),
                    data = parser.getAttributeValue(ANDROID, "data"),
                    targetPackage = parser.getAttributeValue(ANDROID, "targetPackage"),
                    targetClass = parser.getAttributeValue(ANDROID, "targetClass"),
                    icon = icon,
                )
            }
        }
        return found
    }

    @Test
    fun the_static_shortcuts_open_search_and_downloads_in_this_flavour() {
        val statics = statics()
        assertEquals(listOf("search", "downloads"), statics.map { it.id })
        assertEquals(listOf(AppLink.Search, AppLink.Downloads), statics.map { it.link })
        for (shortcut in statics) {
            // Extras, not a URL.
            assertEquals(null, shortcut.data)
            // The flavour's own package, so the fixtures' shortcuts never open the app's.
            assertEquals(context.packageName, shortcut.targetPackage)
            assertEquals(MainActivity::class.java.name, shortcut.targetClass)
            assertNotEquals(0, shortcut.icon)
            // The id the activity reports as used when the shortcut's link opens it.
            assertEquals(shortcut.id, ContinueShortcut.shortcutOf(shortcut.link!!))
        }
    }

    @Test
    fun the_continue_shortcut_follows_the_queue_and_waits_while_it_is_not_known() = runTest {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        val said = mutableListOf<ContinueShortcut.Target?>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) {
            ContinueShortcut.targets(store).collect { said += it }
        }
        // Not asked for yet: the shortcut already published stands.
        assertEquals(emptyList<ContinueShortcut.Target?>(), said)

        // The first title's next episode is not out; the second's is.
        store.apply(listOf(continueList(entry(900001u, "Kumo", nextEpisode = 1u, nextAiringIn = 3_600), entry(900002u, "Natsukaze"))))
        assertEquals(listOf(ContinueShortcut.Target(900002u, "Natsukaze", 1u)), said)

        // A progress change that leaves the same title and episode first publishes nothing again.
        store.apply(listOf(continueList(entry(900001u, "Kumo", nextEpisode = 1u, nextAiringIn = 1_800), entry(900002u, "Natsukaze"))))
        assertEquals(1, said.size)

        store.apply(listOf(Event.Content(ContentFor.Continue, ContentList.Empty)))
        assertEquals(listOf(ContinueShortcut.Target(900002u, "Natsukaze", 1u), null), said)
    }

    @Test
    fun the_continue_shortcut_says_continue_and_its_show_and_episode_and_plays_through_the_app() {
        val shortcut = ContinueShortcut(context)
        shortcut.publish(ContinueShortcut.Target(900001u, "Hoshikuzu no Tabibito", 7u))
        val first = ShortcutManagerCompat.getDynamicShortcuts(context).single()
        assertEquals("continue", first.id)
        assertEquals("Continue", first.shortLabel)
        assertEquals("Hoshikuzu no Tabibito E7", first.longLabel)
        assertEquals(0, first.rank)
        assertEquals(AppLink.Continue(900001u), first.intent.appLink())
        assertEquals(MainActivity::class.java.name, first.intent.component?.className)
        assertEquals(null, first.intent.data)

        // One Continue, which follows the queue.
        shortcut.publish(ContinueShortcut.Target(900002u, "Natsukaze", 8u))
        val now = ShortcutManagerCompat.getDynamicShortcuts(context).single()
        assertEquals("Natsukaze E8", now.longLabel)
        assertEquals(AppLink.Continue(900002u), now.intent.appLink())

        // Nothing to continue: off the launcher's list.
        shortcut.publish(null)
        assertTrue(ShortcutManagerCompat.getDynamicShortcuts(context).isEmpty())
        shortcut.publish(ContinueShortcut.Target(900002u, "Natsukaze", 8u))
        assertEquals(listOf("continue"), ShortcutManagerCompat.getDynamicShortcuts(context).map { it.id })
    }

    @Test
    fun the_long_label_stays_under_25_characters() {
        assertEquals("Hoshikuzu no Tabibito E7", ContinueShortcut.longLabel("Hoshikuzu no Tabibito", 7u))
        val long = ContinueShortcut.longLabel("The Library Above the Clouds", 12u)
        assertEquals("The Library Above t… E12", long)
        assertTrue(long.length < 25)
    }

    private fun continueList(vararg entries: Entry) = Event.Content(ContentFor.Continue, ContentList.Entries(entries.toList()))

    private fun entry(id: UInt, title: String, nextEpisode: UInt? = null, nextAiringIn: Long? = null) = Entry(
        id = id, title = title, secondary = null, format = "TV", episodes = 12u, year = 2026u, score = null,
        studio = null, synopsis = "", coverUrl = null, bannerUrl = null,
        genres = emptyList(), availableOn = emptyList(), progress = null, airingIn = null,
        airingEpisode = null, nextEpisode = nextEpisode, nextAiringIn = nextAiringIn, lastAired = null, resume = null,
        related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = title, titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
    )

    private companion object {
        const val ANDROID = "http://schemas.android.com/apk/res/android"
    }
}
