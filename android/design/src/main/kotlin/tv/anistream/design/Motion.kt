// The motion tokens, from motion-android.md: springs for anything spatial, never bounce; fades for
// opacity; linear only for countdowns. Everything the platform animates keeps its own motion.

package tv.anistream.design

import androidx.compose.animation.core.FastOutLinearInEasing
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.TweenSpec
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween

object Motion {
    /**
     * Selection, toggles, the row dim after marking watched, the pane divider's handle, the
     * fade-through between navigation destinations. Apple's 0.2 s spring.
     */
    fun <T> quick(): SpringSpec<T> = spring(dampingRatio = 1f, stiffness = 1000f)

    /**
     * Content changes inside a screen: progress, Skip opening and Up Next appearing, list insert
     * and remove, the spring back after a cancelled gesture. Apple's 0.3 s spring.
     */
    fun <T> standard(): SpringSpec<T> = spring(dampingRatio = 1f, stiffness = 440f)

    /**
     * Player controls appearing, artwork arriving, the first frame over the art, a page's content
     * in a shared transition, the detail pane's content.
     */
    fun <T> fadeIn(): TweenSpec<T> = tween(durationMillis = 200, easing = LinearOutSlowInEasing)

    /** Player controls hiding, a page's content leaving. */
    fun <T> fadeOut(): TweenSpec<T> = tween(durationMillis = 300, easing = FastOutLinearInEasing)

    // linear: countdowns only, written in place: tween(10_000, easing = LinearEasing).
    // system: no token. Shared bounds and shared elements keep Compose's default boundsTransform.
}
