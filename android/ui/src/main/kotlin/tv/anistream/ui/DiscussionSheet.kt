// An episode's discussion on the phone: AniList's forum thread in a modal bottom sheet. The words
// lead: names and ages above the text, a heart and Reply in haze beneath, spoilers as bars the
// length of the hidden words, and amber only on Post once there is text to post.

package tv.anistream.ui

import android.content.ClipData
import android.net.Uri
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import tv.anistream.design.ShowTitle
import tv.anistream.design.tabular
import tv.anistream.engine.model.CommentBlock
import tv.anistream.engine.model.CommentView
import tv.anistream.engine.model.DiscussionState
import tv.anistream.engine.model.DiscussionView
import tv.anistream.engine.model.Entry
import tv.anistream.engine.model.MediaKind
import tv.anistream.engine.model.PostState
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.store.text

/** How many lines a comment shows before Show more. */
private const val FOLD_LINES = 10

/** The line height the fold counts in. */
private val LINE = 22.dp

/** The discussion a person asked to read, in its sheet, for as long as it stands. */
@Composable
fun DiscussionHost() {
    val store = LocalAppStore.current
    val target by store.discussion.target.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val shown = target ?: return
    val entry = titles[shown.media] ?: return
    DiscussionSheet(entry, shown.episode) { store.discussion.dismiss() }
}

/** The discussion of [episode] of [entry], read from the engine for as long as the sheet is open. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiscussionSheet(entry: Entry, episode: UInt, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val open by store.discussion.open.collectAsStateWithLifecycle()
    val view = open?.takeIf { it.media == entry.id && it.episode == episode }
    LaunchedEffect(entry.id, episode) { store.openDiscussion(entry.id, episode) }
    DisposableEffect(entry.id, episode) { onDispose { store.closeDiscussion() } }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = false)
    val scope = rememberCoroutineScope()
    val close: () -> Unit = { scope.launch { sheetState.hide() }.invokeOnCompletion { onDismiss() } }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
        containerColor = MaterialTheme.colorScheme.surfaceContainer,
        scrimColor = Color.Black.copy(alpha = 0.32f),
        dragHandle = { androidx.compose.material3.BottomSheetDefaults.DragHandle(color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)) },
    ) {
        DiscussionContent(entry, episode, view, close)
    }
}

@Composable
private fun DiscussionContent(entry: Entry, episode: UInt, view: DiscussionView?, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    // The spoilers a person opened stay open for the visit.
    val revealed = remember { mutableStateMapOf<String, Boolean>() }
    var replying by remember { mutableStateOf<CommentView?>(null) }
    var editing by remember { mutableStateOf<CommentView?>(null) }
    var deleting by remember { mutableStateOf<CommentView?>(null) }
    val list = rememberLazyListState()
    // The next page as the end of the comments comes within a few rows.
    LaunchedEffect(list, view?.state) {
        if ((view?.state as? DiscussionState.Ready)?.more != true) return@LaunchedEffect
        snapshotFlow { list.layoutInfo.let { (it.visibleItemsInfo.lastOrNull()?.index ?: 0) >= it.totalItemsCount - 3 } }
            .collect { if (it) store.moreComments() }
    }
    Column(Modifier.fillMaxWidth().fillMaxHeight(0.92f).imePadding()) {
        Header(entry, episode, view)
        Box(Modifier.weight(1f).fillMaxWidth()) {
            when (val state = view?.state ?: DiscussionState.Loading) {
                DiscussionState.Loading -> Waiting(null)
                is DiscussionState.RateLimited -> Waiting(state.seconds)
                DiscussionState.NoThread -> Message(
                    stringResource(R.string.discussion_no_thread),
                    stringResource(R.string.discussion_no_thread_detail),
                    stringResource(R.string.title_open_anilist),
                ) { store.openExternal(view?.url ?: return@Message) }
                DiscussionState.Failed -> Message(
                    stringResource(R.string.discussion_failed),
                    null,
                    stringResource(R.string.action_try_again),
                ) { store.openDiscussion(entry.id, episode) }
                is DiscussionState.Offline -> if (view?.comments.isNullOrEmpty()) {
                    OfflineState()
                } else {
                    Comments(view!!, list, revealed, offlineAge = state.ageMinutes, onReply = { replying = it; editing = null }, onEdit = { editing = it; replying = null }, onDelete = { deleting = it })
                }
                is DiscussionState.Ready -> if (view!!.comments.isEmpty()) {
                    Message(stringResource(R.string.discussion_no_comments), null, null) {}
                } else {
                    Comments(view, list, revealed, offlineAge = null, onReply = { replying = it; editing = null }, onEdit = { editing = it; replying = null }, onDelete = { deleting = it })
                }
            }
        }
        if (view != null) {
            Composer(
                view = view,
                replying = replying,
                editing = editing,
                onDone = { replying = null; editing = null },
                onSignIn = { onDismiss(); navigator.open(settingsRoute(SettingsPane.Accounts)) },
            )
        }
    }
    deleting?.let { comment ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(stringResource(R.string.discussion_delete_title)) },
            text = { Text(stringResource(R.string.discussion_delete_body)) },
            confirmButton = { TextButton(onClick = { store.deleteComment(comment.id); deleting = null }) { Text(stringResource(R.string.action_delete)) } },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text(stringResource(R.string.action_cancel)) } },
        )
    }
}

/** "Episode 7 discussion", the show beneath it in its own face, the count and Open on AniList: final from the first frame. */
@Composable
private fun Header(entry: Entry, episode: UInt, view: DiscussionView?) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val known = remember(entry.id, episode) { store.discussion.count(entry.id, episode) }
    val count = view?.commentCount ?: known
    Row(Modifier.fillMaxWidth().padding(start = 24.dp, end = 12.dp, top = 4.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(
                stringResource(R.string.discussion_title_episode, episode.toInt()),
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.semantics { heading() },
            )
            Text(entry.showTitle, style = ShowTitle.row, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                if (count != null) pluralStringResource(R.plurals.discussion_count, count.toInt(), count.toInt()) else "",
                style = MaterialTheme.typography.bodyMedium.tabular(),
                color = colors.onSurfaceVariant,
                modifier = Modifier.heightIn(min = 20.dp),
            )
        }
        TextButton(
            onClick = { view?.url?.let(store::openExternal) },
            enabled = view != null,
            colors = androidx.compose.material3.ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
        ) {
            Text(stringResource(R.string.title_open_anilist))
        }
    }
}

