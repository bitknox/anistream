// The app's own update: the newer release the engine found, downloaded into the app's own files,
// checked against the release's checksum and against the app as installed, and handed to Android's
// installer. Nothing downloads or installs until the viewer presses Update.

package tv.anistream.store

import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import tv.anistream.engine.model.Notice
import tv.anistream.engine.model.NoticeKind
import tv.anistream.engine.model.Release
import tv.anistream.engine.model.ReleaseAsset
import tv.anistream.engine.model.UpdateStatus

/**
 * A newer release of the app, as the engine found it: its APK, the APK's SHA-256 and its size,
 * and the release's page.
 */
data class UpdateOffer(
    /** The release's version, `2.0.3`. */
    val version: String,
    /** Where the APK downloads from, the release's asset; empty when the release has none. */
    val url: String,
    /** The APK's SHA-256 in hex, as the release's `.sha256` gives it. */
    val sha256: String,
    /** The APK's size in bytes, or 0 where the release does not say. */
    val bytes: Long,
    /** The release's page on GitHub, which Update opens where there is no APK to install. */
    val page: String = "",
    /** What changed, in short, as the release's notes begin. */
    val notes: String = "",
) {
    /** The notes' first line, for the notification that says it is installed. */
    val headline: String? get() = notes.lineSequence().map(String::trim).firstOrNull(String::isNotEmpty)

    /** `v2.0.3`, as the notice and the row name it. */
    val name: String get() = "v" + version.removePrefix("v")

    /** Whether there is an APK to download and check; without one, Update opens [page]. */
    val installable: Boolean get() = url.isNotEmpty() && ApkChecksum.expected(sha256) != null

    companion object {
        /**
         * What [release] offers this app: its APK, the phone's unless [asset] picks another (the
         * TV's), or its page alone where it has none.
         */
        fun of(release: Release, asset: ReleaseAsset? = release.android): UpdateOffer = UpdateOffer(
            version = release.version,
            url = asset?.url.orEmpty(),
            sha256 = asset?.sha256.orEmpty(),
            bytes = asset?.size?.toLong() ?: 0,
            page = release.pageUrl,
            notes = release.notes,
        )
    }
}

/** What the last check for a newer release said, for Settings' Check now. */
sealed interface UpdateCheck {
    /** Nothing yet this session. */
    data object Unknown : UpdateCheck

    data object Checking : UpdateCheck

    data object UpToDate : UpdateCheck

    /** A newer release: the update's state says which. */
    data object Found : UpdateCheck

    /** GitHub did not answer, in the engine's words. */
    data class Failed(val reason: String) : UpdateCheck
}

/** Why an update was not installed. Each says so in one plain sentence, which the notice shows. */
sealed interface UpdateFailure {
    val text: String

    /** The download did not finish: no connection, the server's answer, a short or long file. */
    data class Download(val reason: String?) : UpdateFailure {
        override val text get() = "The update did not download. Check the connection and try again."
    }

    /** Android did not start the download job. */
    data object NotStarted : UpdateFailure {
        override val text get() = "Android did not start the download. Try again."
    }

    /** The file is not the one the release names. */
    data object Checksum : UpdateFailure {
        override val text get() = "The download does not match the release's checksum, so it was not installed."
    }

    /** Android does not read the file as an app. */
    data object NotAnApp : UpdateFailure {
        override val text get() = "The download is not an Android app, so it was not installed."
    }

    /** The file is some other app. */
    data class Package(val found: String) : UpdateFailure {
        override val text get() = "The download is a different app, so it was not installed."
    }

    /** The file is not signed with the key the installed app is. */
    data object Signer : UpdateFailure {
        override val text get() = "The download is not signed with anistream's key, so it was not installed."
    }

    /** The file is this version or an older one. */
    data object NotNewer : UpdateFailure {
        override val text get() = "The download is not newer than this version, so it was not installed."
    }

    /** The device has too little room for the APK and the installer's copy of it. */
    data object Storage : UpdateFailure {
        override val text get() = "There isn't enough free space for the update. Free some space and try again."
    }

    /** Android's installer said no, in its own words when it gave any. */
    data class Install(val message: String?) : UpdateFailure {
        override val text get() = if (message.isNullOrBlank()) "Android did not install the update." else "Android did not install the update: ${message.trimEnd('.')}."
    }
}

/** Where an update stands. */
sealed interface UpdateState {
    /** The offer, for every state but [None]. */
    val offer: UpdateOffer? get() = null

    /** Whether it is under way: asked for, downloading, checked or with the installer. */
    val busy: Boolean get() = false

    /** No newer release is known. */
    data object None : UpdateState

