// An episode's discussion on TV: a read-only panel from the episode's actions, never from the
// player. Up and down move comment to comment, each focused with a 3 dp silk ring; Select on one
// with a spoiler opens it. No likes, replies, posting, images or links.

package tv.anistream.tv.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import tv.anistream.design.ObiSilk
import tv.anistream.engine.model.CommentView
import tv.anistream.engine.model.DiscussionState
import tv.anistream.engine.model.DiscussionView
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.MediaKind
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.text

/** The read-only discussion of [episode] of [entry], read from the engine for as long as it is open. */
@Composable
fun DiscussionPanel(entry: Entry, episode: UInt) {
    val store = LocalAppStore.current
    val open by store.discussion.open.collectAsStateWithLifecycle()
    val view = open?.takeIf { it.media == entry.id && it.episode == episode }
    LaunchedEffect(entry.id, episode) { store.openDiscussion(entry.id, episode) }
    DisposableEffect(entry.id, episode) { onDispose { store.closeDiscussion() } }
    val context = LocalContext.current
    Column(Modifier.fillMaxSize()) {
        Column(Modifier.padding(start = 16.dp, bottom = 4.dp)) {
            val count = view?.commentCount ?: store.discussion.count(entry.id, episode)
            Text(
                if (count != null) Strings.tvDiscussionCountReadOnly(context, count.toInt()) else "",
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Haze,
            )
        }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (val state = view?.state ?: DiscussionState.Loading) {
                DiscussionState.Loading -> Line("")
                is DiscussionState.RateLimited -> Line(Strings.discussionWaiting(context, "${state.seconds} s"))
                DiscussionState.NoThread -> Line(stringResource(R.string.discussion_no_thread))
                DiscussionState.Failed -> Failure(onRetry = { store.openDiscussion(entry.id, episode) })
                is DiscussionState.Offline, is DiscussionState.Ready -> {
                    if (view!!.comments.isEmpty()) Line(stringResource(R.string.discussion_no_comments)) else Comments(view)
                }
            }
        }
        Text(
            stringResource(R.string.tv_discussion_footer),
            style = MaterialTheme.typography.bodyMedium,
            color = ObiSilk.Haze,
            modifier = Modifier.fillMaxWidth().padding(start = 16.dp, top = 14.dp),
        )
    }
}

@Composable
private fun Line(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, top = 12.dp))
}

@Composable
private fun Failure(onRetry: () -> Unit) {
    val focus = remember { FocusRequester() }
    Column(Modifier.padding(start = 16.dp, top = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(R.string.discussion_failed), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
        TvListRow(stringResource(R.string.action_try_again), onClick = onRetry, modifier = Modifier.focusRequester(focus))
    }
    LaunchedEffect(Unit) { focus.tryFocus() }
}

/** Every comment in a flat list, replies stepped in; the first takes focus. */
@Composable
private fun Comments(view: DiscussionView) {
    val flat = remember(view.comments) { flatten(view.comments) }
    val revealed = remember { mutableStateMapOf<Int, Boolean>() }
    val list = rememberLazyListState()
    val first = remember { FocusRequester() }
    LazyColumn(state = list, verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxSize()) {
        itemsIndexed(flat, key = { _, c -> c.first.id.toLong() }) { index, (comment, depth) ->
            CommentCard(comment, depth, view, revealed, if (index == 0) Modifier.focusRequester(first) else Modifier)
        }
    }
    LaunchedEffect(Unit) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (first.tryFocus()) return@LaunchedEffect
        }
    }
}

private fun flatten(comments: List<CommentView>, depth: Int = 0): List<Pair<CommentView, Int>> =
    comments.flatMap { listOf(it to depth) + flatten(it.replies, 1) }

@Composable
private fun CommentCard(comment: CommentView, depth: Int, view: DiscussionView, revealed: MutableMap<Int, Boolean>, modifier: Modifier) {
    val context = LocalContext.current
    var focused by remember { mutableStateOf(false) }
    val hasSpoiler = comment.body.any { block -> block.spans.any { it.spoiler } }
    val open = revealed[comment.id.toInt()] == true
    val size = if (depth == 0) 32.dp else 24.dp
    Row(
        modifier
            .padding(start = if (depth == 0) 0.dp else 44.dp)
            .fillMaxWidth()
            .onFocusChanged { focused = it.isFocused }
            .then(if (focused) Modifier.border(3.dp, ObiSilk.Silk, RoundedCornerShape(12.dp)) else Modifier)
            .clickable(enabled = hasSpoiler && !open) { revealed[comment.id.toInt()] = true }
            .focusable()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.size(size).background(avatarColor(comment.author), CircleShape), contentAlignment = Alignment.Center) {
            Text(comment.author.take(1).uppercase(), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Silk, fontWeight = FontWeight.SemiBold)
        }
        Column(Modifier.weight(1f)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(comment.author, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, color = ObiSilk.Silk)
                Text(
                    "${comment.age.text(context)} · ${Strings.tvDiscussionLikes(context, comment.likeCount.toInt())}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Haze,
                )
            }
            for (block in comment.body) {
                val media = block.spans.firstOrNull { it.media != null }
                if (media != null) {
                    Text(
                        stringResource(if (media.media == MediaKind.VIDEO) R.string.discussion_video else R.string.discussion_image),
                        style = MaterialTheme.typography.bodyLarge,
                        color = ObiSilk.Haze,
                    )
                    continue
                }
                Text(
                    buildAnnotatedString {
                        for (span in block.spans) {
                            if (span.spoiler && !open) {
                                withStyle(SpanStyle(color = Color.Transparent, background = ObiSilk.RaisedSelected)) { append(span.text) }
                            } else {
                                withStyle(SpanStyle(fontWeight = if (span.bold) FontWeight.SemiBold else null)) { append(span.text) }
                            }
                        }
                    },
                    style = MaterialTheme.typography.bodyLarge,
                    color = ObiSilk.Silk,
                )
            }
            if (focused && hasSpoiler && !open) {
                Text(stringResource(R.string.tv_discussion_select_spoiler), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.padding(top = 4.dp))
            }
        }
    }
}

private fun avatarColor(name: String): Color {
    val palette = listOf(0xFF4F6382, 0xFF6E5470, 0xFF7A6A45, 0xFF5A6878)
    return Color(palette[(name.hashCode() and 0x7fffffff) % palette.size])
}
