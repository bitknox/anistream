// "Report a bug": the entry points share one way to open the pre-filled GitHub issue. It opens in
// the browser through an intent, where the reporter signs in to GitHub, writes what happened and
// submits.

package tv.anistream.ui

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.net.Uri
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import tv.anistream.engine.model.DeviceClass
import tv.anistream.store.AppStore
import tv.anistream.store.BugContext
import tv.anistream.store.BugReportDevice
import tv.anistream.store.bugReportUrl

/** What a report says of this device: the app's version, the system's, the screen's class and the build. */
internal fun bugReportDevice(context: Context): BugReportDevice {
    val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull()
    val tablet = context.resources.configuration.smallestScreenWidthDp >= 600
    val debuggable = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
    val build = when {
        context.packageName.endsWith(".fixtures") -> "fixtures"
        debuggable -> "debug"
        else -> "release"
    }
    return BugReportDevice(
        appVersion = version ?: "",
        osVersion = android.os.Build.VERSION.RELEASE ?: "",
        device = if (tablet) DeviceClass.TABLET else DeviceClass.PHONE,
        build = build,
    )
}

/** The newest problem the app has told of this session, for a report started from About. */
internal fun lastProblem(store: AppStore): BugContext? =
    store.notices.log.value.firstOrNull { it.isAlert }?.let { BugContext("message", it.notice.text) }

/** Open the bug report with an intent, with [context] as its error, or the newest problem. */
internal fun openBugReport(context: Context, store: AppStore, problem: BugContext? = null) {
    val url = store.bugReportUrl(bugReportDevice(context), problem ?: lastProblem(store))
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        context.startActivity(intent)
    } catch (_: ActivityNotFoundException) {
        // No browser: nothing to open, and the engine's link is no use to copy from here.
    }
}

/** A list that could not load: why, and a Report button that opens the bug report with it. */
@Composable
internal fun FailedQuiet(reason: String) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    Column {
        Quiet(reason)
        TextButton(
            onClick = { openBugReport(context, store, BugContext("load", reason)) },
            colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            modifier = Modifier.padding(horizontal = 8.dp),
        ) { Text("Report") }
    }
}