    /** A newer release, and nothing under way: "Update to v2.0.3". */
    data class Available(override val offer: UpdateOffer) : UpdateState

    /**
     * Update was pressed, and Android's "Install unknown apps" is off for the app: the viewer is
     * asked to turn it on, and [opened] once its page in Android's settings is open.
     */
    data class Allowing(override val offer: UpdateOffer, val opened: Boolean = false) : UpdateState {
        override val busy get() = true
    }

    /** The APK on its way, [done] of [total] bytes; [total] is 0 while nobody has said. */
    data class Downloading(override val offer: UpdateOffer, val done: Long, val total: Long) : UpdateState {
        override val busy get() = true

        /** How far, 0 to 1, or `null` with no total. */
        val fraction: Float? get() = if (total > 0) (done.toDouble() / total).toFloat().coerceIn(0f, 1f) else null
    }

    /** The APK is down, and its checksum, package and key are being checked. */
    data class Verifying(override val offer: UpdateOffer) : UpdateState {
        override val busy get() = true
    }

    /** Android's installer has it, and asks the viewer to confirm. */
    data class Installing(override val offer: UpdateOffer) : UpdateState {
        override val busy get() = true
    }

    /** Installed: the new version starts the next time the app does. */
    data class Done(override val offer: UpdateOffer) : UpdateState

    /** Not installed, and why. Pressing Update again starts over. */
    data class Failed(override val offer: UpdateOffer, val failure: UpdateFailure) : UpdateState
}

/** What the installer said of the session the update was handed to it in. */
sealed interface InstallResult {
    data object Installed : InstallResult

    /** The viewer said no in Android's dialog. */
    data object Cancelled : InstallResult

    data class Failed(val message: String?) : InstallResult
}

/** What the update needs of Android, which the app gives it. */
interface UpdateHost {
    /** The package the APK must be: the app flavour's own, the phone's unless the host says. */
    val packageName: String get() = ApkIdentity.PACKAGE

    /** Whether there is room for [offer]'s APK and the installer's copy of it. */
    fun hasRoom(offer: UpdateOffer): Boolean = true

    /** Whether Android lets the app install packages: its "Install unknown apps" switch. */
    fun canInstall(): Boolean

    /**
     * Open Android's "Install unknown apps" page for the app, keeping [offer] until
     * [returnedFromSettings]: where Android stops the app as the switch turns on, the activity it
     * starts again finds it there ([UpdateModel.returning]).
     */
    fun openInstallSettings(offer: UpdateOffer)

    /** The viewer is back from that page, with the switch on or not. */
    fun returnedFromSettings()

    /** Start the job that downloads [offer], which runs [UpdateModel.run]; whether Android took it. */
    fun startDownload(offer: UpdateOffer): Boolean

    /** Stop that job, if it runs. */
    fun stopDownload()

    /** What the APK at [file] is, as Android reads it; `null` when it is no APK. */
    fun inspect(file: File): ApkFacts?

    /** The app as installed. */
    fun installed(): ApkFacts

    /** Hand [file] to Android's installer in a session, whose result comes to [UpdateModel.installResult]. */
    @Throws(IOException::class)
    fun install(file: File, offer: UpdateOffer)

    /** Give up the installer's session, if one is open. */
    fun abandonInstall()
}

/**
 * The update, from the release the engine found to Android's installer: available, downloading,
 * verifying, installing, then done or failed, and cancelled at any step back to available. Each
 * step moves only from the one before it, so a step that comes after a cancel changes nothing.
 * Without a host, as in the fixtures flavour and the screenshots, it only says what is offered.
 *
 * @param notices where a failure is said, as a notice.
 * @param io where the download and the checks run.
 * @param asset the file of a release this app installs: the phone's APK, or the TV's.
 */
