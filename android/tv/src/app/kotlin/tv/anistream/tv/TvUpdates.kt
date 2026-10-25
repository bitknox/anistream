// The TV's own update on Android, for the steps in the store's Updates.kt: Android's "Install
// unknown apps" switch, the download in the process's own scope, what PackageManager reads of the
// APK, and the installer's session, whose answer comes back to TvUpdateReceiver. Nothing installs
// until the viewer presses Update, and Android's installer always asks them to confirm.

package tv.anistream.tv

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import tv.anistream.store.ApkFacts
import tv.anistream.store.ApkIdentity
import tv.anistream.store.HttpApkFetch
import tv.anistream.store.InstallResult
import tv.anistream.store.UpdateFiles
import tv.anistream.store.UpdateHost
import tv.anistream.store.UpdateModel
import tv.anistream.store.UpdateOffer

class TvUpdates(
    private val app: TvApp,
    private val model: UpdateModel,
    private val scope: CoroutineScope,
) : UpdateHost, UpdateLauncher {
    /** Where the APK is kept: the app's own files, which nothing else can read. */
    val files = UpdateFiles(app.filesDir.resolve("updates"))

    private val preferences get() = app.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    private val _launch = MutableStateFlow<Intent?>(null)
    override val launch: StateFlow<Intent?> = _launch.asStateFlow()

    override fun launched(intent: Intent, started: Boolean) {
        _launch.compareAndSet(intent, null)
        if (started) return
        // Some TVs have no page for the switch under its own action: their Settings, at its top.
        if (intent.action == Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES) {
            _launch.value = Intent(Settings.ACTION_SETTINGS)
        } else if (intent.action != Settings.ACTION_SETTINGS) {
            model.cancel()
        }
    }

    override fun resumed() = model.resumed()

    /** The installer's session, while it has the update. */
    private var session: Int? = null

    /** The download, while it runs. */
    private var download: Job? = null

    override val packageName: String get() = TV_PACKAGE

    override fun hasRoom(offer: UpdateOffer): Boolean = roomForUpdate(offer.bytes, app.filesDir.usableSpace)

    override fun canInstall(): Boolean = app.packageManager.canRequestPackageInstalls()

    override fun openInstallSettings(offer: UpdateOffer) {
        preferences.edit()
            .putString(KEPT_VERSION, offer.version)
            .putString(KEPT_URL, offer.url)
            .putString(KEPT_SHA256, offer.sha256)
            .putLong(KEPT_BYTES, offer.bytes)
            .putString(KEPT_PAGE, offer.page)
            .putLong(KEPT_AT, System.currentTimeMillis())
            .apply()
        _launch.value = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${app.packageName}"))
    }

    override fun returnedFromSettings() {
        preferences.edit()
            .remove(KEPT_VERSION).remove(KEPT_URL).remove(KEPT_SHA256).remove(KEPT_BYTES).remove(KEPT_PAGE).remove(KEPT_AT)
            .apply()
    }

    /**
     * The offer Update was pressed for as Android's page opened, when the app starts again within
     * [KEEP_MILLIS] of it, as it may once the switch changes. Read once.
     */
    fun kept(): UpdateOffer? {
        val saved = preferences
        val at = saved.getLong(KEPT_AT, 0)
        val offer = saved.getString(KEPT_VERSION, null)?.let { version ->
            UpdateOffer(
                version = version,
                url = saved.getString(KEPT_URL, null).orEmpty(),
                sha256 = saved.getString(KEPT_SHA256, null).orEmpty(),
                bytes = saved.getLong(KEPT_BYTES, 0),
                page = saved.getString(KEPT_PAGE, null).orEmpty(),
            )
        }
        returnedFromSettings()
        return offer?.takeIf { System.currentTimeMillis() - at in 0..KEEP_MILLIS }
    }

    /** The download runs in the process's scope while the TV is on; leaving the app does not stop it. */
    override fun startDownload(offer: UpdateOffer): Boolean {
        download?.cancel()
        download = scope.launch { model.run(HttpApkFetch()) }
        return true
    }

    override fun stopDownload() {
        download?.cancel()
        download = null
    }

    override fun inspect(file: File): ApkFacts? {
        val manager = app.packageManager
        val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            manager.getPackageArchiveInfo(file.path, PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()))
        } else {
            manager.getPackageArchiveInfo(file.path, PackageManager.GET_SIGNING_CERTIFICATES)
        }
        return info?.facts()
    }

    override fun installed(): ApkFacts {
        val manager = app.packageManager
        val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            manager.getPackageInfo(app.packageName, PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()))
        } else {
            manager.getPackageInfo(app.packageName, PackageManager.GET_SIGNING_CERTIFICATES)
        }
        return info.facts()
    }

    /**
     * Write [file] into a session of Android's installer and commit it, with [TvUpdateReceiver]
     * to hear how it went. The installer keeps its own copy, and asks the viewer to confirm.
     */
    override fun install(file: File, offer: UpdateOffer) {
        val installer = app.packageManager.packageInstaller
        abandonInstall()
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(app.packageName)
            setSize(file.length())
            setInstallReason(PackageManager.INSTALL_REASON_USER)
        }
        val id = try {
            installer.createSession(params)
        } catch (e: RuntimeException) {
            throw IOException(e.message, e)
        }
        session = id
        try {
            installer.openSession(id).use { open ->
                file.inputStream().use { input ->
                    open.openWrite("anistream.apk", 0, file.length()).use { output ->
                        input.copyTo(output)
                        open.fsync(output)
                    }
                }
                open.commit(TvUpdateReceiver.status(app, id).intentSender)
            }
            Log.i(TvApp.TAG, "update ${offer.version}: handed to the installer, session $id")
        } catch (e: Exception) {
            runCatching { installer.abandonSession(id) }
            session = null
            throw if (e is IOException) e else IOException(e.message, e)
        }
    }

    override fun abandonInstall() {
        val id = session ?: return
        session = null
        runCatching { app.packageManager.packageInstaller.abandonSession(id) }
    }

    /** The installer asks the viewer to confirm, in its own screen. */
    fun confirm(intent: Intent) {
        _launch.value = intent
    }

    /** The answer for a session other than this process's own is an older one's: not this update's. */
    fun answers(id: Int): Boolean = session == null || session == id

    /** At every start: any APK a stopped process left, and its installer's sessions. */
    fun clearLeftovers() {
        files.clear()
        val installer = app.packageManager.packageInstaller
        for (left in installer.mySessions) runCatching { installer.abandonSession(left.sessionId) }
    }

    private companion object {
        const val PREFERENCES = "updates"
        const val KEPT_VERSION = "keptVersion"
        const val KEPT_URL = "keptUrl"
        const val KEPT_SHA256 = "keptSha256"
        const val KEPT_BYTES = "keptBytes"
        const val KEPT_PAGE = "keptPage"
        const val KEPT_AT = "keptAt"

        /** How long a press waits on Android's page. */
        const val KEEP_MILLIS = 15 * 60_000L
    }
}

