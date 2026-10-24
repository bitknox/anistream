package tv.anistream.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Shader
import java.io.ByteArrayOutputStream
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.Entry

/**
 * A made-up title long enough to wrap, on a near-white banner: the lightest art a title can have,
 * for shots of the page's head. The Apple apps' `pale` showcase draws the same.
 */
object PaleBanner {
    val ID: AnilistId = 900001u
    const val URL = "https://img.example/banner/pale.png"

    /** [entry] renamed, with the near-white banner. */
    fun retitle(entry: Entry): Entry = entry.copy(
        title = "Kamifusen Toudai no Bannin to Nanatsu no Kaze no Tegami",
        secondary = "The Paper Balloon Lighthouse Keeper and the Letters of the Seven Winds",
        titleRomaji = "Kamifusen Toudai no Bannin to Nanatsu no Kaze no Tegami",
        titleEnglish = "The Paper Balloon Lighthouse Keeper and the Letters of the Seven Winds",
        titleNative = "紙風船灯台の番人と七つの風の手紙",
        bannerUrl = URL,
    )

    /** The banner's PNG for [url], or `null` for any other. */
    fun bytes(url: String): ByteArray? = if (url == URL) png else null

    /** White warming to paper, with a few faint pastel shapes. */
    private val png: ByteArray by lazy {
        val bitmap = Bitmap.createBitmap(960, 360, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val wash = Paint().apply {
            shader = LinearGradient(0f, 0f, 960f, 360f, 0xFFFFFFFF.toInt(), 0xFFF7F5F0.toInt(), Shader.TileMode.CLAMP)
        }
        canvas.drawRect(0f, 0f, 960f, 360f, wash)
        val shape = Paint(Paint.ANTI_ALIAS_FLAG)
        for ((x, y, r, color) in listOf(
            listOf(170, 50, 110, 0xFFFAE6EB.toInt()),
            listOf(770, 190, 130, 0xFFE6F2FC.toInt()),
            listOf(440, 50, 60, 0xFFFCF5DB.toInt()),
        )) {
            shape.color = color
            canvas.drawCircle(x.toFloat(), y.toFloat(), r.toFloat(), shape)
        }
        ByteArrayOutputStream().use { out ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            out.toByteArray()
        }
    }
}
