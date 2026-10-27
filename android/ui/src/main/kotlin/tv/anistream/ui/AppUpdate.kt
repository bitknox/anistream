// The app's own update on screen: Settings' row ("Update to v2.0.3", then "Downloading 34 %",
// then "Installing"), the notice that says a release is out with Update as its button, the notice
// that follows the download with its progress line, and the one sentence that asks for Android's
// "Install unknown apps".

package tv.anistream.ui

import tv.anistream.store.plainReason
import androidx.compose.ui.platform.LocalContext
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import tv.anistream.design.Motion
import tv.anistream.design.tabular
import tv.anistream.store.UpdateCheck
import tv.anistream.store.UpdateOffer
import tv.anistream.store.UpdateState
import tv.anistream.store.Wording
import tv.anistream.i18n.R
import androidx.compose.ui.res.stringResource
import tv.anistream.i18n.Strings
import android.content.Context
import tv.anistream.store.percentText

/** What the update's row and notice say at each step. */
internal object UpdateWords {
    /** "34%", or nothing while the size is not known. */
    fun percent(state: UpdateState.Downloading): String? = state.fraction?.let { percentText((it * 100).toInt()) }

    /** The row's headline: "Update to v2.0.3", "Downloading 34%", "Installing". */
    fun title(context: Context, state: UpdateState): String? = when (state) {
        UpdateState.None -> null
        is UpdateState.Available, is UpdateState.Allowing, is UpdateState.Failed -> Strings.updatesUpdateTo(context, state.offer?.name.orEmpty())
        is UpdateState.Downloading -> percent(state)?.let { Strings.updatesDownloadingPercent(context, it) } ?: Strings.downloadsCaptionRunningPlain(context)
        is UpdateState.Verifying, is UpdateState.Installing -> Strings.updatesInstallingPlain(context)
        is UpdateState.Done -> Strings.updatesUpdated(context, state.offer.version)
    }

    /**
     * The line under it: the APK's size, or that Update opens the release's page where this build
     * does not install it; how much is down; and what the installer waits on.
     */
    fun line(context: Context, state: UpdateState, installs: Boolean): String? = when (state) {
        UpdateState.None -> null
        is UpdateState.Available, is UpdateState.Allowing, is UpdateState.Failed ->
            if (installs) state.offer?.bytes?.takeIf { it > 0 }?.let { Wording.bytes(context, it) } else Strings.updatesOpensPage(context)
        is UpdateState.Downloading ->
            if (state.total > 0) Strings.downloadsLiveProgress(context, Wording.bytes(context, state.done), Wording.bytes(context, state.total)) else Wording.bytes(context, state.done)
        is UpdateState.Verifying -> Strings.updatesCheckingDownloadShort(context)
        // Android asks to confirm the first time; anistream closes as the update goes in.
        is UpdateState.Installing -> Strings.updatesClosesAsInstalls(context)
        is UpdateState.Done -> Strings.updatesOpensNextTime(context)
    }

    /** The notice under way: "Downloading version 2.0.3 · 34%", "Installing version 2.0.3". */
    fun notice(context: Context, state: UpdateState): String? = when (state) {
        is UpdateState.Downloading -> Strings.updatesDownloading(context, state.offer.version) + (percent(state)?.let { " · $it" } ?: "") // i18n-skip
        is UpdateState.Verifying, is UpdateState.Installing -> Strings.updatesInstalling(context, state.offer?.version.orEmpty())
        else -> null
    }

    /**
     * What the last check said: "Checking", "anistream is up to date", or why it did not. A release
     * it found is the row above it.
     */
    fun check(context: Context, check: UpdateCheck): String? = when (check) {
        UpdateCheck.Unknown -> null
        UpdateCheck.Checking -> Strings.updatesChecking(context)
        UpdateCheck.UpToDate -> Strings.updatesUpToDate(context)
        UpdateCheck.Found -> null
        is UpdateCheck.Failed -> Strings.updatesNotCheckedBecause(context, plainReason(context, check.reason))
    }

    /** The notice that a release is out. */
    fun out(context: Context, offer: UpdateOffer): String = Strings.updatesAvailable(context, offer.version)

    /** Its button: Update where this build installs it, the release's page where not. */
    fun action(context: Context, offer: UpdateOffer, installs: Boolean): String =
        if (installs) Strings.updatesUpdateTo(context, offer.name) else Strings.updatesOpenRelease(context)

    /** How far the bar is: the download's share, full once it is down, none before. */
    fun progress(state: UpdateState): Float? = when (state) {
        is UpdateState.Downloading -> state.fraction ?: 0f
        is UpdateState.Verifying, is UpdateState.Installing -> 1f
        else -> null
    }
}

/**
 * Settings' row for the update, in About's Updates group while a newer release is known: the
 * glyph in amber while pressing it starts the update, the bar in water while it downloads, and
 * Cancel while it is under way. A failure says why under it, in silk.
 */
