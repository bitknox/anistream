// What the device says about itself for trailers: Remove animations, power saving, heat, a screen
// reader and memory, told to the engine once and on every change.

package tv.anistream.store

import android.app.ActivityManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.view.accessibility.AccessibilityManager
import android.accessibilityservice.AccessibilityServiceInfo

/** Reads the device's state, and says when it changes. */
object DeviceWatch {
    /** A device with less than 4 GB, as the system reports memory (a 4 GB phone reports about 3.7). */
    private const val FOUR_GB_REPORTED = 3_500L * 1024 * 1024

    /** The device now. */
    fun read(context: Context): DeviceState {
        val power = context.getSystemService(PowerManager::class.java)
        val accessibility = context.getSystemService(AccessibilityManager::class.java)
        val memory = context.getSystemService(ActivityManager::class.java)
        val scale = Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
        val info = ActivityManager.MemoryInfo().also { memory?.getMemoryInfo(it) }
        return DeviceState(
            reduceMotion = scale == 0f,
            lowPower = power?.isPowerSaveMode == true,
            thermalSerious = (power?.currentThermalStatus ?: PowerManager.THERMAL_STATUS_NONE) >= PowerManager.THERMAL_STATUS_SEVERE,
            screenReader = accessibility?.isEnabled == true &&
                accessibility.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_SPOKEN).isNotEmpty(),
            lowMemory = memory?.isLowRamDevice == true || (info.totalMem in 1 until FOUR_GB_REPORTED),
        )
    }

    /** Call [changed] with the device now, and again each time it changes. Close to stop. */
    fun watch(context: Context, changed: (DeviceState) -> Unit): AutoCloseable {
        val app = context.applicationContext
        var said: DeviceState? = null
        val say = {
            val now = read(app)
            if (now != said) {
                said = now
                changed(now)
            }
        }
        val main = Handler(Looper.getMainLooper())
        val power = app.getSystemService(PowerManager::class.java)
        val accessibility = app.getSystemService(AccessibilityManager::class.java)
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) = say()
        }
        app.registerReceiver(receiver, IntentFilter(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED))
        val thermal = PowerManager.OnThermalStatusChangedListener { say() }
        power?.addThermalStatusListener(app.mainExecutor, thermal)
        val reader = AccessibilityManager.AccessibilityStateChangeListener { say() }
        accessibility?.addAccessibilityStateChangeListener(reader)
        val animations = object : ContentObserver(main) {
            override fun onChange(selfChange: Boolean) = say()
        }
        app.contentResolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, animations)
        say()
        return AutoCloseable {
            app.unregisterReceiver(receiver)
            power?.removeThermalStatusListener(thermal)
            accessibility?.removeAccessibilityStateChangeListener(reader)
            app.contentResolver.unregisterContentObserver(animations)
        }
    }
}
