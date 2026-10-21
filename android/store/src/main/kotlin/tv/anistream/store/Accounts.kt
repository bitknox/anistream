// Accounts and plugin stores, as far as the first screens need them: whether a tracker is
// connected, and which stores are joined.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.anistream_model.StoreRow
import uniffi.anistream_model.SyncState

/** Each tracker's sync, as the engine last said it. */
class AccountsModel {
    private val _trackers = MutableStateFlow<Map<String, SyncState>>(emptyMap())

    /** By tracker id: `anilist`, `mal`, `kitsu`, `simkl`, `trakt`. */
    val trackers: StateFlow<Map<String, SyncState>> = _trackers.asStateFlow()

    /** The first connected tracker's user name, for the account button. */
    val signedIn: SyncState? get() = _trackers.value.values.firstOrNull { it.connected }

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
