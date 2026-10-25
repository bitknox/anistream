// Android TV's Watch Next row: the app writes one program per show from the engine's Continue
// queue, under Google's rules, and keeps only what changed in step.

package tv.anistream.tv.home

import android.annotation.SuppressLint
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.core.content.edit
import androidx.core.net.toUri
import androidx.tvprovider.media.tv.TvContractCompat
import androidx.tvprovider.media.tv.WatchNextProgram
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.EpisodeRow
import tv.anistream.store.AppStore
import tv.anistream.store.ContinueQueue

/** What kind of entry a show gets on the row. */
enum class WatchNextKind(val type: Int) {
    /** An episode stopped part-way. */
    Continue(TvContractCompat.WatchNextPrograms.WATCH_NEXT_TYPE_CONTINUE),

    /** The next episode, once the last one was finished. */
    Next(TvContractCompat.WatchNextPrograms.WATCH_NEXT_TYPE_NEXT),

    /** A new episode of a show you were caught up on. */
    New(TvContractCompat.WatchNextPrograms.WATCH_NEXT_TYPE_NEW),
}

/** One show's entry on the row. */
data class WatchNextCard(
    val id: AnilistId,
    val episode: UInt,
    val kind: WatchNextKind,
    val title: String,
    /** "Episode 8 · A Town Without Night", or "Episode 8" before its title is known. */
    val description: String,
    val art: String?,
    val positionMillis: Long,
    val durationMillis: Long,
) {
    /** What the entry shows, so an unchanged one is not written again. */
    val signature: String get() = listOf(episode, kind, title, description, art, positionMillis / 60_000).joinToString("|")

    companion object {
        /** Under two minutes in, the episode has hardly started: it is still the next one. */
        const val STARTED_SECS = 120.0

        /** With three minutes or fewer left, the episode counts as finished. */
        const val ENDING_MINUTES = 3

        /** The row from the Continue [queue]: one entry per show with an episode out to play. */
        fun of(queue: ContinueQueue, episodes: Map<AnilistId, List<EpisodeRow>>): List<WatchNextCard> =
            queue.all.filter { it.playable }.map { item ->
                val resume = item.resume
                val started = resume != null && resume.position > STARTED_SECS &&
                    (item.minutesLeft ?: Int.MAX_VALUE) > ENDING_MINUTES
                val kind = when {
                    started -> WatchNextKind.Continue
                    item.isNew -> WatchNextKind.New
                    else -> WatchNextKind.Next
                }
                val row = episodes[item.id]?.firstOrNull { it.number == item.episode.toString() }
                val duration = resume?.fraction?.takeIf { it > 0 }?.let { resume.position / it }
                    ?: row?.durationSecs?.toDouble()
                WatchNextCard(
                    id = item.id,
                    episode = item.episode,
                    kind = kind,
                    title = item.entry.title,
                    description = listOfNotNull(item.longEpisode, row?.title).joinToString(" · "),
                    art = item.entry.bannerUrl ?: item.entry.coverUrl,
                    positionMillis = if (started) (resume.position * 1000).toLong() else 0L,
                    durationMillis = ((duration ?: 0.0) * 1000).toLong(),
                )
            }
    }
}

/** The link an entry opens: the player on that episode, with the title page under it. */
fun playLink(id: AnilistId, episode: UInt): String = "$SCHEME://play/$id/$episode"

/** The title and episode a [playLink] names, if it is one. */
fun parsePlayLink(link: String?): Pair<AnilistId, UInt>? {
    val uri = link?.toUri() ?: return null
    if (uri.scheme != SCHEME || uri.host != "play") return null
    val parts = uri.pathSegments
    if (parts.size != 2) return null
    val id = parts[0].toUIntOrNull() ?: return null
    val episode = parts[1].toUIntOrNull() ?: return null
    return id to episode
}

private const val SCHEME = "anistream-tv"

/** The row's writer, for This TV's switch; `null` in a composition with none, as in the tests. */
val LocalWatchNext = staticCompositionLocalOf<WatchNext?> { null }

/**
 * Keeps the row in step with the Continue queue, a few seconds after it settles. "Show anistream
 * on the home screen" off, it removes every entry it wrote. Fire OS reads none of this, so there
 * it writes nothing.
 */
