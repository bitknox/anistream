// The time the app reads its words off: "out 2h ago", today in Calendar, when a broadcast airs.

package tv.anistream.store

import java.time.Clock
import java.time.Instant
import java.time.ZoneId

/**
 * The clock every screen and its wording reads: the system's, in the device's own zone. A
 * screenshot test holds it at the moment its scene was recorded, so the same scene always draws
 * the same days and times.
 */
object AppClock {
    @Volatile
    var clock: Clock = Clock.systemDefaultZone()

    fun now(): Instant = clock.instant()

    val zone: ZoneId get() = clock.zone
}