class UpdateModel(
    private val notices: NoticesModel? = null,
    private val io: CoroutineDispatcher = Dispatchers.IO,
    private val asset: (Release) -> ReleaseAsset? = { it.android },
) {
    private val _state = MutableStateFlow<UpdateState>(UpdateState.None)
    val state: StateFlow<UpdateState> = _state.asStateFlow()

    private val _notice = MutableStateFlow<UpdateOffer?>(null)

    /** The offer to say as a notice, with Update as its button, until [told]. */
    val notice: StateFlow<UpdateOffer?> = _notice.asStateFlow()

    private val _check = MutableStateFlow<UpdateCheck>(UpdateCheck.Unknown)

    /** What the last check said. */
    val check: StateFlow<UpdateCheck> = _check.asStateFlow()

    private var host: UpdateHost? = null
    private var files: UpdateFiles? = null

    /** The newest offer, which a step under way does not take up until it ends. */
    private var latest: UpdateOffer? = null

    /** Whether this build updates itself: the app flavour, whose host is attached. */
    val updatesItself: Boolean get() = host != null

    /** Whether Update installs [offer] here, rather than opening its page. */
    fun installs(offer: UpdateOffer): Boolean = updatesItself && offer.installable

    /** The app's side, and where the APK is kept. */
    fun attach(host: UpdateHost, files: UpdateFiles) {
        this.host = host
        this.files = files
    }

    /**
     * The engine found [offer], or no newer release with `null`; [tell] says it as a notice too.
     * A step under way goes on with the release it started with.
     */
    fun offer(offer: UpdateOffer?, tell: Boolean = false) {
        latest = offer
        _check.value = if (offer == null) UpdateCheck.UpToDate else UpdateCheck.Found
        _state.update { current ->
            when {
                current.busy -> current
                offer == null -> UpdateState.None
                current is UpdateState.Done && current.offer.version == offer.version -> current
                current.offer == offer -> current
                else -> UpdateState.Available(offer)
            }
        }
        _notice.value = if (tell) offer else _notice.value?.takeIf { it == offer }
    }

    /**
     * What the engine's check said: at launch, from its day-old answer or one request, and for
     * Check now. A release is offered whenever it is the newest, and told as a notice only when
     * the engine says to.
     */
    fun apply(status: UpdateStatus) {
        when (status) {
            is UpdateStatus.Checking -> checking()
            is UpdateStatus.UpToDate -> offer(null)
            is UpdateStatus.Available -> offer(UpdateOffer.of(status.release, asset(status.release)), tell = status.notify)
            is UpdateStatus.Failed -> checkFailed(status.reason)
        }
    }

    /** The notice for [offer] is shown. */
    fun told(offer: UpdateOffer) {
        if (_notice.value == offer) _notice.value = null
    }

    /** A check is out, asked for with Check now. */
    fun checking() {
        _check.value = UpdateCheck.Checking
    }

    /** The check did not get an answer, for [reason]; what was offered stands. */
    fun checkFailed(reason: String) {
        _check.value = UpdateCheck.Failed(reason)
    }

    /**
     * The viewer pressed Update: the download starts, after "Install unknown apps" where Android
     * needs it turned on first.
     */
    fun press() {
        val host = host ?: return
        val offer = when (val current = _state.value) {
            is UpdateState.Available -> current.offer
            is UpdateState.Failed -> latest ?: current.offer
            else -> return
        }
        if (!offer.installable) return
        if (host.canInstall()) download(offer) else _state.value = UpdateState.Allowing(offer)
    }

    /** "Open settings", under the explanation: Android's page for the switch. */
    fun openInstallSettings() {
        val current = _state.value as? UpdateState.Allowing ?: return
        _state.value = current.copy(opened = true)
        host?.openInstallSettings(current.offer)
    }

    /**
     * The app started again while Android's page was open for [offer], as Android may when the
     * switch turns on: the press goes on where it was, once the app is on screen ([resumed]).
     */
    fun returning(offer: UpdateOffer) {
        if (_state.value.busy) return
        latest = latest ?: offer
        _state.value = UpdateState.Allowing(offer, opened = true)
    }

    /** "Not now", under the explanation. */
    fun notNow() {
        val current = _state.value as? UpdateState.Allowing ?: return
        _state.value = UpdateState.Available(latest ?: current.offer)
    }

    /**
     * The app is on screen again. Back from Android's page with the switch on, the download
     * starts; with it still off, the update waits for another press.
     */
    fun resumed() {
        val current = _state.value as? UpdateState.Allowing ?: return
        if (!current.opened) return
        val host = host ?: return
        host.returnedFromSettings()
        if (host.canInstall()) download(current.offer) else _state.value = UpdateState.Available(current.offer)
    }

    private fun download(offer: UpdateOffer) {
        if (host?.hasRoom(offer) == false) {
            _state.value = UpdateState.Downloading(offer, 0, offer.bytes)
            fail(offer, UpdateFailure.Storage)
            return
        }
        _state.value = UpdateState.Downloading(offer, 0, offer.bytes)
        if (host?.startDownload(offer) != true) fail(offer, UpdateFailure.NotStarted)
    }

    /** Android stopped the download job itself, as when the viewer stops it from the task manager. */
    fun stopped() {
        val current = _state.value
        if (current is UpdateState.Downloading || current is UpdateState.Verifying) {
            fail(current.offer ?: return, UpdateFailure.Download("Android stopped the download"))
        }
    }

    /**
     * Cancel, at any step: the job stops and deletes what it fetched, or the installer's session
     * goes, and the file with it.
     */
    fun cancel() {
        val current = _state.value
        if (!current.busy) return
        val offer = current.offer ?: return
        _state.value = UpdateState.Available(latest ?: offer)
        when (current) {
            is UpdateState.Downloading, is UpdateState.Verifying -> host?.stopDownload()
            is UpdateState.Installing -> {
                host?.abandonInstall()
                files?.clear()
            }
            else -> {}
        }
    }

    /**
     * The download job's work: the APK fetched into the app's files, checked, and handed to the
     * installer. It returns once the installer has the file, or the update failed or was
     * cancelled, which deletes the file. A file kept from an install the viewer said no to is
     * checked again rather than fetched again. One run at a time: a run cancelled as another
     * starts has finished with the files before the next one begins.
     */
    suspend fun run(fetch: ApkFetch): Unit = running.withLock {
        val host = host ?: return
        val files = files ?: return
        val offer = (_state.value as? UpdateState.Downloading)?.offer ?: return
        val apk = files.apk(offer)
        var handed = false
        try {
            try {
                withContext(io) {
                    if (!apk.exists()) {
                        files.keepOnly(apk)
                        fetch.fetch(offer.url, apk, offer.bytes) { done, total -> progress(offer, done, total) }
                    }
                }
            } catch (e: IOException) {
                fail(offer, UpdateFailure.Download(e.message))
                return
            }
            if (!move<UpdateState.Downloading>(offer, UpdateState.Verifying(offer))) return
            val failure = withContext(io) { verify(apk, offer, host) }
            if (failure != null) {
                fail(offer, failure)
                return
            }
            if (!move<UpdateState.Verifying>(offer, UpdateState.Installing(offer))) return
            try {
                withContext(io) { host.install(apk, offer) }
                handed = true
            } catch (e: IOException) {
                fail(offer, UpdateFailure.Install(e.message))
            } catch (e: SecurityException) {
                fail(offer, UpdateFailure.Install(e.message))
            }
        } finally {
            // Kept only while the installer has it, for another press if the viewer says no.
            if (!handed) withContext(NonCancellable + io) { files.clear() }
        }
    }

    private val running = Mutex()

    /** How far the download is, said each whole percent, or each megabyte without a total. */
    private fun progress(offer: UpdateOffer, done: Long, total: Long) {
        _state.update { current ->
            if (current !is UpdateState.Downloading || current.offer != offer) return@update current
            val said = if (total > 0) current.done * 100 / total else current.done / 1_000_000
            val now = if (total > 0) done * 100 / total else done / 1_000_000
            val whole = total in 1..done
            if (current.total == total && now == said && !whole) current else current.copy(done = done, total = total)
        }
    }

    /** The installer's answer: installed, the viewer said no, or it failed. */
    fun installResult(result: InstallResult) {
        val current = _state.value as? UpdateState.Installing ?: return
        when (result) {
            InstallResult.Installed -> {
                _state.value = UpdateState.Done(current.offer)
                files?.clear()
            }
            // Kept, so pressing Update again goes to the installer without downloading again.
            InstallResult.Cancelled -> _state.value = UpdateState.Available(latest ?: current.offer)
            is InstallResult.Failed -> fail(current.offer, UpdateFailure.Install(result.message))
        }
    }

    /** Take a step out of an [S] for [offer], and say whether it was still there to take. */
    private inline fun <reified S : UpdateState> move(offer: UpdateOffer, next: UpdateState): Boolean {
        var moved = false
        _state.update { current ->
            if (current is S && current.offer == offer) {
                moved = true
                next
            } else {
                moved = false
                current
            }
        }
        return moved
    }

    /** The update failed while under way: said as a notice, and the file deleted. */
    private fun fail(offer: UpdateOffer, failure: UpdateFailure) {
        var failed = false
        _state.update { current ->
            if (current.busy && current.offer == offer) {
                failed = true
                UpdateState.Failed(offer, failure)
            } else {
                failed = false
                current
            }
        }
        files?.clear()
        if (failed) notices?.post(Notice(kind = NoticeKind.ALERT, text = failure.text, hint = null))
    }

    companion object {
        /**
         * Whether [apk] is the release [offer] names and may replace the app [host] says is
         * installed: its checksum first, then its package, its key and its version. `null` when
         * it may.
         */
        fun verify(apk: File, offer: UpdateOffer, host: UpdateHost): UpdateFailure? {
            if (!ApkChecksum.matches(apk, offer.sha256)) return UpdateFailure.Checksum
            return ApkIdentity.check(host.inspect(apk), host.installed(), host.packageName)
        }
    }
}