class WatchNext(
    private val context: Context,
    private val store: AppStore,
    private val scope: CoroutineScope,
    private val activity: ComponentName,
) {
    private val prefs = context.getSharedPreferences("watch-next", Context.MODE_PRIVATE)

    /** Whether the row is written: This TV's switch, on by default. */
    var enabled by mutableStateOf(prefs.getBoolean(ENABLED, true))
        private set

    /** Turn the row on or off; off removes every entry written. */
    fun enable(on: Boolean) {
        enabled = on
        prefs.edit { putBoolean(ENABLED, on) }
        scope.launch { withContext(Dispatchers.IO) { write(if (on) latest else emptyList()) } }
    }

    /** The row as the queue last gave it, for turning it back on. */
    @Volatile
    private var latest: List<WatchNextCard> = emptyList()

    private val supported: Boolean
        get() = !context.packageManager.hasSystemFeature(FIRE_TV)

    @OptIn(FlowPreview::class)
    fun start() {
        if (!supported) return
        scope.launch {
            store.phase.first { it == AppStore.Phase.Running }
            store.loadContinue()
            combine(
                store.titles.entries,
                store.continuing.list.state,
                store.continuing.hidden,
                store.titles.episodes,
            ) { entries, _, _, episodes -> WatchNextCard.of(store.continuing.queue(entries), episodes) }
                .distinctUntilChanged()
                .debounce(SETTLE_MILLIS)
                .collect { cards ->
                    latest = cards
                    if (enabled) withContext(Dispatchers.IO) { write(cards) }
                }
        }
    }

    /** Make the row hold [cards]: insert the new, update the changed, delete the gone. */
    @SuppressLint("RestrictedApi")
    @Synchronized
    private fun write(cards: List<WatchNextCard>) {
        val resolver = context.contentResolver
        val written = prefs.all.filterKeys { it.startsWith(ROW) }
        val keep = cards.map { ROW + it.id }.toSet()
        try {
            for ((key, value) in written) {
                if (key in keep) continue
                val program = (value as? String)?.substringBefore('|')?.toLongOrNull() ?: continue
                resolver.delete(TvContractCompat.buildWatchNextProgramUri(program), null, null)
                prefs.edit { remove(key) }
            }
            for (card in cards) {
                val key = ROW + card.id
                val old = written[key] as? String
                if (old?.substringAfter('|') == card.signature) continue
                val values = program(card).toContentValues()
                val existing = old?.substringBefore('|')?.toLongOrNull()
                val updated = existing != null &&
                    resolver.update(TvContractCompat.buildWatchNextProgramUri(existing), values, null, null) > 0
                val id = if (updated) {
                    existing
                } else {
                    resolver.insert(TvContractCompat.WatchNextPrograms.CONTENT_URI, values)?.lastPathSegment?.toLongOrNull()
                }
                if (id != null) prefs.edit { putString(key, "$id|${card.signature}") }
            }
        } catch (e: RuntimeException) {
            // A launcher without the provider, or one that refuses the write: the row is optional.
            Log.w(TAG, "Watch Next not written", e)
        }
    }

    // The setters are BasePreviewProgram.Builder's, restricted to the library only so that
    // subclasses return their own type: calling them through WatchNextProgram.Builder is the API.
    @SuppressLint("RestrictedApi")
    private fun program(card: WatchNextCard): WatchNextProgram {
        val intent = Intent(Intent.ACTION_VIEW, playLink(card.id, card.episode).toUri()).setComponent(activity)
        return WatchNextProgram.Builder()
            .setType(TvContractCompat.WatchNextPrograms.TYPE_TV_EPISODE)
            .setWatchNextType(card.kind.type)
            .setLastEngagementTimeUtcMillis(System.currentTimeMillis())
            .setTitle(card.title)
            .setDescription(card.description)
            .setEpisodeNumber(card.episode.toInt())
            .setPosterArtAspectRatio(TvContractCompat.PreviewPrograms.ASPECT_RATIO_16_9)
            .apply { card.art?.let { setPosterArtUri(it.toUri()) } }
            .setLastPlaybackPositionMillis(card.positionMillis.toInt())
            .setDurationMillis(card.durationMillis.toInt())
            .setInternalProviderId(card.id.toString())
            .setIntentUri(intent.toUri(Intent.URI_INTENT_SCHEME).toUri())
            .build()
    }

    companion object {
        const val TAG = "anistream-watch-next"

        /** How long the queue rests before the row follows it: within the five seconds Google asks. */
        const val SETTLE_MILLIS = 3_000L
        private const val ENABLED = "enabled"
        private const val ROW = "row/"
        private const val FIRE_TV = "amazon.hardware.fire_tv"
    }
}
