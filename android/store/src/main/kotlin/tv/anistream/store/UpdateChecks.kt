// What the update checks a downloaded APK with, where it keeps the file, and how it fetches it.

package tv.anistream.store

import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlin.concurrent.thread
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/** The SHA-256 a release gives its APK, checked against the file. */
object ApkChecksum {
    private val hex = Regex("[0-9a-f]{64}")

    /** [file]'s SHA-256, in lower-case hex. */
    fun of(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(BUFFER)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    /**
     * The digest [text] gives: the whole of it, or the first word of a `.sha256` file as
     * `sha256sum` writes it, `<hex>  <name>`, in either case. `null` when it is no SHA-256.
     */
    fun expected(text: String): String? =
        text.trim().split(Regex("\\s+")).firstOrNull()?.lowercase()?.takeIf(hex::matches)

    /** Whether [file] is what [sha256] says it is. A digest that does not read is never a match. */
    fun matches(file: File, sha256: String): Boolean {
        val expected = expected(sha256) ?: return false
        return MessageDigest.isEqual(expected.toByteArray(), of(file).toByteArray())
    }

    private const val BUFFER = 64 * 1024
}

/**
 * What Android reads of an APK, or of the app as installed: its package, its version code, the
 * SHA-256 of each certificate it is signed with now, and of each in its key's rotation history,
 * oldest first, the current one last.
 */
data class ApkFacts(
    val packageName: String,
    val versionCode: Long,
    val signers: Set<String>,
    val lineage: Set<String> = signers,
)

/** Whether a downloaded APK may replace the installed app: the same app, the same key, a newer version. */
object ApkIdentity {
    /** The app flavour's package, the only one that updates itself. */
    const val PACKAGE = "tv.anistream.app"

    /**
     * Why [apk] may not replace [installed], or `null` when it may: it is an APK, both are
     * [PACKAGE], it is signed with the installed app's certificates (or with a key whose rotation
     * history holds them, as Android takes a rotated key), and its version code is higher.
     */
    fun check(apk: ApkFacts?, installed: ApkFacts, expectedPackage: String = PACKAGE): UpdateFailure? {
        apk ?: return UpdateFailure.NotAnApp
        if (apk.packageName != expectedPackage || installed.packageName != expectedPackage) {
            return UpdateFailure.Package(apk.packageName)
        }
        val signers = installed.signers.map(::fingerprint).toSet()
        val current = apk.signers.map(::fingerprint).toSet()
        val lineage = apk.lineage.map(::fingerprint).toSet()
        val same = signers == current
        val rotated = current.size == 1 && signers.size == 1 && lineage.containsAll(current) && lineage.containsAll(signers)
        if (signers.isEmpty() || !(same || rotated)) return UpdateFailure.Signer
        if (apk.versionCode <= installed.versionCode) return UpdateFailure.NotNewer
        return null
    }

    /** A certificate's SHA-256 in one spelling: upper-case hex in pairs, `4D:66:4A:…`. */
    fun fingerprint(digest: String): String =
        digest.filter { it.isLetterOrDigit() }.uppercase().chunked(2).joinToString(":")

    /** The SHA-256 of [certificate], an X.509 certificate's DER bytes, as [fingerprint] spells it. */
    fun fingerprint(certificate: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(certificate).joinToString(":") { "%02X".format(it) }
}

/**
 * Where a downloaded APK is kept: a folder of the app's own files, emptied after an install, a
 * failure, a cancel and every start. Nothing else is kept in it.
 */
class UpdateFiles(val dir: File) {
    /** The APK of [offer], once it is down whole; while it downloads it is beside it, `.part`. */
    fun apk(offer: UpdateOffer): File = dir.resolve("anistream-${offer.version.filter { it.isLetterOrDigit() || it == '.' || it == '-' }}.apk")

    /** Delete everything here. */
    fun clear() {
        dir.listFiles()?.forEach { it.deleteRecursively() }
    }

