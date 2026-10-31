// The screenshot tests' capture calls, with an accessibility check over the screen they draw.
//
// Compose is one View, so the Accessibility Test Framework sees none of it. This reads the
// Compose semantics tree of every window instead (the screen, and any dialog or popup over it)
// and reports what a screen reader or a keyboard would trip on:
//
//   NO-NAME           something clickable or focusable has no text, description or state to speak
//   SMALL-TARGET      something clickable is under 48 by 48 dp (phone sizes: the TV's focus
//                     targets are judged by the remote, so they are not measured)
//   PROGRESS-NO-NAME  a progress bar has no name
//   FOCUS-ONLY        something takes focus but has no action and no name
//
// The tree is read as it stands. Accessibility is not forced on, which would make Compose build
// the whole accessibility tree on every frame and stall some shots.
//
// It only reports: findings go to build/reports/a11y/semantics-<pid>.tsv in the module whose
// test ran, and CI prints them in the job summary. Set ANISTREAM_A11Y_STRICT=1 and the capture
// throws instead, so a test fails on the first screen with a finding.

@file:OptIn(InternalComposeUiApi::class)

package tv.anistream.testkit

import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.InternalComposeUiApi
import androidx.compose.ui.platform.ViewRootForTest
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsNodeInteraction
import com.github.takahirom.roborazzi.captureRoboImage as roboCapture
import com.github.takahirom.roborazzi.captureScreenRoboImage as roboScreen
import java.io.File

/** What the check found on one node of one screen. */
data class A11yFinding(val shot: String, val kind: String, val where: String, val detail: String) {
    override fun toString() = "$shot\t$kind\t$where\t$detail"
}

/** Captures the node as Roborazzi does, after checking the semantics of every window. */
fun SemanticsNodeInteraction.captureRoboImage(filePath: String) {
    A11yShots.check(filePath)
    roboCapture(filePath)
}

/** Captures the whole screen as Roborazzi does, after checking the semantics of every window. */
fun captureScreenRoboImage(filePath: String) {
    A11yShots.check(filePath)
    roboScreen(filePath)
}

object A11yShots {
    /** The smallest touch target, in dp. A pixel of rounding is allowed. */
    private const val MIN_TARGET_DP = 47.5f

    private val strict get() = System.getenv("ANISTREAM_A11Y_STRICT") == "1"

    private val out by lazy {
        File("build/reports/a11y").apply { mkdirs() }.resolve("semantics-${ProcessHandle.current().pid()}.tsv")
    }

    /** Checks the screen as it is now, for the shot that [filePath] names. */
    fun check(filePath: String) {
        val shot = File(filePath).nameWithoutExtension
        val found = try {
            findings(shot)
        } catch (t: Throwable) {
            // A check that cannot read the tree is a finding about the check, not about the app.
            listOf(A11yFinding(shot, "CHECK-FAILED", "-", t.toString()))
        }
        if (found.isEmpty()) return
        synchronized(A11yShots::class.java) { out.appendText(found.joinToString("") { "$it\n" }) }
        if (strict && found.any { it.kind != "CHECK-FAILED" }) {
            throw AssertionError("Accessibility findings in $shot:\n" + found.joinToString("\n"))
        }
    }

    private fun findings(shot: String): List<A11yFinding> {
        val owners = mutableListOf<ViewRootForTest>()
        fun find(v: View) {
            if (v is ViewRootForTest) owners += v
            if (v is ViewGroup) for (i in 0 until v.childCount) find(v.getChildAt(i))
        }
        windowRoots().forEach(::find)
        val found = mutableListOf<A11yFinding>()
        for (owner in owners) {
            val view = owner.view
            val density = view.resources.displayMetrics.density
            val phone = !isTelevision(view)
            visit(owner.semanticsOwner.rootSemanticsNode, shot, view.width.toFloat(), view.height.toFloat(), density, phone, found)
        }
        return found
    }

    /** The decor views of every window the app has open, including dialogs and popups. */
    private fun windowRoots(): List<View> {
        val global = Class.forName("android.view.WindowManagerGlobal")
        val instance = global.getMethod("getInstance").invoke(null)
        @Suppress("UNCHECKED_CAST")
        val names = global.getMethod("getViewRootNames").invoke(instance) as Array<String>
        val rootView = global.getMethod("getRootView", String::class.java)
        return names.mapNotNull { rootView.invoke(instance, it) as? View }
    }

    private fun isTelevision(view: View): Boolean {
        val ui = view.resources.configuration.uiMode and android.content.res.Configuration.UI_MODE_TYPE_MASK
        return ui == android.content.res.Configuration.UI_MODE_TYPE_TELEVISION
    }

    private fun visit(
        node: SemanticsNode, shot: String, width: Float, height: Float, density: Float, phone: Boolean,
        into: MutableList<A11yFinding>,
    ) {
        val c = node.config
        val p = SemanticsProperties
        val a = SemanticsActions
        val b = node.boundsInRoot
        val hidden = c.contains(p.InvisibleToUser) || c.contains(p.HideFromAccessibility)
        val onScreen = b.width > 0 && b.height > 0 && b.right > 0 && b.bottom > 0 && b.left < width && b.top < height
        if (!hidden && onScreen) {
            val spoken = buildList {
                c.getOrNull(p.ContentDescription)?.let { addAll(it) }
                c.getOrNull(p.Text)?.let { t -> addAll(t.map { it.text }) }
                c.getOrNull(p.EditableText)?.let { add(it.text) }
                c.getOrNull(p.StateDescription)?.let { add(it) }
            }.filter { it.isNotBlank() }
            val named = spoken.isNotEmpty()
            val clickable = c.contains(a.OnClick)
            val focusable = c.contains(a.RequestFocus) || c.contains(p.Focused)
            val isProgress = c.contains(p.ProgressBarRangeInfo)
            val where = "role=${c.getOrNull(p.Role) ?: ""} text=\"${spoken.joinToString(" | ").take(80)}\" " +
                "bounds=${b.left.toInt()},${b.top.toInt()},${b.right.toInt()},${b.bottom.toInt()}"
            fun flag(kind: String, detail: String) { into += A11yFinding(shot, kind, where, detail) }
            if ((clickable || focusable) && !named && !isProgress) flag("NO-NAME", if (clickable) "clickable" else "focusable")
            if (phone && clickable && (b.width / density < MIN_TARGET_DP || b.height / density < MIN_TARGET_DP)) {
                flag("SMALL-TARGET", "${(b.width / density).toInt()}x${(b.height / density).toInt()} dp")
            }
            if (focusable && !clickable && !c.contains(a.SetProgress) && !named) flag("FOCUS-ONLY", "focusable with no action or name")
            if (isProgress && !named) flag("PROGRESS-NO-NAME", c[p.ProgressBarRangeInfo].toString())
        }
        node.children.forEach { visit(it, shot, width, height, density, phone, into) }
    }
}
