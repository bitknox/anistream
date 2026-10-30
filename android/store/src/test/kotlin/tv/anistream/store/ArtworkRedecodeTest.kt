package tv.anistream.store

import android.graphics.Bitmap
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** A larger ask that decodes to the same pixels as the decode kept is answered once, not forever. */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class ArtworkRedecodeTest {
    private val dispatcher = UnconfinedTestDispatcher()
    private val asked = mutableListOf<String>()

    /** Decodes a 1600 px banner the way BitmapDecoder does: halving while it stays larger than asked. */
    private fun store() = ArtworkStore(
        TestScope(dispatcher),
        decoder = { _, size ->
            var sample = 1
            while (SOURCE / (sample * 2) >= size) sample *= 2
            val side = SOURCE / sample
            Artwork(Bitmap.createBitmap(side, side * 9 / 16, Bitmap.Config.ARGB_8888), size, SOURCE)
        },
        now = { 0L },
        decoding = dispatcher,
        main = dispatcher,
    ) { url -> asked += url; true }

    @Test
    fun a_second_decode_of_the_same_size_does_not_ask_again() {
        val store = store()
        store.request(BANNER, 500)
        store.ready(BANNER, ByteArray(0))
        store.request(BANNER, 720)
        store.ready(BANNER, ByteArray(0))
        // 500 and 720 both decode to 800 px. One ask each; the answer to the second must not ask again.
        assertEquals(listOf(BANNER, BANNER), asked)
    }

    private companion object {
        const val SOURCE = 1600
        const val BANNER = "https://img.example/banner/2.jpg"
    }
}
