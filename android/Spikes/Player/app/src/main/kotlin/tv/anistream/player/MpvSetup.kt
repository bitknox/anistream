package tv.anistream.player

import android.content.Context
import java.io.File

/** What mpv needs on disk before it starts: fontconfig's configuration and the CA bundle. */
object MpvSetup {
    /**
     * mpv-android's fonts.conf (Utils.writeFontsConf): fontconfig over the system's fonts, with
     * Android's own families for the generic names, and its cache in the app's cache dir. mpv
     * passes `~~/fonts.conf` to libass, so it goes in mpv's `config-dir`.
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

    /** The properties the engine reads (Apple's HostedOps.reportedProperties). */
    val reportedProperties = listOf(
        "time-pos", "duration", "pause", "speed", "volume", "chapter-list", "video-params",
        "current-tracks/video", "track-list", "aid", "sid", "sub-delay", "paused-for-cache",
        "frame-drop-count", "seeking",
    )
}
