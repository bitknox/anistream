// The time left on a sign-in code, for the line under it: counted from the code's `expires_at`.

package tv.anistream.store

import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import tv.anistream.engine.ffi.deviceCodeExpiry

object CodeCountdown {
    /** Whole seconds left of a code that stops working at [expiresAt] (Unix seconds), never below nought. */
    fun secondsLeft(expiresAt: Long, nowMillis: Long): Long = maxOf(0L, expiresAt - nowMillis / 1000)

    /** "Expires in 15 min", "Expires in 45 s" in the last minute, then "Code expired". */
    fun line(secondsLeft: Long): String = deviceCodeExpiry(secondsLeft)

    /**
     * The seconds left, now and then at each turn of the second, ending after it says nought. A
     * code that is out already says nought once, which a screen reads as "do not ask again".
     */
    fun ticks(expiresAt: Long, clock: () -> Long = System::currentTimeMillis): Flow<Long> = flow {
        while (true) {
            val now = clock()
            val left = secondsLeft(expiresAt, now)
            emit(left)
            if (left == 0L) break
            delay(1000 - now % 1000)
        }
    }
}
