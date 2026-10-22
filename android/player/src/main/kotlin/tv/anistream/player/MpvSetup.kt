package tv.anistream.player

import android.content.Context
import android.os.Build
import java.io.File

/**
 * What mpv needs before it starts: fontconfig's configuration and the CA bundle on disk, and the
 * options every mpv of the app's shares.
 */
object MpvSetup {
    /**
     * mpv-android's fonts.conf (Utils.writeFontsConf): fontconfig over the system's fonts, with
     * Android's own families for the generic names, and its cache in the app's cache dir. mpv
     * passes `~~/fonts.conf` to libass, so it goes in mpv's `config-dir`. The names the caption
     * settings' typefaces go by (CaptionStyle.FAMILIES) are there too, as Android's fonts.xml has
     * them.
     */
    fun writeFontsConf(configDir: File, cacheDir: File) {
        val conf = """
            <?xml version="1.0"?>
            <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
            <fontconfig>
              <dir>/system/fonts</dir>
              <dir>/product/fonts</dir>
              <cachedir>${cacheDir.path}/fontconfig</cachedir>
              <alias><family>serif</family><prefer><family>Noto Serif</family></prefer></alias>
              <alias><family>sans-serif</family><prefer><family>Roboto</family><family>Noto Sans</family></prefer></alias>
              <alias><family>monospace</family><prefer><family>Droid Sans Mono</family></prefer></alias>
              <alias><family>sans-serif-condensed</family><prefer><family>Roboto Condensed</family><family>Roboto</family></prefer></alias>
              <alias><family>sans-serif-monospace</family><prefer><family>Droid Sans Mono</family></prefer></alias>
              <alias><family>serif-monospace</family><prefer><family>Cutive Mono</family><family>Droid Sans Mono</family></prefer></alias>
              <alias><family>casual</family><prefer><family>Coming Soon</family></prefer></alias>
              <alias><family>cursive</family><prefer><family>Dancing Script</family></prefer></alias>
              <alias><family>sans-serif-smallcaps</family><prefer><family>Carrois Gothic SC</family></prefer></alias>
            </fontconfig>
        """.trimIndent()
        val file = File(configDir, "fonts.conf")
        if (!file.exists() || file.readText() != conf) file.writeText(conf)
    }

    /**
     * FFmpeg's mbedTLS reads its roots from one PEM file. Rather than ship one, this joins the
     * system's own store (Conscrypt's module since Android 14, so it follows its updates), which
     * holds one PEM certificate per file. Returns the bundle, or null when there is no store.
     */
    fun caBundle(cacheDir: File): File? {
        val store = listOf("/apex/com.android.conscrypt/cacerts", "/system/etc/security/cacerts")
            .map(::File).firstOrNull { it.isDirectory } ?: return null
        val certs = store.listFiles().orEmpty().filter { it.isFile }.sortedBy { it.name }
        val bundle = File(cacheDir, "cacerts.pem")
        val stamp = "${store.path} ${certs.size} ${certs.maxOfOrNull { it.lastModified() }}"
        val stampFile = File(cacheDir, "cacerts.stamp")
        if (bundle.exists() && stampFile.exists() && stampFile.readText() == stamp) return bundle
        bundle.bufferedWriter().use { out ->
            for (cert in certs) {
                val text = cert.readText()
                val begin = text.indexOf("-----BEGIN CERTIFICATE-----")
                val end = text.indexOf("-----END CERTIFICATE-----")
                if (begin >= 0 && end > begin) out.write(text.substring(begin, end + 25) + "\n")
            }
        }
        stampFile.writeText(stamp)
        return bundle
    }

    /**
     * The options both mpv instances share, after Apple's MpvClient where they apply: the
     * android GLES context and MediaCodec in place of MoltenVK and VideoToolbox.
     */
    fun baseOptions(context: Context): List<Pair<String, String>> {
        writeFontsConf(context.filesDir, context.cacheDir)
        val options = mutableListOf(
            "config" to "yes",
            "config-dir" to context.filesDir.path,
            "idle" to "yes",
            "input-default-bindings" to "no",
            "input-vo-keyboard" to "no",
            "embeddedfonts" to "yes",
            "gpu-shader-cache-dir" to context.cacheDir.path,
            "icc-cache-dir" to context.cacheDir.path,
            "ytdl" to "no",
            "save-position-on-quit" to "no",
            // mpv-android's: the defaults (150 and 50 MiB) are a desktop's.
            "demuxer-max-bytes" to "${64 * 1024 * 1024}",
            "demuxer-max-back-bytes" to "${64 * 1024 * 1024}",
        )
        caBundle(context.cacheDir)?.let {
            options += "tls-verify" to "yes"
            options += "tls-ca-file" to it.path
        }
        return options
    }

    /**
     * The player's own options on top of [baseOptions]: GLES `gpu-next` on the android context
     * once a surface is there (`vo` is set then), MediaCodec where it works, and HDR tone-mapped
     * to SDR until a phone has been checked (the plan's call 2).
     */
    fun playerOptions(context: Context, hwdec: String): List<Pair<String, String>> = baseOptions(context) + listOf(
        "vo" to "null",
        "force-window" to "no",
        "gpu-context" to "android",
        "opengl-es" to "yes",
        "hwdec" to hwdec,
        "hwdec-codecs" to "h264,hevc,mpeg4,mpeg2video,vp8,vp9,av1",
        "ao" to "audiotrack,opensles",
        "audio-set-media-role" to "yes",
        "target-colorspace-hint" to "no",
        "keep-open" to "no",
    )

    /**
     * The hardware decoding to start with: MediaCodec, zero-copy then copying, on a device, and
     * none in the emulator, whose MediaCodec stalls under FFmpeg after the first flush (AS5).
     */
    val defaultHwdec: String get() = if (isEmulator) "no" else "mediacodec,mediacodec-copy"

    /** Whether this is the Android Emulator, by what its images say of themselves. */
    val isEmulator: Boolean
        get() = Build.HARDWARE.contains("ranchu") || Build.HARDWARE.contains("goldfish") ||
            Build.PRODUCT.startsWith("sdk_") || Build.FINGERPRINT.contains("emulator") ||
            Build.FINGERPRINT.startsWith("generic")
}
