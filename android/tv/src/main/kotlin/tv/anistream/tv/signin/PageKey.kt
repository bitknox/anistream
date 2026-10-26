// The QR pages' one-time key. The page address carries it after the #, which a browser never
// sends, so it goes from the TV to the phone only through the QR code. The page seals the form
// with it before posting, and the TV fills a form in with it sealed too: nothing typed crosses the
// network in plain text, though the page itself is plain HTTP. AES-256-GCM, the phone's side in
// page-crypto.js (@noble/ciphers), this side the platform's own cipher, with the page's path as
// the additional data so an answer sealed for one page opens on no other.

package tv.anistream.tv.signin

import java.security.GeneralSecurityException
import java.security.SecureRandom
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

class PageKey(private val key: ByteArray = ByteArray(KEY_BYTES).also(random::nextBytes)) {
    init {
        require(key.size == KEY_BYTES)
    }

    /** "#k=" and the key in base64url, for the end of the page's address. */
    val fragment: String get() = "#k=" + encoder.encodeToString(key)

    /** [text] sealed for the page at [path]: a fresh nonce, then the ciphertext and its tag, in base64url. */
    fun seal(path: String, text: String): String {
        val nonce = ByteArray(NONCE_BYTES).also(random::nextBytes)
        val sealed = cipher(Cipher.ENCRYPT_MODE, nonce, path).doFinal(text.toByteArray())
        return encoder.encodeToString(nonce + sealed)
    }

    /** What [seal], or the page, sealed for [path] under this key; null for anything else. */
    fun open(path: String, sealed: String): String? {
        val bytes = runCatching { Base64.getUrlDecoder().decode(sealed.trim()) }.getOrNull() ?: return null
        if (bytes.size < NONCE_BYTES + TAG_BITS / 8) return null
        return try {
            String(cipher(Cipher.DECRYPT_MODE, bytes.copyOfRange(0, NONCE_BYTES), path).doFinal(bytes, NONCE_BYTES, bytes.size - NONCE_BYTES))
        } catch (_: GeneralSecurityException) {
            null
        }
    }

    private fun cipher(mode: Int, nonce: ByteArray, path: String): Cipher = Cipher.getInstance("AES/GCM/NoPadding").apply {
        init(mode, SecretKeySpec(key, "AES"), GCMParameterSpec(TAG_BITS, nonce))
        updateAAD(path.toByteArray())
    }

    companion object {
        private const val KEY_BYTES = 32
        private const val NONCE_BYTES = 12
        private const val TAG_BITS = 128
        private val random = SecureRandom()
        private val encoder = Base64.getUrlEncoder().withoutPadding()

        /** The key in a page address's [fragment], "#k=" and its base64url; null when there is none. */
        fun from(fragment: String): PageKey? {
            val text = fragment.removePrefix("#").removePrefix("k=")
            val bytes = runCatching { Base64.getUrlDecoder().decode(text) }.getOrNull() ?: return null
            return if (bytes.size == KEY_BYTES) PageKey(bytes) else null
        }
    }
}
