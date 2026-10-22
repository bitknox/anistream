package tv.anistream.app

import android.app.Application
import android.graphics.Bitmap
import java.io.File
import java.nio.file.Files
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.engine.model.WidgetContinue
import tv.anistream.engine.model.WidgetData
import tv.anistream.widgets.WidgetArt

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = Application::class)
class WidgetKeeperTest {
    private val root: File = Files.createTempDirectory("widgets").toFile()

    @After
    fun tearDown() {
        root.deleteRecursively()
    }

    private fun title(id: UInt, banner: Boolean) = WidgetContinue(
        id, "Title $id", episode = 1u,
        coverUrl = "https://img.example/cover/$id.jpg",
        bannerUrl = if (banner) "https://img.example/banner/$id.jpg" else null,
    )

    @Test
    fun the_art_kept_is_each_drawn_titles_wide_art_and_poster() {
        val data = WidgetData(
            version = 2u, generatedAt = 0,
            continueWatching = listOf(title(1u, banner = true), title(2u, banner = false), title(3u, banner = true), title(4u, banner = true)),
        )
        assertEquals(
            listOf(
                "https://img.example/banner/1.jpg", "https://img.example/cover/1.jpg",
                // No banner: the poster is its wide art too, kept once.
                "https://img.example/cover/2.jpg",
                "https://img.example/banner/3.jpg", "https://img.example/cover/3.jpg",
            ),
            WidgetKeeper.artOf(data),
        )
    }

    @Test
    fun art_no_record_names_is_let_go() {
        val art = WidgetArt.of(root)
        val image = Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888)
        art.keep("https://img.example/cover/1.jpg", image)
        art.keep("https://img.example/cover/2.jpg", image)
        art.keepOnly(setOf("https://img.example/cover/2.jpg"))
        assertFalse(art.has("https://img.example/cover/1.jpg"))
        assertTrue(art.has("https://img.example/cover/2.jpg"))
    }
}
