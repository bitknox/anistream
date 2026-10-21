// The app's own sections in a system trace (Perfetto, `atrace_apps`), for start-up and the main
// thread: what the platform's own sections cannot name.

package tv.anistream.store

import android.os.Trace

/** [block], as a section named "anistream: [name]" in a system trace. Costs nothing when no trace runs. */
inline fun <T> traced(name: String, block: () -> T): T {
    Trace.beginSection("anistream: $name")
    try {
        return block()
    } finally {
        Trace.endSection()
    }
}
