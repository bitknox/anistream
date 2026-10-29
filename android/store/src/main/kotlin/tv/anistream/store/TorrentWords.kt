// What the phone and the TV say of torrents' protection and the connection check, in one place so
// both say it the same way.

package tv.anistream.store

import android.content.Context
import tv.anistream.i18n.R
import tv.anistream.i18n.Strings
import tv.anistream.engine.model.FailedCheck
import tv.anistream.engine.model.GuardView
import tv.anistream.engine.model.TorrentProtection

object TorrentWords {
    /** What the connection check does, said wherever torrents are on. */
    val CHECK_NOTE = R.string.torrents_check_note

    /** The line over the protection choices, as torrents turn on or their protection changes. */
    val WHY = R.string.torrents_risk

    /** What turning on says before it is asked for. */
    val OFF_NOTE = R.string.torrents_off_detail

    /** No protection's confirmation. */
    val NONE_TITLE = R.string.torrents_unprotected_title
    val NONE_BODY = R.string.torrents_unprotected_detail

    /** Warn only's confirmation. */
    val WARN_TITLE = R.string.torrents_keep_running_title
    val WARN_BODY = R.string.torrents_warn_body

    /** Edit proxy's line over its form. */
    val EDIT_NOTE = R.string.torrents_proxy_keeps_saved

    /** Under the password, on Edit proxy. */
    val PASSWORD_KEPT = R.string.torrents_password_keep

    /** What the torrent index is, where it is asked for, with the legal line beside it. */
    fun indexAbout(context: Context): String =
        Strings.torrentsIndexDetail(context) + " " + Strings.legalHostsNoContent(context) // i18n-skip

    /** Said once torrents' settings are saved: turned on, the protection, the index, an edited proxy. */
    val SAID_ON = R.string.torrents_on
    val SAID_PROTECTION = R.string.torrents_protection_saved
    val SAID_INDEX = R.string.torrents_index_saved
    val SAID_PROXY = R.string.torrents_proxy_saved

    /** While a proxy's check runs, before it is saved. */
    val CHECKING_PROXY = R.string.torrents_checking_proxy

    /** What a form was doing with the proxy it checked: what stays when the check fails. */
    enum class Kept { EditedProxy, Protection, TurningOn }

    /** Why a proxy was not saved, from the check's [view] of it, and what [kept] says stays. */
    fun proxyFailed(context: Context, view: GuardView, kept: Kept): String {
        val why = when (view) {
            GuardView.Leaking -> Strings.torrentsLeftOutsideProxy(context)
            else -> Strings.torrentsCheckNotMadeProxy(context)
        }
        return when (kept) {
            Kept.EditedProxy -> Strings.torrentsProxyNotSaved(context, why)
            Kept.Protection -> Strings.torrentsProtectionNotSaved(context, why)
            Kept.TurningOn -> Strings.torrentsStillOff(context, why)
        }
    }

    /** Why an edited proxy was not saved, from the check's [view] of it. */
    fun editFailed(context: Context, view: GuardView): String = proxyFailed(context, view, Kept.EditedProxy)

    /** "proxy.example:1080 · as mika": where the proxy is, and who it lets in. */
    fun proxyLine(context: Context, host: String, port: String, user: String): String =
        listOfNotNull("${host.trim()}:${port.trim()}", user.trim().takeIf { it.isNotEmpty() }?.let { Strings.torrentsProxyAs(context, it) })
            .joinToString(" · ") // i18n-skip

    /** The index field's note: what is missing, or what it is for. */
    fun indexNote(context: Context, index: String): String {
        val trimmed = index.trim()
        return when {
            trimmed.isEmpty() -> Strings.torrentsIndexNeeded(context)
            !indexOk(trimmed) -> Strings.torrentsIndexScheme(context)
            else -> indexAbout(context)
        }
    }

