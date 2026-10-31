// A destination's frame: the large flexible top app bar over the screen's content.

package tv.anistream.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.snap
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeFlexibleTopAppBar
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import tv.anistream.design.Motion
import tv.anistream.i18n.R as I18nR
import androidx.compose.ui.res.stringResource

/**
 * The large flexible top app bar, 112 at rest under the status bar (136 with a [subtitle]), which
 * collapses to the 64 small bar on surfaceContainer as the content scrolls and opens again at the
 * top. Continue and Library carry the [account] button, which opens Settings; a page pushed over
 * them carries the back arrow, [onBack]. Under the title stands the one offline line while
 * offline, a job's activity once it runs past 4 s, or [subtitle]. Work shorter than that shows
 * where its result lands, as placeholders or a ring in its button, never as a bar here.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun ScreenFrame(
    title: String,
    subtitle: String? = null,
    account: Account? = null,
    onBack: (() -> Unit)? = null,
    actions: @Composable () -> Unit = {},
    /**
     * Home's own words under the title, in place of the generic offline line and job activity:
     * with [ownLine], the screen's line is the only one, and none stands when it is `null`.
     */
    line: FrameLine? = null,
    ownLine: Boolean = false,
    /**
     * Home's activity once it runs past 4 s, in the account button's place in the bar's top row,
     * so the bar keeps its height and nothing under it moves.
     */
    activity: String? = null,
    content: @Composable (PaddingValues) -> Unit,
) {
    val scroll = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val colors = MaterialTheme.colorScheme
    val type = MaterialTheme.typography
    // Under the title: the offline line, a job past 4 s in words, or the screen's own line.
    val generic = frameLine(subtitle)
    val line = if (ownLine) line else line ?: generic
    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        containerColor = colors.surface,
        contentWindowInsets = WindowInsets(0),
        topBar = {
            LargeFlexibleTopAppBar(
                // The bar draws its title twice, large at rest and small once collapsed, each in a
                // text style of its own: the large one in the brief's role rather than the bar's
                // default displaySmall, and the subtitle in bodyLarge rather than titleMedium.
                title = { Text(title, style = expanded(type.headlineMedium)) },
                subtitle = line?.let {
                    { FrameLineText(it, expanded(type.bodyLarge)) }
                },
                navigationIcon = {
                    if (onBack != null) {
                        IconButton(onClick = onBack) { Icon(Icons.Back, contentDescription = stringResource(I18nR.string.action_back)) }
                    }
                },
                actions = {
                    actions()
                    AnimatedContent(
                        targetState = activity,
                        transitionSpec = {
                            (fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut())).using(SizeTransform(clip = false) { _, _ -> snap() })
                        },
                        contentAlignment = Alignment.CenterEnd,
                        label = "frame-activity", // i18n-skip
                    ) { words ->
                        when {
                            words != null -> ActivityLabel(words)
                            account != null -> AccountButton(account)
                        }
                    }
                },
                expandedHeight = if (line == null) 112.dp else 136.dp,
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = colors.surface,
                    scrolledContainerColor = colors.surfaceContainer,
                    titleContentColor = colors.onSurface,
                    actionIconContentColor = colors.onSurface,
                    navigationIconContentColor = colors.onSurface,
                ),
                scrollBehavior = scroll,
            )
        },
    ) { padding ->
        content(padding)
    }
}

/**
 * [style] in the bar's expanded row, which sets a larger text style than its collapsed one; the
 * collapsed row's own style otherwise.
 */
@Composable
private fun expanded(style: TextStyle): TextStyle {
    val current = LocalTextStyle.current
    return if (current.fontSize >= style.fontSize) style else current
}

/** Who is signed in, for the account button, and what it opens. */
data class Account(
    /** The signed-in user's name, whose initial the button shows; `null` when signed out. */
    val name: String?,
    val open: () -> Unit,
)

/** Accounts and settings: the user's initial on a raised circle, or the account glyph signed out. */
/** Work running past 4 s, in words beside a small ring, read out once when it shows. */
@Composable
private fun ActivityLabel(words: String) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier
            .heightIn(min = 48.dp)
            .padding(end = 16.dp)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        CircularProgressIndicator(Modifier.size(14.dp), color = colors.onSurface, strokeWidth = 1.5.dp, trackColor = colors.outlineVariant)
        Text(words, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1)
    }
}

@Composable
private fun AccountButton(account: Account) {
    val label = account.name?.let { stringResource(I18nR.string.account_button_named, it) } ?: stringResource(I18nR.string.nav_accounts_and_settings)
    IconButton(onClick = account.open, modifier = Modifier.semantics { contentDescription = label }) {
        val name = account.name
        if (name == null) {
            Icon(Icons.Account, contentDescription = null)
        } else {
            Box(
                Modifier.size(32.dp).background(MaterialTheme.colorScheme.surfaceContainerHighest, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Text(name.take(1).uppercase(), style = MaterialTheme.typography.labelLarge)
            }
        }
    }
}
