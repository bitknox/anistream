// "Report a bug": the address of a pre-filled GitHub issue, which the app opens in the browser.
// The engine writes the text, the same on every front end, and strips what is private from it;
// this adds what only the app knows about itself.

package tv.anistream.store

import tv.anistream.engine.ffi.bugReportUrl
import tv.anistream.engine.model.BugError
import tv.anistream.engine.model.BugReport
import tv.anistream.engine.model.DeviceClass
import tv.anistream.engine.model.ProviderRow

/** What a bug report says about this app and this device. */
data class BugReportDevice(
    val appVersion: String,
    val osVersion: String,
    val device: DeviceClass,
    val build: String,
    val platform: String = "Android",
)

/** An error a report starts from: what kind of problem, and what it said. */
data class BugContext(
    val kind: String,
    val message: String,
    /** The title of the show it was about, which the report removes wherever it appears. */
    val title: String? = null,
)

object BugReports {
    /** The tracker's address when the engine's cannot be used. */
    const val FALLBACK = "https://github.com/bitknox/anistream/issues/new"

    /** The address for [device], the engine's sources and an optional error. */
    fun url(device: BugReportDevice, providers: List<ProviderRow>, context: BugContext?): String =
        bugReportUrl(
            BugReport(
                appVersion = device.appVersion,
                platform = device.platform,
                osVersion = device.osVersion,
                device = device.device,
                build = device.build,
                providers = providers,
                error = context?.let { BugError(kind = it.kind, message = it.message) },
                `private` = listOfNotNull(context?.title),
            ),
        )
}

/** The address of a new issue for this device, with [context] as its last error. */
fun AppStore.bugReportUrl(device: BugReportDevice, context: BugContext? = null): String =
    BugReports.url(device, providers.rows.value, context)
