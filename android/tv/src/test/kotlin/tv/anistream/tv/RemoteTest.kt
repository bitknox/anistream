package tv.anistream.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import tv.anistream.engine.ffi.FfiPairedPhone
import tv.anistream.tv.remote.grouped
import tv.anistream.tv.remote.qr
import tv.anistream.tv.screens.megabytes
import tv.anistream.tv.screens.phoneLine

/** What the remote's screens write: the code in groups, the QR code's quiet zone, a phone's line. */
class RemoteTest {
    @Test
    fun a_code_reads_in_two_groups_of_three() {
        assertEquals("042 917", grouped("042917"))
        assertEquals("12345", grouped("12345"))
    }

    @Test
    fun the_qr_code_keeps_a_quiet_zone_of_four() {
        val matrix = qr(TvShots.PAIR_LINK)
        for (i in 0 until matrix.width) {
            for (edge in 0 until 4) {
                assertTrue(!matrix[i, edge] && !matrix[edge, i])
            }
        }
        // A finder pattern's corner just inside the zone.
        assertTrue(matrix[4, 4])
    }

    @Test
    fun a_phone_reads_as_when_it_was_last_used() {
        val now = 1_800_000_000L
        val phone = FfiPairedPhone("ab", "Mika's phone", pairedAt = now - 10 * 86_400, lastSeen = null)
        assertEquals("Paired 10 days ago", phoneLine(phone, now))
        assertEquals("Used today", phoneLine(phone.copy(lastSeen = now - 60), now))
        assertEquals("Used yesterday", phoneLine(phone.copy(lastSeen = now - 86_400), now))
    }

    @Test
    fun the_image_cache_reads_in_whole_megabytes() {
        assertEquals("0 MB", megabytes(0))
        assertEquals("40 MB", megabytes(40L shl 20))
        assertEquals("2 MB", megabytes((3L shl 19)))
    }
}
