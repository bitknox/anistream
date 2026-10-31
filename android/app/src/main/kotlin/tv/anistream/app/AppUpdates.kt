// The app's own update on Android, for the steps in the store's Updates.kt: Android's "Install
// unknown apps" switch, the download job, what PackageManager reads of the APK, and the
// installer's session, whose answer comes back to UpdateReceiver.

package tv.anistream.app

import tv.anistream.store.percentText
import tv.anistream.i18n.Strings
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import android.util.Log
import androidx.compose.ui.graphics.toArgb
import androidx.core.app.NotificationCompat
import java.io.File
import java.io.IOException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import tv.anistream.design.ObiSilk
import tv.anistream.store.ApkFacts
import tv.anistream.store.ApkIdentity
import tv.anistream.store.InstallResult
import tv.anistream.store.UpdateFiles
import tv.anistream.store.UpdateHost
import tv.anistream.store.UpdateModel
import tv.anistream.store.UpdateOffer
import tv.anistream.store.UpdateState
import tv.anistream.store.Wording
import tv.anistream.ui.AppLink
import tv.anistream.ui.Destination

/**
 * The update's side in Android. What needs an activity, Android's page for the switch and the
 * installer's confirmation, waits in [launch] for the app's own to start it once it is on screen.
 */
class AppUpdates(private val app: AnistreamApp, private val model: UpdateModel) : UpdateHost {
    /** Where the APK is kept: the app's own files, which nothing else can read. */
    val files = UpdateFiles(app.filesDir.resolve("updates"))

    private val preferences get() = app.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    private val _launch = MutableStateFlow<Intent?>(null)

    /** An activity to start from the app's own, once it is on screen. */
    val launch: StateFlow<Intent?> = _launch.asStateFlow()

    /** [intent] is started, or could not be. */
    fun launched(intent: Intent, started: Boolean) {
        _launch.compareAndSet(intent, null)
        if (!started) model.cancel()
    }

    /** The installer's session, while it has the update. */
    private var session: Int? = null

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
     * [KEEP_MILLIS] of it, as it may once the switch turns on. Read once.
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

    override fun startDownload(offer: UpdateOffer): Boolean {
        // The job's notification needs the permission, which is asked for the first time one does.
        app.notifier.wantPermission()
        return UpdateJob.schedule(app, offer)
    }

    override fun stopDownload() = UpdateJob.cancel(app)

