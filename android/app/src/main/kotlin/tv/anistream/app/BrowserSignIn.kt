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
 * itself, so it is handed to the engine and never logged, kept or shown. The tab open, and whether
 * the app left for it, are the store's ([tv.anistream.store.AccountsModel.tab]), so an activity
 * made again while it is open carries on with it rather than opening another.
 */
internal class BrowserSignIn(private val activity: ComponentActivity, private val store: AppStore) {
    private val accounts get() = store.accounts

    fun start() {
        activity.lifecycleScope.launch {
            activity.repeatOnLifecycle(Lifecycle.State.STARTED) {
                accounts.signIn.collect { request ->
                    accounts.toOpen(request)?.let(::launch)
                }
            }
        }
    }

    private fun launch(request: SignInRequest) {
        accounts.tabOpened(request)
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
        } catch (_: ActivityNotFoundException) {
            store.cancelSignIn(request)
        }
    }

    /**
     * An intent the activity was handed: when it is a redirect to anistream://auth/…, it goes to
     * the engine, and ends the open sign-in when it is that one's own. Says whether it was one.
     */
    fun handle(intent: Intent?): Boolean {
        val data = intent?.data ?: return false
        val tracker = trackerOf(data) ?: return false
        // Taken off the intent, so a recreated activity cannot hand it over twice.
        intent.data = null
        store.redirected(tracker, data.toString())
        return true
    }

    /**
     * The activity is in front again. Coming back from the tab without a redirect means the tab
     * was closed: the redirect, when there is one, arrives first, as a new intent.
     */
    fun resumed() {
        if (!accounts.leftForTab) return
        accounts.leftForTab = false
        val request = accounts.tab ?: return
        activity.lifecycleScope.launch {
            // A redirect that arrives with the resume is handled first.
            delay(CLOSE_GRACE_MILLIS)
            if (accounts.tab == request) store.cancelSignIn(request)
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
