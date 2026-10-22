// Accounts, plugin stores and downloads, as far as the browse screens need them: whether a
// tracker is connected, which stores are joined, and what the downloads take.

package tv.anistream.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.engine.model.ConflictRow
import tv.anistream.engine.model.DeviceCodePrompt
import tv.anistream.engine.model.DownloadRow
import tv.anistream.engine.model.DownloadState
import tv.anistream.engine.model.SeedRow
import tv.anistream.engine.model.InviteLinkView
import tv.anistream.engine.model.InvitePreview
import tv.anistream.engine.model.InvitedStore
import tv.anistream.engine.model.StorePluginRow
import tv.anistream.engine.model.StorePluginStatus
import tv.anistream.engine.model.JobId
import tv.anistream.engine.model.StoreRow
import tv.anistream.engine.model.SyncState
import tv.anistream.engine.model.TitleLanguage

/**
 * A sign-in the engine asked the browser for (`Event::Authorize`): open [url] in it, and hand
 * back the redirect it ends on, the one to [callbackScheme].
 */
data class SignInRequest(val tracker: String, val url: String, val callbackScheme: String) {
    /**
     * Whether [callback] is this request's own redirect: for its tracker, and carrying back the
     * `state` it asked with, in the query or the fragment, when it asked with one. The engine checks
     * the same, and goes on waiting for a redirect that is not.
     */
    fun answeredBy(tracker: String, callback: String): Boolean {
        if (tracker != this.tracker) return false
        val asked = parameter(url, "state") ?: return true
        return parameter(callback, "state") == asked
    }

    private companion object {
        /** [name]'s value among [url]'s query and fragment parameters, decoded. */
        fun parameter(url: String, name: String): String? {
            val uri = runCatching { java.net.URI(url) }.getOrNull() ?: return null
            return listOfNotNull(uri.rawQuery, uri.rawFragment)
                .flatMap { it.split('&') }
                .map { it.substringBefore('=') to it.substringAfter('=', "") }
                .firstOrNull { it.first == name }
                ?.let { java.net.URLDecoder.decode(it.second, Charsets.UTF_8) }
        }
    }
}

