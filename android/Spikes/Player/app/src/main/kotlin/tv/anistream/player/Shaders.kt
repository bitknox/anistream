package tv.anistream.player

import android.content.Context
import java.io.File

/**
 * The engine's upscaling chains, rebuilt from `crates/anistream-engine/src/playback/shaders.rs`
 * (`choose()` and `Mode::chain`) rule for rule, so the spike sets the same files in the same
 * order as the engine would. The files are the repo's `assets/shaders`, bundled as app assets
 * and copied to the app's files dir, where mpv can read them.
 */
object Shaders {
    enum class Look { Off, Natural, Sharp }

    enum class Hdr { Hdr10, Hdr10Plus, Hlg, DolbyVision }

    data class Source(val width: Int, val height: Int, val hdr: Hdr?)

    data class Display(val width: Int, val height: Int)

    data class Chain(val files: List<String>, val label: String, val upscaler: String?)

    private const val ANISD = "AniSD_ArtCNN_C4F32_i4.glsl"
    private const val ARTCNN_LIGHT = "ArtCNN_C4F16.glsl"
    private const val KRIG = "KrigBilateral.glsl"
    private const val SSIM = "SSimDownscaler.glsl"
    private val FALLBACK_DISPLAY = Display(1920, 1080)
    private const val CNN_MIN_SCALE = 1.3
    private const val LEVELS = 2

    private val OFF = Chain(emptyList(), "Off", null)

    private enum class Tier { Sd, Hd, Fhd }

    private fun tier(s: Source): Tier {
        val lines = maxOf(s.height, s.width * 9 / 16)
        return when {
            lines < 600 -> Tier.Sd
            lines < 900 -> Tier.Hd
            else -> Tier.Fhd
        }
    }

    /** `picture::classify`, from mpv's `video-params` as JSON fields. */
    fun hdrOf(gamma: String?, colormatrix: String?, dynamic: Boolean): Hdr? = when {
        colormatrix == "dolbyvision" -> Hdr.DolbyVision
        gamma == "hlg" -> Hdr.Hlg
        gamma == "pq" -> if (dynamic) Hdr.Hdr10Plus else Hdr.Hdr10
        else -> null
    }

    fun choose(look: Look, source: Source, display: Display?, onBattery: Boolean = false, eased: Int = 0): Chain {
        if (look == Look.Off) return OFF
        when (source.hdr) {
            Hdr.DolbyVision -> return Chain(emptyList(), "Dolby Vision: none", null)
            null -> {}
            else -> return Chain(listOf(KRIG), "HDR: chroma only", null)
        }
        val d = display?.takeIf { it.width > 0 && it.height > 0 } ?: FALLBACK_DISPLAY
        val scale = if (source.width == 0 || source.height == 0) 1.0 else
            minOf(d.width.toDouble() / source.width, d.height.toDouble() / source.height)
        val level = minOf(eased + if (onBattery) 1 else 0, LEVELS)
        val tier = tier(source)
        val chroma = {
            if (scale < 1.0) Chain(listOf(KRIG, SSIM), "Chroma only", null)
            else Chain(listOf(KRIG), "Chroma only", null)
        }
        if (scale <= CNN_MIN_SCALE) {
            return if (look == Look.Sharp && scale >= 1.0) sharpNative(tier, level) ?: chroma() else chroma()
        }
        return when (look) {
            Look.Off -> OFF
            Look.Natural -> {
                val rungs = when (tier) {
                    Tier.Sd -> listOf(ANISD to "ArtCNN AniSD", ARTCNN_LIGHT to "ArtCNN C4F16")
                    Tier.Hd, Tier.Fhd -> listOf(ARTCNN_LIGHT to "ArtCNN C4F16")
                }
                val (model, label) = rungs.getOrNull(level) ?: return chroma()
                val files = mutableListOf(model, KRIG)
                if (scale < 2.0) files += SSIM
                Chain(files, label, "ArtCNN")
            }
            Look.Sharp -> {
                val (mode, rungs) = when (tier) {
                    Tier.Sd -> 'C' to listOf(true, false)
                    Tier.Hd -> 'B' to listOf(true, false)
                    Tier.Fhd -> 'A' to listOf(false)
                }
                val hq = rungs.getOrNull(level) ?: return chroma()
                Chain(modeChain(mode, hq), "Anime4K $mode (${if (hq) "HQ" else "Fast"})", "Anime4K")
            }
        }
    }

    private fun modeChain(mode: Char, hq: Boolean): List<String> {
        val first = when (mode to hq) {
            ('A' to true) -> listOf("Anime4K_Restore_CNN_VL.glsl", "Anime4K_Upscale_CNN_x2_VL.glsl")
            ('A' to false) -> listOf("Anime4K_Restore_CNN_M.glsl", "Anime4K_Upscale_CNN_x2_M.glsl")
            ('B' to true) -> listOf("Anime4K_Restore_CNN_Soft_VL.glsl", "Anime4K_Upscale_CNN_x2_VL.glsl")
            ('B' to false) -> listOf("Anime4K_Restore_CNN_Soft_M.glsl", "Anime4K_Upscale_CNN_x2_M.glsl")
            ('C' to true) -> listOf("Anime4K_Upscale_Denoise_CNN_x2_VL.glsl")
            else -> listOf("Anime4K_Upscale_Denoise_CNN_x2_M.glsl")
        }
        val last = if (hq) "Anime4K_Upscale_CNN_x2_M.glsl" else "Anime4K_Upscale_CNN_x2_S.glsl"
        return listOf("Anime4K_Clamp_Highlights.glsl") + first +
            listOf("Anime4K_AutoDownscalePre_x2.glsl", "Anime4K_AutoDownscalePre_x4.glsl", last)
    }

    private fun sharpNative(tier: Tier, level: Int): Chain? {
        val (file, label) = when {
            tier == Tier.Fhd && level == 0 -> "Anime4K_Restore_CNN_VL.glsl" to "Anime4K A restore (HQ)"
            tier == Tier.Fhd && level == 1 -> "Anime4K_Restore_CNN_M.glsl" to "Anime4K A restore (Fast)"
            tier == Tier.Hd && level == 0 -> "Anime4K_Restore_CNN_Soft_VL.glsl" to "Anime4K B restore (HQ)"
            tier == Tier.Hd && level == 1 -> "Anime4K_Restore_CNN_Soft_M.glsl" to "Anime4K B restore (Fast)"
            else -> return null
        }
        return Chain(listOf("Anime4K_Clamp_Highlights.glsl", file), label, "Anime4K")
    }

    /** Copies the bundled shaders (assets/shaders) to the files dir once, and returns it. */
    fun install(context: Context): File {
        val dir = File(context.filesDir, "shaders")
        dir.mkdirs()
        val names = context.assets.list("shaders").orEmpty()
        for (name in names) {
            val target = File(dir, name)
            context.assets.open("shaders/$name").use { input ->
                val bytes = input.readBytes()
                if (!target.exists() || target.length() != bytes.size.toLong()) target.writeBytes(bytes)
            }
        }
        return dir
    }

    fun paths(dir: File, chain: Chain): String = chain.files.joinToString(":") { File(dir, it).path }
}
