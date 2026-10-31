package tv.anistream.design

import androidx.annotation.DrawableRes
import androidx.compose.foundation.border
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp

/**
 * A language's flag beside its name: 20 by 15 dp, corners 2 and a hairline edge in the faint rule
 * so a white flag stays visible on a light surface. Decoration: the row's label is the name, so
 * it has no description for TalkBack.
 */
@Composable
fun LanguageFlag(@DrawableRes res: Int, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(2.dp)
    Image(
        painter = painterResource(res),
        contentDescription = null,
        contentScale = ContentScale.FillBounds,
        modifier = modifier.size(width = 20.dp, height = 15.dp).clip(shape).border(0.5.dp, ObiSilk.FaintRule, shape),
    )
}
