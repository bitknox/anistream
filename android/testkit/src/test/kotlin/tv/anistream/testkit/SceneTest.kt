package tv.anistream.testkit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.model.ContentFor
import tv.anistream.engine.model.ContentList
import tv.anistream.engine.model.Event

class SceneTest {
    @Test
    fun a_recorded_scene_reads_back_as_it_was_written() {
        for (name in listOf("fresh", "watching")) {
            val scene = Scene.named(name)
            assertTrue("$name has events", scene.events.isNotEmpty())
            assertEquals(scene.toJson(), Scene.parse(scene.toJson()).toJson())
        }
    }

    @Test
    fun the_watching_scene_has_something_under_way() {
        val continuing = Scene.named("watching").events
            .filterIsInstance<Event.Content>()
            .first { it.view == ContentFor.Continue }
            .content as ContentList.Entries
        assertEquals(900001u, continuing.v1.first().id)
        assertNotNull(continuing.v1.first().resume)
    }

    @Test
    fun the_art_is_the_test_kits_by_the_fixtures_own_urls() {
        val png = byteArrayOf(0x89.toByte(), 'P'.code.toByte(), 'N'.code.toByte(), 'G'.code.toByte())
        for (url in listOf("cover/900001-xl.jpg", "banner/900004.jpg", "ep/900001/7.jpg")) {
            val bytes = FixtureArt.bytes(FixtureArt.HOST + url)
            assertNotNull(url, bytes)
            assertTrue(url, bytes!!.take(4) == png.toList())
        }
        assertNull(FixtureArt.bytes(FixtureArt.HOST + "people/voice-ito.jpg"))
        assertNull(FixtureArt.bytes("https://elsewhere.example/cover/900001.jpg"))
        // Recorded scenes name the fixtures' address, never the port the fake listened on.
        assertTrue(Scene.named("watching").toJson().contains(FixtureArt.HOST + "cover/900001"))
        assertTrue(!FixtureArt.SERVED.containsMatchIn(Scene.named("watching").toJson()))
    }
}
