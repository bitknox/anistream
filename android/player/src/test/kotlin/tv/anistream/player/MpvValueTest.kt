package tv.anistream.player

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import tv.anistream.engine.model.PlayerEnd
import tv.anistream.engine.model.PlayerReport

class MpvValueTest {
    @Test
    fun jsonReadsAndWritesAsMpvAndTheEngineDo() {
        val json = """{"type":"sub","id":3,"title":"Signs \"&\" Songs","lang":"eng","selected":false,"time":12.5,"list":[1,-2.5e3,null,"é"]}"""
        val value = MpvValue.parse(json)!!
        assertEquals("sub", value["type"].string)
        assertEquals(3L, value["id"].long)
        assertEquals("Signs \"&\" Songs", value["title"].string)
        assertEquals(false, value["selected"].bool)
        assertEquals(12.5, value["time"].double!!, 0.0)
        assertEquals(listOf(MpvValue.Int64(1), MpvValue.Real(-2500.0), MpvValue.None, MpvValue.Text("é")), value["list"].array)
        assertEquals(MpvValue.None, value["missing"])
        assertEquals("""{"type":"sub","id":3,"title":"Signs \"&\" Songs","lang":"eng","selected":false,"time":12.5,"list":[1,-2500,null,"é"]}""", value.json)
    }

    @Test
    fun whatIsNotOneJsonValueIsNothing() {
        assertNull(MpvValue.parse("[1,2"))
        assertNull(MpvValue.parse("{} {}"))
        assertNull(MpvValue.parse("yes"))
    }

    @Test
    fun mpvsEventsBecomeTheEnginesReports() {
        assertEquals(PlayerReport.Property("time-pos", "12.5"), reportOf(MpvEvent(MpvNative.Event.PROPERTY_CHANGE, "time-pos", "12.5")))
        assertNull(reportOf(MpvEvent(MpvNative.Event.PROPERTY_CHANGE, "estimated-vf-fps", "23.9")))
        // The decoding that runs, which Stats for nerds says, rather than the one asked for.
        assertEquals(PlayerReport.Property("hwdec-current", "\"no\""), reportOf(MpvEvent(MpvNative.Event.PROPERTY_CHANGE, "hwdec-current", "\"no\"")))
        assertEquals(PlayerReport.FileLoaded, reportOf(MpvEvent.of(MpvNative.Event.FILE_LOADED)))
        assertEquals(PlayerReport.Ended(PlayerEnd.ERROR), reportOf(MpvEvent(MpvNative.Event.END_FILE, null, """{"reason":"error","error":"loading failed"}""")))
        assertNull(reportOf(MpvEvent.of(MpvNative.Event.SEEK)))
    }
}
