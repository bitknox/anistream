// What the phone and the TV say of torrents' protection and the connection check, in one place so
// both say it the same way.

package tv.anistream.store

import tv.anistream.engine.model.FailedCheck
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.TorrentProtection

object TorrentWords {
    /** What the connection check does, said wherever torrents are on. */
    const val CHECK_NOTE =
        "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, " +
            "and pauses torrents if that check fails. Your VPN or proxy is what protects you."

    /** The line over the protection choices, as torrents turn on or their protection changes. */
    const val WHY = "Other people can see your IP address while you download and share a torrent. Choose what protects it before torrents start."

    /** What turning on says before it is asked for. */
    const val OFF_NOTE = "Off by default. Turning this on asks how to protect your connection first."

    /** No protection's confirmation. */
    const val NONE_TITLE = "Use torrents without protection?"
    const val NONE_BODY = "Other people can see your IP address while you download and share a torrent, and anistream does not check the connection."

    /** Warn only's confirmation. */
    const val WARN_TITLE = "Keep torrents running if a check fails?"
    const val WARN_BODY = "anistream will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address."

    /** Edit proxy's line over its form. */
    const val EDIT_NOTE = "Torrents keep using the saved proxy until the new one passes the connection check."

    /** Under the password, on Edit proxy. */
    const val PASSWORD_KEPT = "Leave the password empty to keep the saved one."

    /** While Edit proxy's check runs. */
    const val CHECKING_PROXY = "Checking the proxy"

    /** Why an edited proxy was not saved, from the check's [view] of it. */
    fun editFailed(view: GuardView): String = when (view) {
        GuardView.Leaking -> "Not saved. Torrent traffic left outside this proxy, so the saved proxy stays."
        else -> "Not saved. The check could not be made through this proxy, so the saved proxy stays."
    }

    /** "proxy.example:1080 · as mika": where the proxy is, and who it lets in. */
    fun proxyLine(host: String, port: String, user: String): String =
        listOfNotNull("${host.trim()}:${port.trim()}", user.trim().takeIf { it.isNotEmpty() }?.let { "as $it" }).joinToString(" · ")

    /** The index field's note: what is missing, or what it is for. */
    fun indexNote(index: String): String {
        val trimmed = index.trim()
        return when {
            trimmed.isEmpty() -> "Torrents need an index address."
            !indexOk(trimmed) -> "Use an http or https address."
            else -> "The RSS search address anistream asks for torrents. anistream ships none."
        }
    }

    /** Whether [index] is an http or https address. */
    fun indexOk(index: String): Boolean = Regex("^https?://\\S+$").matches(index.trim())

    /** Whether a proxy's [host] and [port] are whole. */
    fun proxyOk(host: String, port: String): Boolean =
        host.isNotBlank() && port.trim().toIntOrNull()?.let { it in 1..65535 } == true

    /** The check's note for what a failed check does, and that none runs with no protection. */
    fun checkNote(onFailure: FailedCheck?, protection: TorrentProtection?): String = when {
        protection == TorrentProtection.NONE -> "With no protection, anistream does not check the connection."
        onFailure == FailedCheck.WARN -> CHECK_NOTE.replace("pauses torrents if that check fails", "warns you if that check fails")
        onFailure == FailedCheck.STOP -> CHECK_NOTE.replace("pauses torrents if that check fails", "stops torrents if that check fails")
        else -> CHECK_NOTE
    }

    /** "SOCKS5 proxy", "VPN on this device", "No protection". */
    fun protection(protection: TorrentProtection?): String = when (protection) {
        TorrentProtection.PROXY -> "SOCKS5 proxy"
        TorrentProtection.DEVICE, null -> "VPN on this device"
        TorrentProtection.NONE -> "No protection"
    }

    /** What each protection does, under its name. [device] is "phone", "tablet" or "TV". */
    fun protectionNote(protection: TorrentProtection, device: String): String = when (protection) {
        TorrentProtection.PROXY -> "Torrent traffic goes through a proxy you run or rent."
        TorrentProtection.DEVICE -> "Uses the VPN already running on this $device."
        TorrentProtection.NONE -> "Your address is visible to everyone in the torrent."
    }

    /** If a check fails, as its choices say it. */
    fun failed(onFailure: FailedCheck): String = when (onFailure) {
        FailedCheck.PAUSE -> "Pause torrents"
        FailedCheck.STOP -> "Stop torrents"
        FailedCheck.WARN -> "Warn only"
    }

    /** The check's verdict as a row says it: its status, the line under it, and its dot. */
    data class Check(val status: String, val note: String?, val problem: Boolean, val passed: Boolean)

    fun check(check: ProvidersModel.Check?, now: Long = System.currentTimeMillis(), onFailure: FailedCheck? = null): Check {
        // What happened to torrents on a failed check, as "If a check fails" has it.
        val then = when (onFailure) {
            FailedCheck.WARN -> "Torrents keep running."
            FailedCheck.STOP -> "Torrents are stopped."
            else -> "Torrents are paused."
        }
        val ago = check?.let {
            val seconds = ((now - it.at) / 1000).coerceAtLeast(0)
            when {
                seconds < 60 -> "$seconds s ago"
                seconds < 3_600 -> "${seconds / 60} min ago"
                seconds < 86_400 -> "${seconds / 3_600} h ago"
                else -> Wording.plural((seconds / 86_400).toInt(), "day") + " ago"
            }
        }
        return when (val view = check?.view) {
            is GuardView.Protected -> Check("Passed" + (ago?.let { " · $it" } ?: ""), "Leaving through ${view.exit}", problem = false, passed = true)
            GuardView.Checking -> Check("Checking", "Checking where torrent traffic leaves from", problem = false, passed = false)
            GuardView.Leaking -> Check("Failed", "Traffic left outside the protection. $then", problem = true, passed = false)
            GuardView.Unreachable -> Check("Failed", "The check could not be made. $then", problem = true, passed = false)
            GuardView.Unprotected -> Check("Unprotected", "No protection is set", problem = false, passed = false)
            null -> Check("Not checked yet", null, problem = false, passed = false)
        }
    }
}
