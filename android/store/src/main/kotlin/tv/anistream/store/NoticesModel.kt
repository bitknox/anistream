// Notices: what the work tells the user beyond the answer it was asked for.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.model.Notice
import tv.anistream.engine.model.NoticeKind

/** One notice as the app keeps it: when it came, and a number to tell it from one with the same words. */
data class NoticeItem(val id: Int, val notice: Notice, val at: Long) {
    val isAlert: Boolean get() = notice.kind == NoticeKind.ALERT
}

/** The snackbar's current notice, and the log of every notice this session. */
class NoticesModel(private val clock: () -> Long = System::currentTimeMillis) {
    private val _current = MutableStateFlow<NoticeItem?>(null)

    /** What the snackbar shows, until it is dismissed or another arrives. */
    val current: StateFlow<NoticeItem?> = _current.asStateFlow()

    private val _log = MutableStateFlow<List<NoticeItem>>(emptyList())

    /** Every notice, newest first. */
    val log: StateFlow<List<NoticeItem>> = _log.asStateFlow()

    private val _status = MutableStateFlow<String?>(null)

    /** The engine's status line, when it last set one. */
    val status: StateFlow<String?> = _status.asStateFlow()

    private var count = 0

    internal fun post(notice: Notice) {
        count += 1
        val item = NoticeItem(count, notice, clock())
        _current.value = item
        _log.value = (listOf(item) + _log.value).take(KEPT)
    }

    internal fun status(text: String) {
        _status.value = text.ifEmpty { null }
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