/** What Android reads of an APK or the installed app, as the update's check takes it. */
internal fun PackageInfo.facts(): ApkFacts {
    val signing = signingInfo
    val current = signing?.apkContentsSigners.orEmpty().map { ApkIdentity.fingerprint(it.toByteArray()) }.toSet()
    val lineage = if (signing == null || signing.hasMultipleSigners()) current
    else signing.signingCertificateHistory.orEmpty().map { ApkIdentity.fingerprint(it.toByteArray()) }.toSet()
    return ApkFacts(packageName, longVersionCode, current, lineage)
}

/** What the installer says of the update's session. Only the app's own pending intents reach it. */
class TvUpdateReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION_STATUS) return
        val app = context.applicationContext as TvApp
        val updater = app.updater as? TvUpdates ?: return
        val updates = app.store.updates
        val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
        val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
        val id = intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID, -1)
        Log.i(TvApp.TAG, "update: the installer says $status for session $id${message?.let { ", $it" }.orEmpty()}")
        if (!updater.answers(id)) return
        when (status) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> confirmation(intent)?.let(updater::confirm)
            PackageInstaller.STATUS_SUCCESS -> updates.installResult(InstallResult.Installed)
            PackageInstaller.STATUS_FAILURE_ABORTED -> updates.installResult(InstallResult.Cancelled)
            PackageInstaller.STATUS_FAILURE_STORAGE -> updates.installResult(InstallResult.NoSpace)
            else -> updates.installResult(InstallResult.Failed(message))
        }
    }

    /** The installer's screen asking the viewer to confirm. */
    private fun confirmation(intent: Intent): Intent? = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
    } else {
        @Suppress("DEPRECATION")
        intent.getParcelableExtra(Intent.EXTRA_INTENT)
    }

    companion object {
        const val ACTION_STATUS = "tv.anistream.tv.action.UPDATE_STATUS"

        /** Where the installer answers for session [id]: mutable, for the installer to fill in. */
        fun status(context: Context, id: Int): PendingIntent = PendingIntent.getBroadcast(
            context,
            id,
            Intent(ACTION_STATUS, null, context, TvUpdateReceiver::class.java),
            PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }
}