    override fun inspect(file: File): ApkFacts? = app.packageManager
        .getPackageArchiveInfo(file.path, PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()))
        ?.facts()

    override fun installed(): ApkFacts = app.packageManager
        .getPackageInfo(app.packageName, PackageManager.PackageInfoFlags.of(PackageManager.GET_SIGNING_CERTIFICATES.toLong()))
        .facts()

    /**
     * Write [file] into a session of Android's installer and commit it, with [UpdateReceiver] to
     * hear how it went. The installer keeps its own copy, and asks the viewer to confirm.
     */
    override fun install(file: File, offer: UpdateOffer) {
        val installer = app.packageManager.packageInstaller
        abandonInstall()
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(app.packageName)
            setSize(file.length())
            setInstallReason(PackageManager.INSTALL_REASON_USER)
            // With UPDATE_PACKAGES_WITHOUT_USER_ACTION (the app flavour's manifest), Android lets an
            // app that installed itself update itself without its dialog, from Android 12, which
            // the app's minimum is past. Where it still wants the viewer to confirm, the session
            // says so (STATUS_PENDING_USER_ACTION) and the dialog shows as before.
            setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
        }
        val id = try {
            installer.createSession(params)
        } catch (e: RuntimeException) {
            throw IOException(e.message, e)
        }
        session = id
        mark(offer)
        try {
            installer.openSession(id).use { open ->
                file.inputStream().use { input ->
                    open.openWrite("anistream.apk", 0, file.length()).use { output ->
                        input.copyTo(output)
                        open.fsync(output)
                    }
                }
                open.commit(UpdateReceiver.status(app, id).intentSender)
            }
            Log.i(AnistreamApp.TAG, "update ${offer.version}: handed to the installer, session $id")
        } catch (e: Exception) {
            runCatching { installer.abandonSession(id) }
            session = null
            unmark()
            throw if (e is IOException) e else IOException(e.message, e)
        }
    }

    override fun abandonInstall() {
        unmark()
        val id = session ?: return
        session = null
        runCatching { app.packageManager.packageInstaller.abandonSession(id) }
    }

    /** The destination on screen, which the Updated notification comes back to. */
    var section: Destination = Destination.Home

    /**
     * Mark [offer] as the one the in-app updater is installing, with the destination it was
     * taken from: what the new version's first start tells the Updated notification by
     * ([UpdatedReceiver]). An install by adb or by hand has no mark, and no notification.
     */
    private fun mark(offer: UpdateOffer) = mark(app, offer, section)

    /** The install the mark was for did not happen: the viewer said no, it failed, or was cancelled. */
    fun unmark() = unmark(app)

    /** The installer asks the viewer to confirm, in its own dialog. */
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

    /** What the in-app updater installed, as the mark says it. */
    data class Installed(val version: String, val section: Destination, val headline: String?)

    companion object {
        private const val PREFERENCES = "updates"

        /**
         * The update the in-app updater marked before installing it, when [installed] is that
         * version and the mark is from the last [MARK_MILLIS]. Read once: the mark goes either way.
         */
        fun takeMark(context: Context, installed: String?): Installed? {
            val saved = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            val version = saved.getString(MARK_VERSION, null)
            val at = saved.getLong(MARK_AT, 0)
            val mark = version?.takeIf { it == installed && System.currentTimeMillis() - at in 0..MARK_MILLIS }?.let {
                Installed(it, saved.getString(MARK_SECTION, null)?.let(Destination::named) ?: Destination.Home, saved.getString(MARK_HEADLINE, null))
            }
            unmark(context)
            return mark
        }

        /** Mark [offer] as being installed by the in-app updater, from [section]. */
        fun mark(context: Context, offer: UpdateOffer, section: Destination) {
            context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).edit()
                .putString(MARK_VERSION, offer.version)
                .putString(MARK_SECTION, section.route)
                .putString(MARK_HEADLINE, offer.headline)
                .putLong(MARK_AT, System.currentTimeMillis())
                .commit()
        }

        fun unmark(context: Context) {
            context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).edit()
                .remove(MARK_VERSION).remove(MARK_SECTION).remove(MARK_HEADLINE).remove(MARK_AT)
                .apply()
        }

        private const val MARK_VERSION = "installingVersion"
        private const val MARK_SECTION = "installingSection"
        private const val MARK_HEADLINE = "installingHeadline"
        private const val MARK_AT = "installingAt"

        /** How long a mark waits for its install: the dialog, then the install itself. */
        private const val MARK_MILLIS = 60 * 60_000L

        private const val KEPT_VERSION = "keptVersion"
        private const val KEPT_URL = "keptUrl"
        private const val KEPT_SHA256 = "keptSha256"
        private const val KEPT_BYTES = "keptBytes"
        private const val KEPT_PAGE = "keptPage"
        private const val KEPT_AT = "keptAt"

        /** How long a press waits on Android's page. */
        private const val KEEP_MILLIS = 15 * 60_000L
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

/**
 * What the installer says of the update's session, and the job notification's Cancel. Only the
 * app's own pending intents reach it.
 */
class UpdateReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val app = context.applicationContext as AnistreamApp
        val updates = app.store.updates
        when (intent.action) {
            ACTION_CANCEL -> updates.cancel()
            ACTION_STATUS -> {
                val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
                val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
                val id = intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID, -1)
                Log.i(AnistreamApp.TAG, "update: the installer says $status for session $id${message?.let { ", $it" }.orEmpty()}")
                if (app.appUpdates?.answers(id) == false) return
                when (status) {
                    PackageInstaller.STATUS_PENDING_USER_ACTION ->
                        intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)?.let { app.appUpdates?.confirm(it) }
                    PackageInstaller.STATUS_SUCCESS -> updates.installResult(InstallResult.Installed)
                    PackageInstaller.STATUS_FAILURE_ABORTED -> {
                        app.appUpdates?.unmark()
                        updates.installResult(InstallResult.Cancelled)
                    }
                    PackageInstaller.STATUS_FAILURE_STORAGE -> {
                        app.appUpdates?.unmark()
                        updates.installResult(InstallResult.NoSpace)
                    }
                    else -> {
                        app.appUpdates?.unmark()
                        updates.installResult(InstallResult.Failed(message))
                    }
                }
            }
        }
    }

    companion object {
        const val ACTION_STATUS = "tv.anistream.action.UPDATE_STATUS"
        const val ACTION_CANCEL = "tv.anistream.action.UPDATE_CANCEL"

        /** Where the installer answers for session [id]: mutable, for the installer to fill in. */
        fun status(context: Context, id: Int): PendingIntent = PendingIntent.getBroadcast(
            context,
            id,
            Intent(ACTION_STATUS, null, context, UpdateReceiver::class.java),
            PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        fun cancel(context: Context): PendingIntent = PendingIntent.getBroadcast(
            context,
            0,
            Intent(ACTION_CANCEL, null, context, UpdateReceiver::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
    }
}

/** The update job's notification, which a user-initiated job carries: how far, and Cancel. */
class UpdateNotifier(private val context: Context) {
    init {
        context.getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, context.getString(R.string.updates_channel), NotificationManager.IMPORTANCE_LOW).apply {
                description = context.getString(R.string.updates_channel_description)
            },
        )
    }

    /** "Downloading anistream v2.0.3", "34 % · 16 MB of 48 MB", with the bar and Cancel. */
    fun progress(state: UpdateState): Notification {
        val downloading = state as? UpdateState.Downloading
        val percent = downloading?.fraction?.let { (it * 100).toInt() }
        val text = when {
            downloading == null -> Strings.updatesStartingDownload(context)
            downloading.total > 0 -> percentText(percent ?: 0) + " · " + Strings.downloadsRatioOf(context, Wording.bytes(context, downloading.done), Wording.bytes(context, downloading.total)) // i18n-skip
            else -> Wording.bytes(context, downloading.done)
        }
        val open = Intent(context, MainActivity::class.java)
            .setAction(Intent.ACTION_MAIN)
            .addCategory(Intent.CATEGORY_LAUNCHER)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        return NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            // Download progress is water, as everywhere in the app.
            .setColor(ObiSilk.Water.toArgb())
            .setOnlyAlertOnce(true)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setContentTitle(state.offer?.let { Strings.updatesDownloading(context, it.name) } ?: context.getString(R.string.updates_channel))
            .setContentText(text)
            .setProgress(100, percent ?: 0, percent == null)
            .setContentIntent(PendingIntent.getActivity(context, 0, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
            .addAction(0, context.getString(R.string.updates_cancel), UpdateReceiver.cancel(context))
            .build()
    }

    /**
     * "anistream updated to v2.0.2", with the release's first line of notes under it, opening the
     * app on the destination the update was taken from.
     */
    fun updated(installed: AppUpdates.Installed): Notification = NotificationCompat.Builder(context, CHANNEL)
        .setSmallIcon(R.drawable.ic_launcher_monochrome)
        .setColor(ObiSilk.Water.toArgb())
        .setContentTitle(Strings.updatesUpdated(context, installed.version))
        .apply { installed.headline?.let(::setContentText) }
        .setAutoCancel(true)
        .setContentIntent(
            PendingIntent.getActivity(
                context,
                UPDATED,
                context.linkIntent(AppLink.Section(installed.section)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            ),
        )
        .build()

    /** Post [updated]'s notification. Without the permission to post, Android shows nothing. */
    fun postUpdated(installed: AppUpdates.Installed) {
        context.getSystemService(NotificationManager::class.java).notify(UPDATED, updated(installed))
    }

    companion object {
        const val CHANNEL = "updates"

        /** The Updated notification's id, beside the job's 4201. */
        const val UPDATED = 4202
    }
}

/**
 * The new version's first word from Android, after the app was replaced: the Updated notification,
 * when the in-app updater marked this version before installing it, and nothing after an install
 * by adb or by hand.
 */
class UpdatedReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_MY_PACKAGE_REPLACED) return
        val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull()
        val installed = AppUpdates.takeMark(context, version) ?: return
        Log.i(AnistreamApp.TAG, "update: anistream is ${installed.version} now, from the in-app updater")
        UpdateNotifier(context).postUpdated(installed)
    }
}
