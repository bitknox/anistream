// The destinations A1 builds: their bar, and a line saying they are not in this build yet.

package tv.anistream.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

@Composable
internal fun NotYet(destination: Destination, account: (() -> Unit)? = null) {
    ScreenFrame(title = destination.label, account = account?.let { Account(null, it) }) { padding ->
        Column(Modifier.padding(top = padding.calculateTopPadding())) {
            Quiet("Not in this build yet")
        }
    }
}
