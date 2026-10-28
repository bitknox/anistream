// What the player shows over the picture, as AndroidPlayer, AndroidPlayerUpNext and T4/T5 have it:
// one card in the corner at a time (the stall prompt, Undo, Up Next, Skip), the resume note, an
// alert, "Still watching?", and the start that could not play.

package tv.anistream.player.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.FastOutLinearInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import tv.anistream.design.Motion
import tv.anistream.design.ObiSilk
import tv.anistream.design.tabular
import tv.anistream.engine.model.NoticeKind
import tv.anistream.player.PlayerModel
import tv.anistream.player.PlayerModel.Corner
import tv.anistream.ui.ArtworkImage
import tv.anistream.ui.pixels

/**
 * The corner, right 48 and bottom 88: one card at a time. Skip shows only while the controls are
 * hidden; the others stay with the controls up. [throughGround] is the fade through the ground at
 * Up Next's zero, which the card fades with, read as it draws.
 */
@Composable
fun CornerCard(model: PlayerModel, chromeVisible: Boolean, throughGround: () -> Float, modifier: Modifier = Modifier) {
    val corner = model.corner
    val shown = when (corner) {
        Corner.Skip -> !chromeVisible
        null -> false
        else -> true
    }
    AnimatedContent(
        if (shown) corner else null,
        modifier = modifier.graphicsLayer { alpha = 1f - throughGround() },
        contentAlignment = Alignment.BottomEnd,
        transitionSpec = {
            fadeIn(Motion.standard()) togetherWith fadeOut(if (targetState == null) Motion.quick() else Motion.standard())
        },
        label = "corner", // i18n-skip
    ) { card ->
        when (card) {
            Corner.Skip -> SkipButton(model)
            Corner.Undo -> UndoNote(model)
            Corner.UpNext -> UpNextCard(model)
            Corner.Stall -> StallCard(model)
            null -> Box(Modifier.size(1.dp))
        }
    }
}

/** "Skip opening": a 40 dp surfaceContainer pill in a 48 dp target, dimming 8 s after the segment starts. */
@Composable
private fun SkipButton(model: PlayerModel) {
    val skip = model.skip ?: return
    val haptics = LocalHapticFeedback.current
    val touch = remember { MutableInteractionSource() }
    val down by touch.collectIsPressedAsState()
    val dim = model.skipDimmed && !down
    val strength by animateFloatAsState(if (dim) 0.4f else 1f, if (dim) Motion.fadeOut() else Motion.fadeIn(), label = "skip") // i18n-skip
    Box(
        Modifier
            .height(48.dp)
            .graphicsLayer { alpha = strength }
            .clickable(interactionSource = touch, indication = null, role = androidx.compose.ui.semantics.Role.Button) {
                haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                model.takeSkip()
            },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier.height(40.dp).clip(RoundedCornerShape(20.dp)).background(ObiSilk.Raised).padding(start = 16.dp, end = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(skip.label, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk)
            Icon(PlayerIcons.SkipNext, contentDescription = null, tint = ObiSilk.Silk, modifier = Modifier.size(20.dp))
        }
    }
}

