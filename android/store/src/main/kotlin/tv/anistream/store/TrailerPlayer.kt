// The web view a trailer plays in, and what its page says: one view for the title page's banner,
// the Play trailer sheet and the TV's full-screen trailer.

package tv.anistream.store

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Color
import android.net.Uri
import android.view.View
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import tv.anistream.engine.ffi.trailerBaseUrl
import tv.anistream.engine.ffi.trailerEmbedPage
import tv.anistream.engine.ffi.trailerMessage
import tv.anistream.engine.ffi.trailerScript
import tv.anistream.engine.model.AppIdentity
import tv.anistream.engine.model.EmbedStyle
import tv.anistream.engine.model.Trailer
import tv.anistream.engine.model.TrailerControl
import tv.anistream.engine.model.TrailerMessage

/** What the trailer's page said, as the screens read it. */
sealed interface TrailerSignal {
    /** A frame is on screen: the banner may fade away over it. */
    data object Playing : TrailerSignal

    /** The player is up, or paused; nothing changes on screen. */
    data object Quiet : TrailerSignal

    /** Half a second before the end, where the engine's page says so: the banner comes back now. */
    data object Ending : TrailerSignal

    data object Ended : TrailerSignal

    /** The player refused it, with YouTube's code; `0` is the page's own, the player API not loading. */
    data class Failed(val code: Int) : TrailerSignal
}

/**
 * A trailer's web view, made for the system's WebView the way the trailers spec asks: the engine's
 * page loaded under the app's identifier so YouTube sees a Referer, JavaScript for the player and
 * nothing else, no touch or key reaches the player, no new windows, no cookies kept for other
 * sites and nothing kept after [release].
 *
 * Every call is made on the main thread.
 */
class TrailerPlayer(context: Context, private val trailer: Trailer, private val style: EmbedStyle, private val onSignal: (TrailerSignal) -> Unit) {
    private var released = false

    /** The view to put in the layout; transparent until the page has a picture to show. */
    val view: WebView = WebView(context)

    /** Whether the player can play this trailer at all: YouTube's only. */
    val playable: Boolean

    init {
        val app = AppIdentity(ORIGIN)
        val page = trailerEmbedPage(trailer, style, app)
        playable = page != null
        configure(app)
        if (page != null) view.loadDataWithBaseURL(trailerBaseUrl(app), page, "text/html", "utf-8", null)
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun configure(app: AppIdentity) {
        view.setBackgroundColor(Color.TRANSPARENT)
        view.isVerticalScrollBarEnabled = false
        view.isHorizontalScrollBarEnabled = false
        view.overScrollMode = View.OVER_SCROLL_NEVER
        view.isLongClickable = false
        view.isHapticFeedbackEnabled = false
        view.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
        with(view.settings) {
            javaScriptEnabled = true
            domStorageEnabled = true
            mediaPlaybackRequiresUserGesture = false
            allowFileAccess = false
            allowContentAccess = false
            setSupportMultipleWindows(false)
            javaScriptCanOpenWindowsAutomatically = false
            setSupportZoom(false)
            cacheMode = WebSettings.LOAD_DEFAULT
        }
        CookieManager.getInstance().setAcceptThirdPartyCookies(view, false)
        view.webViewClient = object : WebViewClient() {
            // The page is the only page: nothing the player does navigates the view away.
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest) = request.isForMainFrame
        }
        view.webChromeClient = object : WebChromeClient() {
            override fun onCreateWindow(view: WebView?, isDialog: Boolean, isUserGesture: Boolean, resultMsg: android.os.Message?) = false
        }
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(view, BRIDGE, setOf(app.origin)) { _, message: WebMessageCompat, _: Uri, _, _ ->
                message.data?.let(::heard)
            }
        } else {
            // No bridge to hear the page through: the banner stays and Play trailer opens the browser.
            onSignal(TrailerSignal.Failed(0))
        }
    }

    private fun heard(text: String) {
        if (released) return
        if (text.trim() == ENDING) {
            onSignal(TrailerSignal.Ending)
            return
        }
        when (val message = trailerMessage(text)) {
            TrailerMessage.Playing -> onSignal(TrailerSignal.Playing)
            TrailerMessage.Ready, TrailerMessage.Paused -> onSignal(TrailerSignal.Quiet)
            TrailerMessage.Ended -> onSignal(TrailerSignal.Ended)
            is TrailerMessage.Error -> onSignal(TrailerSignal.Failed(message.code.toInt()))
            null -> {}
        }
    }

    /** Tell the page to play, pause, mute or unmute. */
    fun control(control: TrailerControl) {
        if (!released) view.evaluateJavascript(trailerScript(control), null)
    }

    /** Stop and let go of the view's decoder, cookies and cache. */
    fun release() {
        if (released) return
        released = true
        view.stopLoading()
        view.loadUrl("about:blank")
        view.clearHistory()
        view.clearCache(true)
        CookieManager.getInstance().removeAllCookies(null)
        (view.parent as? android.view.ViewGroup)?.removeView(view)
        view.destroy()
    }

    companion object {
        /** The app's identifier as YouTube knows it: the Referer and the player's `origin`. */
        const val ORIGIN = "https://tv.anistream.app"

        /** What the engine's page posts half a second before the end, where it does. */
        const val ENDING = "ending"
    }
}

private const val BRIDGE = "anistream"
