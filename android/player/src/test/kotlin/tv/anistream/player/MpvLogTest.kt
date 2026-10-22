// mpv's log lines, as logcat may hold them.

package tv.anistream.player

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class MpvLogTest {
    @Test
    fun anAddressInALineKeepsItsSchemeAndHostAndLosesItsToken() {
        assertEquals(
            "Opening failed or was aborted: https://cdn.example (403)",
            MpvLog.redacting("Opening failed or was aborted: https://cdn.example/a/b.m3u8?token=s3cret&e=1 (403)"),
        )
        assertEquals("tcp: http://127.0.0.1:7878 failed", MpvLog.redacting("tcp: http://127.0.0.1:7878/mend/abc0123/seg-4.ts failed"))
        assertEquals("a from 'https://cdn.example' and b", MpvLog.redacting("a from 'https://user:pw@cdn.example/x' and b"))
        assertEquals("no address here", MpvLog.redacting("no address here"))
        assertFalse("sig=" in MpvLog.redacting("one https://a.example/1?sig=x then https://b.example/2?sig=y"))
    }

    @Test
    fun mpvsLinesAreSaidAtTheirLevelWithoutTheTokensInTheirAddresses() {
        val event = MpvEvent(
            MpvNative.Event.LOG_MESSAGE, "ffmpeg",
            """{"level":"error","text":"https: HTTP error 403 for https://cdn.example/hls/7/seg-12.ts?token=s3cret"}""",
        )
        assertEquals(android.util.Log.ERROR to "[ffmpeg] https: HTTP error 403 for https://cdn.example", MpvLog.said(event))
    }
}
