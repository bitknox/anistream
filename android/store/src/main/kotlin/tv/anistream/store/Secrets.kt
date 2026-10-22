// Where the engine keeps the trackers' and plugin stores' tokens, behind the app's host: a private
// file of values sealed with an AES-GCM key that the Android Keystore holds and never lets out.

package tv.anistream.store

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Where secrets are kept, by id: the keystore-sealed file in the app, memory in a test. */
interface SecretBackend {
    /** The secret kept under [id], or `null` when there is none or it could not be read. */
    fun secret(id: String): String?

    /** Keep [value] under [id] in place of whatever was there. False when it was refused. */
    fun setSecret(value: String, id: String): Boolean

    /** Forget what is kept under [id]. True when nothing is kept there now, whether or not anything was. */
    fun deleteSecret(id: String): Boolean
}

/** Secrets in memory, for tests: never the keystore. */
class MemorySecrets(items: Map<String, String> = emptyMap()) : SecretBackend {
    private val items = items.toMutableMap()

    /** Everything kept, for a test to look at. */
    val all: Map<String, String> @Synchronized get() = items.toMap()

    @Synchronized override fun secret(id: String): String? = items[id]

    @Synchronized override fun setSecret(value: String, id: String): Boolean {
        items[id] = value
        return true
    }

    @Synchronized override fun deleteSecret(id: String): Boolean {
        items.remove(id)
        return true
    }
}

/**
 * Secrets in [file], each value sealed with AES-GCM under [key], with its id as the associated
 * data, so a value moved to another id does not open. A line per secret, the id and the sealed
 * value (IV first) each in Base64; the file is replaced whole on every change, through a
 * temporary file beside it.
 *
 * A value that does not open, because the key is gone (the app's data restored onto another
 * device, say), reads as no secret: the account then signs in again.
 *
 * @param key the key, asked for the first time it is needed: the keystore's in the app, a plain
 *   AES key in a test.
 */
class SealedFileSecrets(private val file: File, key: () -> SecretKey) : SecretBackend {
    private val key by lazy(key)
    private var sealed: MutableMap<String, ByteArray>? = null

    private fun items(): MutableMap<String, ByteArray> = sealed ?: read().also { sealed = it }

    private fun read(): MutableMap<String, ByteArray> {
        if (!file.exists()) return mutableMapOf()
        val lines = runCatching { file.readLines() }.getOrDefault(emptyList())
        if (lines.firstOrNull() != HEADER) return mutableMapOf()
        val decoder = Base64.getDecoder()
        return lines.drop(1).mapNotNull { line ->
            val parts = line.split(' ')
            if (parts.size != 2) return@mapNotNull null
            runCatching { String(decoder.decode(parts[0])) to decoder.decode(parts[1]) }.getOrNull()
        }.toMap(mutableMapOf())
    }

    private fun write(items: Map<String, ByteArray>): Boolean = runCatching {
        val encoder = Base64.getEncoder()
        val text = buildString {
            appendLine(HEADER)
            for ((id, value) in items) {
                append(encoder.encodeToString(id.toByteArray())).append(' ')
                appendLine(encoder.encodeToString(value))
            }
        }
        file.parentFile?.mkdirs()
        val temporary = File(file.path + ".partial")
        temporary.writeText(text)
        check(temporary.renameTo(file)) { "the secrets file could not be replaced" }
    }.isSuccess

    @Synchronized override fun secret(id: String): String? {
        val value = items()[id] ?: return null
        return runCatching {
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(TAG_BITS, value, 0, IV_BYTES))
            cipher.updateAAD(id.toByteArray())
            String(cipher.doFinal(value, IV_BYTES, value.size - IV_BYTES))
        }.getOrNull()
    }

    @Synchronized override fun setSecret(value: String, id: String): Boolean {
        val sealedValue = runCatching {
            // The key makes the IV: a keystore key refuses one handed to it.
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.ENCRYPT_MODE, key)
            cipher.updateAAD(id.toByteArray())
            val iv = cipher.iv
            check(iv.size == IV_BYTES) { "an IV of ${iv.size} bytes" }
            iv + cipher.doFinal(value.toByteArray())
        }.getOrNull() ?: return false
        val next = items().toMutableMap().apply { put(id, sealedValue) }
        if (!write(next)) return false
        sealed = next
        return true
    }

    @Synchronized override fun deleteSecret(id: String): Boolean {
        if (id !in items()) return true
        val next = items().toMutableMap().apply { remove(id) }
        if (!write(next)) return false
        sealed = next
        return true
    }

    companion object {
        private const val HEADER = "anistream secrets 1"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val IV_BYTES = 12
        private const val TAG_BITS = 128
    }
}

/** The app's key in the Android Keystore, made the first time it is asked for. */
object KeystoreKey {
    const val ALIAS = "anistream-secrets"

    fun get(alias: String = ALIAS): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(alias, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }
}
