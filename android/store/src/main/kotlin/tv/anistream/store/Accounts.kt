// Accounts, plugin stores and downloads, as far as the browse screens need them: whether a
// tracker is connected, which stores are joined, and what the downloads take.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.anistream_model.DownloadRow
import uniffi.anistream_model.StoreRow
import uniffi.anistream_model.SyncState

/** Each tracker's sync, as the engine last said it. */
class AccountsModel {
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

    internal fun apply(state: SyncState) {
        _trackers.value = _trackers.value + (state.tracker to state)
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