/** "Skipped opening · Undo", for 6 s of playback. */
@Composable
private fun UndoNote(model: PlayerModel) {
    val undo = model.undo ?: return
    val haptics = LocalHapticFeedback.current
    Row(
        Modifier.height(40.dp).clip(RoundedCornerShape(20.dp)).background(ObiSilk.Raised).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(undo.label, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk)
        Text(" · ", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
        TextButton(
            onClick = {
                haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                model.undoSkip()
            },
            contentPadding = PaddingValues(horizontal = 12.dp),
        ) { Text("Undo", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk) }
    }
}

/**
 * Up Next, as AndroidPlayerUpNext draws it: 336 wide on surfaceContainer at corners 16, the next
 * episode's still, "UP NEXT", its number and title, the countdown's bar drawn from the playback
 * clock, "Plays in 8 s", Cancel and Play now. It never rises: opacity only.
 */
@Composable
private fun UpNextCard(model: PlayerModel) {
    val card = model.upNext ?: return
    val haptics = LocalHapticFeedback.current
    val countdown = model.countdown
    Column(
        Modifier
            .width(336.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(ObiSilk.Raised)
            .padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 8.dp)
            .semantics { paneTitle = "Up next" },
    ) {
        Row(Modifier.height(60.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Box(Modifier.size(96.dp, 54.dp).clip(RoundedCornerShape(8.dp)).background(ObiSilk.RaisedSelected)) {
                card.next.thumbnail?.let { ArtworkImage(it, plate = null, maxPixelSize = pixels(96.dp), modifier = Modifier.fillMaxSize()) }
            }
            Column(Modifier.weight(1f)) {
                Text("UP NEXT", style = MaterialTheme.typography.labelSmall, color = ObiSilk.Haze)
                Text("Episode ${card.next.number}", style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
                card.next.title?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
        if (countdown != null) {
            LinearProgressIndicator(
                progress = { countdown.second },
                modifier = Modifier.padding(top = 12.dp).fillMaxWidth().height(4.dp),
                color = ObiSilk.Amber,
                trackColor = ObiSilk.Rule,
                gapSize = 4.dp,
                drawStopIndicator = {},
            )
        }
        Row(Modifier.padding(top = 4.dp).height(48.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(
                countdown?.let { "Plays in ${it.first} s" }.orEmpty(),
                style = MaterialTheme.typography.bodyMedium.tabular(),
                color = ObiSilk.Haze,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = model::cancelNext, modifier = Modifier.height(40.dp), contentPadding = PaddingValues(horizontal = 12.dp)) {
                Text("Cancel", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk)
            }
            Button(
                onClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                    model.playNext()
                },
                modifier = Modifier.height(40.dp),
                contentPadding = PaddingValues(start = 12.dp, end = 16.dp),
                colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
            ) {
                Icon(PlayerIcons.Play, contentDescription = null, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(8.dp))
                Text("Play now", style = MaterialTheme.typography.labelLarge)
            }
        }
    }
}

/** "Source is buffering · 9 s", counting up, with Keep waiting and Switch source. */
@Composable
private fun StallCard(model: PlayerModel) {
    val stall = model.stall ?: return
    Column(
        Modifier.width(336.dp).clip(RoundedCornerShape(16.dp)).background(ObiSilk.Raised).padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 8.dp),
    ) {
        Text("Source is buffering · ${stall.held(model.clock)} s", style = MaterialTheme.typography.titleMedium.tabular(), color = ObiSilk.Silk)
        Text("Another stream may play without waiting.", style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
        Row(Modifier.padding(top = 4.dp).height(48.dp).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End)) {
            TextButton(onClick = model::keepWaiting, modifier = Modifier.height(40.dp)) {
                Text("Keep waiting", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk)
            }
            FilledTonalButton(
                onClick = model::switchSource,
                modifier = Modifier.height(40.dp),
                colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
            ) { Text("Switch source", style = MaterialTheme.typography.labelLarge) }
        }
    }
}

/** "Resumed at 12:04 · Start over", for 6 s of playback, at the other corner. */
@Composable
fun ResumeNote(model: PlayerModel, modifier: Modifier = Modifier) {
    val note = model.resume
    AnimatedVisibility(note != null, enter = fadeIn(Motion.standard()), exit = fadeOut(Motion.quick()), modifier = modifier) {
        val shown = note ?: return@AnimatedVisibility
        Row(
            Modifier.height(40.dp).clip(RoundedCornerShape(20.dp)).background(ObiSilk.Raised).padding(start = 16.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(shown.label, style = MaterialTheme.typography.labelLarge.tabular(), color = ObiSilk.Silk)
            Text(" · ", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze)
            TextButton(onClick = model::startOver, contentPadding = PaddingValues(horizontal = 12.dp)) {
                Text("Start over", style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk)
            }
        }
    }
}

/** A message on the video, under the top bar, for 4 s. */
@Composable
fun AlertNote(model: PlayerModel, modifier: Modifier = Modifier) {
    val alert = model.alert
    AnimatedVisibility(alert != null, enter = fadeIn(Motion.standard()), exit = fadeOut(Motion.quick()), modifier = modifier) {
        val shown = alert ?: return@AnimatedVisibility
        Row(
            Modifier.clip(RoundedCornerShape(20.dp)).background(ObiSilk.RaisedHigh).padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (shown.kind == NoticeKind.ALERT) Box(Modifier.size(8.dp).clip(RoundedCornerShape(4.dp)).background(ObiSilk.Vermilion))
            Text(shown.text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Silk)
        }
    }
}

/**
 * "Still watching?", drawn in the player as a dialog is (not a Dialog window, so the system bars
 * stay hidden): 312 wide on surfaceContainerHigh at corners 28, over the black 32 % scrim.
 */
@Composable
fun StillWatching(model: PlayerModel) {
    val haptics = LocalHapticFeedback.current
    AnimatedVisibility(model.stillWatching, enter = fadeIn(Motion.standard()), exit = fadeOut(Motion.quick())) {
        Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.32f)).clickable(interactionSource = null, indication = null) {}, contentAlignment = Alignment.Center) {
            Column(
                Modifier.width(312.dp).clip(RoundedCornerShape(28.dp)).background(ObiSilk.RaisedHigh).padding(start = 24.dp, end = 24.dp, top = 24.dp, bottom = 16.dp)
                    .semantics { paneTitle = "Still watching?" },
            ) {
                Text("Still watching?", style = MaterialTheme.typography.titleLarge, color = ObiSilk.Silk)
                Text(
                    "Paused after ${model.stillWatchingAfter} episodes in a row.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Haze,
                    modifier = Modifier.padding(top = 16.dp),
                )
                Row(Modifier.padding(top = 24.dp).fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                    TextButton(onClick = model::stopWatching) { Text("Stop", color = ObiSilk.Silk) }
                    FilledTonalButton(
                        onClick = {
                            haptics.performHapticFeedback(HapticFeedbackType.ContextClick)
                            model.continueWatching()
                        },
                        colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
                    ) { Text("Continue watching") }
                }
            }
        }
    }
}

/**
 * What the starting screen says under its line: after 20 s of finding nothing, Choose source and
 * Stop; and when the playback never played, "Couldn't play episode 7" with Choose source and Close.
 */
@Composable
internal fun StartingChoices(model: PlayerModel, onChooseSource: () -> Unit) {
    val starting = model.starting
    if (model.failure == null && (starting == null || !starting.offersChoice(model.clock))) return
    Row(Modifier.padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (model.failure != null) {
            Button(
                onClick = onChooseSource,
                colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
            ) { Text("Choose source") }
            TextButton(onClick = model::viewerClosed) { Text("Close", color = ObiSilk.Silk) }
        } else {
            FilledTonalButton(
                onClick = onChooseSource,
                colors = ButtonDefaults.filledTonalButtonColors(containerColor = ObiSilk.RaisedSelected, contentColor = ObiSilk.Silk),
            ) { Text("Choose source") }
            TextButton(onClick = model::viewerClosed) { Text("Stop", color = ObiSilk.Silk) }
        }
    }
}

/** The 250 ms fade into the ground at Up Next's zero, and out of it on the next first frame (T5). */
internal val ThroughGround = tween<Float>(250, easing = FastOutLinearInEasing)

/** The semantics of a control drawn over the video, for TalkBack. */
internal fun Modifier.described(label: String) = semantics { contentDescription = label }
