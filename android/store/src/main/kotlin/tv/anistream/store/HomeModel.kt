// Home: the engine's one view of the spotlight and the shelves, as it last sent it.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.core.AnilistId
import tv.anistream.engine.model.HiddenTitle
import tv.anistream.engine.model.HomeItem
import tv.anistream.engine.model.HomeSection
import tv.anistream.engine.model.HomeView
import tv.anistream.engine.model.Playable
import tv.anistream.engine.model.SectionKind

/**
 * Home as the engine sends it, whole each time: the spotlight and the sections in order. The
 * apps draw it as it is; nothing here decides what goes where.
 */
class HomeModel {
    private val _view = MutableStateFlow<HomeView?>(null)

    /** Home as it stands; `null` until the engine's first send. */
    val view: StateFlow<HomeView?> = _view.asStateFlow()

    private val _playable = MutableStateFlow<Map<AnilistId, Playable>>(emptyMap())

    /** What the sources check found since Home was last sent, by title. */
    val playable: StateFlow<Map<AnilistId, Playable>> = _playable.asStateFlow()

    private val _failures = MutableStateFlow(0)

    /** How many sends reported a part that did not load: one notice each, with Try again. */
    val failures: StateFlow<Int> = _failures.asStateFlow()

    private val _hidden = MutableStateFlow<List<HiddenTitle>?>(null)

    /** The titles hidden with Not interested, most recent first; `null` until asked for. */
    val hidden: StateFlow<List<HiddenTitle>?> = _hidden.asStateFlow()

    internal fun apply(view: HomeView, titles: TitleCache) {
        val entries = buildList {
            view.spotlight?.let { add(it.entry) }
            for (section in view.sections) for (item in section.items) add(item.entry)
        }
        titles.merge(entries)
        _view.value = view
        // The send says what the check found for every title on it.
        _playable.value = emptyMap()
        if (view.failed) _failures.value += 1
    }

    internal fun playable(id: AnilistId, playable: Playable) {
        _playable.value = _playable.value + (id to playable)
    }

    internal fun hidden(titles: List<HiddenTitle>, cache: TitleCache) {
        cache.merge(titles.map { it.entry })
        _hidden.value = titles
    }

    /** Whether the sources check found nothing for [id], the only answer Home shows. */
    fun notFound(id: AnilistId, sent: Playable): Boolean = (_playable.value[id] ?: sent) == Playable.NOT_FOUND
}

/** The section of [kind], when Home has one. */
fun HomeView.section(kind: SectionKind): HomeSection? = sections.firstOrNull { it.kind == kind }

/** The items of [section] without the titles in [gone], the ones swiped away and not yet resent. */
fun HomeSection.without(gone: Set<AnilistId>): List<HomeItem> =
    if (gone.isEmpty()) items else items.filterNot { it.entry.id in gone }
