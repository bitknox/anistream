package tv.anistream.app

import android.app.Application
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import tv.anistream.store.UpdateOffer
import tv.anistream.ui.AppLink
import tv.anistream.ui.Destination

/**
 * The Updated notification: posted by the new version's first start when the in-app updater
 * marked that version before installing it, opening the destination it was taken from, and
 * nothing after an install by adb or by hand.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [36])
class UpdatedTest {
    private val context: Context = RuntimeEnvironment.getApplication()
    private val manager = context.getSystemService(NotificationManager::class.java)

    /** The version this test app is, as the replaced package reports it. */
    private val version = context.packageManager.getPackageInfo(context.packageName, 0).versionName!!
    private val offer = UpdateOffer(version, "https://example.invalid/a.apk", "0f".repeat(32), 1, notes = "Smoother calendar\n\nThe calendar scrolls without dropping frames.")

    private fun replaced() = UpdatedReceiver().onReceive(context, Intent(Intent.ACTION_MY_PACKAGE_REPLACED))

    @Test
    fun an_update_the_app_installed_says_so_and_opens_where_it_was() {
        AppUpdates.mark(context, offer, Destination.Library)
        replaced()
        val notification = shadowOf(manager).getNotification(UpdateNotifier.UPDATED)
        assertEquals("Updated to version $version", notification.extras.getString("android.title"))
        assertEquals("Smoother calendar", notification.extras.getCharSequence("android.text").toString())
        val opens = shadowOf(notification.contentIntent).savedIntent
        assertEquals(AppLink.Section(Destination.Library), opens.appLink())
        // Said once: the mark went with it.
        manager.cancelAll()
        replaced()
        assertNull(shadowOf(manager).getNotification(UpdateNotifier.UPDATED))
    }

    @Test
    fun an_install_by_adb_or_by_hand_says_nothing() {
        replaced()
        assertTrue(shadowOf(manager).allNotifications.isEmpty())
    }

    @Test
    fun a_mark_for_another_version_or_one_that_did_not_install_says_nothing() {
        AppUpdates.mark(context, offer.copy(version = "99.0.0"), Destination.Library)
        replaced()
        assertTrue(shadowOf(manager).allNotifications.isEmpty())
        // The viewer said no: the mark goes, and a later install by hand says nothing.
        AppUpdates.mark(context, offer, Destination.Library)
        AppUpdates.unmark(context)
        replaced()
        assertTrue(shadowOf(manager).allNotifications.isEmpty())
    }

    @Test
    fun other_broadcasts_are_not_its_own() {
        AppUpdates.mark(context, offer, Destination.Library)
        UpdatedReceiver().onReceive(context, Intent(Intent.ACTION_PACKAGE_REPLACED))
        assertTrue(shadowOf(manager).allNotifications.isEmpty())
    }
}
