// Where streams come from, as Providers and Settings show it: each provider, why there are none,
// the connection check's verdict, and the order failover walks them in.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.ProviderRow

/** The providers, as the engine last said them. */
class ProvidersModel(private val clock: () -> Long = { AppClock.now().toEpochMilli() }) {
    private val _rows = MutableStateFlow<List<ProviderRow>>(emptyList())

    /** Each provider, in the order the engine lists them. */
    val rows: StateFlow<List<ProviderRow>> = _rows.asStateFlow()

    private val _loaded = MutableStateFlow(false)

    /** Whether the engine has said the providers at all. */
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    private val _note = MutableStateFlow<String?>(null)

    /** Why there are no sources, when there are none. */
    val note: StateFlow<String?> = _note.asStateFlow()

    private val _rebuilding = MutableStateFlow(false)

    /** Whether the sources are being rebuilt, from `SourcesChanged` until the rebuilt list. */
    val rebuilding: StateFlow<Boolean> = _rebuilding.asStateFlow()

    /** The connection check's verdict for torrents, and when it was said, on [clock]. */
    data class Check(val view: GuardView, val at: Long)

    private val _check = MutableStateFlow<Check?>(null)

    /** The connection check's last verdict; `null` while torrents are off or before it said. */
    val check: StateFlow<Check?> = _check.asStateFlow()

    /** A protection checked before it is saved: under way, then the check's verdict on it. */
    sealed interface Trial {
        data object Checking : Trial

        data class Said(val view: GuardView) : Trial
    }

    private val _trial = MutableStateFlow<Trial?>(null)

    /** The check Edit proxy asked for; `null` until it asks, and once it is done with it. */
    val trial: StateFlow<Trial?> = _trial.asStateFlow()

    internal fun trialStarted() {
        _trial.value = Trial.Checking
    }

    internal fun trialSaid(view: GuardView) {
        // A verdict no form is waiting for any more is not kept.
        if (_trial.value == Trial.Checking) _trial.value = Trial.Said(view)
    }

    /** Done with the check: its form closed, or it started again. */
    fun clearTrial() {
        _trial.value = null
    }

    private val _preference = MutableStateFlow<String?>(null)

    /**
     * The source pinned for the title last asked about (`ProviderPreference`), `null` for
     * automatic: what a show's Source row stands on.
     */
    val preference: StateFlow<String?> = _preference.asStateFlow()

    internal fun preference(provider: String?) {
        _preference.value = provider
    }

    internal fun apply(rows: List<ProviderRow>) {
        _rows.value = rows
        _loaded.value = true
        _rebuilding.value = false
        if (rows.isNotEmpty()) _note.value = null
    }

    internal fun note(text: String) {
        _note.value = text
    }

    internal fun sourcesChanged() {
        _rebuilding.value = true
    }

    internal fun guardSaid(view: GuardView?) {
        _check.value = view?.let { Check(it, clock()) }
    }

    fun row(id: String): ProviderRow? = _rows.value.firstOrNull { it.id == id }
}

/** How a provider stands, as the Providers list says it beside the name. */
enum class ProviderStanding { Working, Slow, Failing, HeldBack }

val ProviderRow.standing: ProviderStanding
    get() = when {
        heldBack -> ProviderStanding.HeldBack
        !usable || state == "down" -> ProviderStanding.Failing
        state == "degraded" -> ProviderStanding.Slow
        else -> ProviderStanding.Working
    }

/** Whether torrents may run: the check passed, or the config accepts no protection. */
val GuardView.passes: Boolean
    get() = this is GuardView.Protected || this == GuardView.Unprotected

/**
 * Where streams come from, as Providers moves them: ports of `anistream_model::sources`'s order
 * rules, so a drag writes the order the terminal's keys would.
 */
object SourceOrder {
    /**
     * [order] with the source [id] moved one place up the chain ([step] below zero) or down it,
     * or `null` when it is already at that end or has no row. `order_with_source_moved`.
     *
     * One place is past the next row on screen. An entry with no row, `"plugins"`, torrents while
     * they are off, a plugin named but not installed, is stepped over and kept. `"plugins"` is
     * first spelled out as the plugins on screen it stands for, and stays right after them.
     */
    fun moved(order: List<String>, rows: List<ProviderRow>, id: String, step: Int): List<String>? {
        val spelled = order.toMutableList()
        val unnamed = rows.filter { it.kind == "plugin" && it.id !in spelled }.map { it.id }
        val at = spelled.indexOf("plugins").takeIf { it >= 0 } ?: spelled.size
        spelled.addAll(at, unnamed)
        val from = spelled.indexOf(id).takeIf { it >= 0 } ?: return null
        val shown = { entry: String -> entry != "plugins" && rows.any { it.id == entry } }
        val to = if (step < 0) {
            (from - 1 downTo 0).firstOrNull { shown(spelled[it]) } ?: return null
        } else {
            (from + 1 until spelled.size).firstOrNull { shown(spelled[it]) } ?: return null
        }
        val entry = spelled.removeAt(from)
        spelled.add(to, entry)
        return spelled
    }

    /**
     * [order] with [id] moved to where the row at [target] is on screen: one place at a time, by
     * the rule above, so a drag writes what the same number of key presses would.
     */
    fun movedTo(order: List<String>, rows: List<ProviderRow>, id: String, target: Int): List<String>? {
        val from = rows.indexOfFirst { it.id == id }
        if (from < 0 || target == from || target !in rows.indices) return null
        var moved = order
        repeat(kotlin.math.abs(target - from)) {
            moved = moved(moved, rows, id, if (target < from) -1 else 1) ?: return moved
        }
        return moved
    }

    /**
     * The rows in the order failover tries them: by where each stands in [order], a plugin it does
     * not name where `"plugins"` stands, or last when that is not there either.
     */
    fun sorted(rows: List<ProviderRow>, order: List<String>): List<ProviderRow> {
        fun rank(row: ProviderRow): Int {
            val at = order.indexOf(row.id)
            if (at >= 0) return at
            if (row.kind == "plugin") order.indexOf("plugins").takeIf { it >= 0 }?.let { return it }
            return order.size
        }
        return rows.withIndex().sortedWith(compareBy({ rank(it.value) }, { it.index })).map { it.value }
    }
}
