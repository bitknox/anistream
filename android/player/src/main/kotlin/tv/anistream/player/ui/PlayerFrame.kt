// The player as the app's frame shows it: up while the model is, over the navigation bar.

package tv.anistream.player.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import tv.anistream.engine.core.AnilistId
import tv.anistream.player.PlayerModel
import tv.anistream.ui.PlayerSlot

/** [model]'s screen in the frame's player slot: the frame navigates to it while it is up. */
@Stable
class PlayerFrame(private val model: PlayerModel) : PlayerSlot {
    override val presented: Boolean get() = model.isPresented

    override val title: AnilistId? get() = model.titleId

    @Composable
    override fun Content() = PlayerScreen(model)
}