/**
 * The comments are on their way: nothing for 600 ms, then placeholders at the comments' size, and
 * "Still loading from AniList" after 4 s. A pause AniList asked for keeps the placeholders and
 * counts down.
 */
@Composable
private fun Waiting(seconds: UInt?) {
    var stage by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) {
        delay(600)
        stage = 1
        delay(3_400)
        stage = 2
    }
    if (stage == 0 && seconds == null) return
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
        repeat(4) {
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Box(Modifier.size(32.dp).background(MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f), CircleShape))
                Column(Modifier.weight(1f)) {
                    TextPlaceholder(0.3f, 20.dp)
                    TextPlaceholder(0.9f, 22.dp)
                    TextPlaceholder(0.6f, 22.dp)
                }
            }
        }
        val line = when {
            seconds != null -> Strings.discussionWaiting(LocalContext.current, "${seconds} s")
            stage >= 2 -> stringResource(R.string.discussion_still_loading)
            else -> null
        }
        if (line != null) Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** A state in the content's place: a title, what it means, and the one way forward. */
@Composable
private fun Message(title: String, line: String?, action: String?, onAction: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 32.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        if (line != null) Text(line, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        if (action != null) TextButton(onClick = onAction) { Text(action) }
    }
}

@Composable
private fun Comments(
    view: DiscussionView,
    list: androidx.compose.foundation.lazy.LazyListState,
    revealed: MutableMap<String, Boolean>,
    offlineAge: UInt?,
    onReply: (CommentView) -> Unit,
    onEdit: (CommentView) -> Unit,
    onDelete: (CommentView) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    LazyColumn(state = list, contentPadding = PaddingValues(bottom = 8.dp)) {
        if (offlineAge != null) {
            item(key = "offline") {
                Text(
                    Strings.discussionOfflineCached(LocalContext.current, ageWords(LocalContext.current, offlineAge)),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
                )
            }
        }
        items(view.comments, key = { it.id.toLong() }, contentType = { "comment" }) { comment ->
            Column {
                CommentRow(comment, view, depth = 0, parent = null, revealed, onReply, onEdit, onDelete)
                // Hairlines only between top-level comments, inset to the text column.
                Box(Modifier.padding(start = 68.dp, end = 24.dp).fillMaxWidth().height(1.dp).background(colors.outlineVariant.copy(alpha = 0.5f)))
            }
        }
    }
}

