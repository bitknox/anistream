// AniList's and MyAnimeList's sign-in, in a Custom Tab: the engine asks for it with `Authorize`,
// the browser comes back to anistream://auth/<tracker> through the activity's intent filter, and
// the redirect goes to the engine as it came, with `FinishSignIn`. Closing the tab without signing
// in says so, with `CancelSignIn`.

package tv.anistream.app

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.browser.customtabs.CustomTabColorSchemeParams
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.ui.graphics.toArgb
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import tv.anistream.design.ObiSilk
import tv.anistream.store.AppStore
import tv.anistream.store.SignInRequest

/**
 * The browser half of a sign-in, for one activity. The redirect carries the code or the token
 * itself, so it is handed to the engine and never logged, kept or shown.
 */
internal class BrowserSignIn(private val activity: ComponentActivity, private val store: AppStore) {
    /** The sign-in a tab was opened for, until its redirect comes back or it is closed. */
    private var open: SignInRequest? = null

    /** Whether the activity left for the tab, so its coming back means the tab closed. */
    private var away = false

    fun start() {
        activity.lifecycleScope.launch {
            activity.repeatOnLifecycle(Lifecycle.State.STARTED) {
                store.accounts.signIn.collect { request ->
                    if (request != null && request != open) launch(request)
                }
            }
        }
    }

    private fun launch(request: SignInRequest) {
        open = request
        val colors = CustomTabColorSchemeParams.Builder()
            .setToolbarColor(ObiSilk.Ground.toArgb())
            .setNavigationBarColor(ObiSilk.Ground.toArgb())
            .build()
        val tab = CustomTabsIntent.Builder()
            .setColorScheme(CustomTabsIntent.COLOR_SCHEME_DARK)
            .setDefaultColorSchemeParams(colors)
            .setShowTitle(true)
            // No provider cookie outlives the sign-in where the browser offers that.
            .setEphemeralBrowsingEnabled(true)
            .build()
        try {
            tab.launchUrl(activity, Uri.parse(request.url))
            away = true
        } catch (_: ActivityNotFoundException) {
            open = null
            store.cancelSignIn(request)
        }
    }

    /**
     * An intent the activity was handed: when it is the redirect to anistream://auth/…, it ends
     * the open sign-in. Says whether it was one.
     */
    fun handle(intent: Intent?): Boolean {
        val data = intent?.data ?: return false
        val tracker = trackerOf(data) ?: return false
        val request = open?.takeIf { it.tracker == tracker } ?: store.accounts.signIn.value?.takeIf { it.tracker == tracker }
        open = null
        away = false
        // Taken off the intent, so a recreated activity cannot hand it over twice.
        intent.data = null
        if (request != null) store.finishSignIn(request, data.toString())
        return true
    }

    /**
     * The activity is in front again. Coming back from the tab without its redirect means the tab
     * was closed: the redirect, when there is one, arrives first, as a new intent.
     */
    fun resumed() {
        if (!away) return
        away = false
        val request = open ?: return
        activity.lifecycleScope.launch {
            // A redirect that arrives with the resume is handled first.
            delay(CLOSE_GRACE_MILLIS)
            if (open == request) {
                open = null
                store.cancelSignIn(request)
            }
        }
    }

    companion object {
        private const val SCHEME = "anistream"
        private const val CLOSE_GRACE_MILLIS = 400L

        /**
         * The tracker a sign-in's redirect is for: anistream://auth/<tracker>, or the App Link
         * https://anistream.tv/auth/<tracker>. `null` for any other link.
         */
        fun trackerOf(data: Uri): String? {
            val segments = data.pathSegments
            return when {
                data.scheme == SCHEME && data.host == "auth" -> segments.firstOrNull()
                data.scheme == "https" && data.host == APP_LINK_HOST && segments.firstOrNull() == "auth" -> segments.getOrNull(1)
                else -> null
            }?.takeIf { it.isNotEmpty() }
        }

        private const val APP_LINK_HOST = "anistream.tv"
    }
}
