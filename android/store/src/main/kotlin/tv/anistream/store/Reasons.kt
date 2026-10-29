package tv.anistream.store

import android.content.Context
import tv.anistream.i18n.Strings
import java.util.Locale

/**
 * The first clause of an engine reason, in plain words: raw error chains never reach the screen
 * (copy rule 5). They go to Messages and the bug report whole.
 */
fun plainReason(context: Context, raw: String): String {
    val lower = raw.lowercase(Locale.ROOT)
    val anilist = "anilist" in lower
    return when {
        "timed out" in lower || "timeout" in lower ->
            if (anilist) Strings.reasonTimeoutAnilist(context) else Strings.reasonTimeoutServer(context)
        "401" in lower || "unauthori" in lower || "expired" in lower && "token" in lower -> Strings.errorSignInExpired(context)
        "429" in lower || "rate limit" in lower || "too many requests" in lower ->
            if (anilist) Strings.reasonBusyAnilist(context) else Strings.reasonBusyServer(context)
        Regex("\\b5\\d\\d\\b").containsMatchIn(lower) ->
            if (anilist) Strings.reasonProblemAnilist(context) else Strings.reasonProblemServer(context)
        "error sending request" in lower || "connection" in lower || "dns" in lower || "network" in lower ||
            "unreachable" in lower || "connect" in lower ->
            if (anilist) Strings.reasonSilentAnilist(context) else Strings.reasonSilentServer(context)
        else -> firstClause(context, raw)
    }
}

/** "could not read the list: bad json at line 3" as "Could not read the list." The words are the engine's. */
fun firstClause(context: Context, raw: String): String {
    val clause = raw.substringBefore(": ").substringBefore(" (").trim().trimEnd('.')
    if (clause.isEmpty()) return Strings.errorGeneric(context)
    return clause.replaceFirstChar { it.titlecase(Locale.getDefault()) } + "." // i18n-skip
}