private fun ageWords(context: android.content.Context, minutes: UInt): String = when {
    minutes < 60u -> Strings.discussionAgeMinutes(context, minutes.toInt())
    minutes < 1_440u -> Strings.discussionAgeHours(context, (minutes / 60u).toInt())
    else -> Strings.discussionAgeDays(context, (minutes / 1_440u).toInt())
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun CommentRow(
    comment: CommentView,
    view: DiscussionView,
    depth: Int,
    parent: CommentView?,
    revealed: MutableMap<String, Boolean>,
    onReply: (CommentView) -> Unit,
    onEdit: (CommentView) -> Unit,
    onDelete: (CommentView) -> Unit,
) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val clipboard = LocalClipboard.current
    val scope = rememberCoroutineScope()
    val avatar = if (depth == 0) 32.dp else 24.dp
    var menu by remember { mutableStateOf(false) }
    var more by remember(comment.id) { mutableStateOf(false) }
    val pending = comment.pending != PostState.POSTED
    Row(
        Modifier
            .fillMaxWidth()
            .padding(start = if (depth == 0) 24.dp else 0.dp, end = 24.dp, top = 12.dp, bottom = 4.dp)
            .alpha(if (pending) 0.5f else 1f)
            .combinedClickable(onClick = {}, onLongClick = { menu = true }),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        val tint = avatarTint(comment.author)
        // The picture comes over the initial as it arrives; until then, and without one, the initial stands.
        val arrived = remember(comment.avatar) { comment.avatar?.let(store.artwork::arrivals) }
        arrived?.collectAsStateWithLifecycle()?.value
        val pictured = comment.avatar != null && store.artwork.image(comment.avatar!!) != null
        Box(Modifier.size(avatar).clip(CircleShape).background(tint), contentAlignment = Alignment.Center) {
            if (comment.avatar != null) ArtworkImage(comment.avatar, tint, 96, Modifier.size(avatar))
            if (!pictured) Text(comment.author.take(1).uppercase(), style = MaterialTheme.typography.labelMedium, color = colors.onSurface)
        }
        Column(Modifier.weight(1f)) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(comment.author, style = MaterialTheme.typography.bodyMedium.copy(fontWeight = FontWeight.Medium), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                Text(
                    when (comment.pending) {
                        PostState.SENDING -> stringResource(R.string.discussion_sending)
                        else -> comment.age.text(context)
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.onSurfaceVariant,
                )
                if (depth > 1 && parent != null) {
                    Text(parent.author, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, maxLines = 1)
                }
            }
            Body(comment, view, revealed)
            if (comment.pending == PostState.FAILED) {
                Text(
                    stringResource(R.string.discussion_post_failed),
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.clickable { store.postComment(comment.markdown, parent?.id) }.padding(vertical = 4.dp),
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                val liked = comment.liked
                Row(
                    Modifier
                        .heightIn(min = 36.dp)
                        .clickable(onClickLabel = stringResource(R.string.discussion_like)) { store.likeComment(comment.id) }
                        .padding(end = 16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Icon(
                        if (liked) Icons.HeartFilled else Icons.Heart,
                        contentDescription = stringResource(R.string.discussion_like),
                        tint = if (liked) colors.onSurface else colors.onSurfaceVariant,
                        modifier = Modifier.size(18.dp),
                    )
                    RollingCount(comment.likeCount.toInt(), MaterialTheme.typography.labelMedium.tabular(), if (liked) colors.onSurface else colors.onSurfaceVariant)
                }
                if (view.canPost) {
                    TextButton(onClick = { onReply(comment) }, contentPadding = PaddingValues(horizontal = 4.dp)) {
                        Text(stringResource(R.string.discussion_reply), style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant)
                    }
                }
                Spacer(Modifier.weight(1f))
                if (comment.own) {
                    Box {
                        IconButton(onClick = { menu = true }, modifier = Modifier.size(36.dp)) {
                            Icon(Icons.More, contentDescription = stringResource(R.string.action_more), tint = colors.onSurfaceVariant, modifier = Modifier.size(18.dp))
                        }
                    }
                }
                Menu(expanded = menu, onDismissRequest = { menu = false }) {
                    if (view.canPost) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.discussion_reply)) }, onClick = { menu = false; onReply(comment) })
                    }
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.discussion_copy_text)) },
                        onClick = {
                            menu = false
                            scope.launch { clipboard.setClipEntry(androidx.compose.ui.platform.ClipEntry(ClipData.newPlainText("", plain(comment)))) }
                        },
                    )
                    DropdownMenuItem(text = { Text(stringResource(R.string.title_open_anilist)) }, onClick = { menu = false; store.openExternal(view.url) })
                    if (comment.own) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.action_edit)) }, onClick = { menu = false; onEdit(comment) })
                        DropdownMenuItem(text = { Text(stringResource(R.string.action_delete)) }, onClick = { menu = false; onDelete(comment) })
                    }
                }
            }
            // Replies step in to the text column; deeper ones are flattened at the reply indent.
            val replies = if (depth == 0) comment.replies else if (more) comment.replies else emptyList()
            for (reply in replies) {
                CommentRow(reply, view, depth = depth + 1, parent = comment, revealed, onReply, onEdit, onDelete)
            }
            if (depth > 0 && comment.hiddenReplies > 0u && !more) {
                TextButton(onClick = { more = true }, contentPadding = PaddingValues(horizontal = 0.dp)) {
                    Text(pluralStringResource(R.plurals.discussion_more_replies, comment.hiddenReplies.toInt(), comment.hiddenReplies.toInt()), style = MaterialTheme.typography.labelMedium)
                }
            }
        }
    }
}