@Composable
internal fun UpdateRow(place: Place) {
    val store = LocalAppStore.current
    val state by store.updates.state.collectAsStateWithLifecycle()
    val offer = state.offer ?: return
    val colors = MaterialTheme.colorScheme
    val installs = store.updates.installs(offer)
    val pressable = state is UpdateState.Available || state is UpdateState.Failed
    val progress = UpdateWords.progress(state)
    RowFrame(
        place,
        onClick = if (pressable) store::takeUpdate else null,
        modifier = Modifier.heightIn(min = if (progress != null) 88.dp else 72.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Icon(Icons.Downloads, contentDescription = null, tint = if (pressable) colors.primary else colors.onSurfaceVariant)
            Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
                Text(UpdateWords.title(LocalContext.current, state).orEmpty(), style = MaterialTheme.typography.titleMedium.tabular())
                UpdateWords.line(LocalContext.current, state, installs)?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
                }
                (state as? UpdateState.Failed)?.let {
                    Text(it.failure.text(LocalContext.current), style = MaterialTheme.typography.bodyMedium, color = colors.onSurface)
                }
                if (progress != null) UpdateBar(progress, Modifier.padding(top = 8.dp))
            }
            if (state.busy && state !is UpdateState.Allowing) QuietButton(stringResource(R.string.action_cancel), onClick = store.updates::cancel)
        }
    }
}

/** Check now, and what the last check said under it. */
@Composable
internal fun CheckNowRow(place: Place) {
    val store = LocalAppStore.current
    val check by store.updates.check.collectAsStateWithLifecycle()
    val checking = check == UpdateCheck.Checking
    InfoRow(
        place,
        stringResource(R.string.action_check_now),
        UpdateWords.check(LocalContext.current, check),
        onClick = if (checking) null else store::checkForUpdates,
        trailing = { WorkRing(checking) },
        modifier = Modifier.semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
    )
}

/** The download's bar: water on the rule, 4 high, moving on the standard spring. */
@Composable
private fun UpdateBar(fraction: Float, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val shown by animateFloatAsState(fraction.coerceIn(0f, 1f), Motion.standard(), label = "update")
    LinearProgressIndicator(
        progress = { shown },
        modifier = modifier.fillMaxWidth().height(4.dp),
        color = colors.secondary,
        trackColor = colors.outline,
        gapSize = 4.dp,
        drawStopIndicator = {},
    )
}

/**
 * The notice while the update is under way, where the snackbars are: its words and Cancel on a
 * line 48 high, the bar under them, on surfaceContainerHigh at corners 4. Not shown while
 * [hidden], on Settings, whose row says the same.
 */
@Composable
internal fun UpdateNotice(hidden: Boolean, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val state by store.updates.state.collectAsStateWithLifecycle()
    val words = UpdateWords.notice(LocalContext.current, state)
    AnimatedVisibility(
        visible = words != null && !hidden,
        enter = fadeIn(Motion.fadeIn()),
        exit = fadeOut(Motion.fadeOut()),
        modifier = modifier,
    ) {
        // Kept as it was while it fades out.
        val last = words ?: return@AnimatedVisibility
        Surface(
            shape = MaterialTheme.shapes.extraSmall,
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
            contentColor = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp).fillMaxWidth(),
        ) {
            Column(Modifier.padding(start = 16.dp, end = 4.dp, bottom = 12.dp)) {
                Row(Modifier.heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        last,
                        style = MaterialTheme.typography.bodyMedium.tabular(),
                        modifier = Modifier.weight(1f),
                    )
                    QuietButton(stringResource(R.string.action_cancel), onClick = store.updates::cancel)
                }
                UpdateBar(UpdateWords.progress(state) ?: 1f, Modifier.padding(end = 12.dp).semantics { contentDescription = last })
            }
        }
    }
}

/**
 * The engine's word that a release is out, said once as a notice with Update as its button, and
 * the explanation Update shows the first time, where Android's "Install unknown apps" is off.
 */
@Composable
internal fun UpdatePrompts() {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val context = LocalContext.current
    val notice by store.updates.notice.collectAsStateWithLifecycle()
    LaunchedEffect(notice) {
        val offer = notice ?: return@LaunchedEffect
        store.updates.told(offer)
        snackbars.show(UpdateWords.out(context, offer), UpdateWords.action(context, offer, store.updates.installs(offer)), store::takeUpdate)
    }
    val state by store.updates.state.collectAsStateWithLifecycle()
    val allowing = state as? UpdateState.Allowing ?: return
    // While Android's page is open, and as the app comes back to it, the dialog is down.
    if (allowing.opened) return
    AlertDialog(
        onDismissRequest = store.updates::notNow,
        title = { Text(stringResource(R.string.updates_allow_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.updates_allow_step_1))
                Text(stringResource(R.string.updates_allow_step_2))
                Text(stringResource(R.string.updates_allow_step_3))
            }
        },
        confirmButton = { Button(onClick = store.updates::openInstallSettings) { Text(stringResource(R.string.action_open_settings)) } },
        dismissButton = { QuietButton(stringResource(R.string.action_not_now), onClick = store.updates::notNow) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}