    /** Delete everything here but [apk], and make the folder if it is not there. */
    fun keepOnly(apk: File) {
        dir.mkdirs()
        dir.listFiles()?.filter { it != apk }?.forEach { it.deleteRecursively() }
    }
}

/** Fetches an APK. */
fun interface ApkFetch {
    /**
     * Download [url] into [into], whole or not at all: written beside it and moved into place once
     * it is all there, saying how far it is as it goes, [expected] bytes where that is known.
     * Throws [IOException] for anything short of the whole file.
     */
    suspend fun fetch(url: String, into: File, expected: Long, progress: (done: Long, total: Long) -> Unit)
}

/**
 * The APK over HTTPS, as the release's asset is: redirects followed, the size checked against
 * what the release and the server say, and at most [limit] bytes. [plainHttp] lets an update
 * test's build fetch from this Mac.
 */
class HttpApkFetch(
    private val plainHttp: Boolean = false,
    private val limit: Long = 512L * 1024 * 1024,
    private val timeoutMillis: Int = 30_000,
) : ApkFetch {
    override suspend fun fetch(url: String, into: File, expected: Long, progress: (done: Long, total: Long) -> Unit): Unit = coroutineScope {
        val address = runCatching { URL(url) }.getOrElse { throw IOException("not a link: $url") }
        if (address.protocol != "https" && !(plainHttp && address.protocol == "http")) {
            throw IOException("the update is fetched over HTTPS only")
        }
        val part = File(into.path + ".part")
        val connection = address.openConnection() as HttpURLConnection
        // A cancel closes the connection, which on Android ends a read that waits on the network;
        // the JDK's waits for its read timeout. The close goes on a thread of its own, since the
        // JDK's waits for the read to let go of the stream, and the cancel waits for nothing.
        var finished = false
        val closing = launch(start = CoroutineStart.UNDISPATCHED) {
            try {
                awaitCancellation()
            } finally {
                if (!finished) thread(isDaemon = true, name = "update-close") { connection.disconnect() }
            }
        }
        try {
            connection.instanceFollowRedirects = true
            connection.connectTimeout = timeoutMillis
            connection.readTimeout = timeoutMillis
            connection.setRequestProperty("Accept", "application/octet-stream")
            connection.setRequestProperty("User-Agent", "anistream-android")
            val status = connection.responseCode
            if (status !in 200..299) throw IOException("the server answered $status")
            val length = connection.contentLengthLong
            if (expected > 0 && length >= 0 && length != expected) {
                throw IOException("the server's file is $length bytes, the release's $expected")
            }
            val total = if (expected > 0) expected else length.coerceAtLeast(0)
            if (total > limit) throw IOException("the file is $total bytes")
            into.parentFile?.mkdirs()
            var done = 0L
            connection.inputStream.use { input ->
                part.outputStream().use { output ->
                    val buffer = ByteArray(BUFFER)
                    progress(0, total)
                    while (true) {
                        currentCoroutineContext().ensureActive()
                        val read = input.read(buffer)
                        if (read < 0) break
                        done += read
                        if (done > limit || (total > 0 && done > total)) throw IOException("the file is longer than ${if (total > 0) total else limit} bytes")
                        output.write(buffer, 0, read)
                        progress(done, total)
                    }
                    output.fd.sync()
                }
            }
            if (total > 0 && done != total) throw IOException("the download stopped at $done of $total bytes")
            if (!part.renameTo(into)) throw IOException("the download could not be kept")
        } catch (e: IOException) {
            part.delete()
            currentCoroutineContext().ensureActive()
            throw e
        } catch (e: Throwable) {
            part.delete()
            throw e
        } finally {
            val active = currentCoroutineContext().isActive
            finished = active
            closing.cancel()
            if (active) connection.disconnect()
        }
    }

    private companion object {
        const val BUFFER = 64 * 1024
    }
}
