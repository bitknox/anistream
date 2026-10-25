// Where a launch opens: Continue for the launcher, and for the fixtures flavour, what its extras
// name.

package tv.anistream.tv

import tv.anistream.engine.core.AnilistId
import tv.anistream.tv.shell.Destination

data class Start(
    val destination: Destination = Destination.Continue,
    val title: AnilistId? = null,
    val query: String? = null,
    /** Whether first launch's pages may show: always on a release; on fixtures, `--ez firstLaunch true`. */
    val firstLaunch: Boolean = true,
    /** Whether the update's page opens over the destination: fixtures' `--es update <stage>`. */
    val update: Boolean = false,
)
