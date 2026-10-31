package tv.anistream.tv

import org.junit.Assert.assertEquals
import org.junit.Test
import tv.anistream.engine.model.Release
import tv.anistream.engine.model.ReleaseAsset

class UpdateAssetTest {
    private fun asset(name: String) = ReleaseAsset(name, "https://example.invalid/$name", "ab".repeat(32), 1uL)

    private val release = Release(
        version = "2.0.3",
        publishedAt = 0,
        pageUrl = "https://example.invalid/release",
        notes = "",
        androidTv = asset("anistream-2.0.3-android-tv-arm64.apk"),
        androidTvArmv7 = asset("anistream-2.0.3-android-tv-armv7.apk"),
    )

    @Test
    fun a32BitSystemTakesTheArmv7File() {
        val abis = arrayOf("armeabi-v7a", "armeabi")
        assertEquals("anistream-2.0.3-android-tv-armv7.apk", tvUpdateAsset(release, abis)?.name)
    }

    @Test
    fun a64BitSystemTakesTheArm64File() {
        val abis = arrayOf("arm64-v8a", "armeabi-v7a", "armeabi")
        assertEquals("anistream-2.0.3-android-tv-arm64.apk", tvUpdateAsset(release, abis)?.name)
    }
}