    /** Whether [index] is an http or https address. */
    fun indexOk(index: String): Boolean = Regex("^https?://\\S+$").matches(index.trim())

    /** Whether a proxy's [host] and [port] are whole. */
    fun proxyOk(host: String, port: String): Boolean =
        host.isNotBlank() && port.trim().toIntOrNull()?.let { it in 1..65535 } == true

    /** The check's note for what a failed check does, and that none runs with no protection. */
    fun checkNote(context: Context, onFailure: FailedCheck?, protection: TorrentProtection?): String = when {
        protection == TorrentProtection.NONE -> Strings.torrentsProtectNoneNote(context)
        onFailure == FailedCheck.WARN -> Strings.torrentsCheckNoteWarn(context)
        onFailure == FailedCheck.STOP -> Strings.torrentsCheckNoteStop(context)
        else -> Strings.torrentsCheckNote(context)
    }

    /** "SOCKS5 proxy", "VPN on this device", "No protection". */
    fun protection(context: Context, protection: TorrentProtection?): String = when (protection) {
        TorrentProtection.PROXY -> Strings.torrentsProtectProxyName(context)
        TorrentProtection.DEVICE, null -> Strings.torrentsProtectVpnName(context)
        TorrentProtection.NONE -> Strings.torrentsProtectNone(context)
    }

    /** What each protection does, under its name. */
    fun protectionNote(context: Context, protection: TorrentProtection): String = when (protection) {
        TorrentProtection.PROXY -> Strings.torrentsProtectProxyDetail(context)
        TorrentProtection.DEVICE -> Strings.torrentsProtectVpnDetail(context)
        TorrentProtection.NONE -> Strings.torrentsProtectNoneDetail(context)
    }

    /** If a check fails, as its choices say it. */
    fun failed(context: Context, onFailure: FailedCheck): String = when (onFailure) {
        FailedCheck.PAUSE -> Strings.torrentsFailPause(context)
        FailedCheck.STOP -> Strings.torrentsFailStop(context)
        FailedCheck.WARN -> Strings.torrentsFailWarn(context)
    }

    /** The check's verdict as a row says it: its status, the line under it, and its dot. */
    data class Check(val status: String, val note: String?, val problem: Boolean, val passed: Boolean)

    fun check(context: Context, check: ProvidersModel.Check?, now: Long = System.currentTimeMillis(), onFailure: FailedCheck? = null): Check {
        // What happened to torrents on a failed check, as "If a check fails" has it.
        val then = when (onFailure) {
            FailedCheck.WARN -> Strings.torrentsFailWarnResult(context)
            FailedCheck.STOP -> Strings.torrentsFailStopResult(context)
            else -> Strings.torrentsFailPauseResult(context)
        }
        val ago = check?.let {
            val seconds = ((now - it.at) / 1000).coerceAtLeast(0)
            Wording.ago(context, seconds)
        }
        return when (val view = check?.view) {
            is GuardView.Protected -> Check(
                Strings.torrentsCheckPassed(context) + (ago?.let { " · $it" } ?: ""), // i18n-skip
                Strings.torrentsCheckPassedExit(context, view.exit),
                problem = false,
                passed = true,
            )
            GuardView.Checking -> Check(Strings.torrentsCheckRunning(context), Strings.torrentsCheckRunningLine(context), problem = false, passed = false)
            GuardView.Leaking -> Check(Strings.torrentsCheckFailed(context), Strings.torrentsCheckLeaked(context, then), problem = true, passed = false)
            GuardView.Unreachable -> Check(Strings.torrentsCheckFailed(context), Strings.torrentsCheckUnmade(context, then), problem = true, passed = false)
            GuardView.Unprotected -> Check(Strings.torrentsCheckNone(context), Strings.torrentsCheckNoneLine(context), problem = false, passed = false)
            null -> Check(Strings.torrentsCheckWaitingLine(context), null, problem = false, passed = false)
        }
    }
}
