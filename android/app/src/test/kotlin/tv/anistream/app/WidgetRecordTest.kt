package tv.anistream.app

import android.app.Application
import java.io.File
import java.nio.file.Files
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import tv.anistream.engine.core.Translation
import tv.anistream.engine.model.WidgetAiring
import tv.anistream.engine.model.WidgetContinue
import tv.anistream.engine.model.WidgetData
import tv.anistream.engine.model.WidgetNextAiring

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], application = Application::class)
class WidgetRecordTest {
    private val root: File = Files.createTempDirectory("widgets").toFile()

    @After
    fun tearDown() {
        root.deleteRecursively()
    }

    private val data = WidgetData(
        version = 2u,
        generatedAt = 1_792_000_000,
        continueWatching = listOf(
            WidgetContinue(
                900001u, "Stardust Wayfarer", episode = 7u, watched = 6u, episodes = 12u, fraction = 0.42,
                coverUrl = "https://img.example/stardust.png", episodeTitle = "A Town Without Night", resume = true,
                positionSecs = 606u, durationSecs = 1440u, minutesLeft = 14u, nextAiring = 1_792_100_000, nextAiringEpisode = 8u,
                bannerUrl = "https://img.example/stardust-banner.png", coverColor = "#1a2a5e",
            ),
            WidgetContinue(900006u, "Summer Wind Relay", episode = 8u),
        ),
        airingToday = listOf(
            WidgetAiring(900002u, "The Library Above the Clouds", episode = 6u, airsAt = 1_792_163_730, onList = true, translation = Translation.SUB),
        ),
        signedIn = true,
        tracker = "anilist",
        nextAiring = WidgetNextAiring(900002u, "The Library Above the Clouds", 6u, 1_792_163_730),
        nextDays = listOf(WidgetAiring(900006u, "Summer Wind Relay", episode = 9u, airsAt = 1_792_250_000, watched = false, translation = Translation.DUB, coverColor = "#e4a050")),
    )

    @Test
    fun a_record_written_reads_back_whole_in_a_process_of_its_own() {
        val record = WidgetRecord.of(root)
        assertNull(record.latest.value)
        record.write(data)
        assertEquals(data, record.latest.value)
        // A widget's process, started after the app's was gone, reads it from the file.
        assertEquals(data, WidgetRecord.of(root).latest.value)
        assertEquals(listOf("record.json"), root.resolve("widgets").list()!!.toList())
    }

    @Test
    fun a_record_of_another_shape_reads_as_none() {
        val record = WidgetRecord.of(root)
        record.write(data.copy(version = 3u))
        assertNull(WidgetRecord.of(root).latest.value)
        root.resolve("widgets/record.json").writeText("not a record")
        assertNull(WidgetRecord.of(root).read())
    }
}
