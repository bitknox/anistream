// Where a launch opens: Continue for the launcher, and for the fixtures flavour, what its extras
// name.

package tv.anistream.tv

import tv.anistream.engine.core.AnilistId
import tv.anistream.tv.shell.Destination

data class Start(
    val destination: Destination = Destination.Continue,
    val title: AnilistId? = null,
    val query: String? = null,
)
