// Light dismiss, as native-ux.md has it: a sheet with nothing to lose closes on a click or tap
// outside it, one that holds what the person typed stays open until its buttons or Back close it,
// and a dialog that asks a question waits for its answer.

package tv.anistream.ui

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheetProperties
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.window.DialogProperties

/**
 * True once any of [values] has changed from what it was as the surface opened, and from then on,
 * even if it changes back: the person has typed or chosen something.
 */
@Composable
internal fun changedSinceOpened(vararg values: Any?): Boolean {
    val now = values.toList()
    val opened = remember { now }
    var changed by rememberSaveable { mutableStateOf(false) }
    val differs = now != opened
    SideEffect { if (differs) changed = true }
    return changed || differs
}

/** A sheet's properties: a click outside closes it only while it holds nothing typed. Back always does. */
@OptIn(ExperimentalMaterial3Api::class)
internal fun sheetProperties(holdsInput: Boolean): ModalBottomSheetProperties =
    ModalBottomSheetProperties(shouldDismissOnClickOutside = !holdsInput)

/** A dialog that asks a question: Back is its Cancel, and a click outside does nothing. */
internal val AskingDialog = DialogProperties(dismissOnClickOutside = false)