/** Each tracker's sync, as the engine last said it, and the sign-ins and codes waited on. */
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

    private val _signIn = MutableStateFlow<SignInRequest?>(null)

    /** The sign-in the browser is open for, AniList's or MyAnimeList's, until it comes back. */
    val signIn: StateFlow<SignInRequest?> = _signIn.asStateFlow()

    private val _deviceCode = MutableStateFlow<DeviceCodePrompt?>(null)

    /** The code Simkl or Trakt waits to be typed on another screen, until the sign-in ends. */
    val deviceCode: StateFlow<DeviceCodePrompt?> = _deviceCode.asStateFlow()

    private val _conflicts = MutableStateFlow<List<ConflictRow>>(emptyList())

    /** What the last library pull found different here and on AniList. */
    val conflicts: StateFlow<List<ConflictRow>> = _conflicts.asStateFlow()

    internal fun authorize(request: SignInRequest) {
        _signIn.value = request
    }

    /** The browser came back from [request], or was closed. */
    internal fun signInEnded(request: SignInRequest) {
        if (_signIn.value == request) _signIn.value = null
        if (tab == request) {
            tab = null
            leftForTab = false
        }
    }

    /**
     * The sign-in a browser tab is open for, until its own redirect comes back or the tab is
     * closed. Kept here rather than by the activity, so one made again while the tab is open
     * neither opens a second tab nor loses track of the first.
     */
    var tab: SignInRequest? = null
        private set

    /** The app left for the tab: coming back to it without the redirect means the tab was closed. */
    var leftForTab = false

    /** The sign-in to open a tab for now: [request], unless a tab is open for it already. */
    fun toOpen(request: SignInRequest?): SignInRequest? = request?.takeIf { it != tab }

    /** A tab was opened for [request]. */
    fun tabOpened(request: SignInRequest) {
        tab = request
        leftForTab = true
    }

    internal fun deviceCode(prompt: DeviceCodePrompt?) {
        _deviceCode.value = prompt
    }

    internal fun conflicts(rows: List<ConflictRow>) {
        _conflicts.value = rows
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

/** The plugin stores, as the engine last said them, and an invite link being checked or spent. */
class StoresModel {
    /** What a pasted invite link leads to. */
    sealed interface Preview {
        /** Being checked against the store it names. */
        data class Checking(val link: String) : Preview

        /** The store, checked against the key the link carries. */
        data class Store(val store: InvitedStore) : Preview

        /** Why it leads nowhere, in words to show in its place. */
        data class Failed(val reason: String) : Preview
    }

    /** What a store screen waits on, by the job the engine runs it as. */
    sealed interface Work {
        data object Join : Work
        data class Leave(val store: String) : Work
        data class Install(val store: String, val plugin: String) : Work
        data class Remove(val plugin: String) : Work
        data class Invite(val store: String) : Work
    }

    private val _stores = MutableStateFlow<List<StoreRow>>(emptyList())

    /** Every joined store, in the order the engine lists them. */
    val stores: StateFlow<List<StoreRow>> = _stores.asStateFlow()

    private val _loaded = MutableStateFlow(false)

    /** Whether the engine has said the stores at all. */
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    private val _preview = MutableStateFlow<Preview?>(null)

    /** The invite link being checked, and what it leads to. */
    val preview: StateFlow<Preview?> = _preview.asStateFlow()

    private val _joined = MutableStateFlow<String?>(null)

    /** The store just joined, for the screen to move into, until it is shown. */
    val joined: StateFlow<String?> = _joined.asStateFlow()

    private val _invite = MutableStateFlow<InviteLinkView?>(null)

    /** An invite link to show and send: another device of yours, from a store's page. */
    val invite: StateFlow<InviteLinkView?> = _invite.asStateFlow()

    private val _joining = MutableStateFlow<String?>(null)

    /**
     * The invite link the join sheet is open with: `""` for an empty sheet, a link for one the
     * app was opened with, `null` while it is closed.
     */
    val joining: StateFlow<String?> = _joining.asStateFlow()

    private val _working = MutableStateFlow<Map<JobId, Work>>(emptyMap())

    /** What is under way. */
    val working: StateFlow<Map<JobId, Work>> = _working.asStateFlow()

    internal fun apply(rows: List<StoreRow>) {
        _stores.value = rows
        _loaded.value = true
    }

    internal fun checking(link: String) {
        _preview.value = Preview.Checking(link)
    }

    internal fun previewed(answer: InvitePreview) {
        _preview.value = when (answer) {
            is InvitePreview.Store -> Preview.Store(answer.v1)
            is InvitePreview.Failed -> Preview.Failed(answer.reason)
        }
    }

    internal fun joined(store: String) {
        _joined.value = store
        _preview.value = null
        _joining.value = null
    }

    internal fun invited(link: InviteLinkView) {
        _invite.value = link
    }

    internal fun started(work: Work, job: JobId?) {
        job ?: return
        _working.value = _working.value + (job to work)
    }

    internal fun done(job: JobId) {
        if (job in _working.value) _working.value = _working.value - job
    }

    /** Open the join sheet, with [link] in it when the app was handed one. */
    fun openJoin(link: String = "") {
        _joining.value = link
    }

    /** Forget the link being checked: the field no longer holds an invite. */
    fun clearPreview() {
        _preview.value = null
    }

    /** The join sheet closed: forget the link being checked. */
    fun closeJoin() {
        _joining.value = null
        _preview.value = null
    }

    /** The store just joined has been shown. */
    fun clearJoined() {
        _joined.value = null
    }

    /** The invite link has been shown and closed. */
    fun clearInvite() {
        _invite.value = null
    }

    fun isWorking(working: Map<JobId, Work>, work: Work): Boolean = work in working.values

    fun store(id: String): StoreRow? = _stores.value.firstOrNull { it.id == id }
}

/** Updates waiting in this store. `StoreRow::updates`. */
val StoreRow.updates: Int get() = plugins.count { it.status is StorePluginStatus.Update }

/** What a plugin may contact, in a line: "Contacts open-archive.example and 1 more". */
val StorePluginRow.contactsLine: String
    get() {
        val first = hosts.firstOrNull() ?: return "Contacts no hosts"
        return if (hosts.size > 1) "Contacts $first and ${hosts.size - 1} more" else "Contacts $first"
    }

/** The version installed here: the one an update replaces, or the store's own. */
val StorePluginRow.installedVersion: String
    get() = (status as? StorePluginStatus.Update)?.from ?: version

/** What an invite link the app was handed holds: the store's own `https://…/join/{code}#{key}`. */
object InviteLinks {
    /**
     * The https invite in [url], when it is one: an https link as the store makes it, or one the
     * app was opened with, `anistream://join?link=<the https link>`, or the https link with the
     * app's scheme in front of its host, `anistream://plugins.example/join/{code}#{key}`.
     */
    fun invite(url: String): String? {
        val uri = runCatching { java.net.URI(url.trim()) }.getOrNull() ?: return null
        return when (uri.scheme?.lowercase()) {
            "https" -> url.trim().takeIf { (uri.rawPath ?: "").contains("/join/") }
            "anistream" -> when (uri.host) {
                "join" -> uri.rawQuery?.split('&')
                    ?.firstOrNull { it.startsWith("link=") }
                    ?.removePrefix("link=")
                    ?.let { java.net.URLDecoder.decode(it, Charsets.UTF_8) }
                    ?.let(::invite)
                "auth", null -> null
                else -> invite("https" + url.trim().substring("anistream".length))
            }
            else -> null
        }
    }

    /** Whether pasted [text] reads as an invite link, before the engine checks it. */
    fun looksLikeInvite(text: String): Boolean = invite(text) != null
}

/**
 * The downloads, as the engine last said them: the queue, what is still seeding, and each running
 * download's speed, worked out from how far it moved between two updates. The Library's Downloads
 * row reads their size.
 */
class DownloadsModel(private val clock: () -> Long = { System.currentTimeMillis() }) {
    private val _rows = MutableStateFlow<List<DownloadRow>>(emptyList())

    /** The queue, in the engine's order. */
    val rows: StateFlow<List<DownloadRow>> = _rows.asStateFlow()

    private val _loaded = MutableStateFlow(false)
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    private val _seeding = MutableStateFlow<List<SeedRow>>(emptyList())

    /** Watched episodes still seeding. */
    val seeding: StateFlow<List<SeedRow>> = _seeding.asStateFlow()

    private val _rates = MutableStateFlow<Map<Long, Double>>(emptyMap())

    /** Bytes a second for each running download, by its id. */
    val rates: StateFlow<Map<Long, Double>> = _rates.asStateFlow()

    private var last: Map<Long, Pair<ULong, Long>> = emptyMap()

    /** The bytes the downloads take on this device. */
    fun bytes(rows: List<DownloadRow>): Long = rows.sumOf { it.bytesOnDisk.toLong() }

    internal fun apply(rows: List<DownloadRow>) {
        val now = clock()
        val rates = mutableMapOf<Long, Double>()
        val seen = mutableMapOf<Long, Pair<ULong, Long>>()
        for (row in rows.filter { it.state == DownloadState.ACTIVE }) {
            seen[row.id] = row.downloaded to now
            val before = last[row.id] ?: continue
            val seconds = (now - before.second) / 1000.0
            if (seconds <= 0.2 || row.downloaded < before.first) {
                _rates.value[row.id]?.let { rates[row.id] = it }
                seen[row.id] = before
                continue
            }
            val rate = (row.downloaded - before.first).toDouble() / seconds
            // Half the last reading and half this one: steady enough to read, quick to follow.
            rates[row.id] = _rates.value[row.id]?.let { (it + rate) / 2 } ?: rate
        }
        last = seen
        _rates.value = rates
        _rows.value = rows
        _loaded.value = true
    }

    internal fun seeding(rows: List<SeedRow>) {
        _seeding.value = rows
    }

    /** Seconds until a running download finishes, at its speed. */
    fun secondsLeft(row: DownloadRow, rates: Map<Long, Double>): Long? {
        if (row.state != DownloadState.ACTIVE || row.total <= row.downloaded) return null
        val rate = rates[row.id]?.takeIf { it > 0 } ?: return null
        return kotlin.math.ceil((row.total - row.downloaded).toDouble() / rate).toLong()
    }

    companion object {
        /** The queue by title, each title where its first row is. */
        fun groups(rows: List<DownloadRow>): List<DownloadGroup> =
            rows.groupBy { it.anilistId }.map { (id, rows) -> DownloadGroup(id, rows) }

        /** Whether anything waits or runs: what keeps the download job going. */
        fun busy(rows: List<DownloadRow>): Boolean =
            rows.any { it.state == DownloadState.ACTIVE || it.state == DownloadState.QUEUED }
    }
}

/** One title's downloads. */
data class DownloadGroup(val id: tv.anistream.engine.core.AnilistId, val rows: List<DownloadRow>) {
    /** The title, as the queue names it. */
    val title: String get() = rows.firstOrNull()?.title.orEmpty()

    val bytesOnDisk: Long get() = rows.sumOf { it.bytesOnDisk.toLong() }
}

/** What a download takes on disk now: a finished one's size, and what the rest have so far. */
val DownloadRow.bytesOnDisk: ULong get() = if (state == DownloadState.DONE) maxOf(total, downloaded) else downloaded
