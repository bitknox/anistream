package tv.anistream.store

import org.junit.Test
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import tv.anistream.engine.model.CurrentMatch

class MatchPickerModelTest {
    private val id = 900001u
    private fun matched(by: Boolean = true) = CurrentMatch("archive", "Made-up Archive", "Stardust Wayfarer", by)

    @Test
    fun aQuestionIsAskedUntilTheTitleIsMatched() {
        val model = MatchPickerModel()
        model.asked(id, "archive", "Made-up Archive", emptyList())
        assertTrue(model.state.value.asks(id))
        model.matched(id, matched(by = false))
        assertFalse(model.state.value.asks(id))
        assertEquals("Made-up Archive", model.state.value.source(id)?.second)
    }

    @Test
    fun anAnswerToAnOlderSearchIsNotShown() {
        val model = MatchPickerModel()
        model.searching(id, "star")
        model.searching(id, "stardust")
        model.found(id, "star", "archive", "Made-up Archive", emptyList())
        assertEquals(MatchSearch.Searching("stardust"), model.state.value.searches[id])
        model.found(id, "stardust", "archive", "Made-up Archive", emptyList())
        assertEquals(MatchSearch.Empty("stardust", "Made-up Archive"), model.state.value.searches[id])
    }

    @Test
    fun aPickSettlesWhenTheEngineSaysTheMatch() {
        val model = MatchPickerModel()
        model.asked(id, "archive", "Made-up Archive", emptyList())
        model.picking(id, "archive:show", "Stardust Wayfarer", "Made-up Archive")
        assertEquals("archive:show", model.state.value.picking[id])
        model.matched(id, matched())
        val state = model.state.value
        assertNull(state.picking[id])
        assertFalse(state.asks(id))
        assertEquals("Stardust Wayfarer", state.settled?.title)
    }

    @Test
    fun closingTheSheetForgetsItsSearch() {
        val model = MatchPickerModel()
        model.searching(id, "star")
        model.closed(id)
        assertNull(model.state.value.searches[id])
    }

    @Test
    fun letTheEngineMatchForgetsWhatWasChosen() {
        val model = MatchPickerModel()
        model.matched(id, matched())
        model.cleared(id)
        assertNull(model.state.value.current[id])
    }
}
