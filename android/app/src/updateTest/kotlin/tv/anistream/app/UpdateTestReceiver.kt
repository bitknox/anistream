// The update test's offer (android/scripts/update-test.sh), in its builds alone: a release as the
// engine would report it, pointing at the APK the test serves from this Mac.
//
//   adb shell am broadcast -n tv.anistream.app/.UpdateTestReceiver \
//     --es version 2.0.1 --es url http://127.0.0.1:8471/test.apk --es sha256 <hex> --el size <bytes>

package tv.anistream.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import tv.anistream.store.UpdateOffer

class UpdateTestReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val app = context.applicationContext as AnistreamApp
        val offer = UpdateOffer(
            version = intent.getStringExtra("version") ?: return,
            url = intent.getStringExtra("url").orEmpty(),
            sha256 = intent.getStringExtra("sha256").orEmpty(),
            bytes = intent.getLongExtra("size", 0),
            page = intent.getStringExtra("page").orEmpty(),
        )
        app.store.updates.offer(offer, tell = intent.getBooleanExtra("notify", true))
        Log.i(AnistreamApp.TAG, "update test: offered ${offer.version}")
    }
}
