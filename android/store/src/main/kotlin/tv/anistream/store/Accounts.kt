// Accounts, plugin stores and downloads, as far as the browse screens need them: whether a
// tracker is connected, which stores are joined, and what the downloads take.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.JobId
import tv.anistream.engine.model.StoreRow
import tv.anistream.engine.model.SyncState
import tv.anistream.engine.model.TitleLanguage

/** Each tracker's sync, as the engine last said it. */
class AccountsModel(private val clock: () -> Long = { System.currentTimeMillis() }) {
    private val _trackers = MutableStateFlow<Map<String, SyncState>>(emptyMap())

    /** By tracker id: `anilist`, `mal`, `kitsu`, `simkl`, `trakt`. */
    val trackers: StateFlow<Map<String, SyncState>> = _trackers.asStateFlow()

    /** The first connected tracker's user name, for the account button. */
    val signedIn: SyncState? get() = _trackers.value.values.firstOrNull { it.connected }

    /**
     * Whether no tracker is signed in, once the engine has said the trackers at all, which it
     * does at launch: before that, `false`, so a screen does not flash its signed-out state.
     */
    fun signedOut(trackers: Map<String, SyncState>): Boolean =
        trackers.isNotEmpty() && trackers.values.none { it.connected }

    private val _syncedAt = MutableStateFlow<Map<String, Long>>(emptyMap())

    /**
     * When each tracker last said a sync went through with nothing left to send, in milliseconds
     * on [clock]: "synced 2 min ago".
     */
    val syncedAt: StateFlow<Map<String, Long>> = _syncedAt.asStateFlow()

    private val _viewerTitleLanguage = MutableStateFlow<TitleLanguage?>(null)

    /**
     * The title language the signed-in viewer set on AniList, once asked (`LoadTitleLanguage`):
     * `null` signed out, or when AniList did not say.
     */
    val viewerTitleLanguage: StateFlow<TitleLanguage?> = _viewerTitleLanguage.asStateFlow()

    private val _working = MutableStateFlow<Map<JobId, String>>(emptyMap())

    /** The sign-ins and syncs under way, by job: the tracker each is for, or `sync`. */
    val working: StateFlow<Map<JobId, String>> = _working.asStateFlow()

    internal fun apply(state: SyncState) {
        _trackers.value = _trackers.value + (state.tracker to state)
        if (!state.connected) {
            _syncedAt.value = _syncedAt.value - state.tracker
        } else if (state.last != null && state.outbox == 0u && state.pullError == null) {
            _syncedAt.value = _syncedAt.value + (state.tracker to clock())
        }
    }

    internal fun viewerTitleLanguage(language: TitleLanguage?) {
        _viewerTitleLanguage.value = language
    }

    internal fun started(tracker: String, job: JobId?) {
        job ?: return
        _working.value = _working.value + (job to tracker)
    }

    internal fun done(job: JobId) {
        if (job in _working.value) _working.value = _working.value - job
    }

    /** Whether a sign-in or a sync for [tracker] is under way. */
    fun isWorking(working: Map<JobId, String>, tracker: String): Boolean = tracker in working.values

    companion object {
        /** The trackers, in the order the Accounts page lists them: AniList, then the others. */
        val order = listOf("anilist", "mal", "kitsu", "simkl", "trakt")

        /** A tracker's name as a person writes it. */
        fun name(tracker: String): String = when (tracker) {
            "anilist" -> "AniList"
            "mal", "myanimelist" -> "MyAnimeList"
            "kitsu" -> "Kitsu"
            "simkl" -> "Simkl"
            "trakt" -> "Trakt"
            else -> tracker.replaceFirstChar { it.uppercase() }
        }
    }
}

/** The plugin stores, as the engine last said them. */
class StoresModel {
    private val _stores = MutableStateFlow<List<StoreRow>>(emptyList())

    /** Every joined store, in the order the engine lists them. */
    val stores: StateFlow<List<StoreRow>> = _stores.asStateFlow()

    private val _loaded = MutableStateFlow(false)

    /** Whether the engine has said the stores at all. */
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    internal fun apply(rows: List<StoreRow>) {
        _stores.value = rows
        _loaded.value = true
    }
}

/** The downloads, as the engine last said them: the Library's Downloads row reads their size. */
class DownloadsModel {
    private val _rows = MutableStateFlow<List<DownloadRow>>(emptyList())
    val rows: StateFlow<List<DownloadRow>> = _rows.asStateFlow()

    private val _loaded = MutableStateFlow(false)
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    /** The bytes the downloads take on this device. */
    fun bytes(rows: List<DownloadRow>): Long = rows.sumOf { it.downloaded.toLong() }

    internal fun apply(rows: List<DownloadRow>) {
        _rows.value = rows
        _loaded.value = true
    }
}
