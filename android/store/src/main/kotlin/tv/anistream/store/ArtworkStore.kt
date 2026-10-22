// Covers and banners: asked of the engine once per URL, decoded off the main thread, and kept
// decoded in memory, the largest decode per URL, up to a budget.

package tv.anistream.store

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** A decoded cover or banner, and what it was decoded from. */
class Artwork(
    val image: Bitmap,
    /** The long side the file was decoded for at most: the largest any screen had asked for. */
    val decodedFor: Int,
    /** The long side of the image in the file. */
    val sourceLongSide: Int,
) {
    val longSide: Int get() = maxOf(image.width, image.height)

    /** Whether this decode is enough for [size] pixels: decoded for that many, or all there is. */
    fun covers(size: Int): Boolean = decodedFor >= size || longSide >= sourceLongSide

    /** What it costs to keep: its pixels. */
    val cost: Int get() = image.allocationByteCount
}

/** Turns a file's bytes into art at most [maxPixelSize] on its long side; `null` when it is not an image. */
fun interface ArtworkDecoder {
    fun decode(bytes: ByteArray, maxPixelSize: Int): Artwork?
}

/** Android's decoder, downsampling by powers of two while the file stays larger than asked for. */
object BitmapDecoder : ArtworkDecoder {
    override fun decode(bytes: ByteArray, maxPixelSize: Int): Artwork? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        val longSide = maxOf(bounds.outWidth, bounds.outHeight)
        if (longSide <= 0) return null
        var sample = 1
        while (longSide / (sample * 2) >= maxPixelSize.coerceAtLeast(1)) sample *= 2
        val options = BitmapFactory.Options().apply { inSampleSize = sample }
        val image = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options) ?: return null
        image.prepareToDraw()
        return Artwork(image, maxPixelSize, longSide)
    }
}

/**
 * The artwork the screens show.
 *
 * A screen asks with [request] when art comes on screen and reads it with [image]. The engine
 * answers with the file's bytes (`ArtworkReady`), which are decoded to the largest size any
 * screen has asked for, off the main thread, and kept, one decode per URL.
 *
 * A URL is asked of the engine once while it is on its way, and not again while its decode is
 * enough. A larger ask asks again, and the smaller decode shows until the larger lands. A URL that
 * failed is asked for again only after [RETRY_AFTER_MILLIS]; its plate stays meanwhile.
 *
 * @param fetch asks the engine for a URL (`FetchArtwork`); false when there is no engine to ask
 *   yet, so a later ask tries again.
 */
class ArtworkStore(
    private val scope: CoroutineScope,
    private val decoder: ArtworkDecoder = BitmapDecoder,
    private val costLimit: Long = 96L shl 20,
    private val now: () -> Long = { System.nanoTime() / 1_000_000 },
    private val decoding: CoroutineDispatcher = Dispatchers.Default,
    private val main: CoroutineDispatcher = Dispatchers.Main,
    private val fetch: (String) -> Boolean,
) {
    /**
     * Each URL's arrivals, bumped each time its art lands: what the screen showing it observes,
     * and only that screen's image, so a cover landing never redraws every other one on screen.
     * Read and written on the main thread.
     */
    private val arrivals = mutableMapOf<String, MutableStateFlow<Int>>()

    private val _landed = MutableStateFlow(0)

    /** How many times art has landed in all, for a test waiting until it stops. */
    val landed: StateFlow<Int> = _landed.asStateFlow()

    /** Bumped each time [url]'s art lands, which is what a screen showing it observes. */
    fun arrivals(url: String): StateFlow<Int> = arrivals.getOrPut(url) { MutableStateFlow(0) }

    private val cache = object : LinkedHashMap<String, Artwork>(64, 0.75f, true) {}
    private var cost = 0L
    private val inFlight = mutableSetOf<String>()
    private val sizes = mutableMapOf<String, Int>()
    private val failedAt = mutableMapOf<String, Long>()

    /** The art for [url], when it is here. */
    fun image(url: String): Artwork? = cache[url]

    /** Ask for [url]'s art, decoded to at most [maxPixelSize] pixels on its long side. */
    fun request(url: String, maxPixelSize: Int) {
        sizes[url] = maxOf(sizes[url] ?: 0, maxPixelSize)
        if (cache[url]?.covers(maxPixelSize) == true) return
        ask(url)
    }

    /** Ask the engine for [url], unless it is on its way or failed a moment ago. */
    private fun ask(url: String) {
        if (url in inFlight) return
        failedAt[url]?.let { if (now() - it < RETRY_AFTER_MILLIS) return }
        if (fetch(url)) inFlight += url
    }

    /** Whether [url] has been asked for and not answered. */
    fun isPending(url: String): Boolean = url in inFlight

    /** Whether any art has been asked for and not answered. */
    val hasPending: Boolean get() = inFlight.isNotEmpty()

    internal fun ready(url: String, bytes: ByteArray) {
        val size = sizes[url] ?: 1024
        scope.launch(decoding) {
            val decoded = runCatching { decoder.decode(bytes, size) }.getOrNull()
            withContext(main) { decoded(url, decoded) }
        }
    }

    internal fun failed(url: String) {
        inFlight -= url
        failedAt[url] = now()
    }

    private fun decoded(url: String, decoded: Artwork?) {
        inFlight -= url
        val cached = cache[url]
        if (decoded == null) {
            if (cached == null) failedAt[url] = now()
            return
        }
        failedAt -= url
        // The largest decode is the one kept: a smaller one answering late changes nothing.
        if (cached == null || cached.longSide < decoded.longSide) {
            keep(url, decoded)
            arrivals[url]?.let { it.value += 1 }
            _landed.value += 1
        }
        // A screen asked for more while these bytes were decoding.
        val wanted = sizes[url]
        if (wanted != null && cache[url]?.covers(wanted) != true) ask(url)
    }

    private fun keep(url: String, art: Artwork) {
        cache.remove(url)?.let { cost -= it.cost }
        cache[url] = art
        cost += art.cost
        val oldest = cache.entries.iterator()
        while (cost > costLimit && oldest.hasNext()) {
            val entry = oldest.next()
            if (entry.key == url) continue
            cost -= entry.value.cost
            oldest.remove()
        }
    }

    companion object {
        /** How long a URL that failed waits before it is asked for again. */
        const val RETRY_AFTER_MILLIS = 60_000L
    }
}
