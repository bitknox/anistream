// Where the player's sheets open: a list of many tracks shows the one in use.

package tv.anistream.player.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class PlayerSheetsTest {
    private val twenty = (1L..20L).toList()

    @Test
    fun theTracksSheetOpensOnTheSubtitlesInUse() {
        // The title, the heading and Off come first: the last of twenty tracks is row 22.
        assertEquals(21, tracksOpeningRow(twenty, 20L))
        assertEquals(10, tracksOpeningRow(twenty, 9L))
    }

    @Test
    fun theTracksSheetOpensAtTheTopNearIt() {
        assertEquals(0, tracksOpeningRow(twenty, null))
        assertEquals(0, tracksOpeningRow(twenty, 1L))
        assertEquals(0, tracksOpeningRow(twenty, 99L))
        assertEquals(0, tracksOpeningRow(emptyList(), null))
    }
}
