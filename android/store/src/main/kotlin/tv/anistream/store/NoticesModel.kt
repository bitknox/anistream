// Notices: what the work tells the user beyond the answer it was asked for.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import android.content.Context
import tv.anistream.engine.model.Notice
import tv.anistream.engine.model.NoticeKind

/** One notice as the app keeps it: when it came, and a number to tell it from one with the same words. */
data class NoticeItem(val id: Int, val notice: Notice, val at: Long, val words: ((Context) -> String)? = null) {
    val isAlert: Boolean get() = notice.kind == NoticeKind.ALERT

    /** What it says, in the person's language: the app's own words when it made the notice, else the engine's message. */
    fun text(context: Context): String = words?.invoke(context) ?: notice.text(context)
}

/** What a notice says, in the person's language. */
fun Notice.text(context: Context): String = msg?.text(context) ?: text

/** The snackbar's current notice, and the log of every notice this session. */
class NoticesModel(private val clock: () -> Long = System::currentTimeMillis) {
    private val _current = MutableStateFlow<NoticeItem?>(null)

    /** What the snackbar shows, until it is dismissed or another arrives. */
    val current: StateFlow<NoticeItem?> = _current.asStateFlow()

    private val _log = MutableStateFlow<List<NoticeItem>>(emptyList())

    /** Every notice, newest first. */
    val log: StateFlow<List<NoticeItem>> = _log.asStateFlow()

    private var count = 0

    internal fun post(notice: Notice, words: ((Context) -> String)? = null) {
        count += 1
        val item = NoticeItem(count, notice, clock(), words)
        _current.value = item
        _log.value = (listOf(item) + _log.value).take(KEPT)
    }

    /** Keep [notice] in the log only: something on screen says it already. */
    internal fun log(notice: Notice) {
        count += 1
        _log.value = (listOf(NoticeItem(count, notice, clock())) + _log.value).take(KEPT)
    }

    /** Take the snackbar down, if it still shows [item]. */
    fun dismiss(item: NoticeItem) {
        if (_current.value?.id == item.id) _current.value = null
    }

    companion object {
        /** How many notices the log keeps. */
        const val KEPT = 200
    }
}