/** A calm tint for a name's initial, the same name always the same one. */
private fun avatarTint(name: String): Color {
    val palette = listOf(0xFF4F6382, 0xFF6E5470, 0xFF7A6A45, 0xFF5A6878)
    return Color(palette[(name.hashCode() and 0x7fffffff) % palette.size])
}

/** The comment's words as plain text, for Copy. */
private fun plain(comment: CommentView): String =
    comment.body.joinToString("\n\n") { block -> block.spans.joinToString("") { it.text } } // i18n-skip

/** The text of a comment: its paragraphs, folded past ten lines behind Show more. */
@Composable
private fun Body(comment: CommentView, view: DiscussionView, revealed: MutableMap<String, Boolean>) {
    val colors = MaterialTheme.colorScheme
    var expanded by remember(comment.id) { mutableStateOf(false) }
    var overflows by remember(comment.id) { mutableStateOf(false) }
    Column(
        Modifier
            .fillMaxWidth()
            .foldAt(LINE * FOLD_LINES, expanded) { overflows = it },
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        comment.body.forEachIndexed { index, block -> Block(comment.id.toInt(), index, block, view, revealed) }
    }
    if (overflows && !expanded) {
        TextButton(onClick = { expanded = true }, contentPadding = PaddingValues(horizontal = 0.dp)) {
            Text(stringResource(R.string.discussion_show_more), style = MaterialTheme.typography.labelMedium)
        }
    }
}

/**
 * Cuts its content at [max] unless [open], and says whether it is cut: the content is measured at
 * its natural height, then laid out clipped.
 */
private fun Modifier.foldAt(max: Dp, open: Boolean, onOverflow: (Boolean) -> Unit): Modifier =
    layout { measurable, constraints ->
        val placeable = measurable.measure(constraints.copy(maxHeight = androidx.compose.ui.unit.Constraints.Infinity))
        val cap = max.roundToPx()
        val height = if (open) placeable.height else minOf(placeable.height, cap)
        onOverflow(placeable.height > cap)
        layout(placeable.width, height) { placeable.place(0, 0) }
    }.clipToBounds()

