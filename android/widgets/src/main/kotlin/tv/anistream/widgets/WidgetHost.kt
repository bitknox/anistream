// What the widgets ask of the app they run in: its record of them, its art, and the intents that
// open it. Android's widgets run in the app's own process, so this is the application itself.

package tv.anistream.widgets

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapShader
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Shader
import java.io.File
import java.security.MessageDigest
import kotlinx.coroutines.flow.StateFlow
import tv.anistream.engine.model.WidgetData
import tv.anistream.ui.AppLink

/** The app, as its widgets see it. The application implements it. */
interface WidgetHost {
    /** The record of the widgets the app keeps (`WidgetSnapshot`), as it is written; `null` before the first. */
    val widgetRecord: StateFlow<WidgetData?>

    /** Where the record's art is kept. */
    val widgetArt: WidgetArt

    /** The explicit intent that opens the app on [link], or as the launcher opens it for `null`. */
    fun widgetIntent(link: AppLink?): Intent

    /**
     * Ask for a new record, soon: there is none yet, or none for today. Asking again while one is
     * on its way does nothing.
     */
    fun refreshWidgetRecord()
}

/** The host the app's widgets run in: the application, when it is one. */
fun widgetHost(context: Context): WidgetHost? = context.applicationContext as? WidgetHost

/**
 * The record's art, as files under the app's own: one JPEG per address, written by the app when a
 * record lands (through the engine, as every other picture), and read by the widgets as they draw.
 * The widgets never fetch anything themselves.
 */
class WidgetArt(private val directory: File) {
    /** The file [url]'s art is kept in. */
    fun file(url: String): File = File(directory, key(url) + ".jpg")

    /** Whether [url]'s art is kept. */
    fun has(url: String): Boolean = file(url).isFile

    /** Keep [image] as [url]'s art: written whole and renamed into place, so a reader never sees half. */
    fun keep(url: String, image: Bitmap) {
        directory.mkdirs()
        val target = file(url)
        val next = File(target.path + ".next")
        runCatching {
            next.outputStream().use { image.compress(Bitmap.CompressFormat.JPEG, 88, it) }
            if (!next.renameTo(target)) next.delete()
        }.onFailure { next.delete() }
    }

    /** Remove the art no address in [urls] names, once the record that named it is gone. */
    fun keepOnly(urls: Set<String>) {
        val kept = urls.map { key(it) + ".jpg" }.toSet()
        directory.listFiles()?.filter { it.name !in kept }?.forEach(File::delete)
    }

    /**
     * [url]'s art, cut to fill [width] × [height] pixels from its middle, as a widget draws it;
     * `null` when it is not kept or does not decode. Decoded small: a widget's pictures all share
     * one budget.
     */
    fun image(url: String?, width: Int, height: Int): Bitmap? {
        if (url == null || width <= 0 || height <= 0) return null
        val file = file(url).takeIf(File::isFile) ?: return null
        return runCatching {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(file.path, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
            var sample = 1
            while (bounds.outWidth / (sample * 2) >= width && bounds.outHeight / (sample * 2) >= height) sample *= 2
            val decoded = BitmapFactory.decodeFile(file.path, BitmapFactory.Options().apply { inSampleSize = sample })
                ?: return null
            cover(decoded, width, height)
        }.getOrNull()
    }

    companion object {
        /** Where the app keeps it. */
        fun of(files: File): WidgetArt = WidgetArt(files.resolve("widgets/art"))

        private fun key(url: String): String =
            MessageDigest.getInstance("SHA-256").digest(url.toByteArray()).take(16).joinToString("") { "%02x".format(it) }

        /** [image] with its corners rounded to [radius] pixels, transparent outside them. */
        fun rounded(image: Bitmap, radius: Float): Bitmap {
            if (radius <= 0f) return image
            val out = Bitmap.createBitmap(image.width, image.height, Bitmap.Config.ARGB_8888)
            val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { shader = BitmapShader(image, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP) }
            Canvas(out).drawRoundRect(0f, 0f, image.width.toFloat(), image.height.toFloat(), radius, radius, paint)
            return out
        }

        /** A plate of [color], [width] × [height] with [radius] corners: art's stand-in. */
        fun plate(color: Int, width: Int, height: Int, radius: Float): Bitmap {
            val out = Bitmap.createBitmap(width.coerceAtLeast(1), height.coerceAtLeast(1), Bitmap.Config.ARGB_8888)
            val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = color }
            Canvas(out).drawRoundRect(0f, 0f, out.width.toFloat(), out.height.toFloat(), radius, radius, paint)
            return out
        }

        /** [image] scaled to cover [width] × [height] and cut to it from the middle. */
        internal fun cover(image: Bitmap, width: Int, height: Int): Bitmap {
            val scale = maxOf(width.toFloat() / image.width, height.toFloat() / image.height)
            val w = (width / scale).toInt().coerceIn(1, image.width)
            val h = (height / scale).toInt().coerceIn(1, image.height)
            val cut = Bitmap.createBitmap(image, (image.width - w) / 2, (image.height - h) / 2, w, h)
            return Bitmap.createScaledBitmap(cut, width, height, true)
        }
    }
}
