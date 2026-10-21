// What the player asks of the device: the power it runs on, and the screen it plays on.

package tv.anistream.player

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.PowerManager
import android.view.Display

object Device {
    /** On battery, and whether power saver is on: the upscaling chain starts a step lighter for either. */
    fun power(context: Context): PowerState {
        val battery = context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val plugged = (battery?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0) != 0
        val saver = context.getSystemService(PowerManager::class.java)?.isPowerSaveMode ?: false
        return PowerState(onBattery = battery != null && !plugged, lowPower = saver)
    }

    /**
     * The display as the engine's upscaling reads it: its physical pixels the long way round, as
     * the phone's player is always landscape, with its density and refresh rate.
     */
    fun display(display: Display, density: Float): DisplayInfo {
        val mode = display.mode
        val (w, h) = mode.physicalWidth to mode.physicalHeight
        return DisplayInfo(maxOf(w, h), minOf(w, h), density.toDouble(), mode.refreshRate.toDouble().takeIf { it > 0 })
    }
}
