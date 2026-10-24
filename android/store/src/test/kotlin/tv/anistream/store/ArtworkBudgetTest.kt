package tv.anistream.store

import android.graphics.Bitmap
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The art kept: no more than the budget, as a TV sets it, and nothing once the cache is cleared. */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class ArtworkBudgetTest {
    private val dispatcher = UnconfinedTestDispatcher()
    private val asked = mutableListOf<String>()

    /** A store whose every decode is a 100 px square, 40,000 bytes, with room for two. */
    private fun store() = ArtworkStore(
        TestScope(dispatcher),
        decoder = { _, size -> Artwork(Bitmap.createBitmap(100, 100, Bitmap.Config.ARGB_8888), size, 100) },
        costLimit = 2 * SQUARE + 1,
        now = { 0L },
        decoding = dispatcher,
        main = dispatcher,
    ) { url -> asked += url; true }

    private fun ArtworkStore.land(url: String) {
        request(url, 100)
        ready(url, ByteArray(0))
    }

    @Test
    fun keeps_no_more_than_its_budget_dropping_the_oldest() {
        val store = store()
        store.land(A)
        store.land(B)
        store.land(C)
        assertNull(store.image(A))
        assertNotNull(store.image(B))
        assertNotNull(store.image(C))
        assertEquals(2 * SQUARE, store.held.value)
    }

    @Test
    fun clear_drops_every_decode_and_shown_art_is_asked_for_again() {
        val store = store()
        store.land(A)
        val arrivals = store.arrivals(A)
        val before = arrivals.value
        store.clear()
        assertNull(store.image(A))
        assertEquals(0L, store.held.value)
        assertEquals(before + 1, arrivals.value)
        asked.clear()
        store.request(A, 100)
        assertEquals(listOf(A), asked)
    }

    private companion object {
        const val SQUARE = 100L * 100 * 4
        const val A = "https://img.example/cover/1.jpg"
        const val B = "https://img.example/cover/2.jpg"
        const val C = "https://img.example/cover/3.jpg"
    }
}