@Composable
private fun Block(commentId: Int, index: Int, block: CommentBlock, view: DiscussionView, revealed: MutableMap<String, Boolean>) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val onlyMedia = block.spans.isNotEmpty() && block.spans.all { it.media != null }
    if (onlyMedia) {
        Column {
            for (span in block.spans) {
                val link = span.link ?: continue
                val host = remember(link) { Uri.parse(link).host.orEmpty() }
                if (span.media == MediaKind.IMAGE && view.loadImages) {
                    ArtworkImage(link, colors.surfaceContainerHighest, 960, Modifier.fillMaxWidth().heightIn(max = 240.dp).clip(RoundedCornerShape(8.dp)))
                } else {
                    Text(
                        if (span.media == MediaKind.IMAGE) Strings.discussionImageHost(context, host) else Strings.discussionVideoHost(context, host),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                        modifier = Modifier.clickable { store.openExternal(link) }.padding(vertical = 4.dp),
                    )
                }
            }
        }
        return
    }
    val text = buildAnnotatedString {
        block.spans.forEachIndexed { spanIndex, span ->
            val key = "$commentId/$index/$spanIndex"
            val style = SpanStyle(
                fontWeight = if (span.bold) FontWeight.SemiBold else null,
                fontStyle = if (span.italic) FontStyle.Italic else null,
                textDecoration = if (span.strike) TextDecoration.LineThrough else null,
            )
            when {
                // A spoiler is a bar the length of its words until it is opened: Raised behind
                // transparent text, so nothing reflows when it opens.
                span.spoiler && revealed[key] != true -> withLink(
                    LinkAnnotation.Clickable(key, TextLinkStyles(SpanStyle(color = Color.Transparent, background = colors.surfaceContainerHighest))) { revealed[key] = true },
                ) { append(span.text) }
                span.link != null -> withLink(LinkAnnotation.Url(span.link!!, TextLinkStyles(SpanStyle(color = colors.primary, textDecoration = TextDecoration.Underline)))) {
                    withStyle(style) { append(span.text) }
                }
                else -> withStyle(style) { append(span.text) }
            }
        }
    }
    if (block.quote) {
        Row(Modifier.fillMaxWidth().heightIn(min = 22.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Box(Modifier.width(1.dp).fillMaxHeight().background(colors.outline))
            Text(text, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
    } else {
        Text(text, style = MaterialTheme.typography.bodyLarge)
    }
}

/** The field and Post, or what stands in their place: sign in, or posting waiting for the network. */
@Composable
private fun Composer(view: DiscussionView, replying: CommentView?, editing: CommentView?, onDone: () -> Unit, onSignIn: () -> Unit) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        when {
            !view.signedIn -> OutlinedButton(onClick = onSignIn, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.discussion_sign_in)) }
            !view.canPost -> Text(stringResource(R.string.discussion_offline_post), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(8.dp))
            else -> {
                var field by remember(replying?.id, editing?.id) { mutableStateOf(TextFieldValue(editing?.markdown.orEmpty(), TextRange(editing?.markdown?.length ?: 0))) }
                val target = editing ?: replying
                if (target != null) {
                    Row(
                        Modifier.fillMaxWidth().background(colors.surfaceContainerHighest, RoundedCornerShape(12.dp)).padding(start = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            if (editing != null) stringResource(R.string.discussion_edit_comment) else stringResource(R.string.discussion_replying_to, target.author),
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSurfaceVariant,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(onClick = onDone) {
                            Icon(Icons.Clear, contentDescription = stringResource(R.string.discussion_cancel_reply), modifier = Modifier.size(18.dp))
                        }
                    }
                    Spacer(Modifier.height(6.dp))
                }
                Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextField(
                        value = field,
                        onValueChange = { field = it },
                        modifier = Modifier.weight(1f),
                        placeholder = { Text(stringResource(R.string.discussion_add_comment)) },
                        maxLines = 5,
                        shape = RoundedCornerShape(24.dp),
                        colors = androidx.compose.material3.TextFieldDefaults.colors(focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent),
                    )
                    val ready = field.text.isNotBlank()
                    Button(
                        onClick = {
                            val text = field.text.trim()
                            if (editing != null) store.editComment(editing.id, text) else store.postComment(text, replying?.id)
                            field = TextFieldValue("")
                            onDone()
                        },
                        enabled = ready,
                        modifier = Modifier.heightIn(min = 48.dp),
                    ) { Text(stringResource(if (editing != null) R.string.action_save else R.string.discussion_post)) }
                }
                // Spoiler wraps the selection in AniList's marks.
                if (field.selection.length > 0) {
                    TextButton(onClick = {
                        val s = field.selection
                        val t = field.text
                        field = TextFieldValue(t.substring(0, s.min) + "~!" + t.substring(s.min, s.max) + "!~" + t.substring(s.max), TextRange(s.max + 4))
                    }) { Text(stringResource(R.string.discussion_spoiler)) }
                }
            }
        }
    }
}
